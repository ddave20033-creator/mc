//! Block entities: furnaces (smelting and grilling), chests with their contents, and their
//! models: chest lids, and the items lying in chests and on furnaces; and gun stations with
//! what lies on them.
//!
//! - `furnace`: smelting and grilling.
//! - `gun_bench`: what lies on a gun station and the last change there.
//! - `render`: the models, and where slots lie in chests.

mod furnace;
mod gun_bench;
mod render;

pub use furnace::{doneness, grill_box, part, Doneness, Furnace, Grilled, BURN_TIME, FLIP_TIME, GRILL_TIME};
pub use gun_bench::{bench_event, BenchEvent, BenchItem, GunBench, LOADER_ROUND};
pub use render::{
    build_chest_items, build_chest_lid, build_door, build_furnace_items, build_glow, chest_cell,
    chest_cell_at, chest_cell_size, chest_side, furnace_flame_spot,
};

use crate::item::{Slot, Stack};
use crate::net::container;
use crate::world::*;
use glam::IVec3;

/// Block entities that hold items.
#[derive(Default)]
pub struct BlockEntities {
    pub furnaces: FastMap<IVec3, Furnace>,
    pub chests: FastMap<IVec3, Box<[Slot; 27]>>,
    /// Gun stations (by their left half) and what lies on them.
    pub benches: FastMap<IVec3, GunBench>,
}

impl BlockEntities {
    /// A chest's contents: 27 slots, or 54 for a double chest (its left half first).
    pub fn chest_slots(&self, w: &World, p: IVec3) -> Vec<Slot> {
        let (a, b) = crate::sim::rules::chest_halves(w, p);
        let get = |q: IVec3| self.chests.get(&q).map_or([None; 27], |c| **c);
        let mut out = get(a).to_vec();
        if let Some(b) = b {
            out.extend(get(b));
        }
        out
    }

    /// Stores `slots` (as `chest_slots` gives them) into the chest's halves.
    pub fn set_chest_slots(&mut self, w: &World, p: IVec3, slots: &[Slot]) {
        let (a, b) = crate::sim::rules::chest_halves(w, p);
        for (q, part) in std::iter::once(a).chain(b).zip(slots.chunks(27)) {
            let c = self.chests.entry(q).or_insert_with(|| Box::new([None; 27]));
            for (s, v) in c.iter_mut().zip(part) {
                *s = *v;
            }
        }
    }

    /// What is in the chest (both halves of a double one) at `p`, as it is sent
    /// (`Msg::Container`): its kind and its slots.
    pub fn container(&self, w: &World, p: IVec3) -> Option<(u8, Vec<Slot>)> {
        if !is_chest(w.geti(p)) {
            return None;
        }
        self.chests.get(&p)?;
        Some((container::CHEST, self.chest_slots(w, p)))
    }

    /// Stores what a `Msg::Container` says is in the chest at `p`.
    pub fn apply_container(&mut self, w: &World, p: IVec3, kind: u8, slots: &[Slot]) {
        if kind == container::CHEST {
            let n = if crate::sim::rules::chest_halves(w, p).1.is_some() { 54 } else { 27 };
            let all: Vec<Slot> = (0..n).map(|i| slots.get(i).copied().flatten()).collect();
            self.set_chest_slots(w, p, &all);
        }
    }

    /// The block at `p` is now `b`: a block entity there of another kind is gone (a furnace,
    /// chest or gun station broken).
    pub fn forget_unless(&mut self, p: IVec3, b: Block) {
        if !is_furnace(b) {
            self.furnaces.remove(&p);
        }
        if !is_chest(b) {
            self.chests.remove(&p);
        }
        if !is_gun_bench(b) {
            self.benches.remove(&p);
        }
    }

    /// Removes the block entity at `p`, returning its contents.
    pub fn remove(&mut self, p: IVec3) -> Vec<Stack> {
        let mut out = Vec::new();
        if let Some(f) = self.furnaces.remove(&p) {
            out.extend(f.contents());
        }
        if let Some(c) = self.chests.remove(&p) {
            out.extend(c.iter().flatten().copied());
        }
        if let Some(b) = self.benches.remove(&p) {
            // What lies on it (the boxes of rounds belong to it: their rounds drop), and the
            // rounds in the boxes in the drawer.
            use crate::item::{box_ammo, box_count, AMMO_BOX, BOX_AMMO};
            let boxes = b.items.iter().filter(|i| i.stack.item == AMMO_BOX).map(|i| i.stack.data);
            let all: Vec<u16> = boxes.chain(b.boxes.iter().flatten().copied()).collect();
            out.extend(b.items.iter().filter(|i| i.stack.item != AMMO_BOX).map(|i| i.stack));
            // The loader and the magazine on it.
            if b.loader {
                out.push(Stack::one(crate::item::MAG_LOADER));
            }
            out.extend(b.loader_mag);
            // The grenades in the crate.
            for (kind, n) in [crate::item::FRAG_GRENADE, crate::item::SMOKE_GRENADE].into_iter().zip(b.grenades) {
                if n > 0 {
                    out.push(Stack::new(kind, n));
                }
            }
            for kind in BOX_AMMO {
                let mut rounds: u32 = all.iter().filter(|&&v| box_ammo(v) == Some(kind)).map(|&v| box_count(v) as u32).sum();
                while rounds > 0 {
                    let n = rounds.min(64) as u8;
                    out.push(Stack::new(kind, n));
                    rounds -= n as u32;
                }
            }
        }
        out
    }
}
