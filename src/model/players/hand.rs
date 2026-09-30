//! The first-person hand: the arm or the held item, with Minecraft's swing, equip, bob,
//! eating and sword-blocking animations. Built on the CPU each frame in world space.

use crate::model::player::ARM as ARM_LAYERS;
use crate::model::spring::Spring3;
use crate::model::emit_box;
use crate::item::{icon, GunKind, Icon, ItemId, NONE};
use crate::util::vertex_light;
use crate::world::mesh::{flags, Vertex};
use crate::world::TORCH;
use glam::{Mat4, Vec2, Vec3};
use std::f32::consts::{PI, TAU};
fn t(x: f32, y: f32, z: f32) -> Mat4 {
    Mat4::from_translation(Vec3::new(x, y, z))
}
fn rx(d: f32) -> Mat4 {
    Mat4::from_rotation_x(d.to_radians())
}
fn ry(d: f32) -> Mat4 {
    Mat4::from_rotation_y(d.to_radians())
}
fn rz(d: f32) -> Mat4 {
    Mat4::from_rotation_z(d.to_radians())
}

/// Aimed with a scope, the gun comes this much further (model pixels): down to the scope's
/// axis instead of the iron sights', and back so the eyepiece is close to the eye.
const SCOPE_EYE: Vec3 = Vec3::new(0.0, -0.95, 13.6);

/// Seconds a gun is looked over (the inspect key): brought up showing its right side, turned
/// to look down its top, then over to its left side, and back.
pub const INSPECT_TIME: f32 = 4.4;

/// Blocks per Blockbench pixel of the first-person pistol (`pistol_vm`): its camera is at
/// the model's origin, so this only sets how far in front of the eye it is.
const VIEW_PX: f32 = 1.0 / 64.0;

/// Arm swing duration in seconds (a bit snappier than Minecraft's 6 ticks).
const SWING_TIME: f32 = 0.24;
/// Fastest swing, used when actions follow each other very quickly.
const MIN_SWING_TIME: f32 = 0.09;
/// Seconds the arm follows through after throwing a grenade.
const THROW_TIME: f32 = 0.3;


pub struct HandAnim {
    swing: f32,
    swinging: bool,
    /// Duration of the current swing; shorter when actions come in quick succession.
    swing_time: f32,
    /// Animation clock and the time of the last swing request (to measure the action rhythm).
    clock: f32,
    last_request: f32,
    equip: f32,
    walk_dist: f32,
    bob: f32,
    /// The held item lagging behind the view's turning (yaw, pitch, roll in degrees): a
    /// spring, so it swings back a little past rest when the turning stops.
    sway: Spring3,
    pub held: ItemId,
    /// The held stack's state (`Stack::data`, `Stack::damage`): a magazine's rounds, dirt.
    pub held_data: u16,
    pub held_damage: u16,
    /// Hand height, 1 = normal; lowered out of view when the body's arms take over
    /// (First Person Model's dynamic hands).
    pub lower: f32,
    /// Blocking with a sword: held across the view (Minecraft 1.8).
    pub blocking: bool,
    /// Seconds spent eating or drinking the held item (None when not).
    pub eating: Option<f32>,
    /// The held grenade being readied (the right button held: raised, its pin pulled by the
    /// other hand, then drawn back to throw): for how long (seconds), and how hard it would
    /// be thrown now (0..1; the hand goes up with it). Set by the game each frame.
    pub grenade: Option<(f32, f32)>,
    /// Seconds since a grenade left the hand (the arm follows through, then the next one
    /// comes up).
    thrown: Option<f32>,
    /// Chopping with an axe: the arms and the axe are drawn by the chop's rig then
    /// (`chop_rig`), not here.
    pub hidden: bool,
    /// Where the middle of the readied grenade was drawn last frame (like `torch_tip`): it
    /// is thrown from there.
    pub grenade_tip: Option<Vec3>,
    /// The held fishing rod: what it is doing (set by the game each frame), and where its tip
    /// was drawn last frame (like `torch_tip`: the line leaves from there).
    pub rod: Option<crate::model::angler::RodAnim>,
    pub rod_tip: Option<Vec3>,
    /// With the first-person body: a held lantern hangs from the fist by its chain and swings;
    /// otherwise it is held still by its handle.
    pub fancy_lantern: bool,
    lantern_swing: crate::model::lantern::SmoothSwing,
    /// The liquid in a held bucket, and the bucket swinging on its handle.
    bucket: crate::model::bucket::Slosh,
    /// Blend 0..1 from the normal hold to the blocking pose.
    block: f32,
    /// Where the held torch's fire was drawn last frame (world, but in the hand's own
    /// projection), for its flame particles.
    pub torch_tip: Option<Vec3>,
    /// Seconds since the held gun last fired (while its shot animation is playing), and since
    /// its trigger was pulled on an empty chamber.
    shot: Option<f32>,
    dry: Option<f32>,
    /// The muzzle flash of the last shot: how much is left (1 .. 0), its size and its turn.
    flash: f32,
    flash_size: f32,
    flash_seed: f32,
    /// Where the held pistol's muzzle was drawn last frame (like `torch_tip`).
    pub muzzle_tip: Option<Vec3>,
    /// And its ejection port, where the spent cases fly out, and its laser sight's lens.
    pub eject_tip: Option<Vec3>,
    pub laser_tip: Option<Vec3>,
    /// The weapon light's lens (where its light comes from), when there is one.
    pub light_tip: Option<Vec3>,
    /// Which way the held gun's barrel points (in the hand's own view).
    pub barrel_dir: Option<Vec3>,
    /// The held revolver's chambers: the head of what is in each (in the hand's own view),
    /// where the cases come out.
    pub chamber_tips: Option<[Vec3; 6]>,
    /// The held pistol: aimed down the sights (0 from the hip .. 1 aimed), how far a reload
    /// has got (0..1) and its attachments.
    pub aim: f32,
    pub reload: Option<f32>,
    pub gun_mods: u8,
    /// How dirty the held gun looks (`pistol_view::dirt_level`).
    pub gun_dirt: u8,
    /// The held pistol's state and what its reload does (set by the game; the shot and the
    /// reload's progress are `shot` and `reload`).
    pub gun_state: crate::model::pistol_view::GunAnim,
    /// Sprinting and sneaking this frame (set by the game), and how far the gun has gone into
    /// the sprinting and the crouched pose (0..1, eased).
    pub sprinting: bool,
    pub crouching: bool,
    sprint: f32,
    crouch: f32,
    /// How much the player is walking on the ground (0 standing .. 1 at walking speed and
    /// more when running), eased: how much the gun sways with the steps.
    stride: f32,
    /// The player's velocity in the view's frame (x right, y up, z forward; set by the
    /// game), and the gun's lagging copy of it: it leans into a strafe and trails a jump.
    pub motion: Vec3,
    lag: Spring3,
    /// The gun's sprint and crouch poses (x, y; 0..1 with a little overshoot).
    poses: Spring3,
    /// Seconds into looking the held gun over (set by the game).
    pub inspect: Option<f32>,
    /// The held guide book: how far it is lifted up to read (0 held low .. 1 in front of the
    /// eyes) and how it looks. Set by the game each frame.
    pub book: Option<(f32, crate::model::book::BookView)>,
    /// Where the view's middle falls on the book's pages (see `book::page_hit`), from the
    /// last build.
    pub book_hit: Option<crate::model::book::BookHit>,
    /// The held gun's see-through glass from the last build (drawn blended after the hand),
    /// and its scope's eyepiece when that shows the scope's view: middle, right and up
    /// (unit), radius (world, in the hand's own view).
    pub glass: Vec<Vertex>,
    pub eyepiece: Option<(Vec3, Vec3, Vec3, f32)>,
    /// How much of the view (from its middle to its top edge) the eyepiece covers when fully
    /// aimed, last seen so: the scope's field of view is the same wherever it is held.
    pub scope_across: f32,
}

