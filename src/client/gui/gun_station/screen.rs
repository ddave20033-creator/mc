//! The open gun station each frame: the inventory along the bottom, the camera swaying with
//! the mouse and going down to the drawer, where the mouse is on the table, what lights up
//! under it, and its buttons (what they do: `actions`).

use super::anim::scene_at;
use super::draw::{glow_of, glow_under};
use super::pick::Pick;
use super::pieces::{as_laid, lying_pieces};
use super::layout::free_spot_near;
use super::table::DRAWER_DEPTH;
use crate::client::gui::SlotRef;
use crate::client::gui::station::{Screen2, hit_plane};
use crate::client::{Container, Game};
use crate::entity::BenchItem;
use crate::item::{AMMO_BOX, BOX_AMMO, belongs_on_bench};
use crate::ui::rgba;
use crate::util::ray_box;
use glam::{IVec3, Vec3};

/// With something in the hand: where the mouse opens the drawer (down past this fraction of
/// the screen above the inventory, looking over the table) and shuts it again (looking into
/// the drawer, up past this one: to the table's edge at the top of the view), and how long
/// the camera takes to go down to it.
const DRAWER_OPEN: f32 = 0.84;
const DRAWER_CLOSE: f32 = 0.26;
const DRAWER_GLIDE: f32 = 0.35;
/// Seconds the mouse must stay there for the drawer to open (or close).
const DRAWER_DWELL: f32 = 0.35;
/// Moving the mouse this far (GUI pixels) with something picked up drags it.
const DRAG: f32 = 6.0;

