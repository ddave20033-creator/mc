//! Grenades: the right mouse button held pulls the pin (the other hand takes it out) and
//! keeps the grenade ready in the raised hand; let go, it is thrown, the farther the longer
//! it was held (the farthest after `FULL_POWER` seconds). Its fuse runs from the moment the
//! button went down: held `FUSE` seconds, a frag grenade goes off in the hand (a smoke
//! grenade is dropped at the feet). The spoon flies off as it leaves the hand. They fly,
//! bounce and roll. After its fuse a frag grenade explodes, hurting everything around (less behind
//! cover) and blowing blocks away; a smoke grenade pours out a thick cloud for a while.
//!
//! Every player's game flies its own copy of every grenade, but the server's copy decides the
//! blast: it breaks the blocks, hurts, and tells everyone where it went off.

use crate::audio::Sound;
use crate::client::Game;
use crate::entity::player::raycast_solid;
use crate::item::*;
use crate::item::inventory::take;
use crate::model::grenade::{RAISE_TIME, power};
use crate::net::Msg;
use crate::sim::grenade::*;
use crate::util::vertex_light;
use crate::world::mesh::Vertex;
use glam::{Mat4, Vec3};

/// The right button held (seconds): the grenade comes up in front, then the other hand
/// pulls the pin and it is ready to throw once the pin is out; held until `FULL_POWER`,
/// it is thrown the farthest (`model::grenade`, as the hands show it). How fast it leaves
/// the hand, the least and the most.
const PIN_OUT: f32 = RAISE_TIME + 0.35;
const THROW_SPEED: (f32, f32) = (6.0, 21.0);

#[derive(Default)]
pub(in crate::client) struct Grenades {
    pub(in crate::client) list: Vec<Grenade>,
    /// The view shaking after a blast near by.
    pub(in crate::client) shake: f32,
    /// The grenade in the hand being readied (the right button held).
    pub(in crate::client) hold: Option<Hold>,
    /// Where the readied grenade is in the hand: as the first-person hand shows it, and on
    /// the player model (set each frame; thrown from there).
    pub(in crate::client) hand_fp: Option<Vec3>,
    pub(in crate::client) hand_tp: Option<Vec3>,
}

/// A grenade being readied: which (the hotbar slot and the item), for how long the button
/// has been held, and whether it has been let go (it is thrown once the pin is out).
#[derive(Clone, Copy)]
pub(in crate::client) struct Hold {
    slot: usize,
    item: ItemId,
    pub(in crate::client) t: f32,
    released: bool,
}

impl Game {
    /// Holding a grenade: the right button pressed raises it and pulls the pin, held keeps it
    /// ready, let go throws it (once the pin is out; let go sooner, it is thrown as soon as
    /// the pin comes out). Putting it away, or a menu opening, before it is thrown puts the
    /// pin back.
    pub(in crate::client) fn update_grenade_hold(&mut self, dt: f32, control: bool) {
        let slot = self.me.items.hotbar_slot;
        let item = self.held();
        let kind = GrenadeKind::of(item).filter(|_| !self.spectator());
        if let Some(h) = self.tools.grenades.hold {
            if !control || h.slot != slot || h.item != item || kind.is_none() {
                self.tools.grenades.hold = None;
            }
        }
        match &mut self.tools.grenades.hold {
            None => {
                // (on a rifle station's grenade crate, the grenade goes into it instead)
                let crate_ = self.crate_under_crosshair().is_some();
                if control && kind.is_some() && self.input.right_pressed && self.me.aim.action_cooldown <= 0.0 && !crate_ {
                    self.tools.grenades.hold = Some(Hold { slot, item, t: 0.0, released: false });
                }
            }
            Some(h) => {
                let was = h.t;
                h.t += dt;
                h.released |= !self.input.right_down;
                let (t, released) = (h.t, h.released);
                // The ring is caught and the pin starts coming out.
                let pull = RAISE_TIME + 0.08;
                if was < pull && t >= pull {
                    self.audio.play(Sound::PinPull, None, 0.8);
                }
                if released && t >= PIN_OUT {
                    self.tools.grenades.hold = None;
                    self.throw_grenade(t);
                } else if t >= FUSE {
                    // Held too long: a frag grenade goes off in the hand; a smoke grenade is
                    // let go at the feet.
                    self.tools.grenades.hold = None;
                    self.grenade_in_hand_goes_off();
                }
            }
        }
        self.me.hand.grenade = self.tools.grenades.hold.map(|h| (h.t, power(h.t)));
    }

