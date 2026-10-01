//! Command recording of the frame's passes (shadow, scope, main, UI), and what they share:
//! the recording context and the chunks' indirect draws.

mod main;
mod scope;
mod shadow;
mod ui;

pub(super) use main::CLEAR;

use super::arena::Arena;
use super::chunks::ChunkVertex;
use super::dynamic::DynRanges;
use crate::engine::resources::bytes;
use crate::engine::{Buffer, Gpu};
use std::mem::size_of;
use wgpu::util::DrawIndexedIndirectArgs;

/// Indirect draw commands per frame (chunks' opaque parts, a few each).
pub(super) const MAX_INDIRECT: usize = 65536;

/// Sets the pipeline's immediates (both shader stages read them).
pub(super) fn push<T: Copy>(pass: &mut wgpu::RenderPass, v: &T) {
    pass.set_immediates(0, bytes(std::slice::from_ref(v)));
}

/// Viewport and scissor both the `width` x `height` rectangle at (`x`, `y`).
pub(super) fn set_view(pass: &mut wgpu::RenderPass, x: u32, y: u32, width: u32, height: u32) {
    pass.set_viewport(x as f32, y as f32, width as f32, height as f32, 0.0, 1.0);
    pass.set_scissor_rect(x, y, width, height);
}

/// What every pass of one frame records with.
pub(super) struct Rec<'a> {
    pub gpu: &'a Gpu,
    /// The dynamic vertex buffer, and where this frame's ranges are in it.
    pub dyn_buf: wgpu::Buffer,
    pub dyn_ranges: DynRanges,
}

impl Rec<'_> {
    /// Dynamic range `i` (`dynamic::PARTICLES`...): first vertex and count.
    pub fn range(&self, i: usize) -> (u32, u32) {
        self.dyn_ranges.range(i)
    }

    pub fn bind_dyn(&self, pass: &mut wgpu::RenderPass) {
        pass.set_vertex_buffer(0, self.dyn_buf.slice(..));
    }

    /// Binds the dynamic buffer and draws its range `i`, if it has anything.
    pub fn draw_dyn(&self, pass: &mut wgpu::RenderPass, i: usize) {
        let (first, n) = self.range(i);
        if n > 0 {
            self.bind_dyn(pass);
            pass.draw(first..first + n, 0..1);
        }
    }
}

/// An indirect draw of a chunk: the mesh page it reads, and the command.
type IndirectDraw = (usize, DrawIndexedIndirectArgs);

/// Where a chunk's mesh is: its page, its vertices and indices in it (bytes), and its head
/// as an instance (`ChunkVertex`: the head is read per instance, from the buffer's start).
#[derive(Clone, Copy)]
pub(super) struct ChunkMesh {
    pub page: usize,
    pub vertices: u64,
    pub indices: u64,
    pub instance: u32,
}

impl ChunkMesh {
    pub fn new(page: usize, range: u64, vertex_offset: u64, index_offset: u64) -> Self {
        Self { page, vertices: range + vertex_offset, indices: range + index_offset, instance: (range / 16) as u32 }
    }

    /// Binds its buffers for `draw`.
    pub fn bind(&self, pass: &mut wgpu::RenderPass, arena: &Arena) {
        let buffer = arena.buffer(self.page);
        pass.set_vertex_buffer(0, buffer.slice(self.vertices..));
        pass.set_vertex_buffer(1, buffer.slice(..));
        pass.set_index_buffer(buffer.slice(self.indices..), wgpu::IndexFormat::Uint32);
    }

    /// Draws `count` indices from `first` (bound).
    pub fn draw(&self, pass: &mut wgpu::RenderPass, first: u32, count: u32) {
        pass.draw_indexed(first..first + count, 0, self.instance..self.instance + 1);
    }
}

/// One indirect draw of `count` indices from `first` of a chunk's mesh.
fn chunk_draw(m: &ChunkMesh, first: u32, count: u32) -> IndirectDraw {
    (
        m.page,
        DrawIndexedIndirectArgs {
            index_count: count,
            instance_count: 1,
            first_index: (m.indices / 4) as u32 + first,
            base_vertex: (m.vertices / size_of::<ChunkVertex>() as u64) as i32,
            first_instance: m.instance,
        },
    )
}

/// Records `draws` as indirect draws from `ind`, from command `base` on: one command per mesh
/// page (the meshes share a few big buffers), in order within each. Returns the next free
/// command, or None (nothing recorded) when they do not fit.
fn record_indirect(
    pass: &mut wgpu::RenderPass,
    arena: &Arena,
    ind: &Buffer,
    base: usize,
    draws: &mut [IndirectDraw],
    gpu: &Gpu,
) -> Option<usize> {
    // (the chunks' heads are instances: without indirect draws starting at an instance, they
    // are drawn one by one)
    if base + draws.len() > MAX_INDIRECT || !gpu.indirect_first_instance {
        return None;
    }
    draws.sort_by_key(|(page, _)| *page);
    let flat: Vec<u8> = draws.iter().flat_map(|(_, c)| c.as_bytes().iter().copied()).collect();
    let stride = size_of::<DrawIndexedIndirectArgs>();
    ind.write(base * stride, &flat);
    let mut i = 0;
    while i < draws.len() {
        let page = draws[i].0;
        let n = draws[i..].iter().take_while(|(p, _)| *p == page).count();
        let buffer = arena.buffer(page);
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.set_vertex_buffer(1, buffer.slice(..));
        pass.set_index_buffer(buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.multi_draw_indexed_indirect(&ind.handle, ((base + i) * stride) as u64, n as u32);
        i += n;
    }
    Some(base + draws.len())
}
