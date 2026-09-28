//! `rustcraft --gun-shots <folder>`: every gun is held standing, walked, crouch-walked, run,
//! strafed and aimed with (standing and walking), and pictures are taken along the way, so
//! the first-person animations can be checked frame by frame. Runs in bench mode (a copy of
//! the latest world in the temp folder, never the real save): a flat stone lane is laid out
//! to walk on, with a post at its end to aim at.

use super::*;
use crate::item::{gun_mod, gun_state, set_gun_mods, set_gun_rounds, set_gun_state, GunKind, Stack};

/// The guns tried, with their attachments.
const GUNS: [(GunKind, u8, &str); 3] = [
    (GunKind::Pistol, 0, "pistol"),
    (GunKind::Pistol, gun_mod::SILENCER | gun_mod::LASER | gun_mod::EXTENDED_MAGAZINE, "pistol_mods"),
    (GunKind::Pistol, gun_mod::SCOPE, "pistol_scope"),
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
}

const fn scene(name: &'static str, keys: &'static [Bind], aim: bool, secs: f32, first: f32, every: f32) -> Scene {
    Scene { name, keys, aim, fire: 0.0, reload: false, inspect: false, camera: 0, state: 0, secs, first, every }
}

const SCENES: [Scene; 15] = [
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
    Scene { inspect: true, ..scene("inspect", &[], false, 4.6, 0.1, 0.25) },
    Scene { fire: 0.45, camera: 2, ..scene("tpfire", &[], false, 1.4, 0.02, 0.05) },
    Scene { reload: true, camera: 2, ..scene("tpreload", &[], false, 3.2, 0.05, 0.15) },
    Scene { state: gun_state::NO_MAG | gun_state::CHAMBER_EMPTY, ..scene("nomag", &[], false, 0.6, 0.5, 1.0) },
    Scene { state: gun_state::CHAMBER_EMPTY | gun_state::LOCKED, ..scene("locked", &[], false, 0.6, 0.5, 1.0) },
];

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
            gun: 0,
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

    /// Gun shot mode, every frame while playing.
    pub(super) fn gun_shots_step(&mut self, dt: f32) {
        self.hide_hud = false;
        self.time_of_day = 0.25;
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
            }
            return;
        }
        let (Some(start), Some(&(_, _, gun_name))) = (g.start, GUNS.get(g.gun)) else {
            println!("gun shots done: {}", g.dir.display());
            self.camera.mode = 0;
            self.quit = true;
            return;
        };
        let scene = &SCENES[g.scene];
        if g.t == 0.0 {
            // Every scene starts at the lane's start, looking down it, gun at rest.
            self.player.pos = start;
            self.player.vel = Vec3::ZERO;
            self.player.flying = false;
            self.yaw = 0.0;
            self.pitch = -0.02;
            self.body_yaw = 0.0;
            self.hotbar_slot = g.gun;
            self.w_sprint = false;
            self.camera.mode = scene.camera;
            self.guns.reload = None;
            if scene.reload {
                if let Some(s) = self.inventory.slots[g.gun].as_mut() {
                    set_gun_rounds(s, 0);
                    set_gun_state(s, gun_state::CHAMBER_EMPTY, true);
                    set_gun_state(s, gun_state::LOCKED, true);
                }
                self.guns.reload_pressed = true;
            } else if let Some(s) = self.inventory.slots[g.gun].as_mut() {
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
        if scene.inspect && g.t == 0.0 {
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
