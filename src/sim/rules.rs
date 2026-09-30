//! The world's rules that the server applies and a player's game foresees alike: what holds a
//! block up, a double chest's halves, what a mined block leaves behind.

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
