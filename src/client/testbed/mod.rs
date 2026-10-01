//! `rustcraft --test <script> [folder]`: runs a test script (a built-in one by name, or a
//! file) that sets up a world, moves the player, holds keys, opens screens, takes pictures
//! and runs checks (surfaces fighting over the same place, missing textures, flicker), and
//! writes `report.md` with everything it saw next to the pictures. Nothing touches the real
//! saves or settings. The commands are in `testbed/README.md`.

mod checks;
mod script;

use crate::client::{Container, Game, Screen};
use crate::item::{GunKind, Stack};
use crate::app::lang::t;
use crate::save::WorldMeta;
use crate::app::settings::Settings;
use crate::world::*;
use crate::world::gen::SEA;
use glam::{IVec3, Mat4, Vec2, Vec3};
use std::fmt::Write as _;
use std::path::PathBuf;
use winit::keyboard::KeyCode;

use script::{Cmd, Origin, WorldKind};

/// The built-in scripts (`testbed/*.txt`), by name.
pub const BUILT_IN: &[(&str, &str)] = &[
    ("menus", include_str!("../../../testbed/menus.txt")),
    ("guns", include_str!("../../../testbed/guns.txt")),
    ("buckets", include_str!("../../../testbed/buckets.txt")),
    ("trees", include_str!("../../../testbed/trees.txt")),
    ("checks", include_str!("../../../testbed/checks.txt")),
    ("blocks", include_str!("../../../testbed/blocks.txt")),
    ("felling", include_str!("../../../testbed/felling.txt")),
    ("mobs", include_str!("../../../testbed/mobs.txt")),
    ("book", include_str!("../../../testbed/book.txt")),
    ("station", include_str!("../../../testbed/station.txt")),
];

/// The runs that drive the game themselves (no settings saved, the mouse not grabbed, going
/// on in the background).
pub(super) struct TestModes {
    /// `--bench` mode state.
    pub(super) bench: Option<super::bench::Bench>,
    /// `--aa-shots`: anti-aliasing comparison pictures (runs in bench mode).
    pub(super) shots: Option<super::bench::Shots>,
    /// `--test`: a test script running.
    pub(super) testbed: Option<Testbed>,
    /// A test asked for the menus' backdrop sharp.
    pub(super) no_blur: bool,
}

impl TestModes {
    /// A bench, picture or test run is going on.
    pub(super) fn any(&self) -> bool {
        self.bench.is_some() || self.testbed.is_some()
    }
}

pub struct Testbed {
    name: String,
    dir: PathBuf,
    cmds: Vec<Cmd>,
    next: usize,
    /// Seconds (or frames) before the next command.
    wait: f32,
    frames: u32,
    /// The world is being made ready: waiting for it to load and settle.
    loading: Option<f32>,
    /// Where positions are measured from.
    origin: Vec3,
    /// Keys held and until when; mouse buttons held; the view turning (degrees a second,
    /// until when).
    held: Vec<(KeyCode, f32)>,
    buttons: [Option<f32>; 2],
    turning: Option<(f32, f32)>,
    time_of_day: Option<f32>,
    clock: f32,
    report: String,
    shots: Vec<(String, String)>,
    problems: Vec<String>,
    started: std::time::Instant,
    /// Blocks set by the script, for the world's server (`Msg::Edit`).
    edits: Vec<(IVec3, crate::world::block::Block)>,
}

