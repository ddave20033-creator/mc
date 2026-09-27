//! Guns: shooting (left mouse button; held down for automatic fire), aiming down the sights
//! (right), reloading (a magazine, or a shotgun's shells one by one), working a bolt or a
//! pump, bullets in flight, pellets, spent cases, recoil, the laser sight and the gun HUD
//! (crosshair, scope, ammo), and the state of the gun station screen (drawn in
//! `gui::gun_station`), where guns are put together part by part, cleaned and tuned.

use super::*;
use crate::entity::player::{ray_boxes, raycast_solid};
use crate::item::*;
use crate::lang::tf;
use crate::model::ballistics::{self, CaseKind, Cases};
use crate::audio::Sound;
use crate::model::gun::{self, PARTS};

/// Seconds to raise the gun to the eye.
const AIM_TIME: f32 = 0.16;
/// Every shot widens the cone of the next ones for a moment (degrees).
const BLOOM_PER_SHOT: f32 = 1.3;
const BLOOM_MAX: f32 = 6.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum BenchMode {
    Assemble,
    Clean,
    Tune,
}

/// A bullet (or a pellet) in flight.
struct Bullet {
    pos: Vec3,
    vel: Vec3,
    /// Where it was fired from (knocks what it hits away from there).
    from: Vec3,
    /// It flies from the eye, but is drawn starting at the muzzle: this offset fades out
    /// over the first blocks.
    offset: Vec3,
    traveled: f32,
    range: f32,
    gravity: f32,
    damage: f32,
    knockback: f32,
    /// Another player's, only to be seen: what it hits is up to them.
    visual: bool,
    /// A shotgun pellet (it leaves a smaller hole).
    small: bool,
}

#[derive(Default)]
pub(super) struct Guns {
    /// Where the held gun's muzzle, ejection port and laser lens were drawn last frame
    /// (first person).
    pub(super) muzzle: Option<Vec3>,
    pub(super) eject: Option<Vec3>,
    pub(super) laser_from: Option<Vec3>,
    /// The same on the player model (third person).
    pub(super) muzzle_tp: Option<Vec3>,
    pub(super) eject_tp: Option<Vec3>,
    /// The last muzzle flash as the world sees it (third person; the first-person view draws
    /// its own on the gun): how much is left, where, which way, how big, its turn.
    flash: Option<(f32, Vec3, Vec3, f32, f32)>,
    /// The flash's light on the surroundings: how much is left and where.
    pub(super) flash_light: (f32, Vec3),
    /// How hot the barrel is from firing (smoke curls out of it above 1), and when the last
    /// wisp came out.
    heat: f32,
    wisp: f32,
    /// When the last "jammed" or "no bullets" message was shown.
    last_message: f32,
    pub(super) bench: Bench,
    /// Aimed down the sights: 0 from the hip .. 1 aimed.
    pub(super) aim: f32,
    /// Reloading: seconds so far (for a shotgun, of the shell going in), and whether the
    /// magazine was empty (a pistol's slide is released at the end).
    pub(super) reload: Option<f32>,
    reload_empty: bool,
    /// The reload key was pressed.
    pub(super) reload_pressed: bool,
    /// Working the bolt or the pump after a shot: seconds so far; the case comes out halfway.
    cycle: Option<f32>,
    cycle_ejected: bool,
    /// Extra spread from the last shots (degrees), for the aim and the crosshair.
    bloom: f32,
    /// Recoil that has not come back yet (radians of pitch).
    recover: f32,
    bullets: Vec<Bullet>,
    pub(super) cases: Cases,
    /// Where the laser sight's dot is.
    laser_dot: Option<Vec3>,
    /// Muzzle flashes of the other players' shots (as `flash`).
    remote_flashes: Vec<(f32, Vec3, Vec3, f32, f32)>,
    /// Holes the bullets left in the blocks.
    holes: Vec<Hole>,
}

/// A bullet hole: where on which face of which block (it goes when the block does), how it
/// is turned, how big, and when it was made.
struct Hole {
    pos: Vec3,
    normal: Vec3,
    block: IVec3,
    id: u8,
    turn: f32,
    size: f32,
    born: f32,
}

/// Bullet holes kept at most, and for how long (they shrink away over the last seconds).
const MAX_HOLES: usize = 300;
const HOLE_LIFE: f32 = 120.0;

pub(super) struct Bench {
    pub(super) mode: BenchMode,
    /// The gun being put together, the parts put in so far (in order), which of them the
    /// player paid for (in creative they are free), and when the last one went in.
    pub(super) kind: GunKind,
    pub(super) installed: usize,
    pub(super) taken: [bool; PARTS],
    pub(super) installed_at: f32,
    /// A gun was just finished: it stays on show until the next one is started.
    pub(super) finished: bool,
    /// The gun being cleaned or tuned, how dirty each of its parts is (0 clean .. 1), and the
    /// gun's dirt when `dirt` was last matched to it (another gun put in reloads it).
    pub(super) gun: Slot,
    pub(super) dirt: [f32; PARTS],
    pub(super) dirt_of: Option<(ItemId, u16)>,
    /// The gun floating over the table while it is put together: turned by dragging with the
    /// right mouse button (it sways by itself until then).
    pub(super) yaw: f32,
    pub(super) turned: bool,
    pub(super) drag_from: Option<Vec2>,
    /// A part was clicked but is not in the inventory: when (flashes its line red).
    pub(super) missing_at: f32,
    /// When the gun being cleaned or tuned was laid on the table (the hand puts it down), and
    /// the inventory slot it came from (it goes back there).
    pub(super) placed_at: f32,
    pub(super) from_slot: Option<usize>,
    /// Whether a gun lay on the table last frame (to notice one put there with the mouse).
    pub(super) had_gun: bool,
    /// The hand fitting an attachment or taking one off.
    pub(super) fit: Option<FitAnim>,
    /// What the mouse points at on the table.
    pub(super) hover: Option<Pick>,
    /// The hand with the brush: 0 away .. 1 scrubbing at `scrub_point`.
    pub(super) brush: f32,
    pub(super) scrub_point: Vec3,
}

