//! The minigames screen: a big title, a grid of cards (the soccer game first, the rest empty
//! places for games to come) and the back button.

use super::{backdrop, Action};
use crate::app::lang::t;
use crate::ui::*;
use glam::Vec2;
use std::f32::consts::TAU;

const COLS: usize = 5;
const ROWS: usize = 2;
/// A card's height to its width.
const TALL: f32 = 1.2;

pub fn minigames(ui: &mut Ui) -> Action {
    let (w, h, s) = (ui.w, ui.h, ui.s);
    backdrop(ui, 1.0);

    // The title.
    let big = (s * 2.3).round();
    let title = t("menu.minigames").to_uppercase();
    let ty = (18.0 * s).round();
    let a = ui.appear();
    let drop = ui.style(a, Vec2::new(0.0, -((1.0 - a) * 8.0 * s).round()));
    ui.text_centered(&title, w * 0.5, ty, big, WHITE, true);
    ui.restore(drop);

    // The grid, as large as fits between the title and the back button.
    let gap = (10.0 * s).round();
    let top = ty + 7.0 * big + 18.0 * s;
    let bottom = h - 40.0 * s;
    let mut cw = ((w * 0.8).min(w - 32.0 * s) - gap * (COLS - 1) as f32) / COLS as f32;
    if (cw * TALL) * ROWS as f32 + gap * (ROWS - 1) as f32 > bottom - top {
        cw = ((bottom - top - gap * (ROWS - 1) as f32) / ROWS as f32) / TALL;
    }
    let (cw, ch) = (cw.round(), (cw * TALL).round());
    let gw = cw * COLS as f32 + gap * (COLS - 1) as f32;
    let gh = ch * ROWS as f32 + gap * (ROWS - 1) as f32;
    let (gx, gy) = (((w - gw) * 0.5).round(), (top + (bottom - top - gh) * 0.5).round());

    let mut act = Action::None;
    for i in 0..COLS * ROWS {
        let (x, y) = (gx + (i % COLS) as f32 * (cw + gap), gy + (i / COLS) as f32 * (ch + gap));
        let a = ui.appear();
        let rise = ui.style(a, Vec2::new(0.0, ((1.0 - a) * 10.0 * s).round()));
        let hovered = ui.hit(x, y, cw, ch);
        let lift = ui.anim(i as u64 ^ 0x6a3e_5000, hovered);
        if i == 0 {
            soccer_card(ui, x, y - (lift * 3.0 * s).round(), cw, ch, lift);
        } else {
            empty_card(ui, x, y, cw, ch, lift);
        }
        if hovered {
            ui.set_tooltip(t("skin.soon"));
        }
        ui.restore(rise);
    }

    let (bw, bh) = ((76.0 * s).round(), (20.0 * s).round());
    if ui.button(&format!("<  {}", t("gui.back")), (12.0 * s).round(), h - bh - 12.0 * s, bw, bh, true) {
        act = Action::Back;
    }
    act
}

/// A place for a game to come: dark glass with a faint plus.
fn empty_card(ui: &mut Ui, x: f32, y: f32, w: f32, h: f32, hover: f32) {
    let s = ui.s;
    let r = 5.0 * s;
    ui.rect(x, y, w, h, lerp_color(rgba(255, 255, 255, 34), rgba(255, 255, 255, 60), hover), r);
    ui.rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, lerp_color(rgba(22, 22, 28, 205), rgba(32, 32, 40, 220), hover), r - 1.0);
    let (len, th) = ((14.0 * s).round(), (1.5 * s).round().max(1.0));
    let (cx, cy) = ((x + w * 0.5).round(), (y + h * 0.5).round());
    let c = lerp_color(rgba(255, 255, 255, 70), rgba(255, 255, 255, 120), hover);
    ui.solid(cx - len * 0.5, cy - th * 0.5, len, th, c);
    ui.solid(cx - th * 0.5, cy - len * 0.5, th, len, c);
}

