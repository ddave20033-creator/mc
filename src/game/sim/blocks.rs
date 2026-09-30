//! Block rules: placing and breaking (with drops and block entities), double chests,
//! support for plants and torches, falling sand and gravel, and growing trees.

use crate::game::*;
use crate::item::inventory::{self};
use crate::item::*;
impl Game {
    pub(in crate::game) fn block_tint(&self, p: IVec3, b: u8) -> [u8; 3] {
        let (g, f) = self.terrain.gen.tints(p.x, p.z);
        match tint_kind(b, 0) {
            TintKind::Grass => g,
            TintKind::Foliage => f,
            TintKind::Spruce => SPRUCE_TINT,
            TintKind::Birch => BIRCH_TINT,
            TintKind::None => [255; 3],
        }
    }

    pub(in crate::game) fn set_block(&mut self, p: IVec3, b: u8) {
        let old = self.terrain.world.geti(p);
        if self.terrain.world.seti(p, b) {
            // (a cut trunk gone or changed took its cut with it: `World::set`)
            self.terrain.world.record_fluid_change(p, old, b, self.time);
            // Fluids flow on the host only.
            if !self.is_client() {
                self.fluids.notify(&self.terrain.world, p);
            }
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

    /// What happens to the world when a player mines a block: its drops (unless in creative),
    /// the contents of a chest/furnace/table, ice turning into water, and the block rules
    /// around it. On a LAN, the host runs this for everyone.
    pub(in crate::game) fn break_world(&mut self, p: IVec3, held: ItemId, creative: bool) {
        let b = self.terrain.world.geti(p);
        let mut contents = self.level.block_entities.remove(p);
        self.split_chest(p, b);
        contents.extend(self.remove_other_half(p, b));
        let replacement = self.left_after_mining(p, b, creative);
        self.set_block(p, replacement);
        self.bare_under_trunk(p, b);
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
        self.level.saplings.retain(|(q, _)| *q != p);
        self.block_updated(p);
    }

    /// A block placed (or a fluid poured or scooped up) by a player, with the world's rules:
    /// plants washed away by fluids, block entities, saplings, support and falling blocks.
    pub(in crate::game) fn place_world(&mut self, at: IVec3, b: u8) {
        let old = self.terrain.world.geti(at);
        if fluid_breaks(old) && is_fluid(b) {
            self.break_naturally(at);
        }
        let b = self.join_chest(at, b);
        self.set_block(at, b);
        if is_furnace(b) {
            self.level.block_entities.furnaces.insert(at, Default::default());
        } else if is_chest(b) {
            self.level.block_entities.chests.insert(at, Box::new([None; 27]));
        } else if is_sapling(b) {
            let t = 60.0 + self.random() * 120.0;
            self.level.saplings.push((at, t));
        }
        self.block_updated(at);
    }

    /// A double chest half placed at `at` turns the single chest it pairs with (facing the
    /// same way) into the other half. Without one it becomes a single chest.
    pub(in crate::game) fn join_chest(&mut self, at: IVec3, b: u8) -> u8 {
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
    pub(in crate::game) fn split_chest(&mut self, p: IVec3, b: u8) {
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
    pub(in crate::game) fn chest_to_place(&self, at: IVec3, hit: IVec3, facing: u8, sneaking: bool) -> u8 {
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
    pub(in crate::game) fn edit_block(&mut self, at: IVec3, b: u8) {
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
    pub(in crate::game) fn break_block(&mut self, p: IVec3) {
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
    /// So do the other blocks of a big furnace; what was in it is returned.
    pub(in crate::game) fn remove_other_half(&mut self, p: IVec3, b: u8) -> Vec<Stack> {
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

    /// A trunk cut down off grass leaves its mark on the grass under it: a circle of bare
    /// soil the grass slowly grows back over (`update_stump_marks`).
    pub(in crate::game) fn bare_under_trunk(&mut self, p: IVec3, b: u8) {
        if !is_log(b) || is_branch(b) || log_axis(b) != 1 {
            return;
        }
        let below = p - IVec3::Y;
        let g = self.terrain.world.geti(below);
        if matches!(g, GRASS | SNOWY_GRASS) {
            self.set_block(below, stump_mark(g, 0));
            self.block_updated(below);
        }
    }

    /// The grass growing back over the marks of cut-down trunks near the player, a stage at
    /// a time (about a minute each), where nothing covers them. Found by looking round, so
    /// marks in a world just loaded grow back too.
    pub(in crate::game) fn update_stump_marks(&mut self, dt: f32) {
        const EVERY: f32 = 2.0;
        const STAGE_SECS: f32 = 60.0;
        self.level.stump_scan -= dt;
        if self.level.stump_scan > 0.0 {
            return;
        }
        self.level.stump_scan = EVERY;
        let c = self.player.pos.floor().as_ivec3();
        let w = &self.terrain.world;
        let found: Vec<(IVec3, u8)> = crate::world::terrain::listed_near(&self.terrain.stump_marks, c, 32, 12)
            .map(|p| (p, w.geti(p)))
            .filter(|&(_, b)| is_stump_mark(b))
            .collect();
        for (p, b) in found {
            if self.random() >= EVERY / STAGE_SECS {
                continue;
            }
            let above = self.terrain.world.geti(p + IVec3::Y);
            if is_opaque(above) || is_log(above) {
                continue;
            }
            let next = if stump_stage(b) + 1 < STUMP_STAGES { b + 1 } else { soil(b) };
            self.set_block(p, next);
            self.block_updated(p);
        }
    }

    /// Broken by the world (lost support): always drops like a hand-mined block.
    pub(in crate::game) fn break_naturally(&mut self, p: IVec3) {
        let b = self.terrain.world.geti(p);
        let contents = self.remove_other_half(p, b);
        self.set_block(p, AIR);
        let r = self.random();
        for s in drops(b, NONE, r).into_iter().chain(contents) {
            self.spawn_drop(p.as_vec3() + Vec3::splat(0.5), s);
        }
        self.level.saplings.retain(|(q, _)| *q != p);
        self.block_updated(p);
    }

    pub(in crate::game) fn supported(w: &World, p: IVec3, b: u8) -> bool {
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

    /// Block rules after a change at `p`: support for plants/torches and falling sand.
    pub(in crate::game) fn block_updated(&mut self, p: IVec3) {
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

    pub(in crate::game) fn start_fall(&mut self, p: IVec3) {
        let b = self.terrain.world.geti(p);
        self.set_block(p, AIR);
        self.level.falling.push(FallingBlock {
            pos: Vec3::new(p.x as f32 + 0.5, p.y as f32, p.z as f32 + 0.5),
            vel_y: 0.0,
            block: b,
        });
        self.block_updated(p);
    }

    /// Grows a sapling into a tree (the generator's shapes). Returns false if there is not
    /// enough room for its wood.
    pub(in crate::game) fn grow_tree(&mut self, p: IVec3, sapling: u8) -> bool {
        let log = log_of_sapling(sapling);
        let seed = (self.random() * u32::MAX as f32) as u32;
        let shape = crate::world::trees::tree_shape(log, seed);
        let w = &self.terrain.world;
        let free = |b: u8| b == AIR || is_leaves(b) || is_plant(b) || is_sapling(b);
        if shape.iter().any(|&(d, _, soft)| !soft && !free(w.geti(p + d))) {
            return false;
        }
        for (d, b, soft) in shape {
            let q = p + d;
            let cur = self.terrain.world.geti(q);
            if !soft || cur == AIR || is_plant(cur) {
                self.set_block(q, b);
            }
        }
        true
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
            assert!(Game::supported(&world, at, torch));
        }
        world.seti(anchor, AIR);
        for offset in [IVec3::NEG_Z, IVec3::X, IVec3::Z, IVec3::NEG_X] {
            let torch = wall_torch_for_support(offset).unwrap();
            assert!(!Game::supported(&world, anchor - offset, torch));
        }
    }
}
