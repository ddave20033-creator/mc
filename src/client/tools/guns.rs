//! Guns: shooting (left mouse button; held down for automatic fire), aiming down the sights
//! (right), reloading (a magazine, or a shotgun's shells one by one), working a bolt or a
//! pump, bullets in flight, pellets, spent cases, recoil, the laser sight and the gun HUD
//! (crosshair, scope, ammo). Guns are put together, taken apart, cleaned and tuned at the gun
//! station (`gui::gun_station`).

use crate::client::*;
use crate::entity::player::{ray_boxes, raycast_solid};
use crate::item::*;
use crate::lang::tf;
use crate::model::ballistics::{self, Cases};
use crate::audio::Sound;
use crate::model::pistol_view::{
    reload_anim_time, reload_seconds, GunAnim, ReloadKind, RELOAD_MAG_IN, RELOAD_MAG_OUT, RELOAD_SLIDE,
};

/// Seconds to raise the gun to the eye.
const AIM_TIME: f32 = 0.16;
/// Every shot widens the cone of the next ones for a moment (degrees).
const BLOOM_PER_SHOT: f32 = 1.3;
const BLOOM_MAX: f32 = 6.0;

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
    /// How big a hole it leaves (`hole_size`).
    hole: f32,
}

#[derive(Default)]
pub(in crate::client) struct Guns {
    /// Looking the held gun over (the inspect key): seconds so far, and the gun.
    pub(in crate::client) inspect: Option<(f32, ItemId)>,
    /// Seconds after a shot before sprinting is possible again (a shot ends a sprint).
    pub(in crate::client) no_sprint: f32,
    /// Where the held gun's muzzle, ejection port and laser lens were drawn last frame
    /// (first person).
    pub(in crate::client) muzzle: Option<Vec3>,
    pub(in crate::client) eject: Option<Vec3>,
    pub(in crate::client) laser_from: Option<Vec3>,
    /// Where the held gun's weapon light is (first person; and on the player model).
    pub(in crate::client) light_from: Option<Vec3>,
    pub(in crate::client) light_tp: Option<Vec3>,
    /// Which way the held gun points (first person: its barrel, or its scope's axis): its
    /// shots and its laser go that way, not where the view looks.
    pub(in crate::client) gun_dir: Option<Vec3>,
    /// The same on the player model (third person).
    pub(in crate::client) muzzle_tp: Option<Vec3>,
    pub(in crate::client) eject_tp: Option<Vec3>,
    /// The last muzzle flash as the world sees it (third person; the first-person view draws
    /// its own on the gun): how much is left, where, which way, how big, its turn.
    flash: Option<(f32, Vec3, Vec3, f32, f32)>,
    /// The flash's light on the surroundings: how much is left and where.
    pub(in crate::client) flash_light: (f32, Vec3),
    /// How hot the barrel is from firing (smoke curls out of it above 1), and when the last
    /// wisp came out.
    heat: f32,
    wisp: f32,
    /// When the last "jammed" or "no bullets" message was shown.
    last_message: f32,
    /// Aimed down the sights: 0 from the hip .. 1 aimed.
    pub(in crate::client) aim: f32,
    /// Reloading (the R key): seconds so far, and what it does (a revolver's: `cylinder`).
    pub(in crate::client) reload: Option<f32>,
    pub(in crate::client) plan: ReloadPlan,
    pub(in crate::client) cylinder: Option<crate::client::revolver::Cylinder>,
    /// The gun a reload is for (its hotbar slot and item): put away, or another one taken up,
    /// the reload stops (it must not go on with the other gun).
    reload_owner: Option<(usize, ItemId)>,
    /// The held revolver's chambers (first person): the head of what is in each.
    pub(in crate::client) chambers: Option<[Vec3; 6]>,
    /// The reload key was pressed.
    pub(in crate::client) reload_pressed: bool,
    /// Extra spread from the last shots (degrees), for the aim and the crosshair.
    bloom: f32,
    /// Recoil that has not come back yet (radians of pitch).
    recover: f32,
    bullets: Vec<Bullet>,
    pub(in crate::client) cases: Cases,
    /// Muzzle flashes of the other players' shots (as `flash`).
    remote_flashes: Vec<(f32, Vec3, Vec3, f32, f32)>,
    /// Holes the bullets left in the blocks.
    holes: Vec<Hole>,
}

/// A reload going on: what it does (the magazine out and a new one in, only out, only in, or
/// only the slide), whether the slide is pulled at the end (to chamber a round), whether the
/// magazine coming out is empty, how long it takes, the magazine going in (taken from the
/// inventory at the start) and which of its steps are done.
#[derive(Default)]
pub(in crate::client) struct ReloadPlan {
    kind: ReloadKind,
    rack: bool,
    old_empty: bool,
    pub(in crate::client) length: f32,
    pub(in crate::client) new_mag: Option<Stack>,
    out_done: bool,
    in_done: bool,
    rack_done: bool,
}

/// A bullet hole: where on which face of which block (it goes when the block does), how it
/// is turned (and whether mirrored, so no two look alike), how big, and when it was made.
struct Hole {
    pos: Vec3,
    normal: Vec3,
    block: IVec3,
    id: Block,
    turn: f32,
    mirrored: bool,
    size: f32,
    born: f32,
}

/// Bullet holes kept at most, and for how long (they shrink away over the last seconds).
const MAX_HOLES: usize = 300;
const HOLE_LIFE: f32 = 60.0;

