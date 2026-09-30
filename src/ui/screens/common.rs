//! The death, pause and loading screens, and the pieces other screens are built from: the
//! menu backdrop, a title bar, card titles and the bottom action bar.

use super::Action;
use crate::lang::t;
use crate::ui::*;
use glam::Vec2;

pub fn death(ui: &mut Ui, message: &str) -> Action {
    let (w, h, s) = (ui.w, ui.h, ui.s);
    // A red wash that breathes slowly.
    let beat = 0.5 + 0.5 * (ui.time * 2.2).sin();
    ui.gradient(0.0, 0.0, w, h, rgba(110, 8, 8, 120), rgba(40, 0, 0, 190));
    ui.vignette(rgba(60, 0, 0, (150.0 + 50.0 * beat) as u8));
    let (pw, ph) = (240.0 * s, 118.0 * s);
    let (px, py) = ((w * 0.5 - pw * 0.5).round(), ((h - ph) * 0.45).round());
    ui.panel(px, py, pw, ph);
    let big = (s * 2.0).round();
    ui.text_centered(t("death.title"), w * 0.5, py + 14.0 * s, big, rgba(255, 120, 110, 255), true);
    ui.rect(w * 0.5 - 14.0 * s, py + 32.0 * s, 28.0 * s, s, DANGER, s * 0.5);
    for (i, line) in ui.wrap(message, pw - 24.0 * s, s).iter().enumerate().take(2) {
        ui.text_centered(line, w * 0.5, py + 40.0 * s + i as f32 * 10.0 * s, s, rgba(222, 214, 214, 255), false);
    }
    let (bw, bh) = (pw - 28.0 * s, 22.0 * s);
    let x = (px + 14.0 * s).round();
    let mut act = Action::None;
    if ui.button_primary(t("death.respawn"), x, py + ph - 58.0 * s, bw, bh, true) {
        act = Action::Respawn;
    }
    if ui.button(t("death.title_screen"), x, py + ph - 31.0 * s, bw, bh, true) {
        act = Action::ToTitle;
    }
    act
}

/// Darkened panorama backdrop used behind menus.
pub fn backdrop(ui: &mut Ui, strength: f32) {
    let (w, h) = (ui.w, ui.h);
    let a = |v: f32| (v * strength).clamp(0.0, 255.0) as u8;
    ui.gradient(
        0.0,
        0.0,
        w,
        h * 0.5,
        rgba(10, 10, 14, a(140.0)),
        rgba(10, 10, 14, a(30.0)),
    );
    ui.gradient(
        0.0,
        h * 0.5,
        w,
        h * 0.5 + 1.0,
        rgba(10, 10, 14, a(30.0)),
        rgba(10, 10, 14, a(170.0)),
    );
    ui.vignette(rgba(0, 0, 0, a(160.0)));
}

/// A screen's title bar: a dark band along the top, the title at the left of the content
/// (`x`, `w` wide) with a small accent line under it, and `note` in a chip at its right.
pub fn screen_header(ui: &mut Ui, title: &str, note: &str, x: f32, w: f32) {
    let (s, sw) = (ui.s, ui.w);
    let bh = (34.0 * s).round();
    ui.gradient(0.0, 0.0, sw, bh, rgba(12, 12, 16, 210), rgba(12, 12, 16, 150));
    ui.hgradient(0.0, bh, sw * 0.5, 1.0, rgba(255, 255, 255, 0), rgba(255, 255, 255, 30));
    ui.hgradient(sw * 0.5, bh, sw * 0.5, 1.0, rgba(255, 255, 255, 30), rgba(255, 255, 255, 0));
    let big = (s * 1.5).round();
    ui.text(title, x, (bh - 7.0 * big) * 0.5 - s, big, WHITE, true);
    ui.rect(x, bh - 7.0 * s, 20.0 * s, s, ACCENT, s * 0.5);
    if !note.is_empty() {
        let nw = ui.text_width(note, s) + 8.0 * s;
        ui.chip(note, x + w - nw, (bh - 11.0 * s) * 0.5, rgba(170, 176, 196, 255));
    }
}

/// A card's title centered on `cx`: large, unless that would not fit in `max_w`.
pub fn card_title(ui: &mut Ui, text: &str, cx: f32, y: f32, max_w: f32) {
    let s = ui.s;
    let big = (s * 1.5).round();
    let size = if ui.text_width(text, big) <= max_w { big } else { s };
    ui.text_centered(text, cx, y + (big - size) * 3.5, size, WHITE, true);
}

/// A dark band along the bottom of the screen from `y`, for a screen's buttons.
pub fn action_bar(ui: &mut Ui, y: f32) {
    let (w, h) = (ui.w, ui.h);
    ui.gradient(0.0, y, w, h - y, rgba(12, 12, 16, 150), rgba(12, 12, 16, 215));
    ui.hgradient(0.0, y, w * 0.5, 1.0, rgba(255, 255, 255, 0), rgba(255, 255, 255, 30));
    ui.hgradient(w * 0.5, y, w * 0.5, 1.0, rgba(255, 255, 255, 30), rgba(255, 255, 255, 0));
}

/// What the pause menu offers for LAN play.
pub enum PauseLan<'a> {
    /// Single player: can be opened to the LAN.
    Available,
    /// Hosting at this address.
    Open(&'a str),
    /// Joined someone else's game.
    Joined,
}

