//! Furnaces without a screen, the same buttons everywhere: a right click puts in (meat on a
//! corner of the top, things to smelt into the front's mouth above, fuel into the firebox
//! below) and turns over the meat on a corner; a left click takes out what is there
//! (instead of mining the furnace; from the mouth what is smelted first).
//! What is smelted stays in the mouth until taken. The corner of the top under the
//! crosshair is lit up a little.

use crate::client::*;
use crate::entity::block_entity::{doneness, grill_box, part, Doneness, BURN_TIME};
use crate::audio::Sound;
use crate::entity::Furnace;
use crate::item::inventory;
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

    /// A left click on a furnace part with something in it takes that out instead of
    /// starting to mine the furnace; keeping the button held then does not mine it either
    /// (until it is let go). Returns true while mining is held off.
    pub(in crate::client) fn furnace_left_click(&mut self) -> bool {
        if !self.input.left_down {
            self.furnace_hold = false;
            return false;
        }
        if self.input.left_pressed {
            if let Some((p, k)) = self.furnace_part {
                if self.use_furnace(p, k, true) {
                    self.furnace_hold = true;
                    self.mining = None;
                }
            }
        }
        self.furnace_hold
    }

    /// Finds the furnace part under the crosshair (after targeting).
    pub(in crate::client) fn aim_furnace(&mut self) {
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
        let f = self.level.block_entities.furnaces.get(&p).unwrap_or(&empty);
        (f.has(k) || held.is_some_and(|h| f.accepts(k, h.item))).then_some((p, k))
    }

    /// The corner of a furnace's top where the held meat goes (or the meat to take or turn
    /// over lies) is lit up a little. The front's openings show the usual block outline.
    pub(in crate::client) fn furnace_frame(&self) -> Option<[Vec3; 4]> {
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

    /// A click on the aimed part of a furnace, the same everywhere: a right click puts in
    /// (meat on the top, things to smelt in the mouth, fuel in the firebox) and turns meat
    /// over; a left click (`take`) takes out (meat off the top; from the mouth what is
    /// smelted first, then what is still to smelt; the fuel). Returns false when it does
    /// nothing there.
    pub(in crate::client) fn use_furnace(&mut self, p: IVec3, k: u8, take: bool) -> bool {
        let slot = self.hotbar_slot;
        let held = self.inventory.slots[slot];
        let tier = furnace_tier(self.terrain.world.geti(p));
        let f = self.level.block_entities.furnaces.entry(p).or_default();
        f.tier = tier;
        let puts = held.is_some_and(|h| f.accepts(k, h.item));
        let able = if take {
            f.has(k)
        } else {
            puts || (k < 4 && f.has(k))
        };
        if !able {
            return false;
        }
        let r = f.use_part(k, held, take);
        if r.used > 0 && !self.creative() {
            inventory::take(&mut self.inventory.slots[slot], r.used);
        }
        // The server does it for real and sends back what comes out.
        let offered = held
            .filter(|_| r.used > 0)
            .map(|h| Stack { count: r.used, ..h });
        self.send(crate::net::Msg::FurnaceUse {
            p,
            part: k,
            take,
            offered,
        });
        self.hand.swing();
        self.action_cooldown = 0.2;
        true
    }

    /// What the burning furnaces near the player sound like: a furnace crackles, a blast or
    /// advanced furnace roars with its bellows (looping sounds: an id, the sound, where, how
    /// loud). A furnace that just finished smelting something dings.
    pub(in crate::client) fn furnace_sounds(&mut self) -> Vec<(u64, Sound, Vec3, f32)> {
        let near = self.player.pos;
        let mut loops = Vec::new();
        let mut dings = Vec::new();
        for (p, f) in &self.level.block_entities.furnaces {
            let at = p.as_vec3() + Vec3::splat(0.5);
            if at.distance_squared(near) > 24.0 * 24.0 {
                continue;
            }
            // (a furnace broken here, the server not having said so yet: quiet)
            let b = self.terrain.world.geti(*p);
            if !is_furnace(b) {
                continue;
            }
            let made = f.output.map_or(0, |s| s.count as u32);
            let before = self.level.furnace_heard.insert(*p, made);
            if before.is_some_and(|b| made > b) {
                dings.push(at);
            }
            if f.burn <= 0.0 {
                continue;
            }
            let id = (p.x as u64 & 0xfffff) << 40 | (p.y as u64 & 0xfffff) << 20 | (p.z as u64 & 0xfffff);
            let sound = match furnace_base(b) {
                Some(FURNACE) | None => Sound::FireCrackle,
                Some(_) => Sound::BlastRoar,
            };
            loops.push((id, sound, at, 0.8));
        }
        self.level.furnace_heard
            .retain(|p, _| self.level.block_entities.furnaces.contains_key(p));
        for at in dings {
            self.audio.play(Sound::SmeltDone, Some(at), 0.8);
        }
        loops
    }

    /// Steam and smoke off the meat on lit furnaces near the player: light while it cooks,
    /// more once the side on the fire is done, dark and thick when it burns.
    pub(in crate::client) fn furnace_fx(&mut self, dt: f32) {
        for (p, f) in self.level.block_entities.furnaces.iter_mut() {
            if self.terrain.world.is_loaded(p.x, p.z) {
                f.tier = furnace_tier(self.terrain.world.geti(*p));
            }
        }
        // Between the server's updates (every second, or when something changes) the
        // furnaces go on here as they do there: flips turn, fuel burns, meat cooks on
        // the side on the fire, smelting goes on.
        for f in self.level.block_entities.furnaces.values_mut() {
            for g in f.grill.iter_mut().flatten() {
                g.flip = (g.flip - dt).max(0.0);
            }
            if f.burn > 0.0 {
                f.burn = (f.burn - dt).max(0.0);
                for g in f.grill.iter_mut().flatten() {
                    if g.flip <= 0.0 {
                        g.cook[g.down as usize] += dt;
                    }
                }
                if f.input.is_some_and(|i| f.smelts(i.item).is_some()) {
                    f.cook = (f.cook + dt).min(f.smelt_time());
                }
            }
        }
        let near = self.player.pos;
        let mut puffs = Vec::new();
        let mut sparks = Vec::new();
        let mut fires = Vec::new();
        let mut chimneys = Vec::new();
        for (p, f) in &self.level.block_entities.furnaces {
            if f.burn <= 0.0 || p.as_vec3().distance_squared(near) > 40.0 * 40.0 {
                continue;
            }
            // Sparks fly out of the mouth while the piece in it is red hot.
            let heating = f.input.is_some_and(|i| f.smelts(i.item).is_some())
                && (0.3..0.7).contains(&(f.cook / f.smelt_time()));
            let b = self.terrain.world.geti(*p);
            if !is_furnace(b) {
                continue;
            }
            // Smoke out of a blast furnace's chimney, and the vents on an advanced furnace.
            match (furnace_base(b), facing(b)) {
                (Some(BLAST_FURNACE), _) if is_chimney(self.terrain.world.geti(*p + IVec3::Y)) => {
                    chimneys.push(p.as_vec3() + Vec3::new(0.5, 2.0, 0.5));
                }
                (Some(ADV_FURNACE), Some(fac)) => {
                    let r = furnace_right(fac).as_vec3();
                    for k in [0.3, 0.7, 1.3, 1.7] {
                        chimneys.push(p.as_vec3() + Vec3::new(0.5, 2.02, 0.5) + r * (k - 0.5));
                    }
                }
                _ => {}
            }
            // The resource pack's flame particles burn in the firebox, like on its torches.
            if let (true, Some(fac)) = (self.torch_particles, facing(b)) {
                // Bigger with more fuel: embers, a fire, a blaze.
                let n = match f.fuel.map_or(0, |s| s.count) {
                    0 => 1,
                    1..=16 => 2,
                    _ => 3,
                };
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
            // Several per frame at a high rate: a fire, not a few sparks.
            let mut due = dt * 12.0 * n as f32;
            while due > 0.0 {
                if self.random() < due.min(1.0) {
                    let k = self.random();
                    let at = crate::entity::block_entity::furnace_flame_spot(p, fac, k);
                    if self.random() < 0.75 {
                        self.particles.fire(at);
                    } else {
                        self.particles.flame(at + Vec3::Y * 0.08);
                    }
                }
                due -= 1.0;
            }
        }
        for top in chimneys {
            if self.random() < dt * 4.0 {
                let jitter = Vec3::new(self.random() - 0.5, 0.0, self.random() - 0.5) * 0.2;
                let w = &self.terrain.world;
                let (sky, blk) = (w.sky_estimate(top), w.block_light_estimate(top));
                let gray = 60 + (self.random() * 50.0) as u8;
                self.particles.smoke_shaded(top + jitter, gray, sky, blk);
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
