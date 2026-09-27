//! Geometry built on the CPU every frame (entities, held items, the hand, particles), in
//! world-space vertices: the shared pieces (boxes, items, torches, crack overlays) and the
//! models made of them.

pub mod ballistics;
pub mod gun;
pub mod hand;
pub mod lantern;
pub mod particles;
pub mod player;

use crate::item::{icon, Icon, ItemId};
use crate::world::mesh::{corner_pos, corner_uv, flags, Vertex, CORNERS};
use crate::world::textures::{tex, ITEM_MASKS, MASK};
use crate::world::{
    face_texture, icon_tint, is_plant, is_stairs, is_water, tint_kind, TintKind, TORCH,
};
use glam::{Mat4, Vec3};
/// Any item centered on the origin with unit size: a cube for blocks, a thin double-sided
/// sprite for everything else.
pub fn emit_held(out: &mut Vec<Vertex>, m: Mat4, item: ItemId, light: [u8; 4], fl: u8) {
    if item == TORCH as ItemId {
        emit_torch(out, m, light, fl, 94);
        return;
    }
    if item == crate::item::PISTOL {
        gun::emit_pistol(out, m * gun::gun_to_unit(), light, fl, 0);
        return;
    }
    match icon(item) {
        Icon::Block(b) => emit_item(out, m, b, light, fl),
        Icon::Flat(layer) => emit_sprite(out, m, layer, light, fl),
    }
}

/// Like `emit_held`, but a flat item is only its front and back (no edge walls): for items
/// lying flat and seen from above, where the edges hardly show, so a chest full of them
/// stays cheap.
pub fn emit_lying(out: &mut Vec<Vertex>, m: Mat4, item: ItemId, light: [u8; 4], fl: u8) {
    let Icon::Flat(layer) = icon(item) else {
        emit_held(out, m, item, light, fl);
        return;
    };
    if item == TORCH as ItemId {
        emit_held(out, m, item, light, fl);
        return;
    }
    let t = 1.0 / 32.0;
    for (z, normal) in [(t, 4u8), (-t, 5u8)] {
        let v = [
            (Vec3::new(-0.5, -0.5, z), [0.0, 1.0]),
            (Vec3::new(0.5, -0.5, z), [1.0, 1.0]),
            (Vec3::new(0.5, 0.5, z), [1.0, 0.0]),
            (Vec3::new(-0.5, 0.5, z), [0.0, 0.0]),
        ]
        .map(|(p, uv)| Vertex {
            pos: m.transform_point3(p).to_array(),
            uv,
            layer: layer as f32,
            light: [light[0], light[1], light[2], normal],
            tint: [255, 255, 255, fl],
        });
        if normal == 5 {
            out.extend_from_slice(&[v[0], v[2], v[1], v[0], v[3], v[2]]);
        } else {
            out.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
        }
    }
}

/// Wooden torch with a charcoal tip and two crossed, frame-animated flame planes.
pub fn emit_torch(out: &mut Vec<Vertex>, m: Mat4, light: [u8; 4], fl: u8, seed: u8) {
    emit_box(
        out,
        m,
        Vec3::new(-0.05, -0.34, -0.05),
        Vec3::new(0.05, 0.15, 0.05),
        [
            tex::TORCH_WOOD,
            tex::TORCH_WOOD,
            tex::TORCH_CAP,
            tex::TORCH_WOOD,
            tex::TORCH_WOOD,
            tex::TORCH_WOOD,
        ],
        [[255; 3]; 6],
        light,
        fl,
    );
    // The glowing head: as thin as the stick (a hair wider so the faces do not fight).
    emit_box(
        out,
        m,
        Vec3::new(-0.053, 0.1, -0.053),
        Vec3::new(0.053, 0.17, 0.053),
        [
            tex::TORCH_CHAR,
            tex::TORCH_CHAR,
            tex::TORCH_CAP,
            tex::TORCH_CHAR,
            tex::TORCH_CHAR,
            tex::TORCH_CHAR,
        ],
        [[255; 3]; 6],
        light,
        fl,
    );

    for (a, b) in [
        (Vec3::new(-0.11, 0.0, -0.11), Vec3::new(0.11, 0.0, 0.11)),
        (Vec3::new(0.11, 0.0, -0.11), Vec3::new(-0.11, 0.0, 0.11)),
    ] {
        let pts = [
            (a + Vec3::Y * 0.16, [0.0, 1.0]),
            (b + Vec3::Y * 0.16, [1.0, 1.0]),
            (b + Vec3::Y * 0.39, [1.0, 0.0]),
            (a + Vec3::Y * 0.39, [0.0, 0.0]),
        ];
        let v = pts.map(|(p, uv)| Vertex {
            pos: m.transform_point3(p).to_array(),
            uv,
            layer: tex::TORCH_FLAME as f32,
            light: [light[0], light[1], light[2], 6],
            // The red tint channel carries one constant animation phase per torch.
            tint: [seed, 255, 255, fl | flags::EMISSIVE],
        });
        out.extend_from_slice(&[
            v[0], v[1], v[2], v[0], v[2], v[3], v[0], v[2], v[1], v[0], v[3], v[2],
        ]);
    }
}

