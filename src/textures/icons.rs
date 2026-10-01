//! Item icons drawn from the items' 3D models: the fixed ones made with the textures, and
//! `render_icon` for an item as it is (drawn while the game runs, `client::gui::icons`).

use super::*;

/// The guns' and grenades' item icons, drawn from their 3D models (the Blockbench pistol and
/// grenades) as they are held: turned a little to show them in 3D, lit from the upper left,
/// drawn at twice the size and scaled down for smooth edges.
pub(super) fn render_item_icons(base: &mut [u8]) {
    use crate::item::*;
    let mut loaded = Stack::one(PISTOL);
    set_gun_rounds(&mut loaded, 12);
    let mut full = Stack::one(PISTOL_MAGAZINE);
    set_gun_rounds(&mut full, 12);
    let mut ext = Stack::one(EXTENDED_MAGAZINE);
    set_gun_rounds(&mut ext, 20);
    let mut revolver = Stack::one(REVOLVER);
    set_gun_rounds(&mut revolver, 6);
    let mut loader = Stack::one(SPEEDLOADER);
    set_gun_rounds(&mut loader, 6);
    let mut ak = Stack::one(AK47);
    set_gun_rounds(&mut ak, 30);
    let mut ak_mag = Stack::one(AK_MAGAZINE);
    set_gun_rounds(&mut ak_mag, 30);
    let icons = [
        (tex::PISTOL, loaded),
        (tex::PISTOL_PARTS, Stack::one(PISTOL_FRAME)),
        (tex::PISTOL_PARTS + 1, Stack::one(PISTOL_BARREL)),
        (tex::PISTOL_PARTS + 2, Stack::one(PISTOL_SPRING)),
        (tex::PISTOL_PARTS + 3, Stack::one(PISTOL_SLIDE)),
        (tex::PISTOL_PARTS + 4, full),
        (tex::BULLET, Stack::one(BULLET)),
        (tex::GUN_ATTACHMENTS, Stack::one(SCOPE)),
        (tex::GUN_ATTACHMENTS + 1, Stack::one(SILENCER)),
        (tex::GUN_ATTACHMENTS + 2, ext),
        (tex::GUN_ATTACHMENTS + 3, Stack::one(LASER_SIGHT)),
        (tex::FLASHLIGHT, Stack::one(FLASHLIGHT)),
        (tex::REVOLVER, revolver),
        (tex::REVOLVER_PARTS, Stack::one(REVOLVER_FRAME)),
        (tex::REVOLVER_PARTS + 1, Stack::one(REVOLVER_BARREL)),
        (tex::REVOLVER_PARTS + 2, Stack::one(REVOLVER_SPRING)),
        (tex::REVOLVER_PARTS + 3, Stack::one(REVOLVER_CYLINDER)),
        (tex::REVOLVER_PARTS + 4, Stack::one(REVOLVER_HAMMER)),
        (tex::SPEEDLOADER, loader),
        (tex::MAGNUM_ROUND, Stack::one(MAGNUM_ROUND)),
        (tex::FRAG_GRENADE, Stack::one(FRAG_GRENADE)),
        (tex::SMOKE_GRENADE, Stack::one(SMOKE_GRENADE)),
        (tex::TARGET_DUMMY, Stack::one(TARGET_DUMMY)),
        (tex::AK47, ak),
        (tex::AK_PARTS, Stack::one(AK_RECEIVER)),
        (tex::AK_PARTS + 1, Stack::one(AK_GAS_TUBE)),
        (tex::AK_PARTS + 2, Stack::one(AK_BOLT)),
        (tex::AK_PARTS + 3, Stack::one(AK_COVER)),
        (tex::AK_PARTS + 4, ak_mag),
        (tex::RIFLE_ROUND, Stack::one(RIFLE_ROUND)),
        (tex::MAG_LOADER, Stack::one(MAG_LOADER)),
        (tex::BUCKET, Stack::one(BUCKET)),
        (tex::WATER_BUCKET, Stack::one(WATER_BUCKET)),
        (tex::LAVA_BUCKET, Stack::one(LAVA_BUCKET)),
        (tex::LANTERN_ITEM, Stack::one(crate::world::LANTERN as ItemId)),
    ];
    for (layer, st) in icons {
        let img = render_icon(base, &st);
        let dst = layer as usize * TILE * TILE * 4;
        base[dst..dst + TILE * TILE * 4].copy_from_slice(&img);
    }
    let img = fishing_rod_icon(base, TILE);
    let dst = tex::FISHING_ROD as usize * TILE * TILE * 4;
    base[dst..dst + TILE * TILE * 4].copy_from_slice(&img);
}

