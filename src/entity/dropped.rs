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
    pub block: u8,
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
            self.vel.y -= 20.0 * dt;
        }
        // Pushed out of blocks it ends up inside (e.g. a placed block).
        if solid_at(w, self.pos + Vec3::Y * 0.1) {
            self.pos.y += 4.0 * dt;
            self.vel = Vec3::ZERO;
            return;
        }
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
        let (pos, size) = if let Some(flight) = &self.pickup {
            let t = flight.elapsed / PICKUP_TIME;
            (self.pos, 0.3 * (1.0 - t * t).max(0.01))
        } else {
            let bob = (time * 2.2 + self.age).sin() * 0.06 + 0.15;
            (self.pos + Vec3::Y * bob, 0.3)
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
                * Mat4::from_rotation_y(self.age * 1.6 + time * 0.2);
            emit_item_flat_or_block(out, m, &self.stack, size, light, flags::ENTITY);
        }
    }
}

impl FallingBlock {
    /// Returns true when it has landed.
    pub fn update(&mut self, dt: f32, w: &World) -> bool {
        self.vel_y = (self.vel_y - 20.0 * dt).max(-40.0);
        let next = self.pos.y + self.vel_y * dt;
        let below = w.get(
            self.pos.x.floor() as i32,
            (next - 0.01).floor() as i32,
            self.pos.z.floor() as i32,
        );
        if is_solid(below) || next < 0.0 {
            self.pos.y = next.floor().max((next - 0.01).floor() + 1.0);
            return true;
        }
        self.pos.y = next;
        false
    }

    pub fn build(&self, out: &mut Vec<Vertex>, sky: u8, blk: u8) {
        let light = vertex_light(sky, blk);
        let layers = std::array::from_fn(|f| face_texture(self.block, f));
        let min = Vec3::new(self.pos.x - 0.5, self.pos.y, self.pos.z - 0.5);
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
