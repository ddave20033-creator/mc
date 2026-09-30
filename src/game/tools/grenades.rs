//! Grenades: the right mouse button held pulls the pin (the other hand takes it out) and
//! keeps the grenade ready in the raised hand; let go, it is thrown, the farther the longer
//! it was held (the farthest after `FULL_POWER` seconds). Its fuse runs from the moment the
//! button went down: held `FUSE` seconds, a frag grenade goes off in the hand (a smoke
//! grenade is dropped at the feet). The spoon flies off as it leaves the hand. They fly,
//! bounce and roll. After its fuse a frag grenade explodes, hurting everything around (less behind
//! cover) and blowing blocks away; a smoke grenade pours out a thick cloud for a while.
//!
//! On a LAN everyone flies their own copy of every grenade, but the host's copy decides the
//! blast: it breaks the blocks, hurts, and tells the others where it went off.

use crate::game::*;
use crate::audio::Sound;
use crate::entity::player::raycast_solid;
use crate::item::mining::{drops, hardness};
use crate::item::inventory::take;
use crate::item::*;
use crate::net::Msg;
use crate::util::vertex_light;
use glam::Quat;

/// Seconds from the button going down until a frag grenade explodes, and from the throw until
/// a smoke grenade starts smoking; how long it smokes.
const FUSE: f32 = 5.0;
const SMOKE_FUSE: f32 = 1.6;
const SMOKE_TIME: f32 = 16.0;
/// How far the blast blows blocks away and hurts, and the most it hurts (at the middle).
const BLAST_RADIUS: f32 = 2.8;
const HURT_RADIUS: f32 = 6.5;
const MAX_DAMAGE: f32 = 26.0;
/// Size of a grenade, for bouncing.
const RADIUS: f32 = 0.08;
/// The right button held (seconds): the grenade comes up in front, then the other hand
/// pulls the pin and it is ready to throw once the pin is out; held until `FULL_POWER`,
/// it is thrown the farthest (`model::grenade`, as the hands show it). How fast it leaves
/// the hand, the least and the most.
use crate::model::grenade::{power, RAISE_TIME};
const PIN_OUT: f32 = RAISE_TIME + 0.35;
const THROW_SPEED: (f32, f32) = (6.0, 21.0);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::game) enum GrenadeKind {
    Frag,
    Smoke,
}

impl GrenadeKind {
    pub(in crate::game) fn of(item: ItemId) -> Option<Self> {
        match item {
            FRAG_GRENADE => Some(GrenadeKind::Frag),
            SMOKE_GRENADE => Some(GrenadeKind::Smoke),
            _ => None,
        }
    }

    fn from_u8(v: u8) -> Self {
        if v == 1 {
            GrenadeKind::Smoke
        } else {
            GrenadeKind::Frag
        }
    }
}

struct Grenade {
    kind: GrenadeKind,
    pos: Vec3,
    vel: Vec3,
    rot: Quat,
    spin: Vec3,
    fuse: f32,
    /// Seconds of smoke left once a smoke grenade went off.
    smoke: Option<f32>,
    /// The copy that decides (host or single player); the others wait to be told where it
    /// went off.
    real: bool,
    /// The same on every computer: which blocks the blast takes.
    seed: u32,
    /// When the next puff of smoke comes out.
    puff: f32,
}

#[derive(Default)]
pub(in crate::game) struct Grenades {
    list: Vec<Grenade>,
    /// The view shaking after a blast near by.
    pub(in crate::game) shake: f32,
    /// The grenade in the hand being readied (the right button held).
    pub(in crate::game) hold: Option<Hold>,
    /// Where the readied grenade is in the hand: as the first-person hand shows it, and on
    /// the player model (set each frame; thrown from there).
    pub(in crate::game) hand_fp: Option<Vec3>,
    pub(in crate::game) hand_tp: Option<Vec3>,
}

/// A grenade being readied: which (the hotbar slot and the item), for how long the button
/// has been held, and whether it has been let go (it is thrown once the pin is out).
#[derive(Clone, Copy)]
pub(in crate::game) struct Hold {
    slot: usize,
    item: ItemId,
    pub(in crate::game) t: f32,
    released: bool,
}

/// A hash 0..1 of a block and a seed.
fn hash3(p: IVec3, seed: u32) -> f32 {
    let mut h = (p.x as u32).wrapping_mul(0x8da6_b343)
        ^ (p.y as u32).wrapping_mul(0xd816_3841)
        ^ (p.z as u32).wrapping_mul(0xcb1a_b31f)
        ^ seed.wrapping_mul(0x9e37_79b9);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h >> 8) as f32 / (1 << 24) as f32
}

