//! Chunk meshes on the GPU: queued by the terrain, uploaded a few per frame into the shared
//! mesh buffers (`arena`), and given back when replaced or unloaded.

use super::{arena, Renderer};
use crate::engine::{Buffer, Gpu};
use crate::world::mesh::{flags, MeshData, Vertex};
use crate::world::ChunkPos;
use ash::vk;
use glam::Vec3;
use std::mem::size_of;

const MAX_UPLOADS_PER_FRAME: usize = 24;

/// A chunk mesh's vertex on the GPU: the mesher's `Vertex` packed into 20 bytes instead of 32
/// (shaders/vertex.glsl reads it back). Its position is from its chunk's corner (the corner is
/// in the mesh's `ChunkHead`): x and z in 1/2048 blocks from -8, y in 1/128 blocks from -32. uv
/// in 1/4096, signed; a fluid's is its previous y (like a position's) and how long before the
/// upload it changed (1/1024 s; all ones: long ago). The layer's bit 15 is its 0.25 (the
/// leading corners of a fluid spreading).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChunkVertex {
    pub pos: [u16; 3],
    pub layer: u16,
    pub uv: [u16; 2],
    pub light: [u8; 4],
    pub tint: [u8; 4],
}

/// At the start of each chunk mesh, read per instance: the chunk's corner and when the mesh
/// was uploaded (see `ChunkVertex`).
#[repr(C)]
#[derive(Clone, Copy)]
struct ChunkHead {
    x: i32,
    time: f32,
    z: i32,
    _pad: i32,
}

/// `ChunkVertex` and `ChunkHead` sizes: a mesh's range starts with the head; its vertices start
/// at a multiple of the vertex size after it (indirect draws count vertices from the buffer's
/// start).
const HEAD: usize = size_of::<ChunkHead>();
const VERTEX: usize = size_of::<ChunkVertex>();

fn fixed(v: f32, from: f32, steps: f32) -> u16 {
    ((v - from) * steps).round().clamp(0.0, 65535.0) as u16
}

/// Packs a vertex of the chunk whose corner is `(x0, z0)`, uploaded at `time`.
pub fn pack_vertex(v: &Vertex, x0: f32, z0: f32, time: f32) -> ChunkVertex {
    let fluid = v.tint[3] & flags::FLUID != 0;
    let uv = if fluid {
        let before = time - v.uv[1];
        let t = if (0.0..60.0).contains(&before) { (before * 1024.0).round() as u16 } else { u16::MAX };
        [fixed(v.uv[0], -32.0, 128.0), t]
    } else {
        v.uv.map(|c| ((c * 4096.0).round().clamp(-32768.0, 32767.0) as i16) as u16)
    };
    let quarter = v.layer.fract() > 0.1;
    ChunkVertex {
        pos: [fixed(v.pos[0] - x0, -8.0, 2048.0), fixed(v.pos[1], -32.0, 128.0), fixed(v.pos[2] - z0, -8.0, 2048.0)],
        layer: (v.layer as u16 & 0x7fff) | if quarter { 0x8000 } else { 0 },
        uv,
        light: v.light,
        tint: v.tint,
    }
}
/// Packs a mesh's vertices for the GPU (on the worker thread that made it: a frame taking in
/// a burst of new chunks only copies them), the fluids' change times kept relative to `time`
/// (which its `ChunkHead` then gets). Its `vertices` are given up.
pub fn pack_mesh(m: &mut MeshData, time: f32) {
    let (x0, z0) = ((m.pos.0 * 16) as f32, (m.pos.1 * 16) as f32);
    m.packed = m.vertices.iter().map(|v| pack_vertex(v, x0, z0, time)).collect();
    m.packed_time = time;
    m.vertices = Vec::new();
}

/// Gives a mesh's memory back on another thread: freeing a chunk's few megabytes takes the
/// system a millisecond or more, a stutter when a burst of new chunks comes in.
fn drop_later(m: MeshData) {
    use std::sync::{mpsc, OnceLock};
    static BIN: OnceLock<mpsc::Sender<MeshData>> = OnceLock::new();
    let bin = BIN.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<MeshData>();
        // (without the thread the channel is closed, and the mesh is freed here after all)
        let _ = std::thread::Builder::new().name("mesh-free".into()).spawn(move || rx.into_iter().for_each(drop));
        tx
    });
    let _ = bin.send(m);
}

/// Per-frame staging memory for chunk uploads (reused instead of allocating per chunk).
pub(super) const STAGING_SIZE: usize = 16 << 20;

pub(super) struct ChunkGpu {
    /// The head (`ChunkHead`), vertices (from `vertex_offset` on), then (from `index_offset`
    /// on) indices; None for an empty mesh.
    pub mesh: Option<arena::Range>,
    pub vertex_offset: u64,
    pub index_offset: u64,
    pub opaque: u32,
    /// Opaque indices without the faces between leaves and the plants, and the former's count
    /// (see `MeshData`).
    pub solid: u32,
    /// Whole-block faces by direction: the plain ones at the start of the solid indices, the
    /// cut-out ones at their end (see `MeshData`).
    pub dirs: [u32; 6],
    pub cut_dirs: [u32; 6],
    pub leaf_inner: u32,
    pub water: u32,
    pub min: Vec3,
    pub max: Vec3,
}

