//! The guide book's pages and chapter tabs drawn onto a canvas, in a light or a dark theme:
//! the cover, the contents, and each laid-out piece (text, recipes with their arrows filling,
//! furnaces, rows and the guns' numbers as bars; the pictures are `pictures`).

use super::canvas::Canvas;
use super::layout::{INDENT, Layout, MARGIN_TOP, MARGIN_X, Metrics, PAGE_PX, Piece, ROW_TEXT, SHEET_H, SHEET_W, contents_rows, metrics};
use super::pages::smelt_time;
use crate::item::*;
use crate::ui::{Color, Font, rgba, with_alpha};
use crate::textures::tex;
use glam::Vec2;

/// The page's colors.
pub(super) struct Theme {
    pub(super) paper: Color,
    edge: Color,
    pub(super) ink: Color,
    pub(super) soft: Color,
    red: Color,
    pub(super) gold: Color,
    slot_edge: Color,
    slot_fill: Color,
    bar_bg: Color,
    bar: Color,
    hover: Color,
}

pub(super) const LIGHT: Theme = Theme {
    paper: rgba(243, 234, 210, 255),
    edge: rgba(150, 120, 80, 255),
    ink: rgba(56, 42, 32, 255),
    soft: rgba(118, 98, 76, 255),
    red: rgba(150, 44, 32, 255),
    gold: rgba(200, 150, 50, 255),
    slot_edge: rgba(170, 148, 112, 255),
    slot_fill: rgba(228, 214, 184, 255),
    bar_bg: rgba(222, 208, 176, 255),
    bar: rgba(176, 64, 44, 255),
    hover: rgba(150, 44, 32, 40),
};

pub(super) const DARK: Theme = Theme {
    paper: rgba(40, 38, 46, 255),
    edge: rgba(0, 0, 0, 255),
    ink: rgba(222, 216, 204, 255),
    soft: rgba(150, 144, 136, 255),
    red: rgba(240, 132, 104, 255),
    gold: rgba(226, 180, 74, 255),
    slot_edge: rgba(20, 19, 24, 255),
    slot_fill: rgba(64, 62, 72, 255),
    bar_bg: rgba(64, 62, 72, 255),
    bar: rgba(232, 112, 84, 255),
    hover: rgba(240, 132, 104, 40),
};

/// How a page is drawn.
pub(super) struct Look<'a> {
    /// Seconds, for the animations.
    pub(super) time: f32,
    pub(super) theme: &'a Theme,
    /// The contents entry under the crosshair.
    pub(super) hover: Option<usize>,
}

/// Page `n` of the book, drawn onto a new `SHEET_W` x `SHEET_H` canvas.
pub(super) fn draw_page<'a>(font: &'a Font, texture: &'a [u8], lay: &Layout, n: usize, look: &Look) -> Canvas<'a> {
    let m = metrics();
    let th = look.theme;
    let mut cv = Canvas::new(SHEET_W, SHEET_H, th.paper, font, texture);
    let (pw, ph) = PAGE_PX;
    // Darker toward the outer edge and at the spine.
    let right = n % 2 == 1;
    let e = 12.0 * m.u;
    for i in 0..12 {
        let f = 1.0 - i as f32 / 12.0;
        let x = if right { pw - (i + 1) as f32 * e / 12.0 } else { i as f32 * e / 12.0 };
        cv.fill(x, 0.0, e / 12.0, ph, with_alpha(th.edge, 0.16 * f));
        let s = if right { i as f32 * 1.5 } else { pw - (i + 1) as f32 * 1.5 };
        cv.fill(s, 0.0, 1.5, ph, with_alpha(th.edge, 0.2 * f));
    }
    let mut d = Draw { cv: &mut cv, m: &m, look, lay };
    if let Some(page) = lay.pages.get(n) {
        let (ix, iy) = (MARGIN_X * m.u, MARGIN_TOP * m.u);
        for (py, p) in page {
            d.piece(p, ix, iy + py);
        }
    }
    if n > 0 {
        let num = format!("{n}");
        cv.text_centered(&num, pw * 0.5, ph - 13.0 * m.u, m.fs, th.soft);
    }
    cv
}