impl HandAnim {
    pub fn new() -> Self {
        Self {
            swing: 0.0,
            swinging: false,
            swing_time: SWING_TIME,
            clock: 0.0,
            last_request: -1.0,
            equip: 0.0,
            walk_dist: 0.0,
            bob: 0.0,
            sway: Spring3::default(),
            held: NONE,
            held_data: 0,
            held_damage: 0,
            lower: 1.0,
            blocking: false,
            block: 0.0,
            eating: None,
            grenade: None,
            thrown: None,
            hidden: false,
            grenade_tip: None,
            rod: None,
            rod_tip: None,
            fancy_lantern: false,
            lantern_swing: crate::model::lantern::SmoothSwing::default(),
            bucket: Default::default(),
            torch_tip: None,
            shot: None,
            dry: None,
            flash: 0.0,
            flash_size: 1.0,
            flash_seed: 0.0,
            muzzle_tip: None,
            eject_tip: None,
            laser_tip: None,
            light_tip: None,
            barrel_dir: None,
            chamber_tips: None,
            aim: 0.0,
            reload: None,
            gun_mods: 0,
            gun_dirt: 0,
            gun_state: crate::model::pistol_view::GunAnim { chambered: true, ..Default::default() },
            sprinting: false,
            crouching: false,
            sprint: 0.0,
            crouch: 0.0,
            stride: 0.0,
            motion: Vec3::ZERO,
            lag: Spring3::default(),
            poses: Spring3::default(),
            inspect: None,
            book: None,
            book_hit: None,
            glass: Vec::new(),
            eyepiece: None,
            scope_across: 0.185,
        }
    }

    /// A shot from the held gun: its shot animation plays (the slide flies back and the case
    /// is pulled out), and a muzzle flash of this size (0: none, silenced) turned by `seed`
    /// (0..1) lights up for a moment.
    pub fn shoot(&mut self, flash: f32, seed: f32) {
        self.shot = Some(0.0);
        self.dry = None;
        if flash > 0.0 {
            self.flash = 1.0;
            self.flash_size = flash;
            self.flash_seed = seed;
        }
    }

    /// The trigger pulled with nothing in the chamber: it moves, nothing else does.
    pub fn dry_fire(&mut self) {
        self.dry = Some(0.0);
    }

    /// A grenade thrown: the empty arm follows through, then the next one comes up.
    pub fn throw(&mut self) {
        self.thrown = Some(0.0);
        self.grenade = None;
    }

    /// What the held gun is doing, for its moving parts (also on the player model).
    pub fn gun_anim(&self) -> crate::model::pistol_view::GunAnim {
        crate::model::pistol_view::GunAnim {
            shot: self.shot,
            dry: self.dry,
            reload: self.reload,
            aim: self.aim.clamp(0.0, 1.0),
            ..self.gun_state
        }
    }

    /// One swing per action (breaking or placing a block, hitting, throwing). The swing
    /// speed follows the rhythm of the actions, so fast breaking/placing gets fast swings
    /// that finish before the next one instead of one slow swing that ignores them.
    pub fn swing(&mut self) {
        let interval = self.clock - self.last_request;
        self.last_request = self.clock;
        self.swing_time = (interval * 0.9).clamp(MIN_SWING_TIME, SWING_TIME);
        // Restart unless the current swing has barely started (avoids a visible jump back).
        if !self.swinging || self.swing > 0.35 {
            self.swing = 0.0;
            self.swinging = true;
        }
    }

    /// Starts a swing only if the arm is at rest (continuous mining keeps one smooth loop).
    pub fn keep_swinging(&mut self) {
        if !self.swinging {
            self.swing_time = SWING_TIME;
            self.swing = 0.0;
            self.swinging = true;
        }
    }

    /// Current attack swing progress (0 when idle).
    pub fn attack(&self) -> f32 {
        if self.swinging {
            self.swing
        } else {
            0.0
        }
    }

    /// Switch to a new held item with the lower-and-raise animation.
    pub fn equip(&mut self, item: ItemId) {
        if item != self.held {
            self.held = item;
            self.equip = 0.0;
            self.shot = None;
            self.dry = None;
            self.block = 0.0;
            self.lantern_swing = crate::model::lantern::SmoothSwing::default();
            self.bucket = Default::default();
        }
    }