    /// The readied grenade, held `held` seconds, leaves the hand (the farther the longer it
    /// was held): the spoon flies off. A frag grenade's fuse has been running since the
    /// button went down.
    fn throw_grenade(&mut self, held: f32) {
        let Some(kind) = GrenadeKind::of(self.held()) else {
            return;
        };
        self.me.aim.action_cooldown = 0.35;
        let look = self.me.look.dir();
        let (lo, hi) = THROW_SPEED;
        let k = power(held);
        let speed = lo + (hi - lo) * k;
        // From the hand, toward what the crosshair is on.
        let pos = self.grenade_in_hand();
        let aim = (self.eye() + look * 30.0 - pos).normalize_or(look);
        let vel = aim * speed + Vec3::Y * (1.0 + 1.5 * k) + self.me.body.vel * 0.6;
        let fuse = match kind {
            GrenadeKind::Frag => (FUSE - held).max(0.05),
            GrenadeKind::Smoke => SMOKE_FUSE,
        };
        self.let_go_grenade(kind, pos, vel, fuse);
        self.me.hand.throw();
        // (the body's arm swings through too, seen from outside and by the others)
        self.me.hand.swing();
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
            GrenadeKind::Smoke => (hand, self.me.body.vel * 0.5, 0.0),
        };
        self.let_go_grenade(kind, pos, vel, fuse);
        self.me.hand.throw();
    }

    /// Where the readied grenade is: in the hand as it is seen (the first-person hand, or the
    /// player model's), unless a wall is between it and the eyes (then just in front of them).
    fn grenade_in_hand(&self) -> Vec3 {
        let eye = self.eye();
        let look = self.me.look.dir();
        let fallback = eye + look * 0.3 - Vec3::Y * 0.1;
        let hand = if self.me.look.camera.mode == 0 { self.tools.grenades.hand_fp } else { self.tools.grenades.hand_tp };
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
            let slot = self.me.items.hotbar_slot;
            take(&mut self.me.items.inventory.slots[slot], 1);
        }
        self.audio.play(Sound::SpoonFly, None, 0.6);
        // (two draws: one is only 24 bits, and the seed tells the grenades apart over LAN)
        let seed = ((self.random() * 65536.0) as u32) << 16 | (self.random() * 65536.0) as u32;
        self.spawn_grenade(kind, pos, vel, seed, fuse);
        let msg = Msg::Grenade {
            // (the server puts in who threw it)
            id: 0,
            kind: kind as u8,
            pos,
            vel,
            seed,
            fuse,
        };
        self.send(msg);
    }

    /// This game's copy of a grenade starts flying (thrown here, or by someone else), going off
    /// after `fuse` seconds: the server's copy decides the blast (`remote_blast`).
    pub(in crate::client) fn spawn_grenade(&mut self, kind: GrenadeKind, pos: Vec3, vel: Vec3, seed: u32, fuse: f32) {
        self.tools.grenades.list.push(Grenade::new(kind, pos, vel, seed, fuse));
    }

    /// Someone else threw a grenade.
    pub(in crate::client) fn remote_grenade(&mut self, kind: u8, pos: Vec3, vel: Vec3, seed: u32, fuse: f32) {
        self.spawn_grenade(GrenadeKind::from_u8(kind), pos, vel, seed, fuse);
    }

    /// The server says where a grenade went off.
    pub(in crate::client) fn remote_blast(&mut self, pos: Vec3, seed: u32) {
        self.tools.grenades.list.retain(|g| g.seed != seed);
        self.explode(pos, seed);
    }

    /// Grenades fly, bounce and go off; smoke pours out.
    pub(in crate::client) fn update_grenades(&mut self, dt: f32) {
        self.tools.grenades.shake = (self.tools.grenades.shake - dt * 2.0).max(0.0);
        let mut list = std::mem::take(&mut self.tools.grenades.list);
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
                    // (it waits for the server's word; a little longer, if the word is lost)
                    if g.fuse < -2.0 {
                        blasts.push((g.pos, g.seed));
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
        self.tools.grenades.list.extend(list);
        for (at, k) in bounces {
            self.audio.play(Sound::GrenadeBounce, Some(at), 0.3 + 0.7 * k);
        }
        // The first rush of smoke as it catches (the hiss goes on in `grenade_sounds`).
        for at in catches {
            self.audio.play(Sound::SmokePop, Some(at), 0.9);
        }
        for at in puffs {
            let (sky, blk) = self.terrain.world.light_estimate(at + Vec3::Y);
            self.level.particles.smoke_cloud(at, sky, blk);
        }
        for (pos, seed) in blasts {
            self.explode(pos, seed);
        }
    }

    /// The hiss of the smoking grenades (looping sounds, as `furnace_sounds`).
    pub(in crate::client) fn grenade_sounds(&self) -> Vec<(u64, Sound, Vec3, f32)> {
        self.tools.grenades
            .list
            .iter()
            .filter(|g| g.smoke.is_some())
            .map(|g| (1 << 63 | g.seed as u64, Sound::SmokeHiss, g.pos, 0.8))
            .collect()
    }

    /// A frag grenade goes off at `pos`: the bang, fire, smoke and flying debris, the view
    /// shaking near by. (The blocks it blows away and who it hurts are the server's.)
    fn explode(&mut self, pos: Vec3, seed: u32) {
        self.audio.play(Sound::Explosion, Some(pos), 1.0);
        let (sky, blk) = self.terrain.world.light_estimate(pos + Vec3::Y * 0.5);
        self.level.particles.explosion(pos, sky, blk);
        self.tools.guns.flash_light = (3.0, pos + Vec3::Y * 0.5);
        let near = pos.distance(self.eye());
        self.tools.grenades.shake = self.tools.grenades.shake.max((1.0 - near / 18.0).max(0.0));
        let blocks = blast_blocks(&self.terrain.world, pos, seed);
        for q in blocks.iter().take(12) {
            let b = self.terrain.world.geti(*q);
            let tint = self.block_tint(*q, b);
            self.level.particles.burst(&self.terrain.world, *q, b, 8, tint);
        }
    }

    /// This player is caught in a blast: hurt and thrown away from it.
    pub(in crate::client) fn blast_hit(&mut self, dmg: f32, from: Vec3, knock: f32) {
        let before = self.me.vitals.health;
        let dmg = self.armor_hit(dmg, crate::net::hurt::BLAST);
        self.damage(dmg, "death.explosion");
        if self.me.vitals.health < before {
            let away = (self.me.body.pos + Vec3::Y * 0.5 - from).normalize_or(Vec3::Y);
            self.me.body.vel += away * 9.0 * knock + Vec3::Y * 3.0 * knock;
        }
    }

    /// The grenades in flight or lying about.
    pub(in crate::client) fn build_grenades(&self, out: &mut Vec<Vertex>) {
        for g in &self.tools.grenades.list {
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
    use crate::world::*;
    use glam::IVec3;

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