/// A flame: two crossed planes from the origin up to y = 1 (width 1), frame-animated in the
/// shader like a torch's flame; `seed` gives it its own phase. Drawn blended (see
/// `build_scene`), glowing.
pub fn emit_flame(out: &mut Vec<Vertex>, m: Mat4, fl: u8, seed: u8) {
    for (a, b) in [
        (Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 0.0, 0.5)),
        (Vec3::new(0.5, 0.0, -0.5), Vec3::new(-0.5, 0.0, 0.5)),
    ] {
        let pts = [
            (a, [0.0, 1.0]),
            (b, [1.0, 1.0]),
            (b + Vec3::Y, [1.0, 0.0]),
            (a + Vec3::Y, [0.0, 0.0]),
        ];
        let v = pts.map(|(p, uv)| Vertex {
            pos: m.transform_point3(p).to_array(),
            uv,
            layer: tex::TORCH_FLAME as f32,
            light: [255, 255, 255, 6],
            tint: [seed, 255, 255, fl | flags::EMISSIVE],
        });
        out.extend_from_slice(&[
            v[0], v[1], v[2], v[0], v[2], v[3], v[0], v[2], v[1], v[0], v[3], v[2],
        ]);
    }
}

/// Item entity / third-person rendering: `size` is the edge length of a block item.
pub fn emit_item_flat_or_block(
    out: &mut Vec<Vertex>,
    m: Mat4,
    item: ItemId,
    size: f32,
    light: [u8; 4],
    fl: u8,
) {
    let scale = if matches!(icon(item), Icon::Block(_)) {
        size
    } else {
        size * 1.5
    };
    emit_held(
        out,
        m * Mat4::from_translation(Vec3::Y * scale * 0.5) * Mat4::from_scale(Vec3::splat(scale)),
        item,
        light,
        fl,
    );
}

/// Flat item sprite in the XY plane, 1/16 thick, visible from both sides.
/// A flat item as a 3D model, like Minecraft's generated item models: the sprite's front and
/// back, one texture pixel apart, plus a side wall along every edge of its opaque pixels,
/// colored like the pixel it belongs to.
fn emit_sprite(out: &mut Vec<Vertex>, m: Mat4, layer: u32, light: [u8; 4], fl: u8) {
    emit_sprite_sides(out, m, [layer, layer], layer, light, fl);
}

/// A flat item model with its own texture on each face (+Z shows `faces[0]`, -Z
/// `faces[1]`) and on its edges (`wall`), like a piece of meat cooked on one side. The edges
/// follow the front's shape.
pub fn emit_sprite_sides(
    out: &mut Vec<Vertex>,
    m: Mat4,
    faces: [u32; 2],
    wall: u32,
    light: [u8; 4],
    fl: u8,
) {
    let [layer, back] = faces;
    let t = 1.0 / 32.0;
    let vert_on = |l: u32, p: Vec3, uv: [f32; 2], normal: u8| Vertex {
        pos: m.transform_point3(p).to_array(),
        uv,
        layer: l as f32,
        light: [light[0], light[1], light[2], normal],
        tint: [255, 255, 255, fl],
    };
    let vert = |p: Vec3, uv: [f32; 2], normal: u8| vert_on(wall, p, uv, normal);
    for (z, flip) in [(t, false), (-t, true)] {
        let l = if flip { back } else { layer };
        let vert = |p: Vec3, uv: [f32; 2], normal: u8| vert_on(l, p, uv, normal);
        let v = [
            vert(
                Vec3::new(-0.5, -0.5, z),
                [0.0, 1.0],
                if flip { 5 } else { 4 },
            ),
            vert(
                Vec3::new(0.5, -0.5, z),
                [1.0, 1.0],
                if flip { 5 } else { 4 },
            ),
            vert(Vec3::new(0.5, 0.5, z), [1.0, 0.0], if flip { 5 } else { 4 }),
            vert(
                Vec3::new(-0.5, 0.5, z),
                [0.0, 0.0],
                if flip { 5 } else { 4 },
            ),
        ];
        if flip {
            out.extend_from_slice(&[v[0], v[2], v[1], v[0], v[3], v[2]]);
        } else {
            out.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
        }
    }

    let Ok(masks) = ITEM_MASKS.read() else {
        return;
    };
    let Some(mask) = masks.get(layer as usize) else {
        return;
    };
    let n = MASK as i32;
    let opaque = |x: i32, y: i32| {
        (0..n).contains(&x) && (0..n).contains(&y) && mask[y as usize] >> x & 1 == 1
    };
    let f = |i: i32| i as f32 / n as f32;
    for y in 0..n {
        for x in 0..n {
            if !opaque(x, y) {
                continue;
            }
            let (x0, x1) = (f(x) - 0.5, f(x + 1) - 0.5);
            let (top, bottom) = (0.5 - f(y), 0.5 - f(y + 1));
            let uv = [f(x) + 0.5 / n as f32, f(y) + 0.5 / n as f32];
            // (neighbour, edge from, edge to, normal index)
            let sides = [
                ((-1, 0), (x0, bottom), (x0, top), 1),
                ((1, 0), (x1, top), (x1, bottom), 0),
                ((0, -1), (x0, top), (x1, top), 2),
                ((0, 1), (x1, bottom), (x0, bottom), 3),
            ];
            for ((dx, dy), a, b, normal) in sides {
                if opaque(x + dx, y + dy) {
                    continue;
                }
                let v = [
                    vert(Vec3::new(a.0, a.1, -t), uv, normal),
                    vert(Vec3::new(b.0, b.1, -t), uv, normal),
                    vert(Vec3::new(b.0, b.1, t), uv, normal),
                    vert(Vec3::new(a.0, a.1, t), uv, normal),
                ];
                // Both windings: the wall is seen from either side depending on the transform.
                out.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
                out.extend_from_slice(&[v[0], v[2], v[1], v[0], v[3], v[2]]);
            }
        }
    }
}

