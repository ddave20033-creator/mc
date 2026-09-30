//! The bucket as a 3D model (empty, with water or with lava) and the liquid sloshing in it:
//! a tapered galvanized pail with a rolled rim, two ears and a wire handle, the liquid's
//! surface a plane that stays level with the world, leaning against how the bucket is pushed
//! around and swinging back, never far enough to spill.

use crate::item::{ItemId, BUCKET, LAVA_BUCKET, WATER_BUCKET};
use crate::model::prim::{tri_at, Paint, Sides};
use crate::world::mesh::{flags, Vertex};
use crate::world::textures::tex;
use glam::{Mat4, Vec2, Vec3};
use std::f32::consts::TAU;

/// What is in a bucket.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fill {
    Empty,
    Water,
    Lava,
}

impl Fill {
    pub fn of(item: ItemId) -> Option<Fill> {
        match item {
            BUCKET => Some(Fill::Empty),
            WATER_BUCKET => Some(Fill::Water),
            LAVA_BUCKET => Some(Fill::Lava),
            _ => None,
        }
    }
}

pub fn is_bucket(item: ItemId) -> bool {
    Fill::of(item).is_some()
}

// The pail in the unit (centered on the origin): bottom, rim, and their outer radii.
const BOTTOM: f32 = -0.34;
const RIM: f32 = 0.3;
const R_BOTTOM: f32 = 0.25;
const R_TOP: f32 = 0.34;
const WALL: f32 = 0.022;
const FLOOR: f32 = 0.035;
const SIDES: usize = 20;
/// Height of the liquid's surface at rest (the middle of it).
const LEVEL: f32 = 0.16;
/// How far the surface may rise at the wall: a little under the rim.
const FREEBOARD: f32 = 0.035;

fn radius(y: f32) -> f32 {
    R_BOTTOM + (R_TOP - R_BOTTOM) * (y - BOTTOM) / (RIM - BOTTOM)
}

fn inner_radius(y: f32) -> f32 {
    radius(y) - WALL
}

/// The steepest the surface may lean (rise over run, bucket space) and bounce (its middle's
/// height over the rest level) without going over the rim.
fn max_slope(bounce: f32) -> f32 {
    let r = inner_radius(RIM);
    ((RIM - FREEBOARD - LEVEL - bounce.abs()) / r).max(0.0)
}

/// How a liquid moves: its sloshing (the surface's lean) and its bouncing (the middle going
/// up and down while the edge goes the other way).
struct Liquid {
    /// Swings a second, and damping ratio (lower: rocks longer).
    freq: f32,
    zeta: f32,
    bounce_freq: f32,
    bounce_zeta: f32,
    /// How much the surface leans per block/s² the bucket is pushed with.
    lean: f32,
    bounce: f32,
}

const WATER: Liquid = Liquid { freq: 1.7, zeta: 0.11, bounce_freq: 3.4, bounce_zeta: 0.16, lean: 0.03, bounce: 0.0022 };
/// Lava is thick: it leans slower and less, hardly swings back and settles soon.
const LAVA: Liquid = Liquid { freq: 0.8, zeta: 0.75, bounce_freq: 1.6, bounce_zeta: 0.8, lean: 0.018, bounce: 0.0009 };

/// The liquid in one bucket moving about as the bucket is carried: its lean (world height
/// per block across, x and z) and its bounce, from how the bucket's middle moves.
#[derive(Clone, Copy, Default, Debug)]
pub struct Slosh {
    pos: Option<Vec3>,
    vel: Vec3,
    acc: Vec3,
    pub tilt: Vec2,
    tilt_v: Vec2,
    pub bounce: f32,
    bounce_v: f32,
    /// The bucket itself swinging on its handle a little (like the surface's lean, world).
    pub swing: Vec2,
    swing_v: Vec2,
}

