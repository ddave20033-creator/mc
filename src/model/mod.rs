//! Geometry built on the CPU every frame (entities, held items, the hand, particles), in
//! world-space vertices: the shared pieces (boxes, items, torches, crack overlays) and the
//! models made of them.
//!
//! The models live in subfolders: `rig` (the Blockbench bone/animation sampler, springs),
//! `guns`, `players`, `items` and `fx`. Each module is re-exported here under its old path
//! (`crate::model::pistol_view`, `crate::model::player`...), so code elsewhere need not know
//! which subfolder it is in.

mod fx;
mod guns;
mod items;
mod players;
mod rig;
#[cfg(test)]
mod geometry_tests;

pub use fx::particles;
pub use guns::{ak_vm, ballistics, grenade, gun, gun_station, gun_view, pistol_view, pistol_vm, revolver_view, revolver_vm};
pub use items::{angler, book, bucket, dummy, fishing_rod, lantern};
pub use players::{chop_rig, hand, player, tp_rig};
pub use rig::{spring, viewmodel};

use crate::item::{icon, Icon, ItemId};
use crate::world::mesh::{corner_pos, corner_uv, flags, Vertex, CORNERS};
use crate::world::textures::{tex, ITEM_MASKS, MASK};
use crate::world::{
    face_texture, icon_tint, is_log, is_plant, is_stairs, is_water, log_axis, log_radius, tint_kind,
    TintKind, TORCH,
};
use glam::{Mat4, Vec3};
/// Whether an item is drawn as a 3D model of its own (the pistol's parts, attachments,
/// magazines and rounds, the grenades) rather than as a flat icon.
pub fn is_model_item(item: ItemId) -> bool {
    item == crate::item::FRAG_GRENADE
        || item == crate::item::SMOKE_GRENADE
        || item == crate::item::TARGET_DUMMY
        || item == crate::item::MAG_LOADER
        || gun_view::item_rig(&crate::item::Stack::one(item)).is_some()
}

/// Whether an item is a grenade (a frag or a smoke one).
pub fn grenade_item(item: ItemId) -> bool {
    item == crate::item::FRAG_GRENADE || item == crate::item::SMOKE_GRENADE
}

/// Any item centered on the origin with unit size: a cube for blocks, a thin double-sided
/// sprite for everything else.
pub fn emit_held(out: &mut Vec<Vertex>, m: Mat4, item: ItemId, light: [u8; 4], fl: u8) {
    emit_held_data(out, m, &crate::item::Stack::one(item), light, fl);
}