    pub fn update(
        &mut self,
        dt: f32,
        mining: bool,
        walk_speed: f32,
        on_ground: bool,
        look_delta: Vec2,
    ) {
        self.clock += dt;
        if self.swinging {
            self.swing += dt / self.swing_time;
            if self.swing >= 1.0 {
                if mining {
                    // Holding the button on a slow block: loop at the normal pace again once
                    // the quick actions have stopped.
                    if self.clock - self.last_request > SWING_TIME {
                        self.swing_time = SWING_TIME;
                    }
                    self.swing = (self.swing - 1.0).min(0.5);
                } else {
                    self.swing = 0.0;
                    self.swinging = false;
                }
            }
        } else if mining {
            self.keep_swinging();
        }
        self.equip = (self.equip + dt * 4.0).min(1.0);
        if let Some(t) = self.thrown {
            let t = t + dt;
            self.thrown = (t < THROW_TIME).then_some(t);
            if self.thrown.is_none() {
                // (the next grenade, if there is one, comes up from below)
                self.equip = 0.0;
            }
        }
        self.shot = self.shot.map(|t| t + dt).filter(|&t| t < 1.0);
        self.dry = self.dry.map(|t| t + dt).filter(|&t| t < 1.0);
        self.flash = (self.flash - dt / 0.06).max(0.0);
        // The sword swings into the blocking pose and back in about 0.15 s.
        let target = if self.blocking { 1.0 } else { 0.0 };
        let step = dt / 0.15;
        self.block += (target - self.block).clamp(-step, step);
        // Minecraft: bob approaches min(0.1, speed per tick); walk distance grows at 0.6x speed.
        let target = if on_ground {
            (walk_speed / 20.0).min(0.1)
        } else {
            0.0
        };
        self.bob += (target - self.bob) * (1.0 - (-8.0 * dt).exp());
        self.walk_dist += walk_speed * dt * 0.6;
        // Turning the view: the item lags behind and rolls into the turn, swinging back past
        // rest when the turning stops.
        let turn = (look_delta * 0.03).clamp(Vec2::splat(-3.0), Vec2::splat(3.0));
        self.sway
            .step(Vec3::new(turn.x, turn.y, -turn.x * 0.9), 4.2, 0.42, dt);
        // Into and out of the sprint and crouch poses smoothly (about a third of a second into
        // the sprint, a quarter out of it, to shoot again), hardly overshooting.
        let sprint = if self.sprinting && self.reload.is_none() { 1.0 } else { 0.0 };
        let crouch = if self.crouching { 1.0 } else { 0.0 };
        let freq = if sprint < self.poses.x.x { 2.2 } else { 1.5 };
        self.poses.step(Vec3::new(sprint, crouch, 0.0), freq, 0.85, dt);
        self.sprint = self.poses.x.x.max(0.0);
        self.crouch = self.poses.x.y.max(0.0);
        let target = if on_ground { (walk_speed / 4.3).min(1.4) } else { 0.0 };
        self.stride += (target - self.stride) * (1.0 - (-8.0 * dt).exp());
        let target = self.motion.clamp(Vec3::new(-6.0, -9.0, -6.0), Vec3::new(6.0, 9.0, 6.0));
        self.lag.step(target, 2.4, 0.42, dt);
    }

    /// View bobbing, applied in camera space to both the world and the hand (like Minecraft).
    /// Aiming a gun steadies the view (a gun has its own sway instead, see `build_gun`).
    pub fn bob_matrix(&self) -> Mat4 {
        let f = -self.walk_dist * PI;
        let k = self.aim.clamp(0.0, 1.0);
        let b = self.bob * (1.0 - 0.9 * k * k * (3.0 - 2.0 * k));
        t(f.sin() * b * 0.5, -(f.cos() * b).abs(), 0.0)
            * rz(f.sin() * b * 3.0)
            * rx((((-self.walk_dist) * PI - 0.2).cos() * b).abs() * 5.0)
    }

    /// Walking phase used by the hand and camera, in radians.
    pub fn walk_phase(&self) -> f32 {
        self.walk_dist * PI
    }

    /// Minecraft's `ItemInHandRenderer.renderPlayerArm` pose (camera space, blocks) for the
    /// right arm. `s`/`sq`: attack progress and its square root, `eq`: equip progress.
    fn arm_pose(base: Mat4, s: f32, sq: f32, eq: f32) -> Mat4 {
        base * t(
            -0.3 * (sq * PI).sin() + 0.64,
            0.4 * (sq * TAU).sin() - 0.6 - (1.0 - eq) * 0.6,
            -0.4 * (s * PI).sin() - 0.72,
        ) * ry(45.0)
            * ry((sq * PI).sin() * 70.0)
            * rz((s * s * PI).sin() * -20.0)
            * t(-1.0, 3.6, 3.5)
            * rz(120.0)
            * rx(200.0)
            * ry(-135.0)
            * t(5.6, 0.0, 0.0)
    }

    /// The arm model part within that pose: model pixels to the world.
    fn arm_part(pose: Mat4) -> Mat4 {
        pose * t(-5.0 / 16.0, 2.0 / 16.0, 0.0)
            * Mat4::from_scale(Vec3::splat(1.0 / 16.0))
            * rx(180.0)
    }