/// Whether a blast can blow `b` away: not bedrock, obsidian, fluids or anything holding
/// things (chests, furnaces, tables, the gun station), doors or beds.
fn blastable(b: u8) -> bool {
    b != AIR
        && !is_fluid(b)
        && !is_chest(b)
        && furnace_base(b).is_none()
        && b != GUN_STATION
        && !is_gun_bench(b)
        && b != CRAFTING_TABLE
        && !is_door(b)
        && !is_bed(b)
        && hardness(b).is_some_and(|h| h < 10.0)
}

/// The blocks a blast at `pos` blows away: within a ragged ball (the same for the same seed).
fn blast_blocks(world: &World, pos: Vec3, seed: u32) -> Vec<IVec3> {
    let c = pos.floor().as_ivec3();
    let r = BLAST_RADIUS.ceil() as i32;
    let mut out = Vec::new();
    for x in -r..=r {
        for y in -r..=r {
            for z in -r..=r {
                let q = c + IVec3::new(x, y, z);
                let d = (q.as_vec3() + Vec3::splat(0.5)).distance(pos);
                let reach = BLAST_RADIUS * (0.7 + 0.4 * hash3(q, seed));
                if d < reach && blastable(world.geti(q)) {
                    out.push(q);
                }
            }
        }
    }
    out
}

/// Falls, bounces off blocks (losing speed) and rolls to a stop. Returns how hard it hit
/// something this step (blocks per second).
fn fly(g: &mut Grenade, dt: f32, world: &World) -> f32 {
    let solid = |p: Vec3| is_solid(world.get(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32));
    g.vel.y -= 22.0 * dt;
    let mut p = g.pos;
    let mut hit = 0.0f32;
    for axis in 0..3 {
        let mut q = p;
        q[axis] += g.vel[axis] * dt;
        let mut probe = q;
        probe[axis] += g.vel[axis].signum() * RADIUS;
        if !solid(probe) {
            p = q;
            continue;
        }
        hit = hit.max(g.vel[axis].abs());
        g.vel[axis] *= -0.35;
        g.spin *= 0.6;
        if axis == 1 {
            g.vel.x *= 0.75;
            g.vel.z *= 0.75;
        }
    }
    g.pos = p;
    // Rolling along the ground slows it down.
    if solid(p - Vec3::Y * (RADIUS + 0.02)) && g.vel.y.abs() < 1.0 {
        let k = (-3.5 * dt).exp();
        g.vel.x *= k;
        g.vel.z *= k;
        g.spin *= k;
    }
    let angle = g.spin.length() * dt;
    if angle > 0.0 {
        g.rot = (Quat::from_axis_angle(g.spin.normalize(), angle) * g.rot).normalize();
    }
    hit
}

impl Game {
    /// Holding a grenade: the right button pressed raises it and pulls the pin, held keeps it
    /// ready, let go throws it (once the pin is out; let go sooner, it is thrown as soon as
    /// the pin comes out). Putting it away, or a menu opening, before it is thrown puts the
    /// pin back.
    pub(in crate::game) fn update_grenade_hold(&mut self, dt: f32, control: bool) {
        let slot = self.hotbar_slot;
        let item = self.held();
        let kind = GrenadeKind::of(item).filter(|_| !self.spectator());
        if let Some(h) = self.grenades.hold {
            if !control || h.slot != slot || h.item != item || kind.is_none() {
                self.grenades.hold = None;
            }
        }
        match &mut self.grenades.hold {
            None => {
                // (on a rifle station's grenade crate, the grenade goes into it instead)
                let crate_ = self.crate_under_crosshair().is_some();
                if control && kind.is_some() && self.right_pressed && self.action_cooldown <= 0.0 && !crate_ {
                    self.grenades.hold = Some(Hold { slot, item, t: 0.0, released: false });
                }
            }
            Some(h) => {
                let was = h.t;
                h.t += dt;
                h.released |= !self.right_down;
                let (t, released) = (h.t, h.released);
                // The ring is caught and the pin starts coming out.
                let pull = RAISE_TIME + 0.08;
                if was < pull && t >= pull {
                    self.audio.play(Sound::PinPull, None, 0.8);
                }
                if released && t >= PIN_OUT {
                    self.grenades.hold = None;
                    self.throw_grenade(t);
                } else if t >= FUSE {
                    // Held too long: a frag grenade goes off in the hand; a smoke grenade is
                    // let go at the feet.
                    self.grenades.hold = None;
                    self.grenade_in_hand_goes_off();
                }
            }
        }
        self.hand.grenade = self.grenades.hold.map(|h| (h.t, power(h.t)));
    }

