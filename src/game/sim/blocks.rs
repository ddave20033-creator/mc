//! Block rules: placing and breaking (with drops and block entities), double chests,
//! support for plants and torches, falling sand and gravel, and growing trees.

use crate::game::*;
use crate::item::inventory::{self};
use crate::item::*;
impl Game {
    pub(in crate::game) fn block_tint(&self, p: IVec3, b: Block) -> [u8; 3] {
        let (g, f) = self.terrain.gen.tints(p.x, p.z);
        match tint_kind(b, 0) {
            TintKind::Grass => g,
            TintKind::Foliage => f,
            TintKind::Spruce => SPRUCE_TINT,
            TintKind::Birch => BIRCH_TINT,
            TintKind::None => [255; 3],
        }
    }

    pub(in crate::game) fn set_block(&mut self, p: IVec3, b: Block) {
        let old = self.terrain.world.geti(p);
        if self.terrain.world.seti(p, b) {
            // (a cut trunk gone or changed took its cut with it: `World::set`)
            self.terrain.world.record_fluid_change(p, old, b, self.time);
            // Fluids flow on the host only.
            self.terrain.block_changed(p, true);
        }
    }

    /// Drops an item entity with a little random pop.
    pub(in crate::game) fn spawn_drop(&mut self, center: Vec3, stack: Stack) {
        let vel = Vec3::new(
            (self.random() - 0.5) * 2.5,
            3.0 + self.random(),
            (self.random() - 0.5) * 2.5,
        );
        self.add_item(ItemEntity::new(center, vel, stack, 0.4));
    }

    /// A double chest half placed at `at` turns the single chest it pairs with (facing the
    /// same way) into the other half. Without one it becomes a single chest.
    pub(in crate::game) fn join_chest(&mut self, at: IVec3, b: Block) -> Block {
        let (Some(d), Some(f), Some(other)) =
            (chest_partner_offset(b), facing(b), chest_other_half(b))
        else {
            return b;
        };
        if self.terrain.world.geti(at + d) != chest_id(f, 0) {
            return chest_id(f, 0);
        }
        self.set_block(at + d, other);
        b
    }

    /// A double chest half is going away: the other half becomes a single chest.
    pub(in crate::game) fn split_chest(&mut self, p: IVec3, b: Block) {
        let Some(d) = chest_partner_offset(b) else {
            return;
        };
        let q = self.terrain.world.geti(p + d);
        if chest_partner_offset(q) == Some(-d) {
            self.set_block(p + d, chest_id(facing(q).unwrap_or(0), 0));
        }
    }

    /// A chest's halves in inventory order (the left one seen from the front first), and
    /// whether it is a double chest.
    pub(in crate::game) fn chest_halves(&self, p: IVec3) -> (IVec3, Option<IVec3>) {
        crate::sim::rules::chest_halves(&self.terrain.world, p)
    }

    /// Chest to place at `at` for a player looking toward `facing`, like Minecraft: it joins a
    /// single chest beside it that faces the same way. Sneaking places a single chest, or
    /// one joining the chest it was placed against (lined up with it).
    pub(in crate::game) fn chest_to_place(&self, at: IVec3, hit: IVec3, facing: u8, sneaking: bool) -> Block {
        let w = &self.terrain.world;
        let single = |q: IVec3| {
            let b = w.geti(q);
            (base(b) == CHEST).then(|| (b - CHEST) as u8)
        };
        if sneaking {
            if let Some(f) = single(hit) {
                let side = (hit - at).dot(chest_right(f));
                if side != 0 {
                    return chest_id(f, side);
                }
            }
            return chest_id(facing, 0);
        }
        for side in [-1, 1] {
            if single(at + chest_right(facing) * side) == Some(facing) {
                return chest_id(facing, side);
            }
        }
        chest_id(facing, 0)
    }

    /// Changes a block for this player: in single player and on the host with the world's
    /// rules; a LAN player shows it right away and lets the host do the rest.
    pub(in crate::game) fn edit_block(&mut self, at: IVec3, b: Block) {
        let b = self.join_chest(at, b);
        self.set_block(at, b);
        self.send(crate::net::Msg::Place { p: at, b });
    }