impl Guns {
    /// The light of a muzzle flash going on now: where it is and how bright.
    pub(in crate::client) fn flash_light_pos(&self) -> Option<(Vec3, f32)> {
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

/// How big a hole a gun's bullet leaves (blocks across; its `WEAPONS` row's); a shotgun's
/// pellets small ones.
fn hole_size(kind: GunKind) -> f32 {
    if kind.stats().pellets > 1 {
        return 0.07;
    }
    kind.def().hole
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
    pub(in crate::client) fn held_gun(&self) -> Option<(Stack, GunKind)> {
        self.me.items.held_stack().and_then(|s| GunKind::of(s.item).map(|k| (s, k)))
    }

    /// Holding a gun.
    pub(in crate::client) fn holding_gun(&self) -> bool {
        self.held_gun().is_some()
    }

    /// The held gun's attachments (0 when holding something else).
    pub(in crate::client) fn held_gun_mods(&self) -> u8 {
        self.held_gun().map_or(0, |(s, _)| gun_mods(&s))
    }

    /// Switches the held gun's weapon light on or off (when it has one).
    pub(in crate::client) fn toggle_gun_light(&mut self) {
        let slot = self.me.items.hotbar_slot;
        let Some(s) = self.me.items.inventory.slots[slot].as_mut().filter(|s| GunKind::of(s.item).is_some()) else { return };
        let mods = gun_mods(s);
        if mods & gun_mod::LIGHT == 0 {
            return;
        }
        set_gun_mods(s, mods ^ gun_mod::LIGHT_ON);
        self.audio.play(Sound::DryFire, None, 0.35);
    }

    /// Where the held gun's weapon light shines from and which way (when it is on).
    pub(in crate::client) fn own_gun_light(&self) -> Option<(Vec3, Vec3)> {
        let (g, _) = self.held_gun()?;
        let mods = gun_mods(&g);
        if mods & gun_mod::LIGHT == 0 || mods & gun_mod::LIGHT_ON == 0 || self.screen == Screen::Dead {
            return None;
        }
        let look = self.me.look.dir();
        let from = self.tools.guns.light_from.or(self.tools.guns.light_tp).unwrap_or(self.eye() + look * 0.5);
        Some((from, self.tools.guns.gun_dir.unwrap_or(look)))
    }

    /// How dirty the held gun looks (`pistol_view::dirt_level`).
    pub(in crate::client) fn held_gun_dirt(&self) -> u8 {
        self.held_gun()
            .map_or(0, |(s, _)| crate::model::pistol_view::dirt_level(s.damage, max_damage(s.item)))
    }

    /// How much the gun narrows the view now (1 = not at all).
    pub(in crate::client) fn gun_zoom(&self) -> f32 {
        let Some((_, kind)) = self.held_gun() else {
            return 1.0;
        };
        // A scope magnifies only in its eyepiece (the view through it is rendered there).
        let full = kind.stats().sight_zoom;
        let a = smoothstep(0.0, 1.0, self.tools.guns.aim);
        1.0 + (full - 1.0) * a
    }

    /// Aiming, reloading, the bolt or pump, recoil coming back, bullets, spent cases and the
    /// laser, every frame.
    /// What the shots left in the world goes on, whatever the player is doing (dead, asleep,
    /// watching): bullets in flight, falling cases, the other players' flashes, the holes.
    pub(in crate::client) fn update_shots(&mut self, dt: f32) {
        self.update_bullets(dt);
        for (at, kind, hard) in self.tools.guns.cases.update(dt, &self.terrain.world) {
            self.audio.play(kind.def().case_sound, Some(at), 0.06 + 0.18 * hard);
        }
        for f in &mut self.tools.guns.remote_flashes {
            f.0 -= dt / 0.06;
        }
        self.tools.guns.remote_flashes.retain(|f| f.0 > 0.0);
        // Holes go with their block, and after a while.
        let (time, world) = (self.clock.time, &self.terrain.world);
        self.tools.guns
            .holes
            .retain(|h| time - h.born < HOLE_LIFE && world.geti(h.block) == h.id);
        self.tools.guns.flash_light.0 = (self.tools.guns.flash_light.0 - dt / 0.08).max(0.0);
    }

    pub(in crate::client) fn update_guns(&mut self, dt: f32, control: bool) {
        // A spectator's hands are empty; the shots, cases and holes around still go on.
        let held = self.held_gun().filter(|_| !self.spectator());
        let mods = held.map_or(0, |(s, _)| gun_mods(&s));
        let reload_pressed = std::mem::take(&mut self.tools.guns.reload_pressed);
        let owner = held.map(|(s, _)| (self.me.items.hotbar_slot, s.item));
        // (a reload that has ended belongs to no gun)
        if self.tools.guns.reload.is_none() {
            self.tools.guns.reload_owner = None;
        }
        if held.is_none() || (self.tools.guns.reload.is_some() && self.tools.guns.reload_owner != owner) {
            self.cancel_reload();
        }
        if held.is_some() && reload_pressed && control {
            self.start_reload();
        }
        if self.tools.guns.reload.is_some() && self.tools.guns.reload_owner.is_none() {
            self.tools.guns.reload_owner = owner;
        }
        let sprinting = self.me.body.sprinting;
        let g = &mut self.tools.guns;
        let aiming = held.is_some() && control && self.input.right_down && g.reload.is_none();
        let step = dt / AIM_TIME;
        g.aim = if aiming {
            (g.aim + step).min(1.0)
        } else {
            (g.aim - step).max(0.0)
        };
        g.bloom = (g.bloom - dt * 5.0).max(0.0);
        g.no_sprint = (g.no_sprint - dt).max(0.0);
        // Inspecting goes on until it is done, or anything else is done with the gun.
        let item = held.map(|(s, _)| s.item);
        g.inspect = g.inspect.and_then(|(t, it)| {
            let t = t + dt;
            let on = t < crate::model::hand::INSPECT_TIME
                && item == Some(it)
                && !aiming
                && g.reload.is_none()
                && g.no_sprint <= 0.0
                && !sprinting;
            on.then_some((t, it))
        });
        let back = g.recover.min(dt * 0.12);
        g.recover -= back;
        self.me.look.pitch -= back;

        if self.tools.guns.cylinder.is_some() && held.is_some_and(|(_, k)| !k.uses_magazine()) {
            self.update_revolver_reload(dt);
        } else if let (Some(t), Some((_, gun))) = (self.tools.guns.reload, held) {
            // The steps happen with the animation: the old magazine drops out, the new one is
            // seated, the slide slams shut (whatever was not done yet is done at the end).
            let (kind, rack, length) = (self.tools.guns.plan.kind, self.tools.guns.plan.rack, self.tools.guns.plan.length.max(0.01));
            let (was, now) = (
                reload_anim_time(t / length, kind, rack),
                reload_anim_time((t + dt) / length, kind, rack),
            );
            let finished = t + dt >= length;
            let at = self.eye();
            let due = |k: f32| (was < k && now >= k) || finished;
            // Each gun's magazine and slide (or bolt) sound its own.
            let feed = gun.magazine();
            let takes_out = matches!(kind, ReloadKind::Swap | ReloadKind::Eject);
            if takes_out && !self.tools.guns.plan.out_done && due(RELOAD_MAG_OUT) {
                self.tools.guns.plan.out_done = true;
                if let Some(f) = feed {
                    self.audio.play(f.out_sound, Some(at), 0.8);
                }
                self.magazine_out();
            }
            let puts_in = matches!(kind, ReloadKind::Swap | ReloadKind::Insert);
            if puts_in && !self.tools.guns.plan.in_done && due(RELOAD_MAG_IN) {
                self.tools.guns.plan.in_done = true;
                if let Some(f) = feed {
                    self.audio.play(f.in_sound, Some(at), 0.9);
                }
                self.magazine_in();
            }
            if rack && !self.tools.guns.plan.rack_done && due(RELOAD_SLIDE) {
                self.tools.guns.plan.rack_done = true;
                if let Some(f) = feed {
                    self.audio.play(f.rack_sound, Some(at), 0.9);
                }
                self.rack_slide();
            }
            self.tools.guns.reload = (!finished).then_some(t + dt);
        }
        let kind = held.map(|(_, k)| k);
        self.me.hand.aim = self.tools.guns.aim;
        self.me.hand.inspect = self.tools.guns.inspect.map(|(t, _)| t);
        let revolver = kind.is_some_and(|k| !k.uses_magazine());
        let cylinder = self.tools.guns.cylinder.filter(|_| revolver);
        self.me.hand.reload = match (self.tools.guns.reload, kind) {
            // The revolver's: where its reload animation is.
            (_, Some(_)) if revolver => cylinder.map(|c| c.anim().0 / crate::model::revolver_view::RELOAD_END),
            (Some(t), Some(_)) => Some(t / self.tools.guns.plan.length.max(0.01)),
            _ => None,
        };
        self.me.hand.gun_mods = mods;
        self.me.hand.gun_dirt = self.held_gun_dirt();
        // The gun as it is now, after this frame's reload steps (a round just seated, the
        // cylinder just turned on, a magazine just in): as it was at the start of the frame, the
        // gun would be drawn a step behind for a frame.
        let held = self.held_gun().filter(|_| !self.spectator());
        let plan = &self.tools.guns.plan;
        self.me.hand.gun_state = match held {
            // A revolver: its cylinder as it is, and what its reload does.
            Some((g, k)) if !k.uses_magazine() => GunAnim {
                chambered: gun_rounds(&g) > 0,
                cyl: g.data,
                load: cylinder.and_then(|c| c.anim().1),
                ejects: cylinder.is_some_and(|c| c.ejects()),
                loader: cylinder.map_or(0, |c| c.loader()),
                ..GunAnim::default()
            },
            Some((g, k)) => GunAnim {
                kind: plan.kind,
                rack: plan.rack,
                old_empty: plan.old_empty,
                locked: gun_locked(&g),
                no_mag: !gun_has_mag(&g),
                chambered: gun_chambered(&g),
                mag: gun_has_mag(&g).then(|| (gun_rounds(&g), k.magazine_size(gun_mods(&g)))),
                new_mag: plan.new_mag.map(|m| (gun_rounds(&m), magazine_capacity(m.item).unwrap_or(12))),
                ..GunAnim::default()
            },
            None => GunAnim { chambered: true, ..GunAnim::default() },
        };

        // The flash fades in a moment; a hot barrel smokes.
        if let Some(f) = &mut self.tools.guns.flash {
            f.0 -= dt / 0.06;
        }
        if self.tools.guns.flash.is_some_and(|f| f.0 <= 0.0) {
            self.tools.guns.flash = None;
        }
        self.tools.guns.heat = (self.tools.guns.heat - dt * 0.5).max(0.0);
        self.tools.guns.wisp -= dt;
        if held.is_some() && self.tools.guns.heat > 2.0 && self.tools.guns.wisp <= 0.0 {
            self.tools.guns.wisp = 0.35;
            if let Some(m) = self.muzzle_now() {
                let (sky, blk) = self.terrain.world.light_estimate(m);
                self.level.particles.gun_smoke(m, Vec3::Y * 0.4, 1, sky, blk);
            }
        }

    }

    /// Starts looking the held gun over (again from the start if already).
    pub(in crate::client) fn start_inspect(&mut self) {
        let Some((gun, _)) = self.held_gun() else { return };
        if self.tools.guns.reload.is_none() && self.tools.guns.aim < 0.3 {
            self.tools.guns.inspect = Some((0.0, gun.item));
        }
    }

    /// The R key: a round into the chamber if it is empty and the magazine has one;
    /// otherwise the magazine in it out and the fullest loaded one from the inventory in, or
    /// only the one in it out when there is no other, or only a new one in when there is none
    /// in it. The slide is pulled at the end when the chamber is empty.
    fn start_reload(&mut self) {
        let Some((gun, kind)) = self.held_gun() else { return };
        if self.tools.guns.reload.is_some() {
            return;
        }
        if !kind.uses_magazine() {
            self.start_revolver_reload();
            return;
        }
        let has_mag = gun_has_mag(&gun);
        let rounds = if has_mag { gun_rounds(&gun) } else { 0 };
        let chambered = gun_chambered(&gun);
        let (what, new_mag) = if has_mag && !chambered && rounds > 0 {
            (ReloadKind::Rack, None)
        } else {
            if has_mag && chambered && rounds >= kind.magazine_size(gun_mods(&gun)) {
                return;
            }
            match (has_mag, self.take_magazine(kind)) {
                (true, Some(m)) => (ReloadKind::Swap, Some(m)),
                (true, None) => (ReloadKind::Eject, None),
                (false, Some(m)) => (ReloadKind::Insert, Some(m)),
                (false, None) => {
                    self.gun_message(t("gun.no_mags"));
                    return;
                }
            }
        };
        let rack = match what {
            ReloadKind::Rack => true,
            ReloadKind::Eject => false,
            _ => !chambered,
        };
        self.tools.guns.plan = ReloadPlan {
            kind: what,
            rack,
            old_empty: rounds == 0,
            length: reload_seconds(what, rack),
            new_mag,
            ..ReloadPlan::default()
        };
        self.tools.guns.aim = 0.0;
        self.tools.guns.reload = Some(0.0);
    }

    /// Stops a reload (the gun was put away): the magazine it was bringing goes back.
    pub(in crate::client) fn cancel_reload(&mut self) {
        self.tools.guns.cylinder = None;
        self.tools.guns.reload_owner = None;
        if self.tools.guns.reload.take().is_some() {
            if let Some(m) = self.tools.guns.plan.new_mag.take() {
                if !self.creative() {
                    self.give(m);
                }
            }
        }
    }

    /// Holding a magazine with rounds in it for this gun (in creative there always is one).
    fn has_loaded_magazine(&self, kind: GunKind) -> bool {
        self.creative()
            || self.me.items.inventory.slots.iter().flatten().any(|s| magazine_gun(s.item) == Some(kind) && gun_rounds(s) > 0)
    }

    /// The fullest loaded magazine for this gun, taken out of the inventory (in creative a
    /// full new one).
    fn take_magazine(&mut self, kind: GunKind) -> Option<Stack> {
        if self.creative() {
            let item = kind.magazine_item()?;
            let mut m = Stack::one(item);
            set_gun_rounds(&mut m, magazine_capacity(item).unwrap_or(12));
            return Some(m);
        }
        let (i, _) = self
            .me.items.inventory
            .slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.filter(|s| magazine_gun(s.item) == Some(kind) && gun_rounds(s) > 0).map(|s| (i, gun_rounds(&s))))
            .max_by_key(|&(i, r)| (r, std::cmp::Reverse(i)))?;
        self.me.items.inventory.slots[i].take()
    }

    /// The held gun, to change it.
    fn held_gun_mut(&mut self) -> Option<&mut Stack> {
        let slot = self.me.items.hotbar_slot;
        self.me.items.inventory.slots[slot].as_mut().filter(|s| GunKind::of(s.item).is_some())
    }

    /// The magazine drops out of the grip onto the ground, with the rounds left in it (in
    /// creative it is gone). The slide stays where it is.
    fn magazine_out(&mut self) {
        let Some(gun) = self.held_gun_mut() else { return };
        if !gun_has_mag(gun) {
            return;
        }
        let mods = gun_mods(gun);
        let standard = GunKind::of(gun.item).and_then(|k| k.magazine_item()).unwrap_or(PISTOL_MAGAZINE);
        let item = if mods & gun_mod::EXTENDED_MAGAZINE != 0 { EXTENDED_MAGAZINE } else { standard };
        // As dirty as it looked in the gun (like one taken out at the gun station).
        let mut mag = Stack { damage: gun.damage, ..Stack::one(item) };
        set_gun_rounds(&mut mag, gun_rounds(gun));
        set_gun_rounds(gun, 0);
        set_gun_state(gun, gun_state::NO_MAG, true);
        set_gun_mods(gun, mods & !gun_mod::EXTENDED_MAGAZINE);
        if self.creative() {
            return;
        }
        let look = look_dir(self.me.look.yaw, 0.0);
        let right = look.cross(Vec3::Y).normalize_or_zero();
        let at = self.eye() - Vec3::Y * 0.7 + look * 0.35 + right * 0.15;
        let vel = self.me.body.vel * 0.8 + look * 0.4 - Vec3::Y * 0.5;
        self.add_item(crate::entity::dropped::ItemEntity::new(at, vel, mag, 1.0));
    }

    /// The new magazine is pushed into the grip.
    fn magazine_in(&mut self) {
        let Some(mag) = self.tools.guns.plan.new_mag.take() else { return };
        let Some(gun) = self.held_gun_mut() else {
            self.give(mag);
            return;
        };
        set_gun_state(gun, gun_state::NO_MAG, false);
        set_gun_rounds(gun, gun_rounds(&mag));
        let mods = gun_mods(gun) & !gun_mod::EXTENDED_MAGAZINE;
        let ext = if mag.item == EXTENDED_MAGAZINE { gun_mod::EXTENDED_MAGAZINE } else { 0 };
        set_gun_mods(gun, mods | ext);
    }

    /// The slide pulled back and let go: it takes the magazine's top round into the chamber,
    /// or, with none there, stays back on an empty magazine (or closes on nothing without one).
    fn rack_slide(&mut self) {
        let Some(gun) = self.held_gun_mut() else { return };
        let has_mag = gun_has_mag(gun);
        if has_mag && gun_rounds(gun) > 0 {
            set_gun_rounds(gun, gun_rounds(gun) - 1);
            set_gun_state(gun, gun_state::CHAMBER_EMPTY, false);
            set_gun_state(gun, gun_state::LOCKED, false);
        } else {
            set_gun_state(gun, gun_state::CHAMBER_EMPTY, true);
            set_gun_state(gun, gun_state::LOCKED, has_mag);
        }
    }

    /// Left click with a gun: fires (a dirty gun may jam, an empty one only clicks). A shotgun
    /// being loaded stops loading to fire.
    pub(in crate::client) fn shoot(&mut self) {
        let Some((gun, kind)) = self.held_gun() else { return };
        let stats = kind.stats();
        if self.tools.guns.cylinder.is_some() {
            // The revolver's cylinder is out: loading stops, to shoot.
            self.revolver_stop_loading();
            return;
        }
        if self.me.aim.action_cooldown > 0.0 || self.tools.guns.reload.is_some() {
            return;
        }
        // Firing ends looking it over.
        self.tools.guns.inspect = None;
        let slot = self.me.items.hotbar_slot;
        // The time overshot since the gun was ready counts (up to a slow frame's worth), so
        // the rate of fire does not drop with the frame rate.
        self.me.aim.action_cooldown = stats.fire_delay + self.me.aim.action_cooldown.max(-0.05);
        // Firing ends a sprint (the gun comes up to shoot), for a moment after.
        self.tools.guns.no_sprint = 0.4;
        self.input.w_sprint = false;
        let revolver = !kind.uses_magazine();
        if revolver && gun_rounds(&gun) == 0 && (self.creative() || self.me.items.inventory.count(kind.ammo()) > 0 || self.me.items.inventory.slots.iter().flatten().any(|s| s.item == SPEEDLOADER && gun_rounds(s) > 0)) {
            // Nothing live in the cylinder, and something to load it with: it only clicks (R
            // reloads), like the others.
            let key = crate::keys::display(self.settings.keys.get(Bind::Reload));
            self.dry_fire(Some(&tf("gun.empty_reload", &[&key])));
            return;
        }
        if !revolver && !gun_chambered(&gun) {
            // Nothing in the chamber: the trigger only clicks (R readies it, from the hip or
            // aimed alike).
            let can_reload = (gun_has_mag(&gun) && gun_rounds(&gun) > 0) || self.has_loaded_magazine(kind);
            if can_reload {
                let key = crate::keys::display(self.settings.keys.get(Bind::Reload));
                self.dry_fire(Some(&tf("gun.empty_reload", &[&key])));
            } else {
                self.dry_fire(Some(t("gun.no_ammo")));
            }
            return;
        }
        let dirt = gun.damage as f32 / stats.dirt_max as f32;
        // Dirt makes it jam more and more often; a completely dirty one does not fire at all. A
        // jam comes before the revolver's cylinder turns, so it costs no round.
        if dirt >= 1.0 || (dirt > 0.6 && self.random() < (dirt - 0.6) * 1.2) {
            self.dry_fire(Some(t("gun.jammed")));
            return;
        }
        // The revolver's next chamber comes under the hammer: only a live round there fires.
        if revolver && !self.revolver_pull() {
            self.dry_fire((gun_rounds(&gun) == 0).then(|| t("gun.no_ammo")));
            return;
        }
        let mods = gun_mods(&gun);
        let silenced = mods & gun_mod::SILENCER != 0;
        let creative = self.creative();
        if let Some(s) = &mut self.me.items.inventory.slots[slot] {
            // The next round comes up from the magazine into the chamber; after the last one
            // the chamber is empty, and an empty magazine holds the slide back. A revolver
            // fires the round under its hammer (its case stays in the cylinder).
            if revolver {
                // (its case stays in the chamber: `revolver_pull`)
            } else if gun_has_mag(s) && gun_rounds(s) > 0 {
                set_gun_rounds(s, gun_rounds(s) - 1);
            } else {
                set_gun_state(s, gun_state::CHAMBER_EMPTY, true);
                set_gun_state(s, gun_state::LOCKED, gun_has_mag(s));
            }
            if !creative {
                s.damage = (s.damage + 1).min(stats.dirt_max);
            }
        }
        let seed = self.random();
        self.me.hand.shoot(if silenced { 0.0 } else { stats.flash }, seed);

        let eye = self.eye();
        // The shot goes where the gun points (the scope's middle, with one), not where the
        // view looks; but not where a gun being looked over or carried at a run points (the
        // barrel far off the view: it would go sideways or back).
        let view = self.me.look.dir();
        let look = self.tools.guns.gun_dir.filter(|d| d.dot(view) > 0.985).unwrap_or(view);
        let aim = smoothstep(0.0, 1.0, self.tools.guns.aim);
        let spread = shot_spread(stats, mods, aim, self.tools.guns.bloom);
        self.tools.guns.bloom = (self.tools.guns.bloom + BLOOM_PER_SHOT).min(BLOOM_MAX);
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
            self.tools.guns.bullets.push(Bullet {
                pos: eye,
                vel: dir * speed,
                from: self.me.body.pos,
                offset: muzzle - eye,
                traveled: 0.0,
                range: stats.range,
                gravity: stats.gravity,
                damage: stats.damage,
                knockback: stats.knockback,
                visual: false,
                hole: hole_size(kind),
            });
        }
        // The others see the shot too.
        let shot = crate::net::Msg::Shot {
            // (the server puts in who shot)
            id: 0,
            kind: kind as u8,
            mods,
            eye,
            seed,
            bullets: sent,
        };
        self.send(shot);

        // Recoil: the view kicks up (and a little to the side); most of it comes back.
        let kick = (stats.kick_hip + (stats.kick_aimed - stats.kick_hip) * aim).to_radians();
        self.me.look.pitch = (self.me.look.pitch + kick).min(1.55);
        self.tools.guns.recover += kick * 0.6;
        self.me.look.yaw += (self.random() - 0.5) * (0.4 + kick.to_degrees() * 0.15).to_radians();

        // The spent case flies out of the ejection port (a revolver's when it is reloaded).
        if !revolver {
            self.eject_case(kind);
        }

        self.shot_fx(kind, silenced, muzzle, look, seed, true);
        // The barrel heats up.
        self.tools.guns.heat = (self.tools.guns.heat + 0.07 * stats.flash.max(0.8)).min(3.0);
    }

