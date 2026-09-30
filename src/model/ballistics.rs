//! What guns leave in the world: spent cases that fly out to the side, bounce and lie on the
//! ground for a while, tracer streaks of bullets in flight and the laser sight's dot.

use crate::item::{ItemId, BULLET, MAGNUM_ROUND, RIFLE_ROUND};
use crate::util::vertex_light;
use crate::world::mesh::{flags, Vertex};
use crate::world::textures::tex;
use crate::world::{is_solid, World};
use glam::{Mat4, Quat, Vec3};

/// Seconds a spent case stays on the ground.
const CASE_LIFE: f32 = 10.0;
const MAX_CASES: usize = 64;
/// Blocks per model unit of a spent case on the ground (the guns' own case, drawn about as big
/// as a round dropped as an item, so it can be seen).
const CASE_SCALE: f32 = 0.04;

/// What kind of case: sizes and colours differ.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CaseKind {
    /// 9x19 mm.
    Pistol,
    /// .357 Magnum: a case 33 mm long against the pistol's 19, with a rim.
    Magnum,
    /// 7.62x39 mm: a bottlenecked case 39 mm long.
    Rifle,
}

impl CaseKind {
    /// What it sounds like landing.
    pub fn sound(self) -> crate::audio::Sound {
        use crate::audio::Sound;
        match self {
            CaseKind::Pistol => Sound::CaseBrass,
            CaseKind::Magnum => Sound::CaseMagnum,
            CaseKind::Rifle => Sound::CaseRifle,
        }
    }

    /// The round it is the case of.
    fn ammo(self) -> ItemId {
        match self {
            CaseKind::Pistol => BULLET,
            CaseKind::Magnum => MAGNUM_ROUND,
            CaseKind::Rifle => RIFLE_ROUND,
        }
    }

    /// Half its size (a case lies along x), from the guns' model of it.
    fn look(self) -> Vec3 {
        let (len, wide) = super::gun_view::round_size(self.ammo(), true);
        Vec3::new(len, wide, wide) * CASE_SCALE * 0.5
    }
}

struct Case {
    kind: CaseKind,
    pos: Vec3,
    vel: Vec3,
    rot: Quat,
    /// Turning speed (axis * radians per second).
    spin: Vec3,
    age: f32,
    resting: bool,
}

#[derive(Default)]
pub struct Cases {
    list: Vec<Case>,
}

impl Cases {
    /// A spent case thrown out at `pos` with velocity `vel`, tumbling by `spin`.
    pub fn eject(&mut self, pos: Vec3, vel: Vec3, spin: Vec3, kind: CaseKind) {
        if self.list.len() >= MAX_CASES {
            self.list.remove(0);
        }
        self.list.push(Case {
            kind,
            pos,
            vel,
            rot: Quat::IDENTITY,
            spin,
            age: 0.0,
            resting: false,
        });
    }