impl Slosh {
    /// Steps it: the bucket's middle is at `pos` (world) now. A jump (a teleport, a new
    /// bucket) starts it over at rest.
    pub fn update(&mut self, fill: Fill, pos: Vec3, dt: f32) {
        let liquid = if fill == Fill::Lava { &LAVA } else { &WATER };
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let Some(prev) = self.pos.filter(|p| p.distance(pos) < 1.5) else {
            *self = Slosh { pos: Some(pos), ..Default::default() };
            return;
        };
        self.pos = Some(pos);
        let dt = dt.min(0.05);
        // The velocity and the push (acceleration) smoothed a little: frame to frame they are
        // noisy.
        let vel = ((pos - prev) / dt).clamp_length_max(30.0);
        let old = self.vel;
        self.vel += (vel - self.vel) * (crate::util::damp(25.0, dt));
        let acc = ((self.vel - old) / dt).clamp_length_max(120.0);
        self.acc += (acc - self.acc) * (crate::util::damp(18.0, dt));

        // The surface lies across the pull it feels (gravity less the push): it leans back
        // from where the bucket is pushed, like a pendulum, and swings about that.
        let target = -Vec2::new(self.acc.x, self.acc.z) * liquid.lean;
        let bounce_target = -self.acc.y * liquid.bounce;
        let n = (dt / (1.0 / 240.0)).ceil().clamp(1.0, 32.0) as usize;
        let h = dt / n as f32;
        let (w, wb) = (TAU * liquid.freq, TAU * liquid.bounce_freq);
        let ws = TAU * 1.3;
        let swing_target = -Vec2::new(self.acc.x, self.acc.z) * 0.012;
        for _ in 0..n {
            let a = (target - self.tilt) * (w * w) - self.tilt_v * (2.0 * liquid.zeta * w);
            self.tilt_v += a * h;
            self.tilt += self.tilt_v * h;
            let a = (bounce_target - self.bounce) * (wb * wb) - self.bounce_v * (2.0 * liquid.bounce_zeta * wb);
            self.bounce_v += a * h;
            self.bounce += self.bounce_v * h;
            let a = (swing_target - self.swing) * (ws * ws) - self.swing_v * (2.0 * 0.35 * ws);
            self.swing_v += a * h;
            self.swing += self.swing_v * h;
        }
        // Past the rim it would spill: it stops there (and loses the swing that took it).
        let top = max_slope(0.0) * 0.95;
        if self.tilt.length() > top {
            self.tilt = self.tilt.clamp_length_max(top);
            let out = self.tilt.normalize_or_zero();
            let v = self.tilt_v.dot(out);
            if v > 0.0 {
                self.tilt_v -= out * v * 1.6;
            }
        }
        let b = (RIM - FREEBOARD - LEVEL) * 0.35;
        if self.bounce.abs() > b {
            self.bounce = self.bounce.clamp(-b, b);
            self.bounce_v *= -0.3;
        }
        self.swing = self.swing.clamp_length_max(0.2);
    }

    /// The bucket's tilt from swinging on its handle: a rotation to put before the bucket.
    /// (Hanging from its handle, its bottom trails: it leans toward where it is pushed.)
    pub fn swing_matrix(&self) -> Mat4 {
        Mat4::from_rotation_x(-self.swing.y) * Mat4::from_rotation_z(self.swing.x)
    }
}

/// The liquid's surface for a bucket drawn with `m`: level with the world (leaning by `tilt`,
/// world height per block) where it is drawn in the world, level with the bucket on an icon.
pub struct Surface {
    pub tilt: Vec2,
    pub bounce: f32,
    /// World "up" is the bucket's own up (an icon, a picture).
    pub own_up: bool,
}

impl Surface {
    pub fn still(own_up: bool) -> Surface {
        Surface { tilt: Vec2::ZERO, bounce: 0.0, own_up }
    }
}

/// Where the middle of the handle's grip is, up (bucket space): the bucket hangs from there.
pub fn handle_top() -> f32 {
    RIM - 0.07 + SPAN * 1.05 * (GRIP * std::f32::consts::PI).sin()
}

/// Where along the handle (0..1, ear to ear) the wooden grip starts.
const GRIP: f32 = 0.34;

/// Half the handle's width: its ends well out past the rim's bead.
const SPAN: f32 = R_TOP + 0.05;

