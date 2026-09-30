//! Which chunks a view draws, and how much of each: the frustum test, the distance limit and
//! the detail far chunks leave out.

use super::arena::Arena;
use super::chunks::ChunkGpu;
use crate::world::{ChunkPos, FastMap};
use ash::vk;
use glam::{Mat4, Vec3, Vec4};

pub(super) struct Frustum([Vec4; 6]);

impl Frustum {
    pub fn new(m: Mat4) -> Self {
        let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));
        Self([r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2])
    }

    pub fn visible(&self, min: Vec3, max: Vec3) -> bool {
        self.0.iter().all(|p| {
            let v = Vec3::new(
                if p.x >= 0.0 { max.x } else { min.x },
                if p.y >= 0.0 { max.y } else { min.y },
                if p.z >= 0.0 { max.z } else { min.z },
            );
            p.truncate().dot(v) + p.w >= 0.0
        })
    }
}

/// A few index ranges of a chunk's mesh to draw, next ones joined.
#[derive(Clone, Copy, Default)]
pub(super) struct Parts {
    ranges: [(u32, u32); 4],
    n: usize,
}

impl Parts {
    /// Adds `count` indices from `first` (after the ranges already in).
    pub fn push(&mut self, first: u32, count: u32) {
        if count == 0 {
            return;
        }
        let n = self.n;
        if n > 0 && self.ranges[n - 1].0 + self.ranges[n - 1].1 == first {
            self.ranges[n - 1].1 += count;
        } else if n < self.ranges.len() {
            self.ranges[n] = (first, count);
            self.n += 1;
        } else {
            // Out of slots: draw on to the end of this one.
            self.ranges[n - 1].1 = first + count - self.ranges[n - 1].0;
        }
    }

    /// The ranges (first, count).
    pub fn iter(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.ranges[..self.n].iter().copied()
    }
}

impl ChunkGpu {
    /// The solid indices to draw when only the whole-block faces turned toward `facing`
    /// directions (see `FACE_N`) show: the plain faces (drawn without alpha testing), and the
    /// rest (the other solid ones and the cut-out faces).
    pub fn solid_parts(&self, facing: [bool; 6]) -> (Parts, Parts) {
        let (mut plain, mut rest) = (Parts::default(), Parts::default());
        let mut at = 0;
        for (d, &count) in self.dirs.iter().enumerate() {
            if facing[d] {
                plain.push(at, count);
            }
            at += count;
        }
        let cut_total: u32 = self.cut_dirs.iter().sum();
        let mut cut_at = self.solid - cut_total;
        rest.push(at, cut_at - at);
        for (d, &count) in self.cut_dirs.iter().enumerate() {
            if facing[d] {
                rest.push(cut_at, count);
            }
            cut_at += count;
        }
        (plain, rest)
    }
}

/// A chunk drawn this frame: where its mesh is and how far away it is.
pub(super) struct VisibleChunk {
    /// Index ranges of its opaque part to draw: the faces turned toward the camera, without
    /// the small detail far chunks leave out. Plain whole-block faces (drawn without alpha
    /// testing), and the rest.
    pub plain: Parts,
    pub parts: Parts,
    /// Opaque indices drawn from the start, all directions (without the small detail far
    /// chunks leave out): for views that do not leave out faces by direction.
    pub drawn: u32,
    pub buffer: vk::Buffer,
    pub vertices: u64,
    pub indices: u64,
    /// Opaque indices (where the water's start).
    pub opaque: u32,
    pub water: u32,
    pub dist2: f32,
}

/// The chunks seen from `cam` through `view_proj`, up to a little over `view_distance` away
/// (horizontally), in the map's order. `detail_px` is how many pixels a block at distance 1
/// covers in this view.
pub(super) fn select_chunks(
    chunks: &FastMap<ChunkPos, ChunkGpu>,
    arena: &Arena,
    view_proj: Mat4,
    cam: Vec3,
    view_distance: f32,
    detail_px: f32,
    capacity: usize,
) -> Vec<VisibleChunk> {
    let frustum = Frustum::new(view_proj);
    let max_d = view_distance + 24.0;
    let mut visible: Vec<VisibleChunk> = Vec::with_capacity(capacity);
    for c in chunks.values() {
        let Some(r) = c.mesh else { continue };
        let center = (c.min + c.max) * 0.5;
        let (dx, dz) = (center.x - cam.x, center.z - cam.z);
        let dist2 = dx * dx + dz * dz;
        if dist2 > max_d * max_d || !frustum.visible(c.min, c.max) {
            continue;
        }
        // Far chunks leave out what the shaders would drop anyway: grass and flowers
        // (world.vert, under ~5 pixels a block) and the faces between leaves (closed
        // crowns, under ~1.5).
        let near = cam.clamp(c.min, c.max);
        let block_px = detail_px / near.distance(cam).max(1e-3);
        let drawn = if block_px >= 5.0 {
            c.opaque
        } else if block_px >= 1.5 {
            c.solid + c.leaf_inner
        } else {
            c.solid
        };
        // Whole-block faces turned away from the camera are left out by direction:
        // +X faces only show from the +X side of the chunk's west edge, and so on.
        let facing = [
            cam.x > c.min.x,
            cam.x < c.max.x,
            cam.y > c.min.y,
            cam.y < c.max.y,
            cam.z > c.min.z,
            cam.z < c.max.z,
        ];
        let (plain, mut parts) = c.solid_parts(facing);
        parts.push(c.solid, drawn - c.solid);
        visible.push(VisibleChunk {
            plain,
            parts,
            drawn,
            buffer: arena.buffer(r),
            vertices: r.offset,
            indices: r.offset + c.index_offset,
            opaque: c.opaque,
            water: c.water,
            dist2,
        });
    }
    visible
}