/// The fishing rod's icon (`size` square) drawn from its model: corner to corner like
/// Minecraft's, the butt at the bottom left, the tip at the top right bending a little, the
/// reel hanging below it with its crank toward the viewer.
fn fishing_rod_icon(base: &[u8], size: usize) -> Vec<u8> {
    use crate::model::items::fishing_rod::{emit, RodPose};
    use glam::{Mat4, Vec3, Vec4};
    let s = std::f32::consts::FRAC_1_SQRT_2;
    // The rod's +Z to the upper right, its up to the upper left, its crank's side (+X) toward
    // the viewer; turned a little about itself to show the reel's side. (That is a mirror
    // image, a left-handed reel: the only way to show the crank with the reel under the rod
    // and the tip at the top right. Its triangles are turned back below.)
    let view = Mat4::from_cols(
        Vec4::new(0.0, 0.0, 1.0, 0.0),
        Vec4::new(-s, s, 0.0, 0.0),
        Vec4::new(s, s, 0.0, 0.0),
        Vec4::W,
    ) * Mat4::from_rotation_z(-0.45)
        // (thicker than it is, or it would be a hair across the icon)
        * Mat4::from_scale(Vec3::new(1.8, 1.8, 1.0));
    let pose = RodPose { crank: 2.4, bend: 0.1, bend_dir: Vec3::NEG_Y };
    let mut verts = Vec::new();
    emit(&mut verts, view, &pose, [255, 255, 255, 0], 0);
    for tri in verts.chunks_exact_mut(3) {
        tri.swap(1, 2);
    }
    rasterize(base, &verts, size)
}

/// An item's icon drawn from its 3D model as it is (its state: rounds, attachments, dirt), a
/// `TILE` square, from the texture layers `base` (the items' model pages in it).
pub fn render_icon(base: &[u8], st: &crate::item::Stack) -> Vec<u8> {
    let mut verts = Vec::new();
    // A three-quarter view: turned toward the viewer's left, looked at a little from above.
    let turn = -0.4;
    // A bucket from higher up, to show what is in it.
    let down = if crate::model::items::bucket::is_bucket(st.item) { 0.62 } else { 0.35 };
    let view = glam::Mat4::from_rotation_x(down) * glam::Mat4::from_rotation_y(turn);
    crate::model::emit_held_data(&mut verts, view, st, [255, 255, 255, 0], 0);
    // A long gun and its long parts lie across the icon corner to corner, the muzzle end up, to
    // fill it (as Minecraft draws its long items).
    let (mut lo, mut hi) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
    for v in &verts {
        lo = lo.min(glam::Vec2::new(v.pos[0], v.pos[1]));
        hi = hi.max(glam::Vec2::new(v.pos[0], v.pos[1]));
    }
    let size = hi - lo;
    let long = crate::item::GunKind::of(st.item).is_some_and(|k| k.long()) || crate::item::AK_PARTS[..4].contains(&st.item);
    if long && size.x > size.y * 2.0 {
        let c = (lo + hi) * 0.5;
        let tip = glam::Mat4::from_translation(c.extend(0.0))
            * glam::Mat4::from_rotation_z(35f32.to_radians())
            * glam::Mat4::from_translation(-c.extend(0.0));
        for v in &mut verts {
            v.pos = tip.transform_point3(glam::Vec3::from(v.pos)).to_array();
        }
    }
    rasterize(base, &verts, TILE)
}