/// A bucket item: `fill` in it, `surface` how the liquid lies, `handle` how far the handle
/// is let down toward its back (-Z; 0 up, 1 lying on the rim).
#[allow(clippy::too_many_arguments)]
pub fn emit(out: &mut Vec<Vertex>, m: Mat4, fill: Fill, surface: &Surface, handle: f32, light: [u8; 4], fl: u8) {
    let flip = m.determinant() < 0.0;
    let metal = tex::BUCKET_METAL;
    let mut q = Quads { out, m, flip, light, fl };
    let steel = [255u8, 255, 255];
    // Lava lights up the inside of the bucket.
    let lava = fill == Fill::Lava;
    let inside = if lava { [255u8, 168, 112] } else { [168u8, 170, 174] };
    let floor_in = [140u8, 142, 146];
    let ang = |i: usize| i as f32 / SIDES as f32 * TAU;
    let at = |a: f32, r: f32, y: f32| Vec3::new(a.cos() * r, y, a.sin() * r);
    for i in 0..SIDES {
        let (a0, a1) = (ang(i), ang(i + 1));
        let (u0, u1) = (i as f32 / SIDES as f32 * 3.0, (i + 1) as f32 / SIDES as f32 * 3.0);
        let mid = (a0 + a1) * 0.5;
        let outward = Vec3::new(mid.cos(), 0.0, mid.sin());
        // Outside wall (the texture's rows from the rim down).
        q.quad(
            [at(a0, R_BOTTOM, BOTTOM), at(a1, R_BOTTOM, BOTTOM), at(a1, R_TOP, RIM), at(a0, R_TOP, RIM)],
            [[u0, 1.0], [u1, 1.0], [u1, 0.08], [u0, 0.08]],
            outward,
            metal,
            steel,
        );
        // The rolled rim: a band standing out a little around the top.
        let (b0, b1) = (RIM - 0.04, RIM + 0.012);
        let rb = |y: f32| radius(y.min(RIM)) + 0.014;
        q.quad(
            [at(a0, rb(b0), b0), at(a1, rb(b0), b0), at(a1, rb(b1), b1), at(a0, rb(b1), b1)],
            [[u0, 0.08], [u1, 0.08], [u1, 0.0], [u0, 0.0]],
            outward,
            metal,
            [236, 238, 242],
        );
        q.quad(
            [at(a0, radius(b0), b0), at(a1, radius(b0), b0), at(a1, rb(b0), b0), at(a0, rb(b0), b0)],
            [[u0, 0.08], [u1, 0.08], [u1, 0.1], [u0, 0.1]],
            Vec3::NEG_Y,
            metal,
            [200, 202, 206],
        );
        // Its top, from the bead in to the inside wall.
        q.quad(
            [at(a0, rb(b1), b1), at(a1, rb(b1), b1), at(a1, inner_radius(RIM), b1), at(a0, inner_radius(RIM), b1)],
            [[u0, 0.0], [u1, 0.0], [u1, 0.03], [u0, 0.03]],
            Vec3::Y,
            metal,
            [246, 247, 250],
        );
        // Inside wall, down to the floor.
        let yf = BOTTOM + FLOOR;
        let lit = q.light;
        if lava {
            q.light = [255, 255, 255, lit[3]];
        }
        q.quad(
            [at(a0, inner_radius(yf), yf), at(a1, inner_radius(yf), yf), at(a1, inner_radius(RIM), b1), at(a0, inner_radius(RIM), b1)],
            [[u0, 1.0], [u1, 1.0], [u1, 0.0], [u0, 0.0]],
            -outward,
            metal,
            inside,
        );
        q.light = lit;
        // The floor inside and the bottom outside (fans from the middle).
        let c = Vec3::new(0.0, yf, 0.0);
        let uv = |p: Vec3| [0.5 + p.x, 0.5 + p.z];
        let (p0, p1) = (at(a0, inner_radius(yf), yf), at(a1, inner_radius(yf), yf));
        q.tri([c, p0, p1], [uv(c), uv(p0), uv(p1)], Vec3::Y, metal, floor_in);
        let c = Vec3::new(0.0, BOTTOM, 0.0);
        let (p0, p1) = (at(a0, R_BOTTOM, BOTTOM), at(a1, R_BOTTOM, BOTTOM));
        q.tri([c, p0, p1], [uv(c), uv(p0), uv(p1)], Vec3::NEG_Y, metal, [190, 192, 196]);
    }

    // The ears, and the wire handle between them over the top.
    let ear_y = RIM - 0.07;
    let ear_r = radius(ear_y);
    let span = SPAN;
    // (the ears reach out past the rim's bead, so the wire clears it)
    for side in [-1.0f32, 1.0] {
        let (a, b) = (ear_r - 0.004, span + 0.012);
        let (lo, hi) = if side > 0.0 { (a, b) } else { (-b, -a) };
        q.cuboid(Vec3::new(lo, ear_y - 0.045, -0.035), Vec3::new(hi, ear_y + 0.03, 0.035), metal, [214, 216, 220]);
    }
    let pivot = Vec3::new(0.0, ear_y, 0.0);
    let down = handle.clamp(0.0, 1.0) * 1.35;
    let turn = Mat4::from_translation(pivot) * Mat4::from_rotation_x(-down) * Mat4::from_translation(-pivot);
    let arc = |t: f32| {
        let a = t * std::f32::consts::PI;
        // A tall arch: round over the top, the ends coming down to the ears.
        turn.transform_point3(pivot + Vec3::new(-a.cos() * span, a.sin() * span * 1.05, 0.0))
    };
    // The wire from each ear up to the wooden grip, which lies straight across the top
    // around it (the wire goes on inside it).
    const SEGS: usize = 6;
    let wire = 0.011;
    for (from, to) in [(0.0, GRIP), (1.0 - GRIP, 1.0)] {
        for i in 0..SEGS {
            let t0 = from + (to - from) * i as f32 / SEGS as f32;
            let t1 = from + (to - from) * (i + 1) as f32 / SEGS as f32;
            q.rod(arc(t0), arc(t1), wire, metal, [150, 152, 158]);
        }
    }
    let (g0, g1) = (arc(GRIP), arc(1.0 - GRIP));
    q.rod(g0, g1, 0.03, tex::PLANKS, [200, 170, 130]);

    if fill != Fill::Empty {
        emit_surface(&mut q, fill, surface);
    }
}

