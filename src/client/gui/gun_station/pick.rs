//! What the mouse is on at a gun station: something lying on its table (or a part of it),
//! and in its drawer the brush, the boxes of rounds, the loader and the handle; what the
//! table's top hides is not found. The testbed's `pickmap` draws it for the whole view.

use super::anim::{Scene, scene};
use super::pieces::Piece;
use super::table::{DRAWER_DEPTH, Table};
use crate::client::Game;
use crate::client::gui::station::{Screen2, hit_plane_t};
use crate::entity::{BenchItem, GunBench};
use crate::item::AMMO_BOX;
use crate::model::gun_station as station_model;
use crate::model::viewmodel::{Cube, cube_matrix};
use crate::util::ray_box;
use glam::{IVec3, Mat4, Vec2, Vec3};

/// What the mouse can point at on the table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::client) enum Pick {
    /// Something lying there (by id).
    Item(u16),
    /// An attachment on a gun lying there (the gun's id, the `gun_mod` bit).
    Mod(u16, u8),
    /// The brush in the drawer.
    Brush,
    /// A box of rounds in the drawer.
    Ammo(u8),
    /// The magazine in a gun lying there (the gun's id).
    Mag(u16),
    /// The drawer's handle (opens and shuts it).
    Handle,
    /// The rifle station's magazine loader in its drawer (or the magazine on it).
    Loader,
}

impl Pick {
    /// The thing lying on the table it is (or is on).
    pub(super) fn item(self) -> Option<u16> {
        match self {
            Pick::Item(id) | Pick::Mod(id, _) | Pick::Mag(id) => Some(id),
            _ => None,
        }
    }
}

/// How far along the ray `o`, `d` it goes into a cube that `m` puts in the world.
fn ray_cube(m: Mat4, c: &Cube, o: Vec3, d: Vec3) -> Option<f32> {
    let inv = m.inverse();
    let (a, b) = (Vec3::from(c.from), Vec3::from(c.to));
    ray_box(inv.transform_point3(o), inv.transform_vector3(d), a.min(b), a.max(b), 64.0)
}

/// The nearest of the pieces (and the things drawn flat) the mouse ray (`o` + t `d`) goes
/// through, and how far along it.
pub(super) fn pick(table: &Table, pieces: &[Piece], flats: &[BenchItem], o: Vec3, d: Vec3) -> Option<(Pick, f32)> {
    let mut best: Option<(Pick, f32)> = None;
    let mut take = |p: Pick, t: f32| {
        if best.is_none_or(|(_, bt)| t < bt) {
            best = Some((p, t));
        }
    };
    for pc in pieces {
        let Some(p) = pc.pick else { continue };
        for c in pc.visible() {
            if let Some(t) = ray_cube(pc.mats[c.bone] * cube_matrix(c), c, o, d) {
                take(p, t);
            }
        }
    }
    for it in flats {
        if it.stack.item == AMMO_BOX {
            let size = station_model::ammo_box_size();
            let inv = table.box_matrix(it).inverse();
            let (lo, hi) = (Vec3::new(-size.x * 0.5, 0.0, -size.z * 0.5), Vec3::new(size.x * 0.5, size.y, size.z * 0.5));
            if let Some(t) = ray_box(inv.transform_point3(o), inv.transform_vector3(d), lo, hi, 64.0) {
                take(Pick::Item(it.id), t);
            }
            continue;
        }
        let c = table.at(it.x, it.z);
        let r = Vec3::new(0.1, 0.03, 0.1);
        if let Some(t) = ray_box(o, d, c - r, c + r, 64.0) {
            take(Pick::Item(it.id), t);
        }
    }
    best
}

/// Whether something `t` along the ray `o`, `d` is behind the table's top (its slab, top and
/// front edge): what lies in the drawer under it. A ray that is below the slab's underside
/// inside the table's outline went through the slab.
pub(super) fn hidden_by_top(table: &Table, o: Vec3, d: Vec3, t: f32) -> bool {
    /// How thick the table's top is (blocks).
    const SLAB: f32 = 0.12;
    if d.y.abs() < 1e-6 {
        return false;
    }
    let under = (table.center.y - SLAB - o.y) / d.y;
    if under <= 0.0 || t <= under + 1e-3 {
        return false;
    }
    let (x, z) = table.local(o + d * under);
    x.abs() <= table.wide * 0.5 && z.abs() <= 0.5
}

