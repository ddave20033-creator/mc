//! The gun stations drawn in the world: each one's block model with its drawer, what lies on
//! its table (lit up under the mouse), what is held over it and the brush, for this player
//! and the others working there.

use super::anim::scene_at;
use super::pick::Pick;
use super::pieces::{Piece, as_laid, loader_piece, lying_pieces};
use super::table::Table;
use crate::client::{Container, Game, Screen};
use crate::entity::BenchItem;
use crate::item::{AMMO_BOX, Stack, belongs_on_bench};
use crate::model::guns::gun_station as station_model;
use crate::model::guns::gun_view;
use crate::model::rig::viewmodel::{cube_matrix, emit_cube};
use crate::util::vertex_light;
use crate::world::is_gun_bench;
use crate::world::mesh::{Vertex, flags};
use glam::{Mat3, Mat4, Quat, Vec2, Vec3};
use std::f32::consts::FRAC_PI_2;

/// The tint of what the mouse is on where what is held goes on or into it.
const GOES: [u8; 3] = [150, 255, 150];

/// Lights up what the mouse is on: brighter than white can make it (the light's own
/// channel), green with `tint`.
fn light_up(verts: &mut [Vertex], tint: Option<[u8; 3]>) {
    for v in verts {
        if let Some(c) = tint {
            v.tint[..3].copy_from_slice(&c);
        }
        v.light[0] = v.light[0].saturating_add(90);
        v.light[1] = v.light[1].saturating_add(90);
    }
}

/// Draws the pieces (the one under the mouse, `hover`, lit up: green where what is held on
/// the mouse goes on or into it) and the things lying flat.
pub(super) fn emit(out: &mut Vec<Vertex>, table: &Table, pieces: &[Piece], flats: &[BenchItem], hover: Option<(Pick, bool)>, light: [u8; 4]) {
    let hover_ok = hover.is_some_and(|h| h.1);
    let hover = hover.map(|h| h.0);
    for pc in pieces {
        let lit = pc.pick.is_some() && pc.pick == hover;
        let from = out.len();
        let first = gun_view::layers(pc.kind, pc.dirt);
        for c in pc.visible() {
            emit_cube(out, c, pc.mats[c.bone] * cube_matrix(c), first, light, flags::ENTITY);
        }
        if lit {
            light_up(&mut out[from..], Some(if hover_ok { GOES } else { pc.tint }));
        }
    }
    for it in flats {
        let lit = hover == Some(Pick::Item(it.id));
        let from = out.len();
        if it.stack.item == AMMO_BOX {
            station_model::emit_ammo_box(out, table.box_matrix(it), it.stack.data, light, flags::ENTITY);
            if lit {
                light_up(&mut out[from..], hover_ok.then_some(GOES));
            }
            continue;
        }
        let size = 0.28;
        let m = Mat4::from_translation(table.at(it.x, it.z) + Vec3::Y * (size / 32.0 + 0.004))
            * Mat4::from_quat(Quat::from_rotation_y(it.turn) * Quat::from_mat3(&Mat3::from_cols(table.right, Vec3::Y, table.toward)))
            * Mat4::from_rotation_x(-FRAC_PI_2)
            * Mat4::from_scale(Vec3::splat(size));
        crate::model::emit_lying(out, m, &it.stack, light, flags::ENTITY);
        if lit {
            light_up(&mut out[from..], None);
        }
    }
}

/// Something held on the mouse over a gun station, drawn in 3D where it shows (`at`: over
/// the spot it would be laid on, lifted).
pub(super) fn emit_hold(out: &mut Vec<Vertex>, table: &Table, st: Stack, at: Vec3, light: [u8; 4]) {
    let (x, z) = table.local(at);
    let it = BenchItem { id: 0, stack: as_laid(st), x, z, turn: 0.0 };
    let lift = Vec3::Y * (at.y - table.center.y);
    match lying_pieces(table, &it) {
        Some(mut pieces) => {
            for pc in &mut pieces {
                pc.pick = None;
                for m in &mut pc.mats {
                    *m = Mat4::from_translation(lift) * *m;
                }
            }
            emit(out, table, &pieces, &[], None, light);
        }
        None => {
            let from = out.len();
            emit(out, table, &[], &[it], None, light);
            for v in &mut out[from..] {
                v.pos[1] += lift.y;
            }
        }
    }
}

/// The patch of table under some pieces.
pub(super) fn glow_of<'a>(table: &Table, pieces: impl Iterator<Item = &'a Piece>) -> Option<[Vec3; 4]> {
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for pc in pieces {
        let (a, b) = pc.extent(table);
        lo = lo.min(a);
        hi = hi.max(b);
    }
    if lo.x > hi.x {
        return None;
    }
    let (pad, y) = (0.025, Vec3::Y * 0.002);
    let (x0, x1, z0, z1) = (lo.x - pad, hi.x + pad, lo.y - pad, hi.y + pad);
    Some([table.at(x0, z0) + y, table.at(x1, z0) + y, table.at(x1, z1) + y, table.at(x0, z1) + y])
}

/// The patch of table under what the mouse is on (the whole thing lying there).
pub(super) fn glow_under(table: &Table, pieces: &[Piece], flats: &[BenchItem], h: Pick) -> Option<[Vec3; 4]> {
    let id = h.item()?;
    glow_of(table, pieces.iter().filter(|pc| pc.pick.and_then(Pick::item) == Some(id))).or_else(|| {
        flats.iter().find(|f| f.id == id).map(|f| table.square(f.x, f.z, if f.stack.item == AMMO_BOX { 0.28 } else { 0.12 }))
    })
}