    /// Builds the arm (empty hand) or the held item in world space.
    pub fn build(
        &mut self,
        out: &mut Vec<Vertex>,
        cam_to_world: Mat4,
        sky: u8,
        blk: u8,
        dt: f32,
        skin: u8,
    ) {
        let light = vertex_light(sky, blk);
        let fl = flags::VIEWMODEL;
        self.torch_tip = None;
        self.grenade_tip = None;
        self.rod_tip = None;
        self.glass.clear();
        self.eyepiece = None;
        self.muzzle_tip = None;
        self.eject_tip = None;
        self.chamber_tips = None;
        self.laser_tip = None;
        self.light_tip = None;
        self.barrel_dir = None;
        self.book_hit = None;
        if self.hidden {
            return;
        }
        if let (crate::item::GUIDE_BOOK, Some((read, view))) = (self.held, self.book) {
            // A little light to read by, even at night.
            let light = vertex_light(sky, blk.max(9));
            let base = cam_to_world * rx(self.sway.x.y) * ry(self.sway.x.x) * rz(self.sway.x.z);
            let eq = {
                let e = self.equip;
                e * e * (3.0 - 2.0 * e)
            };
            self.build_book(out, base, read, &view, eq, light, fl, skin);
            return;
        }
        let s = self.attack();
        let sq = s.sqrt();
        let eq = {
            let e = self.equip;
            e * e * (3.0 - 2.0 * e)
        } * self.lower;
        // Hand lags slightly behind camera rotation.
        // Aimed, the gun stays nearly on the sight line while turning.
        let k = 1.0 - 0.75 * self.aim.clamp(0.0, 1.0);
        let sw = self.sway.x * k;
        let base = cam_to_world * rx(sw.y) * ry(sw.x) * rz(sw.z);

        if let Some(tt) = self.thrown {
            // A grenade just thrown: the empty arm follows through.
            let k = (tt / THROW_TIME).min(1.0);
            let m = Self::arm_part(Self::arm_pose(base, k, k.sqrt(), 1.0));
            emit_box(
                out,
                m,
                Vec3::new(-3.0, -10.0, -2.0),
                Vec3::new(1.0, 2.0, 2.0),
                ARM_LAYERS.map(|layer| crate::world::textures::skin_layer(layer, skin)),
                [[255; 3]; 6],
                light,
                fl,
            );
            return;
        }
        if let (Some((t, power)), true) = (self.grenade, crate::model::grenade_item(self.held)) {
            self.build_grenade_hold(out, base, t, power, light, fl, skin);
            return;
        }
        if self.held == crate::item::FISHING_ROD {
            self.build_rod(out, base, eq, light, fl, skin);
            return;
        }
        if self.held == NONE {
            let m = Self::arm_part(Self::arm_pose(base, s, sq, eq));
            emit_box(
                out,
                m,
                Vec3::new(-3.0, -10.0, -2.0),
                Vec3::new(1.0, 2.0, 2.0),
                ARM_LAYERS.map(|layer| crate::world::textures::skin_layer(layer, skin)),
                [[255; 3]; 6],
                light,
                fl,
            );
            return;
        }

        // Held item: arm transform + attack transform + item display transform.
        let f = (s * s * PI).sin();
        let f1 = (sq * PI).sin();
        // Eating or drinking (Minecraft's applyEatTransform): the item comes up to the mouth
        // and bobs there.
        let eat = match self.eating {
            Some(used) => {
                let total = crate::entity::survival::USE_TIME * 20.0;
                let left = (total - used * 20.0).max(0.0) + 1.0;
                let frac = left / total;
                let bob = if frac < 0.8 {
                    ((left / 4.0 * PI).cos() * 0.1).abs()
                } else {
                    0.0
                };
                let k = 1.0 - frac.min(1.0).powi(27);
                t(k * 0.6, bob - k * 0.5, 0.0) * ry(k * 90.0) * rx(k * 10.0) * rz(k * 30.0)
            }
            None => Mat4::IDENTITY,
        };
        let m = base
            * eat
            * t(-0.4 * f1, 0.2 * (sq * TAU).sin(), -0.2 * (s * PI).sin())
            * t(0.56, -0.52 - (1.0 - eq) * 0.6, -0.72)
            * ry(45.0 + f * -20.0)
            * rz(f1 * -20.0)
            * rx(f1 * -80.0)
            * ry(-45.0);
        if let Some(kind) = GunKind::of(self.held) {
            self.build_gun(kind, out, base, light, fl, eq, skin);
            return;
        }
        let flat = matches!(icon(self.held), Icon::Flat(_));
        if self.held == crate::world::LANTERN as ItemId && self.fancy_lantern {
            // Hanging by its chain from the fist. Its body follows the hand with the same
            // gravity-driven swing as the player model.
            let grip = base * t(-0.025, 0.125, 0.0) * rz(10.0);
            let pose = Self::arm_pose(grip, s, sq, eq);
            emit_box(
                out,
                Self::arm_part(pose),
                Vec3::new(-3.0, -10.0, -2.0),
                Vec3::new(1.0, 2.0, 2.0),
                ARM_LAYERS.map(|layer| crate::world::textures::skin_layer(layer, skin)),
                [[255; 3]; 6],
                light,
                fl,
            );
            let turn = glam::Quat::from_xyzw(0.2077, -0.6488, 0.4433, 0.5825).normalize();
            let center = Vec3::splat(0.5);
            // Block model space (0..1) to the world, then the lantern's own pixel space.
            let block = pose
                * t(-0.684, 0.117, -0.439)
                * Mat4::from_translation(center)
                * Mat4::from_quat(turn)
                * Mat4::from_translation(-center);
            let pivot = block.transform_point3(Vec3::new(0.5, 11.0 / 16.0, 0.5));
            let style = crate::model::lantern::FIRST_PERSON;
            let dir = self.lantern_swing.update(style, pivot, dt);
            let forward = -cam_to_world.z_axis.truncate();
            let yaw = forward.z.atan2(forward.x);
            crate::model::lantern::emit_held_lantern(out, style, pivot, dir, yaw, light, fl);
            return;
        }
        if let Some(fill) = crate::model::bucket::Fill::of(self.held) {
            self.build_bucket(out, base, fill, s, sq, eq, light, fl, dt, skin);
            return;
        }
        let lantern = self.held == crate::world::LANTERN as ItemId;
        let item = if crate::model::is_model_item(self.held) {
            // A gun's part, a magazine, an attachment, a round or a grenade: held low in the
            // right hand, turned a little so its side and top show, swinging with it.
            base * eat
                * t(-0.4 * f1, 0.2 * (sq * TAU).sin(), -0.2 * (s * PI).sin())
                * t(0.3, -0.3 - (1.0 - eq) * 0.6, -0.56)
                * ry(-32.0 + f * 10.0)
                * rx(14.0 + f1 * -40.0)
                * rz(-6.0)
                * Mat4::from_scale(Vec3::splat(0.36))
        } else if self.held == TORCH as ItemId || lantern {
            // Upright in the fist (a lantern without the first-person body too).
            m * t(0.08, 0.2, 0.06) * rz(-12.0) * Mat4::from_scale(Vec3::splat(0.82))
        } else if flat {
            m * t(1.13 / 16.0, 3.2 / 16.0, 1.13 / 16.0)
                * ry(-90.0)
                * rz(25.0)
                * Mat4::from_scale(Vec3::splat(0.68))
        } else {
            m * ry(45.0) * Mat4::from_scale(Vec3::splat(0.4))
        };
        let item = if self.block > 0.0 {
            // Minecraft 1.8's ItemRenderer: transformFirstPersonItem (no swing), then
            // doBlockTransformations, then the 1.8 handheld first-person display transform.
            let guard = base
                * t(0.56, -0.52 - (1.0 - eq) * 0.6, -0.72)
                * ry(45.0)
                * Mat4::from_scale(Vec3::splat(0.4))
                * t(-0.5, 0.2, 0.0)
                * ry(30.0)
                * rx(-80.0)
                * ry(60.0)
                * t(0.0, 4.0 / 16.0, 2.0 / 16.0)
                * ry(-135.0)
                * rz(25.0)
                * Mat4::from_scale(Vec3::splat(1.7));
            let k = self.block * self.block * (3.0 - 2.0 * self.block);
            blend(item, guard, k)
        } else {
            item
        };
        if lantern {
            // Half-size lantern model (pixels) held still by the top of its handle.
            let k = 0.5 / 16.0;
            let m = item * t(0.0, 0.12 - 11.0 * k, 0.0) * Mat4::from_scale(Vec3::splat(k));
            use crate::model::lantern::{emit_lantern, LanternKind};
            emit_lantern(out, m, light, fl, LanternKind::Standing);
            return;
        }
        if self.held == TORCH as ItemId {
            self.torch_tip = Some(item.transform_point3(crate::model::player::TORCH_TIP));
        }
        let st = crate::item::Stack { data: self.held_data, damage: self.held_damage, ..crate::item::Stack::one(self.held) };
        crate::model::emit_held_data(out, item, &st, light, fl);
    }
}

