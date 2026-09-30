//! The pieces the CPU-built models are made of: a triangle, a quad (two triangles) and a box
//! (six quads), as world-space vertices.
//!
//! A quad's corners go counter-clockwise seen from its front; its triangles are (0, 1, 2) and
//! (0, 2, 3), wound the other way (0, 2, 1) and (0, 3, 2) for its back.

use crate::world::mesh::{box_uv, corner_pos, corner_uv, Vertex, CORNERS, FACE_V};
use glam::{Mat4, Vec3};

/// Which sides of a triangle or quad are drawn (the faces seen from behind are culled): its
/// front, its back, or both (the front's triangles first).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sides {
    Front,
    Back,
    Both,
}

/// How a vertex is drawn besides where: its texture layer, its light (`light[3]` is replaced by
/// `face`, the face index its shading by direction goes by), its tint and flags.
#[derive(Clone, Copy)]
pub struct Paint {
    pub layer: u32,
    pub light: [u8; 4],
    pub face: u8,
    pub tint: [u8; 3],
    pub fl: u8,
}

impl Paint {
    fn vertex(&self, p: Vec3, uv: [f32; 2]) -> Vertex {
        let l = self.light;
        let t = self.tint;
        Vertex {
            pos: p.to_array(),
            uv,
            layer: self.layer as f32,
            light: [l[0], l[1], l[2], self.face],
            tint: [t[0], t[1], t[2], self.fl],
        }
    }
}

/// A triangle whose corners are already in the world.
pub fn tri_at(out: &mut Vec<Vertex>, pos: [Vec3; 3], uvs: [[f32; 2]; 3], paint: &Paint, sides: Sides) {
    let v: [Vertex; 3] = std::array::from_fn(|i| paint.vertex(pos[i], uvs[i]));
    if sides != Sides::Back {
        out.extend_from_slice(&v);
    }
    if sides != Sides::Front {
        out.extend_from_slice(&[v[0], v[2], v[1]]);
    }
}

/// A quad whose corners are already in the world.
pub fn quad_at(out: &mut Vec<Vertex>, pos: [Vec3; 4], uvs: [[f32; 2]; 4], paint: &Paint, sides: Sides) {
    let v: [Vertex; 4] = std::array::from_fn(|i| paint.vertex(pos[i], uvs[i]));
    if sides != Sides::Back {
        out.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
    }
    if sides != Sides::Front {
        out.extend_from_slice(&[v[0], v[2], v[1], v[0], v[3], v[2]]);
    }
}

/// A quad with its corners in model space, `m` taking them to the world.
pub fn quad(out: &mut Vec<Vertex>, m: Mat4, corners: [Vec3; 4], uvs: [[f32; 2]; 4], paint: &Paint, sides: Sides) {
    quad_at(out, corners.map(|c| m.transform_point3(c)), uvs, paint, sides);
}

/// The texture points of a rectangle of the texture (u left, v top, u right, v bottom) at a
/// quad's corners: its bottom left first, counter-clockwise.
pub fn rect_uvs([u0, v0, u1, v1]: [f32; 4]) -> [[f32; 2]; 4] {
    [[u0, v1], [u1, v1], [u1, v0], [u0, v0]]
}

/// How a box's faces are textured.
#[derive(Clone, Copy)]
pub enum BoxUv {
    /// The whole texture on each face, the faces standing up (the sides) showing only its rows
    /// `rows` (0 top .. 1 bottom): one part of something longer, like half of an arm.
    Rows([f32; 2]),
    /// A rectangle of the texture for each face (u left, v top, u right, v bottom).
    Rects([[f32; 4]; 6]),
    /// Minecraft's model UVs for a box inside a block (0..1): each face shows the piece of the
    /// texture where it lies on the block's face.
    Model,
}

/// A box from `min` to `max` in model space, `m` taking it to the world: its faces in the
/// order of `world::mesh::FACE_N` (+X, -X, +Y, -Y, +Z, -Z), each drawn as `paint` gives it
/// (None: left out), its front outward.
pub fn cuboid(out: &mut Vec<Vertex>, m: Mat4, min: Vec3, max: Vec3, uv: BoxUv, mut paint: impl FnMut(usize) -> Option<Paint>) {
    for face in 0..6 {
        let Some(p) = paint(face) else { continue };
        let mut pos = [Vec3::ZERO; 4];
        let mut uvs = [[0.0; 2]; 4];
        for (i, &(su, sv)) in CORNERS.iter().enumerate() {
            let local = min + (max - min) * Vec3::from(corner_pos(face, su, sv));
            pos[i] = m.transform_point3(local);
            uvs[i] = match uv {
                BoxUv::Rows(rows) => {
                    let mut uv = corner_uv(su, sv);
                    if FACE_V[face] == [0, 1, 0] {
                        uv[1] = rows[0] + (rows[1] - rows[0]) * uv[1];
                    }
                    uv
                }
                BoxUv::Rects(r) => {
                    let [cu, cv] = corner_uv(su, sv);
                    let r = r[face];
                    [r[0] + (r[2] - r[0]) * cu, r[1] + (r[3] - r[1]) * cv]
                }
                BoxUv::Model => box_uv(face, local.to_array()),
            };
        }
        quad_at(out, pos, uvs, &p, Sides::Front);
    }
}
