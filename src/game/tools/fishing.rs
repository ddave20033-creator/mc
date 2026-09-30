//! Fishing. With the rod in the hand the right mouse button held draws it back (the longer,
//! the farther it will cast, up to `CHARGE_TIME`); let go, it whips forward and the bobber
//! flies out, the line running off the reel. On water it floats and waits for a fish:
//! nibbles first (it twitches), then the bite (pulled under, with a splash): a notch of the
//! wheel toward you in time sets the hook.
//!
//! The mouse wheel works the reel: toward you reels in, away lets line out; with Shift (no
//! sneaking with the rod in hand) it shifts the reel's gear (1 slow and strong .. 5 fast and
//! weak). A hooked fish fights (`Fight`): it pulls, surges away and rests, and the line's
//! tension has to be kept in the middle (the bar under the crosshair) by reeling as fast as
//! it lets you, in a gear that suits what it does: too tight too long, the line snaps; too
//! slack too long, the fish shakes the hook. Kept in the middle, the fish tires; brought
//! close, it is landed.
//!
//! Only the angler's own game runs this; the others see the rod, the line and the bobber from
//! the pose (`net::Pose::rod`).

use crate::game::*;
use crate::audio::Sound;
use crate::item::inventory::damage;
use crate::item::{rod_gear, set_rod_gear, Stack, FISHING_ROD, RAW_FISH, ROD_GEARS};
use crate::model::angler::{self, RodAnim, CAST_TIME, LIFT_TIME, WHIP_FORWARD};
use crate::util::vertex_light;
use crate::world::mesh::fluid_height;

/// Seconds of drawing back for the farthest cast.
const CHARGE_TIME: f32 = 1.2;
/// The most line on the reel (m).
pub(in crate::game) const MAX_LINE: f32 = 64.0;
/// How fast the bobber leaves the rod: the weakest and the strongest cast (about 4 and 35 m).
const CAST_SPEED: (f32, f32) = (7.5, 22.0);
const GRAVITY: f32 = 14.0;
/// Seconds after the bite to set the hook.
const STRIKE_TIME: f32 = 1.7;
/// A fish this close (m of line) is landed.
const LAND_DIST: f32 = 2.2;
/// Line (m) reeled in or let out per notch of the wheel in each gear.
pub(in crate::game) const PER_NOTCH: [f32; ROD_GEARS as usize] = [0.3, 0.45, 0.65, 0.9, 1.2];
/// How much a pulling fish stops the reel in each gear (a strong low gear keeps winning line
/// against it, a fast high one only while it rests).
const STALL: [f32; ROD_GEARS as usize] = [0.0, 0.3, 0.6, 0.85, 1.1];
/// How much a notch of the wheel tightens the line (in) and eases it (out), by gear: fine
/// in the low gears, coarse in the high ones.
const TIGHTEN: [f32; ROD_GEARS as usize] = [0.03, 0.045, 0.06, 0.08, 0.1];
const EASE: [f32; ROD_GEARS as usize] = [0.035, 0.05, 0.065, 0.085, 0.105];
/// The tension's zones: the middle (the fish tires fast), too tight (it snaps if it lasts),
/// too slack (the fish gets away if it lasts); the tension the line snaps at at once.
pub(in crate::game) const GOOD: (f32, f32) = (0.32, 0.7);
pub(in crate::game) const TIGHT: f32 = 0.85;
pub(in crate::game) const SLACK: f32 = 0.2;
const BREAK: f32 = 1.3;
const OVER_TIME: f32 = 1.2;
const SLACK_TIME: f32 = 3.0;

/// The kinds of fish: English name, Hungarian name (as caught: "Fogtál egy ... -os pontyot"),
/// their weights (kg), how strong they are, how common, and their colours (back, belly).
struct Species {
    en: &'static str,
    hu: &'static str,
    kg: (f32, f32),
    strength: f32,
    common: f32,
    tint: [[u8; 3]; 2],
}

const SPECIES: [Species; 6] = [
    Species { en: "perch", hu: "sügért", kg: (0.1, 0.8), strength: 0.55, common: 30.0, tint: [[96, 118, 60], [226, 190, 120]] },
    Species { en: "bream", hu: "keszeget", kg: (0.3, 2.5), strength: 0.7, common: 24.0, tint: [[150, 146, 120], [214, 208, 186]] },
    Species { en: "trout", hu: "pisztrángot", kg: (0.4, 3.5), strength: 1.0, common: 16.0, tint: [[92, 110, 104], [232, 196, 196]] },
    Species { en: "carp", hu: "pontyot", kg: (1.0, 12.0), strength: 1.05, common: 18.0, tint: [[128, 104, 52], [222, 186, 110]] },
    Species { en: "pike", hu: "csukát", kg: (1.5, 10.0), strength: 1.25, common: 8.0, tint: [[82, 104, 62], [206, 212, 170]] },
    Species { en: "catfish", hu: "harcsát", kg: (4.0, 35.0), strength: 1.45, common: 3.0, tint: [[62, 62, 58], [170, 166, 150]] },
];

/// A hooked fish and the line's fight with it (no world in it: `step` is all of it).
#[derive(Clone, Debug)]
pub(in crate::game) struct Fight {
    pub(in crate::game) species: usize,
    pub(in crate::game) weight: f32,
    pub(in crate::game) stamina: f32,
    pub(in crate::game) max_stamina: f32,
    /// How strong its surges are.
    power: f32,
    /// How hard it pulls now (about 0.1 resting .. 2 surging).
    pub(in crate::game) pull: f32,
    /// How taut the line is (see `GOOD`, `TIGHT`, `SLACK`, `BREAK`).
    pub(in crate::game) tension: f32,
    /// Line out to it (m).
    pub(in crate::game) dist: f32,
    /// Notches per second of the wheel, lately (+ in, - out).
    crank_rate: f32,
    /// Line reeled (m, - let out) not yet come in: it comes in smoothly, not a notch at once.
    reel_left: f32,
    /// Seconds to its next move; how fast it moves the line now (tension per second, +
    /// away, - toward the angler), the move's, and how quickly it goes over to it.
    next: f32,
    drift: f32,
    drift_to: f32,
    snap: f32,
    /// Seconds (weighted) too tight, and too slack.
    pub(in crate::game) over: f32,
    pub(in crate::game) slack: f32,
    /// Which way it swims across (radians per second about the angler) and wants to.
    side: f32,
    side_to: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::game) enum FightEnd {
    Landed,
    Snapped,
    Escaped,
}

impl Fight {
    /// A fish of a kind chosen by how common they are (`r`: random numbers 0..1), `dist` out.
    pub(in crate::game) fn new(dist: f32, r: &mut dyn FnMut() -> f32) -> Self {
        let total: f32 = SPECIES.iter().map(|s| s.common).sum();
        let mut pick = r() * total;
        let mut species = 0;
        for (i, s) in SPECIES.iter().enumerate() {
            if pick < s.common {
                species = i;
                break;
            }
            pick -= s.common;
        }
        // Mostly small ones: the big ones are rare.
        let (lo, hi) = SPECIES[species].kg;
        let weight = lo + (hi - lo) * r().powf(2.4);
        Self::of(species, weight, dist)
    }

    pub(in crate::game) fn of(species: usize, weight: f32, dist: f32) -> Self {
        let sp = &SPECIES[species];
        let big = (weight / sp.kg.1).clamp(0.0, 1.0);
        let stamina = 4.0 + 6.0 * weight.sqrt();
        Fight {
            species,
            weight,
            stamina,
            max_stamina: stamina,
            power: sp.strength * (0.65 + 0.45 * big),
            pull: 0.5,
            tension: 0.45,
            dist,
            crank_rate: 0.0,
            reel_left: 0.0,
            next: 0.8,
            drift: 0.0,
            drift_to: 0.0,
            snap: 4.0,
            over: 0.0,
            slack: 0.0,
            side: 0.0,
            side_to: 0.0,
        }
    }