/// Something on the gun station's table under the mouse.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Pick {
    /// A part (lying on the table, or on the gun being put together).
    Part(usize),
    /// The gun itself.
    Gun,
    /// An attachment (index into ATTACHMENTS), on the gun or lying beside it.
    Attachment(usize),
    /// The bare table top.
    Table,
}

/// The hand fitting an attachment onto the gun on the table, or taking one off.
#[derive(Clone, Copy, Debug)]
pub(super) struct FitAnim {
    pub(super) index: usize,
    pub(super) removing: bool,
    pub(super) start: f32,
}

impl Default for Bench {
    fn default() -> Self {
        Self {
            mode: BenchMode::Assemble,
            kind: GunKind::Pistol,
            installed: 0,
            taken: [false; PARTS],
            installed_at: -10.0,
            finished: false,
            gun: None,
            dirt: [0.0; PARTS],
            dirt_of: None,
            yaw: 0.0,
            turned: false,
            drag_from: None,
            missing_at: -10.0,
            placed_at: -10.0,
            from_slot: None,
            had_gun: false,
            fit: None,
            hover: None,
            brush: 0.0,
            scrub_point: Vec3::ZERO,
        }
    }
}

impl Bench {
    /// What the parts put in so far were made of (it goes back to the player when assembling
    /// stops): the pistol's own part items, or the materials of the others.
    pub(super) fn parts(&self) -> Vec<Stack> {
        let mut out = Vec::new();
        for (i, part) in self.kind.parts().iter().enumerate().take(self.installed.min(PARTS)) {
            if !self.taken[i] || self.finished {
                continue;
            }
            match part.item {
                Some(item) => out.push(Stack::one(item)),
                None => out.extend(part.cost.iter().map(|&(item, n)| Stack::new(item, n))),
            }
        }
        out
    }

    /// Everything on the bench that belongs to the player: what the parts put in so far were
    /// made of, and the gun being cleaned or tuned.
    pub(super) fn items(&self) -> Vec<Stack> {
        let mut out = self.parts();
        out.extend(self.gun);
        out
    }

    /// Stops assembling (after the parts went back to the player).
    pub(super) fn reset_assembly(&mut self) {
        self.installed = 0;
        self.taken = [false; PARTS];
        self.finished = false;
    }

    /// Empties the bench (after its items went back to the player).
    pub(super) fn clear(&mut self) {
        let (mode, kind) = (self.mode, self.kind);
        *self = Self {
            mode,
            kind,
            ..Self::default()
        };
    }
}

impl Guns {
    /// The light of a muzzle flash going on now: where it is and how bright.
    pub(super) fn flash_light_pos(&self) -> Option<(Vec3, f32)> {
        let (k, pos) = self.flash_light;
        (k > 0.0).then_some((pos, 2.5 * k))
    }
}

/// How far the shots scatter (degrees; `aim` 0 from the hip .. 1 aimed). A single bullet
/// from a steady gun goes exactly where the crosshair is; only shots in quick succession
/// scatter. A shotgun's pellets always spread in a cone around it.
fn shot_spread(stats: &crate::item::Stats, mods: u8, aim: f32, bloom: f32) -> f32 {
    let bloom = bloom * (1.0 - 0.7 * aim);
    if stats.pellets <= 1 {
        return bloom;
    }
    let hip = if mods & gun_mod::LASER != 0 {
        stats.spread_laser
    } else {
        stats.spread_hip
    };
    hip + (stats.spread_aimed - hip) * aim + bloom
}

/// `dir` raised just enough that a bullet at `speed`, falling with `gravity`, comes down on
/// the line of sight `dist` blocks away.
fn zeroed(dir: Vec3, dist: f32, speed: f32, gravity: f32) -> Vec3 {
    let t = dist / speed.max(1.0);
    (dir * dist + Vec3::Y * 0.5 * gravity * t * t).normalize_or(dir)
}

/// What a gun sounds like firing.
fn shot_sound(kind: GunKind, silenced: bool) -> Sound {
    if silenced {
        return Sound::ShotSilenced;
    }
    match kind {
        GunKind::Pistol => Sound::ShotPistol,
        GunKind::DesertEagle => Sound::ShotMagnum,
        GunKind::M16 => Sound::ShotRifle,
        GunKind::Sniper => Sound::ShotSniper,
        GunKind::Shotgun => Sound::ShotShotgun,
    }
}

/// The case a gun throws out.
fn case_kind(kind: GunKind) -> CaseKind {
    match kind {
        GunKind::Pistol => CaseKind::Pistol,
        GunKind::DesertEagle => CaseKind::Magnum,
        GunKind::M16 => CaseKind::Rifle,
        GunKind::Sniper => CaseKind::Bmg,
        GunKind::Shotgun => CaseKind::Shell,
    }
}

/// A random direction within `deg` degrees of `dir` (more often near the middle).
fn scatter(dir: Vec3, deg: f32, r1: f32, r2: f32) -> Vec3 {
    let right = dir.cross(Vec3::Y).normalize_or_zero();
    let up = right.cross(dir);
    let angle = r1 * TAU;
    let off = deg.to_radians().tan() * r2.sqrt();
    (dir + (right * angle.cos() + up * angle.sin()) * off).normalize()
}

impl Game {
    /// The held gun: its stack and kind.
    fn held_gun(&self) -> Option<(Stack, GunKind)> {
        self.inventory.slots[self.hotbar_slot].and_then(|s| GunKind::of(s.item).map(|k| (s, k)))
    }

    /// Holding a gun.
    pub(super) fn holding_gun(&self) -> bool {
        self.held_gun().is_some()
    }