/// The liquid's top: a fan from the middle out to where the surface meets the inside wall.
fn emit_surface(q: &mut Quads, fill: Fill, surface: &Surface) {
    // The surface's slope in the bucket's own space.
    let slope = if surface.own_up {
        surface.tilt
    } else {
        let n = Vec3::new(-surface.tilt.x, 1.0, -surface.tilt.y);
        let local = q.m.inverse().transform_vector3(n);
        if local.y > 1e-3 {
            Vec2::new(-local.x / local.y, -local.z / local.y)
        } else {
            // Upside down: as far over as it goes (the wrong way up is nothing to see).
            Vec2::new(-local.x, -local.z).normalize_or_zero() * 10.0
        }
    };
    let bounce = surface.bounce.clamp(-0.04, 0.04);
    let slope = slope.clamp_length_max(max_slope(bounce));
    let height = |x: f32, z: f32, rel: f32| LEVEL + slope.x * x + slope.y * z + bounce * (1.0 - 2.0 * rel * rel);
    let k = (R_TOP - R_BOTTOM) / (RIM - BOTTOM);
    // Where the surface meets the inside wall along angle a (inner radius r(y) = r0 + k y).
    let wall = |a: f32| {
        let s = slope.x * a.cos() + slope.y * a.sin();
        let r0 = inner_radius(0.0);
        let y = (LEVEL - bounce + s * r0) / (1.0 - s * k);
        let r = inner_radius(y) - 0.002;
        Vec3::new(a.cos() * r, y, a.sin() * r)
    };
    // The world's animated water and lava frames (at their speeds, the water's texture
    // drifting like still water's), not drawn as the world's fluids: those move their
    // corners up and down in waves bigger than a bucket.
    let time = clock();
    let frames = tex::FLUID_FRAMES as f32;
    let (layer, tint, fl, drift) = match (fill, q.fl) {
        // An icon: the plain textures, water given its colour.
        (Fill::Water, 0) => (tex::WATER, [70u8, 120, 205], 0, Vec2::ZERO),
        (Fill::Lava, 0) => (tex::LAVA, [255, 255, 255], 0, Vec2::ZERO),
        (Fill::Water, f) => {
            let frame = (time * 10.0).floor() % frames;
            (tex::WATER_ANIM + frame as u32, [104, 160, 226], f, Vec2::new(0.03, 0.045) * time)
        }
        (_, f) => {
            let frame = (time * 6.67).floor() % frames;
            (tex::LAVA_ANIM + frame as u32, [255, 255, 255], f | flags::EMISSIVE, Vec2::new(0.01, 0.015) * time)
        }
    };
    let fl0 = q.fl;
    q.fl = fl;
    let light = q.light;
    if fill == Fill::Lava {
        // Lava lights itself.
        q.light = [255, 255, 255, light[3]];
    }
    // (The textures repeat: only the drift's fraction matters.)
    let drift = Vec2::new(drift.x.fract(), drift.y.fract());
    let uv = |p: Vec3| [0.5 + p.x * 1.4 + drift.x, 0.5 + p.z * 1.4 + drift.y];
    let c = Vec3::new(0.0, height(0.0, 0.0, 0.0), 0.0);
    for i in 0..SIDES {
        let (a0, a1) = (i as f32 / SIDES as f32 * TAU, (i + 1) as f32 / SIDES as f32 * TAU);
        let (w0, w1) = (wall(a0), wall(a1));
        let mid = |w: Vec3| {
            let (x, z) = (w.x * 0.55, w.z * 0.55);
            Vec3::new(x, height(x, z, 0.55), z)
        };
        let (m0, m1) = (mid(w0), mid(w1));
        q.tri([c, m1, m0], [uv(c), uv(m1), uv(m0)], Vec3::Y, layer, tint);
        q.quad([m0, m1, w1, w0], [uv(m0), uv(m1), uv(w1), uv(w0)], Vec3::Y, layer, tint);
    }
    q.fl = fl0;
    q.light = light;
}

