//! Dropped items (falling, merging, picked up by whoever walks over them) and falling sand and
//! gravel, on the server.

use super::Server;
use crate::item::{max_stack, ItemId, Stack};
use crate::net::Msg;
use crate::world::*;
use glam::Vec3;

/// Seconds a dropped item lies before it is gone.
const ITEM_LIFE: f32 = 300.0;

impl Server {
    /// Dropped items: they fall and slide (not in chunks not loaded: they wait there instead of
    /// falling through the missing ground), go after five minutes, and are picked up by a
    /// living player walking over them (who gets it: `Msg::Give`).
    pub(super) fn update_items(&mut self, dt: f32) {
        let takers: Vec<(u8, Vec3)> = self
            .peers
            .iter()
            .filter_map(|p| p.alive_pose().map(|pose| (p.id, pose.pos + Vec3::Y * 0.9)))
            .collect();
        let mut given = Vec::new();
        let mut i = 0;
        while i < self.level.items.len() {
            let p = self.level.items[i].pos;
            if !self.world.is_loaded(p.x.floor() as i32, p.z.floor() as i32) {
                i += 1;
                continue;
            }
            self.level.items[i].update(dt, &self.world);
            let it = &self.level.items[i];
            if it.age > ITEM_LIFE || it.pos.y < -64.0 {
                self.level.items.swap_remove(i);
                continue;
            }
            let taker = (it.pickup_delay <= 0.0)
                .then(|| takers.iter().find(|(_, c)| (it.pos + Vec3::Y * 0.2).distance(*c) < 1.5))
                .flatten();
            if let Some(&(id, _)) = taker {
                let it = self.level.items.swap_remove(i);
                given.push((id, it.stack));
                continue;
            }
            i += 1;
        }
        for (id, stack) in given {
            self.send_to(id, &Msg::Give(stack));
        }
        self.merge_items();
    }

    /// Items of a kind lying together merge into one stack (as far as it goes).
    fn merge_items(&mut self) {
        const NEAR: f32 = 0.5;
        let items = &mut self.level.items;
        let mut order: Vec<usize> = (0..items.len()).collect();
        order.sort_unstable_by(|&a, &b| items[a].pos.x.total_cmp(&items[b].pos.x));
        let mut gone = vec![false; items.len()];
        for (i, &a) in order.iter().enumerate() {
            if gone[a] {
                continue;
            }
            for &b in &order[i + 1..] {
                if items[b].pos.x - items[a].pos.x >= NEAR {
                    break;
                }
                let (x, y) = (&items[a], &items[b]);
                let fits = x.stack.count as u16 + y.stack.count as u16 <= max_stack(x.stack.item) as u16;
                if !gone[b] && x.stack.stacks_with(&y.stack) && fits && x.pos.distance_squared(y.pos) < NEAR * NEAR {
                    gone[b] = true;
                    let (count, age, delay) = (y.stack.count, y.age, y.pickup_delay);
                    let x = &mut items[a];
                    x.stack.count += count;
                    x.age = x.age.min(age);
                    x.pickup_delay = x.pickup_delay.max(delay);
                }
            }
        }
        if gone.contains(&true) {
            let mut i = 0;
            items.retain(|_| {
                i += 1;
                !gone[i - 1]
            });
        }
    }

    /// Falling sand and gravel: landed, it is a block again (or, landing where a block is,
    /// drops as an item).
    pub(super) fn update_falling(&mut self, dt: f32) {
        let mut landed = Vec::new();
        let mut i = 0;
        while i < self.level.falling.len() {
            let p = self.level.falling[i].pos;
            if !self.world.is_loaded(p.x.floor() as i32, p.z.floor() as i32) {
                i += 1;
                continue;
            }
            if self.level.falling[i].update(dt, &self.world) {
                landed.push(self.level.falling.swap_remove(i));
            } else {
                i += 1;
            }
        }
        for f in landed {
            let at = f.pos.floor().as_ivec3();
            if is_replaceable(self.world.geti(at)) {
                self.set_block(at, f.block);
                self.block_updated(at);
            } else {
                self.spawn_drop(f.pos + Vec3::Y * 0.5, Stack::one(f.block as ItemId));
            }
        }
    }
}
