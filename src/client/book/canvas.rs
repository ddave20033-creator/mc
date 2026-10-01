//! A small software canvas the guide book's pages are drawn on (then uploaded as texture
//! layers onto the 3D book): filled shapes, the UI's bitmap font, and the block textures for
//! item icons, the way the item screens draw them.

use crate::item::{Icon, Stack, block_of, icon, max_damage};
use crate::ui::{Color, Font};
use crate::world::{TintKind, face_texture, icon_tint, tint_kind};
use crate::world::textures::{TILE, tex};
use glam::Vec2;

pub struct Canvas<'a> {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[u8; 4]>,
    font: &'a Font,
    /// Every block texture layer at full size (`TILE` x `TILE` RGBA each).
    tex: &'a [u8],
}

impl<'a> Canvas<'a> {
    pub fn new(w: usize, h: usize, fill: Color, font: &'a Font, tex: &'a [u8]) -> Self {
        Self {
            w,
            h,
            px: vec![to_bytes(fill); w * h],
            font,
            tex,
        }
    }

    pub fn font(&self) -> &Font {
        self.font
    }

    fn blend(&mut self, x: i32, y: i32, c: [f32; 4]) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 || c[3] <= 0.0 {
            return;
        }
        let p = &mut self.px[y as usize * self.w + x as usize];
        let a = c[3].min(1.0);
        for i in 0..3 {
            p[i] = (p[i] as f32 * (1.0 - a) + c[i] * 255.0 * a).round().clamp(0.0, 255.0) as u8;
        }
        p[3] = 255;
    }

    /// The pixels whose centers are inside the rectangle.
    pub fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, c: Color) {
        let (x0, y0) = ((x - 0.5).ceil() as i32, (y - 0.5).ceil() as i32);
        let (x1, y1) = ((x + w - 0.5).ceil() as i32, (y + h - 0.5).ceil() as i32);
        for py in y0.max(0)..y1.min(self.h as i32) {
            for px in x0.max(0)..x1.min(self.w as i32) {
                self.blend(px, py, c);
            }
        }
    }

    /// A convex polygon.
    pub fn poly(&mut self, p: &[Vec2], c: Color) {
        let (mut lo, mut hi) = (p[0], p[0]);
        for q in p {
            lo = lo.min(*q);
            hi = hi.max(*q);
        }
        let n = p.len();
        let area: f32 = (0..n).map(|i| p[i].perp_dot(p[(i + 1) % n])).sum();
        for py in (lo.y.floor() as i32).max(0)..(hi.y.ceil() as i32).min(self.h as i32) {
            for px in (lo.x.floor() as i32).max(0)..(hi.x.ceil() as i32).min(self.w as i32) {
                let q = Vec2::new(px as f32 + 0.5, py as f32 + 0.5);
                let inside = (0..n).all(|i| {
                    let (a, b) = (p[i], p[(i + 1) % n]);
                    (b - a).perp_dot(q - a) * area.signum() >= 0.0
                });
                if inside {
                    self.blend(px, py, c);
                }
            }
        }
    }

    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, c: Color) {
        for py in ((cy - r).floor() as i32).max(0)..((cy + r).ceil() as i32).min(self.h as i32) {
            for px in ((cx - r).floor() as i32).max(0)..((cx + r).ceil() as i32).min(self.w as i32) {
                let d = Vec2::new(px as f32 + 0.5 - cx, py as f32 + 0.5 - cy).length();
                if d <= r {
                    self.blend(px, py, c);
                }
            }
        }
    }

    /// Text with its top left at (x, y), each font pixel `size` pixels. Returns the width.
    pub fn text(&mut self, s: &str, x: f32, y: f32, size: f32, c: Color, shadow: bool) -> f32 {
        if shadow {
            let sc = [c[0] * 0.25, c[1] * 0.25, c[2] * 0.25, c[3]];
            self.text(s, x + size, y + size, size, sc, false);
        }
        let (x, y) = (x.round(), y.round());
        let mut pen = x;
        for ch in s.chars() {
            let g = *self.font.glyph(ch);
            if g.w > 0.0 {
                for gx in 0..8u32 {
                    let cx = pen + (gx as f32 - g.minx as f32) * size;
                    if g.top & (1 << gx) != 0 {
                        self.fill(cx, y - size, size, size, c);
                    }
                    for (gy, row) in g.bits.iter().enumerate() {
                        if row & (1 << gx) != 0 {
                            self.fill(cx, y + gy as f32 * size, size, size, c);
                        }
                    }
                }
            }
            pen += g.adv * size;
        }
        self.font.text_width(s, size)
    }

    pub fn text_centered(&mut self, s: &str, cx: f32, y: f32, size: f32, c: Color) {
        let w = self.font.text_width(s, size);
        self.text(s, cx - w * 0.5, y, size, c, false);
    }

    /// A texel of block texture `layer` (u, v in 0..1).
    fn sample(&self, layer: u32, u: f32, v: f32) -> [u8; 4] {
        let t = TILE as f32;
        let (x, y) = (
            ((u * t) as usize).min(TILE - 1),
            ((v * t) as usize).min(TILE - 1),
        );
        let i = ((layer as usize * TILE + y) * TILE + x) * 4;
        match self.tex.get(i..i + 4) {
            Some(p) => [p[0], p[1], p[2], p[3]],
            None => [0; 4],
        }
    }

    /// Block texture `layer` on the parallelogram with corners `p` (top left, top right, bottom
    /// right, bottom left of the texture), darkened by `shade` and tinted.
    pub fn tex_quad(&mut self, p: [Vec2; 4], layer: u32, shade: f32, tint: [u8; 3], alpha: f32) {
        let (o, ex, ey) = (p[0], p[1] - p[0], p[3] - p[0]);
        let det = ex.perp_dot(ey);
        if det.abs() < 1e-4 {
            return;
        }
        let (mut lo, mut hi) = (p[0], p[0]);
        for q in p {
            lo = lo.min(q);
            hi = hi.max(q);
        }
        // Only the grass block's side keeps its dirt untinted (its alpha is the mask).
        let masked = layer == tex::GRASS_SIDE;
        for py in (lo.y.floor() as i32).max(0)..(hi.y.ceil() as i32).min(self.h as i32) {
            for px in (lo.x.floor() as i32).max(0)..(hi.x.ceil() as i32).min(self.w as i32) {
                let q = Vec2::new(px as f32 + 0.5, py as f32 + 0.5) - o;
                let u = q.perp_dot(ey) / det;
                let v = ex.perp_dot(q) / det;
                if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                    continue;
                }
                let t = self.sample(layer, u, v);
                if t[3] < 128 {
                    continue;
                }
                let mask = if masked {
                    ((t[3] as f32 / 255.0 - 0.6) / 0.4).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let c = std::array::from_fn(|i| {
                    if i == 3 {
                        alpha
                    } else {
                        let k = 1.0 + (tint[i] as f32 / 255.0 - 1.0) * mask;
                        t[i] as f32 / 255.0 * k * shade
                    }
                });
                self.blend(px, py, c);
            }
        }
    }

    /// A flat sprite of `layer`, `2r` wide, centered on `c`.
    pub fn sprite(&mut self, c: Vec2, r: f32, layer: u32, tint: [u8; 3], alpha: f32) {
        let v = Vec2::new;
        self.tex_quad(
            [v(c.x - r, c.y - r), v(c.x + r, c.y - r), v(c.x + r, c.y + r), v(c.x - r, c.y + r)],
            layer,
            1.0,
            tint,
            alpha,
        );
    }

    /// An isometric block with different textures on its top, left and right faces.
    #[allow(clippy::too_many_arguments)]
    pub fn cube(&mut self, c: Vec2, r: f32, top: u32, left: u32, right: u32, top_tint: [u8; 3], side_tint: [u8; 3]) {
        let k = 0.866 * r;
        let v = Vec2::new;
        self.tex_quad(
            [v(c.x, c.y - r), v(c.x + k, c.y - r * 0.5), v(c.x, c.y), v(c.x - k, c.y - r * 0.5)],
            top,
            1.0,
            top_tint,
            1.0,
        );
        self.tex_quad(
            [v(c.x - k, c.y - r * 0.5), v(c.x, c.y), v(c.x, c.y + r), v(c.x - k, c.y + r * 0.5)],
            left,
            0.8,
            side_tint,
            1.0,
        );
        self.tex_quad(
            [v(c.x, c.y), v(c.x + k, c.y - r * 0.5), v(c.x + k, c.y + r * 0.5), v(c.x, c.y + r)],
            right,
            0.62,
            side_tint,
            1.0,
        );
    }

    /// An item icon `size` pixels big with its count and durability bar, like the item
    /// screens' `draw_stack`.
    pub fn stack(&mut self, x: f32, y: f32, size: f32, st: &Stack) {
        let c = Vec2::new(x + size * 0.5, y + size * 0.5);
        match icon(st.item) {
            Icon::Block(b) => {
                let tint = icon_tint(b);
                let top = if tint_kind(b, 2) != TintKind::None { tint } else { [255; 3] };
                let side = if tint_kind(b, 0) != TintKind::None { tint } else { [255; 3] };
                self.cube(
                    c,
                    size * 0.47,
                    face_texture(b, 2),
                    face_texture(b, 5),
                    face_texture(b, 0),
                    top,
                    side,
                );
            }
            Icon::Flat(layer) => {
                let tint = block_of(st.item).map(icon_tint).unwrap_or([255; 3]);
                self.sprite(c, size * 0.5, layer, tint, 1.0);
            }
        }
        let px = (size / 16.0).max(1.0);
        let max = max_damage(st.item);
        if max > 0 && st.damage > 0 {
            let f = 1.0 - st.damage as f32 / max as f32;
            let (bx, by, bw) = (x + 2.0 * px, y + size - 3.0 * px, size - 4.0 * px);
            self.fill(bx, by, bw, 2.0 * px, [0.0, 0.0, 0.0, 1.0]);
            self.fill(bx, by, (bw * f).round().max(px), px, [1.0 - f, f, 0.0, 1.0]);
        }
        if st.count > 1 {
            let text = st.count.to_string();
            let tw = self.font.text_width(&text, 1.0);
            self.text(&text, x + size - tw, y + size - 7.0, 1.0, [1.0; 4], true);
        }
    }
}

fn to_bytes(c: Color) -> [u8; 4] {
    std::array::from_fn(|i| (c[i] * 255.0).round().clamp(0.0, 255.0) as u8)
}