    /// The held gun's attachments (0 when holding something else).
    pub(super) fn held_gun_mods(&self) -> u8 {
        self.held_gun().map_or(0, |(s, _)| gun_mods(&s))
    }

    /// How much the gun narrows the view now (1 = not at all).
    pub(super) fn gun_zoom(&self) -> f32 {
        let Some((s, kind)) = self.held_gun() else {
            return 1.0;
        };
        let stats = kind.stats();
        let full = if kind.shown_mods(gun_mods(&s)) & gun_mod::SCOPE != 0 {
            stats.scope_zoom
        } else {
            stats.sight_zoom
        };
        let a = smoothstep(0.0, 1.0, self.guns.aim);
        1.0 + (full - 1.0) * a
    }

    /// Aiming, reloading, the bolt or pump, recoil coming back, bullets, spent cases and the
    /// laser, every frame.
    pub(super) fn update_guns(&mut self, dt: f32, control: bool) {
        // A spectator's hands are empty; the shots, cases and holes around still go on.
        let held = self.held_gun().filter(|_| !self.spectator());
        let mods = held.map_or(0, |(s, _)| gun_mods(&s));
        let reload_pressed = std::mem::take(&mut self.guns.reload_pressed);
        if held.is_none() {
            self.guns.reload = None;
            self.guns.cycle = None;
        } else if reload_pressed && control {
            self.start_reload();
        }
        let g = &mut self.guns;
        let aiming = held.is_some() && control && self.right_down && g.reload.is_none();
        let step = dt / AIM_TIME;
        g.aim = if aiming {
            (g.aim + step).min(1.0)
        } else {
            (g.aim - step).max(0.0)
        };
        g.bloom = (g.bloom - dt * 5.0).max(0.0);
        let back = g.recover.min(dt * 0.12);
        g.recover -= back;
        self.pitch -= back;

        if let (Some(t), Some((_, kind))) = (self.guns.reload, held) {
            let length = kind.stats().reload;
            let (was, t) = (t / length, t + dt);
            let now = t / length;
            let at = self.player.eye();
            let crossed = |k: f32| was < k && now >= k;
            if kind.stats().shells {
                if crossed(0.45) {
                    self.audio.play(Sound::ShellIn, Some(at), 0.8);
                }
            } else {
                if crossed(0.12) {
                    self.audio.play(Sound::MagOut, Some(at), 0.8);
                }
                if crossed(0.6) {
                    self.audio.play(Sound::MagIn, Some(at), 0.9);
                }
                if crossed(0.84) && self.guns.reload_empty {
                    self.audio.play(Sound::SlideRelease, Some(at), 0.9);
                }
            }
            if t >= length {
                self.guns.reload = None;
                self.finish_reload();
            } else {
                self.guns.reload = Some(t);
            }
        }
        // The bolt or the pump: the spent case comes out halfway.
        if let (Some(t), Some((_, kind))) = (self.guns.cycle, held) {
            let length = kind.stats().cycle;
            let start = length * 0.12;
            if t < start && t + dt >= start {
                let sound = if kind == GunKind::Shotgun { Sound::PumpCycle } else { Sound::BoltCycle };
                self.audio.play(sound, Some(self.player.eye()), 0.9);
            }
            let t = t + dt;
            if t >= length * 0.35 && !self.guns.cycle_ejected {
                self.guns.cycle_ejected = true;
                self.eject_case(kind);
            }
            self.guns.cycle = (t < length).then_some(t);
        }

        let kind = held.map(|(_, k)| k);
        self.hand.aim = self.guns.aim;
        self.hand.reload = match (self.guns.reload, kind) {
            (Some(t), Some(k)) => Some(t / k.stats().reload),
            _ => None,
        };
        self.hand.cycle = match (self.guns.cycle, kind) {
            (Some(t), Some(k)) => Some(t / k.stats().cycle),
            _ => None,
        };
        self.hand.gun_mods = mods;
        self.hand.gun_empty = held.is_some_and(|(g, _)| gun_rounds(&g) == 0);
        self.hand.reload_empty = self.guns.reload_empty;

        self.update_bullets(dt);
        for (at, shell, hard) in self.guns.cases.update(dt, &self.terrain.world) {
            let sound = if shell { Sound::CaseShell } else { Sound::CaseBrass };
            self.audio.play(sound, Some(at), 0.25 + 0.75 * hard);
        }

        // The flash fades in a moment; a hot barrel smokes.
        if let Some(f) = &mut self.guns.flash {
            f.0 -= dt / 0.06;
        }
        if self.guns.flash.is_some_and(|f| f.0 <= 0.0) {
            self.guns.flash = None;
        }
        for f in &mut self.guns.remote_flashes {
            f.0 -= dt / 0.06;
        }
        self.guns.remote_flashes.retain(|f| f.0 > 0.0);
        // Holes go with their block, and after a while.
        let (time, world) = (self.time, &self.terrain.world);
        self.guns
            .holes
            .retain(|h| time - h.born < HOLE_LIFE && world.geti(h.block) == h.id);
        self.guns.flash_light.0 = (self.guns.flash_light.0 - dt / 0.08).max(0.0);
        self.guns.heat = (self.guns.heat - dt * 0.5).max(0.0);
        self.guns.wisp -= dt;
        if held.is_some() && self.guns.heat > 2.0 && self.guns.wisp <= 0.0 {
            self.guns.wisp = 0.35;
            if let Some(m) = self.muzzle_now() {
                let (sky, blk) = self.terrain.world.light_estimate(m);
                self.particles.gun_smoke(m, Vec3::Y * 0.4, 1, sky, blk);
            }
        }

        // The laser points where the view does; its dot sits on whatever is there.
        self.guns.laser_dot = None;
        if let Some((_, kind)) = held.filter(|_| mods & gun_mod::LASER != 0 && self.screen != Screen::Dead) {
            let range = kind.stats().range.min(120.0);
            let eye = self.player.eye();
            let dir = look_dir(self.yaw, self.pitch);
            self.guns.laser_dot = self.laser_hit(eye, dir, range, None);
        }
    }

