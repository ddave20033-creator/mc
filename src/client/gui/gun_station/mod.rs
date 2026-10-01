//! The gun station: a bench two blocks wide. Opened, the camera glides over its table (as over
//! a crafting table) and sways a little left and right with the mouse; its drawer slides out,
//! the cleaning brush in it. There is nothing on the screen but the inventory along the
//! bottom: everything happens on the table, in 3D.
//!
//! - Anything from the inventory is laid on the table where the mouse points, and picked up
//!   again (or dragged somewhere else) with the mouse.
//! - A right click on a gun takes it apart there: it comes apart the way a real one does
//!   (the pistol's "strip" animation) and its parts lie beside each other.
//! - A right click on a part puts a gun together from the parts on the table (a frame, a
//!   barrel, a recoil spring and a slide; a magazine if there is one): they fly to the middle
//!   and go together.
//! - An attachment dragged onto a gun goes on it ("fit_*" animations); clicking one on a gun
//!   takes it off and lays it beside the gun.
//! - The brush from the drawer scrubs whatever it is held down on: a part clean quickly, a
//!   whole gun slowly. Dirt shows on the guns and parts themselves (`pistol_view::dirt_level`).
//!
//! What lies on the table is the station's block entity (`entity::GunBench`): saved with the
//! world and the same for every player; each change goes to the others (`Msg::Bench`), with
//! the animation everyone plays (`BenchEvent`). What may lie there and what goes on or into
//! what are the station's rules (`item::gun_bench`).
//!
//! - `table`: the table top's geometry; `pieces`: the guns' models posed lying on it;
//!   `layout`: where things lie (each in its own place, all on the table).
//! - `anim`: what lies there as it is drawn now, with the last change's animation.
//! - `draw`: the stations drawn in the world; `pick`: what the mouse is on.
//! - `screen`: the open station each frame (the inventory, the drawer, the mouse);
//!   `actions`: what a click does there; `grenade_crate`: the rifle station's crate.

mod actions;
mod anim;
mod draw;
mod grenade_crate;
mod layout;
mod pick;
mod pieces;
mod screen;
mod table;
#[cfg(test)]
mod tests;

use crate::client::{Container, Game, Screen};
use crate::entity::{BenchEvent, GunBench};
use crate::item::AMMO_BOX;
use glam::{IVec3, Vec2, Vec3};

use anim::event_length;
use layout::free_spot;
use pick::Pick;
use table::Table;

/// At the open gun station: the brush, the mouse on its table and in its drawer, the camera,
/// scrubbing, and what was last sent.
pub(in crate::client) struct BenchUi {
    /// Holding its brush, and where it is.
    brush: bool,
    brush_at: Option<Vec3>,
    /// Where on the open gun station's table the mouse points (x, z), if it does.
    pub(super) spot: Option<(f32, f32)>,
    /// Where what is held on the mouse would lie on the open gun station's table.
    held_spot: Option<(f32, f32)>,
    /// Where in the open drawer the mouse points (on its floor), if it does.
    pub(super) drawer_spot: Option<Vec3>,
    /// Where what is held on the mouse shows over the open gun station (world), for the
    /// others to see it there too.
    pub(in crate::client) hold_at: Option<Vec3>,
    /// The camera's sway with the mouse (-1 .. 1).
    pub(super) pan: f32,
    /// Looking into its drawer (the mouse went down to it), and how far the camera has gone
    /// down to it (0 over the table .. 1 over the drawer).
    pub(in crate::client) in_drawer: bool,
    pub(super) focus: f32,
    /// How long the mouse has stayed where it opens (or closes) the drawer.
    dwell: f32,
    /// Something picked up off the table: where the mouse was, to drag it.
    drag: Option<Vec2>,
    /// What the mouse points at there, and whether that is where what is held goes (it
    /// lights green).
    hover: Option<Pick>,
    hover_ok: bool,
    /// Scrubbing now; the dirt scrubbed off not yet taken off, and whether that is not sent
    /// yet; when the table was last sent.
    scrubbing: bool,
    scrub: f32,
    scrub_dirty: bool,
    sent: f32,
}

