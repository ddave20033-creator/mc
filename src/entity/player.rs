use crate::world::{is_fluid, is_lava, is_solid, World, AIR, FIRE};
use glam::{IVec3, Vec3};

pub const EYE_HEIGHT: f32 = 1.62;
/// How much lower the eyes are while sneaking (Minecraft: 1.62 -> 1.27).
const SNEAK_DROP: f32 = 0.35;
const HALF_W: f32 = 0.3;
const TALL: f32 = 1.8;

pub struct MoveInput {
    pub forward: f32,
    pub strafe: f32,
    pub up: bool,
    pub down: bool,
    pub sprint: bool,
    pub sneak: bool,
    /// Using an item (blocking with a sword): walk at a fifth of the speed, no sprinting.
    pub using: bool,
}

#[derive(Default)]
pub struct Player {
    pub pos: Vec3,
    pub vel: Vec3,
    pub on_ground: bool,
    pub flying: bool,
    pub spawned: bool,
    pub sprinting: bool,
    pub sneaking: bool,
    /// Smoothed sneak amount 0..1 for the camera and the model.
    pub crouch: f32,
    /// Bumped into a wall horizontally during the last update.
    pub hit_wall: bool,
}

pub fn look_dir(yaw: f32, pitch: f32) -> Vec3 {
    Vec3::new(
        yaw.cos() * pitch.cos(),
        pitch.sin(),
        yaw.sin() * pitch.cos(),
    )
}

fn solid_at(world: &World, x: i32, y: i32, z: i32) -> bool {
    if !world.is_loaded(x, z) {
        return true;
    }
    is_solid(world.get(x, y, z))
}

/// Area of solid ground directly under the feet. Keeping some overlap prevents
/// a crouching player from balancing on a sub-pixel corner and then falling.
fn support_area(world: &World, p: Vec3) -> f32 {
    let y = (p.y - 0.1).floor() as i32;
    if (p.y - (y + 1) as f32).abs() > 0.05 {
        return 0.0;
    }
    let (min, max) = (
        p - Vec3::new(HALF_W, 0.0, HALF_W),
        p + Vec3::new(HALF_W, 0.0, HALF_W),
    );
    let mut area = 0.0;
    for x in min.x.floor() as i32..=(max.x - 1e-4).floor() as i32 {
        for z in min.z.floor() as i32..=(max.z - 1e-4).floor() as i32 {
            if solid_at(world, x, y, z) {
                let dx = (max.x.min(x as f32 + 1.0) - min.x.max(x as f32)).max(0.0);
                let dz = (max.z.min(z as f32 + 1.0) - min.z.max(z as f32)).max(0.0);
                area += dx * dz;
            }
        }
    }
    area
}

fn collides(world: &World, p: Vec3) -> bool {
    let min = p - Vec3::new(HALF_W, 0.0, HALF_W);
    let max = p + Vec3::new(HALF_W, TALL, HALF_W);
    for x in min.x.floor() as i32..=(max.x - 1e-4).floor() as i32 {
        for y in min.y.floor() as i32..=(max.y - 1e-4).floor() as i32 {
            for z in min.z.floor() as i32..=(max.z - 1e-4).floor() as i32 {
                if solid_at(world, x, y, z) {
                    return true;
                }
            }
        }
    }
    false
}

impl Player {
    pub fn eye(&self) -> Vec3 {
        self.pos + Vec3::Y * (EYE_HEIGHT - self.crouch * SNEAK_DROP)
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
    pub fn fluid(&self, world: &World) -> u8 {
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

    fn move_axis(&mut self, world: &World, axis: usize, delta: f32) -> bool {
        if delta == 0.0 {
            return false;
        }
        let mut p = self.pos;
        p[axis] += delta;
        if !collides(world, p) {
            self.pos = p;
            return false;
        }
        let (lo, hi) = if axis == 1 {
            (0.0, TALL)
        } else {
            (-HALF_W, HALF_W)
        };
        if delta > 0.0 {
            p[axis] = (p[axis] + hi).floor() - hi - 0.001;
        } else {
            p[axis] = (p[axis] + lo).floor() + 1.0 - lo + 0.001;
        }
        if !collides(world, p) {
            self.pos = p;
        }
        true
    }

    pub fn update(&mut self, dt: f32, world: &World, yaw: f32, input: &MoveInput) {
        let fwd = Vec3::new(yaw.cos(), 0.0, yaw.sin());
        let right = Vec3::new(-yaw.sin(), 0.0, yaw.cos());
        let mut wish = fwd * input.forward + right * input.strafe;
        if wish.length_squared() > 0.0 {
            wish = wish.normalize();
        }
        let fluid = self.fluid(world);
        let in_fluid = fluid != AIR;
        let in_lava = is_lava(fluid);
        self.sneaking = input.sneak && !self.flying && !in_fluid;
        self.sprinting =
            input.sprint && input.forward > 0.0 && !in_fluid && !self.sneaking && !input.using;
        let target = if self.sneaking { 1.0 } else { 0.0 };
        self.crouch += (target - self.crouch) * (1.0 - (-14.0 * dt).exp());

        if self.flying {
            let speed = if input.sprint { 22.0 } else { 11.0 };
            let mut target = wish * speed;
            target.y = (input.up as i32 - input.down as i32) as f32
                * if input.sprint { 12.0 } else { 8.0 };
            self.vel = self.vel.lerp(target, 1.0 - (-10.0 * dt).exp());
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
            } * if input.using { 0.2 } else { 1.0 };
            let accel = if self.on_ground || in_fluid {
                14.0
            } else {
                3.5
            };
            let t = 1.0 - (-accel * dt).exp();
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
                self.vel.y = (self.vel.y - 28.0 * dt).max(-60.0);
                if input.up && self.on_ground {
                    self.vel.y = 8.7;
                }
            }
        }

        let delta = self.vel * dt;
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
            if self.move_axis(world, 0, d.x) {
                self.vel.x = 0.0;
                self.hit_wall = true;
            }
            if self.move_axis(world, 2, d.z) {
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
    raycast_by(world, origin, dir, max_dist, |b| {
        b != AIR && !is_fluid(b) && b != FIRE
    })
}

/// Like `raycast`, but also stops at fluid source blocks (for buckets).
pub fn raycast_fluid(
    world: &World,
    origin: Vec3,
    dir: Vec3,
    max_dist: f32,
) -> Option<(IVec3, IVec3)> {
    raycast_by(world, origin, dir, max_dist, |b| {
        b != AIR && b != FIRE && (!is_fluid(b) || crate::world::fluid_level(b) == 0)
    })
}

fn raycast_by(
    world: &World,
    origin: Vec3,
    dir: Vec3,
    max_dist: f32,
    hit: impl Fn(u8) -> bool,
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
        if hit(b) {
            return Some((pos, prev));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkData, STONE};
    use std::sync::Arc;

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
