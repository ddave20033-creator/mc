//! The target dummy: a static mob. It stands where it was set up (facing whoever set it
//! up), rocks when hit and counts the damage it takes, which is shown over it; it never dies.
//! A sneaking hit takes it down (it drops as its item).

use super::*;
use crate::item::TARGET_DUMMY as DUMMY_ITEM;
use glam::{Mat4, Vec2};
use std::f32::consts::{FRAC_PI_2, TAU};

pub const TARGET_DUMMY: MobKind = MobKind(2);

pub const DEF: MobDef = MobDef {
    kind: TARGET_DUMMY,
    key: "target_dummy",
    en: "Target Dummy",
    hu: "Gyakorlóbábu",
    health: 20.0,
    size: (0.35, 1.95),
    model,
    egg: DUMMY_ITEM,
    state: || MobState::Dummy(Tally::default()),
    hooks: Hooks { tick: Some(rock), hurt: Some(hit), ..NO_HOOKS },
    ..STATIC
};

/// Seconds without a hit after which a dummy starts counting from zero again.
pub const DUMMY_RESET: f32 = 6.0;

/// What a dummy keeps count of, and how it rocks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tally {
    /// The damage it has taken since it was last left alone for `DUMMY_RESET` seconds, the
    /// last hit, and seconds since that hit.
    pub taken: f32,
    pub last_hit: f32,
    pub since_hit: f32,
    /// How far its body is tipped (radians toward model +X and +Z), and how fast.
    pub tilt: Vec2,
    tilt_vel: Vec2,
}

impl Default for Tally {
    fn default() -> Self {
        Tally { taken: 0.0, last_hit: 0.0, since_hit: f32::MAX, tilt: Vec2::ZERO, tilt_vel: Vec2::ZERO }
    }
}

impl Tally {
    /// (a dummy has no legs: the limb values carry how it is tipped)
    pub fn to_net(&self, net: &mut crate::net::MobNet) {
        net.taken = self.taken;
        net.last_hit = self.last_hit;
        net.limb_swing = self.tilt.x;
        net.limb_amount = self.tilt.y;
    }

    pub fn apply_net(&mut self, net: &crate::net::MobNet) {
        if net.taken != self.taken {
            self.since_hit = if net.taken > 0.0 { 0.0 } else { f32::MAX };
        }
        self.taken = net.taken;
        self.last_hit = net.last_hit;
    }

    /// A player's copy, gliding: tipped as the limbs say, and counting time since the hit.
    pub fn follow(&mut self, limb_swing: f32, limb_amount: f32, dt: f32) {
        self.tilt = Vec2::new(limb_swing, limb_amount);
        self.since_hit += dt;
    }
}

impl Mob {
    /// A target dummy's tally.
    pub fn tally(&self) -> Option<&Tally> {
        match &self.state {
            MobState::Dummy(t) => Some(t),
            _ => None,
        }
    }
}

/// A hit: every hit counts (it is never out of reach for a moment like a hurt animal), and
/// it rocks away from where the hit came from.
fn hit(m: &mut Mob, amount: f32, from: Option<Vec3>, knockback: f32) -> bool {
    let away = from
        .and_then(|src| ((m.center() - src) * Vec3::new(1.0, 0.0, 1.0)).try_normalize())
        .unwrap_or_else(|| {
            let a = m.rand() * TAU;
            Vec3::new(a.cos(), 0.0, a.sin())
        });
    // Into model space (the model is turned by `FRAC_PI_2 - body_yaw`, see `model`).
    let local = Mat4::from_rotation_y(m.body_yaw - FRAC_PI_2).transform_vector3(away);
    let MobState::Dummy(t) = &mut m.state else {
        return false;
    };
    if t.since_hit > DUMMY_RESET {
        t.taken = 0.0;
    }
    t.taken += amount;
    t.last_hit = amount;
    t.since_hit = 0.0;
    let kick = (1.6 + amount * 0.2) * knockback.clamp(0.3, 2.0);
    t.tilt_vel += Vec2::new(local.x, local.z) * kick.min(5.5);
    true
}

/// Rocks back upright, and forgets the damage after a while left alone.
fn rock(m: &mut Mob, dt: f32, _w: &World, _steering: bool) -> MobEvent {
    let MobState::Dummy(t) = &mut m.state else {
        return MobEvent::None;
    };
    t.since_hit += dt;
    if t.since_hit > DUMMY_RESET {
        t.taken = 0.0;
        t.last_hit = 0.0;
    }
    // A springy wooden foot: it rocks back and forth a few times before it settles.
    let acc = -t.tilt * 55.0 - t.tilt_vel * 3.0;
    t.tilt_vel += acc * dt;
    t.tilt += t.tilt_vel * dt;
    if t.tilt.length() > 0.6 {
        t.tilt = t.tilt.normalize() * 0.6;
        t.tilt_vel *= 0.5;
    }
    MobEvent::None
}

/// The Blockbench model (`model::items::dummy`), its front (+Z) toward where it faces.
fn model(m: &Mob, out: &mut Vec<Vertex>, light: [u8; 4]) {
    let root = Mat4::from_translation(m.pos)
        * Mat4::from_rotation_y(FRAC_PI_2 - m.body_yaw)
        * Mat4::from_scale(Vec3::splat(1.0 / 16.0));
    let tilt = m.tally().map_or(Vec2::ZERO, |t| t.tilt);
    crate::model::items::dummy::emit(out, root, tilt, light, 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dummy_counts_every_hit_and_rocks_back() {
        let w = crate::entity::mob::tests::flat_world();
        let mut d = Mob::new(TARGET_DUMMY, Vec3::new(4.5, 64.0, 4.5), 0.0, 5);
        assert!(d.hurt(5.0, Some(Vec3::new(0.0, 65.0, 4.5)), 1.0));
        // (again at once: a dummy is never out of reach)
        assert!(d.hurt(3.0, None, 1.0));
        let t = *d.tally().unwrap();
        assert_eq!((t.taken, t.last_hit), (8.0, 3.0));
        assert!(t.tilt_vel.length() > 0.0 && d.alive());
        let ctx = MobCtx { players: vec![], people: vec![], mobs: vec![] };
        for _ in 0..200 {
            d.update(0.05, &w, &ctx);
        }
        let t = *d.tally().unwrap();
        assert!(t.tilt.length() < 0.05, "still tipped {}", t.tilt);
        assert_eq!(t.taken, 0.0);
        // It never moved (only stood on the floor).
        assert_eq!((d.pos.x, d.pos.z), (4.5, 4.5));
        assert!((d.pos.y - 64.0).abs() < 0.01);
        d.push(Vec3::X);
        assert_eq!(d.vel.x, 0.0);
    }
}
