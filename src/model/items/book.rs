//! The guide book held open: two covers, the stacks of pages with the pages the reader is on
//! drawn onto them (texture layers made by `game::book`), and a page turning over when its
//! reader turns one. Held in both hands in first person, and in front of the chest on the
//! player model, where the others see which page it is open at and what is on it.

use crate::model::emit_box;
use crate::world::mesh::Vertex;
use crate::world::textures::tex;
use glam::{Mat4, Vec2, Vec3};
use std::f32::consts::PI;

/// Seconds a page takes to turn over.
pub const TURN_TIME: f32 = 0.45;
/// Seconds a page takes to turn while leafing through several (to a chapter far away).
pub const RIFFLE_TIME: f32 = 0.16;
/// Block texture layers of one page (2 across, 3 down), and the part of them the page fills.
pub const SHEET_LAYERS: u32 = 6;
pub const SHEET_FILL: Vec2 = Vec2::new(168.0 * 1.52 / 256.0, 216.0 * 1.52 / 384.0);

/// A page's size (model pixels): across, along the spine, and the stack's thickness.
pub const PAGE_W: f32 = 5.6;
pub const PAGE_H: f32 = PAGE_W * 216.0 / 168.0;
const STACK: f32 = 0.9;
const COVER: f32 = 0.35;
/// How far each half is tipped up from the spine (the book lies open in a shallow V).
const TIP: f32 = 0.16;
/// Gap between a page and the spine.
const GAP: f32 = 0.3;

/// How an open book looks: the page turning over (0 none, going to 1 from the right to the
/// left, to -1 back), and the first texture layer of each page shown on it (None: a plain
/// written page): the left and right pages under the turning one, and the turning page's
/// side facing up when it lies on the right and the side facing up when it lies on the left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BookView {
    pub turn: f32,
    pub pages: [Option<u32>; 4],
    /// The chapter tabs along the top: the first of their `TAB_LAYERS` layers.
    pub tabs: Option<u32>,
    /// Turned around to show the pages to someone in front (0 read by the holder .. 1).
    pub show: f32,
}

/// Chapter tabs along the top of the book: this many on each half, drawn on `TAB_LAYERS`
/// layers side by side (the left half's tabs on the first two), `TAB_PX` texels tall.
pub const TABS_PER_HALF: usize = 4;
pub const TAB_LAYERS: u32 = 4;
pub const TAB_PX: f32 = 56.0;
/// How far the tabs stick out over the pages' tops (model pixels). They come out from
/// between the pages, under the top ones, so a turning page never touches them.
const TAB_H: f32 = PAGE_W / 256.0 * TAB_PX;
const TAB_IN: f32 = 0.0;
const TAB_Y: f32 = STACK * 0.6;
/// How far a turning page stays above the pages it lies on (so they never fight over the
/// same pixels).
const HOVER: f32 = 0.06;

/// What the middle of the view is on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BookHit {
    /// A page: the right one or the left, and where on it (0..1 across from its left edge,
    /// 0..1 down from its top).
    Page(bool, Vec2),
    /// A chapter tab (0 the leftmost).
    Tab(usize),
}

/// The part of a page's texture that is shown (texels).
fn sheet_texels() -> Vec2 {
    Vec2::new(256.0, 384.0) * SHEET_FILL
}

/// Texture coordinate of texel `p` in the layer starting at texel `start`: kept half a texel
/// inside the layer, so the filtering never reaches around to its other edge.
fn layer_uv(p: f32, start: f32) -> f32 {
    (p - start).clamp(0.5, 127.5) / 128.0
}

