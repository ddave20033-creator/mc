//! The server's chunks: loaded around the players (generated from the seed, or the edited
//! ones back from memory), and let go farther away. Only this far is the world simulated (the
//! players see farther: they generate the rest themselves, and get the edited chunks).

use crate::world::gen::Generator;
use crate::world::jobs::{Done, Job, Workers};
use crate::world::{ChunkPos, FastSet, World};
use std::sync::Arc;

/// Chunks loaded around each player (the simulation distance), and kept until this far.
pub(super) const RADIUS: i32 = 8;
const KEEP: i32 = 10;
/// Threads generating the server's chunks.
const THREADS: usize = 2;

pub(super) struct Loader {
    workers: Workers,
    generating: FastSet<ChunkPos>,
    /// Offsets within `RADIUS`, nearest first.
    offsets: Vec<(i32, i32)>,
}

impl Loader {
    pub fn new(gen: Arc<Generator>) -> Self {
        let r = RADIUS;
        let mut offsets: Vec<(i32, i32)> = (-r..=r)
            .flat_map(|x| (-r..=r).map(move |z| (x, z)))
            .filter(|(x, z)| x * x + z * z <= r * r)
            .collect();
        offsets.sort_by_key(|(x, z)| x * x + z * z);
        Self { workers: Workers::with_threads(gen, THREADS), generating: FastSet::default(), offsets }
    }

    /// Loads and lets go of chunks around the players' chunks `centers`; returns the chunks
    /// loaded now.
    pub fn update(&mut self, world: &mut World, centers: &[ChunkPos]) -> Vec<ChunkPos> {
        let near = |p: ChunkPos, r: i32| centers.iter().any(|c| (p.0 - c.0).pow(2) + (p.1 - c.1).pow(2) <= r * r);
        let mut loaded = Vec::new();
        while let Ok(done) = self.workers.rx.try_recv() {
            if let Done::Generated(p, data) = done {
                self.generating.remove(&p);
                if near(p, KEEP) && !world.chunks.contains_key(&p) {
                    // (an edited copy wins over the generated one)
                    let data = world.saved.remove(&p).unwrap_or_else(|| Arc::new(*data));
                    world.chunks.insert(p, data);
                    loaded.push(p);
                }
            }
        }
        // Far ones go (the edited ones are kept, to be saved and to come back).
        let far: Vec<ChunkPos> = world.chunks.keys().copied().filter(|&p| !near(p, KEEP)).collect();
        for p in far {
            if let Some(c) = world.chunks.remove(&p) {
                if world.modified.contains(&p) {
                    world.saved.insert(p, c);
                }
            }
        }
        // Near ones come, nearest first.
        let cap = self.workers.threads * 3;
        for c in centers {
            for &(dx, dz) in &self.offsets {
                let p = (c.0 + dx, c.1 + dz);
                if world.chunks.contains_key(&p) || self.generating.contains(&p) {
                    continue;
                }
                if let Some(data) = world.saved.remove(&p) {
                    world.chunks.insert(p, data);
                    loaded.push(p);
                } else if self.generating.len() < cap {
                    self.workers.submit(Job::Generate(p));
                    self.generating.insert(p);
                }
            }
        }
        loaded
    }
}