impl Testbed {
    /// The script `what` (a built-in name or a file), its pictures and report going to `dir`
    /// (default `test-out/<name>`).
    pub fn new(what: &str, dir: Option<PathBuf>) -> Testbed {
        let (name, text) = match BUILT_IN.iter().find(|(n, _)| *n == what) {
            Some((n, t)) => (n.to_string(), t.to_string()),
            None => (
                std::path::Path::new(what).file_stem().map_or("test".into(), |s| s.to_string_lossy().into_owned()),
                std::fs::read_to_string(what).unwrap_or_else(|e| format!("echo cannot read {what}: {e}\nquit")),
            ),
        };
        let dir = dir.unwrap_or_else(|| PathBuf::from("test-out").join(&name));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let (mut cmds, errors) = script::parse(&text);
        if !matches!(cmds.first(), Some(Cmd::World(_))) {
            cmds.insert(0, Cmd::World(WorldKind::Bench));
        }
        Testbed {
            name,
            dir,
            cmds,
            next: 0,
            wait: 0.0,
            frames: 0,
            loading: None,
            origin: Vec3::ZERO,
            held: Vec::new(),
            buttons: [None; 2],
            turning: None,
            time_of_day: None,
            clock: 0.0,
            report: String::new(),
            shots: Vec::new(),
            problems: errors,
            started: std::time::Instant::now(),
            edits: Vec::new(),
        }
    }

    fn log(&mut self, line: impl AsRef<str>) {
        let _ = writeln!(self.report, "- `{:6.2}s` {}", self.clock, line.as_ref());
    }
}

impl Game {
    /// Every frame of a test: the held keys, buttons and turning, then the script's next
    /// commands once the last one's time is up.
    pub(super) fn testbed_step(&mut self, dt: f32) {
        let Some(tb) = self.test.testbed.as_mut() else { return };
        if self.boot.is_some() {
            return;
        }
        tb.clock += dt;
        let now = tb.clock;
        if let Some(t) = tb.time_of_day {
            self.level.time_of_day = t;
        }
        // Held keys and buttons.
        tb.held.retain(|&(_, until)| until > now);
        let held: Vec<KeyCode> = tb.held.iter().map(|h| h.0).collect();
        self.input.keys.retain(|k| held.contains(k));
        for k in held {
            self.input.keys.insert(k);
        }
        let tb = self.test.testbed.as_mut().unwrap();
        for (i, b) in tb.buttons.iter_mut().enumerate() {
            let down = b.is_some_and(|until| until > now);
            if !down {
                *b = None;
            }
            if i == 0 {
                self.input.left_down = down;
            } else {
                self.input.right_down = down;
            }
        }
        if let Some((rate, until)) = tb.turning {
            if now < until {
                self.me.look.yaw += rate.to_radians() * dt;
            } else {
                tb.turning = None;
            }
        }
        self.input.mouse_delta = Vec2::ZERO;
        // A world being made ready: loaded, entered, and the chunks around drawn.
        if let Some(since) = tb.loading {
            let ready = self.screen == Screen::Playing && self.gfx.renderer.pending() == 0 && self.gfx.renderer.chunk_count() > 60;
            if (ready && now - since > 1.5) || now - since > 25.0 {
                let tb = self.test.testbed.as_mut().unwrap();
                tb.loading = None;
                tb.origin = self.me.body.pos;
                let p = self.me.body.pos;
                tb.log(format!("world ready at {:.1} {:.1} {:.1}", p.x, p.y, p.z));
            }
            return;
        }
        let tb = self.test.testbed.as_mut().unwrap();
        if tb.frames > 0 {
            tb.frames -= 1;
            return;
        }
        if tb.wait > 0.0 {
            tb.wait -= dt;
            return;
        }
        // Commands run until one takes time.
        loop {
            let tb = self.test.testbed.as_mut().unwrap();
            let Some(cmd) = tb.cmds.get(tb.next).cloned() else {
                self.testbed_finish();
                return;
            };
            tb.next += 1;
            let wait = self.testbed_run(cmd);
            // The blocks the command set go to the world's server.
            let edits = self.test.testbed.as_mut().map(|tb| std::mem::take(&mut tb.edits)).unwrap_or_default();
            if !edits.is_empty() {
                self.send(crate::net::Msg::Edit(edits));
            }
            if wait {
                return;
            }
        }
    }