    /// Starts reloading the held gun, if its magazine is not full and there are rounds for it.
    fn start_reload(&mut self) {
        let Some((gun, kind)) = self.held_gun() else { return };
        if self.guns.reload.is_some() {
            return;
        }
        if gun_rounds(&gun) >= kind.magazine_size(gun_mods(&gun)) {
            return;
        }
        if !self.creative() && self.inventory.count(kind.ammo()) == 0 {
            self.gun_message(t("gun.no_ammo"));
            return;
        }
        self.guns.aim = 0.0;
        self.guns.reload = Some(0.0);
        self.guns.reload_empty = gun_rounds(&gun) == 0;
    }

    /// A reload is done: the new magazine is filled from the inventory (free in creative), or
    /// one shell went into a shotgun (then the next one follows while there is room).
    fn finish_reload(&mut self) {
        let slot = self.hotbar_slot;
        let Some((mut gun, kind)) = self.held_gun() else {
            return;
        };
        let have = gun_rounds(&gun);
        let room = kind.magazine_size(gun_mods(&gun)).saturating_sub(have);
        let want = if kind.stats().shells { room.min(1) } else { room };
        let mut got = 0;
        while got < want && (self.creative() || self.inventory.remove_one(kind.ammo())) {
            got += 1;
        }
        set_gun_rounds(&mut gun, have + got);
        if self.inventory.slots[slot].is_some_and(|s| s.item == gun.item) {
            self.inventory.slots[slot] = Some(gun);
        }
        // A shotgun keeps loading while there is room and shells.
        let more = self.creative() || self.inventory.count(kind.ammo()) > 0;
        if kind.stats().shells && got > 0 && room > 1 && more {
            self.guns.reload = Some(0.0);
        }
    }

    /// Left click with a gun: fires (a dirty gun may jam, an empty one reloads). A shotgun
    /// being loaded stops loading to fire.
    pub(super) fn shoot(&mut self) {
        let Some((gun, kind)) = self.held_gun() else { return };
        let stats = kind.stats();
        if stats.shells && self.guns.reload.is_some() && gun_rounds(&gun) > 0 {
            self.guns.reload = None;
        }
        if self.action_cooldown > 0.0 || self.guns.reload.is_some() || self.guns.cycle.is_some() {
            return;
        }
        let slot = self.hotbar_slot;
        self.action_cooldown = stats.fire_delay;
        if gun_rounds(&gun) == 0 {
            if self.creative() || self.inventory.count(kind.ammo()) > 0 {
                self.start_reload();
            } else {
                self.gun_message(t("gun.no_ammo"));
                self.audio.play(Sound::DryFire, None, 0.8);
            }
            return;
        }
        let dirt = gun.damage as f32 / stats.dirt_max as f32;
        // Dirt makes it jam more and more often; a completely dirty one does not fire at all.
        if dirt >= 1.0 || (dirt > 0.6 && self.random() < (dirt - 0.6) * 1.2) {
            self.gun_message(t("gun.jammed"));
            self.audio.play(Sound::DryFire, None, 0.8);
            return;
        }
        let mods = gun_mods(&gun);
        let silenced = mods & gun_mod::SILENCER != 0;
        let creative = self.creative();
        if let Some(s) = &mut self.inventory.slots[slot] {
            set_gun_rounds(s, gun_rounds(s) - 1);
            if !creative {
                s.damage = (s.damage + 1).min(stats.dirt_max);
            }
        }
        let seed = self.random();
        self.hand.shoot(if silenced { 0.0 } else { stats.flash }, seed);

        let eye = self.player.eye();
        let look = look_dir(self.yaw, self.pitch);
        let aim = smoothstep(0.0, 1.0, self.guns.aim);
        let spread = shot_spread(stats, mods, aim, self.guns.bloom);
        self.guns.bloom = (self.guns.bloom + BLOOM_PER_SHOT).min(BLOOM_MAX);
        // What the crosshair is on: the bullets are aimed a little high to drop onto it.
        let range = stats.range;
        let target = self
            .laser_hit(eye, look, range, None)
            .map_or(range, |p| p.distance(eye) + 0.03);

        let muzzle = self.muzzle_now().unwrap_or(eye + look * 0.9);
        let mut sent = Vec::new();
        for _ in 0..stats.pellets {
            let (r1, r2) = (self.random(), self.random());
            // A silencer slows the bullet a little.
            let speed = stats.speed * if mods & gun_mod::SILENCER != 0 { 0.9 } else { 1.0 };
            let dir = zeroed(scatter(look, spread, r1, r2), target, speed, stats.gravity);
            sent.push(dir * speed);
            self.guns.bullets.push(Bullet {
                pos: eye,
                vel: dir * speed,
                from: self.player.pos,
                offset: muzzle - eye,
                traveled: 0.0,
                range: stats.range,
                gravity: stats.gravity,
                damage: stats.damage,
                knockback: stats.knockback,
                visual: false,
                small: stats.pellets > 1,
            });
        }
        // The others see the shot too.
        let shot = crate::net::Msg::Shot {
            id: super::multi::HOST_ID,
            kind: kind as u8,
            mods,
            eye,
            seed,
            bullets: sent,
        };
        if self.is_client() {
            self.send(shot);
        } else {
            self.broadcast(&shot, None);
        }

        // Recoil: the view kicks up (and a little to the side); most of it comes back.
        let kick = (stats.kick_hip + (stats.kick_aimed - stats.kick_hip) * aim).to_radians();
        self.pitch = (self.pitch + kick).min(1.55);
        self.guns.recover += kick * 0.6;
        self.yaw += (self.random() - 0.5) * (0.4 + kick.to_degrees() * 0.15).to_radians();

        // A self-loading gun throws its case out now; a bolt or a pump is worked first.
        if stats.cycle > 0.0 && gun_rounds(&gun) > 1 {
            self.guns.cycle = Some(0.0);
            self.guns.cycle_ejected = false;
        } else if stats.cycle > 0.0 {
            // The last round: the case stays in until the reload.
        } else {
            self.eject_case(kind);
        }

        self.audio.play(shot_sound(kind, silenced), Some(muzzle), 1.0);

        // Muzzle flash, its light, sparks and a puff of smoke (a silencer leaves only a
        // little smoke). The barrel heats up.
        let (sky, blk) = self.terrain.world.light_estimate(muzzle);
        let size = stats.flash;
        if !silenced {
            self.guns.flash = Some((1.0, muzzle, look, 0.1 * size, seed));
            self.guns.flash_light = ((0.6 + 0.2 * size).min(1.0), muzzle + look * 0.3);
            self.particles.sparks(muzzle, look, 3 + (size * 3.0) as usize);
        }
        let puff = if size > 1.5 { 2 } else { 1 };
        self.particles.gun_smoke(muzzle, look, puff, sky, blk);
        self.guns.heat = (self.guns.heat + 0.07 * size.max(0.8)).min(3.0);
    }