    /// Falls, bounces off blocks (a little less each time) and comes to rest lying down.
    /// Returns where cases hit the ground hard enough to be heard: (where, which case, how
    /// hard 0..1).
    pub fn update(&mut self, dt: f32, world: &World) -> Vec<(Vec3, CaseKind, f32)> {
        let mut clinks = Vec::new();
        let solid = |p: Vec3| {
            is_solid(world.get(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32))
        };
        for c in &mut self.list {
            c.age += dt;
            if c.resting {
                // The block under it was broken: fall again.
                if !solid(c.pos - Vec3::Y * (c.kind.look().y + 0.02)) {
                    c.resting = false;
                }
                continue;
            }
            c.vel.y -= 20.0 * dt;
            let mut p = c.pos;
            for axis in 0..3 {
                let mut q = p;
                q[axis] += c.vel[axis] * dt;
                if !solid(q) {
                    p = q;
                    continue;
                }
                if axis == 1 && c.vel.y < 0.0 {
                    let hard = (-c.vel.y / 6.0).min(1.0);
                    if hard > 0.15 {
                        clinks.push((p, c.kind, hard));
                    }
                    // Landed: bounce a little, lose speed, spin slower.
                    p.y = q.y.floor() + 1.0 + c.kind.look().y;
                    c.vel.y *= -0.3;
                    c.vel.x *= 0.55;
                    c.vel.z *= 0.55;
                    c.spin *= 0.5;
                    if c.vel.length() < 0.6 {
                        c.resting = true;
                        // Lying on its side, pointing wherever it pointed.
                        let along = c.rot * Vec3::X;
                        let yaw = (-along.z).atan2(along.x);
                        c.rot = Quat::from_rotation_y(yaw);
                        c.vel = Vec3::ZERO;
                    }
                } else {
                    c.vel[axis] *= -0.3;
                }
            }
            c.pos = p;
            let angle = c.spin.length() * dt;
            if angle > 0.0 {
                c.rot = (Quat::from_axis_angle(c.spin.normalize(), angle) * c.rot).normalize();
            }
        }
        self.list.retain(|c| c.age < CASE_LIFE);
        clinks
    }

    pub fn build(&self, out: &mut Vec<Vertex>, world: &World) {
        for c in &self.list {
            let (sky, blk) = world.light_estimate(c.pos + Vec3::Y * 0.1);
            // The case lies along x, its middle at its place: the model's (nose up) turned over.
            let half = c.kind.look();
            let m = Mat4::from_rotation_translation(c.rot, c.pos)
                * Mat4::from_rotation_z(-std::f32::consts::FRAC_PI_2)
                * Mat4::from_translation(-Vec3::Y * half.x)
                * Mat4::from_scale(Vec3::splat(CASE_SCALE));
            let light = vertex_light(sky, blk);
            super::gun_view::emit_round(out, c.kind.ammo(), true, m, light, flags::ENTITY);
        }
    }
}

/// A glowing streak from `from` to `to`, `width` wide, turned toward the camera at `cam`:
/// a bullet's tracer, or with `laser` the laser sight's red beam.
pub fn emit_tracer(out: &mut Vec<Vertex>, from: Vec3, to: Vec3, cam: Vec3, width: f32, laser: bool) {
    let along = to - from;
    let side = along.cross(cam - (from + to) * 0.5).normalize_or_zero() * width * 0.5;
    if side == Vec3::ZERO {
        return;
    }
    let corners = [from - side, to - side, to + side, from + side];
    let tint = if laser { [200, 20, 15] } else { [255, 226, 150] };
    emit_glow_quad(out, corners, tint);
}

/// The laser sight's red dot at `pos`, facing the camera (`right`, `up`), `size` across.
pub fn emit_laser_dot(out: &mut Vec<Vertex>, pos: Vec3, right: Vec3, up: Vec3, size: f32) {
    let (r, u) = (right * size * 0.5, up * size * 0.5);
    emit_glow_quad(out, [pos - r - u, pos + r - u, pos + r + u, pos - r + u], [255, 40, 30]);
}

