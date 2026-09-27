//! What guns leave in the world: spent cases that fly out to the side, bounce and lie on the
//! ground for a while, tracer streaks of bullets in flight and the laser sight's dot.

use super::emit_box;
use crate::util::vertex_light;
use crate::world::mesh::{flags, Vertex};
use crate::world::textures::tex;
use crate::world::{is_solid, World};
use glam::{Mat4, Quat, Vec3};

/// Seconds a spent case stays on the ground.
const CASE_LIFE: f32 = 10.0;
const MAX_CASES: usize = 64;
/// Half the size of a case (exaggerated a little so it can be seen).
const CASE_HALF: Vec3 = Vec3::new(0.032, 0.013, 0.013);
const BRASS: [u8; 3] = [236, 182, 72];

/// What kind of case: sizes and colours differ.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CaseKind {
    Pistol,
    Magnum,
    Rifle,
    Bmg,
    /// A shotgun shell's red plastic hull.
    Shell,
}

impl CaseKind {
    /// Half its size (a case lies along x) and its colour.
    fn look(self) -> (Vec3, [u8; 3]) {
        match self {
            CaseKind::Pistol => (CASE_HALF, BRASS),
            CaseKind::Magnum => (Vec3::new(0.036, 0.017, 0.017), BRASS),
            CaseKind::Rifle => (Vec3::new(0.05, 0.012, 0.012), BRASS),
            CaseKind::Bmg => (Vec3::new(0.08, 0.02, 0.02), BRASS),
            CaseKind::Shell => (Vec3::new(0.05, 0.022, 0.022), [200, 40, 36]),
        }
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

    pub fn clear(&mut self) {
        self.list.clear();
    }

    /// Falls, bounces off blocks (a little less each time) and comes to rest lying down.
    pub fn update(&mut self, dt: f32, world: &World) {
        let solid = |p: Vec3| {
            is_solid(world.get(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32))
        };
        for c in &mut self.list {
            c.age += dt;
            if c.resting {
                // The block under it was broken: fall again.
                if !solid(c.pos - Vec3::Y * (c.kind.look().0.y + 0.02)) {
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
                    // Landed: bounce a little, lose speed, spin slower.
                    p.y = q.y.floor() + 1.0 + c.kind.look().0.y;
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
    }

    pub fn build(&self, out: &mut Vec<Vertex>, world: &World) {
        for c in &self.list {
            let (sky, blk) = world.light_estimate(c.pos + Vec3::Y * 0.1);
            let m = Mat4::from_rotation_translation(c.rot, c.pos);
            let (half, tint) = c.kind.look();
            let light = vertex_light(sky, blk);
            emit_box(out, m, -half, half, [tex::WOOL; 6], [tint; 6], light, flags::ENTITY);
            if c.kind == CaseKind::Shell {
                // The shell's brass head.
                let head = Vec3::new(-half.x, -half.y - 0.002, -half.z - 0.002);
                let top = Vec3::new(-half.x * 0.5, half.y + 0.002, half.z + 0.002);
                emit_box(out, m, head, top, [tex::WOOL; 6], [BRASS; 6], light, flags::ENTITY);
            }
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
        assert!((c.pos.y - (1.0 + CASE_HALF.y)).abs() < 1e-3, "it rests at {}", c.pos);
        assert!(c.pos.x > 8.5, "it did not fly out to the side: {}", c.pos);
    }
}
