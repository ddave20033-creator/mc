//! The minigames screen: a big title, a grid of cards (one for each game in
//! `crate::minigames::ALL`, the rest empty places for games to come) and the back button.

use super::{backdrop, Action};
use crate::app::lang::t;
use crate::ui::*;
use glam::Vec2;
use crate::minigames::{Minigame, ALL};

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
        if let Some(game) = ALL.get(i) {
            game_card(ui, game, x, y - (lift * 3.0 * s).round(), cw, ch, lift);
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

/// A minigame's card: its picture in a frame lit with its accent when hovered, its name across
/// the bottom.
fn game_card(ui: &mut Ui, game: &Minigame, x: f32, y: f32, w: f32, h: f32, hover: f32) {
    let s = ui.s;
    let r = 5.0 * s;
    // A glow under it while hovered.
    if hover > 0.01 {
        let glow = rgba(0, 0, 0, (120.0 * hover) as u8);
        ui.rect_full(x, y + 4.0 * s, w, h, glow, glow, r, 10.0 * s);
    }
    let rest = lerp_color(game.accent, WHITE, 0.6);
    ui.rect(x, y, w, h, lerp_color(with_alpha(rest, 0.45), with_alpha(rest, 0.9), hover), r);
    (game.card)(ui, [x + 1.0, y + 1.0, w - 2.0, h - 2.0], hover);

    // The name across the bottom, over a dark fade, as large as fits across the card.
    let name = t(game.name).to_uppercase();
    let inset = (2.0 * s).round();
    let mut big = (s * 2.0).round();
    while big > 1.0 && ui.text_width(&name, big) > w - 2.0 * inset - 6.0 * s {
        big -= 1.0;
    }
    let ny = (y + h * 0.78).round();
    let fade_y = ny - 10.0 * s;
    ui.gradient(x + inset, fade_y, w - 2.0 * inset, y + h - inset - fade_y, rgba(8, 10, 12, 0), rgba(8, 10, 12, 170));
    ui.text_centered(&name, x + w * 0.5, ny, big, WHITE, true);
    let uw = (14.0 * s + hover * 14.0 * s).round();
    ui.rect(x + w * 0.5 - uw * 0.5, ny + 7.0 * big + 3.0 * s, uw, s.max(1.0), game.accent, s * 0.5);
}
