//! `rustcraft --gun-shots <folder>`: every gun is held standing, walked, crouch-walked, run,
//! strafed and aimed with (standing and walking), and pictures are taken along the way, so
//! the first-person animations can be checked frame by frame. With `GUN_SHOTS_SIDE=1`, only the
//! player's own moves, seen from the side (standing, walking, running, sneaking, sneak-walking,
//! aiming), with each gun and then empty-handed. With `GUN_SHOTS_FP=1`, only the first-person
//! scenes. With `GUN_SHOTS_RELIEF=1`, a wall of ores and some flowers seen up close. With `GUN_SHOTS_LIGHT=1`, the pistol's light at night against planks and glass. With
//! `GUN_SHOTS_GRENADE=1`, a grenade readied and thrown. With `GUN_SHOTS_FISHING=1` (or one run's
//! name), a pond fished: cast, bite, a fish fought and landed, a line snapped. With `GUN_SHOTS_TURN=1`, only the three guns
//! held (and aimed) seen from the front by a camera that stays put while the player looks
//! around to the right and then to the left, to see what the body does. With `GUN_SHOTS_STATION=1`, the
//! gun stations instead: an AK-47 on the rifle station taken apart and put together again,
//! a pistol on the small one, and both seen standing in the world. Runs in bench mode (a copy of
//! the latest world in the temp folder, never the real save): a flat stone lane is laid out
//! to walk on, with a post at its end to aim at.

use super::camera::{FIXED_FRONT, SIDE_VIEW};
use super::*;
use crate::item::{gun_mod, gun_state, set_gun_mods, set_gun_rounds, set_gun_state, GunKind, Stack};

/// The guns tried, with their attachments.
const GUNS: [(GunKind, u8, &str); 5] = [
    (GunKind::Pistol, 0, "pistol"),
    (GunKind::Pistol, gun_mod::SILENCER | gun_mod::LASER | gun_mod::EXTENDED_MAGAZINE, "pistol_mods"),
    (GunKind::Pistol, gun_mod::SCOPE, "pistol_scope"),
    (GunKind::Revolver, 0, "revolver"),
    (GunKind::Ak, 0, "ak"),
];

/// What is done with each gun: the keys held, aiming or not, how long, and every how many
/// seconds a picture is taken (from `first`).
struct Scene {
    name: &'static str,
    keys: &'static [Bind],
    aim: bool,
    /// Shoot every this many seconds (0: never).
    fire: f32,
    /// Empty the magazine and reload at the start.
    reload: bool,
    /// Look the gun over (the inspect key) at the start.
    inspect: bool,
    /// Camera mode (0 first person, 2 facing the player: the gun on the player model).
    camera: u8,
    /// The gun's state at the start (`gun_state` bits; rounds are set to none with any).
    state: u16,
    secs: f32,
    first: f32,
    every: f32,
    /// The player looks around: right, then left, then ahead again (`look_around`).
    turn: bool,
}

const fn scene(name: &'static str, keys: &'static [Bind], aim: bool, secs: f32, first: f32, every: f32) -> Scene {
    Scene { name, keys, aim, fire: 0.0, reload: false, inspect: false, camera: 0, state: 0, secs, first, every, turn: false }
}

const SCENES: [Scene; 30] = [
    scene("idle", &[], false, 0.9, 0.8, 1.0),
    scene("walk", &[Bind::Forward], false, 1.5, 0.1, 0.1),
    scene("crouch", &[Bind::Forward, Bind::Sneak], false, 1.5, 0.1, 0.1),
    scene("sprint", &[Bind::Forward, Bind::Sprint], false, 1.5, 0.05, 0.1),
    scene("strafe", &[Bind::Right], false, 1.2, 0.1, 0.1),
    scene("aim", &[], true, 1.0, 0.05, 0.1),
    scene("aimwalk", &[Bind::Forward], true, 1.8, 0.3, 0.1),
    Scene { fire: 0.45, ..scene("fire", &[], false, 1.4, 0.02, 0.05) },
    Scene { fire: 0.45, ..scene("aimfire", &[], true, 1.6, 0.52, 0.05) },
    Scene { reload: true, ..scene("reload", &[], false, 3.2, 0.05, 0.15) },
    // Loading round by round, frame by frame (the revolver's), to see every step of it.
    Scene { reload: true, ..scene("loadframes", &[], false, 2.4, 0.9, 0.017) },
    Scene { inspect: true, ..scene("inspect", &[], false, 4.6, 0.1, 0.25) },
    Scene { camera: 2, ..scene("tpidle", &[], false, 0.8, 0.7, 1.0) },
    Scene { camera: 2, ..scene("tpwalk", &[Bind::Forward], false, 1.2, 0.4, 0.2) },
    Scene { camera: 2, ..scene("tpsprint", &[Bind::Forward, Bind::Sprint], false, 1.4, 0.8, 0.2) },
    Scene { camera: 2, ..scene("tpcrouch", &[Bind::Sneak], false, 0.9, 0.8, 1.0) },
    Scene { camera: 2, ..scene("tpaim", &[], true, 0.9, 0.8, 1.0) },
    Scene { fire: 0.45, camera: 2, ..scene("tpfire", &[], false, 1.4, 0.02, 0.05) },
    Scene { reload: true, camera: 2, ..scene("tpreload", &[], false, 3.2, 0.05, 0.15) },
    Scene { state: gun_state::NO_MAG | gun_state::CHAMBER_EMPTY, ..scene("nomag", &[], false, 0.6, 0.5, 1.0) },
    Scene { camera: SIDE_VIEW, ..scene("sideidle", &[], false, 0.9, 0.8, 1.0) },
    Scene { camera: SIDE_VIEW, ..scene("sidewalk", &[Bind::Forward], false, 1.6, 0.7, 0.1) },
    Scene { camera: SIDE_VIEW, ..scene("sidesprint", &[Bind::Forward, Bind::Sprint], false, 1.6, 0.9, 0.1) },
    Scene { camera: SIDE_VIEW, ..scene("sidecrouch", &[Bind::Sneak], false, 0.9, 0.8, 1.0) },
    Scene { camera: SIDE_VIEW, ..scene("sidecrouchwalk", &[Bind::Forward, Bind::Sneak], false, 2.4, 0.9, 0.2) },
    Scene { camera: SIDE_VIEW, ..scene("sideaim", &[], true, 0.9, 0.8, 1.0) },
    Scene { camera: SIDE_VIEW, ..scene("sideaimwalk", &[Bind::Forward], true, 1.8, 0.9, 0.15) },
    Scene { camera: FIXED_FRONT, turn: true, ..scene("turn", &[], false, TURN_SECS, 0.6, 0.25) },
    Scene { camera: FIXED_FRONT, turn: true, ..scene("turnaim", &[], true, TURN_SECS, 0.6, 0.25) },
    Scene { state: gun_state::CHAMBER_EMPTY | gun_state::LOCKED, ..scene("locked", &[], false, 0.6, 0.5, 1.0) },
];