impl HandAnim {
    /// A bucket hanging from the fist by its handle, held like a lantern (the same arm, the
    /// same fist), it stays upright in the world, swinging a little on its
    /// handle as it is moved about, the liquid in it rocking. Used, it tips forward.
    #[allow(clippy::too_many_arguments)]
    fn build_bucket(
        &mut self,
        out: &mut Vec<Vertex>,
        base: Mat4,
        fill: crate::model::bucket::Fill,
        s: f32,
        sq: f32,
        eq: f32,
        light: [u8; 4],
        fl: u8,
        dt: f32,
        skin: u8,
    ) {
        use crate::model::bucket;
        // The arm and the fist exactly as for a lantern; the handle's grip in the fist.
        let f1 = (sq * PI).sin();
        let grip = base * t(-0.025, 0.125, 0.0) * rz(10.0);
        let pose = Self::arm_pose(grip, s, sq, eq);
        emit_box(
            out,
            Self::arm_part(pose),
            Vec3::new(-3.0, -10.0, -2.0),
            Vec3::new(1.0, 2.0, 2.0),
            ARM_LAYERS.map(|layer| crate::world::textures::skin_layer(layer, skin)),
            [[255; 3]; 6],
            light,
            fl,
        );
        let turn = glam::Quat::from_xyzw(0.2077, -0.6488, 0.4433, 0.5825).normalize();
        let center = Vec3::splat(0.5);
        let block = pose
            * t(-0.684, 0.117, -0.439)
            * Mat4::from_translation(center)
            * Mat4::from_quat(turn)
            * Mat4::from_translation(-center);
        // (held toward the fist's left edge, so like the lantern it hangs out beside the arm)
        let size = 0.55;
        let pivot = block.transform_point3(Vec3::new(0.5, 11.0 / 16.0, 0.5)) - base.transform_vector3(Vec3::X) * (0.18 * size);
        // Its ears toward the view's sides, so the handle is seen across.
        let right = base.transform_vector3(Vec3::X);
        let yaw = (-right.z).atan2(right.x);
        let m = Mat4::from_translation(pivot)
            * self.bucket.swing_matrix()
            * Mat4::from_rotation_y(yaw)
            * rx(-20.0 * f1)
            * Mat4::from_scale(Vec3::splat(size))
            * Mat4::from_translation(Vec3::new(0.0, -bucket::handle_top(), 0.0));
        self.bucket.update(fill, m.transform_point3(Vec3::ZERO), dt);
        let surface = bucket::Surface { tilt: self.bucket.tilt, bounce: self.bucket.bounce, own_up: false };
        bucket::emit(out, m, fill, &surface, 0.0, light, fl);
    }

    /// The guide book held open in both hands, like a map: low in the view while looking
    /// ahead, lifted up in front of the eyes (`read` 1) when looking down, both arms holding
    /// it by its sides. Also finds where the middle of the view falls on its pages.
    #[allow(clippy::too_many_arguments)]
    fn build_book(
        &mut self,
        out: &mut Vec<Vertex>,
        base: Mat4,
        read: f32,
        view: &crate::model::book::BookView,
        eq: f32,
        light: [u8; 4],
        fl: u8,
        skin: u8,
    ) {
        use crate::model::book::{book_hit, emit_open_book, PAGE_H, PAGE_W};
        let r = read * read * (3.0 - 2.0 * read);
        // Camera space (blocks): below the view, lying back; read, upright in front of it.
        let pos = Vec3::new(0.0, -0.64 + 0.62 * r - (1.0 - eq) * 0.5, -0.66 + 0.06 * r);
        // Shown to someone: pushed out and turned away to the side (still readable from
        // here, at a slant), as the others see it turned right around to them.
        let s = view.show * view.show * (3.0 - 2.0 * view.show);
        let pos = pos + Vec3::new(0.18 * s, 0.1 * s, -0.25 * s);
        let book = t(pos.x, pos.y, pos.z)
            * Mat4::from_rotation_y((-58.0 * s).to_radians())
            * Mat4::from_rotation_x((30.0 + 52.0 * r + 10.0 * s).to_radians())
            * Mat4::from_scale(Vec3::splat(0.074));
        emit_open_book(out, base * book, view, light, fl);
        // The hands under the cover, out past the pages' outer edges; shown to someone, they
        // move in to hold its lower edge (which stays toward this player as it turns). Each
        // arm takes the hand on its own side of the view, coming up from below and outside
        // it, so it never covers the pages.
        let grip = |side: f32| {
            let hold = Vec3::new(side * (PAGE_W + 2.2), -2.6, PAGE_H * 0.3);
            let shown = Vec3::new(side * 3.2, -1.4, PAGE_H * 0.5 + 0.5);
            book.transform_point3(hold.lerp(shown, s))
        };
        let (a, b) = (grip(1.0), grip(-1.0));
        let (right, left) = if a.x >= b.x { (a, b) } else { (b, a) };
        for (side, hand) in [(1.0f32, right), (-1.0, left)] {
            let shoulder = Vec3::new(side * 0.75, -1.3, 0.2);
            let along = (shoulder - hand).normalize_or(Vec3::Y);
            let arm = base
                * Mat4::from_translation(hand)
                * Mat4::from_quat(glam::Quat::from_rotation_arc(Vec3::Y, along))
                * Mat4::from_scale(Vec3::splat(1.0 / 20.0));
            emit_box(
                out,
                arm,
                Vec3::new(-2.0, -1.0, -2.0),
                Vec3::new(2.0, 13.0, 2.0),
                ARM_LAYERS.map(|layer| crate::world::textures::skin_layer(layer, skin)),
                [[255; 3]; 6],
                light,
                fl,
            );
        }
        // The middle of the view is straight ahead in camera space.
        self.book_hit = book_hit(book, Vec3::ZERO, Vec3::NEG_Z, view.tabs.is_some());
    }

