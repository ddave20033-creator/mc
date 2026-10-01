//! A player's body in the world: walking, sprinting, sneaking, jumping, moving through fluids,
//! flying (and a spectator's flight through blocks), colliding with blocks and doors; and the
//! rays cast from the eye to find the block looked at.

use crate::world::block::Block;
use crate::world::{
    block_boxes, door_closed_side, door_open, door_out, is_door, is_fluid, is_lava, is_solid,
    Boxes, World, AIR,
};
use glam::{IVec3, Vec3};

pub const EYE_HEIGHT: f32 = 1.62;
/// How much lower the eyes are while sneaking (Minecraft: 1.62 -> 1.27).
const SNEAK_DROP: f32 = 0.35;
const HALF_W: f32 = 0.3;
const TALL: f32 = 1.8;
/// Walking up onto something this high happens by itself (stairs, like Minecraft's 0.6).
const STEP: f32 = 0.6;

pub struct MoveInput {
    pub forward: f32,
    pub strafe: f32,
    pub up: bool,
    pub down: bool,
    pub sprint: bool,
    pub sneak: bool,
    /// Using an item (blocking with a sword): walk at a fifth of the speed, no sprinting.
    pub using: bool,
    /// Aiming a gun down its sights: a careful walk, no sprinting.
    pub aiming: bool,
}

#[derive(Default)]
pub struct Player {
    pub pos: Vec3,
    pub vel: Vec3,
    pub on_ground: bool,
    pub flying: bool,
    /// Spectator mode: flies through blocks (nothing stops it).
    pub noclip: bool,
    pub spawned: bool,
    pub sprinting: bool,
    pub sneaking: bool,
    /// Smoothed sneak amount 0..1 for the camera and the model.
    pub crouch: f32,
    /// Bumped into a wall horizontally during the last update.
    pub hit_wall: bool,
    /// Where it was before the last tick (drawn between that and `pos`) and its eye height
    /// then.
    pub prev_pos: Vec3,
    pub prev_crouch: f32,
    /// Its speed before the last tick (the speed drawn goes from it to `vel`, as the place
    /// does: what moves with the walk, the view bobbing and the limbs, does not change in
    /// steps 20 times a second).
    pub prev_vel: Vec3,
    /// How far below where it is it is still drawn after stepping up onto something (a stair,
    /// a slab): the rise is shown as a quick glide, not a jump in one tick. It settles by
    /// frames (`settle`).
    pub step_lag: f32,
}

pub fn look_dir(yaw: f32, pitch: f32) -> Vec3 {
    Vec3::new(
        yaw.cos() * pitch.cos(),
        pitch.sin(),
        yaw.sin() * pitch.cos(),
    )
}

/// The boxes (world coordinates) of the solid block at a cell; a full cube where the world
/// is not loaded yet.
pub fn cell_boxes(world: &World, x: i32, y: i32, z: i32) -> Option<Boxes> {
    if !world.is_loaded(x, z) {
        return Some(Boxes::one([0.0; 3], [1.0; 3]));
    }
    let b = world.get(x, y, z);
    if !is_solid(b) {
        return None;
    }
    let p = IVec3::new(x, y, z);
    Some(block_boxes(b, |d| world.geti(p + d)))
}

/// Every solid box (world coordinates) touching the region `min`..`max`, including doors
/// next to it that swing out into it.
fn boxes_in(world: &World, min: Vec3, max: Vec3, mut f: impl FnMut(Vec3, Vec3)) {
    let (x0, x1) = (min.x.floor() as i32, (max.x - 1e-4).floor() as i32);
    let (z0, z1) = (min.z.floor() as i32, (max.z - 1e-4).floor() as i32);
    for x in x0 - 1..=x1 + 1 {
        for y in min.y.floor() as i32..=(max.y - 1e-4).floor() as i32 {
            for z in z0 - 1..=z1 + 1 {
                let ring = x < x0 || x > x1 || z < z0 || z > z1;
                if ring && !(world.is_loaded(x, z) && swung_out(world.get(x, y, z))) {
                    continue;
                }
                if let Some(bx) = cell_boxes(world, x, y, z) {
                    let o = Vec3::new(x as f32, y as f32, z as f32);
                    for (lo, hi) in bx.iter() {
                        f(o + Vec3::from(*lo), o + Vec3::from(*hi));
                    }
                }
            }
        }
    }
}