impl Renderer {
    pub fn queue_mesh(&mut self, mesh: MeshData) {
        self.pending.push_back(mesh);
    }

    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// The chunk meshes' video memory: (pages, bytes, bytes used, ranges waiting to be freed).
    pub fn mesh_memory(&self) -> (usize, u64, u64, usize) {
        self.arena.stats()
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Drops every chunk mesh (when switching worlds).
    pub fn clear_chunks(&mut self) {
        self.pending.clear();
        for (_, c) in self.chunks.drain() {
            if let Some(r) = c.mesh {
                self.arena.retire(r, self.frame);
            }
        }
    }

    pub fn remove_chunk(&mut self, pos: ChunkPos) {
        self.pending.retain(|m| m.pos != pos);
        if let Some(r) = self.chunks.remove(&pos).and_then(|c| c.mesh) {
            self.arena.retire(r, self.frame);
        }
    }

    /// Uploads a few waiting meshes; `time` is the frame's (the shaders' `camPos.w`).
    pub(super) unsafe fn flush_uploads(&mut self, gpu: &mut Gpu, cmd: vk::CommandBuffer, time: f32) {
        let mut any = false;
        // This frame slot's staging buffer is free: begin_frame waited for its fence.
        let ring = &self.staging[gpu.frame_slot];
        let mut ring_used = 0usize;
        self.uploaded = 0;
        for _ in 0..MAX_UPLOADS_PER_FRAME {
            let Some(mut m) = self.pending.pop_front() else {
                break;
            };
            if m.packed.is_empty() && !m.vertices.is_empty() {
                pack_mesh(&mut m, time);
            }
            // Room for the head, the vertices wherever they line up, and the indices (at
            // least what `total` below comes to). Whether it goes through the staging buffer is
            // decided by this, not by `total`: a mesh whose `most` does not fit is never
            // checked against the room left, so it must not use the staging buffer either.
            let vbytes = m.packed.len() * VERTEX;
            let most = HEAD + VERTEX + vbytes + 16 + std::mem::size_of_val(m.indices.as_slice());
            let staged = most <= STAGING_SIZE;
            if staged && ring_used + most > STAGING_SIZE {
                // Out of staging space this frame; upload it next frame.
                self.pending.push_front(m);
                break;
            }
            let (x0, z0) = ((m.pos.0 * 16) as f32, (m.pos.1 * 16) as f32);
            let mut chunk = ChunkGpu {
                mesh: None,
                vertex_offset: 0,
                index_offset: 0,
                opaque: m.opaque_count,
                solid: m.solid_count,
                dirs: m.dir_counts,
                cut_dirs: m.cut_dir_counts,
                leaf_inner: m.leaf_inner_count,
                water: m.indices.len() as u32 - m.opaque_count,
                min: Vec3::new(x0 - 1.0, m.min_y - 1.0, z0 - 1.0),
                max: Vec3::new(x0 + 17.0, m.max_y + 1.0, z0 + 17.0),
            };
            if !m.packed.is_empty() {
                let Some(range) = self.arena.alloc(gpu, most as u64) else {
                    // The video memory is full: it waits (till far chunks go and free some).
                    self.pending.push_front(m);
                    break;
                };
                let voff = HEAD + (VERTEX - (range.offset as usize + HEAD) % VERTEX) % VERTEX;
                let ioff = (voff + vbytes + 15) & !15;
                let total = ioff + std::mem::size_of_val(m.indices.as_slice());
                let head = ChunkHead { x: m.pos.0 * 16, time: m.packed_time, z: m.pos.1 * 16, _pad: 0 };
                let packed = m.packed.as_slice();
                debug_assert!(total <= most);
                let (src, src_offset) = if staged {
                    ring.write(ring_used, &[head]);
                    ring.write(ring_used + voff, packed);
                    ring.write(ring_used + ioff, m.indices.as_slice());
                    let at = ring_used;
                    ring_used = (ring_used + total + 15) & !15;
                    (ring.handle, at)
                } else {
                    // Larger than the whole staging buffer: use a one-off buffer.
                    let staging = Buffer::new(
                        gpu,
                        total as u64,
                        vk::BufferUsageFlags::TRANSFER_SRC,
                        vk::MemoryPropertyFlags::HOST_VISIBLE
                            | vk::MemoryPropertyFlags::HOST_COHERENT,
                    );
                    staging.write(0, &[head]);
                    staging.write(voff, packed);
                    staging.write(ioff, m.indices.as_slice());
                    let handle = staging.handle;
                    gpu.defer_destroy(staging);
                    (handle, 0)
                };
                gpu.device.cmd_copy_buffer(
                    cmd,
                    src,
                    self.arena.buffer(range),
                    &[vk::BufferCopy {
                        src_offset: src_offset as u64,
                        dst_offset: range.offset,
                        size: total as u64,
                    }],
                );
                chunk.mesh = Some(range);
                chunk.vertex_offset = voff as u64;
                chunk.index_offset = ioff as u64;
                any = true;
            }
            self.uploaded += 1;
            if let Some(old) = self.chunks.insert(m.pos, chunk).and_then(|c| c.mesh) {
                self.arena.retire(old, self.frame);
            }
            drop_later(m);
        }
        if any {
            let barrier = vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(
                    vk::AccessFlags::VERTEX_ATTRIBUTE_READ | vk::AccessFlags::INDEX_READ,
                );
            gpu.device.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::VERTEX_INPUT,
                vk::DependencyFlags::empty(),
                &[barrier],
                &[],
                &[],
            );
        }
    }
}
