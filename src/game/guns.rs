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
use crate::model::gun::PARTS;

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
}

#[derive(Default)]
pub(super) struct Guns {
    /// Where the held gun's muzzle, ejection port and laser lens were drawn last frame
    /// (first person).
    pub(super) muzzle: Option<Vec3>,
    pub(super) eject: Option<Vec3>,
    pub(super) laser_from: Option<Vec3>,
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
}

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
    /// The 3D view: turned by dragging with the right mouse button (sways by itself until
    /// then), and framed smoothly around what is shown (center, scale).
    pub(super) yaw: f32,
    pub(super) pitch: f32,
    pub(super) turned: bool,
    pub(super) drag_from: Option<Vec2>,
    pub(super) frame: Option<(Vec2, f32)>,
    /// Soap bubbles while scrubbing: where (GUI pixels in the view) and when.
    pub(super) bubbles: Vec<(Vec2, f32)>,
    /// A part was clicked but is not in the inventory: when (flashes its line red).
    pub(super) missing_at: f32,
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
            pitch: 0.3,
            turned: false,
            drag_from: None,
            frame: None,
            bubbles: Vec::new(),
            missing_at: -10.0,
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
        let held = self.held_gun();
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
            let t = t + dt;
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
        self.guns.cases.update(dt, &self.terrain.world);

        // The laser points where the view does; its dot sits on whatever is there.
        self.guns.laser_dot = None;
        if let Some((_, kind)) = held.filter(|_| mods & gun_mod::LASER != 0 && self.screen != Screen::Dead) {
            let range = kind.stats().range.min(120.0);
            let eye = self.player.eye();
            let dir = look_dir(self.yaw, self.pitch);
            let world = &self.terrain.world;
            let block = raycast_solid(world, eye, dir, range)
                .and_then(|(hit, _)| ray_boxes(world, eye, dir, hit, range))
                .map(|(d, _)| d);
            let mob = self
                .mobs
                .iter()
                .filter_map(|m| m.ray_hit(eye, dir, block.unwrap_or(range)))
                .fold(None, |a: Option<f32>, d| Some(a.map_or(d, |a| a.min(d))));
            if let Some(d) = mob.or(block) {
                self.guns.laser_dot = Some(eye + dir * (d - 0.03));
            }
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
            }
            return;
        }
        let dirt = gun.damage as f32 / stats.dirt_max as f32;
        // Dirt makes it jam more and more often; a completely dirty one does not fire at all.
        if dirt >= 1.0 || (dirt > 0.6 && self.random() < (dirt - 0.6) * 1.2) {
            self.gun_message(t("gun.jammed"));
            return;
        }
        let mods = gun_mods(&gun);
        let creative = self.creative();
        if let Some(s) = &mut self.inventory.slots[slot] {
            set_gun_rounds(s, gun_rounds(s) - 1);
            if !creative {
                s.damage = (s.damage + 1).min(stats.dirt_max);
            }
        }
        self.hand.shoot();

        let eye = self.player.eye();
        let look = look_dir(self.yaw, self.pitch);
        let aim = smoothstep(0.0, 1.0, self.guns.aim);
        let hip = if mods & gun_mod::LASER != 0 {
            stats.spread_laser
        } else {
            stats.spread_hip
        };
        let spread = hip + (stats.spread_aimed - hip) * aim + self.guns.bloom * (1.0 - 0.7 * aim);
        self.guns.bloom = (self.guns.bloom + BLOOM_PER_SHOT).min(BLOOM_MAX);

        let right = look.cross(Vec3::Y).normalize_or_zero();
        let first_person = self.camera.mode == 0;
        let muzzle = match self.guns.muzzle {
            Some(m) if first_person => m,
            _ => eye - Vec3::Y * 0.3 + look * 0.9 + right * 0.3,
        };
        for _ in 0..stats.pellets {
            let (r1, r2) = (self.random(), self.random());
            let dir = scatter(look, spread, r1, r2);
            // A silencer slows the bullet a little.
            let speed = stats.speed * if mods & gun_mod::SILENCER != 0 { 0.9 } else { 1.0 };
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
            });
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

        // Muzzle flash and smoke (a silencer leaves only a wisp of smoke).
        let (sky, blk) = self.terrain.world.light_estimate(muzzle);
        if mods & gun_mod::SILENCER == 0 {
            let big = if stats.damage * stats.pellets as f32 > 20.0 { 5 } else { 3 };
            for _ in 0..big {
                self.particles.flame(muzzle + look * 0.03);
            }
            for _ in 0..2 {
                self.particles.smoke(muzzle, sky, blk);
            }
        } else {
            self.particles.smoke(muzzle, sky, blk);
        }
    }

    /// A spent case (or shotgun hull) flies out to the right of the gun, tumbling.
    fn eject_case(&mut self, kind: GunKind) {
        let eye = self.player.eye();
        let look = look_dir(self.yaw, self.pitch);
        let right = look.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(look);
        let port = match self.guns.eject {
            Some(p) if self.camera.mode == 0 => p,
            _ => eye - Vec3::Y * 0.25 + look * 0.5 + right * 0.25,
        };
        let r = |g: &mut Self| g.random() - 0.5;
        let vel = right * (2.4 + r(self)) + up * (2.6 + r(self)) - look * 0.6 + self.player.vel * 0.8;
        let spin = Vec3::new(r(self), r(self), r(self)) * 40.0;
        let case = match kind {
            GunKind::Pistol => CaseKind::Pistol,
            GunKind::DesertEagle => CaseKind::Magnum,
            GunKind::M16 => CaseKind::Rifle,
            GunKind::Sniper => CaseKind::Bmg,
            GunKind::Shotgun => CaseKind::Shell,
        };
        self.guns.cases.eject(port, vel, spin, case);
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
            if let Some((id, _)) = player {
                if self.is_client() {
                    self.send(Msg::AttackPlayer { id, dmg, knock });
                } else {
                    self.send_to(id, &Msg::Hurt { dmg, from, knock });
                }
                return false;
            }
            if let Some((i, _)) = mob {
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
                self.particles
                    .impact(&self.terrain.world, b.pos + dir * d + n * 0.02, n, b_id, tint);
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
            let hip = if mods & gun_mod::LASER != 0 {
                stats.spread_laser
            } else {
                stats.spread_hip
            };
            let spread = hip + self.guns.bloom;
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

    /// Opens the gun station's screen.
    pub(super) fn open_gun_station(&mut self, p: IVec3) {
        let bench = &mut self.guns.bench;
        bench.frame = None;
        bench.drag_from = None;
        self.open_container(Container::GunStation(p));
    }

    /// Closing the gun station: what the parts put together so far were made of and the gun
    /// being cleaned or tuned go back to the player.
    pub(super) fn close_gun_station(&mut self) {
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