fn swung_out(b: Block) -> bool {
    is_door(b) && door_open(b) && door_out(b)
}

/// Area of solid ground directly under the feet. Keeping some overlap prevents
/// a crouching player from balancing on a sub-pixel corner and then falling.
fn support_area(world: &World, p: Vec3) -> f32 {
    let (min, max) = (
        p - Vec3::new(HALF_W, 0.1, HALF_W),
        p + Vec3::new(HALF_W, 0.0, HALF_W),
    );
    let mut area = 0.0;
    boxes_in(world, min, max + Vec3::Y * 1e-3, |lo, hi| {
        if (hi.y - p.y).abs() <= 0.05 {
            let dx = (max.x.min(hi.x) - min.x.max(lo.x)).max(0.0);
            let dz = (max.z.min(hi.z) - min.z.max(lo.z)).max(0.0);
            area += dx * dz;
        }
    });
    area
}

/// How far the player at `p` can move along `axis` (up to `delta`) before touching a block.
fn clip_move(world: &World, p: Vec3, axis: usize, delta: f32) -> f32 {
    if delta == 0.0 {
        return 0.0;
    }
    let min = p - Vec3::new(HALF_W, 0.0, HALF_W);
    let max = p + Vec3::new(HALF_W, TALL, HALF_W);
    let (mut smin, mut smax) = (min, max);
    if delta > 0.0 {
        smax[axis] += delta;
    } else {
        smin[axis] += delta;
    }
    let mut d = delta;
    boxes_in(world, smin, smax, |lo, hi| {
        // Only boxes beside the player on the other two axes can be hit.
        if !(0..3)
            .filter(|&k| k != axis)
            .all(|k| min[k] < hi[k] - 1e-4 && max[k] > lo[k] + 1e-4)
        {
            return;
        }
        if d > 0.0 && lo[axis] >= max[axis] - 1e-3 {
            d = d.min(lo[axis] - max[axis]);
        } else if d < 0.0 && hi[axis] <= min[axis] + 1e-3 {
            d = d.max(hi[axis] - min[axis]);
        }
    });
    d
}

impl Player {
    pub fn eye(&self) -> Vec3 {
        self.pos + Vec3::Y * (EYE_HEIGHT - self.crouch * SNEAK_DROP)
    }

    /// Before a tick: where it is now is where the next frames start from.
    pub fn start_tick(&mut self) {
        self.prev_pos = self.pos;
        self.prev_crouch = self.crouch;
        self.prev_vel = self.vel;
    }

    /// Where it is drawn, `between` (0..1) of the way from before the last tick to now (not
    /// across a jump of more than a few blocks: a teleport), lower by what is left of a step up.
    pub fn drawn_pos(&self, between: f32) -> Vec3 {
        if self.prev_pos.distance_squared(self.pos) > 16.0 {
            return self.pos;
        }
        self.between_pos(between) - Vec3::Y * self.step_lag
    }

    /// `between` of the way from before the last tick to now (not across a teleport), without
    /// the step up's glide.
    pub fn between_pos(&self, between: f32) -> Vec3 {
        if self.prev_pos.distance_squared(self.pos) > 16.0 {
            return self.pos;
        }
        self.prev_pos.lerp(self.pos, between)
    }

    /// Its speed over the ground as drawn (see `prev_vel`).
    pub fn drawn_speed(&self, between: f32) -> f32 {
        let v = self.drawn_vel(between);
        Vec3::new(v.x, 0.0, v.z).length()
    }