/// How long a look around takes: settled, to the right, across to the left, back ahead.
const TURN_SECS: f32 = 7.0;

/// Where the player looks (yaw, radians from ahead) `t` seconds into a look around: still for
/// a moment, then 70 degrees to one side, held, across to 70 degrees the other way, held, and
/// back.
fn look_around(t: f32) -> f32 {
    let far = 70f32.to_radians();
    let ease = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    match t {
        t if t < 0.8 => 0.0,
        t if t < 2.0 => far * ease((t - 0.8) / 1.2),
        t if t < 2.8 => far,
        t if t < 4.8 => far * (1.0 - 2.0 * ease((t - 2.8) / 2.0)),
        t if t < 5.6 => -far,
        t => -far * (1.0 - ease((t - 5.6) / 1.2)),
    }
}

/// Seconds for the world to load before the first picture.
const WARMUP: f32 = 10.0;

pub struct GunShots {
    dir: std::path::PathBuf,
    wait: f32,
    /// Where the lane starts (feet), set up once.
    start: Option<Vec3>,
    gun: usize,
    scene: usize,
    t: f32,
    next: f32,
    n: u32,
}

impl GunShots {
    pub fn new(dir: std::path::PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        Self {
            dir,
            wait: WARMUP,
            start: None,
            // `GUN_SHOTS_ONLY=<name>`: start at that gun (e.g. "revolver").
            gun: std::env::var("GUN_SHOTS_ONLY")
                .ok()
                .and_then(|only| GUNS.iter().position(|g| g.2 == only))
                .unwrap_or(0),
            scene: 0,
            t: 0.0,
            next: SCENES[0].first,
            n: 0,
        }
    }
}

