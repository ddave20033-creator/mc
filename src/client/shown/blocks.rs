//! Blocks changed here: set in this game's copy of the world, and placing and breaking as
//! this player does it, shown at once (the other blocks that go with it too, `sim::rules`)
//! and sent to the server, which has the last word.

use crate::client::*;
use crate::item::inventory::{self};
use crate::item::*;
impl Game {
    pub(in crate::client) fn block_tint(&self, p: IVec3, b: Block) -> [u8; 3] {
        let (g, f) = self.terrain.gen.tints(p.x, p.z);
        match tint_kind(b, 0) {
            TintKind::Grass => g,
            TintKind::Foliage => f,
            TintKind::Spruce => SPRUCE_TINT,
            TintKind::Birch => BIRCH_TINT,
            TintKind::None => [255; 3],
        }
    }

    pub(in crate::client) fn set_block(&mut self, p: IVec3, b: Block) {
        let old = self.terrain.world.geti(p);
        if self.terrain.world.seti(p, b) {
            // (a cut trunk gone or changed took its cut with it: `World::set`)
            self.terrain.world.record_fluid_change(p, old, b, self.time);
            // (fluids flow on the server: it sends what they do)
            self.terrain.block_changed(p, true);
        }
    }

    /// Drops an item entity with a little random pop.
    pub(in crate::client) fn spawn_drop(&mut self, center: Vec3, stack: Stack) {
        let vel = Vec3::new(
            (self.random() - 0.5) * 2.5,
            3.0 + self.random(),
            (self.random() - 0.5) * 2.5,
        );
        self.add_item(ItemEntity::new(center, vel, stack, 0.4));
    }

    /// A double chest half placed at `at` turns the single chest it pairs with into the other
    /// half (`rules::chest_join`); what goes at `at`.
    pub(in crate::client) fn join_chest(&mut self, at: IVec3, b: Block) -> Block {
        let (b, other) = crate::sim::rules::chest_join(&self.terrain.world, at, b);
        if let Some((q, ob)) = other {
            self.set_block(q, ob);
        }
        b
    }

    /// A chest's halves in inventory order (the left one seen from the front first), and
    /// whether it is a double chest.
    pub(in crate::client) fn chest_halves(&self, p: IVec3) -> (IVec3, Option<IVec3>) {
        crate::sim::rules::chest_halves(&self.terrain.world, p)
    }

    /// Chest to place at `at` for a player looking toward `facing`, like Minecraft: it joins a
    /// single chest beside it that faces the same way. Sneaking places a single chest, or
    /// one joining the chest it was placed against (lined up with it).
    pub(in crate::client) fn chest_to_place(&self, at: IVec3, hit: IVec3, facing: u8, sneaking: bool) -> Block {
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

    /// Changes a block for this player: shown right away (a chest joining the one beside it
    /// too), and sent to the server, which does it with the world's rules and puts right
    /// what was guessed wrong.
    pub(in crate::client) fn edit_block(&mut self, at: IVec3, b: Block) {
        let b = self.join_chest(at, b);
        self.set_block(at, b);
        self.send(crate::net::Msg::Place { p: at, b });
    }

    /// Mined by the player.
    pub(in crate::client) fn break_block(&mut self, p: IVec3) {
        let b = self.terrain.world.geti(p);
        let held = self.held();
        let creative = self.creative();
        let tint = self.block_tint(p, b);
        let replacement = crate::sim::rules::left_after_mining(&self.terrain.world, p, b, creative);
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

    /// The block `b` at `p` is going away: the blocks that belong with it follow
    /// (`rules::other_cells`; a big furnace's or gun station's things with them).
    pub(in crate::client) fn remove_other_half(&mut self, p: IVec3, b: Block) {
        if let Some(q) = crate::sim::rules::contents_elsewhere(&self.terrain.world, p, b) {
            self.level.block_entities.remove(q);
        }
        for (q, nb) in crate::sim::rules::other_cells(&self.terrain.world, p, b) {
            self.set_block(q, nb);
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub(in crate::client) fn wall_torch_needs_its_mounting_block() {
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