    /// The readied grenade, held `held` seconds, leaves the hand (the farther the longer it
    /// was held): the spoon flies off. A frag grenade's fuse has been running since the
    /// button went down.
    fn throw_grenade(&mut self, held: f32) {
        let Some(kind) = GrenadeKind::of(self.held()) else {
            return;
        };
        self.action_cooldown = 0.35;
        let look = look_dir(self.yaw, self.pitch);
        let (lo, hi) = THROW_SPEED;
        let k = power(held);
        let speed = lo + (hi - lo) * k;
        // From the hand, toward what the crosshair is on.
        let pos = self.grenade_in_hand();
        let aim = (self.player.eye() + look * 30.0 - pos).normalize_or(look);
        let vel = aim * speed + Vec3::Y * (1.0 + 1.5 * k) + self.player.vel * 0.6;
        let fuse = match kind {
            GrenadeKind::Frag => (FUSE - held).max(0.05),
            GrenadeKind::Smoke => SMOKE_FUSE,
        };
        self.let_go_grenade(kind, pos, vel, fuse);
        self.hand.throw();
        // (the body's arm swings through too, seen from outside and by the others)
        self.hand.swing();
        self.audio.play(Sound::Throw, None, 0.3 + 0.5 * k);
    }

    /// Held a frag grenade too long: it goes off right in the hand (a smoke grenade just
    /// drops at the feet and starts smoking).
    fn grenade_in_hand_goes_off(&mut self) {
        let Some(kind) = GrenadeKind::of(self.held()) else {
            return;
        };
        let hand = self.grenade_in_hand();
        let (pos, vel, fuse) = match kind {
            GrenadeKind::Frag => (hand, Vec3::ZERO, 0.0),
            GrenadeKind::Smoke => (hand, self.player.vel * 0.5, 0.0),
        };
        self.let_go_grenade(kind, pos, vel, fuse);
        self.hand.throw();
    }

    /// Where the readied grenade is: in the hand as it is seen (the first-person hand, or the
    /// player model's), unless a wall is between it and the eyes (then just in front of them).
    fn grenade_in_hand(&self) -> Vec3 {
        let eye = self.player.eye();
        let look = look_dir(self.yaw, self.pitch);
        let fallback = eye + look * 0.3 - Vec3::Y * 0.1;
        let hand = if self.camera.mode == 0 { self.grenades.hand_fp } else { self.grenades.hand_tp };
        let Some(hand) = hand.filter(|h| h.distance(eye) < 2.0) else { return fallback };
        let to = hand - eye;
        let d = to.length();
        if d > 1e-3 && raycast_solid(&self.terrain.world, eye, to / d, d + RADIUS).is_some() {
            return fallback;
        }
        hand
    }

    /// A grenade leaves the hand (one fewer in the stack; the spoon flies off) and starts
    /// flying, here and for the others.
    fn let_go_grenade(&mut self, kind: GrenadeKind, pos: Vec3, vel: Vec3, fuse: f32) {
        if !self.creative() {
            let slot = self.hotbar_slot;
            take(&mut self.inventory.slots[slot], 1);
        }
        self.audio.play(Sound::SpoonFly, None, 0.6);
        // (two draws: one is only 24 bits, and the seed tells the grenades apart over LAN)
        let seed = ((self.random() * 65536.0) as u32) << 16 | (self.random() * 65536.0) as u32;
        let real = !self.is_client();
        self.spawn_grenade(kind, pos, vel, seed, real, fuse);
        let msg = Msg::Grenade {
            id: crate::game::multi::HOST_ID,
            kind: kind as u8,
            pos,
            vel,
            seed,
            fuse,
        };
        if self.is_client() {
            self.send(msg);
        } else {
            self.broadcast(&msg, None);
        }
    }

    /// A grenade starts flying (thrown here, or by someone else: `kind` as in the message),
    /// going off after `fuse` seconds.
    pub(in crate::game) fn spawn_grenade(&mut self, kind: GrenadeKind, pos: Vec3, vel: Vec3, seed: u32, real: bool, fuse: f32) {
        let spin = Vec3::new(
            hash3(IVec3::X, seed) - 0.5,
            hash3(IVec3::Y, seed) - 0.5,
            hash3(IVec3::Z, seed) - 0.5,
        ) * 18.0;
        self.grenades.list.push(Grenade {
            kind,
            pos,
            vel,
            rot: Quat::IDENTITY,
            spin,
            fuse,
            smoke: None,
            real,
            seed,
            puff: 0.0,
        });
    }

