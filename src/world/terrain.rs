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
    /// Chunks being meshed, with the ticket of their job (`Job::Mesh`): a mesh coming back for
    /// any other (a job from before the chunk was unloaded) is out of date and dropped.
    meshing: FastMap<ChunkPos, u64>,
    tickets: u64,
    /// How many times each chunk's jobs have failed (`Done::Failed`), till it is given up.
    failed: FastMap<ChunkPos, u8>,
    dirty: FastSet<ChunkPos>,
    meshed: FastSet<ChunkPos>,
    offsets: Vec<(i32, i32)>,
    /// Door halves of each meshed chunk (see `MeshData::doors`).
    pub doors: FastMap<ChunkPos, Vec<IVec3>>,
    /// Chests of each meshed chunk (see `MeshData::chests`).
    pub chests: FastMap<ChunkPos, Vec<IVec3>>,
    /// Gun stations of each meshed chunk (see `MeshData::gun_stations`).
    pub gun_stations: FastMap<ChunkPos, Vec<IVec3>>,
    /// Torches and stump marks of each meshed chunk (see `MeshData::torches`).
    pub torches: FastMap<ChunkPos, Vec<IVec3>>,
    pub stump_marks: FastMap<ChunkPos, Vec<IVec3>>,
    /// LAN host: where the other players are. Chunks around them stay loaded (without
    /// meshes) so the world keeps running there.
    pub extra_centers: Vec<ChunkPos>,
    /// The center and radius everything round was generated and meshed for, nothing left to
    /// do: until either changes or a chunk needs meshing again, the walk round is skipped.
    settled: Option<(ChunkPos, i32)>,
}

/// The positions in a per-chunk list (`Terrain::torches`...) within `reach` blocks across and
/// `up` up and down of `c`.
pub fn listed_near(map: &FastMap<ChunkPos, Vec<IVec3>>, c: IVec3, reach: i32, up: i32) -> impl Iterator<Item = IVec3> + '_ {
    let (x0, x1) = ((c.x - reach).div_euclid(16), (c.x + reach).div_euclid(16));
    let (z0, z1) = ((c.z - reach).div_euclid(16), (c.z + reach).div_euclid(16));
    (x0..=x1)
        .flat_map(move |cx| (z0..=z1).map(move |cz| (cx, cz)))
        .filter_map(|p| map.get(&p))
        .flatten()
        .copied()
        .filter(move |p| (p.x - c.x).abs() <= reach && (p.z - c.z).abs() <= reach && (p.y - c.y).abs() <= up)
}