    /// A block the script sets: here at once, and on the world's server (which takes it from
    /// the world's owner).
    fn test_set(&mut self, p: IVec3, b: Block) {
        self.set_block(p, b);
        if let Some(tb) = self.test.testbed.as_mut() {
            tb.edits.push((p, b));
        }
    }

    /// Runs one command; true if the next has to wait (for time, a frame, or the world).
    fn testbed_run(&mut self, cmd: Cmd) -> bool {
        let tb = self.test.testbed.as_mut().unwrap();
        let origin = tb.origin;
        let at = |o: Origin, v: [f32; 3]| match o {
            Origin::Rel => origin + Vec3::from(v),
            Origin::Abs => Vec3::from(v),
        };
        match cmd {
            Cmd::World(kind) => {
                tb.log(format!("world {kind:?}"));
                match kind {
                    WorldKind::Menu => {
                        self.screen = Screen::MainMenu;
                        return false;
                    }
                    WorldKind::Bench => self.play_world(test_world(None)),
                    WorldKind::New(seed) => self.play_world(test_world(Some(seed))),
                }
                self.test.testbed.as_mut().unwrap().loading = Some(self.test.testbed.as_ref().unwrap().clock);
                return true;
            }
            Cmd::Window(w, h) => {
                let _ = self.gfx.window.request_inner_size(winit::dpi::PhysicalSize::new(w, h));
                tb.log(format!("window {w}x{h}"));
                tb.frames = 3;
                return true;
            }
            Cmd::Lane => {
                let o = self.testbed_lane(origin);
                let tb = self.test.testbed.as_mut().unwrap();
                tb.origin = o;
                tb.log(format!("lane from {:.1} {:.1} {:.1}", o.x, o.y, o.z));
                self.me.body.pos = o;
                self.me.body.start_tick();
                self.me.body.vel = Vec3::ZERO;
            }
            Cmd::Clear(r, h) => {
                let b = origin.floor().as_ivec3();
                for y in 0..h {
                    for z in -r..=r {
                        for x in -r..=r {
                            self.test_set(b + IVec3::new(x, y, z), AIR);
                        }
                    }
                }
            }
            Cmd::Time(t) => tb.time_of_day = Some(t),
            Cmd::Pos(o, v) => {
                self.me.body.pos = at(o, v);
                self.me.body.start_tick();
                self.me.body.vel = Vec3::ZERO;
            }
            Cmd::Look(yaw, pitch) => {
                self.me.look.yaw = yaw.to_radians();
                self.me.look.body_yaw = self.me.look.yaw;
                self.me.look.pitch = pitch.to_radians();
            }
            Cmd::Fly(on) => self.me.body.flying = on,
            Cmd::Camera(m) => self.me.look.camera.mode = m,
            Cmd::Hold(name, count, loaded) => match crate::item::from_key(&name) {
                Some(id) => {
                    let mut st = Stack::new(id, count.clamp(1, 64) as u8);
                    if loaded {
                        if let Some(k) = GunKind::of(id) {
                            let n = k.magazine_size(crate::item::gun_mods(&st));
                            crate::item::set_gun_rounds(&mut st, n);
                        } else if let Some(n) = crate::item::magazine_capacity(id) {
                            crate::item::set_gun_rounds(&mut st, n);
                        }
                    }
                    *self.me.items.held_slot_mut() = Some(st);
                }
                None => tb.problems.push(format!("hold: no item `{name}`")),
            },
            Cmd::Slot(s) => self.me.items.hotbar_slot = s,
            Cmd::Empty => {
                for s in self.me.items.inventory.slots.iter_mut() {
                    *s = None;
                }
                self.me.items.hotbar_slot = 0;
            }
            Cmd::Command(line) => {
                let line = format!("/{line}");
                tb.log(line.clone());
                self.run_command(&line);
            }
            Cmd::Place(o, v, name) => match block_named(&name) {
                Some(b) => {
                    let p = match o {
                        Origin::Rel => origin.floor().as_ivec3() + IVec3::from(v),
                        Origin::Abs => IVec3::from(v),
                    };
                    self.test_set(p, b);
                }
                None => tb.problems.push(format!("place: no block `{name}`")),
            },
            Cmd::Fill(a, b, name) => match block_named(&name) {
                Some(blk) => {
                    let o = origin.floor().as_ivec3();
                    let (lo, hi) = (IVec3::from(a).min(IVec3::from(b)), IVec3::from(a).max(IVec3::from(b)));
                    for y in lo.y..=hi.y {
                        for z in lo.z..=hi.z {
                            for x in lo.x..=hi.x {
                                self.test_set(o + IVec3::new(x, y, z), blk);
                            }
                        }
                    }
                }
                None => tb.problems.push(format!("fill: no block `{name}`")),
            },
            Cmd::Tree(kind, x, z, seed) => {
                let log = match kind.as_str() {
                    "birch" => BIRCH_LOG,
                    "spruce" => SPRUCE_LOG,
                    _ => OAK_LOG,
                };
                let base = origin.floor().as_ivec3() + IVec3::new(x, 0, z);
                for (d, b, soft) in crate::world::trees::tree_shape(log, seed) {
                    let q = base + d;
                    if !soft || self.terrain.world.geti(q) == AIR {
                        self.test_set(q, b);
                    }
                }
            }
            Cmd::Drop(name, v) => match crate::item::from_key(&name) {
                // (the server puts it there: near enough, or the world's owner's)
                Some(id) => {
                    let pos = origin + Vec3::from(v);
                    self.send(crate::net::Msg::DropItem { pos, vel: Vec3::ZERO, stack: Stack::one(id), delay: 1000.0 });
                }
                None => tb.problems.push(format!("drop: no item `{name}`")),
            },
            Cmd::Key(name, secs) => match key_named(&self.settings, &name) {
                Some(k) => {
                    let until = tb.clock + secs;
                    tb.held.push((k, until));
                    self.key_pressed(k);
                }
                None => tb.problems.push(format!("key: no key `{name}`")),
            },
            Cmd::Press(name) => match key_named(&self.settings, &name) {
                Some(k) => self.key_pressed(k),
                None => tb.problems.push(format!("press: no key `{name}`")),
            },
            Cmd::Click(left, secs) => {
                let i = if left { 0 } else { 1 };
                tb.buttons[i] = Some(tb.clock + secs.max(0.02));
                if left {
                    self.input.left_pressed = true;
                    self.input.left_down = true;
                } else {
                    self.input.right_pressed = true;
                    self.input.right_down = true;
                }
            }
            Cmd::Mouse(fx, fy) => {
                let p = Vec2::new(self.ui.w * fx, self.ui.h * fy);
                self.ui.set_mouse(p);
            }
            Cmd::UiKey(name) => match key_named(&self.settings, &name) {
                Some(k) => {
                    self.ui.nav_key(k, false);
                }
                None => tb.problems.push(format!("uikey: no key `{name}`")),
            },
            Cmd::Screen(name) => {
                tb.log(format!("screen {name}"));
                if !self.testbed_screen(&name) {
                    self.test.testbed.as_mut().unwrap().problems.push(format!("screen: unknown `{name}`"));
                }
                self.set_grab(false);
            }
            Cmd::Turn(rate, secs) => tb.turning = Some((rate, tb.clock + secs)),
            Cmd::Set(what, value) => {
                let on = matches!(value.as_str(), "on" | "true" | "1");
                match what.as_str() {
                    "body" => self.settings.first_person_body = on,
                    "blur" => self.test.no_blur = !on,
                    "hud" => self.hud.hide = !on,
                    "fov" => self.settings.fov = value.parse().unwrap_or(self.settings.fov),
                    "gui" => self.settings.gui_scale = value.parse().unwrap_or(self.settings.gui_scale),
                    "view" => self.settings.render_distance = value.parse().unwrap_or(self.settings.render_distance),
                    "debug" => self.hud.debug = on,
                    "dark" => self.settings.dark_ui = on,
                    "lang" => {
                        self.settings.hungarian = value == "hu";
                        crate::app::lang::set_hungarian(self.settings.hungarian);
                    }
                    _ => tb.problems.push(format!("set: unknown `{what}`")),
                }
            }
            Cmd::Wait(s) => {
                tb.wait = s;
                return true;
            }
            Cmd::Frames(n) => {
                tb.frames = n;
                return true;
            }
            Cmd::Shot(name) => {
                let file = format!("{name}.png");
                self.gfx.gpu.capture = Some(tb.dir.join(&file));
                let what = format!(
                    "screen {:?}, at {:.1} {:.1} {:.1}, looking {:.0}° {:.0}°, camera {}",
                    self.screen,
                    self.me.body.pos.x,
                    self.me.body.pos.y,
                    self.me.body.pos.z,
                    self.me.look.yaw.to_degrees(),
                    self.me.look.pitch.to_degrees(),
                    self.me.look.camera.mode
                );
                tb.log(format!("shot `{file}`"));
                tb.shots.push((file, what));
                // (one picture a frame)
                tb.frames = 1;
                return true;
            }
            Cmd::Jitter(on) => {
                // A hair: the view turned a thousandth of a degree and moved a thousandth of a
                // block — nothing that is drawn right changes.
                let k = if on { 1.0 } else { -1.0 };
                self.me.look.yaw += 0.00002 * k;
                self.me.body.pos.x += 0.001 * k;
            }
            Cmd::Compare(a, b, out) => {
                let dir = tb.dir.clone();
                let file = format!("{out}.png");
                match checks::flicker_diff(&dir.join(format!("{a}.png")), &dir.join(format!("{b}.png")), &dir.join(&file)) {
                    Some(share) => {
                        let verdict = if share > 0.002 { "FLICKERS" } else { "steady" };
                        tb.log(format!("flicker `{out}`: {:.3}% of the pixels changed: {verdict} (see `{file}`)", share * 100.0));
                        if share > 0.002 {
                            tb.problems.push(format!("flicker in `{out}`: {:.3}% of the pixels", share * 100.0));
                        }
                        tb.shots.push((file, "the pixels that flickered (red)".into()));
                    }
                    None => tb.problems.push(format!("flicker `{out}`: the pictures could not be read")),
                }
            }
            Cmd::Check(what, radius) => self.testbed_check(&what, radius),
            Cmd::PickMap(name) => {
                if let Screen::Container(Container::GunStation(p)) = self.screen {
                    let file = format!("{name}.png");
                    let path = tb.dir.join(&file);
                    tb.shots.push((file, "the gun station's click map".into()));
                    self.bench_pick_map(p, &path);
                } else {
                    tb.problems.push("pickmap: no gun station open".into());
                }
            }
            Cmd::Echo(text) => tb.log(format!("**{text}**")),
            Cmd::Stats => {
                let mb = |b: u64| b / (1 << 20);
                let (pages, bytes, used, retired) = self.gfx.renderer.mesh_memory();
                let vram = self.gfx.gpu.vram_usage().map_or("?".into(), |(u, b)| format!("{}/{} MB", mb(u), mb(b)));
                let ram = self.clock.sys_stats.get().ram_game;
                let w = &self.terrain.world;
                let blocks: usize = w.chunks.values().map(|c| c.memory()).sum();
                let light: usize = w.light.values().map(|l| l.data.len()).sum();
                tb.log(format!("memory: blocks {} MB, light {} MB", mb(blocks as u64), mb(light as u64)));
                tb.log(format!(
                    "stats: {:?} at {:.1} {:.1} {:.1}; {:.0} fps, {:.1} ms; chunks {} loaded, {} meshed, {} waiting; mesh memory {} pages {} MB ({} MB used), {} to free; VRAM {vram}; RAM {} MB",
                    self.screen,
                    self.me.body.pos.x,
                    self.me.body.pos.y,
                    self.me.body.pos.z,
                    self.clock.fps,
                    self.clock.frame_times.back().copied().unwrap_or(0.0),
                    self.terrain.world.chunks.len(),
                    self.gfx.renderer.chunk_count(),
                    self.gfx.renderer.pending(),
                    pages,
                    mb(bytes),
                    mb(used),
                    retired,
                    mb(ram),
                ))
            }
            Cmd::Lan(args) => {
                for line in self.testbed_lan(&args) {
                    self.test.testbed.as_mut().unwrap().log(line);
                }
            }
            Cmd::Quit => {
                tb.next = tb.cmds.len();
            }
        }
        false
    }