    /// Its velocity as drawn (see `prev_vel`).
    pub fn drawn_vel(&self, between: f32) -> Vec3 {
        self.prev_vel.lerp(self.vel, between)
    }

    /// A frame: what is left of a step up settles (most of it in a tenth of a second).
    pub fn settle(&mut self, dt: f32) {
        self.step_lag *= (-dt * 22.0).exp();
        if self.step_lag < 1e-3 {
            self.step_lag = 0.0;
        }
    }

    /// Its eye, drawn (see `drawn_pos`).
    pub fn drawn_eye(&self, between: f32) -> Vec3 {
        self.drawn_pos(between) + Vec3::Y * (EYE_HEIGHT - self.drawn_crouch(between) * SNEAK_DROP)
    }

    /// How far it is crouched, drawn (between the last two ticks).
    pub fn drawn_crouch(&self, between: f32) -> f32 {
        self.prev_crouch + (self.crouch - self.prev_crouch) * between
    }

    pub fn intersects(&self, b: IVec3) -> bool {
        let min = self.pos - Vec3::new(HALF_W, 0.0, HALF_W);
        let max = self.pos + Vec3::new(HALF_W, TALL, HALF_W);
        let (bmin, bmax) = (b.as_vec3(), b.as_vec3() + Vec3::ONE);
        min.x < bmax.x
            && max.x > bmin.x
            && min.y < bmax.y
            && max.y > bmin.y
            && min.z < bmax.z
            && max.z > bmin.z
    }