/// Two triangles of the quad a, b, c, d seen from the side `toward` points to (the world
/// pipeline drops the other side), or from both sides.
fn push_quad(out: &mut Vec<Vertex>, q: [Vertex; 4], toward: Option<Vec3>) {
    let p = |v: &Vertex| Vec3::from(v.pos);
    let front = (p(&q[1]) - p(&q[0])).cross(p(&q[2]) - p(&q[0]));
    match toward {
        Some(n) if front.dot(n) >= 0.0 => out.extend_from_slice(&[q[0], q[1], q[2], q[0], q[2], q[3]]),
        Some(_) => out.extend_from_slice(&[q[0], q[2], q[1], q[0], q[3], q[2]]),
        None => out.extend_from_slice(&[q[0], q[1], q[2], q[0], q[2], q[3], q[0], q[2], q[1], q[0], q[3], q[2]]),
    }
}

/// A page drawn from its sheet of layers on the top (+Y) of the rectangle x0..x1, z0..z1 at
/// height `y` of `m`, its left edge at x0.
#[allow(clippy::too_many_arguments)]
fn emit_sheet(out: &mut Vec<Vertex>, m: Mat4, x: (f32, f32), z: (f32, f32), y: f32, base: u32, light: [u8; 4], fl: u8) {
    let full = sheet_texels();
    let up = m.transform_vector3(Vec3::Y);
    for row in 0..3u32 {
        for col in 0..2u32 {
            let (px0, py0) = (col as f32 * 128.0, row as f32 * 128.0);
            let (px1, py1) = ((px0 + 128.0).min(full.x), (py0 + 128.0).min(full.y));
            if px1 <= px0 || py1 <= py0 {
                continue;
            }
            let corner = |px: f32, py: f32| Vertex {
                pos: m
                    .transform_point3(Vec3::new(
                        x.0 + (x.1 - x.0) * px / full.x,
                        y,
                        z.0 + (z.1 - z.0) * py / full.y,
                    ))
                    .to_array(),
                uv: [layer_uv(px, px0), layer_uv(py, py0)],
                layer: (base + row * 2 + col) as f32,
                light: [light[0], light[1], light[2], 2],
                tint: [255, 255, 255, fl],
            };
            let q = [corner(px0, py0), corner(px1, py0), corner(px1, py1), corner(px0, py1)];
            push_quad(out, q, Some(up));
        }
    }
}

/// A box without its top face (a page is drawn there instead).
#[allow(clippy::too_many_arguments)]
fn emit_open_box(out: &mut Vec<Vertex>, m: Mat4, min: Vec3, max: Vec3, layers: [u32; 6], light: [u8; 4], fl: u8) {
    let start = out.len();
    emit_box(out, m, min, max, layers, [[255; 3]; 6], light, fl);
    // Six vertices per face, +Y is the third.
    out.drain(start + 12..start + 18);
}