    /// Opens a screen by name, the way the game would. False if there is none by that name.
    fn testbed_screen(&mut self, name: &str) -> bool {
        self.screen = match name {
            "main" => Screen::MainMenu,
            "worlds" | "delete" => {
                self.menus.worlds = crate::save::list_worlds();
                self.menus.selected_world = (!self.menus.worlds.is_empty()).then_some(0);
                if name == "delete" && self.menus.selected_world.is_some() {
                    Screen::DeleteWorld
                } else {
                    Screen::SelectWorld
                }
            }
            "create" => {
                self.menus.create_name = t("create.default_name").to_string();
                Screen::CreateWorld
            }
            "pause" => Screen::Paused,
            "options" => Screen::Options { in_game: false },
            "options_game" => Screen::Options { in_game: true },
            "keys" => Screen::KeyBinds { in_game: false },
            "packs" => Screen::ResourcePacks { in_game: false },
            "credits" => Screen::Credits,
            "multi" => Screen::Multiplayer,
            "skin" => Screen::Skin,
            "dead" => {
                self.me.vitals.death_message = "Steve fell from a high place".into();
                Screen::Dead
            }
            "inventory" => Screen::Container(Container::Inventory),
            "creative" => Screen::Container(Container::Creative),
            "playing" => Screen::Playing,
            _ => return false,
        };
        true
    }

