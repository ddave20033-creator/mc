//! Streams chunks around a center point: generation, (re)meshing and unloading.

use super::gen::Generator;
use super::jobs::{Done, Job, Workers};
use super::mesh::MeshData;
use super::*;
use std::sync::Arc;

pub struct Terrain {
    pub world: World,
    pub gen: Arc<Generator>,
    workers: Workers,
    in_flight: usize,
    generating: FastSet<ChunkPos>,
    meshing: FastSet<ChunkPos>,
    dirty: FastSet<ChunkPos>,
    meshed: FastSet<ChunkPos>,
    offsets: Vec<(i32, i32)>,
    /// Door halves of each meshed chunk (see `MeshData::doors`).
    pub doors: FastMap<ChunkPos, Vec<IVec3>>,
    /// Chests of each meshed chunk (see `MeshData::chests`).
    pub chests: FastMap<ChunkPos, Vec<IVec3>>,
    /// Gun stations of each meshed chunk (see `MeshData::gun_stations`).
    pub gun_stations: FastMap<ChunkPos, Vec<IVec3>>,
    /// LAN host: where the other players are. Chunks around them stay loaded (without
    /// meshes) so the world keeps running there.
    pub extra_centers: Vec<ChunkPos>,
}

/// Chunks kept loaded around each other LAN player (on the host).
const EXTRA_RADIUS: i32 = 6;

pub enum TerrainEvent {
    Mesh(MeshData),
    Unload(ChunkPos),
    /// An edited chunk came back from memory/disk (its fluids may still need to flow).
    Restored(ChunkPos),
}

impl Terrain {
    pub fn new(seed: u32) -> Self {
        let gen = Arc::new(Generator::new(seed));
        let workers = Workers::new(gen.clone());
        // Up to the largest render distance (64) and the unload margin around it.
        let r = 68i32;
        let mut offsets: Vec<(i32, i32)> = (-r..=r)
            .flat_map(|x| (-r..=r).map(move |z| (x, z)))
            .filter(|(x, z)| x * x + z * z <= r * r)
            .collect();
        offsets.sort_by_key(|(x, z)| x * x + z * z);
        Self {
            world: World::new(),
            gen,
            workers,
            in_flight: 0,
            generating: FastSet::default(),
            meshing: FastSet::default(),
            dirty: FastSet::default(),
            meshed: FastSet::default(),
            offsets,
            doors: FastMap::default(),
            chests: FastMap::default(),
            gun_stations: FastMap::default(),
            extra_centers: Vec::new(),
        }
    }

    fn mark_dirty(&mut self, p: ChunkPos) {
        self.dirty.insert(p);
    }

    /// Re-mesh the chunk containing `b`. `wide` also re-meshes all 8 neighbours
    /// (light can spread across borders); otherwise only neighbours touching `b`.
    pub fn block_changed(&mut self, b: IVec3, wide: bool) {
        let (cx, cz) = World::chunk_pos(b.x, b.z);
        let (lx, lz) = (b.x.rem_euclid(16), b.z.rem_euclid(16));
        for dz in -1..=1 {
            for dx in -1..=1 {
                let touches = (dx == 0 || (dx == -1 && lx == 0) || (dx == 1 && lx == 15))
                    && (dz == 0 || (dz == -1 && lz == 0) || (dz == 1 && lz == 15));
                if (wide || touches) && self.world.chunks.contains_key(&(cx + dx, cz + dz)) {
                    self.mark_dirty((cx + dx, cz + dz));
                }
            }
        }
    }

    pub fn is_meshed(&self, p: ChunkPos) -> bool {
        self.meshed.contains(&p)
    }

    /// Are all chunks within `r` of `center` generated and meshed?
    pub fn ready_around(&self, center: ChunkPos, r: i32) -> bool {
        self.offsets
            .iter()
            .take_while(|(x, z)| x * x + z * z <= r * r)
            .all(|(x, z)| self.meshed.contains(&(center.0 + x, center.1 + z)))
    }

