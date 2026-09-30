//! The dynamic vertex buffer: everything that is not a chunk mesh (particles, entities, the
//! hand, the outline...), written again each frame as eight ranges one after the other.

use super::frame::FrameInfo;
use crate::engine::Buffer;
use crate::world::mesh::Vertex;
use glam::Vec3;
use std::mem::size_of;

pub(super) const DYN_MAX_VERTS: usize = 1_000_000;

// The dynamic buffer's ranges, in their order in it.
pub(super) const PARTICLES: usize = 0;
/// Bullet holes and break cracks on the blocks.
pub(super) const OVERLAY: usize = 1;
pub(super) const VIEWMODEL: usize = 2;
/// The targeted block's outline (a line list).
pub(super) const LINES: usize = 3;
pub(super) const ENTITY: usize = 4;
pub(super) const TRANSLUCENT: usize = 5;
pub(super) const VIEWMODEL_GLASS: usize = 6;
pub(super) const LENS: usize = 7;

/// How many vertices of each dynamic range (particles, overlay, viewmodel, lines, entity,
/// translucent, viewmodel glass, lens) fit into the dynamic buffer. Over its size the least
/// needed are cut first (particles and the loose things drawn with them, then entities),
/// never the hand, the gun or the lens, and only whole triangles (lines: whole segments).
pub(super) fn dyn_budget(lens: [usize; 8]) -> [usize; 8] {
    const KEEP_FIRST: [usize; 8] = [VIEWMODEL_GLASS, LENS, VIEWMODEL, OVERLAY, LINES, TRANSLUCENT, ENTITY, PARTICLES];
    let mut left = DYN_MAX_VERTS;
    let mut out = [0; 8];
    for i in KEEP_FIRST {
        let whole = if i == LINES { 2 } else { 3 };
        out[i] = lens[i].min(left) / whole * whole;
        left -= out[i];
    }
    if lens.iter().sum::<usize>() > DYN_MAX_VERTS {
        static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            eprintln!("dynamic vertex buffer full: {} of {} vertices drawn", out.iter().sum::<usize>(), lens.iter().sum::<usize>());
        }
    }
    out
}

/// Where each range landed in this frame's dynamic buffer.
pub(super) struct DynRanges([u32; 9]);

impl DynRanges {
    /// Range `i`'s first vertex and vertex count.
    pub fn range(&self, i: usize) -> (u32, u32) {
        (self.0[i], self.0[i + 1] - self.0[i])
    }
}

/// Writes this frame's dynamic geometry into `buf` (as much as fits, see `dyn_budget`).
pub(super) fn write(buf: &Buffer, f: &FrameInfo) -> DynRanges {
    let lines = outline_lines(f.outline);
    let ranges = [
        f.particles,
        f.overlay,
        f.viewmodel,
        lines.as_slice(),
        f.entity,
        f.translucent,
        f.viewmodel_glass,
        f.lens,
    ];
    let counts = dyn_budget(ranges.map(|r| r.len()));
    let mut offsets = [0u32; 9];
    let mut cursor = 0usize;
    for (i, r) in ranges.iter().enumerate() {
        let n = counts[i];
        if n > 0 {
            buf.write(cursor * size_of::<Vertex>(), &r[..n]);
        }
        offsets[i] = cursor as u32;
        cursor += n;
    }
    offsets[8] = cursor as u32;
    DynRanges(offsets)
}

/// The edges of the box around the targeted block (a hair bigger, so they are not hidden in
/// its faces), as line-list vertices.
fn outline_lines(outline: Option<(Vec3, Vec3)>) -> Vec<Vertex> {
    let mut lines: Vec<Vertex> = Vec::new();
    if let Some((lo, hi)) = outline {
        let e = 0.004;
        let lo = lo - Vec3::splat(e);
        let hi = hi + Vec3::splat(e);
        let c = |x: bool, y: bool, z: bool| {
            [
                if x { hi.x } else { lo.x },
                if y { hi.y } else { lo.y },
                if z { hi.z } else { lo.z },
            ]
        };
        for a in [false, true] {
            for b in [false, true] {
                for pos in [
                    c(false, a, b),
                    c(true, a, b),
                    c(a, false, b),
                    c(a, true, b),
                    c(a, b, false),
                    c(a, b, true),
                ] {
                    lines.push(Vertex {
                        pos,
                        layer: -1.0,
                        ..Default::default()
                    });
                }
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_dynamic_buffer_cuts_particles_before_the_hand() {
        let all = dyn_budget([30, 6, 300, 4, 60, 9, 3, 6]);
        assert_eq!(all, [30, 6, 300, 4, 60, 9, 3, 6]);
        let over = dyn_budget([DYN_MAX_VERTS, 6, 300, 4, 60, 9, 3, 6]);
        assert_eq!(&over[1..], &[6, 300, 4, 60, 9, 3, 6]);
        assert!(over.iter().sum::<usize>() <= DYN_MAX_VERTS);
        assert_eq!(over[0] % 3, 0);
    }
}
