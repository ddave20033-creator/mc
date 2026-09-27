//! Guns: shooting (left mouse button), aiming down the sights (right), reloading, bullets in
//! flight, spent cases, recoil, the laser sight and the gun HUD (crosshair, scope, ammo), and
//! the state of the gun station screen (drawn in `gui::gun_station`), where a pistol is put
//! together part by part, cleaned and fitted with attachments.

use super::*;
use crate::entity::player::{ray_boxes, raycast_solid};
use crate::item::*;
use crate::lang::tf;
use crate::model::ballistics::{self, Cases};
use crate::model::gun::PARTS;

/// Damage of one bullet (a pig has 10 health).
const BULLET_DAMAGE: f32 = 7.0;
const BULLET_KNOCKBACK: f32 = 0.5;
/// Muzzle velocity and the pull that bends a bullet's path down (blocks, seconds).
const BULLET_SPEED: f32 = 180.0;
const BULLET_GRAVITY: f32 = 12.0;
/// How far a bullet flies before it is gone, in blocks (the laser reaches as far).
const BULLET_RANGE: f32 = 80.0;
/// Seconds between shots.
const FIRE_DELAY: f32 = 0.18;
/// Seconds to raise the gun to the eye, and to reload.
const AIM_TIME: f32 = 0.16;
pub(super) const RELOAD_TIME: f32 = 2.0;
/// Where the shots go, as the angle (degrees) of the cone around the aim: from the hip, from
/// the hip with the laser sight, aimed. Every shot widens it for a moment.
const HIP_SPREAD: f32 = 2.4;
const LASER_SPREAD: f32 = 0.7;
const AIMED_SPREAD: f32 = 0.12;
const BLOOM_PER_SHOT: f32 = 1.3;
const BLOOM_MAX: f32 = 6.0;
/// How far the view kicks up per shot (degrees), from the hip and aimed; most of it comes
/// back by itself.
const KICK_HIP: f32 = 1.4;
const KICK_AIMED: f32 = 0.8;
/// How much the view narrows when aimed: with the iron sights and through the scope.
const SIGHT_ZOOM: f32 = 0.78;
const SCOPE_ZOOM: f32 = 0.25;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum BenchMode {
    Assemble,
    Clean,
    Tune,
}

/// A bullet in flight.
struct Bullet {
    pos: Vec3,
    vel: Vec3,
    /// Where it was fired from (knocks what it hits away from there).
    from: Vec3,
    /// It flies from the eye, but is drawn starting at the muzzle: this offset fades out
    /// over the first blocks.
    offset: Vec3,
    traveled: f32,
    age: f32,
}

#[derive(Default)]
pub(super) struct Guns {
    /// Where the held pistol's muzzle and ejection port were drawn last frame (first person).
    pub(super) muzzle: Option<Vec3>,
    pub(super) eject: Option<Vec3>,
    /// And its laser sight's lens (the beam starts there).
    pub(super) laser_from: Option<Vec3>,
    /// When the last "jammed" or "no bullets" message was shown.
    last_message: f32,
    pub(super) bench: Bench,
    /// Aimed down the sights: 0 from the hip .. 1 aimed.
    pub(super) aim: f32,
    /// Reloading: seconds so far, and whether the magazine was empty (the slide gets racked).
    pub(super) reload: Option<f32>,
    reload_empty: bool,
    /// The reload key was pressed.
    pub(super) reload_pressed: bool,
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
    /// Parts put together so far (in order: frame, barrel, spring, slide, magazine), which of
    /// them came out of the inventory (in creative they do not), and when the last one went in.
    pub(super) installed: usize,
    pub(super) taken: [bool; PARTS],
    pub(super) installed_at: f32,
    /// A pistol was just finished: it stays on show until the next one is started.
    pub(super) finished: bool,
    /// The pistol being cleaned or tuned, how dirty each of its parts is (0 clean .. 1), and
    /// the pistol's dirt when `dirt` was last matched to it (another pistol put in reloads it).
    pub(super) gun: Slot,
    pub(super) dirt: [f32; PARTS],
    pub(super) dirt_of: Option<u16>,
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
    /// The parts put in so far (they go back to the player when assembling stops).
    pub(super) fn parts(&self) -> Vec<Stack> {
        (0..self.installed.min(PARTS))
            .filter(|&i| self.taken[i] && !self.finished)
            .map(|i| Stack::one(PISTOL_PARTS[i]))
            .collect()
    }