impl BenchUi {
    pub(in crate::client) fn new() -> Self {
        Self {
            brush: false,
            brush_at: None,
            spot: None,
            held_spot: None,
            drawer_spot: None,
            hold_at: None,
            pan: 0.0,
            in_drawer: false,
            focus: 0.0,
            dwell: 0.0,
            drag: None,
            hover: None,
            hover_ok: false,
            scrubbing: false,
            scrub: 0.0,
            scrub_dirty: false,
            sent: 0.0,
        }
    }
}

impl Game {
    /// The table of the gun station whose left half is `p`.
    pub(in crate::client::gui) fn bench_table(&self, p: IVec3) -> Option<Table> {
        Table::of(p, self.terrain.world.geti(p))
    }

    /// Seconds into the animation of the last change on the table at `p` (when one was seen).
    fn bench_time(&self, p: IVec3) -> Option<f32> {
        let serial = self.level.block_entities.benches.get(&p)?.event.serial;
        self.level.bench_anims.get(&p).filter(|a| a.0 == serial).map(|a| self.clock.time - a.1)
    }

    /// Whether something on the table at `p` is still moving (its things wait until then).
    fn bench_busy(&self, p: IVec3) -> bool {
        let Some(b) = self.level.block_entities.benches.get(&p) else { return false };
        self.bench_time(p).is_some_and(|t| t < event_length(&b.event))
    }

    /// A table's contents came from the others (or were loaded): a new change there plays
    /// from now.
    pub(in crate::client) fn set_bench(&mut self, p: IVec3, bench: GunBench) {
        let serial = bench.event.serial;
        if serial != 0 && self.level.bench_anims.get(&p).is_none_or(|a| a.0 != serial) {
            self.level.bench_anims.insert(p, (serial, self.clock.time));
        }
        self.level.block_entities.benches.insert(p, bench);
    }

    /// Something on the table at `p` changed here: with `event`, what happened (everyone
    /// plays it); it goes to the others.
    fn bench_changed(&mut self, p: IVec3, event: Option<BenchEvent>) {
        let now = self.clock.time;
        let b = self.level.block_entities.benches.entry(p).or_default();
        if let Some(mut e) = event {
            e.serial = b.event.serial.wrapping_add(1).max(1);
            b.event = e;
            self.level.bench_anims.insert(p, (b.event.serial, now));
        }
        let msg = crate::net::Msg::Bench { p, bench: b.clone() };
        self.bench_ui.sent = now;
        self.send(msg);
    }

    /// Where the brush is in this player's hand, for the others to see.
    pub(in crate::client) fn bench_brush_pose(&self) -> Option<Vec3> {
        match self.screen {
            Screen::Container(Container::GunStation(_)) if self.bench_ui.brush => self.bench_ui.brush_at,
            _ => None,
        }
    }

    /// Opens the gun station whose left half is `p`: the camera glides over its table.
    pub(in crate::client) fn open_gun_station(&mut self, p: IVec3) {
        self.bench_ui.brush = false;
        self.bench_ui.brush_at = None;
        self.bench_ui.drag = None;
        self.bench_ui.pan = 0.0;
        self.bench_ui.in_drawer = false;
        self.bench_ui.focus = 0.0;
        self.open_container(Container::GunStation(p));
    }

    /// Closing the gun station: the brush goes back into the drawer.
    pub(in crate::client) fn close_gun_station(&mut self) {
        self.bench_ui.hold_at = None;
        // A box of rounds still on the mouse goes back into the drawer (or onto the table).
        if let (Screen::Container(Container::GunStation(p)), Some(st)) = (self.screen, self.me.items.cursor) {
            if st.item == AMMO_BOX {
                self.me.items.cursor = None;
                let table = self.bench_table(p);
                let bench = self.level.block_entities.benches.entry(p).or_default();
                match bench.boxes.iter().position(|b| b.is_none()) {
                    Some(i) => bench.boxes[i] = Some(st.data),
                    None => {
                        if let Some(t) = table {
                            let (x, z) = free_spot(&t, bench, st, 0.0, 0.0, 0.0);
                            bench.add(st, x, z, 0.0);
                        }
                    }
                }
                self.bench_changed(p, None);
            }
        }
        self.bench_ui.spot = None;
        self.bench_ui.drawer_spot = None;
        self.bench_ui.in_drawer = false;
        self.bench_ui.brush = false;
        self.bench_ui.brush_at = None;
        self.bench_ui.drag = None;
        self.bench_ui.hover = None;
    }
}