impl Game {
    /// What the mouse is on at the station whose left block is `p` (the ray `o`, `d`): the
    /// nearest thing there is to click, and how far along the ray. What lies in the drawer is
    /// only found with the drawer out; what the table's top hides is not found.
    pub(super) fn bench_pick(&self, p: IVec3, table: &Table, bench: &GunBench, scene: &Scene, drawer: f32, o: Vec3, d: Vec3) -> Option<(Pick, f32)> {
        let mut found = pick(table, &scene.0, &scene.1, o, d);
        let nearer = |found: &mut Option<(Pick, f32)>, k: Pick, t: f32| {
            if found.is_none_or(|(_, bt)| t < bt) {
                *found = Some((k, t));
            }
        };
        let (brush, out) = (self.bench_ui.brush, drawer > 0.8);
        if !brush && out {
            for (i, c, m) in station_model::ammo_boxes(table.rifle(), p, table.toward, drawer) {
                if let Some(t) = ray_cube(m, c, o, d) {
                    nearer(&mut found, Pick::Ammo(i as u8), t);
                }
            }
        }
        // The loader's bay, the middle of the rifle station's drawer: the mouse on anything
        // in it (the loader, the magazine on it, the bay's floor) is on the loader, when
        // there is something to do with it; nowhere else is.
        if !brush && out && table.rifle() && bench.loader_takes(self.me.items.cursor) {
            let mut near = hit_plane_t(o, d, table.center.y - DRAWER_DEPTH);
            if bench.loader {
                for (c, m) in station_model::loader_cubes(p, table.toward, drawer) {
                    if let Some(t) = ray_cube(m, c, o, d) {
                        near = Some(near.map_or(t, |n: f32| n.min(t)));
                    }
                }
            }
            if let Some(t) = near.filter(|&t| station_model::in_loader_bay(p, table.toward, drawer, o + d * t)) {
                if found.is_none_or(|(k, bt)| k == Pick::Loader || t < bt) {
                    found = Some((Pick::Loader, t));
                }
            }
        }
        if !brush && self.me.items.cursor.is_none() && out {
            for (c, m) in station_model::brush_in_drawer(table.rifle(), p, table.toward, drawer) {
                if let Some(t) = ray_cube(m, c, o, d) {
                    nearer(&mut found, Pick::Brush, t);
                }
            }
        }
        // The handle shuts the drawer (with something held too): the mouse on it is always
        // on it, whatever lies in the drawer behind it.
        if !brush {
            for (c, m) in station_model::drawer_handle(table.rifle(), p, table.toward, drawer) {
                if let Some(t) = ray_cube(m, c, o, d) {
                    if !matches!(found, Some((Pick::Handle, bt)) if bt <= t) {
                        found = Some((Pick::Handle, t));
                    }
                }
            }
        }
        // The table's top hides what is behind it: what is in the drawer (under the top) is
        // not there to click where the mouse is on the top.
        if found.is_some_and(|(_, t)| hidden_by_top(table, o, d, t)) {
            found = None;
        }
        found
    }

    /// The test's `pickmap`: a map of what a click would do at every point of the view (the
    /// station at `p`, with what is on the mouse now), written as a picture beside the view's
    /// own: the loader's bay green, the boxes of rounds orange, the brush blue, the handle
    /// white, what lies on the table red, nothing black.
    pub(in crate::client) fn bench_pick_map(&self, p: IVec3, path: &std::path::Path) {
        let Some(table) = self.bench_table(p) else { return };
        let (w, h) = (self.ui.w, self.ui.h);
        let view = Screen2 { view_proj: self.me.look.view_proj, w, h };
        let bench = self.level.block_entities.benches.get(&p).cloned().unwrap_or_default();
        let made = scene(&table, &bench, self.bench_time(p));
        let drawer = self.level.bench_drawer.get(&p).copied().unwrap_or(0.0);
        let (mw, mh) = ((w / 4.0) as u32, (h / 4.0) as u32);
        let mut px = Vec::with_capacity((mw * mh * 3) as usize);
        for y in 0..mh {
            for x in 0..mw {
                let (o, d) = view.ray(Vec2::new(x as f32 * 4.0 + 2.0, y as f32 * 4.0 + 2.0));
                let c = match self.bench_pick(p, &table, &bench, &made, drawer, o, d).map(|(k, _)| k) {
                    Some(Pick::Loader) => [40, 220, 60],
                    Some(Pick::Ammo(_)) => [240, 150, 30],
                    Some(Pick::Brush) => [60, 120, 250],
                    Some(Pick::Handle) => [240, 240, 240],
                    Some(_) => [220, 40, 40],
                    None => [0, 0, 0],
                };
                px.extend_from_slice(&c);
            }
        }
        if let Ok(f) = std::fs::File::create(path) {
            let mut e = png::Encoder::new(std::io::BufWriter::new(f), mw, mh);
            e.set_color(png::ColorType::Rgb);
            if let Ok(mut wr) = e.write_header() {
                let _ = wr.write_image_data(&px);
            }
        }
    }
}