    /// Everything on the bench that belongs to the player: the parts put in so far and the
    /// pistol being cleaned or tuned.
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
        let mode = self.mode;
        *self = Self {
            mode,
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
    /// The held pistol's stack.
    fn held_gun(&self) -> Option<Stack> {
        self.inventory.slots[self.hotbar_slot].filter(|s| s.item == PISTOL)
    }

    /// The held pistol's attachments (0 when holding something else).
    pub(super) fn held_gun_mods(&self) -> u8 {
        self.held_gun().map_or(0, |s| gun_mods(&s))
    }

    /// How much the gun narrows the view now (1 = not at all).
    pub(super) fn gun_zoom(&self) -> f32 {
        if self.held_gun().is_none() {
            return 1.0;
        }
        let full = if self.held_gun_mods() & gun_mod::SCOPE != 0 {
            SCOPE_ZOOM
        } else {
            SIGHT_ZOOM
        };
        let a = smoothstep(0.0, 1.0, self.guns.aim);
        1.0 + (full - 1.0) * a
    }

    /// Aiming, reloading, recoil coming back, bullets, spent cases and the laser, every frame.
    pub(super) fn update_guns(&mut self, dt: f32, control: bool) {
        let gun = self.held_gun();
        let mods = gun.map_or(0, |s| gun_mods(&s));
        let reload_pressed = std::mem::take(&mut self.guns.reload_pressed);
        if gun.is_none() {
            self.guns.reload = None;
        } else if reload_pressed && control {
            self.start_reload();
        }
        let g = &mut self.guns;
        let aiming = gun.is_some() && control && self.right_down && g.reload.is_none();
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
        if let Some(t) = self.guns.reload {
            let t = t + dt;
            if t >= RELOAD_TIME {
                self.guns.reload = None;
                self.finish_reload();
            } else {
                self.guns.reload = Some(t);
            }
        }
        self.hand.aim = self.guns.aim;
        self.hand.reload = self.guns.reload.map(|t| t / RELOAD_TIME);
        self.hand.gun_mods = mods;
        self.hand.gun_empty = self.held_gun().is_some_and(|g| gun_rounds(&g) == 0);
        self.hand.reload_empty = self.guns.reload_empty;

        self.update_bullets(dt);
        self.guns.cases.update(dt, &self.terrain.world);

        // The laser points where the view does; its dot sits on whatever is there.
        self.guns.laser_dot = None;
        if gun.is_some() && mods & gun_mod::LASER != 0 && self.screen != Screen::Dead {
            let eye = self.player.eye();
            let dir = look_dir(self.yaw, self.pitch);
            let world = &self.terrain.world;
            let block = raycast_solid(world, eye, dir, BULLET_RANGE)
                .and_then(|(hit, _)| ray_boxes(world, eye, dir, hit, BULLET_RANGE))
                .map(|(d, _)| d);
            let mob = self
                .mobs
                .iter()
                .filter_map(|m| m.ray_hit(eye, dir, block.unwrap_or(BULLET_RANGE)))
                .fold(None, |a: Option<f32>, d| Some(a.map_or(d, |a| a.min(d))));
            if let Some(d) = mob.or(block) {
                self.guns.laser_dot = Some(eye + dir * (d - 0.03));
            }
        }
    }

    /// Starts reloading the held pistol, if its magazine is not full and there are bullets.
    fn start_reload(&mut self) {
        let Some(gun) = self.held_gun() else { return };
        if self.guns.reload.is_some() {
            return;
        }
        if gun_rounds(&gun) >= magazine_size(gun_mods(&gun)) {
            return;
        }
        if !self.creative() && self.inventory.count(BULLET) == 0 {
            self.gun_message(t("gun.no_ammo"));
            return;
        }
        self.guns.aim = 0.0;
        self.guns.reload = Some(0.0);
        self.guns.reload_empty = gun_rounds(&gun) == 0;
    }

    /// The new magazine is in: filled from the inventory's bullets (free in creative).
    fn finish_reload(&mut self) {
        let slot = self.hotbar_slot;
        let Some(mut gun) = self.inventory.slots[slot].filter(|s| s.item == PISTOL) else {
            return;
        };
        let have = gun_rounds(&gun);
        let need = magazine_size(gun_mods(&gun)).saturating_sub(have);
        let mut got = 0;
        while got < need && (self.creative() || self.inventory.remove_one(BULLET)) {
            got += 1;
        }
        set_gun_rounds(&mut gun, have + got);
        // The inventory may have moved the pistol while taking the bullets: find it again.
        if self.inventory.slots[slot].is_some_and(|s| s.item == PISTOL) {
            self.inventory.slots[slot] = Some(gun);
        }
    }

    /// Left click with a pistol: fires a bullet (a dirty pistol may jam, an empty one
    /// reloads).
    pub(super) fn shoot(&mut self) {
        if self.action_cooldown > 0.0 || self.guns.reload.is_some() {
            return;
        }
        let slot = self.hotbar_slot;
        let Some(gun) = self.held_gun() else { return };
        self.action_cooldown = FIRE_DELAY;
        if gun_rounds(&gun) == 0 {
            if self.creative() || self.inventory.count(BULLET) > 0 {
                self.start_reload();
            } else {
                self.gun_message(t("gun.no_ammo"));
            }
            return;
        }
        let dirt = gun.damage as f32 / PISTOL_DIRT_MAX as f32;
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
                s.damage = (s.damage + 1).min(PISTOL_DIRT_MAX);
            }
        }
        self.hand.shoot();

