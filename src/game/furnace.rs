//! Furnaces without a screen: meat goes on the four corners of the top and is turned over by
//! hand, things to smelt go into the front's upper half and fuel into the lower half, and
//! what is smelted pops out of the front. The part under the crosshair gets a small outline
//! when there is something to do with it.

use super::*;
use crate::entity::block_entity::{doneness, grill_box, part, Doneness, BURN_TIME, SMELT_TIME};
use crate::entity::Furnace;
use crate::item::inventory::take;
use crate::item::*;

impl Game {
    /// The furnace part a ray hitting `hit` (from the air block `prev`) at `target_point`
    /// aims at: a corner of the top (0..4), or the front's upper or lower half.
    fn furnace_part_at(&self, hit: IVec3, prev: IVec3) -> Option<u8> {
        let b = self.terrain.world.geti(hit);
        let f = facing(b).filter(|_| is_furnace(b))?;
        let local = self.target_point - hit.as_vec3();
        let normal = prev - hit;
        if normal == IVec3::Y {
            Some((local.x >= 0.5) as u8 + 2 * (local.z >= 0.5) as u8)
        } else if normal == facing_dir(f) {
            Some(if local.y >= 0.5 {
                part::INPUT
            } else {
                part::FUEL
            })
        } else {
            None
        }
    }

    /// Finds the furnace part under the crosshair (after targeting).
    pub(super) fn aim_furnace(&mut self) {
        self.furnace_part = self
            .target
            .and_then(|(hit, prev)| self.furnace_part_at(hit, prev).map(|k| (hit, k)));
    }

    /// The aimed furnace part, if the held item can go in or there is something to take or
    /// turn over.
    fn furnace_part_active(&self) -> Option<(IVec3, u8)> {
        let (p, k) = self.furnace_part?;
        let held = self.inventory.slots[self.hotbar_slot];
        let empty = Furnace::default();
        let f = self.block_entities.furnaces.get(&p).unwrap_or(&empty);
        (f.has(k) || held.is_some_and(|h| f.accepts(k, h.item))).then_some((p, k))
    }