    /// Fluid block at the player's waist, if any.
    pub fn fluid(&self, world: &World) -> Block {
        let p = self.pos + Vec3::Y * 0.4;
        let b = world.get(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
        if is_fluid(b) {
            b
        } else {
            AIR
        }
    }

    pub fn horizontal_speed(&self) -> f32 {
        Vec3::new(self.vel.x, 0.0, self.vel.z).length()
    }

    /// Moves along one axis as far as the blocks allow; true if something was in the way.
    fn move_axis(&mut self, world: &World, axis: usize, delta: f32) -> bool {
        if delta == 0.0 {
            return false;
        }
        let d = clip_move(world, self.pos, axis, delta);
        self.pos[axis] += d;
        (d - delta).abs() > 1e-5
    }

    /// Spectator flight: like creative flying, but through blocks and never landing.
    fn fly_through(&mut self, dt: f32, wish: Vec3, input: &MoveInput) {
        self.flying = true;
        self.on_ground = false;
        self.hit_wall = false;
        self.sneaking = false;
        self.sprinting = input.sprint && input.forward > 0.0;
        self.crouch += (0.0 - self.crouch) * (crate::util::damp(14.0, dt));
        let speed = if input.sprint { 22.0 } else { 11.0 };
        let mut target = wish * speed;
        target.y = (input.up as i32 - input.down as i32) as f32
            * if input.sprint { 12.0 } else { 8.0 };
        self.vel = self.vel.lerp(target, crate::util::damp(10.0, dt));
        self.pos += self.vel * dt;
    }

    pub fn update(&mut self, dt: f32, world: &World, yaw: f32, input: &MoveInput) {
        let fwd = Vec3::new(yaw.cos(), 0.0, yaw.sin());
        let right = Vec3::new(-yaw.sin(), 0.0, yaw.cos());
        let mut wish = fwd * input.forward + right * input.strafe;
        if wish.length_squared() > 0.0 {
            wish = wish.normalize();
        }
        if self.noclip {
            self.fly_through(dt, wish, input);
            return;
        }
        let fluid = self.fluid(world);
        let in_fluid = fluid != AIR;
        let in_lava = is_lava(fluid);
        self.sneaking = input.sneak && !self.flying && !in_fluid;
        self.sprinting =
            input.sprint
                && input.forward > 0.0
                && !in_fluid
                && !self.sneaking
                && !input.using
                && !input.aiming;
        let target = if self.sneaking { 1.0 } else { 0.0 };
        self.crouch += (target - self.crouch) * (crate::util::damp(14.0, dt));

        // The upward speed the move uses (see falling, below).
        let mut mean_vy = None;
        if self.flying {
            let speed = if input.sprint { 22.0 } else { 11.0 };
            let mut target = wish * speed;
            target.y = (input.up as i32 - input.down as i32) as f32
                * if input.sprint { 12.0 } else { 8.0 };
            self.vel = self.vel.lerp(target, crate::util::damp(10.0, dt));
        } else {
            let speed = if in_lava {
                1.2
            } else if in_fluid {
                2.4
            } else if self.sprinting {
                5.6
            } else if self.sneaking {
                1.3
            } else {
                4.3
            } * if input.using {
                0.2
            } else if input.aiming {
                0.6
            } else {
                1.0
            };
            let accel = if self.on_ground || in_fluid {
                14.0
            } else {
                3.5
            };
            let t = crate::util::damp(accel, dt);
            self.vel.x += (wish.x * speed - self.vel.x) * t;
            self.vel.z += (wish.z * speed - self.vel.z) * t;
            if in_fluid {
                let sink = if in_lava { 3.0 } else { 10.0 };
                self.vel.y = (self.vel.y - sink * dt).max(-3.0);
                if input.up {
                    self.vel.y = (self.vel.y + 24.0 * dt).min(if in_lava { 2.0 } else { 3.8 });
                    // Swimming against a block edge pulls you up and out (like Minecraft).
                    if self.hit_wall {
                        self.vel.y = 6.0;
                    }
                }
            } else {
                // Moved by the mean of the speed before and after gravity this frame: a jump
                // goes as high at any frame rate.
                let before = if input.up && self.on_ground { 8.7 } else { self.vel.y };
                self.vel.y = (before - 28.0 * dt).max(-60.0);
                mean_vy = Some((before + self.vel.y) * 0.5);
            }
        }

        let delta = Vec3::new(self.vel.x, mean_vy.unwrap_or(self.vel.y), self.vel.z) * dt;
        let steps = (delta.abs().max_element() / 0.4).ceil().max(1.0) as i32;
        let mut d = delta / steps as f32;
        // The collision epsilon can make on_ground false for one frame at high FPS.
        // Check the actual support under the feet before allowing a crouched step.
        let edge_guard = self.sneaking
            && self.vel.y <= 0.0
            && (self.on_ground || support_area(world, self.pos) > 0.0);
        self.on_ground = false;
        self.hit_wall = false;
        for _ in 0..steps {
            if self.move_axis(world, 1, d.y) {
                if d.y < 0.0 {
                    self.on_ground = true;
                }
                self.vel.y = 0.0;
            }
            if edge_guard {
                let area = support_area(world, self.pos);
                let next_x = support_area(world, self.pos + Vec3::new(d.x, 0.0, 0.0));
                if next_x < 0.025 && next_x < area {
                    d.x = 0.0;
                    self.vel.x = 0.0;
                }
                // Checked with the x move applied, so walking diagonally off a corner is stopped too.
                let next_z = support_area(world, self.pos + Vec3::new(d.x, 0.0, d.z));
                if next_z < 0.025 && next_z < area {
                    d.z = 0.0;
                    self.vel.z = 0.0;
                }
            }
            let start = self.pos;
            let mut bx = self.move_axis(world, 0, d.x);
            let mut bz = self.move_axis(world, 2, d.z);
            // Blocked on the ground: try stepping up onto it (a stair, a slab).
            if (bx || bz) && self.on_ground {
                let flat = self.pos;
                self.pos = start;
                let up = clip_move(world, self.pos, 1, STEP);
                self.pos.y += up;
                let sx = self.move_axis(world, 0, d.x);
                let sz = self.move_axis(world, 2, d.z);
                self.pos.y += clip_move(world, self.pos, 1, -up);
                let gain = |p: Vec3| (p.x - start.x).powi(2) + (p.z - start.z).powi(2);
                if gain(self.pos) > gain(flat) + 1e-6 {
                    (bx, bz) = (sx, sz);
                    // (drawn rising from where it was: the tick's glide leaves the rise out,
                    // `step_lag` brings it in smoothly)
                    let rise = (self.pos.y - start.y).max(0.0);
                    self.prev_pos.y += rise;
                    self.step_lag += rise;
                } else {
                    self.pos = flat;
                }
            }
            if bx {
                self.vel.x = 0.0;
                self.hit_wall = true;
            }
            if bz {
                self.vel.z = 0.0;
                self.hit_wall = true;
            }
        }
        if !self.on_ground
            && !self.flying
            && !in_fluid
            && self.vel.y <= 0.0
            && support_area(world, self.pos) > 0.0
        {
            self.on_ground = true;
            self.vel.y = 0.0;
        }
        if self.flying && self.on_ground && input.down {
            self.flying = false;
        }
    }
}

/// Voxel DDA raycast. Returns (hit block, the empty block in front of it).
pub fn raycast(world: &World, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<(IVec3, IVec3)> {
    raycast_by(world, origin, dir, max_dist, |b| b != AIR && !is_fluid(b))
}

/// Like `raycast`, but also stops at fluid source blocks (for buckets).
pub fn raycast_fluid(
    world: &World,
    origin: Vec3,
    dir: Vec3,
    max_dist: f32,
) -> Option<(IVec3, IVec3)> {
    raycast_by(world, origin, dir, max_dist, |b| {
        b != AIR && (!is_fluid(b) || crate::world::fluid_level(b) == 0)
    })
}

/// Like `raycast`, but only solid blocks stop it (a bullet flies through grass and torches).
pub fn raycast_solid(
    world: &World,
    origin: Vec3,
    dir: Vec3,
    max_dist: f32,
) -> Option<(IVec3, IVec3)> {
    raycast_by(world, origin, dir, max_dist, is_solid)
}

fn raycast_by(
    world: &World,
    origin: Vec3,
    dir: Vec3,
    max_dist: f32,
    hit: impl Fn(Block) -> bool,
) -> Option<(IVec3, IVec3)> {
    let mut pos = origin.floor().as_ivec3();
    let step = IVec3::new(
        dir.x.signum() as i32,
        dir.y.signum() as i32,
        dir.z.signum() as i32,
    );
    let inv = |d: f32| {
        if d != 0.0 {
            (1.0 / d).abs()
        } else {
            f32::INFINITY
        }
    };
    let t_delta = Vec3::new(inv(dir.x), inv(dir.y), inv(dir.z));
    let first = |o: f32, p: i32, d: f32| {
        if d > 0.0 {
            (p as f32 + 1.0 - o) / d
        } else if d < 0.0 {
            (o - p as f32) / -d
        } else {
            f32::INFINITY
        }
    };
    let mut t_max = Vec3::new(
        first(origin.x, pos.x, dir.x),
        first(origin.y, pos.y, dir.y),
        first(origin.z, pos.z, dir.z),
    );
    let mut prev = pos;
    loop {
        let b = world.get(pos.x, pos.y, pos.z);
        // A door beside this cell swung out into it.
        for d in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
            let q = pos + d;
            let qb = world.geti(q);
            if swung_out(qb) && door_closed_side(qb) == -d && hit(qb) {
                if let Some((_, normal)) = ray_boxes(world, origin, dir, q, max_dist) {
                    let front = if normal == IVec3::ZERO { prev } else { pos + normal };
                    return Some((q, if front == q { prev } else { front }));
                }
            }
        }
        if hit(b) {
            if !is_solid(b) {
                return Some((pos, prev));
            }
            // Blocks smaller than the cell (doors, stairs): only their boxes count, and the
            // block goes in front of the face that was hit.
            if let Some((_, normal)) = ray_boxes(world, origin, dir, pos, max_dist) {
                let front = if normal == IVec3::ZERO { prev } else { pos + normal };
                return Some((pos, front));
            }
        }
        prev = pos;
        let t;
        if t_max.x < t_max.y && t_max.x < t_max.z {
            pos.x += step.x;
            t = t_max.x;
            t_max.x += t_delta.x;
        } else if t_max.y < t_max.z {
            pos.y += step.y;
            t = t_max.y;
            t_max.y += t_delta.y;
        } else {
            pos.z += step.z;
            t = t_max.z;
            t_max.z += t_delta.z;
        }
        if t > max_dist {
            return None;
        }
    }
}

/// Where a ray first hits the boxes of the block at `p`: the distance and the outward normal
/// of the face hit (zero when the ray starts inside).
pub fn ray_boxes(
    world: &World,
    origin: Vec3,
    dir: Vec3,
    p: IVec3,
    max_dist: f32,
) -> Option<(f32, IVec3)> {
    let b = world.geti(p);
    let bx = block_boxes(b, |d| world.geti(p + d));
    let o = p.as_vec3();
    let mut best: Option<(f32, IVec3)> = None;
    for (lo, hi) in bx.iter() {
        let (lo, hi) = (o + Vec3::from(*lo), o + Vec3::from(*hi));
        let Some(t) = crate::util::ray_box(origin, dir, lo, hi, max_dist) else {
            continue;
        };
        if best.is_some_and(|(bt, _)| bt <= t) {
            continue;
        }
        let at = origin + dir * t;
        let mut normal = IVec3::ZERO;
        if t > 0.0 {
            for k in 0..3 {
                if dir[k] > 0.0 && (at[k] - lo[k]).abs() < 1e-3 {
                    normal[k] = -1;
                    break;
                }
                if dir[k] < 0.0 && (at[k] - hi[k]).abs() < 1e-3 {
                    normal[k] = 1;
                    break;
                }
            }
        }
        best = Some((t, normal));
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkData, STONE};
    use std::sync::Arc;

    /// A stone floor at y 0 with some blocks on it; returns the world.
    fn floor(blocks: &[(IVec3, Block)]) -> World {
        let mut world = World::new();
        let mut chunk = ChunkData::new();
        for x in 0..16 {
            for z in 0..16 {
                chunk.set(x, 0, z, STONE);
            }
        }
        for (p, b) in blocks {
            chunk.set(p.x as usize, p.y as usize, p.z as usize, *b);
        }
        world.chunks.insert((0, 0), Arc::new(chunk));
        world
    }

    fn walk_east(world: &World, from: Vec3, seconds: f32) -> Player {
        let input = MoveInput {
            forward: 1.0,
            strafe: 0.0,
            up: false,
            down: false,
            sprint: false,
            sneak: false,
            using: false,
            aiming: false,
        };
        let mut player = Player {
            pos: from,
            spawned: true,
            ..Default::default()
        };
        for _ in 0..(seconds * 120.0) as i32 {
            player.update(1.0 / 120.0, world, 0.0, &input);
        }
        player
    }

    #[test]
    fn walks_up_stairs_without_jumping() {
        use crate::world::{stairs_id, STONE};
        // Stairs going up toward +X, then a block to step onto.
        let world = floor(&[
            (IVec3::new(8, 1, 8), stairs_id(1, false)),
            (IVec3::new(9, 1, 8), STONE),
            (IVec3::new(10, 1, 8), STONE),
            (IVec3::new(11, 1, 8), STONE),
            (IVec3::new(12, 1, 8), STONE),
        ]);
        let p = walk_east(&world, Vec3::new(5.5, 1.0, 8.5), 1.2);
        assert!(p.pos.x > 9.5 && (p.pos.y - 2.0).abs() < 1e-3, "{:?}", p.pos);
        // A full block is too high to walk onto.
        let world = floor(&[(IVec3::new(8, 1, 8), STONE)]);
        let p = walk_east(&world, Vec3::new(5.5, 1.0, 8.5), 1.5);
        assert!(p.pos.x < 8.0 && p.pos.y < 1.01, "{:?}", p.pos);
    }

    #[test]
    fn closed_doors_block_and_open_ones_let_through() {
        use crate::world::door_id;
        // Placed looking east: the closed panel is on the west side of the block.
        let door = |open| {
            floor(&[
                (IVec3::new(8, 1, 8), door_id(1, open, false, false)),
                (IVec3::new(8, 2, 8), door_id(1, open, true, false)),
            ])
        };
        let p = walk_east(&door(false), Vec3::new(5.5, 1.0, 8.5), 1.5);
        assert!((p.pos.x - (8.0 - HALF_W)).abs() < 0.01, "{:?}", p.pos);
        let p = walk_east(&door(true), Vec3::new(5.5, 1.0, 8.5), 1.5);
        assert!(p.pos.x > 9.0, "{:?}", p.pos);
        // Swung out (west, toward the walker), the panel still lies along the north side.
        let out = |upper| crate::world::door_set_open(door_id(1, false, upper, false), true, true);
        let world = floor(&[(IVec3::new(8, 1, 8), out(false)), (IVec3::new(8, 2, 8), out(true))]);
        let p = walk_east(&world, Vec3::new(5.5, 1.0, 8.5), 1.5);
        assert!(p.pos.x > 9.0, "{:?}", p.pos);
        let p = walk_east(&world, Vec3::new(5.5, 1.0, 7.9), 1.5);
        assert!(p.pos.x < 7.5, "walked through the swung-out panel: {:?}", p.pos);
        // ...and can be aimed at from the block it swung into.
        let hit = raycast(&world, Vec3::new(7.5, 1.5, 8.9), Vec3::NEG_Z, 3.0);
        assert_eq!(hit.map(|h| h.0), Some(IVec3::new(8, 1, 8)));
        // Aiming through the open door's empty part reaches the block behind it.
        let mut world = door(true);
        world.seti(IVec3::new(10, 1, 8), STONE);
        let hit = raycast(&world, Vec3::new(6.5, 1.5, 8.5), Vec3::X, 6.0);
        assert_eq!(hit.map(|h| h.0), Some(IVec3::new(10, 1, 8)));
        let hit = raycast(&door(false), Vec3::new(6.5, 1.5, 8.5), Vec3::X, 6.0);
        assert_eq!(hit, Some((IVec3::new(8, 1, 8), IVec3::new(7, 1, 8))));
    }

    #[test]
    fn sneaking_stays_on_a_single_block_at_high_fps() {
        let mut world = World::new();
        let mut chunk = ChunkData::new();
        chunk.set(8, 0, 8, STONE);
        world.chunks.insert((0, 0), Arc::new(chunk));
        let input = MoveInput {
            forward: 1.0,
            strafe: 0.0,
            up: false,
            down: false,
            sprint: false,
            sneak: true,
            using: false,
            aiming: false,
        };
        for yaw in [0.0, std::f32::consts::FRAC_PI_4] {
            let mut player = Player {
                pos: Vec3::new(8.5, 1.001, 8.5),
                spawned: true,
                ..Default::default()
            };
            for _ in 0..720 {
                player.update(1.0 / 240.0, &world, yaw, &input);
            }
            assert!(
                player.pos.y >= 0.99,
                "fell while sneaking at yaw {yaw}: {:?}",
                player.pos
            );
            assert!(
                support_area(&world, player.pos) >= 0.024,
                "lost footing at yaw {yaw}: {:?}",
                player.pos
            );
        }
    }
}