    /// Where the held gun's muzzle is now: on the first-person gun, or on the player model.
    fn muzzle_now(&self) -> Option<Vec3> {
        if self.camera.mode == 0 {
            self.guns.muzzle
        } else {
            self.guns.muzzle_tp
        }
    }

    /// A spent case (or shotgun hull) flies out to the right of the gun, tumbling.
    fn eject_case(&mut self, kind: GunKind) {
        let eye = self.player.eye();
        let look = look_dir(self.yaw, self.pitch);
        let right = look.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(look);
        let port = match (self.camera.mode, self.guns.eject, self.guns.eject_tp) {
            (0, Some(p), _) | (_, _, Some(p)) => p,
            _ => eye - Vec3::Y * 0.25 + look * 0.5 + right * 0.25,
        };
        let r = |g: &mut Self| g.random() - 0.5;
        let vel = right * (2.4 + r(self)) + up * (2.6 + r(self)) - look * 0.6 + self.player.vel * 0.8;
        let spin = Vec3::new(r(self), r(self), r(self)) * 40.0;
        self.guns.cases.eject(port, vel, spin, case_kind(kind));
    }

    /// Another player fired (`kind` is the gun's index in GUN_KINDS): their bullets fly
    /// (only to be seen), the muzzle flashes on the gun in their hands and the case comes
    /// out of it.
    pub(super) fn remote_shot(&mut self, id: u8, kind: u8, mods: u8, eye: Vec3, seed: f32, bullets: &[Vec3]) {
        let Some(&kind) = GUN_KINDS.get(kind as usize) else {
            return;
        };
        if eye.distance(self.player.pos) > 192.0 || bullets.is_empty() {
            return;
        }
        let stats = kind.stats();
        let look = bullets[0].normalize_or(Vec3::X);
        let muzzle = self
            .remote_gun_point(id, kind, gun::muzzle(kind, mods))
            .unwrap_or(eye + look * 0.9);
        for &vel in bullets.iter().take(32) {
            self.guns.bullets.push(Bullet {
                pos: eye,
                vel,
                from: eye,
                offset: muzzle - eye,
                traveled: 0.0,
                range: stats.range,
                gravity: stats.gravity,
                damage: 0.0,
                knockback: 0.0,
                visual: true,
                small: stats.pellets > 1,
            });
        }
        let silenced = mods & gun_mod::SILENCER != 0;
        self.audio.play(shot_sound(kind, silenced), Some(muzzle), 1.0);
        let (sky, blk) = self.terrain.world.light_estimate(muzzle);
        let size = stats.flash;
        if !silenced {
            self.guns.remote_flashes.push((1.0, muzzle, look, 0.1 * size, seed));
            self.guns.flash_light = ((0.6 + 0.2 * size).min(1.0), muzzle + look * 0.3);
            self.particles.sparks(muzzle, look, 3 + (size * 3.0) as usize);
        }
        let puff = if size > 1.5 { 2 } else { 1 };
        self.particles.gun_smoke(muzzle, look, puff, sky, blk);
        if let Some(port) = self.remote_gun_point(id, kind, gun::spec(kind).eject) {
            let right = look.cross(Vec3::Y).normalize_or(Vec3::X);
            let up = right.cross(look);
            let r = |g: &mut Self| g.random() - 0.5;
            let vel = right * (2.4 + r(self)) + up * (2.6 + r(self)) - look * 0.6;
            let spin = Vec3::new(r(self), r(self), r(self)) * 40.0;
            self.guns.cases.eject(port, vel, spin, case_kind(kind));
        }
    }

    /// Where a laser from `eye` along `dir` makes its dot: on the first block, mob or player
    /// in the way (not `owner`, who holds it; this player counts when someone else does).
    fn laser_hit(&self, eye: Vec3, dir: Vec3, range: f32, owner: Option<u8>) -> Option<Vec3> {
        let world = &self.terrain.world;
        let block = raycast_solid(world, eye, dir, range)
            .and_then(|(hit, _)| ray_boxes(world, eye, dir, hit, range))
            .map(|(d, _)| d);
        let reach = block.unwrap_or(range);
        let mob = self
            .mobs
            .iter()
            .filter_map(|m| m.ray_hit(eye, dir, reach))
            .fold(None, |a: Option<f32>, d| Some(a.map_or(d, |a| a.min(d))));
        let player = self.pick_other_player(eye, dir, reach, owner).map(|(_, d)| d);
        let me = owner.and_then(|_| {
            let p = self.player.pos;
            let half = Vec3::new(0.3, 0.0, 0.3);
            crate::util::ray_box(eye, dir, p - half, p + half + Vec3::Y * 1.8, reach)
        });
        [block, mob, player, me]
            .into_iter()
            .flatten()
            .fold(None, |a: Option<f32>, d| Some(a.map_or(d, |a| a.min(d))))
            .map(|d| eye + dir * (d - 0.03))
    }