pub fn pause(ui: &mut Ui, lan: PauseLan) -> Action {
    let (w, h, s) = (ui.w, ui.h, ui.s);
    ui.gradient(0.0, 0.0, w, h, rgba(10, 10, 14, 150), rgba(10, 10, 14, 120));
    ui.vignette(rgba(0, 0, 0, 140));
    let rows = match lan {
        PauseLan::Joined => 3.0,
        _ => 4.0,
    };
    let (bh, gap) = ((22.0 * s).round(), (27.0 * s).round());
    let pw = (240.0 * s).round();
    let ph = (44.0 * s + rows * gap + 8.0 * s).round();
    let (px, py) = ((w * 0.5 - pw * 0.5).round(), ((h - ph) * 0.42).round());
    // The card comes up into place.
    let a = ui.appear();
    let rise = ui.style(1.0, Vec2::new(0.0, ((1.0 - a) * 14.0 * s).round()));
    ui.panel(px, py, pw, ph);
    let big = (s * 1.5).round();
    ui.text_centered(t("pause.title"), w * 0.5, py + 13.0 * s, big, WHITE, true);
    ui.rect(w * 0.5 - 14.0 * s, py + 28.0 * s, 28.0 * s, s, ACCENT, s * 0.5);
    ui.restore(rise);
    let (x, bw) = ((px + 14.0 * s).round(), pw - 28.0 * s);
    let y = (py + 40.0 * s).round();
    let mut act = Action::None;
    if ui.button_primary(t("pause.resume"), x, y, bw, bh, true) {
        act = Action::Resume;
    }
    let mut row = y + gap;
    match lan {
        PauseLan::Available => {
            if ui.button(t("pause.lan"), x, row, bw, bh, true) {
                act = Action::OpenLan;
            }
            row += gap;
        }
        PauseLan::Open(addr) => {
            let label = format!("{} {addr}", t("pause.lan_open"));
            ui.button(&label, x, row, bw, bh, false);
            row += gap;
        }
        PauseLan::Joined => {}
    }
    if ui.button(t("menu.options"), x, row, bw, bh, true) {
        act = Action::Options;
    }
    let quit = if matches!(lan, PauseLan::Joined) {
        t("pause.disconnect")
    } else {
        t("pause.quit")
    };
    if ui.button_ex(quit, x, row + gap, bw, bh, true, ButtonKind::Danger) {
        act = Action::ToTitle;
    }
    act
}

/// Generating the world: the logo, a bar filling up with a light running along it, and a tip
/// that changes every few seconds.
pub fn loading(ui: &mut Ui, progress: f32) {
    let (w, h, s) = (ui.w, ui.h, ui.s);
    backdrop(ui, 1.6);
    let lw = (220.0 * s).round();
    let ly = (h * 0.26).round();
    ui.logo(crate::world::textures::tex::LOGO, w * 0.5, ly, lw);
    let cy = (ly + lw / 8.0 + 30.0 * s).round();
    ui.text_centered(t("loading.generating"), w * 0.5, cy - 16.0 * s, s, WHITE, true);
    let (bw, bh) = ((220.0 * s).round(), (4.0 * s).round());
    let x = (w * 0.5 - bw * 0.5).round();
    ui.rect_full(x, cy + s, bw, bh, rgba(0, 0, 0, 80), rgba(0, 0, 0, 80), bh * 0.5, 4.0 * s);
    ui.rect(x, cy, bw, bh, rgba(255, 255, 255, 26), bh * 0.5);
    let p = progress.clamp(0.0, 1.0);
    if p > 0.0 {
        let fw = (bw * p).max(bh);
        ui.rect(x, cy, fw, bh, ACCENT, bh * 0.5);
        // A light running along the filled part.
        let run = (ui.time * 0.7).fract();
        let lw = (30.0 * s).min(fw);
        let lx = x + (fw - lw) * run;
        ui.rect_full(lx, cy + s, lw, bh - 2.0 * s, rgba(255, 255, 255, 70), rgba(255, 255, 255, 20), bh * 0.5, 2.0 * s);
    }
    let pct = format!("{}%", (p * 100.0).round() as i32);
    ui.text_centered(&pct, w * 0.5, cy + bh + 8.0 * s, s, rgba(220, 222, 230, 255), true);
    let tips = [
        t("tip.1"),
        t("tip.2"),
        t("tip.3"),
        t("tip.4"),
        t("tip.5"),
        t("tip.6"),
    ];
    // The tip fades out and the next one in.
    let k = ui.time / 4.0;
    let tip = tips[k as usize % tips.len()];
    let f = k.fract();
    let fade = (f * 6.0).min((1.0 - f) * 6.0).clamp(0.0, 1.0);
    let tw = ui.text_width(tip, s);
    let (tpw, tph) = ((tw + 20.0 * s).round(), (18.0 * s).round());
    let (tx, ty) = ((w * 0.5 - tpw * 0.5).round(), (h - 34.0 * s).round());
    let old = ui.style(fade, Vec2::ZERO);
    ui.rect(tx, ty, tpw, tph, rgba(18, 18, 24, 200), tph * 0.5);
    ui.text(tip, tx + 10.0 * s, ty + (tph - 7.0 * s) * 0.5, s, rgba(200, 204, 218, 255), false);
    ui.restore(old);
}