/// Size of the tabs' texture: `model::book::TAB_LAYERS` layers side by side.
pub(super) const TABS_W: usize = 512;
pub(super) const TABS_H: usize = 128;

/// The chapter tabs, side by side (see `model::book::TABS_PER_HALF`): each a colored tab
/// with its number over its short name; the chapter open now stands out taller with a gold
/// edge, the one aimed at is lighter.
pub(super) fn draw_tabs<'a>(font: &'a Font, texture: &'a [u8], lay: &Layout, open: Option<usize>, hover: Option<usize>, theme: &Theme) -> Canvas<'a> {
    let mut cv = Canvas::new(TABS_W, TABS_H, [0.0; 4], font, texture);
    let colors = [
        rgba(166, 58, 44, 255),
        rgba(190, 120, 40, 255),
        rgba(150, 70, 36, 255),
        rgba(120, 140, 50, 255),
        rgba(60, 120, 120, 255),
        rgba(70, 80, 140, 255),
        rgba(120, 70, 130, 255),
        rgba(110, 96, 80, 255),
    ];
    let bottom = crate::model::items::book::TAB_PX;
    let w = TABS_W as f32 / 8.0;
    for (i, short) in lay.shorts.iter().enumerate().take(8) {
        let x = i as f32 * w;
        let (open, lit) = (open == Some(i), hover == Some(i));
        let top = if open { 1.0 } else { 7.0 };
        let base = colors[i % colors.len()];
        let c = if lit || open { std::array::from_fn(|k| if k == 3 { 1.0 } else { (base[k] * 1.3).min(1.0) }) } else { base };
        if open {
            cv.fill(x + 2.0, top - 1.0, w - 4.0, bottom - top + 1.0, theme.gold);
        }
        cv.fill(x + 3.0, top, w - 6.0, bottom - top, c);
        // A lighter band along the top, like a fold.
        cv.fill(x + 3.0, top, w - 6.0, 2.0, with_alpha([1.0; 4], 0.25));
        let white = rgba(250, 244, 230, 255);
        let n = format!("{}", i + 1);
        cv.text(&n, x + w * 0.5 - font.text_width(&n, 2.0) * 0.5, top + 3.0, 2.0, white, true);
        let tw = font.text_width(short, 1.0);
        cv.text(short, x + (w - tw) * 0.5, top + 23.0, 1.0, white, true);
    }
    cv
}

/// Loops 0..1 every `period` seconds.
pub(super) fn phase(time: f32, period: f32) -> f32 {
    (time / period).fract()
}

/// Drawing one page: its canvas, sizes, look and the book it is in.
pub(super) struct Draw<'c, 'a> {
    pub(super) cv: &'c mut Canvas<'a>,
    pub(super) m: &'c Metrics,
    pub(super) look: &'c Look<'c>,
    lay: &'c Layout,
}

impl Draw<'_, '_> {
    pub(super) fn icon(&mut self, id: ItemId, count: u8, x: f32, y: f32, size: f32) {
        self.cv.stack(x, y, size, &Stack::new(id, count.max(1)));
    }

    pub(super) fn slot(&mut self, x: f32, y: f32, size: f32) {
        let (u, th) = (self.m.u, self.look.theme);
        self.cv.fill(x, y, size, size, th.slot_edge);
        self.cv.fill(x + u, y + u, size - 2.0 * u, size - 2.0 * u, th.slot_fill);
    }