/// A block item centered on the origin with unit size (cube, or a crossed sprite for plants).
pub fn emit_item(out: &mut Vec<Vertex>, m: Mat4, b: u8, light: [u8; 4], fl: u8) {
    let tint = icon_tint(b);
    if is_plant(b) {
        emit_cross(out, m, face_texture(b, 0), tint, light, fl);
    } else if is_stairs(b) {
        let layers = [face_texture(b, 0); 6];
        let tints = [[255; 3]; 6];
        let h = 0.5;
        emit_box(out, m, Vec3::splat(-h), Vec3::new(h, 0.0, h), layers, tints, light, fl);
        emit_box(out, m, Vec3::new(-h, 0.0, 0.0), Vec3::splat(h), layers, tints, light, fl);
    } else {
        let layers = std::array::from_fn(|f| face_texture(b, f));
        let tints = std::array::from_fn(|f| {
            if tint_kind(b, f) != TintKind::None || is_water(b) {
                tint
            } else {
                [255; 3]
            }
        });
        emit_box(
            out,
            m,
            Vec3::splat(-0.5),
            Vec3::splat(0.5),
            layers,
            tints,
            light,
            fl,
        );
    }
}

/// Box from `min` to `max` in model space, transformed by `m`.
#[allow(clippy::too_many_arguments)]
pub fn emit_box(
    out: &mut Vec<Vertex>,
    m: Mat4,
    min: Vec3,
    max: Vec3,
    layers: [u32; 6],
    tints: [[u8; 3]; 6],
    light: [u8; 4],
    fl: u8,
) {
    for face in 0..6 {
        let mut quad = [Vertex::default(); 4];
        for (i, &(su, sv)) in CORNERS.iter().enumerate() {
            let c = corner_pos(face, su, sv);
            let local = min + (max - min) * Vec3::from(c);
            let p = m.transform_point3(local);
            let tn = tints[face];
            quad[i] = Vertex {
                pos: p.to_array(),
                uv: corner_uv(su, sv),
                layer: layers[face] as f32,
                light: [light[0], light[1], light[2], face as u8],
                tint: [tn[0], tn[1], tn[2], fl],
            };
        }
        out.extend_from_slice(&[quad[0], quad[1], quad[2], quad[0], quad[2], quad[3]]);
    }
}

fn emit_cross(out: &mut Vec<Vertex>, m: Mat4, layer: u32, tint: [u8; 3], light: [u8; 4], fl: u8) {
    for (a, b) in [((-0.5, -0.5), (0.5, 0.5)), ((0.5, -0.5), (-0.5, 0.5))] {
        let pts = [
            (Vec3::new(a.0, -0.5, a.1), [0.0, 1.0]),
            (Vec3::new(b.0, -0.5, b.1), [1.0, 1.0]),
            (Vec3::new(b.0, 0.5, b.1), [1.0, 0.0]),
            (Vec3::new(a.0, 0.5, a.1), [0.0, 0.0]),
        ];
        let v = pts.map(|(p, uv)| Vertex {
            pos: m.transform_point3(p).to_array(),
            uv,
            layer: layer as f32,
            light,
            tint: [tint[0], tint[1], tint[2], fl],
        });
        out.extend_from_slice(&[
            v[0], v[1], v[2], v[0], v[2], v[3], v[0], v[2], v[1], v[0], v[3], v[2],
        ]);
    }
}

/// Crack overlay around a block being mined.
pub fn crack_overlay(out: &mut Vec<Vertex>, p: glam::IVec3, progress: f32) {
    let stage = ((progress * 10.0) as u32).min(9);
    let e = 0.004;
    emit_box(
        out,
        Mat4::IDENTITY,
        p.as_vec3() - Vec3::splat(e),
        p.as_vec3() + Vec3::splat(1.0 + e),
        [tex::CRACK + stage; 6],
        [[255; 3]; 6],
        [255, 255, 0, 0],
        flags::OVERLAY,
    );
}