    /// A shot's sound, its muzzle flash (the held gun's, or another player's: `own`), the
    /// flash's light, sparks and a puff of smoke (a silencer leaves only a little smoke).
    fn shot_fx(&mut self, kind: GunKind, silenced: bool, muzzle: Vec3, look: Vec3, seed: f32, own: bool) {
        let shot = if silenced { Sound::ShotSilenced } else { kind.def().shot_sound };
        self.audio.play(shot, Some(muzzle), 1.0);
        let (sky, blk) = self.terrain.world.light_estimate(muzzle);
        let size = kind.stats().flash;
        if !silenced {
            let flash = (1.0, muzzle, look, 0.1 * size, seed);
            if own {
                self.tools.guns.flash = Some(flash);
            } else {
                self.tools.guns.remote_flashes.push(flash);
            }
            self.tools.guns.flash_light = ((0.6 + 0.2 * size).min(1.0), muzzle + look * 0.3);
            self.level.particles.sparks(muzzle, look, 3 + (size * 3.0) as usize);
        }
        let puff = if size > 1.5 { 2 } else { 1 };
        self.level.particles.gun_smoke(muzzle, look, puff, sky, blk);
    }

    /// A shot that does not go off: the trigger only clicks, with a message why (at most about
    /// once a second).
    fn dry_fire(&mut self, message: Option<&str>) {
        if let Some(m) = message {
            self.gun_message(m);
        }
        self.audio.play(Sound::DryFire, None, 0.8);
        self.me.hand.dry_fire();
    }

