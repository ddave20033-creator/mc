//! Block rules on the server: placing and breaking (with drops and block entities), double
//! chests, the other halves of doors, beds, big furnaces and gun stations, support for plants
//! and torches, falling sand and gravel, trees growing from saplings and grass growing back
//! over stump marks.

use super::Server;
use crate::entity::{FallingBlock, ItemEntity};
use crate::item::{drops, ItemId, Stack, NONE};
use crate::sim::rules::{chest_halves, chest_join, contents_elsewhere, left_after_mining, other_cells, supported};
use crate::world::*;
use glam::{IVec3, Vec3};

impl Server {
    /// Sets a block (the players are told: `World::log`), fluids next to it wake up.
    pub fn set_block(&mut self, p: IVec3, b: Block) {
        let old = self.world.geti(p);
        if self.world.seti(p, b) {
            self.fluids.notify(&self.world, p);
            if is_stump_mark(b) && !is_stump_mark(old) {
                self.stump_marks.entry(World::chunk_pos(p.x, p.z)).or_default().push(p);
            }
        }
    }

    /// Drops an item entity with a little random pop.
    pub fn spawn_drop(&mut self, center: Vec3, stack: Stack) {
        let vel = Vec3::new((self.random() - 0.5) * 2.5, 3.0 + self.random(), (self.random() - 0.5) * 2.5);
        self.add_item(ItemEntity::new(center, vel, stack, 0.4));
    }

    pub fn add_item(&mut self, mut it: ItemEntity) {
        it.id = self.entity_id();
        self.level.items.push(it);
    }

    /// What happens to the world when a player mines a block: its drops (unless in creative),
    /// the contents of a chest, furnace or table, ice turning into water, and the block rules
    /// around it.
    pub fn break_world(&mut self, p: IVec3, held: ItemId, creative: bool) {
        let b = self.world.geti(p);
        let mut contents = self.level.block_entities.remove(p);
        contents.extend(self.remove_other_half(p, b));
        let replacement = left_after_mining(&self.world, p, b, creative);
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
    pub fn place_world(&mut self, at: IVec3, b: Block) {
        let old = self.world.geti(at);
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

    /// A double chest half placed at `at` turns the single chest it pairs with into the other
    /// half (`rules::chest_join`); what goes at `at`.
    fn join_chest(&mut self, at: IVec3, b: Block) -> Block {
        let (b, other) = chest_join(&self.world, at, b);
        if let Some((q, ob)) = other {
            self.set_block(q, ob);
        }
        b
    }

    /// A chest's halves (see `rules::chest_halves`).
    pub fn chest_halves(&self, p: IVec3) -> (IVec3, Option<IVec3>) {
        chest_halves(&self.world, p)
    }

    /// The block `b` at `p` is going away: the blocks that belong with it follow
    /// (`rules::other_cells`, without a second drop); what was in a big furnace or gun
    /// station kept by another of its blocks is returned.
    fn remove_other_half(&mut self, p: IVec3, b: Block) -> Vec<Stack> {
        let contents = match contents_elsewhere(&self.world, p, b) {
            Some(q) => self.level.block_entities.remove(q),
            None => Vec::new(),
        };
        for (q, nb) in other_cells(&self.world, p, b) {
            self.set_block(q, nb);
        }
        contents
    }

    /// A trunk cut down off grass leaves its mark on the grass under it: a circle of bare
    /// soil the grass slowly grows back over (`update_stump_marks`).
    pub fn bare_under_trunk(&mut self, p: IVec3, b: Block) {
        if !is_log(b) || is_branch(b) || log_axis(b) != 1 {
            return;
        }
        let below = p - IVec3::Y;
        let g = self.world.geti(below);
        if matches!(g, GRASS | SNOWY_GRASS) {
            self.set_block(below, stump_mark(g, 0));
            self.block_updated(below);
        }
    }

    /// The grass growing back over the marks of cut-down trunks, a stage at a time (about a
    /// minute each), where nothing covers them.
    pub(super) fn update_stump_marks(&mut self, dt: f32) {
        const EVERY: f32 = 2.0;
        const STAGE_SECS: f32 = 60.0;
        self.level.stump_scan -= dt;
        if self.level.stump_scan > 0.0 {
            return;
        }
        self.level.stump_scan = EVERY;
        let w = &self.world;
        let found: Vec<(IVec3, Block)> = self
            .stump_marks
            .values()
            .flatten()
            .map(|&p| (p, w.geti(p)))
            .filter(|&(_, b)| is_stump_mark(b))
            .collect();
        // (the ones gone are forgotten)
        self.stump_marks.retain(|_, v| {
            v.retain(|p| is_stump_mark(w.geti(*p)));
            !v.is_empty()
        });
        for (p, b) in found {
            if self.random() >= EVERY / STAGE_SECS {
                continue;
            }
            let above = self.world.geti(p + IVec3::Y);
            if is_opaque(above) || is_log(above) {
                continue;
            }
            let next = if stump_stage(b) + 1 < STUMP_STAGES { b + 1 } else { soil(b) };
            self.set_block(p, next);
            self.block_updated(p);
        }
    }

    /// Broken by the world (lost support): always drops like a hand-mined block.
    pub fn break_naturally(&mut self, p: IVec3) {
        let b = self.world.geti(p);
        let contents = self.remove_other_half(p, b);
        self.set_block(p, AIR);
        let r = self.random();
        for s in drops(b, NONE, r).into_iter().chain(contents) {
            self.spawn_drop(p.as_vec3() + Vec3::splat(0.5), s);
        }
        self.level.saplings.retain(|(q, _)| *q != p);
        self.block_updated(p);
    }

    /// Block rules after a change at `p`: support for plants and torches, and falling sand.
    pub fn block_updated(&mut self, p: IVec3) {
        let above = p + IVec3::Y;
        for q in [above, p + IVec3::X, p - IVec3::X, p + IVec3::Z, p - IVec3::Z] {
            let b = self.world.geti(q);
            if needs_support(b) && !supported(&self.world, q, b) {
                self.break_naturally(q);
            }
        }
        let w = &self.world;
        if has_gravity(w.geti(above)) && !is_solid(w.geti(p)) {
            self.start_fall(above);
        }
        let w = &self.world;
        if p.y > 0 && has_gravity(w.geti(p)) && !is_solid(w.geti(p - IVec3::Y)) {
            self.start_fall(p);
        }
    }

    fn start_fall(&mut self, p: IVec3) {
        let b = self.world.geti(p);
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
    pub fn grow_tree(&mut self, p: IVec3, sapling: Block) -> bool {
        let log = log_of_sapling(sapling);
        let seed = (self.random() * u32::MAX as f32) as u32;
        let shape = crate::world::trees::tree_shape(log, seed);
        let w = &self.world;
        let free = |b: Block| b == AIR || is_leaves(b) || is_plant(b) || is_sapling(b);
        if shape.iter().any(|&(d, _, soft)| !soft && !free(w.geti(p + d))) {
            return false;
        }
        for (d, b, soft) in shape {
            let q = p + d;
            let cur = self.world.geti(q);
            if !soft || cur == AIR || is_plant(cur) {
                self.set_block(q, b);
            }
        }
        true
    }
}