/// A muzzle flash at `pos` for a shot along `dir`, seen from `cam`: a star facing the
/// camera (an orange one and a smaller white-hot one on it, turned by `seed`) and two flame
/// tongues shooting ahead, crossed around the line of fire. `size` is the star's width in
/// blocks; `k` (1 .. 0) how much of the flash is left.
pub fn emit_muzzle_flash(out: &mut Vec<Vertex>, pos: Vec3, dir: Vec3, cam: Vec3, size: f32, seed: f32, k: f32) {
    let dir = dir.normalize_or_zero();
    let view = (cam - pos).normalize_or_zero();
    if dir == Vec3::ZERO || view == Vec3::ZERO {
        return;
    }
    let grow = 0.7 + 0.3 * k;
    // The star: in the plane facing the camera, turned by the seed.
    let right = view.cross(Vec3::Y).normalize_or(Vec3::X);
    let up = right.cross(view);
    let turn = seed * std::f32::consts::TAU;
    let (r, u) = (
        (right * turn.cos() + up * turn.sin()) * size * 0.5 * grow,
        (up * turn.cos() - right * turn.sin()) * size * 0.5 * grow,
    );
    let quad = |c: Vec3, r: Vec3, u: Vec3| [c - r - u, c + r - u, c + r + u, c - r + u];
    let full = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    let center = pos + dir * size * 0.15;
    emit_sprite(out, quad(center, r, u), tex::MUZZLE_FLASH, [255, 170, 80], full);
    emit_sprite(out, quad(center + view * 0.01, r * 0.55, u * 0.55), tex::MUZZLE_FLASH, [255, 250, 225], full);
    // Tongues: along the line of fire, one turned toward the camera, one across it.
    let len = size * 1.6 * grow;
    let side = dir.cross(view).normalize_or(up);
    let across = dir.cross(side).normalize_or(up);
    for (w, tint) in [(side, [255, 190, 90]), (across, [255, 160, 70])] {
        let w = w * size * 0.45 * grow;
        let (a, b) = (pos, pos + dir * len);
        emit_sprite(out, [a - w, b - w, b + w, a + w], tex::MUZZLE_FLASH_SIDE, tint, [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
    }
}

/// A glowing sprite quad seen from both sides.
fn emit_sprite(out: &mut Vec<Vertex>, corners: [Vec3; 4], layer: u32, tint: [u8; 3], uvs: [[f32; 2]; 4]) {
    let mut light = vertex_light(15, 15);
    light[3] = 6;
    let v: [Vertex; 4] = std::array::from_fn(|i| Vertex {
        pos: corners[i].to_array(),
        uv: uvs[i],
        layer: layer as f32,
        light,
        tint: [tint[0], tint[1], tint[2], flags::EMISSIVE],
    });
    out.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
    out.extend_from_slice(&[v[0], v[2], v[1], v[0], v[3], v[2]]);
}

/// A bright quad seen from both sides.
fn emit_glow_quad(out: &mut Vec<Vertex>, corners: [Vec3; 4], tint: [u8; 3]) {
    let uvs = [[0.4, 0.6], [0.6, 0.6], [0.6, 0.4], [0.4, 0.4]];
    let mut light = vertex_light(15, 15);
    light[3] = 6;
    let v: [Vertex; 4] = std::array::from_fn(|i| Vertex {
        pos: corners[i].to_array(),
        uv: uvs[i],
        layer: tex::WOOL as f32,
        light,
        tint: [tint[0], tint[1], tint[2], flags::EMISSIVE],
    });
    out.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
    out.extend_from_slice(&[v[0], v[2], v[1], v[0], v[3], v[2]]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkData, STONE};
    use std::sync::Arc;

    #[test]
    fn a_spent_case_lands_and_rests_on_the_floor() {
        let mut world = World::new();
        let mut chunk = ChunkData::new();
        for z in 0..16 {
            for x in 0..16 {
                chunk.set(x, 0, z, STONE);
            }
        }
        world.chunks.insert((0, 0), Arc::new(chunk));
        let mut cases = Cases::default();
        cases.eject(
            Vec3::new(8.0, 2.5, 8.0),
            Vec3::new(2.0, 2.5, 0.3),
            Vec3::new(0.0, 6.0, 20.0),
            CaseKind::Pistol,
        );
        for _ in 0..300 {
            cases.update(1.0 / 60.0, &world);
        }
        let c = &cases.list[0];
        assert!(c.resting, "the case is still moving at {}", c.pos);
        assert!((c.pos.y - (1.0 + CaseKind::Pistol.look().y)).abs() < 1e-3, "it rests at {}", c.pos);
        assert!(c.pos.x > 8.5, "it did not fly out to the side: {}", c.pos);
    }
}