/// Seconds since the game started, for the liquids' animation.
fn clock() -> f32 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f32()
}

/// Triangles in bucket space, each turned to face the way it is meant to.
struct Quads<'a> {
    out: &'a mut Vec<Vertex>,
    m: Mat4,
    flip: bool,
    light: [u8; 4],
    fl: u8,
}

impl Quads<'_> {
    fn tri(&mut self, p: [Vec3; 3], uv: [[f32; 2]; 3], facing: Vec3, layer: u32, tint: [u8; 3]) {
        let n = (p[1] - p[0]).cross(p[2] - p[0]);
        let paint = Paint { layer, light: self.light, face: face_index(facing), tint, fl: self.fl };
        let sides = if (n.dot(facing) < 0.0) != self.flip { Sides::Back } else { Sides::Front };
        tri_at(self.out, p.map(|p| self.m.transform_point3(p)), uv, &paint, sides);
    }

    fn quad(&mut self, p: [Vec3; 4], uv: [[f32; 2]; 4], facing: Vec3, layer: u32, tint: [u8; 3]) {
        self.tri([p[0], p[1], p[2]], [uv[0], uv[1], uv[2]], facing, layer, tint);
        self.tri([p[0], p[2], p[3]], [uv[0], uv[2], uv[3]], facing, layer, tint);
    }

    /// An axis-aligned box.
    fn cuboid(&mut self, lo: Vec3, hi: Vec3, layer: u32, tint: [u8; 3]) {
        let c = (lo + hi) * 0.5;
        let h = (hi - lo) * 0.5;
        self.boxed(c, [Vec3::X * h.x, Vec3::Y * h.y, Vec3::Z * h.z], layer, tint);
    }

    /// A square rod from `a` to `b`, `r` thick on each side.
    fn rod(&mut self, a: Vec3, b: Vec3, r: f32, layer: u32, tint: [u8; 3]) {
        let along = b - a;
        let dir = along.normalize_or(Vec3::Y);
        let side = dir.cross(Vec3::Z).normalize_or(Vec3::X);
        let up = side.cross(dir);
        // (a hair longer, so the joints close)
        self.boxed((a + b) * 0.5, [dir * (along.length() * 0.5 + r * 0.5), side * r, up * r], layer, tint);
    }

    /// A box around `c` with half-axes `e`.
    fn boxed(&mut self, c: Vec3, e: [Vec3; 3], layer: u32, tint: [u8; 3]) {
        for axis in 0..3 {
            for s in [-1.0f32, 1.0] {
                let n = e[axis] * s;
                let (u, v) = (e[(axis + 1) % 3], e[(axis + 2) % 3]);
                let f = c + n;
                self.quad(
                    [f - u - v, f + u - v, f + u + v, f - u + v],
                    [[0.2, 0.2], [0.4, 0.2], [0.4, 0.4], [0.2, 0.4]],
                    n,
                    layer,
                    tint,
                );
            }
        }
    }
}