/// `emit_held` for an item with its state: a gun shows its attachments, its magazine (or
/// none), its slide held back, and how dirty it is.
pub fn emit_held_data(out: &mut Vec<Vertex>, m: Mat4, st: &crate::item::Stack, light: [u8; 4], fl: u8) {
    let item = st.item;
    if let Some(fill) = bucket::Fill::of(item) {
        // In the world its liquid lies level with the world; on an icon, with the bucket.
        let own_up = fl & (flags::ENTITY | flags::VIEWMODEL) == 0;
        bucket::emit(out, m, fill, &bucket::Surface::still(own_up), 0.55, light, fl);
        return;
    }
    if item == TORCH as ItemId {
        emit_torch(out, m, light, fl, 94);
        return;
    }
    if item == crate::world::LANTERN as ItemId {
        // The lantern's model (pixels, standing on y = 0, 11 tall) filling the unit.
        let k = 0.9 / 11.0;
        let at = m * Mat4::from_scale(Vec3::splat(k)) * Mat4::from_translation(Vec3::new(0.0, -5.5, 0.0));
        lantern::emit_lantern(out, at, light, fl, lantern::LanternKind::Standing);
        return;
    }
    if item == crate::world::GUN_STATION as ItemId || item == crate::world::RIFLE_BENCH as ItemId {
        gun_station::emit_item(out, item == crate::world::RIFLE_BENCH as ItemId, m, light, fl);
        return;
    }
    if item == crate::item::FRAG_GRENADE || item == crate::item::SMOKE_GRENADE {
        grenade::emit_sized(out, item == crate::item::SMOKE_GRENADE, m, 0.62, light, fl);
        return;
    }
    if item == crate::item::TARGET_DUMMY {
        dummy::emit_sized(out, m, 0.95, light, fl);
        return;
    }
    if item == crate::item::MAG_LOADER {
        gun_station::emit_loader_item(out, m, light, fl);
        return;
    }
    if let Some((kind, bones, pose, mag, upright)) = gun_view::item_rig(st) {
        // A piece of a Blockbench gun (a part, an attachment, a magazine with its rounds
        // showing in its witness holes, a speedloader, a round), its middle at the origin, its
        // size the unit's (a round smaller), the muzzle end to +X, its right side toward +Z.
        use pistol_view::bench;
        let (mats, shown) = viewmodel::bone_matrices(gun_view::bones(kind), &pose, Mat4::IDENTITY);
        let cubes: Vec<&viewmodel::Cube> = gun_view::cubes(kind)
            .iter()
            .filter(|c| bones & (1 << c.bone) != 0 && shown[c.bone])
            .filter(|c| mag.is_none_or(|(n, cap)| bench::mag_cube_shown(pistol_view::rig(kind), c.name, n, cap)))
            .collect();
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for c in &cubes {
            let m = upright * mats[c.bone] * viewmodel::cube_matrix(c);
            for p in [Vec3::from(c.from), Vec3::from(c.to)] {
                let q = m.transform_point3(p);
                lo = lo.min(q);
                hi = hi.max(q);
            }
        }
        // A round keeps its real length (the models share their scale: the 9 mm round is 0.3
        // long, a magnum round longer); anything else fills the unit.
        let round = matches!(item, crate::item::BULLET | crate::item::MAGNUM_ROUND | crate::item::RIFLE_ROUND);
        let k = if round { 0.3 / 2.7 } else { 0.62 / (hi - lo).max_element().max(1e-3) };
        let root = m * Mat4::from_scale(Vec3::splat(k)) * Mat4::from_translation(-(lo + hi) * 0.5) * upright;
        let first = gun_view::layers(kind, pistol_view::dirt_level(st.damage, crate::item::max_damage(item)));
        for c in cubes {
            viewmodel::emit_cube(out, c, root * mats[c.bone] * viewmodel::cube_matrix(c), first, light, fl);
        }
        return;
    }
    if item == crate::item::AMMO_BOX {
        // The box standing in the unit cube, its longest side across it.
        let size = gun_station::ammo_box_size();
        let k = 1.0 / size.max_element();
        let at = m * Mat4::from_scale(Vec3::splat(k)) * Mat4::from_translation(Vec3::new(0.0, -size.y * 0.5, 0.0));
        gun_station::emit_ammo_box(out, at, st.data, light, fl);
        return;
    }
    if let Some(kind) = crate::item::GunKind::of(item) {
        // The Blockbench gun at rest, the size and place of the old gun model.
        let root = m * gun::gun_to_unit(kind) * gun_view::to_gun_space(kind);
        let state = pistol_view::GunAnim {
            locked: crate::item::gun_locked(st),
            no_mag: !crate::item::gun_has_mag(st),
            chambered: crate::item::gun_chambered(st),
            // The revolver's cylinder as it is; the pistol's magazine as full as it is.
            cyl: st.data,
            mag: (kind.uses_magazine() && crate::item::gun_has_mag(st))
                .then(|| (crate::item::gun_rounds(st), kind.magazine_size(crate::item::gun_mods(st)))),
            ..Default::default()
        };
        let mods = crate::item::gun_mods(st);
        let (mats, shown) = gun_view::matrices(kind, &state, mods, true, root);
        let dirt = pistol_view::dirt_level(st.damage, crate::item::max_damage(item));
        let lamp = mods & crate::item::gun_mod::LIGHT != 0 && mods & crate::item::gun_mod::LIGHT_ON != 0;
        gun_view::emit(kind, out, None, &mats, &shown, false, dirt, lamp, &state, light, fl);
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
pub fn emit_lying(out: &mut Vec<Vertex>, m: Mat4, st: &crate::item::Stack, light: [u8; 4], fl: u8) {
    let item = st.item;
    if bucket::is_bucket(item) || item == crate::world::LANTERN as ItemId {
        // Standing up on what it lies on (the flat item's +Z is up).
        let stand = Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2)
            * Mat4::from_scale(Vec3::splat(0.8))
            * Mat4::from_translation(Vec3::Y * 0.3);
        emit_held_data(out, m * stand, st, light, fl);
        return;
    }
    let Icon::Flat(layer) = icon(item) else {
        emit_held_data(out, m, st, light, fl);
        return;
    };
    if item == TORCH as ItemId {
        emit_held_data(out, m, st, light, fl);
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
    st: &crate::item::Stack,
    size: f32,
    light: [u8; 4],
    fl: u8,
) {
    let item = st.item;
    let scale = if matches!(icon(item), Icon::Block(_)) {
        size
    } else {
        size * 1.5
    };
    emit_held_data(
        out,
        m * Mat4::from_translation(Vec3::Y * scale * 0.5) * Mat4::from_scale(Vec3::splat(scale)),
        st,
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
    let vert_uv = |p: Vec3, uv: [f32; 2], normal: u8| vert_on(wall, p, uv, normal);
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

    for w in sprite_walls(layer).iter() {
        let v = [
            vert_uv(Vec3::new(w.a.0, w.a.1, -t), w.uv[0], w.normal),
            vert_uv(Vec3::new(w.b.0, w.b.1, -t), w.uv[1], w.normal),
            vert_uv(Vec3::new(w.b.0, w.b.1, t), w.uv[1], w.normal),
            vert_uv(Vec3::new(w.a.0, w.a.1, t), w.uv[0], w.normal),
        ];
        // Both windings: the wall is seen from either side depending on the transform.
        out.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
        out.extend_from_slice(&[v[0], v[2], v[1], v[0], v[3], v[2]]);
    }
}

/// One side wall of a flat item: from `a` to `b` in the sprite's plane (-0.5..0.5), the
/// texture there and the face it looks to.
struct SpriteWall {
    a: (f32, f32),
    b: (f32, f32),
    uv: [[f32; 2]; 2],
    normal: u8,
}

/// The side walls of a layer's flat item, worked out once from its opaque pixels (again when
/// the textures are made anew): a dropped stack or a held item is drawn every frame.
fn sprite_walls(layer: u32) -> std::sync::Arc<[SpriteWall]> {
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, Mutex};
    type Cache = (u32, Vec<Option<Arc<[SpriteWall]>>>);
    static CACHE: Mutex<Cache> = Mutex::new((u32::MAX, Vec::new()));
    let version = crate::world::textures::ITEM_MASKS_VERSION.load(Ordering::Acquire);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if cache.0 != version {
        *cache = (version, Vec::new());
    }
    let i = layer as usize;
    if let Some(Some(w)) = cache.1.get(i) {
        return w.clone();
    }
    let walls: Arc<[SpriteWall]> = ITEM_MASKS.read().ok().and_then(|m| m.get(i).map(walls_of)).unwrap_or_default().into();
    if cache.1.len() <= i {
        cache.1.resize(i + 1, None);
    }
    cache.1[i] = Some(walls.clone());
    walls
}

/// The walls round the opaque pixels of a mask: a run of pixels along a row (or a column)
/// with the same side bare one wall, the texture's pixels along it.
fn walls_of(mask: &[u128; MASK]) -> Vec<SpriteWall> {
    let n = MASK as i32;
    let opaque = |x: i32, y: i32| (0..n).contains(&x) && (0..n).contains(&y) && mask[y as usize] >> x & 1 == 1;
    let f = |i: i32| i as f32 / n as f32;
    let mut out = Vec::new();
    let mut wall = |a: (f32, f32), b: (f32, f32), uv: [[f32; 2]; 2], normal: u8| out.push(SpriteWall { a, b, uv, normal });
    for y in 0..n {
        let v = f(y) + 0.5 / n as f32;
        for (dy, normal) in [(-1, 2u8), (1, 3u8)] {
            let mut x = 0;
            while x < n {
                if !opaque(x, y) || opaque(x, y + dy) {
                    x += 1;
                    continue;
                }
                let start = x;
                while x < n && opaque(x, y) && !opaque(x, y + dy) {
                    x += 1;
                }
                let (x0, x1) = (f(start) - 0.5, f(x) - 0.5);
                let (ua, ub) = ([f(start), v], [f(x), v]);
                if dy < 0 {
                    let top = 0.5 - f(y);
                    wall((x0, top), (x1, top), [ua, ub], normal);
                } else {
                    let bottom = 0.5 - f(y + 1);
                    wall((x1, bottom), (x0, bottom), [ub, ua], normal);
                }
            }
        }
    }
    for x in 0..n {
        let u = f(x) + 0.5 / n as f32;
        for (dx, normal) in [(-1, 1u8), (1, 0u8)] {
            let mut y = 0;
            while y < n {
                if !opaque(x, y) || opaque(x + dx, y) {
                    y += 1;
                    continue;
                }
                let start = y;
                while y < n && opaque(x, y) && !opaque(x + dx, y) {
                    y += 1;
                }
                let (top, bottom) = (0.5 - f(start), 0.5 - f(y));
                let (ua, ub) = ([u, f(start)], [u, f(y)]);
                if dx < 0 {
                    let x0 = f(x) - 0.5;
                    wall((x0, bottom), (x0, top), [ub, ua], normal);
                } else {
                    let x1 = f(x + 1) - 0.5;
                    wall((x1, top), (x1, bottom), [ua, ub], normal);
                }
            }
        }
    }
    out
}

/// A block item centered on the origin with unit size (cube, or a crossed sprite for plants).
pub fn emit_item(out: &mut Vec<Vertex>, m: Mat4, b: u8, light: [u8; 4], fl: u8) {
    let tint = icon_tint(b);
    if is_plant(b) {
        emit_cross(out, m, face_texture(b, 0), tint, light, fl);
    } else if is_log(b) {
        emit_round_log(out, m, b, light, fl);
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

/// Sides of a round log item, and how many times its bark goes round (as on the placed one).
const LOG_ITEM_SIDES: usize = 12;

/// A log (or branch) item as it looks placed: round, along its axis through the unit, its
/// bark round it and its ends' rings.
fn emit_round_log(out: &mut Vec<Vertex>, m: Mat4, b: u8, light: [u8; 4], fl: u8) {
    let axis = log_axis(b);
    let (ua, va) = match axis {
        0 => (2, 1),
        1 => (0, 2),
        _ => (0, 1),
    };
    let radius = log_radius(b);
    let side = face_texture(b, if axis == 1 { 0 } else { 2 });
    let end = face_texture(b, if axis == 1 { 2 } else { 0 });
    let at = |t: f32, u: f32, v: f32| {
        let mut p = [0.0f32; 3];
        p[axis] = t;
        p[ua] = u;
        p[va] = v;
        m.transform_point3(Vec3::from(p)).to_array()
    };
    let face_of = |n: [f32; 3]| {
        let a = n.map(f32::abs);
        let k = if a[0] >= a[1] && a[0] >= a[2] { 0 } else if a[1] >= a[2] { 1 } else { 2 };
        (k * 2 + (n[k] < 0.0) as usize) as u8
    };
    let mut quad = |ps: [[f32; 3]; 4], uvs: [[f32; 2]; 4], layer: u32, n: [f32; 3]| {
        let face = face_of(n);
        let v: [Vertex; 4] = std::array::from_fn(|i| Vertex {
            pos: ps[i],
            uv: uvs[i],
            layer: layer as f32,
            light: [light[0], light[1], light[2], face],
            tint: [255, 255, 255, fl],
        });
        // Both windings: it is turned every way in the hand.
        out.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
        out.extend_from_slice(&[v[0], v[2], v[1], v[0], v[3], v[2]]);
    };
    // The bark goes round as many times as on the placed log (a whole texture each time).
    let rounds = if radius > 0.3 { 3 } else { 1 };
    let per = LOG_ITEM_SIDES / rounds;
    let k = crate::world::mesh::LOG_END_RIM / radius;
    for i in 0..LOG_ITEM_SIDES {
        let ang = |i: usize| i as f32 / LOG_ITEM_SIDES as f32 * std::f32::consts::TAU;
        let (c0, s0) = (ang(i).cos() * radius, ang(i).sin() * radius);
        let (c1, s1) = (ang(i + 1).cos() * radius, ang(i + 1).sin() * radius);
        let mid = (ang(i) + ang(i + 1)) * 0.5;
        let mut n = [0.0f32; 3];
        n[ua] = mid.cos();
        n[va] = mid.sin();
        let (u0, u1) = ((i % per) as f32 / per as f32, (i % per + 1) as f32 / per as f32);
        quad(
            [at(-0.5, c0, s0), at(-0.5, c1, s1), at(0.5, c1, s1), at(0.5, c0, s0)],
            [[u0, 1.0], [u1, 1.0], [u1, 0.0], [u0, 0.0]],
            side,
            n,
        );
        for t in [-0.5f32, 0.5] {
            let mut n = [0.0f32; 3];
            n[axis] = t * 2.0;
            let uv = |u: f32, v: f32| [0.5 + u * k, 0.5 + v * k];
            quad(
                [at(t, 0.0, 0.0), at(t, c0, s0), at(t, c1, s1), at(t, 0.0, 0.0)],
                [uv(0.0, 0.0), uv(c0, s0), uv(c1, s1), uv(0.0, 0.0)],
                end,
                n,
            );
        }
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
    emit_box_rows(out, m, min, max, layers, tints, light, fl, [0.0, 1.0]);
}

/// `emit_box` whose sides (the faces standing up) show only the rows `rows` (0 top .. 1 bottom)
/// of their textures: one part of something longer, like half of an arm.
#[allow(clippy::too_many_arguments)]
pub fn emit_box_rows(
    out: &mut Vec<Vertex>,
    m: Mat4,
    min: Vec3,
    max: Vec3,
    layers: [u32; 6],
    tints: [[u8; 3]; 6],
    light: [u8; 4],
    fl: u8,
    rows: [f32; 2],
) {
    for face in 0..6 {
        let mut quad = [Vertex::default(); 4];
        let upright = crate::world::mesh::FACE_V[face] == [0, 1, 0];
        for (i, &(su, sv)) in CORNERS.iter().enumerate() {
            let c = corner_pos(face, su, sv);
            let local = min + (max - min) * Vec3::from(c);
            let p = m.transform_point3(local);
            let tn = tints[face];
            let mut uv = corner_uv(su, sv);
            if upright {
                uv[1] = rows[0] + (rows[1] - rows[0]) * uv[1];
            }
            quad[i] = Vertex {
                pos: p.to_array(),
                uv,
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
