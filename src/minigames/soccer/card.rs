//! Soccer's card on the minigames screen: a striped pitch seen from above with the ball on its
//! center spot.

use crate::ui::*;
use glam::Vec2;
use std::f32::consts::TAU;

pub fn draw(ui: &mut Ui, [x, y, w, h]: [f32; 4], hover: f32) {
    let s = ui.s;
    ui.rect(x, y, w, h, rgba(52, 118, 46, 255), 4.0 * s);

    // The mown stripes.
    let inset = (2.0 * s).round();
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
    let m = (6.0 * s).round();
    let (fx, fy, fw, fh) = (x + m, y + m, w - 2.0 * m, h - 2.0 * m);
    outline(ui, fx, fy, fw, fh, lt, line);
    let mid = (fy + fh * 0.5).round();
    ui.solid(fx, mid, fw, lt, line);
    let center = Vec2::new((fx + fw * 0.5).round(), mid);
    ui.ring(center, fw * 0.2, lt, line);
    let (bw, bh) = ((fw * 0.56).round(), (fh * 0.15).round());
    outline(ui, (center.x - bw * 0.5).round(), fy, bw, bh, lt, line);
    let (gw, gh) = ((fw * 0.26).round(), (fh * 0.06).round());
    outline(ui, (center.x - gw * 0.5).round(), fy, gw, gh, lt, line);

    // The ball, bobbing a little, turning while hovered.
    let br = (w * 0.17).round();
    let bob = ((ui.time * 2.4).sin() * 1.5 * s).round();
    let spin = ui.time * 0.25 + hover * 0.6;
    let shadow_w = br * 1.6;
    let shadow = rgba(0, 0, 0, 90);
    ui.rect_full(center.x - shadow_w * 0.5, center.y + br * 0.75, shadow_w, br * 0.4, shadow, shadow, br * 0.2, 3.0 * s);
    ball(ui, center + Vec2::new(0.0, bob - br * 0.15), br, spin);
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
    let corner = |k: usize, rad: f32| {
        let a = turn + k as f32 * TAU / 5.0 - TAU / 4.0;
        c + Vec2::new(a.cos(), a.sin()) * rad
    };
    let seam = (0.8 * s).round().max(1.0);
    for k in 0..5 {
        let from = corner(k, r * 0.34);
        let to = corner(k, r * 0.78);
        let n = (to - from).perp().normalize_or_zero() * seam * 0.5;
        ui.quad([from - n, to - n, to + n, from + n], rgba(90, 94, 108, 255));
        pentagon(ui, corner(k, r * 0.8), r * 0.18, turn + k as f32 * TAU / 5.0 + TAU / 2.0, dark);
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