    /// Someone else threw a grenade (host: from player `id`, which goes on to the others).
    pub(in crate::game) fn remote_grenade(&mut self, id: u8, kind: u8, pos: Vec3, vel: Vec3, seed: u32, fuse: f32) {
        let real = self.is_host();
        self.spawn_grenade(GrenadeKind::from_u8(kind), pos, vel, seed, real, fuse);
        if real {
            let msg = Msg::Grenade { id, kind, pos, vel, seed, fuse };
            self.broadcast(&msg, Some(id));
        }
    }

    /// The host says where a grenade went off.
    pub(in crate::game) fn remote_blast(&mut self, pos: Vec3, seed: u32) {
        self.grenades.list.retain(|g| g.seed != seed);
        self.explode(pos, seed, false);
    }

    /// Grenades fly, bounce and go off; smoke pours out.
    pub(in crate::game) fn update_grenades(&mut self, dt: f32) {
        self.grenades.shake = (self.grenades.shake - dt * 2.0).max(0.0);
        let mut list = std::mem::take(&mut self.grenades.list);
        let mut blasts = Vec::new();
        let mut bounces = Vec::new();
        let mut puffs = Vec::new();
        let mut catches = Vec::new();
        for g in &mut list {
            let hit = fly(g, dt, &self.terrain.world);
            if hit > 1.2 {
                bounces.push((g.pos, (hit / 10.0).min(1.0)));
            }
            g.fuse -= dt;
            match g.kind {
                GrenadeKind::Frag => {
                    // Another's copy waits for the host (a little longer, if the word is lost).
                    if g.fuse <= 0.0 && (g.real || g.fuse < -2.0) {
                        blasts.push((g.pos, g.seed, g.real));
                    }
                }
                GrenadeKind::Smoke if g.fuse <= 0.0 => {
                    if g.smoke.is_none() {
                        catches.push(g.pos);
                    }
                    let left = g.smoke.get_or_insert(SMOKE_TIME);
                    *left -= dt;
                    g.puff -= dt;
                    // Thick at first, thinning out toward the end.
                    let rate = if *left > 4.0 { 0.05 } else { 0.12 };
                    while g.puff <= 0.0 {
                        g.puff += rate;
                        puffs.push(g.pos + Vec3::Y * 0.1);
                    }
                }
                GrenadeKind::Smoke => {}
            }
        }
        list.retain(|g| {
            let blown = g.kind == GrenadeKind::Frag && blasts.iter().any(|b| b.1 == g.seed);
            !blown && g.smoke.is_none_or(|s| s > 0.0)
        });
        self.grenades.list.extend(list);
        for (at, k) in bounces {
            self.audio.play(Sound::GrenadeBounce, Some(at), 0.3 + 0.7 * k);
        }
        // The first rush of smoke as it catches (the hiss goes on in `grenade_sounds`).
        for at in catches {
            self.audio.play(Sound::SmokePop, Some(at), 0.9);
        }
        for at in puffs {
            let (sky, blk) = self.terrain.world.light_estimate(at + Vec3::Y);
            self.particles.smoke_cloud(at, sky, blk);
        }
        for (pos, seed, real) in blasts {
            self.explode(pos, seed, real);
            if real && self.is_host() {
                self.broadcast(&Msg::Blast { pos, seed }, None);
            }
        }
    }

    /// The hiss of the smoking grenades (looping sounds, as `furnace_sounds`).
    pub(in crate::game) fn grenade_sounds(&self) -> Vec<(u64, Sound, Vec3, f32)> {
        self.grenades
            .list
            .iter()
            .filter(|g| g.smoke.is_some())
            .map(|g| (1 << 63 | g.seed as u64, Sound::SmokeHiss, g.pos, 0.8))
            .collect()
    }

    /// A frag grenade goes off at `pos`: the bang, fire, smoke and flying debris, the view
    /// shaking near by; and if this is the copy that decides (`real`), the blocks go and
    /// everything around gets hurt.
    fn explode(&mut self, pos: Vec3, seed: u32, real: bool) {
        self.audio.play(Sound::Explosion, Some(pos), 1.0);
        let (sky, blk) = self.terrain.world.light_estimate(pos + Vec3::Y * 0.5);
        self.particles.explosion(pos, sky, blk);
        self.guns.flash_light = (3.0, pos + Vec3::Y * 0.5);
        let near = pos.distance(self.player.eye());
        self.grenades.shake = self.grenades.shake.max((1.0 - near / 18.0).max(0.0));

        let blocks = blast_blocks(&self.terrain.world, pos, seed);
        for q in blocks.iter().take(12) {
            let b = self.terrain.world.geti(*q);
            let tint = self.block_tint(*q, b);
            self.particles.burst(&self.terrain.world, *q, b, 8, tint);
        }
        if !real {
            return;
        }
        let pick = tool_id(ToolKind::Pickaxe, Tier::Diamond);
        for q in &blocks {
            let b = self.terrain.world.geti(*q);
            self.set_block(*q, AIR);
            // About a third of what is blown away drops.
            if self.random() < 0.3 {
                let r = self.random();
                for s in drops(b, pick, r) {
                    self.spawn_drop(q.as_vec3() + Vec3::splat(0.5), s);
                }
            }
        }
        for q in &blocks {
            self.block_updated(*q);
        }
        self.blast_hurt(pos);
    }

