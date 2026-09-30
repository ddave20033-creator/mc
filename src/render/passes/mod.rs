//! Command recording of the frame's passes (shadow, scope, main, UI), and what they share:
//! the recording context and the chunks' indirect draws.

mod main;
mod scope;
mod shadow;
mod ui;

use super::dynamic::DynRanges;
use crate::engine::{Buffer, Gpu};
use super::chunks::ChunkVertex;
use ash::vk;
use std::mem::size_of;

/// Indirect draw commands per frame (chunks' opaque parts, a few each).
pub(super) const MAX_INDIRECT: usize = 65536;

/// Push constants go to both shader stages.
const STAGES: vk::ShaderStageFlags = vk::ShaderStageFlags::from_raw(
    vk::ShaderStageFlags::VERTEX.as_raw() | vk::ShaderStageFlags::FRAGMENT.as_raw(),
);

fn as_bytes<T: Copy>(v: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v as *const T as *const u8, size_of::<T>()) }
}

/// What every pass of one frame records with.
pub(super) struct Rec<'a> {
    pub gpu: &'a Gpu,
    pub d: &'a ash::Device,
    pub cmd: vk::CommandBuffer,
    /// The frame slot (which of the per-slot buffers and sets are this frame's).
    pub slot: usize,
    /// This frame's dynamic vertex buffer, and where its ranges are.
    pub dyn_buf: vk::Buffer,
    pub dyn_ranges: DynRanges,
}

impl Rec<'_> {
    /// Dynamic range `i` (`dynamic::PARTICLES`...): first vertex and count.
    pub fn range(&self, i: usize) -> (u32, u32) {
        self.dyn_ranges.range(i)
    }

    pub unsafe fn bind_pipe(&self, pipe: vk::Pipeline) {
        self.d.cmd_bind_pipeline(self.cmd, vk::PipelineBindPoint::GRAPHICS, pipe);
    }

    pub unsafe fn bind_sets(&self, layout: vk::PipelineLayout, sets: &[vk::DescriptorSet]) {
        self.d
            .cmd_bind_descriptor_sets(self.cmd, vk::PipelineBindPoint::GRAPHICS, layout, 0, sets, &[]);
    }

    pub unsafe fn push<T: Copy>(&self, layout: vk::PipelineLayout, v: &T) {
        self.d.cmd_push_constants(self.cmd, layout, STAGES, 0, as_bytes(v));
    }

    /// Viewport and scissor both `rect`.
    pub unsafe fn set_view(&self, rect: vk::Rect2D) {
        self.d.cmd_set_viewport(
            self.cmd,
            0,
            &[vk::Viewport {
                x: rect.offset.x as f32,
                y: rect.offset.y as f32,
                width: rect.extent.width as f32,
                height: rect.extent.height as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            }],
        );
        self.d.cmd_set_scissor(self.cmd, 0, &[rect]);
    }

    pub unsafe fn bind_dyn(&self) {
        self.d.cmd_bind_vertex_buffers(self.cmd, 0, &[self.dyn_buf], &[0]);
    }

    /// Binds the dynamic buffer and draws its range `i`, if it has anything.
    pub unsafe fn draw_dyn(&self, i: usize) {
        let (first, n) = self.range(i);
        if n > 0 {
            self.bind_dyn();
            self.d.cmd_draw(self.cmd, n, 1, first, 0);
        }
    }
}

type IndirectDraw = (vk::Buffer, vk::DrawIndexedIndirectCommand);

/// Where a chunk's mesh is: its buffer, its vertices and indices in it (bytes), and its head
/// as an instance (`ChunkVertex`: the head is read per instance, from the buffer's start).
#[derive(Clone, Copy)]
pub(super) struct ChunkMesh {
    pub buffer: vk::Buffer,
    pub vertices: u64,
    pub indices: u64,
    pub instance: u32,
}

impl ChunkMesh {
    pub fn new(buffer: vk::Buffer, range: u64, vertex_offset: u64, index_offset: u64) -> Self {
        Self { buffer, vertices: range + vertex_offset, indices: range + index_offset, instance: (range / 16) as u32 }
    }

    /// Binds its buffers for `draw`.
    pub unsafe fn bind(&self, d: &ash::Device, cmd: vk::CommandBuffer) {
        d.cmd_bind_vertex_buffers(cmd, 0, &[self.buffer, self.buffer], &[self.vertices, 0]);
        d.cmd_bind_index_buffer(cmd, self.buffer, self.indices, vk::IndexType::UINT32);
    }

    /// Draws `count` indices from `first` (bound).
    pub unsafe fn draw(&self, d: &ash::Device, cmd: vk::CommandBuffer, first: u32, count: u32) {
        d.cmd_draw_indexed(cmd, count, 1, first, 0, self.instance);
    }
}

/// One indirect draw of `count` indices from `first` of a chunk's mesh.
fn chunk_draw(m: &ChunkMesh, first: u32, count: u32) -> IndirectDraw {
    (
        m.buffer,
        vk::DrawIndexedIndirectCommand {
            index_count: count,
            instance_count: 1,
            first_index: (m.indices / 4) as u32 + first,
            vertex_offset: (m.vertices / size_of::<ChunkVertex>() as u64) as i32,
            first_instance: m.instance,
        },
    )
}

/// Records `draws` as indirect draws from `ind`, from command `base` on: one command per mesh
/// buffer (the meshes share a few big ones), in order within each. Returns the next free
/// command, or None (nothing recorded) when they do not fit.
unsafe fn record_indirect(
    d: &ash::Device,
    cmd: vk::CommandBuffer,
    ind: &Buffer,
    base: usize,
    draws: &mut [IndirectDraw],
    gpu: &Gpu,
) -> Option<usize> {
    // (the chunks' heads are instances: without indirect draws starting at an instance, they
    // are drawn one by one)
    let multi = gpu.multi_draw_indirect;
    if base + draws.len() > MAX_INDIRECT || !gpu.indirect_first_instance {
        return None;
    }
    draws.sort_by_key(|(b, _)| vk::Handle::as_raw(*b));
    let flat: Vec<vk::DrawIndexedIndirectCommand> = draws.iter().map(|(_, c)| *c).collect();
    let stride = size_of::<vk::DrawIndexedIndirectCommand>();
    ind.write(base * stride, &flat);
    let mut i = 0;
    while i < draws.len() {
        let b = draws[i].0;
        let n = draws[i..].iter().take_while(|(x, _)| *x == b).count();
        d.cmd_bind_vertex_buffers(cmd, 0, &[b, b], &[0, 0]);
        d.cmd_bind_index_buffer(cmd, b, 0, vk::IndexType::UINT32);
        let offset = ((base + i) * stride) as u64;
        if multi {
            d.cmd_draw_indexed_indirect(cmd, ind.handle, offset, n as u32, stride as u32);
        } else {
            for k in 0..n {
                let at = offset + (k * stride) as u64;
                d.cmd_draw_indexed_indirect(cmd, ind.handle, at, 1, stride as u32);
            }
        }
        i += n;
    }
    Some(base + draws.len())
}