        let eye = self.player.eye();
        let look = look_dir(self.yaw, self.pitch);
        let aim = smoothstep(0.0, 1.0, self.guns.aim);
        let hip = if mods & gun_mod::LASER != 0 {
            LASER_SPREAD
        } else {
            HIP_SPREAD
        };
        let spread = hip + (AIMED_SPREAD - hip) * aim + self.guns.bloom * (1.0 - 0.7 * aim);
        let (r1, r2) = (self.random(), self.random());
        let dir = scatter(look, spread, r1, r2);
        self.guns.bloom = (self.guns.bloom + BLOOM_PER_SHOT).min(BLOOM_MAX);

        // Recoil: the view kicks up (and a little to the side); most of it comes back.
        let kick = (KICK_HIP + (KICK_AIMED - KICK_HIP) * aim).to_radians();
        self.pitch = (self.pitch + kick).min(1.55);
        self.guns.recover += kick * 0.6;
        self.yaw += (self.random() - 0.5) * 0.4f32.to_radians();

        let right = look.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(look);
        let first_person = self.camera.mode == 0;
        let muzzle = match self.guns.muzzle {
            Some(m) if first_person => m,
            _ => eye - Vec3::Y * 0.3 + look * 0.9 + right * 0.3,
        };
        self.guns.bullets.push(Bullet {
            pos: eye,
            vel: dir * BULLET_SPEED,
            from: self.player.pos,
            offset: muzzle - eye,
            traveled: 0.0,
            age: 0.0,
        });

        // The spent case flies out to the right, tumbling.
        let port = match self.guns.eject {
            Some(p) if first_person => p,
            _ => eye - Vec3::Y * 0.25 + look * 0.5 + right * 0.25,
        };
        let r = |g: &mut Self| g.random() - 0.5;
        let vel = right * (2.4 + r(self))
            + up * (2.6 + r(self))
            - look * 0.6
            + self.player.vel * 0.8;
        let spin = Vec3::new(r(self), r(self), r(self)) * 40.0;
        self.guns.cases.eject(port, vel, spin);

