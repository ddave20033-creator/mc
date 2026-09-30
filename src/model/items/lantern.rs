//! Lanterns: the block (standing, or hanging from the block above) and the one held in the
//! hand, which hangs from a short chain and swings like a pendulum as the holder moves and
//! looks around.
//!
//! The model follows Minecraft's lantern (a 6x7x6 body, a 4x2x4 cap and a handle loop) with
//! the texture's own layout; model space is in block pixels, x/z centered on the block, y up
//! from its bottom.

use crate::model::prim::{self, BoxUv, Paint, Sides};
use crate::world::mesh::Vertex;
use crate::world::textures::tex;
use glam::{Mat4, Quat, Vec2, Vec3};
use std::f32::consts::FRAC_PI_4;

#[derive(Clone, Copy, PartialEq)]
pub enum LanternKind {
    Standing,
    Hanging,
    /// Hanging from a chain of this many pixels above its handle loop.
    Held(f32),
}

/// How a held lantern looks: its size relative to the block and its chain in pixels.
#[derive(Clone, Copy)]
pub struct HeldStyle {
    pub scale: f32,
    pub chain: f32,
    /// Largest swing away from hanging straight down (radians), and how fast the swing
    /// dies down (per second).
    pub max_tilt: f32,
    pub damping: f32,
}

const HELD_MAX_TILT: f32 = 16.0 * DEG;
const HELD_DAMPING: f32 = 12.0;

/// On the player model (seen by others and in third person).
pub const ON_MODEL: HeldStyle = HeldStyle {
    scale: 0.75,
    chain: 1.5,
    max_tilt: HELD_MAX_TILT,
    damping: HELD_DAMPING,
};

/// Full-size lantern hanging from the hand in first-person view (with the body shown).
pub const FIRST_PERSON: HeldStyle = HeldStyle {
    scale: 1.0,
    chain: 1.5,
    max_tilt: HELD_MAX_TILT,
    damping: HELD_DAMPING,
};

/// Top of the handle loop of a hanging lantern.
const RING_TOP: f32 = 14.0;

impl HeldStyle {
    fn top(self) -> f32 {
        RING_TOP + self.chain
    }
}

/// One quad from its four corners (counter-clockwise seen from the front) with a texture
/// rectangle in 16ths of the layer; `both` also emits the back side.
#[allow(clippy::too_many_arguments)]
fn quad(
    out: &mut Vec<Vertex>,
    m: Mat4,
    c: [Vec3; 4],
    uv: [f32; 4],
    layer: u32,
    light: [u8; 4],
    fl: u8,
    face: u8,
    both: bool,
) {
    let paint = Paint { layer, light, face, tint: [255; 3], fl };
    let sides = if both { Sides::Both } else { Sides::Front };
    prim::quad(out, m, c, prim::rect_uvs(uv.map(|v| v / 16.0)), &paint, sides);
}

/// A box with one texture rectangle for its four sides and one for the top and bottom.
#[allow(clippy::too_many_arguments)]
fn cuboid(
    out: &mut Vec<Vertex>,
    m: Mat4,
    lo: Vec3,
    hi: Vec3,
    side: [f32; 4],
    ends: [f32; 4],
    light: [u8; 4],
    fl: u8,
) {
    let [side, ends] = [side, ends].map(|r| r.map(|v| v / 16.0));
    let uv = BoxUv::Rects([side, side, ends, ends, side, side]);
    prim::cuboid(out, m, lo, hi, uv, |face| Some(Paint { layer: tex::LANTERN, light, face: face as u8, tint: [255; 3], fl }));
}

/// Two planes crossed at 45 degrees around the vertical axis (handle loops, chains),
/// `w` pixels wide from `y0` to `y1`.
#[allow(clippy::too_many_arguments)]
fn crossed(
    out: &mut Vec<Vertex>,
    m: Mat4,
    w: f32,
    y0: f32,
    y1: f32,
    uv: [[f32; 4]; 2],
    layer: u32,
    light: [u8; 4],
    fl: u8,
) {
    for (k, angle) in [FRAC_PI_4, -FRAC_PI_4].into_iter().enumerate() {
        let d = Quat::from_rotation_y(angle) * Vec3::X * (w * 0.5);
        let c = [
            Vec3::new(-d.x, y0, -d.z),
            Vec3::new(d.x, y0, d.z),
            Vec3::new(d.x, y1, d.z),
            Vec3::new(-d.x, y1, -d.z),
        ];
        quad(out, m, c, uv[k], layer, light, fl, 6, true);
    }
}

/// Emits a lantern; `m` maps model pixels to the world.
pub fn emit_lantern(out: &mut Vec<Vertex>, m: Mat4, light: [u8; 4], fl: u8, kind: LanternKind) {
    let lift = if kind == LanternKind::Standing {
        0.0
    } else {
        1.0
    };
    // Body with the glowing window, then the cap.
    cuboid(
        out,
        m,
        Vec3::new(-3.0, lift, -3.0),
        Vec3::new(3.0, 7.0 + lift, 3.0),
        [0.0, 2.0, 6.0, 9.0],
        [0.0, 9.0, 6.0, 15.0],
        light,
        fl,
    );
    cuboid(
        out,
        m,
        Vec3::new(-2.0, 7.0 + lift, -2.0),
        Vec3::new(2.0, 9.0 + lift, 2.0),
        [1.0, 0.0, 5.0, 2.0],
        [1.0, 10.0, 5.0, 14.0],
        light,
        fl,
    );
    match kind {
        LanternKind::Standing => {
            let handle = [11.0, 10.0, 14.0, 12.0];
            crossed(out, m, 3.0, 9.0, 11.0, [handle; 2], tex::LANTERN, light, fl);
        }
        LanternKind::Hanging | LanternKind::Held(_) => {
            let ring = [11.0, 1.0, 14.0, 5.0];
            crossed(
                out,
                m,
                3.0,
                10.0,
                RING_TOP,
                [ring; 2],
                tex::LANTERN,
                light,
                fl,
            );
            let top = match kind {
                LanternKind::Held(chain) => RING_TOP + chain,
                _ => 16.0,
            };
            let len = top - RING_TOP;
            let chain = [[0.0, 0.0, 3.0, len], [3.0, 0.0, 6.0, len]];
            crossed(out, m, 3.0, RING_TOP, top, chain, tex::CHAIN, light, fl);
        }
    }
}