    /// Where the held gun's muzzle is now: on the first-person gun, or on the player model.
    fn muzzle_now(&self) -> Option<Vec3> {
        if self.me.look.camera.mode == 0 {
            self.tools.guns.muzzle
        } else {
            self.tools.guns.muzzle_tp
        }
    }

    /// A spent case (or shotgun hull) flies out to the right of the gun, tumbling.
    fn eject_case(&mut self, kind: GunKind) {
        let eye = self.eye();
        let look = self.me.look.dir();
        let right = look.cross(Vec3::Y).normalize_or_zero();
        let port = match (self.me.look.camera.mode, self.tools.guns.eject, self.tools.guns.eject_tp) {
            (0, Some(p), _) | (_, _, Some(p)) => p,
            _ => eye - Vec3::Y * 0.25 + look * 0.5 + right * 0.25,
        };
        self.throw_case(port, look, self.me.body.vel * 0.8, kind);
    }

    /// A spent case thrown out of `port` to the right of a gun pointing along `look`,
    /// tumbling, with `carry` (the shooter's own speed) on top.
    fn throw_case(&mut self, port: Vec3, look: Vec3, carry: Vec3, kind: GunKind) {
        let right = look.cross(Vec3::Y).normalize_or(Vec3::X);
        let up = right.cross(look);
        let r = |g: &mut Self| g.random() - 0.5;
        let vel = right * (2.4 + r(self)) + up * (2.6 + r(self)) - look * 0.6 + carry;
        let spin = Vec3::new(r(self), r(self), r(self)) * 40.0;
        self.tools.guns.cases.eject(port, vel, spin, kind);
    }

