//! This game's side of the world's things (the server runs them; here they are shown at
//! once, and followed as the server sends them): what is known of the world played (`Level`),
//! blocks placed and broken, mobs, furnaces, felling trees and the trunks left lying.

pub(super) mod blocks;
pub(super) mod mobs;
pub(super) mod furnace;
pub(super) mod felling;
pub(super) mod logs;

use crate::entity::{BlockEntities, FallingBlock, ItemEntity};
use crate::entity::mob::Mob;
use crate::model::particles::Particles;
use crate::world::save::WorldMeta;
use crate::world::FastMap;
use glam::IVec3;

/// What is known of the world being played besides its blocks (`Game::terrain`), as the
/// server sent it: its name and spawn, the time of day, dropped items, falling blocks and
/// trees, lying trunks, mobs, block entities; and the animations of things in it. Made anew
/// for every world joined, so nothing of the last one comes along.
pub(super) struct Level {
    /// The world played (None out of a world).
    pub(super) meta: Option<WorldMeta>,
    /// The world's spawn column.
    pub(super) spawn: (i32, i32),
    /// The time of day (0 sunrise .. 0.25 noon .. 0.75 midnight), as the server keeps it.
    pub(super) time_of_day: f32,
    pub(super) items: Vec<ItemEntity>,
    pub(super) falling: Vec<FallingBlock>,
    /// Trees felled with an axe, falling over.
    falling_trees: Vec<crate::sim::felling::FallingTree>,
    /// The trunks of felled trees lying on the ground.
    pub(super) lying_logs: Vec<crate::sim::felling::LyingLog>,
    pub(super) mobs: Vec<Mob>,
    pub(super) block_entities: BlockEntities,
    /// Chest lid animation 0..1 per chest position.
    pub(super) chest_open: FastMap<IVec3, f32>,
    /// How far each door half is swung open (0..1), easing toward its state.
    pub(super) door_swing: FastMap<IVec3, f32>,
    /// How far each gun station's drawer is out (0..1): it slides out while one is used.
    pub(super) bench_drawer: FastMap<IVec3, f32>,
    /// How much each furnace near by had made when last heard (it dings when that grows).
    furnace_heard: std::collections::HashMap<IVec3, u32>,
    /// The last change seen on each gun station's table and when it started (it plays out).
    pub(super) bench_anims: FastMap<IVec3, (u16, f32)>,
    /// Torches near the player (rescanned every second) and the rescan timer.
    pub(super) torches: Vec<IVec3>,
    pub(super) torch_scan: f32,
    /// Smoke, sparks, crumbs, splashes...
    pub(super) particles: Particles,
}

impl Level {
    pub(super) fn new() -> Self {
        Self {
            meta: None,
            spawn: (0, 0),
            time_of_day: crate::client::MENU_TIME_OF_DAY,
            items: Vec::new(),
            falling: Vec::new(),
            falling_trees: Vec::new(),
            lying_logs: Vec::new(),
            mobs: Vec::new(),
            block_entities: BlockEntities::default(),
            chest_open: Default::default(),
            door_swing: Default::default(),
            bench_drawer: Default::default(),
            furnace_heard: Default::default(),
            bench_anims: Default::default(),
            torches: Vec::new(),
            torch_scan: 0.0,
            particles: Particles::new(),
        }
    }

    /// The chests open (`open`) swing their lids up, the others fall shut.
    pub(super) fn swing_chest_lids(&mut self, open: &[IVec3], dt: f32) {
        for p in open {
            self.chest_open.entry(*p).or_insert(0.0);
        }
        self.chest_open.retain(|p, k| {
            let target = if open.contains(p) { 1.0 } else { 0.0 };
            *k = if target > *k {
                (*k + dt * 3.5).min(1.0)
            } else {
                (*k - dt * 3.0).max(0.0)
            };
            *k > 0.0 || target > 0.0
        });
    }
}
