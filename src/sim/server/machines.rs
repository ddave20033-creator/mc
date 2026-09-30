//! Machines on the server: furnaces burning, cooking and smelting (and a player putting in or
//! taking out), and the gun stations (what lies on them comes from the players who work at
//! them; the rifle station's magazine loader runs here).

use super::peers::furnace_msg;
use super::Server;
use crate::entity::{GunBench, LOADER_ROUND};
use crate::item::{box_without, gun_rounds, set_gun_rounds, Slot, Stack};
use crate::net::Msg;
use crate::world::*;
use glam::IVec3;

impl Server {
    /// Furnaces burn, cook and smelt; all of a big furnace glows while it burns.
    pub(super) fn update_furnaces(&mut self, dt: f32) {
        let mut relight = Vec::new();
        for (p, f) in self.level.block_entities.furnaces.iter_mut() {
            let b = self.world.geti(*p);
            // (in a chunk not loaded the block reads as air: it keeps the tier it had)
            if self.world.is_loaded(p.x, p.z) {
                f.tier = furnace_tier(b);
            }
            let lit = f.update(dt);
            if let (true, Some(base), Some(fac)) = (is_furnace(b), furnace_base(b), facing(b)) {
                for (o, want) in furnace_cells(base, fac, lit) {
                    let q = *p + o;
                    let cur = self.world.geti(q);
                    if cur != want && furnace_base(cur) == Some(base) {
                        relight.push((q, want));
                    }
                }
            }
        }
        for (p, b) in relight {
            self.set_block(p, b);
        }
    }

    /// Player `id` used part `k` of the furnace at `p` (`take`: a left click). They already
    /// took `offered` from their hand; what did not go in comes back to them with whatever they
    /// took out.
    pub(super) fn use_furnace(&mut self, id: u8, p: IVec3, k: u8, take: bool, offered: Slot) {
        if !is_furnace(self.world.geti(p)) {
            if let Some(st) = offered {
                self.send_to(id, &Msg::Give(st));
            }
            return;
        }
        let tier = furnace_tier(self.world.geti(p));
        let f = self.level.block_entities.furnaces.entry(p).or_default();
        f.tier = tier;
        let r = f.use_part(k, offered, take);
        let mut back = r.give;
        if let Some(o) = offered {
            if o.count > r.used {
                back.push(Stack { count: o.count - r.used, ..o });
            }
        }
        for st in back {
            self.send_to(id, &Msg::Give(st));
        }
        // Their copy may have guessed wrong (someone else was quicker): the real one.
        if let Some(f) = self.level.block_entities.furnaces.get(&p) {
            self.send_to(id, &furnace_msg(p, f));
        }
    }

    /// What lies on a gun station, as a player working at it changed it.
    pub(super) fn set_bench(&mut self, p: IVec3, bench: GunBench) {
        self.level.block_entities.benches.insert(p, bench);
    }

    /// Each loader with a magazine on it that is not full pushes a round into it from a box
    /// of the rounds it takes, one after another; everyone sees it.
    pub(super) fn update_loaders(&mut self, dt: f32) {
        let busy: Vec<(IVec3, usize)> =
            self.level.block_entities.benches.iter().filter_map(|(p, b)| b.loader_source().map(|i| (*p, i))).collect();
        self.level.loader_feed.retain(|p, _| busy.iter().any(|(q, _)| q == p));
        for (p, i) in busy {
            let t = self.level.loader_feed.entry(p).or_insert(0.0);
            *t += dt;
            if *t < LOADER_ROUND {
                continue;
            }
            *t -= LOADER_ROUND;
            let Some(bench) = self.level.block_entities.benches.get_mut(&p) else { continue };
            let (Some(v), Some(mag)) = (bench.boxes[i], bench.loader_mag.as_mut()) else { continue };
            bench.boxes[i] = Some(box_without(v, 1));
            let r = gun_rounds(mag) + 1;
            set_gun_rounds(mag, r);
            let msg = Msg::Bench { p, bench: bench.clone() };
            self.broadcast(&msg, None);
        }
    }
}