    /// A small arrow pointing right, filled from the left by `k` (0..1).
    fn arrow(&mut self, x: f32, y: f32, u: f32, k: f32) {
        let th = self.look.theme;
        let v = Vec2::new;
        let shaft = |cv: &mut Canvas, c: Color, w: f32| cv.fill(x, y + 2.0 * u, w, 2.0 * u, c);
        let head = [v(x + 5.0 * u, y - 0.5 * u), v(x + 9.0 * u, y + 3.0 * u), v(x + 5.0 * u, y + 6.5 * u)];
        shaft(self.cv, th.bar_bg, 5.0 * u);
        self.cv.poly(&head, th.bar_bg);
        let fill = 9.0 * u * k.clamp(0.0, 1.0);
        if fill > 0.0 {
            shaft(self.cv, th.soft, fill.min(5.0 * u));
            if fill > 5.0 * u {
                let t = (fill - 5.0 * u) / (4.0 * u);
                let top = y - 0.5 * u + 3.5 * u * t;
                let bot = y + 6.5 * u - 3.5 * u * t;
                self.cv.poly(
                    &[v(x + 5.0 * u, y - 0.5 * u), v(x + 5.0 * u + 4.0 * u * t, top), v(x + 5.0 * u + 4.0 * u * t, bot), v(x + 5.0 * u, y + 6.5 * u)],
                    th.soft,
                );
            }
        }
    }

    /// A puff of smoke `t` (0..1) of the way up from (x, y), `rise` book units high.
    pub(super) fn smoke(&mut self, x: f32, y: f32, t: f32, gray: u8, rise: f32) {
        let u = self.m.u;
        let r = (1.5 + 3.0 * t) * u;
        let (px, py) = (x + (t * 7.0).sin() * 2.0 * u, y - t * rise * u);
        let c = rgba(gray, gray, gray, 255);
        self.cv.circle(px, py, r, with_alpha(c, 0.6 * (1.0 - t)));
    }

    fn piece(&mut self, p: &Piece, x: f32, y: f32) {
        let (m, u) = (self.m, self.m.u);
        let th = self.look.theme;
        match p {
            Piece::Break | Piece::Gap(_) => {}
            Piece::Cover => self.cover(x, y),
            Piece::Contents => self.contents(x, y),
            Piece::Title(lines, _) => {
                let mut ty = y + 2.0 * u;
                for l in lines {
                    self.cv.text_centered(l, x + m.w * 0.5, ty, m.tfs, th.red);
                    ty += m.tlh;
                }
                // A gold rule with a diamond in the middle.
                let (cx, ry) = (x + m.w * 0.5, ty + u);
                self.cv.fill(cx - 40.0 * u, ry, 80.0 * u, 1.0, th.gold);
                let dd = 2.5 * u;
                let v = Vec2::new;
                self.cv.poly(&[v(cx, ry - dd), v(cx + dd, ry + 0.5), v(cx, ry + dd + 1.0), v(cx - dd, ry + 0.5)], th.gold);
            }
            Piece::Head(t) => {
                self.cv.text(t, x, y + 3.0 * u, m.fs, th.red, false);
            }
            Piece::Line { text, indent, dash } => {
                if *dash {
                    self.cv.fill(x + 2.0 * u, y + 3.0, 3.0 * u, 1.0, th.soft);
                }
                let ix = if *indent { INDENT * u } else { 0.0 };
                self.cv.text(text, x + ix, y, m.fs, th.ink, false);
            }
            Piece::Recipe(id) => self.recipe(*id, x, y),
            Piece::Smelt(id, at) => {
                let Some(out) = smelt(*id) else { return };
                let s = 16.0 * u;
                let yy = y + 2.0 * u;
                // The arrow fills as fast as the furnace smelts (twice as fast, to watch).
                let k = phase(self.look.time, smelt_time(smelt_tier(*id)) * 0.5);
                self.icon(*id, 1, x, yy, s);
                self.arrow(x + 18.5 * u, yy + 5.0 * u, u, k);
                self.icon(out, 1, x + 30.0 * u, yy, s);
                let tx = x + 50.0 * u;
                self.cv.text(&name(out), tx, y + u, m.fs, th.ink, false);
                self.cv.text(at, tx, y + u + m.lh, m.fs, th.soft, false);
            }
            Piece::Row(id, lines) => {
                self.icon(*id, 1, x, y + u, 16.0 * u);
                let h = lines.len() as f32 * m.lh;
                let mut ty = y + ((18.0 * u - h) * 0.5).max(0.0) + u;
                for l in lines {
                    self.cv.text(l, x + ROW_TEXT * u, ty, m.fs, th.ink, false);
                    ty += m.lh;
                }
            }
            Piece::Pic(pic) => self.picture(pic, x, y),
            Piece::Stat(label, k, value) => {
                let ty = y + u;
                self.cv.text(label, x, ty, m.fs, th.ink, false);
                let vw = self.cv.font().text_width(value, m.fs);
                self.cv.text(value, x + m.w - vw, ty, m.fs, th.soft, false);
                let bx = x + m.w * 0.4;
                let bw = m.w * 0.6 - vw - 5.0 * u;
                let bh = (3.0 * u).round().max(2.0);
                let by = (ty + 3.5 * m.fs - bh * 0.5).round();
                self.cv.fill(bx, by, bw, bh, th.bar_bg);
                self.cv.fill(bx, by, (bw * k.clamp(0.04, 1.0)).round(), bh, th.bar);
            }
        }
    }