impl Game {
    /// A flat stone lane east of `around` (cleared above), with a post at its far end.
    fn lay_gun_lane(&mut self, around: Vec3) -> Vec3 {
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
                self.set_block(IVec3::new(x, ground, z), STONE);
                for y in ground + 1..ground + 7 {
                    self.set_block(IVec3::new(x, y, z), AIR);
                }
            }
        }
        for y in ground + 1..ground + 4 {
            for z in z0 - 1..=z0 + 1 {
                self.set_block(IVec3::new(x0 + 40, y, z), PLANKS);
            }
        }
        Vec3::new(x0 as f32 + 0.5, ground as f32 + 1.0, z0 as f32 + 0.5)
    }

    /// `GUN_SHOTS_LIGHT`: at midnight, the pistol with its light on, pointed at a wall of planks
    /// with a window of glass in it and a stone pillar behind: the light must go through the
    /// glass (the pillar lit through it) and not through the planks (their shadow on the
    /// ground and the pillar behind them dark). Pictures from the eyes, from the side, and from
    /// above behind the player.
    /// `GUN_SHOTS_GRENADE=1`: a frag grenade, then a smoke grenade, readied with the right
    /// button held (raised, the pin pulled, drawn back higher and higher) until it would go
    /// the farthest, and thrown (also seen from the side); then a frag grenade held until it
    /// goes off in the hand. A picture every 0.1 s.
    fn grenade_shots_step(&mut self, dt: f32, start: Vec3) {
        const PRESS: f32 = 0.3;
        let Some(g) = self.gun_shots.as_mut() else { return };
        let t0 = g.t;
        g.t += dt;
        let (t, which) = (g.t, g.gun);
        use super::camera::SIDE_VIEW;
        let runs = [
            (crate::item::FRAG_GRENADE, "frag", 3.6, 0),
            (crate::item::FRAG_GRENADE, "side", 3.6, SIDE_VIEW),
            (crate::item::SMOKE_GRENADE, "smoke", 3.6, 0),
            (crate::item::FRAG_GRENADE, "cooked", 7.0, 0),
        ];
        let Some((item, name, let_go, camera)) = runs.get(which).copied() else {
            println!("grenade shots done: {}", g.dir.display());
            self.quit = true;
            return;
        };
        if t0 == 0.0 {
            self.inventory.slots[7] = Some(Stack::new(item, 16));
            self.hotbar_slot = 7;
            self.camera.mode = camera;
            self.player.pos = start;
            self.player.vel = Vec3::ZERO;
            self.yaw = 0.0;
            self.pitch = 0.05;
            self.body_yaw = 0.0;
            self.keys.clear();
        }
        self.right_pressed |= t0 < PRESS && t >= PRESS;
        self.right_down = (PRESS..let_go).contains(&t);
        if t >= g.next && t >= PRESS - 0.05 {
            let shot = format!("grenade_{name}_{:02}.png", g.n);
            self.gpu.capture = Some(g.dir.join(shot));
            g.n += 1;
            g.next = t + 0.1;
        }
        if t >= let_go + 0.8 {
            self.camera.mode = 0;
            g.gun += 1;
            g.t = 0.0;
            g.n = 0;
            g.next = 0.0;
            self.right_down = false;
        }
    }

    /// `GUN_SHOTS_BUCKET=1`: a bucket of water, then one of lava, held (with the first-person
    /// body on, then off): looking ahead, down more and more, walking, stopping, turning.
    fn bucket_shots_step(&mut self, dt: f32, start: Vec3) {
        let Some(g) = self.gun_shots.as_mut() else { return };
        let runs = [
            (crate::item::WATER_BUCKET, "water", true),
            (crate::item::LAVA_BUCKET, "lava", true),
            (crate::item::WATER_BUCKET, "water_nobody", false),
            (crate::world::LANTERN as crate::item::ItemId, "lantern", true),
            (crate::item::BUCKET, "dropped", true),
            (crate::item::WATER_BUCKET, "water_tp", true),
            (crate::world::LANTERN as crate::item::ItemId, "lantern_tp", true),
        ];
        let Some(&(item, name, body)) = runs.get(g.gun) else {
            println!("bucket shots done: {}", g.dir.display());
            self.quit = true;
            return;
        };
        let t0 = g.t;
        g.t += dt;
        let t = g.t;
        if name == "dropped" {
            // At night: things that glow lying on the ground ahead, seen from above.
            self.time_of_day = 0.75;
            if t0 == 0.0 {
                self.player.pos = start;
                self.player.vel = Vec3::ZERO;
                self.yaw = 0.0;
                self.pitch = -0.6;
                self.inventory.slots[7] = None;
                self.hotbar_slot = 7;
                let ahead = look_dir(0.0, 0.0);
                let side = ahead.cross(Vec3::Y);
                let things = [crate::item::LAVA_BUCKET, crate::world::LANTERN as crate::item::ItemId, crate::world::TORCH as crate::item::ItemId, crate::item::WATER_BUCKET, crate::item::BUCKET];
                for (i, it) in things.into_iter().enumerate() {
                    let at = start + ahead * 2.6 + side * (i as f32 - 2.0) * 1.1 + Vec3::Y * 0.1;
                    self.items.push(ItemEntity::new(at, Vec3::ZERO, Stack::one(it), 1000.0));
                }
            }
            self.keys.clear();
            for (i, at) in [3.0f32, 4.0].iter().enumerate() {
                if t0 < *at && t >= *at {
                    self.gpu.capture = Some(g.dir.join(format!("bucket_dropped_{i:02}.png")));
                }
            }
            if t > 4.5 {
                g.gun += 1;
                g.t = 0.0;
            }
            return;
        }
        if t0 == 0.0 {
            self.inventory.slots[7] = Some(Stack::one(item));
            self.hotbar_slot = 7;
            self.camera.mode = if name.ends_with("_tp") { 2 } else { 0 };
            self.settings.first_person_body = body;
            self.player.pos = start;
            self.player.vel = Vec3::ZERO;
            self.yaw = 0.0;
            self.body_yaw = 0.0;
        }
        // Pitch over time: ahead, then down to 20, 45, 70, 88 degrees, then ahead walking.
        let pitch = match t {
            t if t < 1.5 => 0.0,
            t if t < 2.5 => -20f32,
            t if t < 3.5 => -45.0,
            t if t < 4.5 => -70.0,
            t if t < 5.5 => -88.0,
            t if t < 7.5 => -30.0,
            _ => 0.0,
        };
        self.pitch = pitch.to_radians();
        self.keys.clear();
        if (5.5..6.7).contains(&t) {
            self.keys.insert(self.settings.keys.get(Bind::Forward));
        }
        if (7.5..8.5).contains(&t) {
            self.yaw = (t - 7.5) * 2.5;
        }
        let shots = [1.2, 2.3, 3.3, 4.3, 5.3, 6.0, 6.5, 6.8, 7.0, 7.2, 7.8, 8.1, 8.5, 8.8];
        if let Some(i) = shots.iter().position(|&at| t0 < at && t >= at) {
            self.gpu.capture = Some(g.dir.join(format!("bucket_{name}_{i:02}.png")));
        }
        if t > 9.0 {
            g.gun += 1;
            g.t = 0.0;
            self.camera.mode = 0;
        }
    }

    /// `GUN_SHOTS_FISHING=1`: a pond is dug ahead of the lane and fished: the rod drawn back
    /// and cast, the bobber landing, nibbles and the bite (made to come soon), the hook set and
    /// a carp fought (a player keeping the tension bar in the middle, shifting gears with
    /// Shift+wheel) and landed; seen from the eyes, from the side, from behind and from the
    /// front; then a fish cranked in too hard, snapping the line. `GUN_SHOTS_FISHING=<name>`
    /// (fp, side, behind, front, snap): only that run.
    fn fishing_shots_step(&mut self, dt: f32, start: Vec3) {
        use super::camera::SIDE_VIEW;
        use super::fishing::{Bite, Fight};
        const RUNS: [(&str, u8, bool); 5] = [("fp", 0, false), ("side", SIDE_VIEW, false), ("behind", 1, false), ("front", 2, false), ("snap", 0, true)];
        let only = std::env::var("GUN_SHOTS_FISHING").ok().filter(|v| RUNS.iter().any(|r| r.0 == v));
        let Some(g) = self.gun_shots.as_mut() else { return };
        while let (Some(o), Some(r)) = (&only, RUNS.get(g.gun)) {
            if r.0 == o {
                break;
            }
            g.gun += 1;
        }
        let t0 = g.t;
        g.t += dt;
        let (t, dir, which) = (g.t, g.dir.clone(), g.gun);
        let crossed = |at: f32| t0 < at && t >= at;
        let Some(&(name, camera, snap)) = RUNS.get(which).filter(|_| only.is_none() || which == g.gun) else {
            println!("fishing shots done: {}", dir.display());
            self.camera.mode = 0;
            self.quit = true;
            return;
        };
        if only.is_some() && RUNS.get(which).map(|r| r.0) != only.as_deref() {
            println!("fishing shots done: {}", dir.display());
            self.quit = true;
            return;
        }
        let base = start.floor().as_ivec3();
        if t0 == 0.0 {
            if which == 0 || only.is_some() {
                // The pond: 3 deep, from 3 blocks ahead, walled with stone.
                let y = base.y - 1;
                for x in base.x + 2..=base.x + 24 {
                    for z in base.z - 8..=base.z + 8 {
                        for dy in -3..=0 {
                            let edge = x == base.x + 2 || x == base.x + 24 || z.abs_diff(base.z) == 8 || dy == -3;
                            self.set_block(IVec3::new(x, y + dy, z), if edge { STONE } else { crate::world::WATER });
                        }
                    }
                }
            }
            let mut rod = Stack::one(crate::item::FISHING_ROD);
            crate::item::set_rod_gear(&mut rod, 3);
            self.inventory.slots[7] = Some(rod);
            self.hotbar_slot = 7;
            self.camera.mode = camera;
            self.player.pos = start;
            self.player.vel = Vec3::ZERO;
            self.yaw = 0.0;
            self.pitch = -0.12;
            self.body_yaw = 0.0;
            self.keys.clear();
            self.fishing.line = None;
            self.right_down = false;
        }
        self.keys.clear();
        // Drawn back from 0.4 s, let go at 1.7 s (a full cast).
        self.right_pressed |= crossed(0.4);
        self.right_down = (0.4..1.7).contains(&t);
        // Nibbles soon after it lands, then the bite.
        if crossed(3.2) {
            if let Some(l) = self.fishing.line.as_mut() {
                l.bite = Bite::Nibble(2, 0.2);
            }
        }
        let line = self.fishing.line.as_ref();
        let striking = line.is_some_and(|l| matches!(l.bite, Bite::Strike(s) if s < 1.3));
        let fighting = line.is_some_and(|l| l.fight.is_some());
        // Set the hook a moment after the bite; a carp of 3 kg is on.
        if striking && !fighting {
            self.scroll = -1.0;
        }
        let mut hooked_now = false;
        if let Some(l) = self.fishing.line.as_mut() {
            if let Some(f) = l.fight.as_mut().filter(|f| f.species != 3 || f.max_stamina < 0.0) {
                *f = Fight::of(3, 3.0, f.dist);
                hooked_now = true;
            }
        }
        if let (Some(f), false) = (self.fishing.line.as_ref().and_then(|l| l.fight.as_ref()), hooked_now) {
            let gear = self.inventory.slots[7].map_or(3, |s| crate::item::rod_gear(&s));
            if snap {
                // Cranked flat out in the lowest gear.
                self.scroll = -2.0;
            } else {
                // Keeping the bar in the middle, in a higher gear when it is far off
                // (Shift+wheel).
                let off = (0.5 - f.tension).abs();
                let want_gear = if off > 0.25 { 5 } else if off > 0.1 { 3 } else { 1 };
                if want_gear != gear && (t * 4.0).fract() < dt * 4.0 {
                    self.keys.insert(self.settings.keys.get(Bind::Sneak));
                    self.scroll = if want_gear > gear { 1.0 } else { -1.0 };
                } else {
                    let rate = ((0.5 - f.tension) * 30.0).clamp(-9.0, 9.0);
                    self.scroll = -rate * dt;
                }
            }
        }
        // Pictures: the cast every 0.1 s, the bite, the fight every second, the landing.
        let caught = self.fishing.line.is_none() && t > 4.0;
        let every = if t < 3.0 { 0.1 } else if fighting { 1.0 } else { 0.25 };
        if let Some(g) = self.gun_shots.as_mut() {
            if t >= g.next && t >= 0.3 && (!caught || g.n < 1000) {
                self.gpu.capture = Some(dir.join(format!("fish_{name}_{:03}.png", g.n)));
                g.n += 1;
                g.next = t + every;
            }
            if caught {
                // Then every 0.1 s for a second and a half (the fish flying out, the rod
                // swung up), and on to the next run.
                g.next = g.next.min(t + 0.1);
                if g.wait > -1.5 {
                    g.wait -= dt;
                } else {
                    g.gun += 1;
                    g.t = 0.0;
                    g.n = 0;
                    g.next = 0.0;
                    g.wait = 0.0;
                    self.right_down = false;
                    self.camera.mode = 0;
                }
            }
            if t > 90.0 {
                g.gun += 1;
                g.t = 0.0;
                g.n = 0;
                g.next = 0.0;
            }
        }
    }

    fn light_shots_step(&mut self, dt: f32, start: Vec3) {
        use crate::world::{GLASS, PLANKS, STONE};
        let Some(g) = self.gun_shots.as_mut() else { return };
        let t0 = g.t;
        g.t += dt;
        let (t, dir) = (g.t, g.dir.clone());
        let crossed = |at: f32| t0 < at && t >= at;
        let base = start.floor().as_ivec3();
        if t0 == 0.0 {
            let (x, y) = (base.x + 4, base.y);
            for z in base.z - 3..=base.z + 3 {
                for dy in 0..3 {
                    let window = (base.z - 1..=base.z).contains(&z) && dy >= 1;
                    self.set_block(IVec3::new(x, y + dy, z), if window { GLASS } else { PLANKS });
                }
            }
            for z in base.z - 2..=base.z + 1 {
                for dy in 0..3 {
                    self.set_block(IVec3::new(x + 5, y + dy, z), STONE);
                }
            }
            let mut pistol = Stack::one(crate::item::PISTOL);
            set_gun_mods(&mut pistol, gun_mod::LIGHT | gun_mod::LIGHT_ON);
            set_gun_rounds(&mut pistol, 12);
            self.inventory.slots[0] = Some(pistol);
            self.hotbar_slot = 0;
            self.player.pos = start;
            self.player.vel = Vec3::ZERO;
            self.yaw = 0.0;
            self.pitch = -0.12;
            self.body_yaw = 0.0;
            self.camera.mode = 0;
            self.keys.clear();
            self.right_down = false;
        }
        for (at, name, mode) in [(2.0, "fp", 0u8), (4.0, "side", SIDE_VIEW), (6.0, "behind", 1)] {
            if crossed(at - 1.2) {
                self.camera.mode = mode;
            }
            if crossed(at) {
                self.gpu.capture = Some(dir.join(format!("light_{name}.png")));
            }
        }
        if t > 6.5 {
            self.camera.mode = 0;
            println!("light shots done");
            self.quit = true;
        }
    }

    /// `GUN_SHOTS_RELIEF`: a wall of every ore with flowers in front of it, seen from a few
    /// spots, to check their 3D details.
    fn relief_shots_step(&mut self, dt: f32, start: Vec3) {
        use crate::world::*;
        let Some(g) = self.gun_shots.as_mut() else { return };
        let t0 = g.t;
        g.t += dt;
        let (t, dir) = (g.t, g.dir.clone());
        let crossed = |at: f32| t0 < at && t >= at;
        let base = start.floor().as_ivec3();
        if t0 == 0.0 {
            let ores = [COAL_ORE, IRON_ORE, COPPER_ORE, GOLD_ORE, DIAMOND_ORE];
            for (i, z) in (base.z - 2..=base.z + 2).enumerate() {
                for dy in 0..3 {
                    self.set_block(IVec3::new(base.x + 4, base.y + dy, z), ores[(i + dy as usize) % ores.len()]);
                }
            }
            self.set_block(IVec3::new(base.x + 4, base.y, base.z + 3), STONE);
            let flowers = [POPPY, DANDELION, OAK_SAPLING, BIRCH_SAPLING, SPRUCE_SAPLING];
            for (i, z) in (base.z - 2..=base.z + 2).enumerate() {
                self.set_block(IVec3::new(base.x + 2, base.y - 1, z), GRASS);
                self.set_block(IVec3::new(base.x + 2, base.y, z), flowers[i]);
            }
            self.inventory.slots[0] = None;
            self.hotbar_slot = 0;
            self.player.pos = start;
            self.player.vel = Vec3::ZERO;
            self.yaw = 0.0;
            self.pitch = -0.1;
            self.body_yaw = 0.0;
            self.camera.mode = 0;
            self.keys.clear();
        }
        // (seconds, name, position offset, yaw, pitch)
        let spots = [
            (1.5, "front", Vec3::ZERO, 0.0, -0.1),
            (2.5, "flowers", Vec3::new(0.6, 0.0, 0.0), 0.0, -0.75),
            (3.5, "slant", Vec3::new(2.2, 0.0, -2.6), 0.9, -0.2),
            (4.5, "close", Vec3::new(2.6, 0.0, 0.3), 0.0, 0.1),
        ];
        for (at, name, off, yaw, pitch) in spots {
            if crossed(at - 0.6) {
                self.player.pos = start + off;
                self.player.vel = Vec3::ZERO;
                self.yaw = yaw;
                self.pitch = pitch;
            }
            if crossed(at) {
                self.gpu.capture = Some(dir.join(format!("relief_{name}.png")));
            }
        }
        if t > 5.0 {
            println!("relief shots done");
            self.quit = true;
        }
    }

    /// `GUN_SHOTS_STATION`: the stations, pictures taken at set moments (seconds, name).
    fn station_shots_step(&mut self, dt: f32, start: Vec3) {
        use crate::entity::GunBench;
        use crate::item::*;
        const SHOTS: [(f32, &str); 16] = [
            (0.4, "world"),
            (2.0, "rifle_laid"),
            (2.6, "rifle_strip_a"),
            (3.1, "rifle_strip_b"),
            (4.2, "rifle_stripped"),
            (5.3, "rifle_assemble_a"),
            (5.9, "rifle_assemble_b"),
            (7.2, "rifle_assembled"),
            (8.3, "loader_a"),
            (9.0, "loader_b"),
            (9.9, "loader_c"),
            (11.6, "small_laid"),
            (12.2, "small_strip"),
            (13.4, "small_stripped"),
            (14.4, "world_after"),
            (14.9, "world_side"),
        ];
        let Some(g) = self.gun_shots.as_mut() else { return };
        let t0 = g.t;
        g.t += dt;
        let t = g.t;
        let crossed = |at: f32| t0 < at && t >= at;
        // The rifle station three blocks ahead, facing back down the lane (its cells across
        // it), the small one beside it.
        let x = start.x.floor() as i32 + 4;
        let (y, z) = (start.y.floor() as i32, start.z.floor() as i32);
        let rifle = IVec3::new(x, y, z - 1);
        let small = IVec3::new(x, y, z + 3);
        if t0 == 0.0 {
            self.player.pos = start;
            self.player.vel = Vec3::ZERO;
            self.yaw = 0.0;
            self.pitch = -0.35;
            self.hotbar_slot = 8;
            for (i, q) in crate::world::bench_cells(rifle, crate::world::rifle_bench_id(3)).into_iter().enumerate() {
                self.set_block(q, if i == 0 { crate::world::rifle_bench_id(3) } else { crate::world::RIFLE_BENCH_PART });
            }
            for (i, q) in crate::world::bench_cells(small, crate::world::gun_bench_id(3, false)).into_iter().enumerate() {
                self.set_block(q, crate::world::gun_bench_id(3, i == 1));
            }
            let mut ak = Stack::one(AK47);
            set_gun_rounds(&mut ak, 30);
            ak.damage = 300;
            let mut mag = Stack::one(AK_MAGAZINE);
            set_gun_rounds(&mut mag, 17);
            let mut bench = GunBench::default();
            bench.add(ak, -0.2, 0.0, 0.0);
            bench.add(mag, 1.15, 0.05, 0.0);
            bench.add(Stack::new(RIFLE_ROUND, 12), 1.2, -0.3, 0.3);
            // The loader in the drawer with a magazine on it, rounds in a box beside it.
            bench.loader = true;
            let mut part = Stack::one(AK_MAGAZINE);
            set_gun_rounds(&mut part, 4);
            bench.loader_mag = Some(part);
            bench.boxes[0] = Some(box_with(0, RIFLE_ROUND, 60));
            self.block_entities.benches.insert(rifle, bench);
            let mut pistol = Stack::one(crate::item::PISTOL);
            set_gun_rounds(&mut pistol, 12);
            let mut bench = GunBench::default();
            bench.add(pistol, -0.2, 0.0, 0.0);
            self.block_entities.benches.insert(small, bench);
        }
        let first_gun = |g: &Self, p: IVec3| {
            g.block_entities.benches.get(&p).and_then(|b| b.items.iter().find(|i| GunKind::of(i.stack.item).is_some()).map(|i| i.id))
        };
        let a_part = |g: &Self, p: IVec3| {
            g.block_entities
                .benches
                .get(&p)
                .and_then(|b| b.items.iter().find(|i| crate::item::AK_PARTS[..4].contains(&i.stack.item)).map(|i| i.id))
        };
        if crossed(1.0) {
            self.open_gun_station(rifle);
        }
        if crossed(2.2) {
            if let (Some(table), Some(id)) = (self.bench_table(rifle), first_gun(self, rifle)) {
                self.bench_right_click(rifle, &table, id);
            }
        }
        if crossed(5.0) {
            if let (Some(table), Some(id)) = (self.bench_table(rifle), a_part(self, rifle)) {
                self.bench_right_click(rifle, &table, id);
            }
        }
        if (7.6..10.6).contains(&t) {
            // Looking down into the drawer at the loader at work.
            self.bench_in_drawer = true;
            self.bench_focus = 1.0;
            if let Some(g) = self.gun_shots.as_mut() {
                if t > 9.2 && ((t0 - 9.2) / 0.05).floor() < ((t - 9.2) / 0.05).floor() {
                    self.gpu.capture = Some(g.dir.join(format!("station_feed_{:02}.png", ((t - 9.2) / 0.05) as u32)));
                }
            }
        }
        // What a click would do everywhere in the view of the drawer (`bench_pick_map`), with
        // the loader as it is (a magazine on it) and with nothing, a magazine and the loader in
        // hand: the loader must only answer in its bay.
        if crossed(8.6) {
            if let Some(dir) = self.gun_shots.as_ref().map(|g| g.dir.clone()) {
                let keep = self.block_entities.benches.get(&rifle).map(|b| (b.loader, b.loader_mag));
                let set = |g: &mut Self, loader: bool, mag: Option<Stack>| {
                    if let Some(b) = g.block_entities.benches.get_mut(&rifle) {
                        b.loader = loader;
                        b.loader_mag = mag;
                    }
                };
                let held_mag = Some(Stack::one(AK_MAGAZINE));
                let cases: [(&str, bool, bool, Option<Stack>); 4] = [
                    ("empty_hand_mag_on", true, true, None),
                    ("empty_hand_bare", true, false, None),
                    ("holding_mag", true, false, held_mag),
                    ("holding_loader", false, false, Some(Stack::one(MAG_LOADER))),
                ];
                for (name, loader, mag_on, cursor) in cases {
                    set(self, loader, if mag_on { keep.and_then(|k| k.1) } else { None });
                    self.cursor = cursor;
                    self.bench_pick_map(rifle, &dir.join(format!("station_pickmap_{name}.png")));
                }
                self.cursor = None;
                if let Some((loader, mag)) = keep {
                    set(self, loader, mag);
                }
                self.gpu.capture = Some(dir.join("station_pickmap_view.png"));
            }
        }
        if crossed(10.6) {
            self.close_container();
            self.open_gun_station(small);
        }
        if crossed(11.8) {
            if let (Some(table), Some(id)) = (self.bench_table(small), first_gun(self, small)) {
                self.bench_right_click(small, &table, id);
            }
        }
        if crossed(14.0) {
            self.close_container();
        }
        if crossed(14.6) {
            self.player.pos = start + Vec3::new(2.5, 0.0, -3.5);
            self.yaw = 0.9;
            self.pitch = -0.3;
        }
        for (at, name) in SHOTS {
            if crossed(at) {
                if let Some(g) = self.gun_shots.as_ref() {
                    self.gpu.capture = Some(g.dir.join(format!("station_{name}.png")));
                }
            }
        }
        // Items on the ground in a row ahead, and a magazine held (in first and third person).
        if crossed(15.3) {
            use crate::entity::dropped::ItemEntity;
            let mut row = Vec::new();
            for n in [30u8, 14, 0] {
                let mut m = Stack::one(AK_MAGAZINE);
                set_gun_rounds(&mut m, n);
                row.push(m);
            }
            let mut pm = Stack::one(PISTOL_MAGAZINE);
            set_gun_rounds(&mut pm, 6);
            row.push(pm);
            row.extend(AK_PARTS[..4].iter().map(|&i| Stack::one(i)));
            let mut ak = Stack::one(AK47);
            set_gun_rounds(&mut ak, 30);
            row.push(ak);
            row.push(Stack::new(RIFLE_ROUND, 5));
            let n = row.len() as f32;
            for (k, st) in row.into_iter().enumerate() {
                let at = start + Vec3::new(2.4, 0.2, (k as f32 - (n - 1.0) * 0.5) * 0.55);
                self.add_item(ItemEntity::new(at, Vec3::ZERO, st, 999.0));
            }
            self.player.pos = start;
            self.yaw = 0.0;
            self.pitch = -0.75;
            let mut m = Stack::one(AK_MAGAZINE);
            set_gun_rounds(&mut m, 14);
            self.inventory.slots[7] = Some(m);
            self.hotbar_slot = 7;
        }
        if crossed(17.0) {
            self.pitch = -0.1;
        }
        if crossed(18.0) {
            self.camera.mode = 2;
        }
        for (at, name) in [(16.8, "ground"), (17.8, "held_fp"), (18.8, "held_tp")] {
            if crossed(at) {
                if let Some(g) = self.gun_shots.as_ref() {
                    self.gpu.capture = Some(g.dir.join(format!("station_{name}.png")));
                }
            }
        }
        if t > 19.2 {
            self.camera.mode = 0;
            println!("station shots done");
            self.quit = true;
        }
    }

    /// Gun shot mode, every frame while playing.
    pub(super) fn gun_shots_step(&mut self, dt: f32) {
        self.hide_hud = false;
        let lamp_test = std::env::var("GUN_SHOTS_LIGHT").is_ok();
        // (the weapon light is looked at at midnight)
        self.time_of_day = if lamp_test { 0.75 } else { 0.25 };
        self.mouse_delta = Vec2::ZERO;
        let Some(g) = self.gun_shots.as_mut() else {
            return;
        };
        if g.wait > 0.0 {
            g.wait -= dt;
            if g.start.is_none() && g.wait < WARMUP - 2.0 {
                let around = self.player.pos;
                let start = self.lay_gun_lane(around);
                if let Some(g) = self.gun_shots.as_mut() {
                    g.start = Some(start);
                }
                for (i, (kind, mods, _)) in GUNS.iter().enumerate() {
                    let mut s = Stack::one(kind.item());
                    set_gun_mods(&mut s, *mods);
                    set_gun_rounds(&mut s, kind.magazine_size(*mods));
                    self.inventory.slots[i] = Some(s);
                }
                // Rounds for the revolver's reloads (loaded straight from the inventory).
                self.inventory.slots[8] = Some(Stack::new(crate::item::MAGNUM_ROUND, 64));
            }
            return;
        }
        if let (Some(start), true) = (g.start, std::env::var("GUN_SHOTS_STATION").is_ok()) {
            self.station_shots_step(dt, start);
            return;
        }
        if let (Some(start), true) = (g.start, std::env::var("GUN_SHOTS_RELIEF").is_ok()) {
            self.relief_shots_step(dt, start);
            return;
        }
        if let (Some(start), true) = (g.start, lamp_test) {
            self.light_shots_step(dt, start);
            return;
        }
        if let (Some(start), true) = (g.start, std::env::var("GUN_SHOTS_FISHING").is_ok()) {
            self.fishing_shots_step(dt, start);
            return;
        }
        if let (Some(start), true) = (g.start, std::env::var("GUN_SHOTS_BUCKET").is_ok()) {
            self.bucket_shots_step(dt, start);
            return;
        }
        if let (Some(start), true) = (g.start, std::env::var("GUN_SHOTS_GRENADE").is_ok()) {
            self.grenade_shots_step(dt, start);
            return;
        }
        let side = std::env::var("GUN_SHOTS_SIDE").is_ok();
        // After the guns, in side mode, the empty hand (a slot left empty).
        let hands = side && g.gun == GUNS.len();
        let gun_name = GUNS.get(g.gun).map(|g| g.2).or(hands.then_some("hands"));
        let (Some(start), Some(gun_name)) = (g.start, gun_name) else {
            println!("gun shots done: {}", g.dir.display());
            self.camera.mode = 0;
            self.quit = true;
            return;
        };
        // `GUN_SHOTS_FP`: only what is seen from the eyes (the first-person view).
        if std::env::var("GUN_SHOTS_FP").is_ok() && SCENES[g.scene].camera != 0 {
            g.scene += 1;
            if g.scene == SCENES.len() {
                g.scene = 0;
                g.gun += 1;
            }
            g.next = SCENES[g.scene].first;
            return;
        }
        let turning = std::env::var("GUN_SHOTS_TURN").is_ok();
        let wanted = if turning { "turn" } else if side { "side" } else { "tp" };
        // Looking around: the three guns as they come, not the pistol's attachments again.
        let skip_gun = turning && GUNS.get(g.gun).is_some_and(|g| g.1 != 0);
        if (turning || side || std::env::var("GUN_SHOTS_TP").is_ok()) && (!SCENES[g.scene].name.starts_with(wanted) || skip_gun) {
            g.scene += 1;
            if g.scene == SCENES.len() {
                g.scene = 0;
                g.gun += 1;
            }
            g.next = SCENES[g.scene].first;
            return;
        }
        let scene = &SCENES[g.scene];
        if g.t == 0.0 {
            // Every scene starts at the lane's start, looking down it, gun at rest.
            self.player.pos = start;
            self.player.vel = Vec3::ZERO;
            self.player.flying = false;
            self.yaw = 0.0;
            self.pitch = -0.02;
            self.body_yaw = 0.0;
            self.hotbar_slot = if hands { 6 } else { g.gun };
            self.w_sprint = false;
            self.camera.mode = scene.camera;
            self.guns.reload = None;
            self.guns.cylinder = None;
            if scene.reload {
                if let Some(s) = self.inventory.slots[g.gun].as_mut() {
                    set_gun_rounds(s, 0);
                    set_gun_state(s, gun_state::CHAMBER_EMPTY, true);
                    set_gun_state(s, gun_state::LOCKED, true);
                    // A revolver fired empty: its six cases are thrown out.
                    if s.item == crate::item::REVOLVER {
                        for k in 0..6 {
                            crate::item::set_revolver_chamber(s, k, crate::item::chamber::SPENT);
                        }
                    }
                }
                self.guns.reload_pressed = true;
            } else if let (Some(s), false) = (self.inventory.slots[g.gun].as_mut(), hands) {
                let (kind, mods, _) = GUNS[g.gun];
                set_gun_rounds(s, kind.magazine_size(mods));
                set_gun_state(s, gun_state::CHAMBER_EMPTY, false);
                set_gun_state(s, gun_state::LOCKED, false);
                set_gun_state(s, gun_state::NO_MAG, false);
                if scene.state != 0 {
                    set_gun_rounds(s, 0);
                    s.data |= scene.state;
                }
            }
        }
        if scene.inspect && g.t == 0.0 && !hands {
            self.guns.inspect = Some((0.0, GUNS[g.gun].0.item()));
        }
        // Clicks at a steady pace (the first right away, or once aimed).
        if scene.fire > 0.0 {
            let at = |t: f32| ((t - scene.first + 0.02).max(0.0) / scene.fire).floor();
            let fire = at(g.t + dt) > at(g.t) || (g.t == 0.0 && scene.first < 0.1);
            self.left_pressed |= fire;
            self.left_down = fire;
        }
        self.keys.clear();
        for b in scene.keys {
            self.keys.insert(self.settings.keys.get(*b));
        }
        self.right_down = scene.aim;
        if scene.turn {
            self.yaw = look_around(g.t);
        }
        g.t += dt;
        if g.t >= g.next {
            let name = format!("{gun_name}_{}_{:02}.png", scene.name, g.n);
            self.gpu.capture = Some(g.dir.join(name));
            g.n += 1;
            g.next += scene.every;
        }
        if g.t >= scene.secs {
            g.scene += 1;
            if g.scene == SCENES.len() {
                g.scene = 0;
                g.gun += 1;
            }
            g.t = 0.0;
            g.n = 0;
            g.next = SCENES[g.scene].first;
            self.keys.clear();
            self.right_down = false;
        }
    }
}