/// The page turning over, as a curved sheet: it turns about the spine from lying on the
/// right to lying on the left (`k` 0..1), its free edge trailing behind in a curl that is
/// strongest halfway. Its upper side (at the start) shows `front`, the other side `back`.
#[allow(clippy::too_many_arguments)]
fn emit_turning_page(out: &mut Vec<Vertex>, m: Mat4, k: f32, forward: bool, front: Option<u32>, back: Option<u32>, light: [u8; 4], fl: u8) {
    let h = PAGE_H * 0.5;
    let reach = PAGE_W + GAP;
    let angle = TIP + (PI - 2.0 * TIP) * k;
    // The trailing edge lags: less turned going forward, more going back.
    let curl = 1.1 * (k * PI).sin() * if forward { -1.0 } else { 1.0 };
    // Height of the spine end: on the right stack at the start, on the left one (upside
    // down) at the end, lifted a little on the way.
    let lift = (STACK + HOVER) * (1.0 - 2.0 * k) + 0.3 * (k * PI).sin();
    let root = Vec3::new(-angle.sin() * lift, angle.cos() * lift, 0.0);
    // The curve along the page (distance from the spine to its point and direction there),
    // stepped finely.
    const STEPS: usize = 48;
    let mut pts = Vec::with_capacity(STEPS + 1);
    let mut p = root;
    for i in 0..=STEPS {
        let s = reach * i as f32 / STEPS as f32;
        let theta = angle + curl * (s / reach).powi(2);
        pts.push((s, p, theta));
        let ds = reach / STEPS as f32;
        p += Vec3::new(theta.cos(), theta.sin(), 0.0) * ds;
    }
    let at = |s: f32| {
        let f = (s / reach * STEPS as f32).clamp(0.0, STEPS as f32 - 1e-3);
        let i = f as usize;
        let (a, b) = (pts[i], pts[i + 1]);
        let t = f - i as f32;
        (a.1.lerp(b.1, t), a.2 + (b.2 - a.2) * t)
    };
    let full = sheet_texels();
    let blank = [tex::BOOK_PAGE];
    for (upper, sheet) in [(true, front), (false, back)] {
        // Across the texture in narrow strips (to follow the curve), 8 texels each, so a
        // strip never runs over from one layer's column into the next.
        let mut cuts: Vec<f32> = (0..).map(|i| i as f32 * 8.0).take_while(|&x| x < full.x - 0.5).collect();
        cuts.push(full.x);
        for row in 0..3u32 {
            let (py0, py1) = (row as f32 * 128.0, ((row + 1) as f32 * 128.0).min(full.y));
            for pair in cuts.windows(2) {
                let (pa, pb) = (pair[0], pair[1]);
                let col = if pa >= 128.0 { 1 } else { 0 };
                // The upper side reads from the spine out; the other from the free edge in.
                let s_of = |px: f32| {
                    let k = px / full.x;
                    GAP + PAGE_W * if upper { k } else { 1.0 - k }
                };
                let corner = |px: f32, py: f32| {
                    let (pos, theta) = at(s_of(px));
                    let normal = Vec3::new(-theta.sin(), theta.cos(), 0.0) * if upper { 1.0 } else { -1.0 };
                    let z = -h + PAGE_H * py / full.y;
                    let (layer, uv) = match sheet {
                        Some(base) => (
                            (base + row * 2 + col) as f32,
                            [layer_uv(px, col as f32 * 128.0), layer_uv(py, py0)],
                        ),
                        None => (blank[0] as f32, [px / full.x, py / full.y]),
                    };
                    (
                        Vertex {
                            pos: m.transform_point3(pos + Vec3::new(0.0, 0.0, z) + normal * 0.005).to_array(),
                            uv,
                            layer,
                            light: [light[0], light[1], light[2], 2],
                            tint: [255, 255, 255, fl],
                        },
                        normal,
                    )
                };
                let q = [corner(pa, py0), corner(pb, py0), corner(pb, py1), corner(pa, py1)];
                let n = m.transform_vector3(q[0].1 + q[2].1);
                push_quad(out, q.map(|c| c.0), Some(n));
            }
        }
    }
}