    /// Bullets fly on (falling a little) and hit the first block, mob or player in their way.
    fn update_bullets(&mut self, dt: f32) {
        let mut bullets = std::mem::take(&mut self.guns.bullets);
        bullets.retain_mut(|b| {
            // The last step ends where the range does.
            let full = b.vel * dt;
            let len = full.length().min(b.range - b.traveled);
            if len <= 0.0 {
                return false;
            }
            let step = full.normalize_or_zero() * len;
            let dir = step / len;
            let world = &self.terrain.world;
            let block = raycast_solid(world, b.pos, dir, len).and_then(|(hit, _)| {
                ray_boxes(world, b.pos, dir, hit, len + 0.01).map(|(d, n)| (hit, d, n))
            });
            let reach = block.map_or(len, |(_, d, _)| d);
            let mob = self
                .mobs
                .iter()
                .enumerate()
                .filter(|(_, m)| m.alive())
                .filter_map(|(i, m)| m.ray_hit(b.pos, dir, reach).map(|d| (i, d)))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            let player = self
                .pick_player(b.pos, dir, reach)
                .filter(|&(_, d)| mob.is_none_or(|(_, md)| d < md));

            use crate::net::Msg;
            let (dmg, knock, from) = (b.damage, b.knockback, b.from);
            if b.visual {
                // Someone else's: it stops at the first mob but does nothing to it (the
                // shooter's game hits it), and passes the players.
                if mob.is_some() {
                    return false;
                }
            } else if let Some((id, _)) = player {
                if self.is_client() {
                    let kind = crate::net::hurt::BULLET;
                    self.send(Msg::AttackPlayer { id, dmg, knock, kind });
                } else {
                    let kind = crate::net::hurt::BULLET;
                    self.send_to(id, &Msg::Hurt { dmg, from, knock, kind });
                }
                return false;
            }
            if let Some((i, _)) = mob.filter(|_| !b.visual) {
                if self.is_client() {
                    let id = self.mobs[i].id;
                    self.send(Msg::AttackMob { id, dmg, knock });
                } else {
                    self.mobs[i].hurt(dmg, Some(from), knock);
                }
                return false;
            }
            if let Some((hit, d, normal)) = block {
                // Chips fly off where the bullet hit.
                let b_id = self.terrain.world.geti(hit);
                let tint = self.block_tint(hit, b_id);
                let n = normal.as_vec3();
                let at = b.pos + dir * d;
                self.particles
                    .impact(&self.terrain.world, at + n * 0.02, n, b_id, tint);
                self.audio.play(Sound::Impact, Some(at), 0.7);
                // A hole where it went in (smaller from a shotgun's pellets).
                if self.guns.holes.len() >= MAX_HOLES {
                    self.guns.holes.remove(0);
                }
                let size = if b.small { 0.07 } else { 0.1 } * (0.85 + 0.3 * self.random());
                let turn = self.random() * TAU;
                self.guns.holes.push(Hole {
                    pos: at,
                    normal: n,
                    block: hit,
                    id: b_id,
                    turn,
                    size,
                    born: self.time,
                });
                return false;
            }
            b.pos += step;
            b.vel.y -= b.gravity * dt;
            b.traveled += len;
            b.traveled < b.range
        });
        self.guns.bullets.extend(bullets);
    }

    /// Tracer streaks of the bullets in flight, the spent cases and the laser's dot.
    pub(super) fn build_gun_effects(&self, out: &mut Vec<Vertex>, cam: Vec3, right: Vec3, up: Vec3) {
        for b in &self.guns.bullets {
            let dir = b.vel.normalize_or_zero();
            let fade = (1.0 - b.traveled / 8.0).max(0.0);
            let head = b.pos + b.offset * fade;
            let tail_len = b.traveled.min(3.0);
            if tail_len > 0.3 {
                ballistics::emit_tracer(out, head - dir * tail_len, head, cam, 0.012, false);
            }
        }
        self.guns.cases.build(out, &self.terrain.world);
        // The first-person view draws the flash on its own gun.
        if let (Some((k, pos, dir, size, seed)), true) = (self.guns.flash, self.camera.mode != 0) {
            ballistics::emit_muzzle_flash(out, pos, dir, cam, size, seed, k);
        }
        for &(k, pos, dir, size, seed) in &self.guns.remote_flashes {
            ballistics::emit_muzzle_flash(out, pos, dir, cam, size, seed, k);
        }
        // The other players' laser sights: the dot where they point, and the faint beam.
        for (id, kind, mods, eye, look) in self.remote_guns() {
            if mods & gun_mod::LASER == 0 {
                continue;
            }
            let range = kind.stats().range.min(120.0);
            let Some(p) = self.laser_hit(eye, look, range, Some(id)) else {
                continue;
            };
            let size = (0.006 + 0.004 * p.distance(cam)).min(0.1);
            ballistics::emit_laser_dot(out, p, right, up, size);
            if let Some(from) = self.remote_gun_point(id, kind, gun::spec(kind).laser) {
                ballistics::emit_tracer(out, from, p, cam, 0.004, true);
            }
        }
        if let Some(p) = self.guns.laser_dot {
            // A small dot near by, still visible far away.
            let size = (0.006 + 0.004 * p.distance(cam)).min(0.1);
            ballistics::emit_laser_dot(out, p, right, up, size);
            // The beam, faint, from the lens (first person) to the dot.
            if let Some(from) = self.guns.laser_from.filter(|_| self.camera.mode == 0) {
                ballistics::emit_tracer(out, from, p, cam, 0.004, true);
            }
        }
    }