impl Game {
    /// Everything lying on every gun station near the camera, its drawer, the brush in it or
    /// in someone's hand.
    pub(in crate::client) fn build_benches(&mut self, out: &mut Vec<Vertex>, dt: f32) {
        let world = &self.terrain.world;
        let near = |p: &glam::IVec3| (p.as_vec3() - self.me.body.pos).length_squared() < 48.0 * 48.0;
        let open_here = match self.screen {
            Screen::Container(Container::GunStation(q)) => Some(q),
            _ => None,
        };
        let remote = self.remote_drawers();
        let remote_brushes = self.remote_brushes();
        let remote_holds = self.remote_bench_holds();
        let step = dt / station_model::open_seconds();
        let stations: Vec<glam::IVec3> = self.terrain.gun_stations.values().flatten().copied().filter(|p| near(p)).collect();
        for p in stations {
            let b = world.geti(p);
            let Some(table) = Table::of(p, b) else { continue };
            let here = open_here == Some(p);
            // The drawer is out while someone looks into it.
            let used = (here && self.bench_ui.in_drawer) || remote.contains(&p);
            let s = self.level.bench_drawer.entry(p).or_insert(0.0);
            *s = if used { (*s + step).min(1.0) } else { (*s - step).max(0.0) };
            let drawer = *s;
            let (sky, blk) = world.light_estimate(table.center + Vec3::Y * 0.2);
            let light = vertex_light(sky, blk);
            let brush_out = (here && self.bench_ui.brush) || remote_brushes.iter().any(|(q, _)| *q == p);
            let bench = self.level.block_entities.benches.get(&p);
            let ammo = bench.map_or([Some(0); 3], |b| b.boxes);
            let handle_lit = here && self.bench_ui.hover == Some(Pick::Handle);
            let loader = bench.map_or(Default::default(), |b| station_model::Loader {
                there: b.loader && table.rifle(),
                feed: b.loader_source().map(|_| self.clock.time),
            });
            station_model::emit_block(out, table.rifle(), p, table.toward, drawer, !brush_out, ammo, loader, handle_lit, light, flags::ENTITY);
            if table.rifle() {
                // The grenades in the crate on the shelf, and how many.
                let n = bench.map_or([0; 2], |b| b.grenades);
                let (sky, blk) = world.light_estimate(table.center - Vec3::Y * 0.7);
                station_model::emit_crate(out, p, table.toward, n, vertex_light(sky, blk), flags::ENTITY);
            }
            if let Some(bench) = bench {
                let made = scene_at(p, &table, bench, self.bench_time(p));
                let (pieces, flats) = (&made.0, &made.1);
                let hover = if here { self.bench_ui.hover.map(|h| (h, self.bench_ui.hover_ok)) } else { None };
                // The magazine on the loader (with the drawer, wherever it is).
                let on_loader = match (loader.there, bench.loader_mag) {
                    (true, Some(mag)) => station_model::loader_mount(p, table.toward, drawer).and_then(|m| loader_piece(m, &mag)),
                    _ => None,
                };
                match on_loader {
                    Some(pc) => {
                        let mut all = pieces.clone();
                        all.push(pc);
                        emit(out, &table, &all, flats, hover, light);
                    }
                    None => emit(out, &table, pieces, flats, hover, light),
                }
            }
            let facing = table.toward.x.atan2(table.toward.z);
            for (_, at) in remote_brushes.iter().filter(|(q, _)| *q == p) {
                station_model::emit_brush(out, *at, facing, 0.0, light, flags::ENTITY);
            }
            // What is held on the mouse, over the table or in the drawer: in 3D, lifted over
            // where it would go.
            if here {
                self.bench_ui.hold_at = None;
                let held_at = match (self.bench_ui.spot, self.bench_ui.drawer_spot) {
                    (Some(_), _) => self.bench_ui.held_spot.map(|(x, z)| (x, z, 0.0)),
                    (None, Some(q)) => {
                        let (x, z) = table.local(q);
                        Some((x, z, q.y - table.center.y))
                    }
                    _ => None,
                };
                let held = self.me.items.cursor.filter(|st| belongs_on_bench(st.item, table.rifle()));
                if let (Some(st), Some((x, z, below))) = (held, held_at) {
                    let at = table.at(x, z) + Vec3::Y * (0.06 + below);
                    self.bench_ui.hold_at = Some(at);
                    emit_hold(out, &table, st, at, light);
                }
            }
            // And what the others hold there.
            for (_, st, at) in remote_holds.iter().filter(|(q, _, _)| *q == p) {
                emit_hold(out, &table, *st, *at, light);
            }
            if here && self.bench_ui.brush {
                if let Some(at) = self.bench_ui.brush_at {
                    let (tilt, wiggle) = if self.bench_ui.scrubbing {
                        let s = (self.clock.time * 26.0).sin();
                        (s * 0.12, table.right * s * 0.015)
                    } else {
                        (0.0, Vec3::ZERO)
                    };
                    station_model::emit_brush(out, at + wiggle, facing, tilt, light, flags::ENTITY);
                }
            }
        }
        self.level.bench_drawer.retain(|p, s| *s > 0.0 && is_gun_bench(world.geti(*p)));
        self.level.bench_anims.retain(|p, _| is_gun_bench(world.geti(*p)));
    }
}