/// The open book: `m` has the spine along Z through its origin and the pages facing +Y, their
/// tops toward -Z (the reader is on the +Z side).
pub fn emit_open_book(out: &mut Vec<Vertex>, m: Mat4, view: &BookView, light: [u8; 4], fl: u8) {
    let cover = [tex::BOOK_COVER; 6];
    let edges = [tex::BOOK_EDGE; 6];
    let white = [[255; 3]; 6];
    let (h, w) = (PAGE_H * 0.5, PAGE_W);
    // The spine, under the middle.
    emit_box(
        out,
        m,
        Vec3::new(-0.6, -COVER - 0.25, -h - 0.3),
        Vec3::new(0.6, -0.25, h + 0.3),
        cover,
        white,
        light,
        fl,
    );
    for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
        let half = m * Mat4::from_rotation_z(side * TIP);
        let (x0, x1) = if side > 0.0 { (0.25, w + GAP + 0.3) } else { (-w - GAP - 0.3, -0.25) };
        emit_box(out, half, Vec3::new(x0, -COVER, -h - 0.3), Vec3::new(x1, 0.0, h + 0.3), cover, white, light, fl);
        let (x0, x1) = if side > 0.0 { (GAP, w + GAP) } else { (-w - GAP, -GAP) };
        let (lo, hi) = (Vec3::new(x0, 0.0, -h), Vec3::new(x1, STACK, h));
        match view.pages[i] {
            Some(base) => {
                emit_open_box(out, half, lo, hi, edges, light, fl);
                emit_sheet(out, half, (x0, x1), (-h, h), STACK, base, light, fl);
            }
            None => {
                let mut layers = edges;
                layers[2] = tex::BOOK_PAGE;
                emit_box(out, half, lo, hi, layers, white, light, fl);
            }
        }
    }
    if let Some(base) = view.tabs {
        // Each half's tabs stick out over its top edge from between its pages.
        for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
            let half = m * Mat4::from_rotation_z(side * TIP);
            let (x0, x1) = if side > 0.0 { (GAP, w + GAP) } else { (-w - GAP, -GAP) };
            let up = half.transform_vector3(Vec3::Y);
            for col in 0..2u32 {
                let (a, b) = (x0 + (x1 - x0) * col as f32 * 0.5, x0 + (x1 - x0) * (col + 1) as f32 * 0.5);
                let v1 = (TAB_PX - 0.5) / 128.0;
                let corner = |x: f32, z: f32, u: f32, v: f32| Vertex {
                    pos: half.transform_point3(Vec3::new(x, TAB_Y, z)).to_array(),
                    uv: [u, v],
                    layer: (base + i as u32 * 2 + col) as f32,
                    light: [light[0], light[1], light[2], 2],
                    tint: [255, 255, 255, fl],
                };
                let (u0, u1) = (0.5 / 128.0, 127.5 / 128.0);
                let q = [
                    corner(a, -h - TAB_H, u0, 0.5 / 128.0),
                    corner(b, -h - TAB_H, u1, 0.5 / 128.0),
                    corner(b, -h + TAB_IN, u1, v1),
                    corner(a, -h + TAB_IN, u0, v1),
                ];
                // Seen from above and from below (turned around to show).
                push_quad(out, q, None);
                let _ = up;
            }
        }
    }
    if view.turn != 0.0 {
        let k = ease(view.turn.abs().min(1.0));
        let forward = view.turn > 0.0;
        let k = if forward { k } else { 1.0 - k };
        emit_turning_page(out, m, k, forward, view.pages[2], view.pages[3], light, fl);
    }
}

/// What a ray (in the space of `m`, as `emit_open_book` takes it) hits of the open book: a
/// page or, with `tabs`, a chapter tab.
pub fn book_hit(m: Mat4, origin: Vec3, dir: Vec3, tabs: bool) -> Option<BookHit> {
    let h = PAGE_H * 0.5;
    let mut best: Option<(f32, BookHit)> = None;
    for side in [-1.0f32, 1.0] {
        let inv = (m * Mat4::from_rotation_z(side * TIP)).inverse();
        let (o, d) = (inv.transform_point3(origin), inv.transform_vector3(dir));
        if d.y.abs() < 1e-6 {
            continue;
        }
        let across = |p: Vec3| if side > 0.0 { (p.x - GAP) / PAGE_W } else { (p.x + GAP + PAGE_W) / PAGE_W };
        let mut consider = |t: f32, hit: BookHit| {
            if t > 0.0 && best.is_none_or(|b| t < b.0) {
                best = Some((t, hit));
            }
        };
        let t = (STACK - o.y) / d.y;
        let p = o + d * t;
        let (a, down) = (across(p), (p.z + h) / PAGE_H);
        if (0.0..1.0).contains(&a) && (0.0..1.0).contains(&down) {
            consider(t, BookHit::Page(side > 0.0, Vec2::new(a, down)));
        }
        if tabs {
            let t = (TAB_Y - o.y) / d.y;
            let p = o + d * t;
            let a = across(p);
            if (0.0..1.0).contains(&a) && (-h - TAB_H..-h + TAB_IN).contains(&p.z) {
                let i = (a * TABS_PER_HALF as f32) as usize + if side > 0.0 { TABS_PER_HALF } else { 0 };
                consider(t, BookHit::Tab(i));
            }
        }
    }
    best.map(|b| b.1)
}