    /// `lane`: a flat stone lane east of `around` (cleared above), a post at its far end.
    /// Returns where it starts (feet).
    fn testbed_lane(&mut self, around: Vec3) -> Vec3 {
        let (x0, z0) = (around.x.floor() as i32, around.z.floor() as i32);
        let w = &self.terrain.world;
        let mut ground = SEA;
        for y in (1..HEIGHT as i32 - 8).rev() {
            if is_solid(w.get(x0, y, z0)) {
                ground = y;
                break;
            }
        }
        for x in x0 - 3..x0 + 42 {
            for z in z0 - 7..=z0 + 7 {
                self.test_set(IVec3::new(x, ground, z), STONE);
                for y in ground + 1..ground + 7 {
                    self.test_set(IVec3::new(x, y, z), AIR);
                }
            }
        }
        for y in ground + 1..ground + 4 {
            for z in z0 - 1..=z0 + 1 {
                self.test_set(IVec3::new(x0 + 40, y, z), PLANKS);
            }
        }
        Vec3::new(x0 as f32 + 0.5, ground as f32 + 1.0, z0 as f32 + 0.5)
    }

    /// `check zfight [chunks]`: the chunks around the origin meshed again, looking for
    /// surfaces in the same place; `check models`: the same in every item's model;
    /// `check textures`: textures blocks or items use that are empty.
    fn testbed_check(&mut self, what: &str, radius: i32) {
        let mut lines = Vec::new();
        let mut bad = 0usize;
        match what {
            "zfight" => {
                let origin = self.test.testbed.as_ref().unwrap().origin;
                let c = ((origin.x / 16.0).floor() as i32, (origin.z / 16.0).floor() as i32);
                let mut chunks = 0;
                for dz in -radius..=radius {
                    for dx in -radius..=radius {
                        let pos = (c.0 + dx, c.1 + dz);
                        let Some(nb) = self.terrain.neighborhood(pos) else { continue };
                        let notches = self.terrain.world.notches_near(pos);
                        let mesh = crate::world::mesh::mesh_chunk(pos, &nb, &[], &notches, &self.terrain.gen);
                        chunks += 1;
                        let (n, found) = checks::coplanar_overlaps_indexed(&mesh.vertices, &mesh.indices, 1);
                        bad += n;
                        if let Some(f) = found.first() {
                            lines.push(format!("chunk {pos:?}: {n} pairs, e.g. at {:.2} {:.2} {:.2}: {}", f.at.x, f.at.y, f.at.z, f.what));
                        }
                    }
                }
                lines.insert(0, format!("{chunks} chunks meshed, {bad} overlapping triangle pairs"));
            }
            "models" => {
                let mut items = 0;
                for id in crate::item::all_items() {
                    items += 1;
                    let mut verts = Vec::new();
                    crate::model::emit_held_data(&mut verts, Mat4::IDENTITY, &Stack::one(id), [255; 4], crate::world::mesh::flags::ENTITY);
                    let (n, found) = checks::coplanar_overlaps(&verts, 1);
                    if let (true, Some(f)) = (n > 0, found.first()) {
                        bad += n;
                        lines.push(format!("`{}`: {n} pairs, e.g. at {:.3} {:.3} {:.3}: {}", crate::item::key(id), f.at.x, f.at.y, f.at.z, f.what));
                    }
                }
                lines.insert(0, format!("{items} item models, {bad} overlapping triangle pairs"));
            }
            "textures" => {
                let layer_bytes = crate::world::textures::TILE * crate::world::textures::TILE * 4;
                let base = &self.gfx.texture_base;
                let empty = |l: u32| {
                    base.get(l as usize * layer_bytes..(l as usize + 1) * layer_bytes)
                        .is_none_or(|px| px.chunks_exact(4).all(|p| p[3] == 0))
                };
                let mut used: Vec<(u32, String)> = Vec::new();
                for b in 1..BLOCK_IDS as Block {
                    let Some(id) = crate::item::item_of_block(b) else { continue };
                    for face in 0..6 {
                        used.push((crate::world::face_texture(b, face), format!("block {b} ({}) face {face}", crate::item::key(id))));
                    }
                }
                for id in crate::item::all_items().into_iter().filter(|&id| id >= 256) {
                    if let crate::item::Icon::Flat(l) = crate::item::icon(id) {
                        used.push((l, format!("item `{}`", crate::item::key(id))));
                    }
                }
                let checked = used.len();
                for (l, who) in used {
                    if empty(l) {
                        bad += 1;
                        lines.push(format!("layer {l} is empty: {who}"));
                    }
                }
                lines.insert(0, format!("{checked} textures in use checked, {bad} empty"));
            }
            other => {
                self.test.testbed.as_mut().unwrap().problems.push(format!("check: unknown `{other}`"));
                return;
            }
        }
        let tb = self.test.testbed.as_mut().unwrap();
        tb.log(format!("check {what}: {}", lines[0]));
        for l in lines.iter().skip(1).take(40) {
            let _ = writeln!(tb.report, "    - {l}");
        }
        if bad > 0 {
            tb.problems.push(format!("check {what}: {}", lines[0]));
        }
    }

