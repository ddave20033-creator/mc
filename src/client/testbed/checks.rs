//! The testbed's checks: surfaces fighting over the same place (z-fighting, which flickers),
//! textures that are missing, and pictures of the same view a hair apart compared for
//! flicker. Each gives a `Finding` list for the report.

use crate::world::mesh::Vertex;
use glam::{Vec2, Vec3};
use std::collections::HashMap;

/// Something a check found: where (world or model space) and what.
pub struct Finding {
    pub at: Vec3,
    pub what: String,
}

/// A triangle, and what it is drawn with.
#[derive(Clone, Copy)]
struct Tri {
    p: [Vec3; 3],
    layer: f32,
}

/// Triangles facing the same way in the same plane that cover the same area (not just
/// touching): they fight over which is drawn and flicker as the view moves. `tris` are
/// vertex triples. The two halves of one quad (and a triangle drawn twice with the same
/// texture) are not counted. At most `limit` findings.
pub fn coplanar_overlaps(verts: &[Vertex], limit: usize) -> (usize, Vec<Finding>) {
    let tris: Vec<Tri> = verts
        .chunks_exact(3)
        .map(|t| Tri { p: [0, 1, 2].map(|i| Vec3::from(t[i].pos)), layer: t[0].layer })
        .collect();
    overlaps(&tris, limit)
}

/// `coplanar_overlaps` for an indexed mesh (a chunk's): `indices` into `verts`.
pub fn coplanar_overlaps_indexed(verts: &[Vertex], indices: &[u32], limit: usize) -> (usize, Vec<Finding>) {
    let tris: Vec<Tri> = indices
        .chunks_exact(3)
        .filter_map(|t| {
            let v = |i: u32| verts.get(i as usize);
            Some(Tri { p: [Vec3::from(v(t[0])?.pos), Vec3::from(v(t[1])?.pos), Vec3::from(v(t[2])?.pos)], layer: v(t[0])?.layer })
        })
        .collect();
    overlaps(&tris, limit)
}

fn overlaps(tris: &[Tri], limit: usize) -> (usize, Vec<Finding>) {
    // Grouped by plane: the normal and the distance, rounded.
    let mut planes: HashMap<(i32, i32, i32, i32), Vec<usize>> = HashMap::new();
    for (i, t) in tris.iter().enumerate() {
        let n = (t.p[1] - t.p[0]).cross(t.p[2] - t.p[0]);
        if n.length_squared() < 1e-10 {
            continue;
        }
        let n = n.normalize();
        let q = |v: f32| (v * 500.0).round() as i32;
        planes.entry((q(n.x), q(n.y), q(n.z), q(n.dot(t.p[0])))).or_default().push(i);
    }
    let mut count = 0;
    let mut found = Vec::new();
    for (key, ids) in planes {
        if ids.len() < 2 {
            continue;
        }
        // In the plane's own 2D coordinates, the triangles bucketed by where they are.
        let n = Vec3::new(key.0 as f32, key.1 as f32, key.2 as f32).normalize_or_zero();
        let u = if n.x.abs() < 0.9 { n.cross(Vec3::X) } else { n.cross(Vec3::Y) }.normalize();
        let v = n.cross(u);
        let flat: Vec<[Vec2; 3]> = ids.iter().map(|&i| tris[i].p.map(|p| Vec2::new(p.dot(u), p.dot(v)))).collect();
        let mut cells: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (k, t) in flat.iter().enumerate() {
            let (lo, hi) = bounds(t);
            for cy in (lo.y.floor() as i32)..=(hi.y.floor() as i32) {
                for cx in (lo.x.floor() as i32)..=(hi.x.floor() as i32) {
                    cells.entry((cx, cy)).or_default().push(k);
                }
            }
        }
        let mut seen = std::collections::HashSet::new();
        for list in cells.values() {
            for a in 0..list.len() {
                for b in a + 1..list.len() {
                    let (i, j) = (list[a].min(list[b]), list[a].max(list[b]));
                    if !seen.insert((i, j)) {
                        continue;
                    }
                    let (ta, tb) = (&tris[ids[i]], &tris[ids[j]]);
                    // The same triangle twice with the same texture shows nothing wrong.
                    if ta.layer == tb.layer && same_points(ta, tb) {
                        continue;
                    }
                    if overlap_2d(&flat[i], &flat[j]) {
                        count += 1;
                        if found.len() < limit {
                            let c = (ta.p[0] + ta.p[1] + ta.p[2]) / 3.0;
                            found.push(Finding {
                                at: c,
                                what: format!("layers {} and {} in one plane (facing {:.2},{:.2},{:.2})", ta.layer, tb.layer, n.x, n.y, n.z),
                            });
                        }
                    }
                }
            }
        }
    }
    (count, found)
}