fn ease(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Another player's book as their poses tell it: a page turns over (for `TURN_TIME`) each
/// time their count of page turns changes.
#[derive(Default)]
pub struct TurnAnim {
    last: Option<u8>,
    /// Progress of the page turning now (0 none), how long it takes, and whether it goes
    /// back.
    t: f32,
    time: f32,
    back: bool,
    /// How far the book is turned around to show it (eases toward what the pose says).
    show: f32,
}

impl TurnAnim {
    /// Eases toward showing the book (or not); returns how far it is turned around.
    pub fn show(&mut self, on: bool, dt: f32) -> f32 {
        let target = if on { 1.0 } else { 0.0 };
        self.show += (target - self.show) * (1.0 - (-8.0 * dt).exp());
        self.show
    }

    /// `book` is the pose's `net::book` bits. Returns the turn to draw (see `BookView`), or
    /// None while the book is not held.
    pub fn update(&mut self, book: u8, dt: f32) -> Option<f32> {
        use crate::net::book::{BACK, OPEN, TURNS};
        if book & OPEN == 0 {
            *self = Self::default();
            return None;
        }
        let turns = book & TURNS;
        if let Some(last) = self.last.filter(|&l| l != turns) {
            // Several turns at once, or one before the last is done: they are leafing
            // through, so the pages go over as quickly as in their hands.
            let many = turns.wrapping_sub(last) & TURNS > 1;
            self.time = if many || self.t > 0.0 { RIFFLE_TIME } else { TURN_TIME };
            self.t = dt;
            self.back = book & BACK != 0;
        } else if self.t > 0.0 {
            self.t += dt;
            if self.t >= self.time {
                self.t = 0.0;
            }
        }
        self.last = Some(turns);
        let k = if self.t > 0.0 { (self.t / self.time).min(1.0) } else { 0.0 };
        Some(if self.back { -k } else { k })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ray_down_onto_the_right_page_hits_it() {
        let m = Mat4::IDENTITY;
        let hit = |x: f32, z: f32| book_hit(m, Vec3::new(x, 5.0, z), Vec3::NEG_Y, true);
        let Some(BookHit::Page(true, at)) = hit(GAP + PAGE_W * 0.25, -PAGE_H * 0.25) else { panic!() };
        assert!((at.x - 0.25).abs() < 0.05 && (at.y - 0.25).abs() < 0.05, "{at}");
        let Some(BookHit::Page(false, at)) = hit(-GAP - PAGE_W * 0.1, PAGE_H * 0.4) else { panic!() };
        assert!((at.x - 0.9).abs() < 0.05 && at.y > 0.85, "{at}");
        assert!(hit(0.0, PAGE_H).is_none());
        // Over the tops: the tabs, the left half's first.
        assert_eq!(hit(-GAP - PAGE_W * 0.9, -PAGE_H * 0.5 - TAB_H * 0.5), Some(BookHit::Tab(0)));
        assert_eq!(hit(GAP + PAGE_W * 0.3, -PAGE_H * 0.5 - TAB_H * 0.5), Some(BookHit::Tab(5)));
    }

    #[test]
    fn another_players_page_turns_follow_their_count() {
        use crate::net::book::{BACK, OPEN};
        let mut a = TurnAnim::default();
        assert_eq!(a.update(0, 0.1), None);
        assert_eq!(a.update(OPEN | 3, 0.1), Some(0.0));
        // One turn forward: a slow page turn.
        let k = a.update(OPEN | 4, 0.1).unwrap();
        assert!(k > 0.0 && a.time == TURN_TIME);
        // The next one before it is done: leafing, quicker.
        let k = a.update(OPEN | BACK | 5, 0.05).unwrap();
        assert!(k < 0.0 && a.time == RIFFLE_TIME);
        for _ in 0..10 {
            a.update(OPEN | BACK | 5, 0.05);
        }
        assert_eq!(a.update(OPEN | BACK | 5, 0.05), Some(0.0));
    }
}