/// A held lantern hanging from `pivot` (the hand) toward `dir` (unit, pointing down along
/// the chain), turned to `yaw` (like the player's: forward is (cos, 0, sin)).
#[allow(clippy::too_many_arguments)]
pub fn emit_held_lantern(
    out: &mut Vec<Vertex>,
    style: HeldStyle,
    pivot: Vec3,
    dir: Vec3,
    yaw: f32,
    light: [u8; 4],
    fl: u8,
) {
    let tilt = Quat::from_rotation_arc(Vec3::NEG_Y, dir.try_normalize().unwrap_or(Vec3::NEG_Y));
    let m = Mat4::from_translation(pivot)
        * Mat4::from_quat(tilt)
        * Mat4::from_rotation_y(-yaw)
        * Mat4::from_scale(Vec3::splat(style.scale / 16.0))
        * Mat4::from_translation(Vec3::new(0.0, -style.top(), 0.0));
    emit_lantern(out, m, light, fl, LanternKind::Held(style.chain));
}

const DEG: f32 = std::f32::consts::PI / 180.0;

/// A restrained, critically damped swing for held lanterns. Camera turns can move the
/// first-person hand much faster than walking does, so hand velocity is filtered.
#[derive(Default)]
pub struct SmoothSwing {
    pivot: Option<Vec3>,
    hand_vel: Vec2,
    lean: Vec2,
    lean_vel: Vec2,
}

impl SmoothSwing {
    pub fn update(&mut self, style: HeldStyle, pivot: Vec3, dt: f32) -> Vec3 {
        let Some(previous) = self.pivot else {
            self.pivot = Some(pivot);
            return Vec3::NEG_Y;
        };
        if !dt.is_finite() || dt <= 0.0 {
            return Vec3::new(self.lean.x, -1.0, self.lean.y).normalize();
        }
        if pivot.distance(previous) > 0.75 {
            *self = Self {
                pivot: Some(pivot),
                ..Self::default()
            };
            return Vec3::NEG_Y;
        }
        self.pivot = Some(pivot);
        let dt = dt.min(0.05);
        let hand_step = pivot - previous;
        let speed = Vec2::new(hand_step.x, hand_step.z) / dt;
        let speed = speed.clamp_length_max(6.0);
        self.hand_vel += (speed - self.hand_vel) * (crate::util::damp(12.0, dt));

        let max_lean = style.max_tilt.tan();
        let target = (-self.hand_vel * 0.032).clamp_length_max(max_lean);
        // Exact critically damped spring step for a constant target during this frame.
        let omega = style.damping.max(1.0);
        let decay = (-omega * dt).exp();
        let error = self.lean - target;
        let momentum = self.lean_vel + error * omega;
        let next = target + (error + momentum * dt) * decay;
        self.lean_vel = (self.lean_vel - momentum * omega * dt) * decay;
        self.lean = next.clamp_length_max(max_lean);
        if next.length() > max_lean {
            self.lean_vel = Vec2::ZERO;
        }
        Vec3::new(self.lean.x, -1.0, self.lean.y).normalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_swing_is_bounded_and_settles_without_oscillating() {
        for fps in [30, 60, 144] {
            for style in [FIRST_PERSON, ON_MODEL] {
                let dt = 1.0 / fps as f32;
                let mut swing = SmoothSwing::default();
                swing.update(style, Vec3::ZERO, dt);
                let mut dir = Vec3::NEG_Y;
                for frame in 1..=(fps / 4) {
                    dir = swing.update(style, Vec3::X * (frame as f32 * dt * 4.8), dt);
                }
                let tilt = dir.angle_between(Vec3::NEG_Y);
                assert!(
                    tilt > 3.0 * DEG && tilt < style.max_tilt,
                    "{fps} FPS: {tilt}"
                );
                let stop = Vec3::X * ((fps / 4) as f32 * dt * 4.8);
                for _ in 0..fps {
                    dir = swing.update(style, stop, dt);
                }
                assert!(
                    dir.angle_between(Vec3::NEG_Y) < 0.5 * DEG,
                    "{fps} FPS: {dir}"
                );
                assert_eq!(swing.update(style, stop + Vec3::X * 3.0, dt), Vec3::NEG_Y);
            }
        }
    }

    #[test]
    fn models_have_geometry() {
        for kind in [
            LanternKind::Standing,
            LanternKind::Hanging,
            LanternKind::Held(5.0),
        ] {
            let mut out = Vec::new();
            emit_lantern(&mut out, Mat4::IDENTITY, [255; 4], 0, kind);
            assert!(out.len() > 60);
            let top = out.iter().map(|v| v.pos[1]).fold(f32::MIN, f32::max);
            let want = match kind {
                LanternKind::Standing => 11.0,
                LanternKind::Hanging => 16.0,
                LanternKind::Held(chain) => RING_TOP + chain,
            };
            assert!((top - want).abs() < 1e-4, "{top}");
        }
    }
}
