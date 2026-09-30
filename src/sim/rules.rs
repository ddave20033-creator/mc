//! The world's rules that the server applies and a player's game foresees alike: what holds a
//! block up, a double chest's halves, the other blocks of a door, bed, big furnace or gun
//! station, what a mined block leaves behind.

use crate::world::*;
use glam::IVec3;

/// Whether the block `b` at `p` is held up (a torch by what it hangs on, a plant by its
/// ground, a door's upper half by its lower one...).
pub fn supported(w: &World, p: IVec3, b: Block) -> bool {
    if let Some(offset) = torch_support_offset(b) {
        return is_opaque(w.geti(p + offset));
    }
    let below = w.geti(p - IVec3::Y);
    match b {
        _ if is_door(b) => {
            if door_upper(b) {
                is_door(below) && !door_upper(below)
            } else {
                is_solid(below) && !is_door(below)
            }
        }
        CACTUS => matches!(below, SAND | CACTUS),
        DEAD_BUSH => matches!(soil(below), SAND | DIRT | GRASS),
        _ if is_plant(b) => matches!(soil(below), GRASS | DIRT | SNOWY_GRASS),
        _ => true,
    }
}

/// A chest's halves in inventory order (the left one seen from the front first), and
/// whether it is a double chest.
pub fn chest_halves(w: &World, p: IVec3) -> (IVec3, Option<IVec3>) {
    let b = w.geti(p);
    match chest_partner_offset(b) {
        Some(d) if chest_partner_offset(w.geti(p + d)) == Some(-d) => {
            if base(b) == CHEST_LEFT {
                (p, Some(p + d))
            } else {
                (p + d, Some(p))
            }
        }
        _ => (p, None),
    }
}

/// What a mined block leaves behind: air, or water for ice (Minecraft: unless it was
/// floating, or mined in creative).
pub fn left_after_mining(w: &World, p: IVec3, b: Block, creative: bool) -> Block {
    if b == ICE && !creative && w.geti(p - IVec3::Y) != AIR {
        WATER
    } else {
        AIR
    }
}

/// A double chest half `b` placed at `at`: what goes there, and the single chest beside it
/// (facing the same way) that becomes its other half. Without one it is a single chest.
pub fn chest_join(w: &World, at: IVec3, b: Block) -> (Block, Option<(IVec3, Block)>) {
    let (Some(d), Some(f), Some(other)) = (chest_partner_offset(b), facing(b), chest_other_half(b)) else {
        return (b, None);
    };
    if w.geti(at + d) != chest_id(f, 0) {
        return (chest_id(f, 0), None);
    }
    (b, Some((at + d, other)))
}

/// The block `b` at `p` going away: what happens to the blocks that belong with it (a door's
/// or bed's other half and the rest of a big furnace or gun station go; a double chest's
/// other half becomes a single chest).
pub fn other_cells(w: &World, p: IVec3, b: Block) -> Vec<(IVec3, Block)> {
    if let Some(d) = chest_partner_offset(b) {
        let q = w.geti(p + d);
        if chest_partner_offset(q) == Some(-d) {
            return vec![(p + d, chest_id(facing(q).unwrap_or(0), 0))];
        }
        return Vec::new();
    }
    if let (Some(base), Some(f)) = (furnace_base(b).filter(|&k| k != FURNACE), facing(b)) {
        let origin = furnace_origin(p, b);
        return furnace_cells(base, f, false)
            .into_iter()
            .map(|(o, _)| origin + o)
            .filter(|&q| q != p && furnace_base(w.geti(q)) == Some(base))
            .map(|q| (q, AIR))
            .collect();
    }
    if is_gun_bench(b) {
        let Some(main) = bench_main(p, b, |q| w.geti(q)) else { return Vec::new() };
        // (the left block's id says how wide it is)
        let main_b = if main == p { b } else { w.geti(main) };
        return bench_cells(main, main_b).into_iter().filter(|&q| q != p && is_gun_bench(w.geti(q))).map(|q| (q, AIR)).collect();
    }
    let q = if is_door(b) {
        p + door_other_half(b)
    } else if is_bed(b) {
        p + bed_other_half(b)
    } else {
        return Vec::new();
    };
    let other = w.geti(q);
    if (is_door(b) && is_door(other)) || (is_bed(b) && is_bed(other)) {
        vec![(q, AIR)]
    } else {
        Vec::new()
    }
}

/// Where the things in the big furnace or gun station `b` at `p` are kept, when that is
/// another of its blocks (its furnace block, the station's left block).
pub fn contents_elsewhere(w: &World, p: IVec3, b: Block) -> Option<IVec3> {
    let keeper = if furnace_base(b).is_some_and(|k| k != FURNACE) && facing(b).is_some() {
        furnace_origin(p, b)
    } else if is_gun_bench(b) {
        bench_main(p, b, |q| w.geti(q))?
    } else {
        return None;
    };
    (keeper != p).then_some(keeper)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> World {
        let mut w = World::new();
        w.chunks.insert((0, 0), std::sync::Arc::new(ChunkData::new()));
        w
    }

    #[test]
    fn a_door_half_takes_the_other_with_it() {
        let mut w = world();
        let at = IVec3::new(4, 20, 4);
        w.seti(at, door_id(0, false, false, false));
        w.seti(at + IVec3::Y, door_id(0, false, true, false));
        assert_eq!(other_cells(&w, at, w.geti(at)), vec![(at + IVec3::Y, AIR)]);
        assert_eq!(other_cells(&w, at + IVec3::Y, w.geti(at + IVec3::Y)), vec![(at, AIR)]);
    }

    #[test]
    fn a_chest_joins_the_one_beside_it_and_splits_again() {
        let mut w = world();
        let at = IVec3::new(4, 20, 4);
        let f = 0;
        // (a half placed to the left of a single chest facing the same way)
        let placed = chest_id(f, 1);
        let beside = at + chest_partner_offset(placed).unwrap();
        w.seti(beside, chest_id(f, 0));
        let (b, other) = chest_join(&w, at, placed);
        let (q, ob) = other.expect("joined");
        assert_eq!((b, q), (placed, beside));
        w.seti(at, b);
        w.seti(q, ob);
        assert!(chest_halves(&w, at).1.is_some());
        assert_eq!(other_cells(&w, at, b), vec![(beside, chest_id(f, 0))]);
        // Nothing beside it: a single chest.
        let lone = IVec3::new(10, 20, 10);
        assert_eq!(chest_join(&w, lone, placed), (chest_id(f, 0), None));
    }
}