    /// The bullet holes, multiplied onto the blocks they are in.
    pub(super) fn build_bullet_holes(&self, out: &mut Vec<Vertex>, cam: Vec3) {
        use crate::world::mesh::flags;
        for h in &self.guns.holes {
            if h.pos.distance_squared(cam) > 64.0 * 64.0 {
                continue;
            }
            let left = HOLE_LIFE - (self.time - h.born);
            let size = h.size * (left / 3.0).min(1.0);
            let n = h.normal;
            let a = if n.y.abs() > 0.5 { Vec3::X } else { Vec3::Y };
            let t1 = n.cross(a).normalize();
            let t2 = n.cross(t1);
            let (s, c) = h.turn.sin_cos();
            let (r, u) = ((t1 * c + t2 * s) * size, (t2 * c - t1 * s) * size);
            let p = h.pos + n * 0.002;
            let corners = [p - r - u, p + r - u, p + r + u, p - r + u];
            let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
            let v: [Vertex; 4] = std::array::from_fn(|i| Vertex {
                pos: corners[i].to_array(),
                uv: uv[i],
                layer: crate::world::textures::tex::BULLET_HOLE as f32,
                light: [255, 255, 0, 0],
                tint: [255, 255, 255, flags::OVERLAY],
            });
            // Both sides.
            out.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
            out.extend_from_slice(&[v[0], v[2], v[1], v[0], v[3], v[2]]);
        }
    }

    /// The gun's part of the HUD while holding one: the scope's picture when aimed through
    /// it, the crosshair (it opens up after shots and fades while aiming), the rounds and the
    /// controls.
    pub(super) fn draw_gun_hud(&mut self) {
        let Some((gun, kind)) = self.held_gun() else { return };
        let stats = kind.stats();
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        let playing = self.screen == Screen::Playing;
        let mods = gun_mods(&gun);
        let aim = self.guns.aim;
        let center = Vec2::new((w * 0.5).round(), (h * 0.5).round());
        let first_person = self.camera.mode == 0;
        let scope = kind.shown_mods(mods) & gun_mod::SCOPE != 0;
        if playing && first_person && scope && aim > 0.97 {
            // The scope's round view: black around it, a fine cross with a gap and dots.
            let r = (h * 0.44).round();
            let far = w.max(h) * 1.5;
            self.ui.ring(center, r + far * 0.5, far, rgba(0, 0, 0, 255));
            self.ui.ring(center, r, 10.0 * s, rgba(0, 0, 0, 150));
            let k = (s * 0.5).max(1.0);
            let black = rgba(0, 0, 0, 230);
            let gap = 10.0 * s;
            self.ui.solid(center.x - r, center.y - k * 0.5, r - gap, k, black);
            self.ui.solid(center.x + gap, center.y - k * 0.5, r - gap, k, black);
            self.ui.solid(center.x - k * 0.5, center.y - r, k, r - gap, black);
            self.ui.solid(center.x - k * 0.5, center.y + gap, k, r - gap, black);
            for i in 1..5 {
                let d = gap + i as f32 * 14.0 * s;
                for (dx, dy) in [(d, 0.0), (-d, 0.0), (0.0, d), (0.0, -d)] {
                    self.ui
                        .solid(center.x + dx - k, center.y + dy - k, 2.0 * k, 2.0 * k, black);
                }
            }
            self.ui.solid(center.x - k, center.y - k, 2.0 * k, 2.0 * k, rgba(220, 30, 30, 255));
        } else if playing && self.camera.mode != 2 && aim < 0.6 {
            // Four lines around a dot, as far apart as the shots scatter.
            let a = 1.0 - aim / 0.6;
            let spread = shot_spread(stats, mods, 0.0, self.guns.bloom);
            let half_fov = (self.fov_current.to_radians() * 0.5).tan();
            let gap = (spread.to_radians().tan() / half_fov * h * 0.5).max(3.0 * s) + 2.0 * s;
            let (len, th) = (6.0 * s, (s * 0.5).max(1.0).round());
            let c = with_alpha(rgba(255, 255, 255, 235), a);
            let shadow = with_alpha(rgba(0, 0, 0, 150), a);
            for (color, o) in [(shadow, 1.0), (c, 0.0)] {
                let (x, y) = (center.x + o, center.y + o);
                self.ui.solid(x - gap - len, y - th * 0.5, len, th, color);
                self.ui.solid(x + gap, y - th * 0.5, len, th, color);
                self.ui.solid(x - th * 0.5, y - gap - len, th, len, color);
                self.ui.solid(x - th * 0.5, y + gap, th, len, color);
                self.ui.solid(x - th * 0.5, y - th * 0.5, th, th, color);
            }
        }

        // Rounds in the magazine / rounds carried, bottom right, and the controls.
        let rounds = gun_rounds(&gun);
        let size = kind.magazine_size(mods);
        let carried = if self.creative() {
            "-".to_string()
        } else {
            self.inventory.count(kind.ammo()).to_string()
        };
        let fs = (s * 1.5).round().max(1.0);
        let big = format!("{rounds}");
        let small = format!(" / {carried}");
        let (bw, sw) = (self.ui.text_width(&big, fs), self.ui.text_width(&small, s));
        let x = w - 12.0 * s - bw - sw;
        let y = h - 34.0 * s;
        let color = if rounds == 0 {
            rgba(255, 110, 90, 255)
        } else if rounds * 4 <= size {
            rgba(255, 210, 110, 255)
        } else {
            WHITE
        };
        gui::draw_stack(&mut self.ui, x - 20.0 * s, y - 4.0 * s, 16.0 * s, &Stack::one(kind.ammo()));
        self.ui.text(&big, x, y - 3.0 * s, fs, color, true);
        self.ui
            .text(&small, x + bw, y + 4.0 * s, s, rgba(200, 200, 205, 255), true);
        let key = crate::keys::display(self.settings.keys.get(Bind::Reload));
        let note = if let Some(t) = self.guns.reload {
            // A bar filling up under the counter.
            let (bx, by, bw2) = (x - 20.0 * s, y + 14.0 * s, bw + sw + 20.0 * s);
            self.ui.solid(bx, by, bw2, 2.0 * s, rgba(0, 0, 0, 160));
            let k = (t / stats.reload).min(1.0);
            self.ui
                .solid(bx, by, bw2 * k, 2.0 * s, rgba(120, 230, 140, 255));
            t_owned("gun.reloading")
        } else if rounds == 0 {
            tf("gun.empty", &[&key])
        } else if stats.auto {
            tf("gun.controls_auto", &[&key])
        } else {
            tf("gun.controls", &[&key])
        };
        let fsn = (s * 0.75).round().max(1.0);
        let nw = self.ui.text_width(&note, fsn);
        self.ui.text(
            &note,
            w - 12.0 * s - nw,
            y + 19.0 * s,
            fsn,
            rgba(210, 210, 215, 220),
            true,
        );
    }

