//! Grenades: thrown with the right mouse button (softly while sneaking), they fly, bounce
//! and roll. After its fuse a frag grenade explodes, hurting everything around (less behind
//! cover) and blowing blocks away; a smoke grenade pours out a thick cloud for a while.
//!
//! On a LAN everyone flies their own copy of every grenade, but the host's copy decides the
//! blast: it breaks the blocks, hurts, and tells the others where it went off.

use super::*;
use crate::audio::Sound;
use crate::entity::player::raycast_solid;
use crate::item::mining::{drops, hardness};
use crate::item::inventory::take;
use crate::item::*;
use crate::model::emit_box;
use crate::net::Msg;
use crate::util::vertex_light;
use crate::world::textures::tex;
use glam::Quat;

/// Seconds from the throw until a frag grenade explodes, and until a smoke grenade starts
/// smoking; how long it smokes.
const FUSE: f32 = 3.2;
const SMOKE_FUSE: f32 = 1.6;
const SMOKE_TIME: f32 = 16.0;
/// How far the blast blows blocks away and hurts, and the most it hurts (at the middle).
const BLAST_RADIUS: f32 = 2.8;
const HURT_RADIUS: f32 = 6.5;
const MAX_DAMAGE: f32 = 26.0;
/// Size of a grenade, for bouncing.
const RADIUS: f32 = 0.08;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum GrenadeKind {
    Frag,
    Smoke,
}

impl GrenadeKind {
    pub(super) fn of(item: ItemId) -> Option<Self> {
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
pub(super) struct Grenades {
    list: Vec<Grenade>,
    /// The view shaking after a blast near by.
    pub(super) shake: f32,
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
    /// Right click with a grenade: pulls the pin and throws it (a short lob while sneaking).
    /// False if not holding one.
    pub(super) fn throw_grenade(&mut self) -> bool {
        let Some(kind) = GrenadeKind::of(self.held()) else {
            return false;
        };
        if self.action_cooldown > 0.0 {
            return true;
        }
        self.action_cooldown = 0.6;
        let look = look_dir(self.yaw, self.pitch);
        let speed = if self.sneaking() { 7.0 } else { 17.0 };
        let pos = self.player.eye() + look * 0.35 - Vec3::Y * 0.1;
        let vel = look * speed + Vec3::Y * 2.5 + self.player.vel * 0.6;
        if !self.creative() {
            let slot = self.hotbar_slot;
            take(&mut self.inventory.slots[slot], 1);
        }
        self.hand.swing();
        self.audio.play(Sound::PinPull, None, 0.7);
        self.audio.play(Sound::Throw, None, 0.8);
        let seed = (self.random() * u32::MAX as f32) as u32;
        let real = !self.is_client();
        self.spawn_grenade(kind, pos, vel, seed, real);
        let msg = Msg::Grenade {
            id: super::multi::HOST_ID,
            kind: kind as u8,
            pos,
            vel,
            seed,
        };
        if self.is_client() {
            self.send(msg);
        } else {
            self.broadcast(&msg, None);
        }
        true
    }

    /// A grenade starts flying (thrown here, or by someone else: `kind` as in the message).
    pub(super) fn spawn_grenade(&mut self, kind: GrenadeKind, pos: Vec3, vel: Vec3, seed: u32, real: bool) {
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
            fuse: match kind {
                GrenadeKind::Frag => FUSE,
                GrenadeKind::Smoke => SMOKE_FUSE,
            },
            smoke: None,
            real,
            seed,
            puff: 0.0,
        });
    }

    /// Someone else threw a grenade (host: from player `id`, which goes on to the others).
    pub(super) fn remote_grenade(&mut self, id: u8, kind: u8, pos: Vec3, vel: Vec3, seed: u32) {
        let real = self.is_host();
        self.spawn_grenade(GrenadeKind::from_u8(kind), pos, vel, seed, real);
        if real {
            let msg = Msg::Grenade { id, kind, pos, vel, seed };
            self.broadcast(&msg, Some(id));
        }
    }

    /// The host says where a grenade went off.
    pub(super) fn remote_blast(&mut self, pos: Vec3, seed: u32) {
        self.grenades.list.retain(|g| g.seed != seed);
        self.explode(pos, seed, false);
    }

    /// Grenades fly, bounce and go off; smoke pours out.
    pub(super) fn update_grenades(&mut self, dt: f32) {
        self.grenades.shake = (self.grenades.shake - dt * 2.0).max(0.0);
        let mut list = std::mem::take(&mut self.grenades.list);
        let mut blasts = Vec::new();
        let mut bounces = Vec::new();
        let mut puffs = Vec::new();
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
    pub(super) fn grenade_sounds(&self) -> Vec<(u64, Sound, Vec3, f32)> {
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
    pub(super) fn blast_hit(&mut self, dmg: f32, from: Vec3, knock: f32) {
        let before = self.health;
        let dmg = self.armor_hit(dmg, crate::net::hurt::BLAST);
        self.damage(dmg, "death.explosion");
        if self.health < before {
            let away = (self.player.pos + Vec3::Y * 0.5 - from).normalize_or(Vec3::Y);
            self.player.vel += away * 9.0 * knock + Vec3::Y * 3.0 * knock;
        }
    }

    /// The grenades in flight or lying about.
    pub(super) fn build_grenades(&self, out: &mut Vec<Vertex>) {
        for g in &self.grenades.list {
            let (sky, blk) = self.terrain.world.light_estimate(g.pos);
            let light = vertex_light(sky, blk);
            let m = Mat4::from_rotation_translation(g.rot, g.pos);
            let fl = crate::world::mesh::flags::ENTITY;
            let v = Vec3::new;
            let steel = [tex::GUN_STEEL; 6];
            match g.kind {
                GrenadeKind::Frag => {
                    let olive = [[98, 110, 64]; 6];
                    emit_box(out, m, v(-0.055, -0.07, -0.055), v(0.055, 0.06, 0.055), [tex::WOOL; 6], olive, light, fl);
                    emit_box(out, m, v(-0.045, 0.06, -0.045), v(0.045, 0.075, 0.045), [tex::WOOL; 6], olive, light, fl);
                    emit_box(out, m, v(-0.02, 0.075, -0.02), v(0.02, 0.105, 0.02), steel, [[200; 3]; 6], light, fl);
                }
                GrenadeKind::Smoke => {
                    let gray = [[130, 136, 140]; 6];
                    emit_box(out, m, v(-0.045, -0.085, -0.045), v(0.045, 0.075, 0.045), steel, gray, light, fl);
                    emit_box(out, m, v(-0.047, 0.0, -0.047), v(0.047, 0.03, 0.047), [tex::WOOL; 6], [[210, 60, 50]; 6], light, fl);
                    emit_box(out, m, v(-0.02, 0.075, -0.02), v(0.02, 0.1, 0.02), steel, [[200; 3]; 6], light, fl);
                }
            }
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