impl Game {
    /// The open gun station: the inventory along the bottom, and whatever the mouse does on
    /// the table. Returns the inventory slot under the mouse.
    pub(in crate::client::gui) fn gun_station_screen(&mut self, p: IVec3) -> Option<SlotRef> {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        let mut hovered = None;
        // The inventory, on a dark strip along the bottom.
        let (pw, ph) = (176.0 * s, 86.0 * s);
        let (px, py) = (((w - pw) * 0.5).round(), (h - ph - 4.0 * s).round());
        self.ui.rect_full(px, py, pw, ph, rgba(10, 11, 16, 150), rgba(10, 11, 16, 190), 5.0 * s, 3.0 * s);
        self.inventory_slots(px, py, 5.0, &mut hovered);
        let over_inventory = self.ui.hit(px, py, pw, ph);
        // A box of rounds belongs to the station: it does not go into the inventory.
        let holding_box = self.me.items.cursor.is_some_and(|st| st.item == AMMO_BOX);
        if holding_box {
            hovered = None;
        }
        // The camera sways a little with the mouse, to see along the table; the mouse going
        // down toward the inventory looks into the drawer (it slides out, the camera goes down
        // to it), and back up over the table (it closes).
        let want = ((self.ui.mouse.x / w.max(1.0)) * 2.0 - 1.0).clamp(-1.0, 1.0);
        self.bench_ui.pan += (want - self.bench_ui.pan) * (crate::util::damp(4.0, self.ui.dt));
        let low = self.ui.mouse.y / py.max(1.0);

        let Some(table) = self.bench_table(p) else { return hovered };
        let ready = self.station.as_ref().is_some_and(|st| st.blend > 0.9 && !st.closing);
        let view = Screen2 { view_proj: self.me.look.view_proj, w, h };
        let (o, d) = view.ray(self.ui.mouse);
        let bench = self.level.block_entities.benches.get(&p).cloned().unwrap_or_default();
        let made = scene_at(p, &table, &bench, self.bench_time(p));
        let busy = self.bench_busy(p);
        let mut found = None;
        let mut spot = None;
        let mut drawer_spot = None;
        let drawer = self.level.bench_drawer.get(&p).copied().unwrap_or(0.0);
        if ready && !over_inventory {
            found = self.bench_pick(p, &table, &bench, &made, drawer, o, d);
            spot = hit_plane(o, d, table.center.y).map(|q| table.local(q)).filter(|&(x, z)| table.on(x, z));
            // Or in the drawer, out in front of the table: on its floor. The table's top hides
            // what is under it: over it, the mouse is on the table; the drawer only where it
            // is out in front of the table.
            drawer_spot = hit_plane(o, d, table.center.y - DRAWER_DEPTH).filter(|&q| {
                let (x, z) = table.local(q);
                spot.is_none() && drawer > 0.8 && x.abs() < table.half_w && (0.46..0.95).contains(&z)
            });
        }
        self.bench_ui.spot = spot;
        self.bench_ui.drawer_spot = drawer_spot;

        // The drawer opens and shuts with its handle (a click, below). Only with something in
        // the hand does it follow the mouse (once it has stayed there a moment): taken out of
        // the drawer and brought up over the table (the mouse on its top, or up at the top of
        // the view), it shuts and the camera goes back up; brought down toward it from the
        // table with what goes into it (the brush, rounds, a box of them), it opens again.
        let settled = self.bench_ui.focus > 0.99 || self.bench_ui.focus < 0.01;
        let holding = self.me.items.cursor.is_some() || self.bench_ui.brush;
        let for_drawer = self.bench_ui.brush || self.me.items.cursor.is_some_and(|st| BOX_AMMO.contains(&st.item) || st.item == AMMO_BOX);
        let over_table = spot.is_some() || low < DRAWER_CLOSE;
        let wants = if self.bench_ui.in_drawer {
            holding && over_table && drawer_spot.is_none()
        } else {
            for_drawer && low > DRAWER_OPEN && !over_inventory
        };
        self.bench_ui.dwell = if settled && wants && !over_inventory { self.bench_ui.dwell + self.ui.dt } else { 0.0 };
        if self.bench_ui.dwell > DRAWER_DWELL {
            self.bench_ui.in_drawer = !self.bench_ui.in_drawer;
            self.bench_ui.dwell = 0.0;
        }
        let target = if self.bench_ui.in_drawer { 1.0 } else { 0.0 };
        let step = self.ui.dt / DRAWER_GLIDE;
        // (toward it; staying put once there)
        self.bench_ui.focus = if self.bench_ui.focus < target {
            (self.bench_ui.focus + step).min(target)
        } else {
            (self.bench_ui.focus - step).max(target)
        };
        let pick = found.map(|(k, _)| k);
        let point = found.map(|(_, t)| o + d * t).or(spot.map(|(x, z)| table.at(x, z))).or(drawer_spot);
        // Where what is held on the mouse would lie (kept from the last frame while it can be).
        self.bench_ui.held_spot = match (self.me.items.cursor, spot) {
            (Some(st), Some((x, z))) if belongs_on_bench(st.item, table.rifle()) => {
                Some(free_spot_near(&table, &bench, as_laid(st), x, z, 0.0, self.bench_ui.held_spot))
            }
            _ => None,
        };
        // What the mouse is on lights up: what a click takes; with something held, only where
        // it goes on or into (green).
        let ok = self.bench_target_ok(&bench, pick);
        self.bench_ui.hover = pick.filter(|_| self.me.items.cursor.is_none() || ok);
        self.bench_ui.hover_ok = self.me.items.cursor.is_some() && ok;

        let (left, right) = (self.ui.pressed, self.ui.right_pressed);
        self.bench_ui.scrubbing = false;
        if self.bench_ui.brush {
            // The brush: where the mouse points, scrubbing what it is held down on.
            self.bench_ui.brush_at = point.map(|q| q + Vec3::Y * 0.005);
            self.bench_ui.hover = None;
            if right || (left && drawer_spot.is_some() && pick.is_none()) {
                // Put back (a right click anywhere, or a click in the drawer).
                self.bench_ui.brush = false;
                self.bench_ui.brush_at = None;
            } else if self.input.left_down {
                if let (Some(Pick::Item(id) | Pick::Mod(id, _)), false) = (pick, busy) {
                    self.scrub(p, id, point);
                }
            }
        } else if let Some(from) = self.bench_ui.drag.filter(|_| !self.input.left_down) {
            // Let go of something picked up: dropped where the mouse is (on the table, or into
            // an inventory slot), once it was dragged.
            self.bench_ui.drag = None;
            if (self.ui.mouse - from).length() > DRAG * s && !busy {
                match (hovered, pick) {
                    (Some(r), _) if over_inventory => self.click_slot(Container::GunStation(p), r, false, false),
                    (_, Some(Pick::Ammo(i))) => self.bench_box_slot(p, i as usize, false),
                    (_, Some(Pick::Loader)) => self.bench_loader_click(p, &table),
                    _ => self.bench_put(p, &table, pick, spot, false),
                }
            }
        } else if (left || right) && ready && !busy && !over_inventory {
            match (self.me.items.cursor, pick) {
                (_, Some(Pick::Ammo(i))) => self.bench_box_slot(p, i as usize, right),
                (_, Some(Pick::Loader)) => self.bench_loader_click(p, &table),
                (_, Some(Pick::Handle)) => {
                    self.bench_ui.in_drawer = !self.bench_ui.in_drawer;
                    self.bench_ui.dwell = 0.0;
                }
                // Looking into the drawer, a click beside it (on nothing) shuts it.
                (None, None) if self.bench_ui.in_drawer && drawer_spot.is_none() && spot.is_none() => {
                    self.bench_ui.in_drawer = false;
                    self.bench_ui.dwell = 0.0;
                }
                (Some(_), _) => self.bench_put(p, &table, pick, spot, right),
                (None, Some(Pick::Brush)) if left => self.bench_ui.brush = true,
                (None, Some(Pick::Item(id))) if left => self.bench_pick_up(p, id),
                (None, Some(Pick::Mod(id, bit))) if left => self.bench_unfit(p, &table, id, bit),
                (None, Some(Pick::Mag(id))) if left => self.bench_mag_out(p, &table, id),
                (None, Some(Pick::Mag(id) | Pick::Item(id))) => self.bench_right_click(p, &table, id),
                _ => {}
            }
        }
        // Not scrubbing any more: what is left goes to the others.
        if !self.bench_ui.scrubbing && self.bench_ui.scrub_dirty {
            self.bench_ui.scrub_dirty = false;
            self.bench_changed(p, None);
        }

        let far = table.right * (table.wide - 1.0);
        let min = p.as_vec3().min(p.as_vec3() + far);
        let over_block = ray_box(o, d, min, min + Vec3::new(1.0, 1.0, 1.0) + far.abs(), 64.0).is_some();
        self.inv_ui.station_inside = over_inventory || over_block || pick.is_some() || spot.is_some() || holding_box;
        self.inv_ui.station_hover = None;
        // A glow on the table under what the mouse is on, or where what is held would go.
        self.inv_ui.station_frame = match (self.bench_ui.hover, self.me.items.cursor, spot) {
            (Some(h), _, _) => glow_under(&table, &made.0, &made.1, h),
            (None, Some(st), Some(_)) if !self.bench_ui.brush && belongs_on_bench(st.item, table.rifle()) => {
                let (x, z) = self.bench_ui.held_spot.unwrap_or((0.0, 0.0));
                let it = BenchItem { id: 0, stack: as_laid(st), x, z, turn: 0.0 };
                match lying_pieces(&table, &it) {
                    Some(pcs) => glow_of(&table, pcs.iter()),
                    None => Some(table.square(x, z, 0.12)),
                }
            }
            _ => None,
        };
        hovered
    }
}