    /// How much fight it has left (0..1).
    pub(in crate::game) fn fresh(&self) -> f32 {
        (self.stamina / self.max_stamina).clamp(0.0, 1.0)
    }

    pub(in crate::game) fn surging(&self) -> bool {
        self.drift > 0.2
    }

    /// `dt` seconds of the fight, the wheel turned `notches` (+ in, - out) in `gear`.
    /// Returns the line reeled in (m, - let out) and how it ended, if it did.
    pub(in crate::game) fn step(&mut self, dt: f32, notches: i32, gear: u8, r: &mut dyn FnMut() -> f32) -> (f32, Option<FightEnd>) {
        let g = (gear.clamp(1, ROD_GEARS) - 1) as usize;
        let s = self.fresh();
        // What it does next, move after move, each its own speed and strength (weaker as it
        // tires): a run away (the line tightens: give it line), a dash toward the angler (the
        // line goes slack: reel in fast), shaking its head (quick turns either way), or
        // holding still.
        self.next -= dt;
        if self.next <= 0.0 {
            let might = (self.power * (0.35 + 0.65 * s)).min(0.95);
            let roll = r();
            let (drift, secs) = if roll < 0.3 + 0.15 * s {
                ((0.25 + 0.6 * r()) * might, 0.7 + 1.8 * r())
            } else if roll < 0.62 {
                (-(0.25 + 0.55 * r()) * (0.5 + 0.5 * might), 0.6 + 1.4 * r())
            } else if roll < 0.82 {
                let sign = if self.drift_to > 0.0 { -1.0 } else { 1.0 };
                (sign * (0.35 + 0.5 * r()) * might, 0.3 + 0.5 * r())
            } else {
                ((r() - 0.5) * 0.15, 0.8 + 1.5 * r())
            };
            self.drift_to = drift;
            self.next = secs;
            self.side_to = (r() - 0.5) * if drift > 0.2 { 0.5 } else { 0.15 };
            // (how fast it goes over to the new move: some are sudden, some gradual)
            self.snap = 2.0 + 8.0 * r();
        }
        self.drift += (self.drift_to - self.drift) * (1.0 - (-self.snap * dt).exp());
        self.pull = (0.5 + self.drift).clamp(0.05, 2.0);
        self.side += (self.side_to - self.side) * (1.0 - (-1.5 * dt).exp());
        // The wheel: its pace, lately.
        let k = 1.0 - (-dt / 0.3).exp();
        self.crank_rate += (notches as f32 / dt.max(1e-4) - self.crank_rate) * k;
        // The line: the fish moves it, a notch in tightens it, a notch out eases it (by more
        // in the higher gears); left alone it slowly goes slack.
        let step = if notches > 0 { TIGHTEN[g] } else { EASE[g] };
        self.tension += self.drift * dt - 0.12 * dt + notches as f32 * step;
        self.tension = self.tension.max(0.0);
        // Line in (as much as the fish lets the gear win), or out.
        let reeled = if notches > 0 {
            notches as f32 * PER_NOTCH[g] * (1.0 - self.pull.min(1.2) * STALL[g]).max(0.0)
        } else {
            notches as f32 * PER_NOTCH[g]
        };
        let before = self.dist;
        self.reel_left += reeled;
        let now = self.reel_left * (1.0 - (-12.0 * dt).exp());
        self.reel_left -= now;
        self.dist -= now;
        // Running, it takes line; coming in, it gives it.
        self.dist += self.drift.max(0.0) * 1.6 * dt;
        // Past the drag, it takes line off the reel.
        if self.tension > 0.72 {
            self.dist += (self.tension - 0.72) * 3.0 * dt;
        }
        self.dist = self.dist.clamp(0.0, MAX_LINE);
        // It tires, fastest while the line is kept in the middle.
        let good = (GOOD.0..GOOD.1).contains(&self.tension);
        let drain = 0.2 + if good { 1.0 } else { 0.0 } + self.pull * self.tension.min(1.0) * 0.5;
        self.stamina = (self.stamina - drain * dt).max(0.0);
        // Too tight or too slack for too long.
        if self.tension > TIGHT {
            self.over += dt * (1.0 + (self.tension - TIGHT) * 8.0);
        } else {
            self.over = (self.over - dt * 0.6).max(0.0);
        }
        if self.tension < SLACK {
            self.slack += dt;
        } else {
            self.slack = (self.slack - dt * 1.5).max(0.0);
        }
        // All the line run off the reel: it parts.
        if self.dist >= MAX_LINE && self.tension > GOOD.1 {
            self.over += dt * 2.0;
        }
        let end = if self.over > OVER_TIME || self.tension > BREAK {
            Some(FightEnd::Snapped)
        } else if self.slack > SLACK_TIME {
            Some(FightEnd::Escaped)
        } else if self.dist <= LAND_DIST {
            Some(FightEnd::Landed)
        } else {
            None
        };
        (before - self.dist, end)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::game) enum Bobber {
    Flying,
    Floating,
    Ground,
}

/// Waiting for a fish (seconds), nibbles to come (how many more, seconds to the next), the
/// bite (seconds left to set the hook).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(in crate::game) enum Bite {
    Wait(f32),
    Nibble(u8, f32),
    Strike(f32),
}

/// The line out and what is on it.
pub(in crate::game) struct Line {
    pub(in crate::game) bobber: Vec3,
    vel: Vec3,
    pub(in crate::game) state: Bobber,
    /// Line off the reel (m).
    pub(in crate::game) length: f32,
    pub(in crate::game) bite: Bite,
    pub(in crate::game) fight: Option<Fight>,
    /// Reeling it all in (a right click with the line out).
    auto_reel: bool,
    /// How far the bobber is pulled under (m), and where it is drawn.
    dip: f32,
    dip_now: f32,
    /// The bobber being dragged across (for its tilt and the wake).
    dragged: f32,
}

#[derive(Default)]
pub(in crate::game) struct Fishing {
    /// Drawing the rod back to cast: seconds the button has been held.
    charge: Option<f32>,
    /// The cast's whip playing (seconds), how hard it was, whether the bobber has left.
    cast: Option<f32>,
    cast_power: f32,
    /// A landed fish: the rod swung up (seconds).
    lift: Option<f32>,
    pub(in crate::game) line: Option<Line>,
    /// The reel's handle (radians), and the wheel's notches not yet used.
    crank: f32,
    scroll: f32,
    notches: i32,
    /// Where the rod's tip was drawn last frame: by the first-person hand and on the model.
    pub(in crate::game) tip_fp: Option<Vec3>,
    pub(in crate::game) tip_tp: Option<Vec3>,
    /// Fighting (0..1, eased), and the line's tension as shown.
    fight_k: f32,
    tension: f32,
    /// A landed fish flying out to the player: from where, seconds, kind, weight.
    flying: Option<(Vec3, f32, usize, f32)>,
    /// The line just parted: its loose end (seconds since, where it was).
    snapped: Option<(f32, Vec3)>,
    /// A message over the hotbar (text, seconds left, colour).
    toast: Option<(String, f32, Color)>,
    /// The gear digits flash when the gear is changed.
    gear_flash: f32,
    /// A bite not seen to yet (the "!" over the crosshair flashes).
    pub(in crate::game) alarm: f32,
    /// Where the reel's handle is going (it follows smoothly).
    crank_to: f32,
    /// Where the bobber is drawn: it follows the game's bobber smoothly (the line's length
    /// changes a notch at a time).
    bob_draw: Option<Vec3>,
    /// The bobber hanging from the tip with no line out: where it is and how it moves.
    hang: Option<(Vec3, Vec3)>,
    /// The point it hangs from (the tip, smoothed).
    hang_tip: Option<Vec3>,
    /// The HUD's shown values, following the real ones smoothly: how far it has come in, the
    /// gear (the highlight slides), the tension (a spring: value, speed), the line out, the
    /// fight bar and the cast's strength coming in, and the fish's strength left.
    hud_in: f32,
    hud_gear: f32,
    hud_tension: (f32, f32),
    hud_len: f32,
    hud_fight: f32,
    hud_charge: f32,
    hud_fresh: f32,
}