    /// The held pistol: the Blockbench model (`pistol_vm`) with its animations, mixed from
    /// what the player does: aiming brings the sights to the middle of the view, walking and
    /// running play their loops in step with the feet, a shot plays the slide flying back,
    /// and a reload plays the magazine change (from an empty magazine with the slide released
    /// at the end; otherwise it stops once the new magazine is in and the slide stays
    /// forward). With an empty magazine the slide stays back and the chamber is empty. The
    /// arms are the player's own, where the model has its arms. On top, the game's own
    /// small movements: breathing, leaning into a strafe, trailing a jump, and the crouched
    /// ready pose; looking it over (the inspect key) turns it in the hand.
    #[allow(clippy::too_many_arguments)]
    fn build_gun(
        &mut self,
        kind: GunKind,
        out: &mut Vec<Vertex>,
        base: Mat4,
        light: [u8; 4],
        fl: u8,
        eq: f32,
        skin: u8,
    ) {
        use crate::model::gun_view;
        use crate::model::viewmodel::{add_anim, bone_matrices, find_anim, find_bone};
        use crate::item::gun_mod;
        let smooth = |x: f32| {
            let x = x.clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        };
        let span = |p: f32, a: f32, b: f32| smooth((p - a) / (b - a));
        let cam = base.w_axis.truncate();
        let mods = self.gun_mods;
        let scope = mods & gun_mod::SCOPE != 0;

        // The game's own small movements (the view's bobbing is taken off the gun by the game).
        let a = smooth(self.aim);
        let sprint = smooth(self.sprint) * (1.0 - a);
        let crouch = smooth(self.crouch);
        let still = 1.0 - 0.92 * a;
        let br = self.clock;
        let rest = (1.0 - self.stride.min(1.0) * 0.6) * (1.0 - 0.55 * a);
        let breath = t((br * 0.9).sin() * 0.0028 * rest, (br * 1.7).sin() * 0.0022 * rest, 0.0)
            * rx((br * 1.7).sin() * 0.35 * rest);
        let lag = self.lag.x * still;
        let inertia = t(-lag.x * 0.005, -lag.y * 0.0035, lag.z * 0.003) * rz(-lag.x * 1.4);
        let ready = crouch * (1.0 - a) * (1.0 - sprint);
        let tucked = t(-0.045 * ready, 0.03 * ready, 0.05 * ready);
        // Aimed, a strafe cants the gun a little toward where it goes (around the view's
        // axis, so the sights stay in front of the eye).
        let cant = rz(-self.lag.x.x * 1.5 * a);
        let base = base * breath * inertia * tucked * cant;

        // Blockbench pixels to blocks, the camera at the model's origin; lowered while it is
        // being taken out.
        let px = Mat4::from_scale(Vec3::splat(VIEW_PX));
        // With a scope, aiming brings its eyepiece up to the eye.
        let to_eye = if scope { SCOPE_EYE * a } else { Vec3::ZERO };
        let root = base * t(0.0, -(1.0 - eq) * 0.6, 0.0) * px * Mat4::from_translation(to_eye);
        let bones = gun_view::bones(kind);
        let bone = |name: &str| find_bone(bones, name);
        let anim = |name: &str| find_anim(gun_view::anims(kind), name);
        // Looking it over: out in front of the view, turning slowly in the hand.
        let root = match (self.inspect, Some(gun_view::gun_bone(kind))) {
            (Some(it), Some(pb)) => {
                let w = span(it, 0.0, 0.55) * (1.0 - span(it, INSPECT_TIME - 0.65, INSPECT_TIME));
                let d = |x: f32| x.to_radians();
                use glam::Quat;
                // The right side toward the view, muzzle to the right and a little away, top
                // tipped toward the eye; then the muzzle away and the top up to look down it;
                // then the left side.
                let right = Quat::from_rotation_y(d(-22.0)) * Quat::from_rotation_x(d(16.0));
                let top = Quat::from_rotation_y(d(-32.0)) * Quat::from_rotation_x(d(72.0));
                let left = Quat::from_rotation_y(d(202.0)) * Quat::from_rotation_x(d(16.0));
                let q = if it < 1.4 {
                    right
                } else if it < 2.0 {
                    right.slerp(top, span(it, 1.4, 2.0))
                } else if it < 2.6 {
                    top
                } else if it < 3.2 {
                    top.slerp(left, span(it, 2.6, 3.2))
                } else {
                    left
                };
                let wobble = Quat::from_rotation_z(d((it * 1.4).sin() * 3.0))
                    * Quat::from_rotation_y(d((it * 0.9).sin() * 4.0));
                let center = (gun_view::muzzle(kind, 0).1 + Vec3::from(bones[pb].origin)) * 0.5;
                // A long gun is held further out, to be seen whole.
                let away = if kind.long() { Vec3::new(0.02, -0.1, -1.15) } else { Vec3::new(0.06, -0.07, -0.47) };
                // The model's muzzle points -Z; turned so it points +X like the turns above.
                let held = base
                    * Mat4::from_rotation_translation(wobble * q, away)
                    * ry(-90.0)
                    * px
                    * Mat4::from_translation(-center);
                blend(root, held, smooth(w))
            }
            _ => root,
        };

        // The animations, added up.
        let mut pose = gun_view::rest_pose(kind);
        if let Some(an) = anim("aim") {
            add_anim(&mut pose, an, a * an.length, 1.0, |_| false);
        }
        // Both loops take two steps.
        let phase = (self.walk_dist * 0.5).rem_euclid(1.0);
        let walk = self.stride.min(1.0) * (1.0 - 0.85 * a) * (1.0 - sprint) * (1.0 - 0.4 * crouch);
        if let Some(an) = anim("walk").filter(|_| walk > 1e-3) {
            add_anim(&mut pose, an, phase * an.length, walk, |_| false);
        }
        if let Some(an) = anim("sprint").filter(|_| sprint > 1e-3) {
            add_anim(&mut pose, an, phase * an.length, sprint, |_| false);
        }
        gun_view::add_gun_anims(kind, &mut pose, &self.gun_anim(), mods, false);
        // Held bigger than modelled, about the grip; aimed, moved so the sights stay in the
        // middle of the view.
        let k = gun_view::HELD_SCALE;
        let gb = gun_view::gun_bone(kind);
        pose[gb].scale *= k;
        let pivot = Vec3::from(bones[gb].origin);
        pose[0].pos += (1.0 - k) * (gun_view::sight_point(kind, mods) - pivot) * a;
        let (mats, shown) = bone_matrices(bones, &pose, root);

        // With a scope, its eyepiece always shows the scope's view (held at the hip too).
        if scope {
            self.eyepiece = gun_view::eyepiece(kind, &mats, &shown);
        }
        {
            let mut glass = std::mem::take(&mut self.glass);
            let lamp = mods & gun_mod::LIGHT != 0 && mods & gun_mod::LIGHT_ON != 0;
            gun_view::emit(kind, out, Some(&mut glass), &mats, &shown, self.eyepiece.is_some(), self.gun_dirt, lamp, &self.gun_anim(), light, fl);
            self.glass = glass;
            // The player's own arms where the model has its arms: the fist at the bone's
            // origin, the arm running back along the bone's +Z (Minecraft's arm, 1.75 times
            // as big, longer so it reaches out of the view).
            // (An arm hanging off the gun, the left one, is not held bigger with it.)
            let unit = mats[0].x_axis.length();
            for name in ["right_arm_mesh", "left_arm_mesh"] {
                let Some(b) = bone(name).filter(|&b| shown[b]) else { continue };
                let big = mats[b].x_axis.length() / unit.max(1e-9);
                let arm = mats[b]
                    * Mat4::from_translation(Vec3::from(bones[b].origin))
                    * Mat4::from_scale(Vec3::splat(1.0 / big.max(1e-3)))
                    * rx(90.0)
                    * Mat4::from_scale(Vec3::new(1.75, 2.45, 1.75));
                emit_box(
                    out,
                    arm,
                    Vec3::new(-2.0, -2.0, -2.0),
                    Vec3::new(2.0, 10.0, 2.0),
                    ARM_LAYERS.map(|layer| crate::world::textures::skin_layer(layer, skin)),
                    [[255; 3]; 6],
                    light,
                    fl,
                );
            }
        }

        // Where the bullet and the flash leave, where the case comes out, where the laser
        // starts.
        let (mb, mp) = gun_view::muzzle(kind, mods);
        let muzzle = mats[mb].transform_point3(mp);
        self.muzzle_tip = Some(muzzle);
        let dir = mats[mb].transform_vector3(Vec3::NEG_Z).normalize_or(Vec3::NEG_Z);
        self.barrel_dir = Some(dir);
        if self.flash > 0.0 {
            let size = 0.11 * self.flash_size * k;
            crate::model::ballistics::emit_muzzle_flash(out, muzzle, dir, cam, size, self.flash_seed, self.flash);
        }
        let (eb, ep) = gun_view::eject(kind);
        self.eject_tip = Some(mats[eb].transform_point3(ep));
        self.chamber_tips = (!kind.uses_magazine()).then(|| {
            std::array::from_fn(|c| {
                let (b, p) = crate::model::revolver_view::chamber_head(c);
                mats[b].transform_point3(p)
            })
        });
        if mods & gun_mod::LASER != 0 {
            let (lb, lp) = gun_view::laser(kind);
            self.laser_tip = Some(mats[lb].transform_point3(lp));
        }
        if mods & gun_mod::LIGHT != 0 {
            let (lb, lp) = gun_view::light(kind);
            self.light_tip = Some(mats[lb].transform_point3(lp));
        }
    }
}

