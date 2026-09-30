//! Grenades in the world, the same wherever they fly: how they fall, bounce and roll, and which
//! blocks a blast takes (the same for the same seed on every computer). The server's copy
//! decides the blast; the players' games fly their own copies to show them.

use crate::item::mining::hardness;
use crate::item::ItemId;
use crate::world::*;
use glam::{IVec3, Quat, Vec3};

/// Seconds from the button going down until a frag grenade explodes, and from the throw until
/// a smoke grenade starts smoking; how long it smokes.
pub const FUSE: f32 = 5.0;
pub const SMOKE_FUSE: f32 = 1.6;
pub const SMOKE_TIME: f32 = 16.0;
/// How far the blast blows blocks away and hurts, and the most it hurts (at the middle).
pub const BLAST_RADIUS: f32 = 2.8;
pub const HURT_RADIUS: f32 = 6.5;
pub const MAX_DAMAGE: f32 = 26.0;
/// Size of a grenade, for bouncing.
pub const RADIUS: f32 = 0.08;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GrenadeKind {
    Frag,
    Smoke,
}

impl GrenadeKind {
    /// The grenade an item is (its line's `grenade`).
    pub fn of(item: ItemId) -> Option<Self> {
        crate::item::item_def(item)?.grenade
    }

    pub fn from_u8(v: u8) -> Self {
        if v == 1 {
            GrenadeKind::Smoke
        } else {
            GrenadeKind::Frag
        }
    }
}

pub struct Grenade {
    pub kind: GrenadeKind,
    pub pos: Vec3,
    pub vel: Vec3,
    pub rot: Quat,
    pub spin: Vec3,
    pub fuse: f32,
    /// Seconds of smoke left once a smoke grenade went off.
    pub smoke: Option<f32>,
    /// The same on every computer: which blocks the blast takes.
    pub seed: u32,
    /// When the next puff of smoke comes out.
    pub puff: f32,
}

impl Grenade {
    /// A grenade starting to fly (spinning as its seed says), going off after `fuse` seconds.
    pub fn new(kind: GrenadeKind, pos: Vec3, vel: Vec3, seed: u32, fuse: f32) -> Grenade {
        let spin = Vec3::new(hash3(IVec3::X, seed) - 0.5, hash3(IVec3::Y, seed) - 0.5, hash3(IVec3::Z, seed) - 0.5) * 18.0;
        Grenade { kind, pos, vel, rot: Quat::IDENTITY, spin, fuse, smoke: None, seed, puff: 0.0 }
    }
}

/// A hash 0..1 of a block and a seed.
pub fn hash3(p: IVec3, seed: u32) -> f32 {
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
pub fn blastable(b: Block) -> bool {
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
pub fn blast_blocks(world: &World, pos: Vec3, seed: u32) -> Vec<IVec3> {
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
pub fn fly(g: &mut Grenade, dt: f32, world: &World) -> f32 {
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

/// How much a blast at `pos` hurts something at `target` (its middle) and throws it: less the
/// farther, and behind cover; None out of reach.
pub fn blast_hurt(world: &World, pos: Vec3, target: Vec3) -> Option<(f32, f32)> {
    let d = target.distance(pos);
    if d >= HURT_RADIUS {
        return None;
    }
    let k = (1.0 - d / HURT_RADIUS).powf(1.4);
    let dir = (target - pos).normalize_or(Vec3::Y);
    let covered = crate::entity::player::raycast_solid(world, pos + dir * 0.3, dir, (d - 0.5).max(0.0)).is_some();
    let cover = if covered { 0.35 } else { 1.0 };
    Some((MAX_DAMAGE * k * cover, 1.6 * k * cover))
}