    /// A crafting grid: the pattern (cells with several items cycle through them), an arrow
    /// filling up and what comes out, under its name.
    fn recipe(&mut self, id: ItemId, x: f32, y: f32) {
        let (m, u) = (self.m, self.m.u);
        let th = self.look.theme;
        let Some((rows, result)) = recipe_view(id) else { return };
        let title = if result.count > 1 { format!("{} \u{d7}{}", name(result.item), result.count) } else { name(result.item) };
        self.cv.text_centered(&title, x + m.w * 0.5, y + u, m.fs, th.soft);
        let cell = (18.0 * u).round();
        let cols = rows.iter().map(|r| r.len()).max().unwrap_or(1);
        let width = cols as f32 * cell + 18.0 * u + cell;
        let gx = (x + (m.w - width) * 0.5).round();
        let gy = (y + m.lh + 3.0 * u).round();
        let cycle = (self.look.time * 0.8) as usize;
        for (r, row) in rows.iter().enumerate() {
            for (c, items) in row.iter().enumerate() {
                let (sx, sy) = (gx + c as f32 * cell, gy + r as f32 * cell);
                self.slot(sx, sy, cell);
                if !items.is_empty() {
                    let item = items[cycle % items.len()];
                    self.icon(item, 1, sx + u, sy + u, cell - 2.0 * u);
                }
            }
        }
        // The arrow fills, then what comes out pops up in its slot.
        let k = phase(self.look.time + id as f32 * 0.37, 2.4);
        let ay = gy + rows.len() as f32 * cell * 0.5 - 3.0 * u;
        self.arrow(gx + cols as f32 * cell + 4.0 * u, ay, u, k / 0.6);
        let rx = gx + cols as f32 * cell + 18.0 * u;
        let ry = (gy + (rows.len() as f32 * cell - cell) * 0.5).round();
        self.slot(rx, ry, cell);
        // When the arrow is full the result pops (grows and settles back).
        let pop = if k > 0.6 { ((k - 0.6) / 0.15 * std::f32::consts::PI).sin().max(0.0) } else { 0.0 };
        let s = (cell - 2.0 * u) * (1.0 + 0.25 * pop);
        let o = (cell - s) * 0.5;
        self.icon(result.item, result.count, rx + o, ry + o, s);
    }

