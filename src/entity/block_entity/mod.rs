//! Block entities: furnaces (smelting and grilling), chests and crafting tables with their
//! contents, and their models: chest lids, and the items lying in chests, on tables and
//! on furnaces; and gun stations with what lies on them.
//!
//! - `furnace`: smelting and grilling.
//! - `gun_bench`: what lies on a gun station and the last change there.
//! - `render`: the models, and where slots lie in chests and on tables.

mod furnace;
mod gun_bench;
mod render;

pub use furnace::{doneness, grill_box, part, Doneness, Furnace, Grilled, BURN_TIME, FLIP_TIME, GRILL_TIME};
pub use gun_bench::{bench_event, BenchEvent, BenchItem, GunBench, LOADER_ROUND};
pub use render::{
    build_chest_items, build_chest_lid, build_door, build_furnace_items, build_glow,
    build_table_items, build_table_made, chest_cell, chest_cell_at, chest_cell_size, chest_side,
    furnace_flame_spot, table_cell, table_cell_at, CHEST_FLOOR, CRAFT_SLIDE, TABLE_CELL,
};

use crate::item::{Slot, Stack};
use crate::world::*;
use glam::IVec3;

/// Block entities that hold items.
#[derive(Default)]
pub struct BlockEntities {
    pub furnaces: FastMap<IVec3, Furnace>,
    pub chests: FastMap<IVec3, Box<[Slot; 27]>>,
    /// Crafting tables keep whatever is left in their 3x3 grid.
    pub tables: FastMap<IVec3, [Slot; 9]>,
    /// Gun stations (by their left half) and what lies on them.
    pub benches: FastMap<IVec3, GunBench>,
}

impl BlockEntities {
    /// Removes the block entity at `p`, returning its contents.
    pub fn remove(&mut self, p: IVec3) -> Vec<Stack> {
        let mut out = Vec::new();
        if let Some(f) = self.furnaces.remove(&p) {
            out.extend(f.contents());
        }
        if let Some(c) = self.chests.remove(&p) {
            out.extend(c.iter().flatten().copied());
        }
        if let Some(t) = self.tables.remove(&p) {
            out.extend(t.iter().flatten().copied());
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