    /// Another player fired (`kind` is the gun's index in GUN_KINDS): their bullets fly
    /// (only to be seen), the muzzle flashes on the gun in their hands and the case comes
    /// out of it.
    pub(in crate::client) fn remote_shot(&mut self, id: u8, kind: u8, mods: u8, eye: Vec3, seed: f32, bullets: &[Vec3]) {
        let Some(&kind) = GUN_KINDS.get(kind as usize) else {
            return;
        };
        if eye.distance(self.me.body.pos) > 192.0 || bullets.is_empty() {
            return;
        }
        let stats = kind.stats();
        let look = bullets[0].normalize_or(Vec3::X);
        // Their slide flies back.
        let time = self.clock.time;
        if let Some(r) = self.session.remotes.iter_mut().find(|r| r.id == id) {
            r.shot_at = Some(time);
        }
        let muzzle = self
            .remote_gun_point(id, kind, crate::model::gun_view::muzzle(kind, mods))
            .unwrap_or(eye + look * 0.9);
        for &vel in bullets.iter().take(32) {
            self.tools.guns.bullets.push(Bullet {
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
                hole: hole_size(kind),
            });
        }
        self.shot_fx(kind, mods & gun_mod::SILENCER != 0, muzzle, look, seed, false);
        // A revolver keeps its cases until it is reloaded.
        let port = kind.uses_magazine().then(|| self.remote_gun_point(id, kind, crate::model::gun_view::eject(kind))).flatten();
        if let Some(port) = port {
            self.throw_case(port, look, Vec3::ZERO, kind);
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
            .level.mobs
            .iter()
            .filter_map(|m| m.ray_hit(eye, dir, reach))
            .fold(None, |a: Option<f32>, d| Some(a.map_or(d, |a| a.min(d))));
        let player = self.pick_other_player(eye, dir, reach, owner).map(|(_, d)| d);
        let me = owner.and_then(|_| {
            let p = self.me.body.pos;
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
        let mut bullets = std::mem::take(&mut self.tools.guns.bullets);
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
                .level.mobs
                .iter()
                .enumerate()
                .filter(|(_, m)| m.alive())
                .filter_map(|(i, m)| m.ray_hit(b.pos, dir, reach).map(|d| (i, d)))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            let player = self
                .pick_player(b.pos, dir, reach)
                .filter(|&(_, d)| mob.is_none_or(|(_, md)| d < md));

            use crate::net::Msg;
            let (dmg, knock, _from) = (b.damage, b.knockback, b.from);
            if b.visual {
                // Someone else's: it stops at the first mob but does nothing to it (the
                // shooter's game hits it), and passes the players.
                if mob.is_some() {
                    return false;
                }
            } else if let Some((id, _)) = player {
                let kind = crate::net::hurt::BULLET;
                self.send(Msg::AttackPlayer { id, dmg, knock, kind });
                return false;
            }
            if let Some((i, d)) = mob.filter(|_| !b.visual) {
                if self.level.mobs[i].kind == crate::content::mobs::TARGET_DUMMY {
                    // Straw flies out of the sack.
                    self.level.particles.impact(&self.terrain.world, b.pos + dir * d, -dir, WOOL, [224, 196, 118]);
                }
                let id = self.level.mobs[i].id;
                self.send(Msg::AttackMob { id, dmg, knock });
                return false;
            }
            if let Some((hit, d, normal)) = block {
                // Chips fly off where the bullet hit.
                let b_id = self.terrain.world.geti(hit);
                let tint = self.block_tint(hit, b_id);
                let n = normal.as_vec3();
                let at = b.pos + dir * d;
                self.level.particles
                    .impact(&self.terrain.world, at + n * 0.02, n, b_id, tint);
                self.audio.play(Sound::Impact, Some(at), 0.7);
                // A hole where it went in, as big as the gun's bullet makes.
                if self.tools.guns.holes.len() >= MAX_HOLES {
                    self.tools.guns.holes.remove(0);
                }
                let size = b.hole * (0.85 + 0.3 * self.random());
                let turn = self.random() * TAU;
                let mirrored = self.random() < 0.5;
                self.tools.guns.holes.push(Hole {
                    pos: at,
                    normal: n,
                    block: hit,
                    id: b_id,
                    turn,
                    mirrored,
                    size,
                    born: self.clock.time,
                });
                return false;
            }
            b.pos += step;
            b.vel.y -= b.gravity * dt;
            b.traveled += len;
            b.traveled < b.range
        });
        self.tools.guns.bullets.extend(bullets);
    }

    /// Tracer streaks of the bullets in flight, the spent cases and the laser's dot.
    pub(in crate::client) fn build_gun_effects(&self, out: &mut Vec<Vertex>, cam: Vec3, right: Vec3, up: Vec3) {
        for b in &self.tools.guns.bullets {
            let dir = b.vel.normalize_or_zero();
            let fade = (1.0 - b.traveled / 8.0).max(0.0);
            let head = b.pos + b.offset * fade;
            let tail_len = b.traveled.min(3.0);
            if tail_len > 0.3 {
                ballistics::emit_tracer(out, head - dir * tail_len, head, cam, 0.012, false);
            }
        }
        self.tools.guns.cases.build(out, &self.terrain.world);
        // The first-person view draws the flash on its own gun.
        if let (Some((k, pos, dir, size, seed)), true) = (self.tools.guns.flash, self.me.look.camera.mode != 0) {
            ballistics::emit_muzzle_flash(out, pos, dir, cam, size, seed, k);
        }
        for &(k, pos, dir, size, seed) in &self.tools.guns.remote_flashes {
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
            if let Some(from) = self.remote_gun_point(id, kind, crate::model::gun_view::laser(kind)) {
                ballistics::emit_tracer(out, from, p, cam, 0.004, true);
            }
        }
    }

    /// The held gun's laser sight, once the gun is where it is drawn this frame (so the dot
    /// does not trail it): zeroed onto where the shot goes (from the eye the way the gun
    /// points, see `shoot`), a small dot there, and the faint beam from the lens (first person).
    pub(in crate::client) fn build_own_laser(&self, out: &mut Vec<Vertex>, cam: Vec3, right: Vec3, up: Vec3) {
        let Some((gun, kind)) = self.held_gun() else { return };
        if gun_mods(&gun) & gun_mod::LASER == 0 || self.screen == Screen::Dead {
            return;
        }
        let range = kind.stats().range.min(120.0);
        let dir = self.tools.guns.gun_dir.unwrap_or_else(|| self.me.look.dir());
        let Some(p) = self.laser_hit(self.eye(), dir, range, None) else { return };
        // A small dot near by, still visible far away.
        let size = (0.006 + 0.004 * p.distance(cam)).min(0.1);
        ballistics::emit_laser_dot(out, p, right, up, size);
        if let Some(from) = self.tools.guns.laser_from.filter(|_| self.me.look.camera.mode == 0) {
            ballistics::emit_tracer(out, from, p, cam, 0.004, true);
        }
    }

    /// The bullet holes, multiplied onto the blocks they are in.
    pub(in crate::client) fn build_bullet_holes(&self, out: &mut Vec<Vertex>, cam: Vec3) {
        use crate::world::mesh::flags;
        for h in &self.tools.guns.holes {
            if h.pos.distance_squared(cam) > 64.0 * 64.0 {
                continue;
            }
            let left = HOLE_LIFE - (self.clock.time - h.born);
            let size = h.size * (left / 3.0).min(1.0);
            let n = h.normal;
            let a = if n.y.abs() > 0.5 { Vec3::X } else { Vec3::Y };
            let t1 = n.cross(a).normalize();
            let t2 = n.cross(t1);
            let (s, c) = h.turn.sin_cos();
            let (r, u) = ((t1 * c + t2 * s) * size, (t2 * c - t1 * s) * size);
            let u = if h.mirrored { -u } else { u };
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
    pub(in crate::client) fn draw_gun_hud(&mut self) {
        let Some((gun, kind)) = self.held_gun() else { return };
        let stats = kind.stats();
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        let playing = self.screen == Screen::Playing;
        let mods = gun_mods(&gun);
        let aim = self.tools.guns.aim;
        let center = Vec2::new((w * 0.5).round(), (h * 0.5).round());
        if playing && self.me.look.camera.mode != 2 && aim < 0.6 {
            // Four lines around a dot, as far apart as the shots scatter.
            let a = 1.0 - aim / 0.6;
            let spread = shot_spread(stats, mods, 0.0, self.tools.guns.bloom);
            let half_fov = (self.me.look.fov.to_radians() * 0.5).tan();
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

        // Rounds ready (in the magazine and the chamber) / rounds in the magazines carried,
        // bottom right, and the controls.
        let rounds = gun_ready_rounds(&gun);
        let size = kind.magazine_size(mods);
        let carried = if self.creative() {
            "-".to_string()
        } else if !kind.uses_magazine() {
            self.me.items.inventory.count(kind.ammo()).to_string()
        } else {
            let in_mags: u32 = self
                .me.items.inventory
                .slots
                .iter()
                .flatten()
                .filter(|s| magazine_gun(s.item) == Some(kind))
                .map(|s| gun_rounds(s) as u32)
                .sum();
            in_mags.to_string()
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
        let note = if let Some(t) = self.tools.guns.reload {
            // A bar filling up under the counter.
            let (bx, by, bw2) = (x - 20.0 * s, y + 14.0 * s, bw + sw + 20.0 * s);
            self.ui.solid(bx, by, bw2, 2.0 * s, rgba(0, 0, 0, 160));
            let k = (t / self.tools.guns.plan.length.max(0.01)).min(1.0);
            self.ui
                .solid(bx, by, bw2 * k, 2.0 * s, rgba(120, 230, 140, 255));
            t_owned("gun.reloading")
        } else if kind.uses_magazine() && !gun_has_mag(&gun) {
            tf("gun.no_mag", &[&key])
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
    pub(in crate::client) fn gun_message(&mut self, text: &str) {
        if self.clock.time - self.tools.guns.last_message > 1.0 {
            self.tools.guns.last_message = self.clock.time;
            self.say(text, rgba(255, 190, 110, 255));
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
        let mut s = Stack::one(PISTOL);
        set_gun_mods(&mut s, gun_mod::SCOPE | gun_mod::LASER);
        set_gun_rounds(&mut s, 20);
        assert_eq!(gun_rounds(&s), 20);
        assert_eq!(gun_mods(&s), gun_mod::SCOPE | gun_mod::LASER);
        set_gun_rounds(&mut s, 3);
        assert_eq!(gun_mods(&s), gun_mod::SCOPE | gun_mod::LASER);
        assert_eq!(GunKind::Pistol.magazine_size(0), 12);
        assert_eq!(GunKind::Pistol.magazine_size(gun_mod::EXTENDED_MAGAZINE), 20);
    }
}
