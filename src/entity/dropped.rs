//! Dropped items (bobbing, merging, flying to whoever picks them up) and falling sand and
//! gravel.

use crate::item::Stack;
use crate::model::{emit_box, emit_item_flat_or_block};
use crate::util::vertex_light;
use crate::world::mesh::{flags, Vertex};
use crate::world::*;
use glam::{Mat4, Vec3};
pub struct ItemEntity {
    pub pos: Vec3,
    pub vel: Vec3,
    pub stack: Stack,
    pub age: f32,
    pub pickup_delay: f32,
    pickup: Option<PickupFlight>,
    /// The liquid in a dropped bucket, rocking as it flies and lands.
    slosh: crate::model::items::bucket::Slosh,
    /// Unique id for LAN play.
    pub id: u32,
}

struct PickupFlight {
    start: Vec3,
    elapsed: f32,
}

const PICKUP_TIME: f32 = 0.25;

pub struct FallingBlock {
    pub pos: Vec3,
    pub vel_y: f32,
    pub block: Block,
}

fn solid_at(w: &World, p: Vec3) -> bool {
    is_solid(w.get(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32))
}

impl ItemEntity {
    pub fn new(pos: Vec3, vel: Vec3, stack: Stack, pickup_delay: f32) -> Self {
        Self {
            pos,
            vel,
            stack,
            age: 0.0,
            pickup_delay,
            pickup: None,
            slosh: Default::default(),
            id: 0,
        }
    }

    pub fn is_picking_up(&self) -> bool {
        self.pickup.is_some()
    }

    pub fn start_pickup(&mut self, time: f32) {
        // Start from the rendered, bobbing position so the item does not jump.
        self.pos.y += (time * 2.2 + self.age).sin() * 0.06 + 0.15;
        self.pickup = Some(PickupFlight {
            start: self.pos,
            elapsed: 0.0,
        });
        self.vel = Vec3::ZERO;
    }

    /// Move toward the player's torso. Returns true once the visual can be removed.
    pub fn update_pickup(&mut self, dt: f32, target: Vec3) -> bool {
        let flight = self.pickup.as_mut().expect("item is being picked up");
        flight.elapsed = (flight.elapsed + dt).min(PICKUP_TIME);
        let t = flight.elapsed / PICKUP_TIME;
        let eased = t * t * (3.0 - 2.0 * t);
        self.pos = flight.start.lerp(target, eased);
        t >= 1.0
    }

    pub fn update(&mut self, dt: f32, w: &World) {
        self.age += dt;
        if let Some(fill) = crate::model::items::bucket::Fill::of(self.stack.item) {
            self.slosh.update(fill, self.pos, dt);
        }
        self.pickup_delay -= dt;
        let in_water = is_water(w.get(
            self.pos.x.floor() as i32,
            (self.pos.y + 0.1).floor() as i32,
            self.pos.z.floor() as i32,
        ));
        if in_water {
            self.vel.y = (self.vel.y + 6.0 * dt).min(1.0);
            self.vel.x *= 1.0 - dt * 2.0;
            self.vel.z *= 1.0 - dt * 2.0;
        } else {
            self.vel.y = (self.vel.y - 20.0 * dt).max(-40.0);
        }
        // Pushed out of blocks it ends up inside (e.g. a placed block).
        if solid_at(w, self.pos + Vec3::Y * 0.1) {
            self.pos.y += 4.0 * dt;
            self.vel = Vec3::ZERO;
            return;
        }
        // Moved in steps of under half a block, so a slow frame does not carry it through a
        // floor or a wall.
        let steps = ((self.vel.abs().max_element() * dt / 0.45).ceil() as usize).clamp(1, 16);
        let dt = dt / steps as f32;
        for _ in 0..steps {
            self.move_step(dt, w);
        }
    }

    fn move_step(&mut self, dt: f32, w: &World) {
        for axis in 0..3 {
            let mut p = self.pos;
            p[axis] += self.vel[axis] * dt;
            let probe = if axis == 1 && self.vel.y < 0.0 {
                p
            } else {
                p + Vec3::Y * 0.05
            };
            if solid_at(w, probe) {
                if axis == 1 && self.vel.y < 0.0 {
                    self.pos.y = self.pos.y.floor().max(p.y.floor() + 1.0);
                    // Ground friction.
                    self.vel.x *= 0.5;
                    self.vel.z *= 0.5;
                }
                self.vel[axis] = 0.0;
            } else {
                self.pos[axis] = p[axis];
            }
        }
    }

    pub fn build(&self, out: &mut Vec<Vertex>, time: f32, sky: u8, blk: u8) {
        let light = vertex_light(sky, blk);
        let elapsed = self.pickup.as_ref().map_or(0.0, |f| f.elapsed);
        let (pos, age) = (self.pos, self.age);
        let (pos, size) = if self.pickup.is_some() {
            let t = elapsed / PICKUP_TIME;
            (pos, 0.3 * (1.0 - t * t).max(0.01))
        } else {
            let bob = (time * 2.2 + age).sin() * 0.06 + 0.15;
            (pos + Vec3::Y * bob, 0.3)
        };
        let copies = if self.stack.count > 16 {
            3
        } else if self.stack.count > 1 {
            2
        } else {
            1
        };
        for i in 0..copies {
            let off =
                Vec3::new(i as f32 * 0.06, i as f32 * 0.05, -(i as f32) * 0.04) * (size / 0.3);
            let m = Mat4::from_translation(pos + off)
                * Mat4::from_rotation_y(age * 1.6 + time * 0.2);
            if let Some(fill) = crate::model::items::bucket::Fill::of(self.stack.item) {
                use crate::model::items::bucket;
                let scale = size * 1.5;
                let m = m * Mat4::from_translation(Vec3::Y * scale * 0.5) * Mat4::from_scale(Vec3::splat(scale));
                let surface = bucket::Surface { tilt: self.slosh.tilt, bounce: self.slosh.bounce, own_up: false };
                bucket::emit(out, m, fill, &surface, 1.0, light, flags::ENTITY);
                continue;
            }
            emit_item_flat_or_block(out, m, &self.stack, size, light, flags::ENTITY);
        }
    }
}

impl FallingBlock {
    /// Returns true when it has landed.
    pub fn update(&mut self, dt: f32, w: &World) -> bool {
        self.vel_y = (self.vel_y - 20.0 * dt).max(-40.0);
        let next = self.pos.y + self.vel_y * dt;
        let (x, z) = (self.pos.x.floor() as i32, self.pos.z.floor() as i32);
        // Every block passed this frame counts (a slow frame is several blocks of fall): it
        // lands on the first solid one, not through a thin floor.
        let from = (self.pos.y - 0.01).floor() as i32;
        let to = (next - 0.01).floor() as i32;
        for y in (to..=from).rev() {
            if is_solid(w.get(x, y, z)) {
                self.pos.y = y as f32 + 1.0;
                return true;
            }
        }
        if next < 0.0 {
            self.pos.y = next.floor().max(to as f32 + 1.0);
            return true;
        }
        self.pos.y = next;
        false
    }

    pub fn build(&self, out: &mut Vec<Vertex>, sky: u8, blk: u8) {
        let light = vertex_light(sky, blk);
        let layers = std::array::from_fn(|f| face_texture(self.block, f));
        let pos = self.pos;
        let min = Vec3::new(pos.x - 0.5, pos.y, pos.z - 0.5);
        emit_box(
            out,
            Mat4::IDENTITY,
            min,
            min + Vec3::ONE,
            layers,
            [[255; 3]; 6],
            light,
            flags::ENTITY,
        );
    }
}