/// The top of the water at `p` (in it or just over it).
fn water_surface(world: &World, p: Vec3) -> Option<f32> {
    let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
    let mut y = p.y.floor() as i32;
    if !is_water(world.get(x, y, z)) {
        if is_water(world.get(x, y - 1, z)) {
            y -= 1;
        } else {
            return None;
        }
    }
    while y < HEIGHT as i32 - 1 && is_water(world.get(x, y + 1, z)) {
        y += 1;
    }
    Some(y as f32 + fluid_height(fluid_level(world.get(x, y, z))))
}

/// The top of the ground at (x, z) near height `y` (a block to climb onto, or down to three to
/// fall to).
fn ground_top(world: &World, p: Vec3) -> Option<f32> {
    let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
    let y0 = p.y.floor() as i32;
    (y0 - 3..=y0 + 1)
        .rev()
        .find(|&y| is_solid(world.get(x, y, z)) && !is_solid(world.get(x, y + 1, z)))
        .map(|y| y as f32 + 1.0)
}

fn solid_at(world: &World, p: Vec3) -> bool {
    is_solid(world.get(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32))
}

/// Enough water around the bobber for fish (a puddle has none).
fn open_water(world: &World, p: Vec3) -> bool {
    let c = p.floor().as_ivec3() - IVec3::Y;
    let mut n = 0;
    for dx in -1..=1 {
        for dz in -1..=1 {
            n += is_water(world.geti(c + IVec3::new(dx, 0, dz))) as i32;
        }
    }
    n >= 5
}