        // Muzzle flash and smoke (a silencer leaves only a wisp of smoke).
        let (sky, blk) = self.terrain.world.light_estimate(muzzle);
        if mods & gun_mod::SILENCER == 0 {
            for _ in 0..3 {
                self.particles.flame(muzzle + look * 0.03);
            }
            for _ in 0..2 {
                self.particles.smoke(muzzle, sky, blk);
            }
        } else {
            self.particles.smoke(muzzle, sky, blk);
        }
    }

    /// Bullets fly on (falling a little) and hit the first block, mob or player in their way.
    fn update_bullets(&mut self, dt: f32) {
        let mut bullets = std::mem::take(&mut self.guns.bullets);
        bullets.retain_mut(|b| {
            // The last step ends where the range does.
            let full = b.vel * dt;
            let len = full.length().min(BULLET_RANGE - b.traveled);
            let step = full.normalize_or_zero() * len;
            if len <= 0.0 {
                return false;
            }
            let dir = step / len;
            let world = &self.terrain.world;
            let block = raycast_solid(world, b.pos, dir, len)
                .and_then(|(hit, _)| ray_boxes(world, b.pos, dir, hit, len + 0.01).map(|(d, n)| (hit, d, n)));
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
            let (dmg, knock, from) = (BULLET_DAMAGE, BULLET_KNOCKBACK, b.from);
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
            b.vel.y -= BULLET_GRAVITY * dt;
            b.traveled += len;
            b.age += dt;
            b.traveled < BULLET_RANGE
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

    /// The gun's part of the HUD while holding a pistol: the scope's picture when aimed
    /// through it, the crosshair (it opens up after shots and fades while aiming), the
    /// rounds and the controls.
    pub(super) fn draw_gun_hud(&mut self) {
        let Some(gun) = self.held_gun() else { return };
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        let playing = self.screen == Screen::Playing;
        let mods = gun_mods(&gun);
        let aim = self.guns.aim;
        let center = Vec2::new((w * 0.5).round(), (h * 0.5).round());
        let first_person = self.camera.mode == 0;
        if playing && first_person && mods & gun_mod::SCOPE != 0 && aim > 0.97 {
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
                LASER_SPREAD
            } else {
                HIP_SPREAD
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

        // Rounds in the magazine / bullets carried, bottom right, and the controls.
        let rounds = gun_rounds(&gun);
        let size = magazine_size(mods);
        let carried = if self.creative() {
            "-".to_string()
        } else {
            self.inventory.count(BULLET).to_string()
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
        gui::draw_stack(&mut self.ui, x - 20.0 * s, y - 4.0 * s, 16.0 * s, &Stack::one(BULLET));
        self.ui.text(&big, x, y - 3.0 * s, fs, color, true);
        self.ui
            .text(&small, x + bw, y + 4.0 * s, s, rgba(200, 200, 205, 255), true);
        let key = crate::keys::display(self.settings.keys.get(Bind::Reload));
        let note = if let Some(t) = self.guns.reload {
            // A bar filling up under the counter.
            let (bx, by, bw2) = (x - 20.0 * s, y + 14.0 * s, bw + sw + 20.0 * s);
            self.ui.solid(bx, by, bw2, 2.0 * s, rgba(0, 0, 0, 160));
            self.ui
                .solid(bx, by, bw2 * (t / RELOAD_TIME), 2.0 * s, rgba(120, 230, 140, 255));
            t_owned("gun.reloading")
        } else if rounds == 0 {
            tf("gun.empty", &[&key])
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

    /// A message about the pistol, at most about once a second.
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

    /// Closing the gun station: the parts put together so far and the pistol being cleaned
    /// or tuned go back to the player.
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
    fn pistol_data_keeps_rounds_and_attachments_apart() {
        let mut s = Stack::one(PISTOL);
        set_gun_mods(&mut s, gun_mod::SCOPE | gun_mod::LASER);
        set_gun_rounds(&mut s, 20);
        assert_eq!(gun_rounds(&s), 20);
        assert_eq!(gun_mods(&s), gun_mod::SCOPE | gun_mod::LASER);
        set_gun_rounds(&mut s, 3);
        assert_eq!(gun_mods(&s), gun_mod::SCOPE | gun_mod::LASER);
        assert_eq!(magazine_size(gun_mods(&s)), 12);
        assert_eq!(magazine_size(gun_mod::EXTENDED_MAGAZINE), 20);
    }
}