    /// What a right click does at the aimed furnace part (a hint under the crosshair).
    pub(super) fn furnace_hint(&self) -> Option<&'static str> {
        let (p, k) = self.furnace_part_active()?;
        let held = self.inventory.slots[self.hotbar_slot];
        let empty = Furnace::default();
        let f = self.block_entities.furnaces.get(&p).unwrap_or(&empty);
        let key = match (k < 4, f.has(k)) {
            (true, true) => "furnace.meat",
            (true, false) => "furnace.put",
            (false, _) if held.is_some_and(|h| f.accepts(k, h.item)) => "furnace.insert",
            (false, _) => "furnace.take",
        };
        Some(t(key))
    }

    /// The corner of a furnace's top where the held meat goes (or the meat to take or turn
    /// over lies) is lit up a little. The front's openings show the usual block outline.
    pub(super) fn furnace_frame(&self) -> Option<[Vec3; 4]> {
        let (p, k) = self.furnace_part_active()?;
        if k < 4 {
            let (lo, hi) = grill_box(k as usize);
            let (lo, hi) = (p.as_vec3() + lo, p.as_vec3() + hi);
            let y = p.y as f32 + 1.003;
            return Some([
                Vec3::new(lo.x, y, lo.z),
                Vec3::new(hi.x, y, lo.z),
                Vec3::new(hi.x, y, hi.z),
                Vec3::new(lo.x, y, hi.z),
            ]);
        }
        None
    }

    /// A right click on the aimed part of a furnace; `flip` when sneaking (turns the meat
    /// over). Returns false when it does nothing there.
    pub(super) fn use_furnace(&mut self, p: IVec3, k: u8, flip: bool) -> bool {
        let slot = self.hotbar_slot;
        let held = self.inventory.slots[slot];
        let f = self.block_entities.furnaces.entry(p).or_default();
        let able = if flip {
            k < 4 && f.has(k)
        } else {
            f.has(k) || held.is_some_and(|h| f.accepts(k, h.item))
        };
        if !able {
            return false;
        }
        let r = f.use_part(k, held, flip);
        if r.used > 0 && !self.creative() {
            take(&mut self.inventory.slots[slot], r.used);
        }
        if self.is_client() {
            // The host does it for real and sends back what comes out.
            let offered = held
                .filter(|_| r.used > 0)
                .map(|h| Stack { count: r.used, ..h });
            self.send(crate::net::Msg::FurnaceUse {
                p,
                part: k,
                flip,
                offered,
            });
        } else {
            for st in r.give {
                self.give(st);
            }
        }
        self.hand.swing();
        self.action_cooldown = 0.2;
        true
    }

    /// Host: a LAN player used a furnace. They already took `offered` from their hand;
    /// what did not go in comes back with whatever they took out.
    pub(super) fn remote_use_furnace(
        &mut self,
        id: u8,
        p: IVec3,
        k: u8,
        flip: bool,
        offered: Slot,
    ) {
        if !is_furnace(self.terrain.world.geti(p)) {
            if let Some(st) = offered {
                self.send_to(id, &crate::net::Msg::Give(st));
            }
            return;
        }
        let f = self.block_entities.furnaces.entry(p).or_default();
        let r = f.use_part(k, offered, flip);
        let mut back = r.give;
        if let Some(o) = offered {
            if o.count > r.used {
                back.push(Stack {
                    count: o.count - r.used,
                    ..o
                });
            }
        }
        for st in back {
            self.send_to(id, &crate::net::Msg::Give(st));
        }
        // Their copy may have guessed wrong (someone else was quicker): the real one.
        if let Some(f) = self.block_entities.furnaces.get(&p) {
            self.send_to(id, &Self::furnace_msg(p, f));
        }
    }

    /// Host: furnaces burn, cook and smelt; smelted items pop out of the front.
    pub(super) fn update_furnaces(&mut self, dt: f32) {
        let mut relight = Vec::new();
        let mut out = Vec::new();
        for (p, f) in self.block_entities.furnaces.iter_mut() {
            let lit = f.update(dt);
            let b = self.terrain.world.geti(*p);
            if let (true, Some(fac)) = (is_furnace(b), facing(b)) {
                let want = if lit {
                    FURNACE_LIT + fac
                } else {
                    FURNACE + fac
                };
                if want != b {
                    relight.push((*p, want));
                }
                if let Some(st) = f.output.take() {
                    out.push((*p, fac, st));
                }
            }
        }
        for (p, b) in relight {
            self.set_block(p, b);
        }
        for (p, fac, st) in out {
            let d = facing_dir(fac).as_vec3();
            let pos = p.as_vec3() + Vec3::new(0.5, 0.35, 0.5) + d * 0.62;
            let side = (self.random() - 0.5) * 0.6;
            let vel = d * 1.8 + Vec3::Y * 2.2 + d.cross(Vec3::Y) * side;
            self.add_item(ItemEntity::new(pos, vel, st, 0.3));
        }
    }

    /// Steam and smoke off the meat on lit furnaces near the player: light while it cooks,
    /// more once the side on the fire is done, dark and thick when it burns.
    pub(super) fn furnace_fx(&mut self, dt: f32) {
        if self.is_client() {
            // Flips turn and smelting goes on smoothly between the host's updates.
            for f in self.block_entities.furnaces.values_mut() {
                for g in f.grill.iter_mut().flatten() {
                    g.flip = (g.flip - dt).max(0.0);
                }
                if f.burn > 0.0 && f.input.is_some_and(|i| smelt(i.item).is_some()) {
                    f.cook = (f.cook + dt).min(SMELT_TIME);
                }
            }
        }
        let near = self.player.pos;
        let mut puffs = Vec::new();
        let mut sparks = Vec::new();
        let mut fires = Vec::new();
        for (p, f) in &self.block_entities.furnaces {
            if f.burn <= 0.0 || p.as_vec3().distance_squared(near) > 40.0 * 40.0 {
                continue;
            }
            // Sparks fly out of the mouth while the piece in it is red hot.
            let heating = f.input.is_some_and(|i| smelt(i.item).is_some())
                && (0.3..0.7).contains(&(f.cook / SMELT_TIME));
            let b = self.terrain.world.geti(*p);
            // The resource pack's flame particles burn in the firebox, like on its torches.
            if let (true, Some(fac)) = (self.torch_particles, facing(b)) {
                let n = f.fuel.map_or(1, |s| if s.count > 16 { 3 } else { 2 });
                fires.push((*p, fac, n));
            }
            if let (true, Some(fac)) = (heating, facing(b)) {
                let d = facing_dir(fac).as_vec3();
                let mouth = p.as_vec3() + Vec3::new(0.5, 0.66, 0.5) + d * 0.52;
                sparks.push((mouth, Vec3::Y.cross(d)));
            }
            for (i, g) in f.grill.iter().enumerate() {
                let Some(g) = g else { continue };
                if g.flip > 0.0 {
                    continue;
                }
                let t = g.cook[g.down as usize];
                let (rate, gray) = match doneness(t) {
                    Doneness::Raw => (1.2, 205),
                    Doneness::Cooked if t < BURN_TIME - 3.0 => (3.0, 150),
                    Doneness::Cooked => (7.0, 80),
                    Doneness::Burnt => (11.0, 35),
                };
                let (lo, hi) = grill_box(i);
                puffs.push((p.as_vec3() + (lo + hi) * 0.5 + Vec3::Y * 0.03, rate, gray));
            }
        }
        for (p, fac, n) in fires {
            if self.random() < dt * 6.0 * n as f32 {
                let k = self.random();
                let at = crate::entity::block_entity::furnace_flame_spot(p, fac, k);
                self.particles.flame(at);
            }
        }
        for (mouth, right) in sparks {
            if self.random() < dt * 2.5 {
                let x = (self.random() - 0.5) * 0.3;
                self.particles.flame(mouth + right * x);
            }
        }
        for (pos, rate, gray) in puffs {
            if self.random() < dt * rate {
                let jitter = Vec3::new(self.random() - 0.5, 0.0, self.random() - 0.5) * 0.25;
                let w = &self.terrain.world;
                let (sky, blk) = (w.sky_estimate(pos), w.block_light_estimate(pos));
                self.particles.smoke_shaded(pos + jitter, gray, sky, blk);
            }
        }
    }
}