    /// A message about the gun, at most about once a second.
    fn gun_message(&mut self, text: &str) {
        if self.time - self.guns.last_message > 1.0 {
            self.guns.last_message = self.time;
            self.say(text, rgba(255, 190, 110, 255));
        }
    }

    /// Opens the gun station: the camera glides over its table. To clean or tune, the held
    /// gun is laid on it once the camera is there.
    pub(super) fn open_gun_station(&mut self, p: IVec3) {
        self.guns.bench.drag_from = None;
        self.open_container(Container::GunStation(p));
        if self.guns.bench.mode != BenchMode::Assemble {
            self.lay_gun_on_bench(0.3);
        }
    }

    /// The hand lays a gun on the table to clean or tune it (after `delay` seconds): the held
    /// one, or else the first in the inventory.
    pub(super) fn lay_gun_on_bench(&mut self, delay: f32) {
        if self.guns.bench.gun.is_some() {
            return;
        }
        let is_gun = |s: &Slot| s.is_some_and(|s| GunKind::of(s.item).is_some());
        let slot = if is_gun(&self.inventory.slots[self.hotbar_slot]) {
            Some(self.hotbar_slot)
        } else {
            self.inventory.slots.iter().position(is_gun)
        };
        if let Some(i) = slot {
            let bench = &mut self.guns.bench;
            bench.gun = self.inventory.slots[i].take();
            bench.from_slot = Some(i);
            bench.placed_at = self.time + delay;
            bench.had_gun = true;
        }
    }

    /// The gun on the table goes back where it came from (or wherever there is room).
    pub(super) fn take_gun_off_bench(&mut self) {
        let bench = &mut self.guns.bench;
        let (Some(gun), from) = (bench.gun.take(), bench.from_slot.take()) else {
            return;
        };
        bench.had_gun = false;
        bench.fit = None;
        match from {
            Some(i) if self.inventory.slots[i].is_none() => self.inventory.slots[i] = Some(gun),
            _ => self.give(gun),
        }
    }

    /// Closing the gun station: what the parts put together so far were made of goes back to
    /// the player, and the gun on the table back into its slot.
    pub(super) fn close_gun_station(&mut self) {
        self.take_gun_off_bench();
        let items = self.guns.bench.items();
        self.guns.bench.clear();
        for s in items {
            self.give(s);
        }
    }
}

fn t_owned(key: &'static str) -> String {
    t(key).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zeroed_bullet_comes_down_on_the_line_of_sight() {
        let (speed, gravity, dt) = (180.0, 12.0, 1.0 / 120.0);
        let dir = Vec3::new(0.2, -0.1, -1.0).normalize();
        for dist in [5.0, 30.0, 80.0] {
            let mut vel = zeroed(dir, dist, speed, gravity) * speed;
            let mut pos = Vec3::ZERO;
            // Fly as `update_bullets` does, until it is as far along as the target.
            while pos.dot(dir) < dist {
                pos += vel * dt;
                vel.y -= gravity * dt;
            }
            let miss = (pos - dir * pos.dot(dir)).length();
            assert!(miss < 0.06, "{dist}: {miss}");
        }
    }

    #[test]
    fn shots_scatter_within_their_cone() {
        let dir = Vec3::new(0.3, -0.2, -1.0).normalize();
        for i in 0..50 {
            let (r1, r2) = (i as f32 / 50.0, ((i * 7) % 50) as f32 / 50.0);
            let d = scatter(dir, 2.0, r1, r2);
            let angle = d.dot(dir).clamp(-1.0, 1.0).acos().to_degrees();
            assert!(angle <= 2.01, "{angle} degrees off");
        }
        assert!(scatter(dir, 0.0, 0.5, 0.5).abs_diff_eq(dir, 1e-6));
    }

    #[test]
    fn gun_data_keeps_rounds_and_attachments_apart() {
        let mut s = Stack::one(M16);
        set_gun_mods(&mut s, gun_mod::SCOPE | gun_mod::LASER);
        set_gun_rounds(&mut s, 45);
        assert_eq!(gun_rounds(&s), 45);
        assert_eq!(gun_mods(&s), gun_mod::SCOPE | gun_mod::LASER);
        set_gun_rounds(&mut s, 3);
        assert_eq!(gun_mods(&s), gun_mod::SCOPE | gun_mod::LASER);
        assert_eq!(GunKind::Pistol.magazine_size(0), 12);
        assert_eq!(GunKind::Pistol.magazine_size(gun_mod::EXTENDED_MAGAZINE), 20);
    }

    #[test]
    fn a_half_built_gun_gives_back_what_it_was_made_of() {
        let mut bench = Bench {
            kind: GunKind::M16,
            installed: 2,
            ..Bench::default()
        };
        bench.taken = [true, true, false, false, false];
        let back: u32 = bench
            .parts()
            .iter()
            .filter(|s| s.item == IRON_INGOT)
            .map(|s| s.count as u32)
            .sum();
        assert_eq!(back, 7);
        bench.kind = GunKind::Pistol;
        assert_eq!(bench.parts(), vec![Stack::one(PISTOL_FRAME), Stack::one(PISTOL_BARREL)]);
    }
}