/// The nearest of the block faces' directions (0 +X, 1 -X, 2 +Y, 3 -Y, 4 +Z, 5 -Z).
fn face_index(n: Vec3) -> u8 {
    let a = n.abs();
    if a.y >= a.x && a.y >= a.z {
        if n.y >= 0.0 { 2 } else { 3 }
    } else if a.x >= a.z {
        if n.x >= 0.0 { 0 } else { 1 }
    } else if n.z >= 0.0 {
        4
    } else {
        5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walking_rocks_the_water_but_never_spills_it() {
        for fill in [Fill::Water, Fill::Lava] {
            let mut s = Slosh::default();
            let dt = 1.0 / 144.0;
            let mut peak: f32 = 0.0;
            // Walking, turning back and forth, stopping hard.
            for i in 0..2000 {
                let t = i as f32 * dt;
                let x = if t < 5.0 { t * 4.3 } else { 5.0 * 4.3 };
                let z = (t * 3.0).sin() * 0.8;
                let y = (t * 11.0).sin().abs() * 0.06;
                s.update(fill, Vec3::new(x, y, z), dt);
                peak = peak.max(s.tilt.length());
                assert!(s.tilt.length() <= max_slope(0.0) + 1e-4, "{fill:?} spills: {:?}", s.tilt);
            }
            assert!(peak > 0.05, "{fill:?} did not move: {peak}");
            // And settles once it stops.
            for _ in 0..3000 {
                s.update(fill, Vec3::new(5.0 * 4.3, 0.0, 0.0), dt);
            }
            assert!(s.tilt.length() < 0.01 && s.bounce.abs() < 0.002, "{fill:?}: {s:?}");
        }
    }

    #[test]
    fn water_rocks_longer_than_lava() {
        let swings = |fill| {
            let mut s = Slosh::default();
            let dt = 1.0 / 120.0;
            let mut n = 0;
            let mut last = 0.0f32;
            for i in 0..1200 {
                let x = if i < 60 { i as f32 * dt * 5.0 } else { 60.0 * dt * 5.0 };
                s.update(fill, Vec3::new(x, 0.0, 0.0), dt);
                if i > 60 && s.tilt.x.signum() != last.signum() && s.tilt.x.abs() > 0.003 {
                    n += 1;
                }
                if s.tilt.x.abs() > 0.003 {
                    last = s.tilt.x;
                }
            }
            n
        };
        assert!(swings(Fill::Water) > swings(Fill::Lava) + 2, "{} {}", swings(Fill::Water), swings(Fill::Lava));
    }

    #[test]
    fn the_surface_stays_inside_the_bucket() {
        for fill in [Fill::Water, Fill::Lava] {
            let mut out = Vec::new();
            let tilted = Mat4::from_rotation_z(0.9);
            let surface = Surface { tilt: Vec2::new(0.5, 0.2), bounce: 0.03, own_up: false };
            emit(&mut out, tilted, fill, &surface, 0.0, [255; 4], flags::ENTITY);
            let back = tilted.inverse();
            for v in out.iter().filter(|v| v.layer as u32 >= tex::WATER_ANIM && (v.layer as u32) < tex::LAVA_ANIM + tex::FLUID_FRAMES) {
                let p = back.transform_point3(Vec3::from(v.pos));
                assert!(p.y < RIM && p.y > BOTTOM, "{p}");
                assert!(Vec2::new(p.x, p.z).length() <= inner_radius(p.y) + 1e-4, "{p}");
            }
        }
    }
}