/// How deep the water is under the bobber (blocks, up to 4).
fn depth(world: &World, p: Vec3) -> i32 {
    let c = p.floor().as_ivec3();
    (0..4).take_while(|d| is_water(world.geti(c - IVec3::Y * *d))).count() as i32
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// "2.4" or "2,4" (Hungarian).
fn kilos(w: f32) -> String {
    let s = format!("{w:.1}");
    if crate::lang::is_hungarian() {
        s.replace('.', ",")
    } else {
        s
    }
}

impl Game {
    fn holding_rod(&self) -> bool {
        self.held() == FISHING_ROD && !self.spectator() && self.player.spawned && self.screen != Screen::Dead
    }

    fn rod_stack(&self) -> Option<Stack> {
        self.inventory.slots[self.hotbar_slot].filter(|s| s.item == FISHING_ROD)
    }

    /// The gear the held rod's reel is in.
    fn rod_gear(&self) -> u8 {
        self.rod_stack().map_or(3, |s| rod_gear(&s))
    }

    /// Where the line leaves the rod (as it was drawn last frame, from the eyes or from
    /// outside), or about there.
    fn rod_tip(&self) -> Vec3 {
        let tip = if self.camera.mode == 0 { self.fishing.tip_fp } else { self.fishing.tip_tp };
        let eye = self.player.eye();
        tip.or(self.fishing.tip_tp)
            .filter(|t| t.distance(eye) < 4.0)
            .unwrap_or_else(|| eye + look_dir(self.yaw, self.pitch) * 1.8 + Vec3::Y * 0.6)
    }

    /// The mouse wheel with the rod in hand: Shift shifts the reel's gear; with the line out
    /// (or drawing back to cast) it turns the reel. Returns whether the wheel was used for it
    /// (otherwise it goes through the hotbar).
    pub(in crate::game) fn fishing_scroll(&mut self) -> bool {
        if !self.holding_rod() {
            self.fishing.scroll = 0.0;
            return false;
        }
        let f = &mut self.fishing;
        f.scroll += self.input.scroll;
        let whole = f.scroll.trunc();
        f.scroll -= whole;
        let n = whole as i32;
        if self.bind_down(Bind::Sneak) {
            // Away from you: up a gear.
            if n != 0 {
                let slot = self.hotbar_slot;
                if let Some(s) = self.inventory.slots[slot].as_mut() {
                    let before = rod_gear(s);
                    let g = (before as i32 + n).clamp(1, ROD_GEARS as i32) as u8;
                    set_rod_gear(s, g);
                    if g != before {
                        let pitch = 0.85 + 0.08 * g as f32;
                        self.audio.play_pitched(Sound::GearClick, None, 0.55, pitch);
                        self.fishing.gear_flash = 0.4;
                    }
                }
            }
            return true;
        }
        if self.fishing.line.is_some() || self.fishing.charge.is_some() || self.fishing.cast.is_some() {
            // Toward you (the wheel turned down) reels in.
            self.fishing.notches -= n;
            return true;
        }
        self.fishing.scroll = 0.0;
        false
    }

    /// Something the right button opens is under the crosshair (it does not cast then).
    fn opens_target(&self) -> bool {
        let Some((hit, _)) = self.target else { return false };
        let b = self.terrain.world.geti(hit);
        b == CRAFTING_TABLE || is_gun_bench(b) || b == GUN_STATION || is_furnace(b) || is_door(b) || is_bed(b) || is_chest(b)
    }

    /// The rod in hand: casting, the bobber, bites and the fight. Putting it away cuts the
    /// line.
    pub(in crate::game) fn update_fishing(&mut self, dt: f32, control: bool) {
        let notches = std::mem::take(&mut self.fishing.notches);
        {
            let f = &mut self.fishing;
            f.gear_flash = (f.gear_flash - dt).max(0.0);
            f.alarm = (f.alarm - dt).max(0.0);
            if let Some((_, t, _)) = &mut f.toast {
                *t -= dt;
            }
            if f.toast.as_ref().is_some_and(|t| t.1 <= 0.0) {
                f.toast = None;
            }
            f.flying = f.flying.map(|(p, t, s, w)| (p, t + dt, s, w)).filter(|v| v.1 < 0.6);
            f.snapped = f.snapped.map(|(t, p)| (t + dt, p)).filter(|v| v.0 < 0.8);
            f.lift = f.lift.map(|t| t + dt).filter(|&t| t < LIFT_TIME);
        }
        if !self.holding_rod() {
            let f = &mut self.fishing;
            f.line = None;
            f.charge = None;
            f.cast = None;
            f.lift = None;
            f.fight_k = 0.0;
            f.tension = 0.0;
            f.hud_in = 0.0;
            f.hang = None;
            f.hang_tip = None;
            f.bob_draw = None;
            return;
        }
        // Drawing back, and letting go to cast.
        if control && self.input.right_pressed {
            let can_cast = self.fishing.cast.is_none() && !self.opens_target() && self.action_cooldown <= 0.0;
            match &mut self.fishing.line {
                Some(line) if line.fight.is_none() => line.auto_reel = true,
                Some(_) => {}
                None if can_cast => {
                    self.fishing.charge = Some(0.0);
                }
                None => {}
            }
        }
        if let Some(c) = self.fishing.charge {
            if !control {
                self.fishing.charge = None;
            } else if self.input.right_down {
                self.fishing.charge = Some(c + dt);
            } else {
                self.fishing.charge = None;
                self.fishing.cast = Some(0.0);
                self.fishing.cast_power = smooth(c / CHARGE_TIME).max(0.05);
                self.audio.play(Sound::FishCast, None, 0.35 + 0.45 * self.fishing.cast_power);
            }
        }
        if let Some(t) = self.fishing.cast {
            let at = WHIP_FORWARD * CAST_TIME;
            if t < at && t + dt >= at {
                self.launch_bobber();
            }
            self.fishing.cast = Some(t + dt).filter(|&t| t < CAST_TIME);
        }
        self.update_line(dt, notches);
        // What the rod shows.
        let f = &mut self.fishing;
        let (fighting, want) = match &f.line {
            Some(Line { fight: Some(fi), .. }) => (true, fi.tension),
            Some(l) if l.dragged > 0.0 => (false, 0.2),
            Some(Line { bite: Bite::Strike(_), state: Bobber::Floating, .. }) => (false, 0.3),
            _ => (false, 0.0),
        };
        f.fight_k += ((fighting as i32 as f32) - f.fight_k) * (1.0 - (-4.0 * dt).exp());
        f.tension += (want - f.tension) * (1.0 - (-10.0 * dt).exp());
        self.smooth_fishing(dt);
    }

    /// What is drawn follows the game smoothly: the reel's handle, the bobber, the bobber
    /// swinging from the tip, and the HUD's values.
    fn smooth_fishing(&mut self, dt: f32) {
        let ease = |rate: f32| 1.0 - (-rate * dt).exp();
        let gear = self.rod_gear() as f32;
        let tip = self.rod_tip();
        let f = &mut self.fishing;
        f.crank += (f.crank_to - f.crank) * ease(14.0);

        // The bobber out on the line.
        match &f.line {
            Some(l) => {
                let to = l.bobber;
                f.bob_draw = match f.bob_draw {
                    Some(b) if l.state != Bobber::Flying && b.distance(to) < 4.0 => Some(b.lerp(to, ease(9.0))),
                    _ => Some(to),
                };
                f.hang = None;
            }
            None => f.bob_draw = None,
        }

        // Hanging from the tip: a pendulum on a short line, swung by the rod's moves.
        if f.line.is_none() {
            const HANG: f32 = 0.55;
            let (mut p, mut v) = f.hang.filter(|(p, _)| p.distance(tip) < 3.0).unwrap_or((tip - Vec3::Y * HANG, Vec3::ZERO));
            let steps = 4;
            let h = dt.min(0.05) / steps as f32;
            // (the tip's jolts smoothed out first, so it sways instead of flailing)
            let anchor = match f.hang_tip {
                Some(a) if a.distance(tip) < 3.0 => a.lerp(tip, ease(10.0)),
                _ => tip,
            };
            f.hang_tip = Some(anchor);
            for _ in 0..steps {
                v.y -= 12.0 * h;
                v *= (-4.5 * h).exp();
                let before = p;
                p += v * h;
                let d = p - anchor;
                if d.length() > HANG {
                    p = anchor + d.normalize() * HANG;
                }
                v = ((p - before) / h).clamp_length_max(2.5);
            }
            f.hang = Some((p, v));
        }

        // The HUD.
        let fight = f.line.as_ref().and_then(|l| l.fight.as_ref());
        let tension = fight.map_or(f.tension, |fi| fi.tension);
        // (the reel's panel only once the bobber is out on the water)
        let in_water = f.line.as_ref().is_some_and(|l| l.state == Bobber::Floating || l.fight.is_some());
        f.hud_in += ((in_water as i32 as f32) - f.hud_in) * ease(8.0);
        f.hud_gear += (gear - f.hud_gear) * ease(16.0);
        if (f.hud_gear - gear).abs() > 5.0 {
            f.hud_gear = gear;
        }
        // (a stiff, well damped spring: it keeps up, with a little life in it)
        let (x, v) = f.hud_tension;
        let a = (tension - x) * 260.0 - v * 26.0;
        let v = v + a * dt.min(0.05);
        f.hud_tension = (x + v * dt.min(0.05), v);
        let len = f.line.as_ref().map_or(0.0, |l| l.length);
        f.hud_len += (len - f.hud_len) * ease(10.0);
        f.hud_fight += ((fight.is_some() as i32 as f32) - f.hud_fight) * ease(7.0);
        f.hud_fresh += (fight.map_or(f.hud_fresh, |fi| fi.fresh()) - f.hud_fresh) * ease(6.0);
        let charging = f.charge.map(|c| smooth(c / CHARGE_TIME));
        f.hud_charge = match charging {
            Some(k) => k,
            None => f.hud_charge * (1.0 - ease(10.0)),
        };
    }

    /// The bobber leaves the rod's tip at the whip's furthest forward, as hard as it was
    /// drawn back, toward where the crosshair is.
    fn launch_bobber(&mut self) {
        let k = self.fishing.cast_power;
        let (lo, hi) = CAST_SPEED;
        let speed = lo + (hi - lo) * k.powf(0.8);
        let look = look_dir(self.yaw, self.pitch);
        // Up a little over the look (a cast goes up and out).
        let flat = Vec3::new(look.x, 0.0, look.z).normalize_or(Vec3::X);
        let up = (look.y + 0.35).clamp(-0.3, 0.9);
        let dir = (flat + Vec3::Y * up).normalize();
        let from = self.rod_tip();
        self.fishing.line = Some(Line {
            bobber: from,
            vel: dir * speed + self.player.vel * 0.5,
            state: Bobber::Flying,
            length: 0.0,
            bite: Bite::Wait(0.0),
            fight: None,
            auto_reel: false,
            dip: 0.0,
            dip_now: 0.0,
            dragged: 0.0,
        });
        self.audio.play(Sound::LineZip, None, 0.5);
        self.action_cooldown = 0.3;
    }

    /// A new wait for a fish: shorter in deeper water.
    fn bite_wait(&mut self, at: Vec3) -> f32 {
        let d = depth(&self.terrain.world, at) as f32;
        (6.0 + 16.0 * self.random()) * (1.0 - 0.08 * d)
    }

    fn toast(&mut self, text: String, color: Color) {
        self.fishing.toast = Some((text, 3.0, color));
    }

    /// The bobber flies, floats or lies; the reel pulls it in; fish bite; the fight goes on.
    fn update_line(&mut self, dt: f32, notches: i32) {
        let Some(mut line) = self.fishing.line.take() else { return };
        let tip = self.rod_tip();
        let gear = self.rod_gear();
        let g = (gear - 1) as usize;
        let world = &self.terrain.world;
        // Where the bobber goes on its own.
        match line.state {
            Bobber::Flying => {
                line.vel.y -= GRAVITY * dt;
                line.vel *= (-0.25 * dt).exp();
                let mut p = line.bobber;
                let mut landed = false;
                for axis in [1, 0, 2] {
                    let mut q = p;
                    q[axis] += line.vel[axis] * dt;
                    if solid_at(world, q) {
                        if axis == 1 && line.vel.y < 0.0 {
                            landed = true;
                        }
                        line.vel[axis] = 0.0;
                    } else {
                        p = q;
                    }
                }
                line.bobber = p;
                if let Some(s) = water_surface(world, p).filter(|&s| p.y <= s) {
                    line.bobber.y = s;
                    let hit = line.vel.length();
                    line.vel = Vec3::ZERO;
                    line.state = Bobber::Floating;
                    line.dip_now = -0.12;
                    let wait = self.bite_wait(p);
                    line.bite = Bite::Wait(wait);
                    self.audio.play(Sound::BobberPlop, Some(p), (0.4 + hit / 20.0).min(1.0));
                    let (sky, blk) = self.terrain.world.light_estimate(p + Vec3::Y);
                    self.particles.splash(Vec3::new(p.x, s, p.z), 10, (hit / 25.0).min(1.0), sky, blk);
                } else if landed && line.vel.length() < 0.5 {
                    line.state = Bobber::Ground;
                    line.vel = Vec3::ZERO;
                } else if landed {
                    // Skidding along the ground.
                    line.vel.x *= 0.5;
                    line.vel.z *= 0.5;
                }
                // The line runs off the reel as it flies (to the end of it).
                let d = line.bobber.distance(tip);
                if d > MAX_LINE {
                    let out = (line.bobber - tip).normalize();
                    line.bobber = tip + out * MAX_LINE;
                    line.vel -= out * line.vel.dot(out).max(0.0);
                }
                line.length = line.length.max(d.min(MAX_LINE));
            }
            Bobber::Floating => {
                let world = &self.terrain.world;
                match water_surface(world, line.bobber) {
                    Some(s) if line.fight.is_none() => line.bobber.y = s,
                    Some(_) => {}
                    None if line.fight.is_none() => {
                        line.state = Bobber::Flying;
                        line.vel = Vec3::ZERO;
                    }
                    None => {}
                }
            }
            Bobber::Ground => {
                if !solid_at(&self.terrain.world, line.bobber - Vec3::Y * 0.1) {
                    line.state = Bobber::Flying;
                    line.vel = Vec3::ZERO;
                }
            }
        }

        // The reel.
        let fighting = line.fight.is_some();
        if notches != 0 {
            self.fishing.crank_to += notches as f32 * 0.9;
            let pitch = 0.9 + 0.06 * gear as f32;
            if notches > 0 {
                self.audio.play_pitched(Sound::ReelClick, None, 0.45, pitch);
            } else {
                self.audio.play_pitched(Sound::LineZip, None, 0.25, pitch);
            }
        }
        if line.auto_reel && !fighting {
            let n = 14.0 * dt;
            line.length -= n;
            self.fishing.crank_to += n * 1.5;
            if (self.time * 8.0).fract() < dt * 8.0 {
                self.audio.play_pitched(Sound::ReelClick, None, 0.35, 1.2);
            }
        }
        if !fighting && notches != 0 {
            line.length -= notches as f32 * PER_NOTCH[g];
            // Reeling scares a nibbling fish off now and then.
            if notches > 0 {
                if let Bite::Nibble(..) = line.bite {
                    if self.random() < 0.5 {
                        line.bite = Bite::Wait(3.0 + 6.0 * self.random());
                    }
                }
            }
        }
        line.length = line.length.clamp(0.0, MAX_LINE);
        // Pulled in along the water or the ground when the line is shorter than the way to it.
        line.dragged = (line.dragged - dt).max(0.0);
        if !fighting && line.state != Bobber::Flying {
            let to = tip - line.bobber;
            let d = to.length();
            if d > line.length + 0.05 {
                let step = ((d - line.length) * (1.0 - (-7.0 * dt).exp())).min(9.0 * dt);
                let flat = Vec3::new(to.x, 0.0, to.z);
                let mut q = line.bobber + flat.normalize_or_zero() * step.min(flat.length());
                let world = &self.terrain.world;
                if let Some(s) = water_surface(world, q + Vec3::Y * 0.3) {
                    q.y = s;
                    line.state = Bobber::Floating;
                } else if let Some(top) = ground_top(world, q + Vec3::Y * 0.3) {
                    q.y = top;
                    line.state = Bobber::Ground;
                } else if flat.length() < 0.3 || d > 0.0 {
                    // Over an edge or up to the rod: straight at it.
                    q = line.bobber + to / d * step;
                }
                line.bobber = q;
                line.dragged = 0.15;
                if line.state == Bobber::Floating && self.random() < dt * 12.0 {
                    let (sky, blk) = self.terrain.world.light_estimate(q + Vec3::Y);
                    self.particles.splash(q, 1, 0.1, sky, blk);
                }
            }
            let near = line.bobber.distance(tip);
            if near < 1.5 || line.length < 0.3 {
                // Reeled all the way in.
                self.audio.play_pitched(Sound::ReelClick, None, 0.5, 0.8);
                self.fishing.line = None;
                return;
            }
        }

        // Fish.
        if line.state == Bobber::Floating && !fighting {
            let open = open_water(&self.terrain.world, line.bobber);
            match line.bite {
                Bite::Wait(secs) if open => {
                    let secs = secs - dt;
                    line.bite = if secs <= 0.0 { Bite::Nibble(1 + (self.random() * 3.0) as u8, 0.3) } else { Bite::Wait(secs) };
                }
                Bite::Wait(_) => {}
                Bite::Nibble(left, secs) => {
                    let secs = secs - dt;
                    if secs > 0.0 {
                        line.bite = Bite::Nibble(left, secs);
                    } else if left > 0 {
                        line.bite = Bite::Nibble(left - 1, 0.7 + 1.3 * self.random());
                        line.dip_now = 0.06 + 0.04 * self.random();
                        let p = line.bobber;
                        self.audio.play(Sound::FishNibble, Some(p), 0.6);
                        let (sky, blk) = self.terrain.world.light_estimate(p + Vec3::Y);
                        self.particles.splash(p, 3, 0.05, sky, blk);
                    } else {
                        // The bite: pulled right under.
                        line.bite = Bite::Strike(STRIKE_TIME);
                        let p = line.bobber;
                        self.audio.play(Sound::FishBite, Some(p), 1.0);
                        let (sky, blk) = self.terrain.world.light_estimate(p + Vec3::Y);
                        self.particles.splash(p, 14, 0.5, sky, blk);
                        self.fishing.alarm = STRIKE_TIME;
                        self.toast(t("fish.bite").to_string(), rgba(255, 230, 120, 255));
                    }
                }
                Bite::Strike(secs) => {
                    if notches > 0 {
                        // Hooked!
                        let d = line.length.max(line.bobber.distance(tip));
                        let mut r = || self.rng.next();
                        line.fight = Some(Fight::new(d, &mut r));
                        line.bite = Bite::Wait(0.0);
                        self.fishing.alarm = 0.0;
                        self.fishing.toast = None;
                        let p = line.bobber;
                        self.audio.play(Sound::FishSplash, Some(p), 0.9);
                    } else if secs - dt <= 0.0 {
                        line.bite = Bite::Wait(self.bite_wait(line.bobber));
                        self.toast(t("fish.missed").to_string(), rgba(220, 220, 220, 255));
                    } else {
                        line.bite = Bite::Strike(secs - dt);
                    }
                }
            }
        }
        line.dip = match (line.bite, &line.fight) {
            (_, Some(_)) => 0.14,
            (Bite::Strike(_), _) => 0.3,
            _ => 0.0,
        };
        // A twitch jumps down at once and floats back up.
        line.dip_now += (line.dip - line.dip_now) * (1.0 - (-6.0 * dt).exp());

        // The fight.
        if let Some(mut fight) = line.fight.take() {
            let mut r = || self.rng.next();
            let (reeled, end) = fight.step(dt, notches, gear, &mut r);
            if reeled > 0.0 {
                self.fishing.crank_to += reeled * 0.5;
            }
            line.length = fight.dist;
            // It swims: across (turning about the angler) and at the length of the line.
            let origin = Vec3::new(tip.x, line.bobber.y, tip.z);
            let flat = Vec3::new(line.bobber.x - tip.x, 0.0, line.bobber.z - tip.z);
            let turn = fight.side * dt;
            let dir = Mat4::from_rotation_y(turn).transform_vector3(flat).normalize_or(Vec3::X);
            let drop = (tip.y - line.bobber.y).max(0.0);
            let across = (fight.dist * fight.dist - drop * drop).max(0.0).sqrt();
            let mut q = origin + dir * across;
            let world = &self.terrain.world;
            if let Some(s) = water_surface(world, q + Vec3::Y * 0.3) {
                q.y = s;
                line.state = Bobber::Floating;
            } else if let Some(top) = ground_top(world, q + Vec3::Y * 0.3) {
                // Dragged up the bank.
                q.y = top;
                line.state = Bobber::Ground;
            } else {
                q.y = line.bobber.y;
            }
            if solid_at(world, q + Vec3::Y * 0.1) {
                // Into a wall: it turns the other way.
                fight.side_to = -fight.side_to;
                fight.side = -fight.side * 0.5;
                q = Vec3::new(line.bobber.x, q.y, line.bobber.z);
            }
            line.bobber = q;
            // Thrashing at the surface, most in a surge and the more the bigger it is.
            let rate = if fight.surging() { 1.6 } else { 0.25 };
            if line.state == Bobber::Floating && self.random() < dt * rate {
                let big = (fight.weight / 8.0).min(1.0);
                self.audio.play(Sound::FishSplash, Some(q), 0.5 + 0.5 * big);
                let (sky, blk) = self.terrain.world.light_estimate(q + Vec3::Y);
                self.particles.splash(q, 6 + (big * 10.0) as usize, 0.3 + 0.5 * big, sky, blk);
            }
            match end {
                Some(FightEnd::Landed) => {
                    self.land_fish(&fight, line.bobber);
                    return;
                }
                Some(FightEnd::Snapped) => {
                    self.snap_line(line.bobber);
                    return;
                }
                Some(FightEnd::Escaped) => {
                    line.bite = Bite::Wait(self.bite_wait(line.bobber));
                    self.toast(t("fish.escaped").to_string(), rgba(220, 220, 220, 255));
                }
                None => line.fight = Some(fight),
            }
        }
        self.fishing.line = Some(line);
    }

    /// Wears the held rod; it may break.
    fn wear_rod(&mut self, n: u16) {
        if self.creative() {
            return;
        }
        let slot = self.hotbar_slot;
        if damage(&mut self.inventory.slots[slot], n) {
            self.audio.play(Sound::LineSnap, None, 0.8);
            self.say(t("fish.rod_broke"), rgba(255, 170, 120, 255));
        }
    }

    /// A fish brought in: it flies out of the water to the player and is theirs.
    fn land_fish(&mut self, fight: &Fight, at: Vec3) {
        let sp = &SPECIES[fight.species];
        let name = if crate::lang::is_hungarian() { sp.hu } else { sp.en };
        let msg = crate::lang::tf("fish.caught", &[&kilos(fight.weight), &name]);
        self.say(msg.clone(), rgba(120, 220, 255, 255));
        self.toast(msg, rgba(150, 230, 255, 255));
        self.audio.play(Sound::FishLand, Some(at), 1.0);
        let (sky, blk) = self.terrain.world.light_estimate(at + Vec3::Y);
        self.particles.splash(at, 18, 0.7, sky, blk);
        let count = (1.0 + fight.weight / 4.0).min(4.0) as u8;
        self.give(Stack::new(RAW_FISH, count));
        self.fishing.flying = Some((at, 0.0, fight.species, fight.weight));
        self.fishing.lift = Some(0.0);
        self.fishing.line = None;
        self.wear_rod(1);
    }

    /// The line parts: the fish is gone with the bobber.
    fn snap_line(&mut self, at: Vec3) {
        let tip = self.rod_tip();
        self.audio.play(Sound::LineSnap, Some(tip), 1.0);
        self.fishing.snapped = Some((0.0, at));
        self.fishing.line = None;
        self.toast(t("fish.snap").to_string(), rgba(255, 120, 100, 255));
        self.wear_rod(3);
    }

    /// What the held rod is doing, for its animation (and the others).
    pub(in crate::game) fn rod_anim(&self) -> Option<RodAnim> {
        if self.held() != FISHING_ROD {
            return None;
        }
        let f = &self.fishing;
        let line = f.line.as_ref();
        Some(RodAnim {
            charge: f.charge.map_or(0.0, |c| smooth(c / CHARGE_TIME)),
            cast: f.cast,
            out: line.is_some(),
            fight: f.fight_k,
            tension: f.tension,
            crank: f.crank,
            lift: f.lift,
            bobber: line.map(|l| f.bob_draw.unwrap_or(l.bobber) - Vec3::Y * l.dip_now),
        })
    }

    /// The rod creaking under a fish's pull (a looping sound).
    pub(in crate::game) fn fishing_sounds(&self) -> Vec<(u64, Sound, Vec3, f32)> {
        match &self.fishing.line {
            Some(Line { fight: Some(f), .. }) if f.tension > 0.35 => {
                vec![(0xf15_4000, Sound::RodCreak, self.rod_tip(), ((f.tension - 0.35) * 1.6).min(1.0))]
            }
            _ => Vec::new(),
        }
    }

    /// The lines from the rods' tips (this player's and the others'), the bobbers, a fish
    /// near the surface on the line or flying out, a line just parted.
    pub(in crate::game) fn build_fishing(&self, out: &mut Vec<Vertex>, cam: Vec3) {
        let world = &self.terrain.world;
        let light = |p: Vec3| {
            let (sky, blk) = world.light_estimate(p + Vec3::Y * 0.3);
            vertex_light(sky, blk)
        };
        let mut rods: Vec<(Vec3, RodAnim, bool)> = self.remote_rods().into_iter().map(|(t, a)| (t, a, false)).collect();
        let own = self.rod_anim().filter(|_| self.holding_rod());
        if let Some(a) = own {
            let tip = if self.camera.mode == 0 { self.fishing.tip_fp } else { self.fishing.tip_tp };
            if let Some(tip) = tip {
                rods.push((tip, a, true));
            }
        }
        for (tip, a, mine) in rods {
            match a.bobber {
                Some(b) => {
                    let len = tip.distance(b);
                    let slack = 1.0 - (a.tension / 0.35).clamp(0.0, 1.0);
                    let flying = a.cast.is_some() && a.fight == 0.0;
                    let sag = if flying { 0.01 * len } else { (0.035 * len * slack).min(2.0) };
                    emit_line(out, tip, b + Vec3::Y * 0.1, cam, sag, light((tip + b) * 0.5));
                    let toward = Vec3::new(tip.x - b.x, 0.0, tip.z - b.z).normalize_or(Vec3::X);
                    let tilt = if a.fight > 0.5 { 0.9 } else { 0.25 * a.tension };
                    // (bigger far off, to be seen out on the water)
                    let size = 1.0 + (b.distance(cam) / 30.0).min(1.0);
                    angler::emit_bobber(out, b, toward, tilt, size, light(b));
                }
                None => {
                    // Hanging from the tip on a short line (this player's swings, see
                    // `smooth_fishing`), along the line.
                    let hang = self.fishing.hang.map(|h| h.0).filter(|_| mine);
                    let sway = Vec3::new((self.time * 1.3).sin(), 0.0, (self.time * 1.1).cos()) * 0.02;
                    let knot = hang.unwrap_or(tip - Vec3::Y * 0.55 + sway);
                    let up = (tip - knot).normalize_or(Vec3::Y);
                    let b = knot - up * 0.1;
                    emit_line(out, tip, knot, cam, 0.0, light(tip));
                    let toward = Vec3::new(up.x, 0.0, up.z).normalize_or(Vec3::X);
                    angler::emit_bobber(out, b, toward, up.y.clamp(-1.0, 1.0).acos(), 1.0, light(tip));
                }
            }
        }
        // A fish close in on the line shows at the surface.
        if let (Some(line), true) = (&self.fishing.line, own.is_some()) {
            if let Some(f) = line.fight.as_ref().filter(|f| f.dist < 6.0) {
                let sp = &SPECIES[f.species];
                let size = (f.weight / 0.5).cbrt().clamp(0.7, 3.0);
                let tip = self.rod_tip();
                let away = Vec3::new(line.bobber.x - tip.x, 0.0, line.bobber.z - tip.z).normalize_or(Vec3::X);
                let wiggle = (self.time * (8.0 + 10.0 * f.pull)).sin() * 0.6;
                let bob = self.fishing.bob_draw.unwrap_or(line.bobber);
                let pos = bob - Vec3::Y * (0.12 * size) + away * 0.12 * size;
                angler::emit_fish(out, pos, away + Vec3::Y * 0.2, size, wiggle, sp.tint, light(pos));
            }
        }
        // A landed fish flying out to the player.
        if let Some((from, t, species, weight)) = self.fishing.flying {
            let k = t / 0.6;
            // (to the feet in front, not into the eyes)
            let ahead = look_dir(self.yaw, 0.0);
            let to = self.player.pos + Vec3::Y * 0.5 + ahead * 0.9;
            let pos = from.lerp(to, k) + Vec3::Y * (4.0 * k * (1.0 - k) * 1.5);
            let size = (weight / 0.5).cbrt().clamp(0.7, 3.0);
            let dir = (to - from).normalize_or(Vec3::X);
            let wiggle = (self.time * 22.0).sin() * 0.7;
            let size = size * (1.0 - 0.6 * smooth((k - 0.7) / 0.3));
            angler::emit_fish(out, pos, dir + Vec3::Y * (1.0 - 2.0 * k), size, wiggle, SPECIES[species].tint, light(pos));
        }
        // The loose end of a parted line, whipping back and falling.
        if let (Some((t, at)), Some(_)) = (self.fishing.snapped, own) {
            let tip = self.rod_tip();
            let k = t / 0.8;
            let reach = (1.0 - k) * 3.0 + 0.4;
            let dir = (at - tip).normalize_or(Vec3::X);
            let end = tip + dir * reach * (1.0 - 0.6 * k) - Vec3::Y * (1.5 * k * k + 0.3);
            emit_line(out, tip, end, cam, 0.3 + 0.8 * k, light(tip));
        }
    }

    /// Bottom right while the bobber is on the water: the reel's handle, its gears (a
    /// highlight sliding to the one it is in) and the line out; under the crosshair the cast's strength while drawing back, and the fight's
    /// tension bar. Everything moves smoothly (`smooth_fishing`).
    pub(in crate::game) fn draw_fishing_hud(&mut self) {
        if !self.holding_rod() {
            return;
        }
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        let f = &self.fishing;
        let line = f.line.as_ref();
        let fight = line.and_then(|l| l.fight.as_ref());
        let fade = smooth(f.hud_in);
        let (gear_at, tension, length) = (f.hud_gear, f.hud_tension.0.max(0.0), f.hud_len);
        let (fight_k, fresh, charge) = (smooth(f.hud_fight), f.hud_fresh, f.hud_charge);
        let (crank, flash, alarm) = (f.crank, f.gear_flash, f.alarm);
        let toast = f.toast.clone();
        let (over, slack) = fight.map_or((0.0, 0.0), |fi| (fi.over / OVER_TIME, fi.slack / SLACK_TIME));
        let time = self.time;
        let ui = &mut self.ui;
        let a = |c: Color, k: f32| with_alpha(c, k * fade);
        // A round dot (for arcs and caps).
        let dot = |ui: &mut crate::ui::Ui, p: Vec2, d: f32, c: Color| {
            ui.rect(p.x - d * 0.5, p.y - d * 0.5, d, d, c, d * 0.5);
        };

        let span = 1.1f32;
        // The reel's panel (only with the bobber on the water).
        if fade > 0.01 {
            // The panel.
            let (pw, ph) = (138.0 * s, 46.0 * s);
            let (x0, y0) = (w - pw - 10.0 * s, h - ph - 36.0 * s + (1.0 - fade) * 12.0 * s);
            ui.rect_full(x0, y0, pw, ph, a(rgba(0, 0, 0, 90), 1.0), a(rgba(0, 0, 0, 90), 1.0), 9.0 * s, 6.0 * s);
            ui.rect_full(x0, y0, pw, ph, a(rgba(26, 32, 42, 205), 1.0), a(rgba(12, 15, 20, 215), 1.0), 8.0 * s, 0.0);

            // The reel's handle, turning as it is wound.
            let c = Vec2::new(x0 + 26.0 * s, y0 + ph * 0.5);
            // The handle.
            let hub = 14.0 * s;
            ui.ring(c, hub, 1.5 * s, a(rgba(150, 158, 170, 220), 1.0));
            let arm = Vec2::new(crank.cos(), crank.sin());
            for i in 0..=8 {
                dot(ui, c + arm * hub * i as f32 / 8.0, 2.0 * s, a(rgba(220, 226, 234, 255), 1.0));
            }
            dot(ui, c + arm * hub, 5.0 * s, a(rgba(255, 210, 70, 255), 1.0));
            dot(ui, c, 4.0 * s, a(rgba(40, 44, 52, 255), 1.0));

            // The gears: the highlight slides to the one the reel is in (and swells a little when
            // it has just been changed).
            let gx = |g: f32| x0 + 60.0 * s + (g - 1.0) * 15.0 * s;
            let gy = y0 + 9.0 * s;
            let swell = 1.0 + 0.25 * (flash / 0.4).clamp(0.0, 1.0);
            let (gw, gh) = (12.0 * s * swell, 13.0 * s * swell);
            let hx = gx(gear_at);
            ui.rect(hx - gw * 0.5, gy + 4.0 * s - gh * 0.5, gw, gh, a(rgba(255, 206, 60, 255), 1.0), 3.0 * s);
            for g in 1..=ROD_GEARS {
                let near = (1.0 - (gear_at - g as f32).abs()).clamp(0.0, 1.0);
                let color = crate::ui::lerp_color(rgba(150, 158, 170, 255), rgba(40, 30, 6, 255), near);
                ui.text_centered(&g.to_string(), gx(g as f32) + 0.5 * s, gy, s, a(color, 1.0), false);
            }

            // The line out.
            let big = (s * 1.5).round().max(1.0);
            let dist = format!("{length:.1} m");
            let tw = ui.text_width(&dist, big);
            ui.text(&dist, x0 + pw - 10.0 * s - tw, y0 + 25.0 * s, big, a(WHITE, 1.0), true);
        }

        // Under the crosshair.
        let center = Vec2::new((w * 0.5).round(), (h * 0.5).round());
        let (bw, bh) = (120.0 * s, 8.0 * s);
        let (bx, by) = (center.x - bw * 0.5, center.y + 24.0 * s);
        // A smooth bar: thin slices blending from one colour to the next, rounded ends.
        let bar = |ui: &mut crate::ui::Ui, x: f32, y: f32, len: f32, hgt: f32, alpha: f32, color: &dyn Fn(f32) -> Color| {
            let n = ((len / s) as usize).max(2);
            let wd = len / n as f32;
            for i in 0..n {
                let k = (i as f32 + 0.5) / n as f32;
                let x1 = x + wd * i as f32;
                let rad = if i == 0 || i + 1 == n { hgt * 0.5 } else { 0.0 };
                let extra = if i + 1 < n { 0.6 } else { 0.0 };
                ui.rect(x1, y, wd + extra, hgt, with_alpha(color(k), alpha), rad.min(wd * 0.5));
            }
        };
        let shade = |k: f32| rgba(0, 0, 0, (140.0 * k) as u8);
        // Drawing back: the cast's strength.
        if charge > 0.01 && fight_k < 0.5 {
            let k = (1.0 - fight_k * 2.0).clamp(0.0, 1.0) * smooth((charge / 0.08).min(1.0));
            ui.rect_full(bx - 2.0 * s, by - 2.0 * s, bw + 4.0 * s, bh + 4.0 * s, shade(k), shade(k), (bh + 4.0 * s) * 0.5, 2.0 * s);
            let len = (bw * charge).max(bh);
            bar(ui, bx, by, len, bh, k, &|t| crate::ui::lerp_color(rgba(255, 236, 110, 255), rgba(255, 120, 40, 255), t * charge));
        }
        // The fight: the line's tension, to be kept in the green middle.
        if fight_k > 0.01 {
            let k = fight_k;
            let yb = by + (1.0 - k) * 8.0 * s;
            let x_of = |v: f32| bx + bw * (v / span).clamp(0.0, 1.0);
            ui.rect_full(bx - 3.0 * s, yb - 3.0 * s, bw + 6.0 * s, bh + 6.0 * s, shade(k), shade(k), (bh + 6.0 * s) * 0.5, 3.0 * s);
            bar(ui, bx, yb, bw, bh, 0.85 * k, &|t| tension_color(t * span));
            // The good part: a soft light frame round it.
            let (g0, g1) = (x_of(GOOD.0), x_of(GOOD.1));
            let lit = with_alpha(WHITE, 0.12 * k);
            ui.rect_full(g0 - 1.5 * s, yb - 2.0 * s, g1 - g0 + 3.0 * s, bh + 4.0 * s, lit, lit, 3.0 * s, 1.5 * s);
            // Warnings: the ends glow, faster and brighter as time runs out there.
            let glow = |v: f32| 0.5 + 0.5 * (time * (5.0 + 9.0 * v)).sin();
            if over > 0.0 {
                let x1 = x_of(TIGHT);
                let c = with_alpha(rgba(255, 235, 220, 255), k * over * glow(over));
                ui.rect_full(x1, yb, bx + bw - x1, bh, c, c, bh * 0.5, 2.0 * s);
            }
            if slack > 0.0 {
                let x1 = x_of(SLACK);
                let c = with_alpha(rgba(220, 235, 255, 255), k * slack * glow(slack));
                ui.rect_full(bx, yb, x1 - bx, bh, c, c, bh * 0.5, 2.0 * s);
            }
            // The marker: a pill riding on the bar.
            let mx = x_of(tension);
            let under = with_alpha(rgba(0, 0, 0, 255), 0.45 * k);
            ui.rect_full(mx - 4.0 * s, yb - 5.0 * s, 8.0 * s, bh + 10.0 * s, under, under, 4.0 * s, 2.0 * s);
            ui.rect(mx - 2.5 * s, yb - 4.0 * s, 5.0 * s, bh + 8.0 * s, with_alpha(WHITE, k), 2.5 * s);
            // How much fight the fish has left.
            let sy = yb + bh + 6.0 * s;
            ui.rect(bx, sy, bw, 3.0 * s, with_alpha(rgba(0, 0, 0, 255), 0.5 * k), 1.5 * s);
            if fresh > 0.005 {
                let (c0, c1) = (with_alpha(rgba(255, 176, 80, 255), k), with_alpha(rgba(235, 120, 50, 255), k));
                ui.rect_full(bx, sy, (bw * fresh).max(3.0 * s), 3.0 * s, c0, c1, 1.5 * s, 0.0);
            }
        }
        // A bite: set the hook! (a pulsing "!")
        if alarm > 0.0 {
            let pulse = 0.5 + 0.5 * (time * 10.0).sin();
            let k = (alarm / 0.3).min(1.0);
            let size = (2.0 * s + (pulse * s).round()).max(1.0);
            ui.text_centered("!", center.x + 0.5 * s, center.y - 28.0 * s - pulse * 2.0 * s, size, with_alpha(rgba(255, 220, 60, 255), k), true);
        }
        if let Some((text, left, color)) = toast {
            let k = smooth((left / 0.5).min(1.0));
            ui.text_centered(&text, center.x, h - 78.0 * s + (1.0 - k) * 6.0 * s, s, with_alpha(color, k), true);
        }
    }
}

/// The colour of a line tension `v` (0 slack .. 1.1 parting): calm blue, green in the good
/// middle, amber getting tight, red past the limit, blended smoothly.
fn tension_color(v: f32) -> Color {
    let keys = [
        (0.0, rgba(70, 120, 215, 255)),
        (SLACK, rgba(80, 170, 220, 255)),
        ((GOOD.0 + GOOD.1) * 0.5, rgba(80, 215, 110, 255)),
        (TIGHT, rgba(245, 190, 60, 255)),
        (1.1, rgba(235, 55, 40, 255)),
    ];
    for w in keys.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if v <= b {
            return crate::ui::lerp_color(ca, cb, smooth(((v - a) / (b - a)).clamp(0.0, 1.0)));
        }
    }
    keys[keys.len() - 1].1
}