/// Chunks kept loaded around each other LAN player (on the host).
const EXTRA_RADIUS: i32 = 6;
/// Times a chunk's job may fail (a panic in the generator or the mesher) before the chunk is
/// given up: left empty, or without a new mesh.
const TRIES: u8 = 3;

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
            meshing: FastMap::default(),
            tickets: 0,
            failed: FastMap::default(),
            dirty: FastSet::default(),
            meshed: FastSet::default(),
            offsets,
            doors: FastMap::default(),
            chests: FastMap::default(),
            gun_stations: FastMap::default(),
            torches: FastMap::default(),
            stump_marks: FastMap::default(),
            settled: None,
            extra_centers: Vec::new(),
        }
    }

    fn mark_dirty(&mut self, p: ChunkPos) {
        self.dirty.insert(p);
        self.settled = None;
    }

    /// Puts chunk `p` into the world (generated, or `restored`: an edited copy back from
    /// memory, whose fluids may still need to flow). Every way a chunk comes in goes through
    /// here: the changes waiting for it are applied, and it and the chunks round it are meshed
    /// again (its blocks change their borders and their light).
    fn insert_chunk(&mut self, p: ChunkPos, data: Arc<ChunkData>, restored: bool, out: &mut Vec<TerrainEvent>) {
        self.world.chunks.insert(p, data);
        self.world.apply_pending(p);
        if restored {
            out.push(TerrainEvent::Restored(p));
        }
        for dz in -1..=1 {
            for dx in -1..=1 {
                let q = (p.0 + dx, p.1 + dz);
                if self.world.chunks.contains_key(&q) {
                    self.mark_dirty(q);
                }
            }
        }
    }

    /// Counts a failed job for chunk `p`; true when it has failed too often.
    fn give_up(&mut self, p: ChunkPos) -> bool {
        let tries = self.failed.entry(p).or_default();
        *tries += 1;
        *tries >= TRIES
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
                    self.failed.remove(&p);
                    let wanted = dist2(p) <= (radius + 3).pow(2) || near_extra(p, EXTRA_RADIUS + 2);
                    if wanted && !self.world.chunks.contains_key(&p) {
                        // An edited copy (LAN: sent by the host meanwhile) wins over the
                        // generated one.
                        match self.world.saved.remove(&p) {
                            Some(c) => self.insert_chunk(p, c, true, out),
                            None => self.insert_chunk(p, Arc::new(*data), false, out),
                        }
                    }
                }
                Done::Failed(p, None) => {
                    self.generating.remove(&p);
                    self.settled = None;
                    let wanted = dist2(p) <= (radius + 3).pow(2) || near_extra(p, EXTRA_RADIUS + 2);
                    if self.give_up(p) && wanted && !self.world.chunks.contains_key(&p) {
                        // (before that it is generated again: the walk round asks for it)
                        eprintln!("chunk {p:?} could not be generated: left empty");
                        self.insert_chunk(p, Arc::new(ChunkData::new()), false, out);
                    }
                }
                Done::Failed(p, Some(ticket)) => {
                    if self.meshing.get(&p) == Some(&ticket) {
                        self.meshing.remove(&p);
                        if self.give_up(p) {
                            // (shown with its old mesh, or none, but not waited for)
                            eprintln!("chunk {p:?} could not be meshed");
                            self.meshed.insert(p);
                        } else {
                            self.mark_dirty(p);
                        }
                    }
                }
                Done::Meshed(mut m, ticket) => {
                    // (a mesh from before the chunk was unloaded is out of date: the chunk's
                    // new job, if it has one, brings the right one)
                    if self.meshing.get(&m.pos) != Some(&ticket) {
                        continue;
                    }
                    self.meshing.remove(&m.pos);
                    self.failed.remove(&m.pos);
                    if self.world.chunks.contains_key(&m.pos) {
                        self.meshed.insert(m.pos);
                        self.world.light.insert(m.pos, std::mem::take(&mut m.light));
                        let pos = m.pos;
                        for (map, list) in [
                            (&mut self.doors, &m.doors),
                            (&mut self.chests, &m.chests),
                            (&mut self.gun_stations, &m.gun_stations),
                            (&mut self.torches, &m.torches),
                            (&mut self.stump_marks, &m.stump_marks),
                        ] {
                            if list.is_empty() {
                                map.remove(&pos);
                            } else {
                                map.insert(pos, list.clone());
                            }
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
            self.torches.remove(&p);
            self.stump_marks.remove(&p);
            self.world.light.remove(&p);
            self.dirty.remove(&p);
            self.meshing.remove(&p);
            out.push(TerrainEvent::Unload(p));
        }

        // Schedule generation and meshing, nearest first.
        let cap = self.workers.threads * 3;
        let walk = self.settled != Some((center, radius));
        let mut unfinished = false;
        for i in 0..if walk { self.offsets.len() } else { 0 } {
            let (dx, dz) = self.offsets[i];
            let r2 = dx * dx + dz * dz;
            if r2 > (radius + 1).pow(2) {
                break;
            }
            let p = (center.0 + dx, center.1 + dz);
            if !self.world.chunks.contains_key(&p) {
                unfinished = true;
            }
            if !self.world.chunks.contains_key(&p) && !self.generating.contains(&p) {
                if let Some(c) = self.world.saved.remove(&p) {
                    self.insert_chunk(p, c, true, out);
                } else if self.in_flight < cap {
                    self.workers.submit(Job::Generate(p));
                    self.generating.insert(p);
                    self.in_flight += 1;
                }
            }
            let urgent = r2 <= 8;
            if r2 <= radius * radius
                && self.dirty.contains(&p)
                && !self.meshing.contains_key(&p)
                && (self.in_flight < cap || urgent)
            {
                if let Some(nb) = self.neighborhood(p) {
                    self.dirty.remove(&p);
                    let anim = self.world.fluid_changes_near(p);
                    let notches = self.world.notches_near(p);
                    self.tickets += 1;
                    let ticket = self.tickets;
                    self.workers.submit(Job::Mesh { pos: p, ticket, nb, anim, notches });
                    self.meshing.insert(p, ticket);
                    self.in_flight += 1;
                }
            }
            if r2 <= radius * radius && (self.dirty.contains(&p) || self.meshing.contains_key(&p)) {
                unfinished = true;
            }
        }
        if walk && !unfinished {
            self.settled = Some((center, radius));
        }

        // Around the other LAN players: generate or restore chunks (no meshes needed).
        for c in &extra {
            for i in 0..self.offsets.len() {
                let (dx, dz) = self.offsets[i];
                if dx * dx + dz * dz > EXTRA_RADIUS * EXTRA_RADIUS {
                    break;
                }
                let p = (c.0 + dx, c.1 + dz);
                if self.world.chunks.contains_key(&p) || self.generating.contains(&p) {
                    continue;
                }
                if let Some(data) = self.world.saved.remove(&p) {
                    self.insert_chunk(p, data, true, out);
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