    pub fn update(&mut self, center: ChunkPos, radius: i32, out: &mut Vec<TerrainEvent>) {
        let dist2 = |p: ChunkPos| (p.0 - center.0).pow(2) + (p.1 - center.1).pow(2);
        let extra = std::mem::take(&mut self.extra_centers);
        let near_extra = |p: ChunkPos, r: i32| {
            extra
                .iter()
                .any(|c| (p.0 - c.0).pow(2) + (p.1 - c.1).pow(2) <= r * r)
        };

        while let Ok(done) = self.workers.rx.try_recv() {
            self.in_flight -= 1;
            match done {
                Done::Generated(p, data) => {
                    self.generating.remove(&p);
                    let wanted = dist2(p) <= (radius + 3).pow(2) || near_extra(p, EXTRA_RADIUS + 2);
                    if wanted && !self.world.chunks.contains_key(&p) {
                        // An edited copy (LAN: sent by the host meanwhile) wins over the
                        // generated one.
                        let data = self
                            .world
                            .saved
                            .remove(&p)
                            .unwrap_or_else(|| Arc::new(*data));
                        self.world.chunks.insert(p, data);
                        self.world.apply_pending(p);
                        for dz in -1..=1 {
                            for dx in -1..=1 {
                                let q = (p.0 + dx, p.1 + dz);
                                if self.world.chunks.contains_key(&q) {
                                    self.mark_dirty(q);
                                }
                            }
                        }
                    }
                }
                Done::Meshed(mut m) => {
                    self.meshing.remove(&m.pos);
                    if self.world.chunks.contains_key(&m.pos) {
                        self.meshed.insert(m.pos);
                        self.world.light.insert(m.pos, std::mem::take(&mut m.light));
                        if m.doors.is_empty() {
                            self.doors.remove(&m.pos);
                        } else {
                            self.doors.insert(m.pos, m.doors.clone());
                        }
                        if m.chests.is_empty() {
                            self.chests.remove(&m.pos);
                        } else {
                            self.chests.insert(m.pos, m.chests.clone());
                        }
                        if m.gun_stations.is_empty() {
                            self.gun_stations.remove(&m.pos);
                        } else {
                            self.gun_stations.insert(m.pos, m.gun_stations.clone());
                        }
                        out.push(TerrainEvent::Mesh(m));
                    }
                }
            }
        }

        // Unload far chunks (edited ones are kept in memory).
        let unload = (radius + 3).pow(2);
        let far: Vec<ChunkPos> = self
            .world
            .chunks
            .keys()
            .copied()
            .filter(|&p| dist2(p) > unload && !near_extra(p, EXTRA_RADIUS + 2))
            .collect();
        for p in far {
            if let Some(c) = self.world.chunks.remove(&p) {
                if self.world.modified.contains(&p) {
                    self.world.saved.insert(p, c);
                }
            }
            self.meshed.remove(&p);
            self.doors.remove(&p);
            self.chests.remove(&p);
            self.gun_stations.remove(&p);
            self.world.light.remove(&p);
            self.dirty.remove(&p);
            out.push(TerrainEvent::Unload(p));
        }

        // Schedule generation and meshing, nearest first.
        let cap = self.workers.threads * 3;
        for i in 0..self.offsets.len() {
            let (dx, dz) = self.offsets[i];
            let r2 = dx * dx + dz * dz;
            if r2 > (radius + 1).pow(2) {
                break;
            }
            let p = (center.0 + dx, center.1 + dz);
            if !self.world.chunks.contains_key(&p) && !self.generating.contains(&p) {
                if let Some(c) = self.world.saved.remove(&p) {
                    self.world.chunks.insert(p, c);
                    out.push(TerrainEvent::Restored(p));
                    self.world.apply_pending(p);
                    for (qx, qz) in [
                        (0, 0),
                        (1, 0),
                        (-1, 0),
                        (0, 1),
                        (0, -1),
                        (1, 1),
                        (-1, -1),
                        (1, -1),
                        (-1, 1),
                    ] {
                        let q = (p.0 + qx, p.1 + qz);
                        if self.world.chunks.contains_key(&q) {
                            self.mark_dirty(q);
                        }
                    }
                } else if self.in_flight < cap {
                    self.workers.submit(Job::Generate(p));
                    self.generating.insert(p);
                    self.in_flight += 1;
                }
            }
            let urgent = r2 <= 8;
            if r2 <= radius * radius
                && self.dirty.contains(&p)
                && !self.meshing.contains(&p)
                && (self.in_flight < cap || urgent)
            {
                if let Some(nb) = self.neighborhood(p) {
                    self.dirty.remove(&p);
                    let anim = self.world.fluid_changes_near(p);
                    self.workers.submit(Job::Mesh { pos: p, nb, anim });
                    self.meshing.insert(p);
                    self.in_flight += 1;
                }
            }
        }

        // Around the other LAN players: generate or restore chunks (no meshes needed).
        for c in &extra {
            for &(dx, dz) in &self.offsets {
                if dx * dx + dz * dz > EXTRA_RADIUS * EXTRA_RADIUS {
                    break;
                }
                let p = (c.0 + dx, c.1 + dz);
                if self.world.chunks.contains_key(&p) || self.generating.contains(&p) {
                    continue;
                }
                if let Some(data) = self.world.saved.remove(&p) {
                    self.world.chunks.insert(p, data);
                    out.push(TerrainEvent::Restored(p));
                } else if self.in_flight < cap {
                    self.workers.submit(Job::Generate(p));
                    self.generating.insert(p);
                    self.in_flight += 1;
                }
            }
        }
        self.extra_centers = extra;
    }

    pub fn neighborhood(&self, p: ChunkPos) -> Option<Box<[Arc<ChunkData>; 9]>> {
        let mut v = Vec::with_capacity(9);
        for dz in -1..=1 {
            for dx in -1..=1 {
                v.push(self.world.chunks.get(&(p.0 + dx, p.1 + dz))?.clone());
            }
        }
        let arr: [Arc<ChunkData>; 9] = v.try_into().ok()?;
        Some(Box::new(arr))
    }
}