    /// The script is done: writes the report and quits.
    fn testbed_finish(&mut self) {
        let Some(tb) = self.test.testbed.as_ref() else { return };
        let mut md = String::new();
        let _ = writeln!(md, "# Test `{}`\n", tb.name);
        let _ = writeln!(md, "Ran {:.1} s (game time {:.1} s). Pictures and this report in `{}`.\n", tb.started.elapsed().as_secs_f32(), tb.clock, tb.dir.display());
        let _ = writeln!(md, "## Problems\n");
        if tb.problems.is_empty() {
            let _ = writeln!(md, "None.\n");
        }
        for p in &tb.problems {
            let _ = writeln!(md, "- {p}");
        }
        let _ = writeln!(md, "\n## Pictures\n");
        for (file, what) in &tb.shots {
            let _ = writeln!(md, "- `{file}`: {what}");
        }
        let _ = writeln!(md, "\n## Steps\n\n{}", tb.report);
        let path = tb.dir.join("report.md");
        let _ = std::fs::write(&path, md);
        println!("test {} done: {} pictures, {} problems; {}", tb.name, tb.shots.len(), tb.problems.len(), path.display());
        self.quit = true;
    }
}

/// A world for the test in the temp folder: a copy of the last played world (`None`), or a
/// new one from `seed`. Creative, cheats on, at noon.
fn test_world(seed: Option<u32>) -> WorldMeta {
    let folder = std::env::temp_dir().join(format!("yourworlds-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    let _ = std::fs::create_dir_all(&folder);
    let latest = if seed.is_none() { crate::save::list_worlds().into_iter().next() } else { None };
    if let Some(w) = &latest {
        let src = std::path::Path::new("saves").join(&w.folder);
        for e in std::fs::read_dir(src).into_iter().flatten().flatten() {
            let _ = std::fs::copy(e.path(), folder.join(e.file_name()));
        }
    }
    let fresh = WorldMeta {
        folder: String::new(),
        name: String::new(),
        seed: seed.unwrap_or(12345),
        creative: true,
        spectator: false,
        cheats: true,
        last_played: 0,
        time_of_day: 0.25,
        spawn: None,
        bed: None,
        player: None,
    };
    WorldMeta {
        folder: folder.to_string_lossy().into_owned(),
        name: "test".into(),
        creative: true,
        cheats: true,
        last_played: 0,
        time_of_day: 0.25,
        ..latest.unwrap_or(fresh)
    }
}

/// A block by its key in the blocks' table (`stone`, `oak_log`, `wall_torch`...), with an
/// optional state after a colon (`oak_door:9`).
fn block_named(name: &str) -> Option<Block> {
    let (key, state) = name.split_once(':').unwrap_or((name, "0"));
    let d = crate::content::blocks::def(by_key(key.strip_prefix("minecraft:").unwrap_or(key))?);
    let state: Block = state.parse().ok()?;
    (state < d.states).then(|| d.id + state)
}

/// A key by a bind's name (`forward`, `jump`, `reload`...), winit's name (`KeyW`, `Space`,
/// `ArrowUp`) or a letter or digit (`W`, `1`).
fn key_named(settings: &Settings, name: &str) -> Option<KeyCode> {
    if let Some((b, _, _)) = crate::app::keys::BINDS.iter().find(|(_, n, _)| *n == name) {
        return Some(settings.keys.get(*b));
    }
    use KeyCode as K;
    let special = match name {
        "Escape" | "Esc" => Some(K::Escape),
        "Enter" => Some(K::Enter),
        "Space" => Some(K::Space),
        "Tab" => Some(K::Tab),
        "Backspace" => Some(K::Backspace),
        "ArrowUp" | "Up" => Some(K::ArrowUp),
        "ArrowDown" | "Down" => Some(K::ArrowDown),
        "ArrowLeft" | "Left" => Some(K::ArrowLeft),
        "ArrowRight" | "Right" => Some(K::ArrowRight),
        _ => None,
    };
    special
        .or_else(|| crate::app::keys::parse(name))
        .or_else(|| crate::app::keys::parse(&format!("Key{}", name.to_uppercase())))
        .or_else(|| crate::app::keys::parse(&format!("Digit{name}")))
}