    fn cover(&mut self, x: f32, y: f32) {
        let (m, u) = (self.m, self.m.u);
        let th = self.look.theme;
        let front = &self.lay.front;
        let t = self.look.time;
        let cx = x + m.w * 0.5;
        let big = m.tfs + m.fs;
        self.cv.text_centered(&front.title, cx, y + 22.0 * u, big, th.red);
        self.cv.text_centered(&front.subtitle, cx, y + 26.0 * u + big * 10.0, m.fs, th.soft);
        // The book floats, and sparkles twinkle around it.
        let bob = (t * 1.8).sin() * 3.0 * u;
        let iy = y + m.h * 0.55 + bob;
        self.cv.sprite(Vec2::new(cx, iy), 30.0 * u, tex::BOOK, [255; 3], 1.0);
        for i in 0..7 {
            let a = i as f32 * 2.4;
            let k = phase(t + i as f32 * 0.29, 1.6);
            let r = (34.0 + 8.0 * (a * 1.7).sin()) * u;
            let (sx, sy) = (cx + a.cos() * r, iy - bob + a.sin() * r * 0.8);
            let s = (1.0 + 2.0 * (k * std::f32::consts::PI).sin()) * u;
            let v = Vec2::new;
            self.cv.poly(&[v(sx, sy - s), v(sx + s * 0.35, sy), v(sx, sy + s), v(sx - s * 0.35, sy)], th.gold);
            self.cv.poly(&[v(sx - s, sy), v(sx, sy - s * 0.35), v(sx + s, sy), v(sx, sy + s * 0.35)], th.gold);
        }
        // Gold corners.
        for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
            let (px, py) = (x + dx * (m.w - 12.0 * u), y + dy * (m.h - 12.0 * u));
            self.cv.fill(px, py + if dy > 0.5 { 11.0 * u } else { 0.0 }, 12.0 * u, 1.0, th.gold);
            self.cv.fill(px + if dx > 0.5 { 11.0 * u } else { 0.0 }, py, 1.0, 12.0 * u, th.gold);
        }
        let mut ty = y + m.h - 26.0 * u;
        for l in self.cv.font().wrap(&front.hint, m.w, m.fs) {
            self.cv.text_centered(&l, cx, ty, m.fs, th.soft);
            ty += m.lh;
        }
    }

    fn contents(&mut self, x: f32, y: f32) {
        let (m, u) = (self.m, self.m.u);
        let th = self.look.theme;
        let front = &self.lay.front;
        self.cv.text_centered(&front.contents, x + m.w * 0.5, y + 2.0 * u, m.tfs, th.red);
        let (top, row) = contents_rows(m);
        let mut ty = top;
        for (i, (title, page)) in self.lay.chapters.iter().enumerate() {
            let label = format!("{}. {}", i + 1, title);
            let num = format!("{page}");
            let hover = self.look.hover == Some(i);
            if hover {
                self.cv.fill(x - 2.0 * u, ty - 2.5 * u, m.w + 4.0 * u, row, th.hover);
            }
            let c = if hover { th.red } else { th.ink };
            let lw = self.cv.text(&label, x, ty, m.fs, c, false);
            let nw = self.cv.font().text_width(&num, m.fs);
            self.cv.text(&num, x + m.w - nw, ty, m.fs, c, false);
            // Dots between the title and the page number.
            let mut dx = x + lw + 3.0 * u;
            while dx < x + m.w - nw - 4.0 * u {
                self.cv.fill(dx, ty + 6.0, 1.0, 1.0, th.soft);
                dx += 3.0;
            }
            ty += row;
        }
        ty += 6.0 * u;
        for l in self.cv.font().wrap(&front.tip, m.w, m.fs) {
            self.cv.text(&l, x, ty, m.fs, th.soft, false);
            ty += m.lh;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::layout::layout;

    #[test]
    fn every_page_draws() {
        let font = Font::new();
        let texture = vec![200u8; crate::textures::TILE * crate::textures::TILE * 4 * tex::LAYERS];
        let lay = layout(&font, true, &("E".into(), "R".into()));
        for theme in [&LIGHT, &DARK] {
            for n in 0..lay.pages.len() {
                let look = Look { time: n as f32 * 0.7, theme, hover: Some(1) };
                let cv = draw_page(&font, &texture, &lay, n, &look);
                assert_eq!(cv.px.len(), SHEET_W * SHEET_H);
            }
        }
    }
}
