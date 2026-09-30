//! Block faces: their directions and corners, and whole cube faces with smooth lighting
//! and ambient occlusion.

use super::*;

// Faces: 0 +X, 1 -X, 2 +Y, 3 -Y, 4 +Z, 5 -Z. u x v = n, so corners are CCW seen from outside.
pub const FACE_N: [[i32; 3]; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];
pub const FACE_U: [[i32; 3]; 6] = [
    [0, 0, -1],
    [0, 0, 1],
    [1, 0, 0],
    [1, 0, 0],
    [1, 0, 0],
    [-1, 0, 0],
];
pub const FACE_V: [[i32; 3]; 6] = [
    [0, 1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
    [0, 1, 0],
    [0, 1, 0],
];
pub const CORNERS: [(i32, i32); 4] = [(-1, -1), (1, -1), (1, 1), (-1, 1)];

/// Unit-cube corner (0..1) for a face corner.
pub fn corner_pos(face: usize, su: i32, sv: i32) -> [f32; 3] {
    let (n, u, v) = (FACE_N[face], FACE_U[face], FACE_V[face]);
    std::array::from_fn(|k| 0.5 + 0.5 * (n[k] + su * u[k] + sv * v[k]) as f32)
}

pub fn corner_uv(su: i32, sv: i32) -> [f32; 2] {
    [(su + 1) as f32 * 0.5, 1.0 - (sv + 1) as f32 * 0.5]
}

/// Texture coordinates for a point `p` (0..1 in the block) on a face, so parts smaller than a
/// block show the matching piece of the texture (Minecraft-style model UVs).
pub fn box_uv(face: usize, p: [f32; 3]) -> [f32; 2] {
    let along = |a: [i32; 3]| {
        let k = a.iter().position(|&c| c != 0).unwrap();
        if a[k] > 0 {
            p[k]
        } else {
            1.0 - p[k]
        }
    };
    [along(FACE_U[face]), 1.0 - along(FACE_V[face])]
}

/// Which glass neighbours a glass face joins with (connected textures): bits 0 -u, 1 +u,
/// 2 +v, 3 -v, then the corners (-u,+v), (+u,+v), (-u,-v), (+u,-v). A neighbour only joins
/// if its own face in this direction is visible too.
pub(super) fn glass_mask(r: &Region, x: i32, y: i32, z: i32, face: usize) -> u8 {
    let (n, u, v) = (FACE_N[face], FACE_U[face], FACE_V[face]);
    let joins = |du: i32, dv: i32| {
        let p = [
            x + du * u[0] + dv * v[0],
            y + du * u[1] + dv * v[1],
            z + du * u[2] + dv * v[2],
        ];
        r.get(p[0], p[1], p[2]) == GLASS && r.get(p[0] + n[0], p[1] + n[1], p[2] + n[2]) != GLASS
    };
    let dirs = [
        (-1, 0),
        (1, 0),
        (0, 1),
        (0, -1),
        (-1, 1),
        (1, 1),
        (-1, -1),
        (1, -1),
    ];
    dirs.iter()
        .enumerate()
        .filter(|(_, &(du, dv))| joins(du, dv))
        .fold(0u8, |m, (bit, _)| m | 1 << bit)
}

#[inline]
fn occludes(b: u8) -> bool {
    is_opaque(b) || is_leaves(b)
}

impl Builder {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn cube_face(
        &mut self,
        r: &Region,
        x: i32,
        y: i32,
        z: i32,
        face: usize,
        layer: u32,
        tint: [u8; 3],
        fl: u8,
        rotated: bool,
    ) {
        let (n, u, v) = (FACE_N[face], FACE_U[face], FACE_V[face]);
        let (fx, fy, fz) = (x + n[0], y + n[1], z + n[2]);
        let base = self.verts.len() as u32;
        let flame_seed = if layer == tex::FURNACE_FRONT_LIT {
            (x + self.ox)
                .wrapping_mul(73)
                .wrapping_add((z + self.oz).wrapping_mul(151)) as u8
        } else {
            tint[0]
        };
        let mut ao = [3u8; 4];
        for (i, &(su, sv)) in CORNERS.iter().enumerate() {
            let s1 = (fx + su * u[0], fy + su * u[1], fz + su * u[2]);
            let s2 = (fx + sv * v[0], fy + sv * v[1], fz + sv * v[2]);
            let c = (s1.0 + sv * v[0], s1.1 + sv * v[1], s1.2 + sv * v[2]);
            let (b1, b2, bc) = (
                r.get(s1.0, s1.1, s1.2),
                r.get(s2.0, s2.1, s2.2),
                r.get(c.0, c.1, c.2),
            );
            let (o1, o2, oc) = (occludes(b1), occludes(b2), occludes(bc));
            ao[i] = if o1 && o2 {
                0
            } else {
                3 - (o1 as u8 + o2 as u8 + oc as u8)
            };

            let (mut ss, mut sb) = r.light(fx, fy, fz);
            let mut cnt = 1;
            if !is_opaque(b1) {
                let l = r.light(s1.0, s1.1, s1.2);
                ss += l.0;
                sb += l.1;
                cnt += 1;
            }
            if !is_opaque(b2) {
                let l = r.light(s2.0, s2.1, s2.2);
                ss += l.0;
                sb += l.1;
                cnt += 1;
            }
            if !is_opaque(bc) && !(is_opaque(b1) && is_opaque(b2)) {
                let l = r.light(c.0, c.1, c.2);
                ss += l.0;
                sb += l.1;
                cnt += 1;
            }
            let p = corner_pos(face, su, sv);
            self.push(Vertex {
                pos: [
                    (x + self.ox) as f32 + p[0],
                    y as f32 + p[1],
                    (z + self.oz) as f32 + p[2],
                ],
                uv: if rotated {
                    let uv = corner_uv(su, sv);
                    [uv[1], 1.0 - uv[0]]
                } else {
                    corner_uv(su, sv)
                },
                layer: layer as f32,
                light: [
                    ao[i] * 85,
                    (ss * 17 / cnt) as u8,
                    (sb * 17 / cnt) as u8,
                    face as u8,
                ],
                tint: [flame_seed, tint[1], tint[2], fl],
            });
        }
        let idx = &mut self.opaque;
        if ao[0] as u32 + ao[2] as u32 > ao[1] as u32 + ao[3] as u32 {
            idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        } else {
            idx.extend_from_slice(&[base + 1, base + 2, base + 3, base + 1, base + 3, base]);
        }
    }
}
