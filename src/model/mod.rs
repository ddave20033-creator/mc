//! Geometry built on the CPU every frame (entities, held items, the hand, particles), in
//! world-space vertices: the shared pieces (boxes, items, torches, crack overlays) and the
//! models made of them.

pub mod hand;
pub mod lantern;
pub mod particles;
pub mod player;

use crate::item::{icon, Icon, ItemId};
use crate::world::mesh::{corner_pos, corner_uv, flags, Vertex, CORNERS};
use crate::world::textures::{tex, ITEM_MASKS, MASK};
use crate::world::{face_texture, icon_tint, is_plant, is_water, tint_kind, TintKind, TORCH};
use glam::{Mat4, Vec3};
/// Any item centered on the origin with unit size: a cube for blocks, a thin double-sided
/// sprite for everything else.
pub fn emit_held(out: &mut Vec<Vertex>, m: Mat4, item: ItemId, light: [u8; 4], fl: u8) {
    if item == TORCH as ItemId {
        emit_torch(out, m, light, fl, 94);
        return;
    }
    match icon(item) {
        Icon::Block(b) => emit_item(out, m, b, light, fl),
        Icon::Flat(layer) => emit_sprite(out, m, layer, light, fl),
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
    emit_box(
        out,
        m,
        Vec3::new(-0.068, 0.11, -0.068),
        Vec3::new(0.068, 0.19, 0.068),
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
    let t = 1.0 / 32.0;
    let vert = |p: Vec3, uv: [f32; 2], normal: u8| Vertex {
        pos: m.transform_point3(p).to_array(),
        uv,
        layer: layer as f32,
        light: [light[0], light[1], light[2], normal],
        tint: [255, 255, 255, fl],
    };
    for (z, flip) in [(t, false), (-t, true)] {
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

/// Flames on a burning entity, like Minecraft's `renderFlame`: layers of the fire animation
/// (fire_0 and fire_1 by turns) turned toward the camera, stacked up the entity's height,
/// each 0.15 higher, a bit narrower and further forward. `feet` is the bottom center of its box.
pub fn emit_entity_fire(out: &mut Vec<Vertex>, feet: Vec3, width: f32, height: f32, cam: Vec3) {
    use crate::world::textures::tex;
    let f = width * 1.4;
    let to_cam = Vec3::new(cam.x - feet.x, 0.0, cam.z - feet.z)
        .try_normalize()
        .unwrap_or(Vec3::Z);
    let side = Vec3::Y.cross(to_cam);
    let mut left = height / f;
    let (mut half, mut drop, mut depth) = (0.5, 0.0, 0.0);
    let forward = 0.3 - left.floor() * 0.02;
    let light = crate::util::vertex_light(15, 15);
    let mut l = 0;
    while left > 0.0 {
        let layer = if l % 2 == 0 { tex::FIRE_0 } else { tex::FIRE_1 };
        let (u0, u1) = if (l / 2) % 2 == 0 {
            (1.0, 0.0)
        } else {
            (0.0, 1.0)
        };
        let at = |x: f32, y: f32| feet + (side * x + Vec3::Y * y + to_cam * (forward + depth)) * f;
        let corners = [
            (at(half, -drop), [u1, 1.0]),
            (at(-half, -drop), [u0, 1.0]),
            (at(-half, 1.4 - drop), [u0, 0.0]),
            (at(half, 1.4 - drop), [u1, 0.0]),
        ];
        let v: [Vertex; 4] = std::array::from_fn(|i| Vertex {
            pos: corners[i].0.to_array(),
            uv: corners[i].1,
            layer: layer as f32,
            light: [light[0], light[1], light[2], 6],
            tint: [255, 255, 255, flags::ENTITY | flags::EMISSIVE],
        });
        // Both sides, so it shows whichever way the quad ends up wound.
        out.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
        out.extend_from_slice(&[v[0], v[2], v[1], v[0], v[3], v[2]]);
        left -= 0.45;
        drop -= 0.15;
        half *= 0.9;
        depth += 0.03;
        l += 1;
    }
}
