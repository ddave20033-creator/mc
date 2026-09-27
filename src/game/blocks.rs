//! Block rules: placing and breaking (with drops and block entities), double chests,
//! support for plants and torches, falling sand and gravel, and growing trees.

use super::*;
use crate::item::inventory::{self};
use crate::item::*;
impl Game {
    pub(super) fn block_tint(&self, p: IVec3, b: u8) -> [u8; 3] {
        let (g, f) = self.terrain.gen.tints(p.x, p.z);
        match tint_kind(b, 0) {
            TintKind::Grass => g,
            TintKind::Foliage => f,
            TintKind::Spruce => SPRUCE_TINT,
            TintKind::Birch => BIRCH_TINT,
            TintKind::None => [255; 3],
        }
    }

    pub(super) fn set_block(&mut self, p: IVec3, b: u8) {
        let old = self.terrain.world.geti(p);
        if self.terrain.world.seti(p, b) {
            self.terrain.world.record_fluid_change(p, old, b, self.time);
            // Fluids flow on the host only.
            if !self.is_client() {
                self.fluids.notify(&self.terrain.world, p);
            }
            self.terrain.block_changed(p, true);
        }
    }

    /// Drops an item entity with a little random pop.
    pub(super) fn spawn_drop(&mut self, center: Vec3, stack: Stack) {
        let vel = Vec3::new(
            (self.random() - 0.5) * 2.5,
            3.0 + self.random(),
            (self.random() - 0.5) * 2.5,
        );
        self.add_item(ItemEntity::new(center, vel, stack, 0.4));
    }

    /// What happens to the world when a player mines a block: its drops (unless in creative),
    /// the contents of a chest/furnace/table, ice turning into water, and the block rules
    /// around it. On a LAN, the host runs this for everyone.
    pub(super) fn break_world(&mut self, p: IVec3, held: ItemId, creative: bool) {
        let b = self.terrain.world.geti(p);
        let contents = self.block_entities.remove(p);
        self.split_chest(p, b);
        self.remove_other_half(p, b);
        let replacement = self.left_after_mining(p, b, creative);
        self.set_block(p, replacement);
        let center = p.as_vec3() + Vec3::splat(0.5);
        if !creative {
            let r = self.random();
            for s in drops(b, held, r) {
                self.spawn_drop(center, s);
            }
        }
        for s in contents {
            self.spawn_drop(center, s);
        }
        self.saplings.retain(|(q, _)| *q != p);
        self.block_updated(p);
    }

    /// A block placed (or a fluid poured or scooped up) by a player, with the world's rules:
    /// plants washed away by fluids, block entities, saplings, support and falling blocks.
    pub(super) fn place_world(&mut self, at: IVec3, b: u8) {
        let old = self.terrain.world.geti(at);
        if fluid_breaks(old) && is_fluid(b) {
            self.break_naturally(at);
        }
        let b = self.join_chest(at, b);
        self.set_block(at, b);
        if is_furnace(b) {
            self.block_entities.furnaces.insert(at, Default::default());
        } else if is_chest(b) {
            self.block_entities.chests.insert(at, Box::new([None; 27]));
        } else if is_sapling(b) {
            let t = 60.0 + self.random() * 120.0;
            self.saplings.push((at, t));
        }
        self.block_updated(at);
    }

