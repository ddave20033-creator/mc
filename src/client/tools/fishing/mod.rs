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

use crate::audio::Sound;
use crate::client::{Game, Screen};
use crate::entity::player::look_dir;
use crate::item::{FISHING_ROD, RAW_FISH, ROD_GEARS, Stack, rod_gear, set_rod_gear};
use crate::item::inventory::damage;
use crate::app::keys::Bind;
use crate::app::lang::t;
use crate::model::items::angler;
use crate::model::items::angler::{CAST_TIME, LIFT_TIME, RodAnim, WHIP_FORWARD};
use crate::ui::{Color, WHITE, rgba, with_alpha};
use crate::util::vertex_light;
use crate::world::{HEIGHT, World, fluid_level, is_solid, is_water, opens_on_use};
use crate::world::mesh::{Vertex, fluid_height};
use glam::{IVec3, Mat4, Vec2, Vec3};

mod draw;
mod reel;
#[cfg(test)]
mod tests;

/// Seconds of drawing back for the farthest cast.
const CHARGE_TIME: f32 = 1.2;
/// The most line on the reel (m).
pub(in crate::client) const MAX_LINE: f32 = 64.0;
/// How fast the bobber leaves the rod: the weakest and the strongest cast (about 4 and 35 m).
const CAST_SPEED: (f32, f32) = (7.5, 22.0);
const GRAVITY: f32 = 14.0;
/// Seconds after the bite to set the hook.
const STRIKE_TIME: f32 = 1.7;
/// A fish this close (m of line) is landed.
const LAND_DIST: f32 = 2.2;
/// Line (m) reeled in or let out per notch of the wheel in each gear.
pub(in crate::client) const PER_NOTCH: [f32; ROD_GEARS as usize] = [0.3, 0.45, 0.65, 0.9, 1.2];
/// How much a pulling fish stops the reel in each gear (a strong low gear keeps winning line
/// against it, a fast high one only while it rests).
const STALL: [f32; ROD_GEARS as usize] = [0.0, 0.3, 0.6, 0.85, 1.1];
/// How much a notch of the wheel tightens the line (in) and eases it (out), by gear: fine
/// in the low gears, coarse in the high ones.
const TIGHTEN: [f32; ROD_GEARS as usize] = [0.03, 0.045, 0.06, 0.08, 0.1];
const EASE: [f32; ROD_GEARS as usize] = [0.035, 0.05, 0.065, 0.085, 0.105];
/// The tension's zones: the middle (the fish tires fast), too tight (it snaps if it lasts),
/// too slack (the fish gets away if it lasts); the tension the line snaps at at once.
pub(in crate::client) const GOOD: (f32, f32) = (0.32, 0.7);
pub(in crate::client) const TIGHT: f32 = 0.85;
pub(in crate::client) const SLACK: f32 = 0.2;
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
pub(in crate::client) struct Fight {
    species: usize,
    weight: f32,
    stamina: f32,
    max_stamina: f32,
    /// How strong its surges are.
    power: f32,
    /// How hard it pulls now (about 0.1 resting .. 2 surging).
    pull: f32,
    /// How taut the line is (see `GOOD`, `TIGHT`, `SLACK`, `BREAK`).
    tension: f32,
    /// Line out to it (m).
    dist: f32,
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
    over: f32,
    slack: f32,
    /// Which way it swims across (radians per second about the angler) and wants to.
    side: f32,
    side_to: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::client) enum FightEnd {
    Landed,
    Snapped,
    Escaped,
}

impl Fight {
    /// A fish of a kind chosen by how common they are (`r`: random numbers 0..1), `dist` out.
    pub(in crate::client) fn new(dist: f32, r: &mut dyn FnMut() -> f32) -> Self {
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

    fn of(species: usize, weight: f32, dist: f32) -> Self {
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
    fn fresh(&self) -> f32 {
        (self.stamina / self.max_stamina).clamp(0.0, 1.0)
    }

    fn surging(&self) -> bool {
        self.drift > 0.2
    }

    /// `dt` seconds of the fight, the wheel turned `notches` (+ in, - out) in `gear`.
    /// Returns the line reeled in (m, - let out) and how it ended, if it did.
    fn step(&mut self, dt: f32, notches: i32, gear: u8, r: &mut dyn FnMut() -> f32) -> (f32, Option<FightEnd>) {
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
        self.drift += (self.drift_to - self.drift) * (crate::util::damp(self.snap, dt));
        self.pull = (0.5 + self.drift).clamp(0.05, 2.0);
        self.side += (self.side_to - self.side) * (crate::util::damp(1.5, dt));
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
        let now = self.reel_left * (crate::util::damp(12.0, dt));
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
pub(in crate::client) enum Bobber {
    Flying,
    Floating,
    Ground,
}

/// Waiting for a fish (seconds), nibbles to come (how many more, seconds to the next), the
/// bite (seconds left to set the hook).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(in crate::client) enum Bite {
    Wait(f32),
    Nibble(u8, f32),
    Strike(f32),
}

/// The line out and what is on it.
pub(in crate::client) struct Line {
    bobber: Vec3,
    vel: Vec3,
    state: Bobber,
    /// Line off the reel (m).
    length: f32,
    bite: Bite,
    fight: Option<Fight>,
    /// Reeling it all in (a right click with the line out).
    auto_reel: bool,
    /// How far the bobber is pulled under (m), and where it is drawn.
    dip: f32,
    dip_now: f32,
    /// The bobber being dragged across (for its tilt and the wake).
    dragged: f32,
}

#[derive(Default)]
pub(in crate::client) struct Fishing {
    /// Drawing the rod back to cast: seconds the button has been held.
    charge: Option<f32>,
    /// The cast's whip playing (seconds), how hard it was, whether the bobber has left.
    cast: Option<f32>,
    cast_power: f32,
    /// A landed fish: the rod swung up (seconds).
    lift: Option<f32>,
    pub(in crate::client) line: Option<Line>,
    /// The reel's handle (radians), and the wheel's notches not yet used.
    crank: f32,
    scroll: f32,
    notches: i32,
    /// Where the rod's tip was drawn last frame: by the first-person hand and on the model.
    pub(in crate::client) tip_fp: Option<Vec3>,
    pub(in crate::client) tip_tp: Option<Vec3>,
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
    alarm: f32,
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
    if crate::app::lang::is_hungarian() {
        s.replace('.', ",")
    } else {
        s
    }
}
