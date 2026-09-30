//! Chunk meshes on the GPU: queued by the terrain, uploaded a few per frame into the shared
//! mesh buffers (`arena`), and given back when replaced or unloaded.

use super::{arena, Renderer};
use crate::engine::{Buffer, Gpu};
use crate::world::mesh::MeshData;
use crate::world::ChunkPos;
use ash::vk;
use glam::Vec3;

const MAX_UPLOADS_PER_FRAME: usize = 24;
/// Per-frame staging memory for chunk uploads (reused instead of allocating per chunk).
pub(super) const STAGING_SIZE: usize = 16 << 20;

pub(super) struct ChunkGpu {
    /// Vertices, then (from `index_offset` on) indices; None for an empty mesh.
    pub mesh: Option<arena::Range>,
    pub index_offset: u64,
    pub opaque: u32,
    /// Opaque indices without the faces between leaves and the plants, and the former's count
    /// (see `MeshData`).
    pub solid: u32,
    /// Whole-block faces by direction, at the end of the solid indices.
    pub dirs: [u32; 6],
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

    pub(super) unsafe fn flush_uploads(&mut self, gpu: &mut Gpu, cmd: vk::CommandBuffer) {
        let mut any = false;
        // This frame slot's staging buffer is free: begin_frame waited for its fence.
        let ring = &self.staging[gpu.frame_slot];
        let mut ring_used = 0usize;
        for _ in 0..MAX_UPLOADS_PER_FRAME {
            let Some(m) = self.pending.pop_front() else {
                break;
            };
            let vbytes = std::mem::size_of_val(m.vertices.as_slice());
            let ioff = (vbytes + 15) & !15;
            let total = ioff + std::mem::size_of_val(m.indices.as_slice());
            if total <= STAGING_SIZE && ring_used + total > STAGING_SIZE {
                // Out of staging space this frame; upload it next frame.
                self.pending.push_front(m);
                break;
            }
            let (x0, z0) = ((m.pos.0 * 16) as f32, (m.pos.1 * 16) as f32);
            let mut chunk = ChunkGpu {
                mesh: None,
                index_offset: 0,
                opaque: m.opaque_count,
                solid: m.solid_count,
                dirs: m.dir_counts,
                leaf_inner: m.leaf_inner_count,
                water: m.indices.len() as u32 - m.opaque_count,
                min: Vec3::new(x0 - 1.0, m.min_y - 1.0, z0 - 1.0),
                max: Vec3::new(x0 + 17.0, m.max_y + 1.0, z0 + 17.0),
            };
            if !m.vertices.is_empty() {
                let range = self.arena.alloc(gpu, total as u64);
                let (src, src_offset) = if total <= STAGING_SIZE {
                    ring.write(ring_used, m.vertices.as_slice());
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
                    staging.write(0, m.vertices.as_slice());
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
                chunk.index_offset = ioff as u64;
                any = true;
            }
            if let Some(old) = self.chunks.insert(m.pos, chunk).and_then(|c| c.mesh) {
                self.arena.retire(old, self.frame);
            }
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