    /// Everyone near a blast gets hurt, less the farther and behind cover, and thrown away.
    fn blast_hurt(&mut self, pos: Vec3) {
        let world = &self.terrain.world;
        let hurt = |target: Vec3| -> Option<(f32, f32)> {
            let d = target.distance(pos);
            if d >= HURT_RADIUS {
                return None;
            }
            let k = (1.0 - d / HURT_RADIUS).powf(1.4);
            let dir = (target - pos).normalize_or(Vec3::Y);
            let covered = raycast_solid(world, pos + dir * 0.3, dir, (d - 0.5).max(0.0)).is_some();
            let cover = if covered { 0.35 } else { 1.0 };
            Some((MAX_DAMAGE * k * cover, 1.6 * k * cover))
        };
        let me = hurt(self.player.pos + Vec3::Y * 0.9);
        let mobs: Vec<(usize, f32, f32)> = self
            .mobs
            .iter()
            .enumerate()
            .filter(|(_, m)| m.alive())
            .filter_map(|(i, m)| hurt(m.center()).map(|(d, k)| (i, d, k)))
            .collect();
        let others: Vec<(u8, f32, f32)> = self
            .remote_positions()
            .into_iter()
            .filter_map(|(id, p)| hurt(p + Vec3::Y * 0.9).map(|(d, k)| (id, d, k)))
            .collect();
        if let Some((dmg, knock)) = me {
            self.blast_hit(dmg, pos, knock);
        }
        for (i, dmg, knock) in mobs {
            self.mobs[i].hurt(dmg, Some(pos), knock);
        }
        for (id, dmg, knock) in others {
            self.send_to(
                id,
                &Msg::Hurt {
                    dmg,
                    from: pos,
                    knock,
                    kind: crate::net::hurt::BLAST,
                },
            );
        }
    }

    /// This player is caught in a blast: hurt and thrown away from it.
    pub(in crate::game) fn blast_hit(&mut self, dmg: f32, from: Vec3, knock: f32) {
        let before = self.health;
        let dmg = self.armor_hit(dmg, crate::net::hurt::BLAST);
        self.damage(dmg, "death.explosion");
        if self.health < before {
            let away = (self.player.pos + Vec3::Y * 0.5 - from).normalize_or(Vec3::Y);
            self.player.vel += away * 9.0 * knock + Vec3::Y * 3.0 * knock;
        }
    }

    /// The grenades in flight or lying about.
    pub(in crate::game) fn build_grenades(&self, out: &mut Vec<Vertex>) {
        for g in &self.grenades.list {
            let (sky, blk) = self.terrain.world.light_estimate(g.pos);
            let light = vertex_light(sky, blk);
            let m = Mat4::from_rotation_translation(g.rot, g.pos);
            let fl = crate::world::mesh::flags::ENTITY;
            // The Blockbench grenades, as big as the old ones were.
            let smoke = matches!(g.kind, GrenadeKind::Smoke);
            use crate::model::grenade::{emit, sized, Look};
            emit(out, smoke, sized(smoke, m, 0.2), Look::THROWN, light, fl);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blast_takes_the_same_ragged_ball_everywhere() {
        let a: Vec<f32> = (0..20).map(|i| hash3(IVec3::new(i, 2, -i), 77)).collect();
        let b: Vec<f32> = (0..20).map(|i| hash3(IVec3::new(i, 2, -i), 77)).collect();
        assert_eq!(a, b);
        assert!(a.iter().all(|v| (0.0..1.0).contains(v)));
        assert!(blastable(STONE) && blastable(DIRT) && blastable(GRASS));
        assert!(!blastable(BEDROCK) && !blastable(OBSIDIAN) && !blastable(WATER));
        assert!(!blastable(GUN_STATION) && !blastable(CRAFTING_TABLE) && !blastable(AIR));
    }
}