fn bounds(t: &[Vec2; 3]) -> (Vec2, Vec2) {
    (t[0].min(t[1]).min(t[2]), t[0].max(t[1]).max(t[2]))
}

fn same_points(a: &Tri, b: &Tri) -> bool {
    a.p.iter().all(|p| b.p.iter().any(|q| p.distance_squared(*q) < 1e-8))
}

/// Whether two triangles share some area (more than touching along an edge or a point).
fn overlap_2d(a: &[Vec2; 3], b: &[Vec2; 3]) -> bool {
    const EPS: f32 = 1e-3;
    for (t, other) in [(a, b), (b, a)] {
        let area = (t[1] - t[0]).perp_dot(t[2] - t[0]);
        if area.abs() < 1e-9 {
            return false;
        }
        for k in 0..3 {
            let (p, q) = (t[k], t[(k + 1) % 3]);
            // The edge's inward side (the triangle's own winding).
            let side = |x: Vec2| (q - p).perp_dot(x - p) * area.signum() / (q - p).length();
            if other.iter().all(|&x| side(x) <= EPS) {
                return false;
            }
        }
    }
    true
}

/// Two pictures of the same view a hair apart: the pixels that changed a lot (more than
/// surfaces just shifting would), written as a picture (changed pixels red over the first
/// picture darkened). Returns the share of pixels that changed.
pub fn flicker_diff(a: &std::path::Path, b: &std::path::Path, out: &std::path::Path) -> Option<f32> {
    let (ia, ib) = (crate::pack::decode_png(&std::fs::read(a).ok()?)?, crate::pack::decode_png(&std::fs::read(b).ok()?)?);
    if ia.w != ib.w || ia.h != ib.h {
        return None;
    }
    let (w, h) = (ia.w as usize, ia.h as usize);
    let luma = |p: &[u8]| 0.3 * p[0] as f32 + 0.59 * p[1] as f32 + 0.11 * p[2] as f32;
    let mut changed = 0usize;
    let mut px = vec![0u8; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            let d = (luma(&ia.rgba[i..]) - luma(&ib.rgba[i..])).abs();
            // A shift shows at edges; a pixel that differs from all its neighbours in the
            // other picture too flickered.
            let steady = (-1i32..=1).any(|dy| {
                (-1i32..=1).any(|dx| {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h && {
                        let j = (ny as usize * w + nx as usize) * 4;
                        (luma(&ia.rgba[i..]) - luma(&ib.rgba[j..])).abs() < 24.0
                    }
                })
            });
            let o = (y * w + x) * 3;
            if d > 24.0 && !steady {
                changed += 1;
                px[o..o + 3].copy_from_slice(&[255, 40, 40]);
            } else {
                for c in 0..3 {
                    px[o + c] = ia.rgba[i + c] / 3;
                }
            }
        }
    }
    write_png(out, w as u32, h as u32, &px);
    Some(changed as f32 / (w * h) as f32)
}

/// An RGB picture as a PNG.
pub fn write_png(path: &std::path::Path, w: u32, h: u32, rgb: &[u8]) {
    let Ok(f) = std::fs::File::create(path) else { return };
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    e.set_color(png::ColorType::Rgb);
    e.set_depth(png::BitDepth::Eight);
    if let Ok(mut wr) = e.write_header() {
        let _ = wr.write_image_data(rgb);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad(out: &mut Vec<Vertex>, lo: [f32; 2], hi: [f32; 2], z: f32, layer: f32) {
        let v = |x: f32, y: f32| Vertex { pos: [x, y, z], layer, ..Default::default() };
        let (a, b, c, d) = (v(lo[0], lo[1]), v(hi[0], lo[1]), v(hi[0], hi[1]), v(lo[0], hi[1]));
        out.extend_from_slice(&[a, b, c, a, c, d]);
    }

    #[test]
    fn overlapping_faces_are_found_and_neighbours_are_not() {
        let mut v = Vec::new();
        // Two faces side by side, touching: fine.
        quad(&mut v, [0.0, 0.0], [1.0, 1.0], 0.0, 1.0);
        quad(&mut v, [1.0, 0.0], [2.0, 1.0], 0.0, 2.0);
        // One a little further: fine.
        quad(&mut v, [0.0, 0.0], [1.0, 1.0], 0.1, 3.0);
        assert_eq!(coplanar_overlaps(&v, 10).0, 0);
        // One over the first, half across it: fighting.
        quad(&mut v, [0.5, 0.0], [1.5, 1.0], 0.0, 4.0);
        let (n, found) = coplanar_overlaps(&v, 10);
        assert!(n > 0 && !found.is_empty());
    }
}