/// Draws triangles (x right, y up, z toward the viewer) looking straight at them, fitted into
/// a `size` square with a small margin: each pixel the nearest triangle's texel from the
/// texture layers, shaded by which way its face looks. Rendered twice as big and averaged
/// down, so edges are smooth; what is not covered is clear.
fn rasterize(base: &[u8], verts: &[crate::world::mesh::Vertex], size: usize) -> Vec<u8> {
    use glam::{Vec2, Vec3};
    let big = size * 2;
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for v in verts {
        let p = Vec2::new(v.pos[0], v.pos[1]);
        lo = lo.min(p);
        hi = hi.max(p);
    }
    let mut out = vec![0u8; size * size * 4];
    if lo.x > hi.x {
        return out;
    }
    let margin = big as f32 * 0.06;
    let scale = (big as f32 - 2.0 * margin) / (hi - lo).max_element().max(1e-6);
    let center = (lo + hi) * 0.5;
    let to_px = |p: Vec3| {
        Vec3::new(
            big as f32 * 0.5 + (p.x - center.x) * scale,
            big as f32 * 0.5 - (p.y - center.y) * scale,
            p.z,
        )
    };
    let mut color = vec![[0f32; 4]; big * big];
    let mut depth = vec![f32::MIN; big * big];
    let light = Vec3::new(-0.45, 0.75, 0.5).normalize();
    let layer_bytes = TILE * TILE * 4;
    for tri in verts.chunks_exact(3) {
        let world: [Vec3; 3] = std::array::from_fn(|i| Vec3::from(tri[i].pos));
        let p = world.map(to_px);
        let n = (world[1] - world[0]).cross(world[2] - world[0]).normalize_or_zero();
        // Faces seen from behind are not drawn (the cubes are closed).
        let n = if n.z < 0.0 { continue } else { n };
        let shade = 0.5 + 0.5 * n.dot(light).max(0.0);
        let area = (p[1].x - p[0].x) * (p[2].y - p[0].y) - (p[2].x - p[0].x) * (p[1].y - p[0].y);
        if area.abs() < 1e-8 {
            continue;
        }
        let x0 = p.iter().map(|q| q.x).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let x1 = (p.iter().map(|q| q.x).fold(f32::MIN, f32::max).ceil() as usize).min(big - 1);
        let y0 = p.iter().map(|q| q.y).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let y1 = (p.iter().map(|q| q.y).fold(f32::MIN, f32::max).ceil() as usize).min(big - 1);
        let layer = tri[0].layer as usize;
        let tint = tri[0].tint;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let w0 = ((p[1].x - fx) * (p[2].y - fy) - (p[2].x - fx) * (p[1].y - fy)) / area;
                let w1 = ((p[2].x - fx) * (p[0].y - fy) - (p[0].x - fx) * (p[2].y - fy)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let z = w0 * p[0].z + w1 * p[1].z + w2 * p[2].z;
                let i = y * big + x;
                if z <= depth[i] {
                    continue;
                }
                let u = w0 * tri[0].uv[0] + w1 * tri[1].uv[0] + w2 * tri[2].uv[0];
                let v = w0 * tri[0].uv[1] + w1 * tri[1].uv[1] + w2 * tri[2].uv[1];
                let tx = ((u * TILE as f32) as usize).min(TILE - 1);
                let ty = ((v * TILE as f32) as usize).min(TILE - 1);
                let t = layer * layer_bytes + (ty * TILE + tx) * 4;
                if t + 3 >= base.len() || base[t + 3] < 128 {
                    continue;
                }
                depth[i] = z;
                color[i] = [
                    base[t] as f32 * tint[0] as f32 / 255.0 * shade,
                    base[t + 1] as f32 * tint[1] as f32 / 255.0 * shade,
                    base[t + 2] as f32 * tint[2] as f32 / 255.0 * shade,
                    255.0,
                ];
            }
        }
    }
    // Averaged down (the colour weighted by coverage).
    for y in 0..size {
        for x in 0..size {
            let mut sum = [0f32; 4];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let c = color[(y * 2 + dy) * big + x * 2 + dx];
                for k in 0..3 {
                    sum[k] += c[k] * c[3] / 255.0;
                }
                sum[3] += c[3];
            }
            let a = sum[3] / 4.0;
            let o = (y * size + x) * 4;
            if a > 0.0 {
                for k in 0..3 {
                    out[o + k] = (sum[k] / (sum[3] / 255.0)).clamp(0.0, 255.0) as u8;
                }
                out[o + 3] = a.round() as u8;
            }
        }
    }
    out
}