/// Between two rigid (uniformly scaled) transforms: position and size linearly, rotation along
/// the shortest arc, so the item turns smoothly instead of being squashed.
impl HandAnim {
    /// A grenade being readied, `t` seconds after the button went down: it comes up in
    /// front, the left hand reaches for the ring and pulls the pin out (the model's
    /// `pull_pin`), carrying it off out of the view; then the grenade is drawn back by the
    /// shoulder, higher and higher with the throw's `power` (0..1), up to the highest once it
    /// would go the farthest. Both arms are the player's own, coming up from below.
    #[allow(clippy::too_many_arguments)]
    fn build_grenade_hold(&mut self, out: &mut Vec<Vertex>, base: Mat4, held: f32, power: f32, light: [u8; 4], fl: u8, skin: u8) {
        use crate::model::grenade::{self, Look, PULL_TIME, RAISE_TIME as RAISE};
        let smooth = |x: f32| {
            let x = x.clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        };
        // Held low as always, up in front for the pull, then drawn back to throw: a little
        // back at first, going up and further back as the throw gets harder (with a little
        // unrest while waiting there).
        let rest = base * rest_hold();
        let front = base * t(0.12, -0.19, -0.58) * ry(-18.0) * rx(8.0) * rz(-4.0);
        let wait = (held * 2.3).sin() * 0.005;
        let low = base * t(0.28, -0.16 + wait, -0.57) * ry(-22.0) * rx(-6.0) * rz(-8.0);
        let high = base * t(0.33, 0.07 + wait, -0.5) * ry(-30.0) * rx(-34.0) * rz(-14.0);
        let cocked = blend(low, high, smooth(power));
        let pull = held - RAISE;
        let m = blend(rest, front, smooth(held / RAISE));
        let m = blend(m, cocked, smooth((pull - 0.55) / 0.3));
        let item = m * Mat4::from_scale(Vec3::splat(0.36));
        let smoke = self.held == crate::item::SMOKE_GRENADE;
        let ring = grenade::emit(out, smoke, grenade::sized(smoke, item, 0.62), Look::readied(held), light, fl);
        self.grenade_tip = Some(item.transform_point3(Vec3::ZERO));

        let inv = base.inverse();
        let layers = ARM_LAYERS.map(|layer| crate::world::textures::skin_layer(layer, skin));
        let mut arm = |hand: Vec3, shoulder: Vec3| {
            let along = (shoulder - hand).normalize_or(Vec3::Y);
            let m = base
                * Mat4::from_translation(hand)
                * Mat4::from_quat(glam::Quat::from_rotation_arc(Vec3::Y, along))
                * Mat4::from_scale(Vec3::splat(1.0 / 28.0));
            let (lo, hi) = (Vec3::new(-2.0, -1.0, -2.0), Vec3::new(2.0, 20.0, 2.0));
            emit_box(out, m, lo, hi, layers, [[255; 3]; 6], light, fl);
        };
        // The right fist under it.
        let grip = inv.transform_point3(item.transform_point3(Vec3::new(0.0, -0.36, 0.04)));
        arm(grip, Vec3::new(0.55, -1.0, -0.15));
        // The left hand only suggests the pull (as the revolver's loading hand does): it
        // comes up near the ring, short of it, and goes off down with the pin.
        if pull < PULL_TIME {
            let start = Vec3::new(-0.35, -0.75, -0.45);
            let reach = smooth((held - 0.05) / (RAISE + 0.08 - 0.05));
            let shoulder = Vec3::new(-0.5, -1.0, -0.15);
            let ring = inv.transform_point3(ring);
            let near = ring + (shoulder - ring).normalize_or(Vec3::NEG_Y) * 0.14;
            arm(start.lerp(near, reach), shoulder);
        }
    }
}