    /// Mined by the player.
    pub(in crate::game) fn break_block(&mut self, p: IVec3) {
        let b = self.terrain.world.geti(p);
        let held = self.held();
        let creative = self.creative();
        let tint = self.block_tint(p, b);
        let replacement = crate::sim::rules::left_after_mining(&self.terrain.world, p, b, creative);
        self.split_chest(p, b);
        self.remove_other_half(p, b);
        self.set_block(p, replacement);
        self.send(crate::net::Msg::Break { p, held, creative });
        if !creative {
            self.needs.exhaust(crate::entity::survival::cost::MINE);
            let wear = wear(held, b);
            let slot = self.hotbar_slot;
            if inventory::damage(&mut self.inventory.slots[slot], wear) {
                // Tool broke.
                self.particles
                    .burst(&self.terrain.world, p, STONE, 12, [255; 3]);
            }
        }
        self.particles.burst(&self.terrain.world, p, b, 28, tint);
        self.hand.swing();
        self.mining = None;
        self.action_cooldown = if creative { 0.2 } else { 0.15 };
    }

    /// A door or bed half is going away: the other half goes with it (without a second drop).
    /// So do the other blocks of a big furnace; what was in it is returned.
    pub(in crate::game) fn remove_other_half(&mut self, p: IVec3, b: Block) -> Vec<Stack> {
        if let (Some(base), Some(f)) = (furnace_base(b).filter(|&k| k != FURNACE), facing(b)) {
            let origin = furnace_origin(p, b);
            let contents = if origin != p {
                self.level.block_entities.remove(origin)
            } else {
                Vec::new()
            };
            for (o, _) in furnace_cells(base, f, false) {
                let q = origin + o;
                if q != p && furnace_base(self.terrain.world.geti(q)) == Some(base) {
                    self.set_block(q, AIR);
                }
            }
            return contents;
        }
        if is_gun_bench(b) {
            // Its other blocks go too; what lay on the table (kept by its left block) drops.
            let w = &self.terrain.world;
            let Some(main) = bench_main(p, b, |q| w.geti(q)) else { return Vec::new() };
            // (the left block's id says how wide it is; it is gone already when it was broken)
            let main_b = if main == p { b } else { w.geti(main) };
            let contents = if main != p { self.level.block_entities.remove(main) } else { Vec::new() };
            for q in bench_cells(main, main_b) {
                if q != p && is_gun_bench(self.terrain.world.geti(q)) {
                    self.set_block(q, AIR);
                }
            }
            return contents;
        }
        let q = if is_door(b) {
            p + door_other_half(b)
        } else if is_bed(b) {
            p + bed_other_half(b)
        } else {
            return Vec::new();
        };
        let other = self.terrain.world.geti(q);
        if (is_door(b) && is_door(other)) || (is_bed(b) && is_bed(other)) {
            self.set_block(q, AIR);
        }
        Vec::new()
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub(in crate::game) fn wall_torch_needs_its_mounting_block() {
        let mut world = World::new();
        let mut chunk = ChunkData::new();
        let anchor = IVec3::new(8, 10, 8);
        chunk.set(
            anchor.x as usize,
            anchor.y as usize,
            anchor.z as usize,
            STONE,
        );
        world.chunks.insert((0, 0), Arc::new(chunk));

        for offset in [IVec3::NEG_Z, IVec3::X, IVec3::Z, IVec3::NEG_X] {
            let torch = wall_torch_for_support(offset).unwrap();
            let at = anchor - offset;
            assert!(crate::sim::rules::supported(&world, at, torch));
        }
        world.seti(anchor, AIR);
        for offset in [IVec3::NEG_Z, IVec3::X, IVec3::Z, IVec3::NEG_X] {
            let torch = wall_torch_for_support(offset).unwrap();
            assert!(!crate::sim::rules::supported(&world, anchor - offset, torch));
        }
    }
}