/// The soccer game's card: a striped pitch seen from above, the ball on its center spot and
/// the name across the bottom.
fn soccer_card(ui: &mut Ui, x: f32, y: f32, w: f32, h: f32, hover: f32) {
    let s = ui.s;
    let r = 5.0 * s;
    // A glow under it while hovered.
    if hover > 0.01 {
        ui.rect_full(x, y + 4.0 * s, w, h, rgba(0, 0, 0, (120.0 * hover) as u8), rgba(0, 0, 0, (120.0 * hover) as u8), r, 10.0 * s);
    }
    ui.rect(x, y, w, h, lerp_color(rgba(200, 240, 190, 120), rgba(240, 255, 235, 230), hover), r);
    ui.rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, rgba(52, 118, 46, 255), r - 1.0);

    // The mown stripes.
    let inset = (3.0 * s).round();
    let (ix, iy, iw, ih) = (x + inset, y + inset, w - 2.0 * inset, h - 2.0 * inset);
    let n = 8;
    for k in 0..n {
        let x0 = (ix + iw * k as f32 / n as f32).round();
        let x1 = (ix + iw * (k + 1) as f32 / n as f32).round();
        let c = if k % 2 == 0 { rgba(64, 140, 56, 255) } else { rgba(56, 128, 50, 255) };
        ui.gradient(x0, iy, x1 - x0, ih, c, lerp_color(c, rgba(30, 80, 28, 255), 0.35));
    }

    // The lines (only the top end: the name covers the bottom one).
    let line = rgba(235, 245, 230, 190);
    let lt = (0.7 * s).round().max(1.0);
    let m = (7.0 * s).round();
    let (fx, fy, fw, fh) = (x + m, y + m, w - 2.0 * m, h - 2.0 * m);
    outline(ui, fx, fy, fw, fh, lt, line);
    let mid = (fy + fh * 0.5).round();
    ui.solid(fx, mid, fw, lt, line);
    let center = Vec2::new((fx + fw * 0.5).round(), mid);
    ui.ring(center, fw * 0.2, lt, line);
    let (bw, bh) = ((fw * 0.56).round(), (fh * 0.15).round());
    outline(ui, (center.x - bw * 0.5).round(), fy, bw, bh, lt, line);
    let (gw, ghh) = ((fw * 0.26).round(), (fh * 0.06).round());
    outline(ui, (center.x - gw * 0.5).round(), fy, gw, ghh, lt, line);

    // The ball, bobbing a little, turning while hovered.
    let br = (w * 0.17).round();
    let bob = ((ui.time * 2.4).sin() * 1.5 * s).round();
    let spin = ui.time * 0.25 + hover * 0.6;
    let shadow_w = br * 1.6;
    ui.rect_full(center.x - shadow_w * 0.5, center.y + br * 0.75, shadow_w, br * 0.4, rgba(0, 0, 0, 90), rgba(0, 0, 0, 90), br * 0.2, 3.0 * s);
    ball(ui, center + Vec2::new(0.0, bob - br * 0.15), br, spin);

    // The name across the bottom, over a dark fade.
    let name = t("minigames.soccer").to_uppercase();
    // (as large as fits across the card)
    let mut big = (s * 2.0).round();
    while big > 1.0 && ui.text_width(&name, big) > iw - 6.0 * s {
        big -= 1.0;
    }
    let ny = (y + h * 0.78).round();
    ui.gradient(ix, ny - 10.0 * s, iw, (y + h - inset) - (ny - 10.0 * s), rgba(10, 30, 10, 0), rgba(10, 30, 10, 170));
    ui.text_centered(&name, x + w * 0.5, ny, big, WHITE, true);
    let uw = (14.0 * s + hover * 14.0 * s).round();
    ui.rect(x + w * 0.5 - uw * 0.5, ny + 7.0 * big + 3.0 * s, uw, s.max(1.0), rgba(120, 230, 120, 255), s * 0.5);
}

/// A hairline rectangle outline.
fn outline(ui: &mut Ui, x: f32, y: f32, w: f32, h: f32, t: f32, c: Color) {
    ui.solid(x, y, w, t, c);
    ui.solid(x, y + h - t, w, t, c);
    ui.solid(x, y + t, t, h - 2.0 * t, c);
    ui.solid(x + w - t, y + t, t, h - 2.0 * t, c);
}

/// A soccer ball: white, shaded at the bottom, a dark pentagon in the middle joined by seams to
/// five around the edge.
fn ball(ui: &mut Ui, c: Vec2, r: f32, turn: f32) {
    let s = ui.s;
    let dark = rgba(28, 30, 40, 255);
    ui.rect_full(c.x - r - s, c.y - r - s, 2.0 * (r + s), 2.0 * (r + s), dark, dark, r + s, 0.0);
    ui.rect_full(c.x - r, c.y - r, 2.0 * r, 2.0 * r, rgba(255, 255, 255, 255), rgba(196, 200, 212, 255), r, 0.0);
    let corner = |k: usize, rad: f32, off: f32| {
        let a = turn + off + k as f32 * TAU / 5.0 - TAU / 4.0;
        c + Vec2::new(a.cos(), a.sin()) * rad
    };
    let seam = (0.8 * s).round().max(1.0);
    for k in 0..5 {
        let from = corner(k, r * 0.34, 0.0);
        let to = corner(k, r * 0.78, 0.0);
        let n = (to - from).perp().normalize_or_zero() * seam * 0.5;
        ui.quad([from - n, to - n, to + n, from + n], rgba(90, 94, 108, 255));
        pentagon(ui, corner(k, r * 0.8, 0.0), r * 0.18, turn + k as f32 * TAU / 5.0 + TAU / 2.0, dark);
    }
    pentagon(ui, c, r * 0.36, turn, dark);
    // A shine at the top left.
    ui.rect_full(c.x - r * 0.62, c.y - r * 0.7, r * 0.42, r * 0.26, rgba(255, 255, 255, 150), rgba(255, 255, 255, 0), r * 0.13, 1.5 * s);
}

fn pentagon(ui: &mut Ui, c: Vec2, r: f32, turn: f32, color: Color) {
    let p = |k: usize| {
        let a = turn + k as f32 * TAU / 5.0 - TAU / 4.0;
        c + Vec2::new(a.cos(), a.sin()) * r
    };
    for k in 1..4 {
        ui.quad([p(0), p(k), p(k + 1), p(k + 1)], color);
    }
}