impl HandAnim {
    /// The fishing rod in the right hand (see `angler`), the left hand on the reel's handle
    /// (off it while the rod is swung). Both arms are the player's own, from below.
    #[allow(clippy::too_many_arguments)]
    fn build_rod(&mut self, out: &mut Vec<Vertex>, base: Mat4, eq: f32, light: [u8; 4], fl: u8, skin: u8) {
        use crate::model::angler;
        let a = self.rod.unwrap_or_default();
        let inv = base.inverse();
        let bobber = a.bobber.map(|b| inv.transform_point3(b));
        let (m, pose) = angler::first_person(&a, self.clock, bobber, eq);
        let p = crate::model::angler::emit_rod(out, base * m, &pose, light, fl);
        self.rod_tip = Some(p.tip);
        let layers = ARM_LAYERS.map(|layer| crate::world::textures::skin_layer(layer, skin));
        let mut arm = |hand: Vec3, shoulder: Vec3| {
            let along = (shoulder - hand).normalize_or(Vec3::Y);
            let m = base
                * Mat4::from_translation(hand)
                * Mat4::from_quat(glam::Quat::from_rotation_arc(Vec3::Y, along))
                * Mat4::from_scale(Vec3::splat(1.0 / 34.0));
            emit_box(out, m, Vec3::new(-2.0, -1.5, -2.0), Vec3::new(2.0, 26.0, 2.0), layers, [[255; 3]; 6], light, fl);
        };
        // The right fist around the grip, a little under it.
        let grip = inv.transform_point3(p.grip) - Vec3::new(0.0, 0.02, 0.0);
        arm(grip, Vec3::new(0.55, -1.0, -0.1));
        // The left hand on the handle's knob, or off to the side while the rod swings.
        let knob = inv.transform_point3(p.crank);
        let off = angler::hand_off_crank(&a);
        let free = angler::first_person_free_hand() + Vec3::new(0.0, -(1.0 - eq) * 0.6, 0.0);
        arm(knob.lerp(free, off), Vec3::new(-0.45, -1.0, -0.1));
    }
}

/// Where a gun's part, a magazine, a round or a grenade is held when nothing is done with it
/// (model item space, before its scale): low in the right hand, turned a little.
fn rest_hold() -> Mat4 {
    t(0.3, -0.3, -0.56) * ry(-32.0) * rx(14.0) * rz(-6.0)
}

fn blend(a: Mat4, b: Mat4, k: f32) -> Mat4 {
    let (sa, ra, ta) = a.to_scale_rotation_translation();
    let (sb, rb, tb) = b.to_scale_rotation_translation();
    Mat4::from_scale_rotation_translation(sa.lerp(sb, k), ra.slerp(rb, k), ta.lerp(tb, k))
}

#[cfg(test)]
mod lantern_view_tests {
    use super::*;

    #[test]
    fn first_person_lantern_trails_a_moving_hand() {
        let mut hand = HandAnim::new();
        hand.equip(crate::world::LANTERN as ItemId);
        hand.fancy_lantern = true;
        let dt = 1.0 / 60.0;
        let mut verts = Vec::new();
        for _ in 0..60 {
            hand.update(dt, false, 0.0, true, Vec2::ZERO);
            verts.clear();
            hand.build(&mut verts, Mat4::IDENTITY, 15, 15, dt, 0);
        }
        let center_x =
            |v: &[Vertex]| v.iter().skip(36).map(|p| p.pos[0]).sum::<f32>() / (v.len() - 36) as f32;
        let rest_x = center_x(&verts);
        for step in 1..=10 {
            verts.clear();
            hand.build(
                &mut verts,
                Mat4::from_translation(Vec3::X * (step as f32 * 0.08)),
                15,
                15,
                dt,
                0,
            );
        }
        assert!(
            center_x(&verts) - 0.8 < rest_x - 0.008,
            "lantern did not trail the hand (rest={}, moving={})",
            rest_x,
            center_x(&verts) - 0.8
        );
    }
}

#[cfg(test)]
mod pistol_view_tests {
    use super::*;

    /// The hand holding the pistol after `secs` of frames, built in camera space.
    fn held(secs: f32, setup: impl Fn(&mut HandAnim)) -> (HandAnim, Vec<Vertex>) {
        let mut hand = HandAnim::new();
        hand.equip(crate::item::PISTOL);
        let dt = 1.0 / 60.0;
        let mut verts = Vec::new();
        for _ in 0..(secs / dt) as usize {
            setup(&mut hand);
            hand.update(dt, false, 0.0, true, Vec2::ZERO);
            verts.clear();
            hand.build(&mut verts, Mat4::IDENTITY, 15, 15, dt, 0);
        }
        (hand, verts)
    }

    #[test]
    fn the_blockbench_pistol_is_drawn_in_front_of_the_eye() {
        let (hand, verts) = held(1.0, |_| {});
        assert!(verts.len() > 2000, "{} vertices", verts.len());
        for v in &verts {
            assert!(v.pos.iter().all(|c| c.is_finite()));
        }
        // Held from the hip: down to the right, in front.
        let muzzle = hand.muzzle_tip.unwrap();
        assert!(muzzle.z < -0.3 && muzzle.y < 0.0 && muzzle.x > 0.0, "{muzzle}");
        // The case comes out of the port, behind the muzzle.
        let port = hand.eject_tip.unwrap();
        assert!(port.z > muzzle.z, "{port} {muzzle}");
    }

    #[test]
    fn aiming_brings_the_sights_to_the_middle() {
        let (hand, _) = held(1.0, |h| h.aim = 1.0);
        let m = hand.muzzle_tip.unwrap();
        // The muzzle is just under the view's axis, straight ahead.
        assert!(m.x.abs() < 0.02 && m.y < 0.0 && m.y > -0.08 && m.z < -0.3, "{m}");
    }

    #[test]
    fn a_shot_and_a_reload_play_through() {
        let (mut hand, _) = held(0.3, |_| {});
        hand.shoot(1.0, 0.3);
        let (_, verts) = {
            let dt = 1.0 / 60.0;
            let mut verts = Vec::new();
            for i in 0..120 {
                hand.reload = Some(i as f32 / 120.0);
                hand.gun_state.rack = true;
                hand.update(dt, false, 0.0, true, Vec2::ZERO);
                verts.clear();
                hand.build(&mut verts, Mat4::IDENTITY, 15, 15, dt, 0);
            }
            ((), verts)
        };
        assert!(verts.len() > 2000);
    }
}