    /// A double chest half placed at `at` turns the single chest it pairs with (facing the
    /// same way) into the other half. Without one it becomes a single chest.
    pub(super) fn join_chest(&mut self, at: IVec3, b: u8) -> u8 {
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
    pub(super) fn split_chest(&mut self, p: IVec3, b: u8) {
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
    pub(super) fn chest_halves(&self, p: IVec3) -> (IVec3, Option<IVec3>) {
        let w = &self.terrain.world;
        let b = w.geti(p);
        match chest_partner_offset(b) {
            Some(d) if chest_partner_offset(w.geti(p + d)) == Some(-d) => {
                if (CHEST_LEFT..CHEST_LEFT + 4).contains(&b) {
                    (p, Some(p + d))
                } else {
                    (p + d, Some(p))
                }
            }
            _ => (p, None),
        }
    }

    /// Chest to place at `at` for a player looking toward `facing`, like Minecraft: it joins a
    /// single chest beside it that faces the same way. Sneaking places a single chest, or
    /// one joining the chest it was placed against (lined up with it).
    pub(super) fn chest_to_place(&self, at: IVec3, hit: IVec3, facing: u8, sneaking: bool) -> u8 {
        let w = &self.terrain.world;
        let single = |q: IVec3| {
            let b = w.geti(q);
            (CHEST..CHEST + 4).contains(&b).then(|| b - CHEST)
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
    pub(super) fn edit_block(&mut self, at: IVec3, b: u8) {
        if self.is_client() {
            let b = self.join_chest(at, b);
            self.set_block(at, b);
            self.send(crate::net::Msg::Place { p: at, b });
        } else {
            self.place_world(at, b);
        }
    }

    /// What a mined block leaves behind: air, or water for ice (Minecraft: unless it was
    /// floating, or mined in creative).
    fn left_after_mining(&self, p: IVec3, b: u8, creative: bool) -> u8 {
        if b == ICE && !creative && self.terrain.world.geti(p - IVec3::Y) != AIR {
            WATER
        } else {
            AIR
        }
    }

    /// Mined by the player.
    pub(super) fn break_block(&mut self, p: IVec3) {
        let b = self.terrain.world.geti(p);
        let held = self.held();
        let creative = self.creative();
        let tint = self.block_tint(p, b);
        if self.is_client() {
            let replacement = self.left_after_mining(p, b, creative);
            self.split_chest(p, b);
            self.remove_other_half(p, b);
            self.set_block(p, replacement);
            self.send(crate::net::Msg::Break { p, held, creative });
        } else {
            self.break_world(p, held, creative);
            // The other LAN players see the debris too.
            self.break_fx(p, b, false, None);
        }
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
    pub(super) fn remove_other_half(&mut self, p: IVec3, b: u8) {
        let q = if is_door(b) {
            p + door_other_half(b)
        } else if is_bed(b) {
            p + bed_other_half(b)
        } else {
            return;
        };
        let other = self.terrain.world.geti(q);
        if (is_door(b) && is_door(other)) || (is_bed(b) && is_bed(other)) {
            self.set_block(q, AIR);
        }
    }

    /// Broken by the world (lost support): always drops like a hand-mined block.
    pub(super) fn break_naturally(&mut self, p: IVec3) {
        let b = self.terrain.world.geti(p);
        self.remove_other_half(p, b);
        self.set_block(p, AIR);
        let r = self.random();
        for s in drops(b, NONE, r) {
            self.spawn_drop(p.as_vec3() + Vec3::splat(0.5), s);
        }
        self.saplings.retain(|(q, _)| *q != p);
        self.block_updated(p);
    }

    pub(super) fn supported(w: &World, p: IVec3, b: u8) -> bool {
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
            DEAD_BUSH => matches!(below, SAND | DIRT | GRASS),
            _ if is_plant(b) => matches!(below, GRASS | DIRT | SNOWY_GRASS),
            _ => true,
        }
    }

    /// Block rules after a change at `p`: support for plants/torches and falling sand.
    pub(super) fn block_updated(&mut self, p: IVec3) {
        let above = p + IVec3::Y;
        for q in [
            above,
            p + IVec3::X,
            p - IVec3::X,
            p + IVec3::Z,
            p - IVec3::Z,
        ] {
            let b = self.terrain.world.geti(q);
            if needs_support(b) && !Self::supported(&self.terrain.world, q, b) {
                self.break_naturally(q);
            }
        }
        let w = &self.terrain.world;
        if has_gravity(w.geti(above)) && !is_solid(w.geti(p)) {
            self.start_fall(above);
        }
        let w = &self.terrain.world;
        if p.y > 0 && has_gravity(w.geti(p)) && !is_solid(w.geti(p - IVec3::Y)) {
            self.start_fall(p);
        }
    }

    pub(super) fn start_fall(&mut self, p: IVec3) {
        let b = self.terrain.world.geti(p);
        self.set_block(p, AIR);
        self.falling.push(FallingBlock {
            pos: Vec3::new(p.x as f32 + 0.5, p.y as f32, p.z as f32 + 0.5),
            vel_y: 0.0,
            block: b,
        });
        self.block_updated(p);
    }

    /// Grows a sapling into a tree. Returns false if there is not enough room.
    pub(super) fn grow_tree(&mut self, p: IVec3, sapling: u8) -> bool {
        let r = self.random();
        let (log, leaves, trunk) = match sapling {
            BIRCH_SAPLING => (BIRCH_LOG, BIRCH_LEAVES, 5 + (r * 3.0) as i32),
            SPRUCE_SAPLING => (SPRUCE_LOG, SPRUCE_LEAVES, 6 + (r * 4.0) as i32),
            _ => (OAK_LOG, OAK_LEAVES, 4 + (r * 3.0) as i32),
        };
        let w = &self.terrain.world;
        if (1..=trunk + 1).any(|dy| {
            !matches!(w.geti(p + IVec3::Y * dy), AIR) && !is_leaves(w.geti(p + IVec3::Y * dy))
        }) {
            return false;
        }
        let mut leaf = Vec::new();
        if sapling == SPRUCE_SAPLING {
            let top = p.y + trunk;
            leaf.push(IVec3::new(p.x, top, p.z));
            for i in 1..trunk - 1 {
                let rad = if i % 2 == 1 { 1 } else { (1 + i / 3).min(3) };
                for dx in -rad..=rad {
                    for dz in -rad..=rad {
                        if dx.abs() + dz.abs() > rad + 1
                            || (dx.abs() == rad && dz.abs() == rad && rad > 1)
                        {
                            continue;
                        }
                        leaf.push(IVec3::new(p.x + dx, top - i, p.z + dz));
                    }
                }
            }
        } else {
            let top = p.y + trunk - 1;
            for dy in -2..=1 {
                let rad: i32 = if dy <= -1 { 2 } else { 1 };
                for dx in -rad..=rad {
                    for dz in -rad..=rad {
                        let corner = dx.abs() == rad && dz.abs() == rad;
                        if corner && (dy == 1 || self.random() < 0.5) {
                            continue;
                        }
                        leaf.push(IVec3::new(p.x + dx, top + dy, p.z + dz));
                    }
                }
            }
        }
        for q in leaf {
            let b = self.terrain.world.geti(q);
            if b == AIR || is_plant(b) {
                self.set_block(q, leaves);
            }
        }
        for dy in 0..trunk {
            self.set_block(p + IVec3::Y * dy, log);
        }
        if self.terrain.world.geti(p - IVec3::Y) == GRASS {
            self.set_block(p - IVec3::Y, DIRT);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub(super) fn wall_torch_needs_its_mounting_block() {
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
            assert!(Game::supported(&world, at, torch));
        }
        world.seti(anchor, AIR);
        for offset in [IVec3::NEG_Z, IVec3::X, IVec3::Z, IVec3::NEG_X] {
            let torch = wall_torch_for_support(offset).unwrap();
            assert!(!Game::supported(&world, anchor - offset, torch));
        }
    }
}