/// See `angler::emit_line`.
fn emit_line(out: &mut Vec<Vertex>, from: Vec3, to: Vec3, cam: Vec3, sag: f32, light: [u8; 4]) {
    angler::emit_line(out, from, to, cam, sag, light);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A player who watches the tension bar and turns the wheel to keep it in the middle
    /// (in when it goes slack, out when it tightens), in a higher gear when it is far off.
    fn play(mut fight: Fight, seed: u32, lazy: bool) -> (Option<FightEnd>, f32) {
        let mut rng = crate::util::Rng::new(seed);
        let mut r = move || rng.next();
        let dt = 1.0 / 60.0;
        let mut wheel = 0.0f32;
        let mut seen = fight.tension;
        for i in 0..(240.0 / dt) as usize {
            // (seen a moment late, as a person would)
            if i % 12 == 0 {
                seen = fight.tension;
            }
            let off = (0.5 - seen).abs();
            let gear = if off > 0.25 { 5 } else if off > 0.1 { 3 } else { 1 };
            let want = if lazy { 0.0 } else { ((0.5 - seen) * 30.0).clamp(-9.0, 9.0) };
            wheel += want * dt;
            let n = wheel.trunc() as i32;
            wheel -= n as f32;
            let (_, end) = fight.step(dt, n, gear, &mut r);
            if end.is_some() {
                return (end, i as f32 * dt);
            }
        }
        (None, 240.0)
    }

    #[test]
    fn every_fish_can_be_landed_by_keeping_the_line_in_the_middle() {
        for species in 0..SPECIES.len() {
            let (lo, hi) = SPECIES[species].kg;
            for weight in [lo, (lo + hi) * 0.5, hi] {
                let mut landed = 0;
                let mut slowest = 0.0f32;
                for seed in 1..6 {
                    let (end, secs) = play(Fight::of(species, weight, 25.0), seed * 77, false);
                    if end == Some(FightEnd::Landed) {
                        landed += 1;
                        slowest = slowest.max(secs);
                    }
                }
                assert!(landed >= 4, "{} {weight} kg: landed {landed} of 5", SPECIES[species].en);
                assert!(slowest < 150.0, "{} {weight} kg: {slowest} s", SPECIES[species].en);
            }
        }
    }

    #[test]
    fn a_fish_left_alone_gets_away_and_one_cranked_hard_snaps_the_line() {
        let (end, _) = play(Fight::of(3, 4.0, 20.0), 5, true);
        assert!(matches!(end, Some(FightEnd::Escaped) | Some(FightEnd::Snapped)), "{end:?}");
        let mut f = Fight::of(3, 4.0, 20.0);
        let mut r = || 0.5;
        let mut end = None;
        for _ in 0..600 {
            end = end.or(f.step(1.0 / 60.0, 1, 1, &mut r).1);
        }
        assert_eq!(end, Some(FightEnd::Snapped));
    }
}
