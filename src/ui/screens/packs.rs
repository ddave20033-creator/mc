//! The resource pack screen: available packs on the left, active ones on the right.

use super::{backdrop, Action};
use crate::app::lang::t;
use crate::ui::*;

/// A pack on the resource pack screen.
pub struct PackEntry {
    /// Name in `resourcepacks/` (or `pack::BUILTIN`).
    pub name: String,
    pub title: String,
    pub description: String,
}

impl PackEntry {
    fn new(pack: &crate::pack::Pack) -> PackEntry {
        PackEntry {
            name: pack.name.clone(),
            title: pack.title().to_string(),
            description: pack.description.clone(),
        }
    }
}

/// The resource pack screen, like Minecraft's: the packs in `resourcepacks/` on the left, the
/// active ones on the right (highest priority on top) with the built-in pack always last.
#[derive(Default)]
pub struct PackScreen {
    pub available: Vec<PackEntry>,
    /// Enabled packs from `resourcepacks/`, highest priority first.
    pub selected: Vec<PackEntry>,
    builtin: Option<PackEntry>,
    /// Scroll of the two lists in pixels.
    scroll: [f32; 2],
    /// Seconds until the folder is looked at again (packs added or removed meanwhile).
    rescan: f32,
}

impl PackScreen {
    pub fn open(&mut self, enabled: &[String]) {
        *self = PackScreen {
            builtin: Some(PackEntry::new(&crate::pack::Pack::builtin())),
            ..Default::default()
        };
        let packs = crate::pack::list();
        for name in enabled.iter().filter(|n| packs.contains(n)) {
            if let Some(pack) = crate::pack::Pack::open(name) {
                self.selected.push(PackEntry::new(&pack));
            }
        }
        self.rescan_folder();
    }

    /// Adds new packs of the folder to the available ones and drops the removed ones.
    fn rescan_folder(&mut self) {
        let packs = crate::pack::list();
        self.available.retain(|e| packs.contains(&e.name));
        self.selected.retain(|e| packs.contains(&e.name));
        for name in packs {
            let known = self.available.iter().chain(&self.selected).any(|e| e.name == name);
            if !known {
                if let Some(pack) = crate::pack::Pack::open(&name) {
                    self.available.push(PackEntry::new(&pack));
                }
            }
        }
        self.available.sort_by_key(|e| e.title.to_lowercase());
        self.rescan = 1.0;
    }

    /// Names of the enabled packs, highest priority first (for the settings).
    pub fn enabled(&self) -> Vec<String> {
        self.selected.iter().map(|e| e.name.clone()).collect()
    }
}

/// What was clicked on a pack row.
#[derive(PartialEq)]
enum PackClick {
    None,
    Add,
    Remove,
    Up,
    Down,
}

/// Which list a pack row is in (and so what its icon's buttons do).
#[derive(Clone, Copy, PartialEq)]
enum PackRow {
    Available,
    Selected { first: bool, last: bool },
    Builtin,
}

/// Pixel-art triangle pointing right (0), left (1), up (2) or down (3), `n` pixels long.
fn arrow(ui: &mut Ui, cx: f32, cy: f32, n: usize, px: f32, dir: u8, c: Color) {
    for i in 0..n {
        let len = (2 * (n - i) - 1) as f32 * px;
        let off = ((i as f32 - n as f32 * 0.5) * px).round();
        let half = (len * 0.5).round();
        match dir {
            0 => ui.solid(cx + off, cy - half, px, len, c),
            1 => ui.solid(cx - off - px, cy - half, px, len, c),
            2 => ui.solid(cx - half, cy - off - px, len, px, c),
            _ => ui.solid(cx - half, cy + off, len, px, c),
        }
    }
}

/// One pack: an icon with the pack's initial, its name and up to two lines of description.
/// Hovering the icon shows its buttons, like in Minecraft: add (available packs), remove and
/// move up or down (active packs).
fn pack_row(ui: &mut Ui, e: &PackEntry, kind: PackRow, x: f32, y: f32, w: f32, h: f32) -> PackClick {
    let s = ui.s;
    let small = (s * 0.75).round().max(1.0);
    let hovered = ui.hit(x, y, w, h);
    if hovered {
        ui.rect(x, y, w, h, rgba(48, 54, 64, 225), 3.0 * s);
    }
    if kind == PackRow::Builtin {
        ui.rect(x, y, 2.0 * s, h, ACCENT, s);
    }

    // Icon: a tile in a colour of the name's hash with the first letter.
    let isz = h - 6.0 * s;
    let (ix, iy) = ((x + 3.0 * s).round(), (y + 3.0 * s).round());
    let hash = e.name.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    let hue = [
        [70, 130, 80],
        [150, 90, 60],
        [70, 100, 150],
        [130, 80, 140],
        [150, 130, 60],
        [60, 130, 130],
    ][hash as usize % 6];
    let top = rgba(hue[0], hue[1], hue[2], 255);
    let bottom = rgba(hue[0] / 2, hue[1] / 2, hue[2] / 2, 255);
    ui.rect_full(ix, iy, isz, isz, top, bottom, 2.0 * s, 0.0);
    let letter: String = e.title.chars().next().unwrap_or('?').to_uppercase().collect();
    ui.text_centered(&letter, ix + isz * 0.5, iy + (isz - 14.0 * s) * 0.5, 2.0 * s, WHITE, true);

    let mut click = PackClick::None;
    let icon_hovered = hovered && ui.hit(ix, iy, isz, isz);
    let px = s.round().max(1.0);
    let on = |hot: bool| if hot { WHITE } else { rgba(200, 200, 200, 230) };
    match kind {
        PackRow::Available if hovered => {
            // The whole row adds the pack; the icon shows the arrow.
            ui.rect(ix, iy, isz, isz, rgba(0, 0, 0, 130), 2.0 * s);
            arrow(ui, ix + isz * 0.5, iy + isz * 0.5, 6, px, 0, on(icon_hovered));
            ui.set_tooltip(t("packs.add"));
            if ui.pressed {
                click = PackClick::Add;
            }
        }
        PackRow::Selected { first, last } if hovered => {
            // Left half removes, right half moves up (top) or down (bottom).
            ui.rect(ix, iy, isz, isz, rgba(0, 0, 0, 130), 2.0 * s);
            let half = isz * 0.5;
            let remove = ui.hit(ix, iy, half, isz);
            let up = !first && ui.hit(ix + half, iy, half, half);
            let down = !last && ui.hit(ix + half, iy + half, half, half);
            arrow(ui, ix + half * 0.5, iy + half, 4, px, 1, on(remove));
            if !first {
                arrow(ui, ix + half * 1.5, iy + half * 0.5, 4, px, 2, on(up));
            }
            if !last {
                arrow(ui, ix + half * 1.5, iy + half * 1.5, 4, px, 3, on(down));
            }
            let (tip, action) = if remove {
                ("packs.remove", PackClick::Remove)
            } else if up {
                ("packs.up", PackClick::Up)
            } else if down {
                ("packs.down", PackClick::Down)
            } else {
                ("", PackClick::None)
            };
            if !tip.is_empty() {
                ui.set_tooltip(t(tip));
                if ui.pressed {
                    click = action;
                }
            }
        }
        PackRow::Builtin if hovered => ui.set_tooltip(t("packs.builtin_tip")),
        _ => {}
    }

    // Name and description.
    let tx = ix + isz + 5.0 * s;
    let tw = x + w - tx - 4.0 * s;
    let mut title = e.title.clone();
    while title.chars().count() > 1 && ui.text_width(&title, s) > tw {
        title.pop();
    }
    ui.text(&title, tx, y + 5.0 * s, s, WHITE, true);
    let lines = if kind == PackRow::Builtin {
        vec![t("packs.builtin").to_string()]
    } else {
        ui.wrap(&e.description, tw, small)
    };
    for (i, line) in lines.iter().take(2).enumerate() {
        ui.text(
            line,
            tx,
            y + 16.0 * s + i as f32 * 9.0 * small,
            small,
            rgba(170, 176, 186, 255),
            false,
        );
    }
    click
}

pub fn resource_packs(ui: &mut Ui, st: &mut PackScreen, in_game: bool) -> Action {
    let (w, h, s) = (ui.w, ui.h, ui.s);
    if in_game {
        ui.gradient(0.0, 0.0, w, h, rgba(0, 0, 0, 150), rgba(0, 0, 0, 120));
    } else {
        backdrop(ui, 1.4);
    }
    st.rescan -= ui.dt;
    if st.rescan <= 0.0 {
        st.rescan_folder();
    }
    ui.text_centered(t("packs.title"), w * 0.5, 10.0 * s, s, WHITE, true);
    let small = (s * 0.75).round().max(1.0);
    ui.text_centered(
        t("packs.hint"),
        w * 0.5,
        22.0 * s,
        small,
        rgba(160, 164, 176, 255),
        false,
    );

    let cw = (200.0 * s).min((w - 24.0 * s) * 0.5).round();
    let (top, bottom) = ((48.0 * s).round(), (h - 36.0 * s).round());
    let row_h = 36.0 * s;
    let xs = [(w * 0.5 - cw - 4.0 * s).round(), (w * 0.5 + 4.0 * s).round()];
    let heads = [t("packs.available"), t("packs.selected")];
    let mut clicks = Vec::new();
    for col in 0..2 {
        let x = xs[col];
        ui.text_centered(heads[col], x + cw * 0.5, top - 11.0 * s, s, rgba(220, 220, 226, 255), true);
        ui.rect(x, top, cw, bottom - top, rgba(0, 0, 0, 110), 3.0 * s);
        let count = if col == 0 {
            st.available.len()
        } else {
            st.selected.len() + 1
        };
        let max_scroll = (count as f32 * row_h - (bottom - top)).max(0.0);
        if ui.hit(x, top, cw, bottom - top) {
            st.scroll[col] -= ui.scroll * row_h * 0.5;
        }
        st.scroll[col] = st.scroll[col].clamp(0.0, max_scroll);
        ui.set_clip(Some([x, top, cw, bottom - top]));
        if col == 0 && st.available.is_empty() {
            let lines = ui.wrap(t("packs.empty"), cw - 16.0 * s, small);
            for (i, line) in lines.iter().enumerate() {
                ui.text_centered(
                    line,
                    x + cw * 0.5,
                    top + 12.0 * s + i as f32 * 9.0 * small,
                    small,
                    rgba(150, 150, 150, 255),
                    false,
                );
            }
        }
        for i in 0..count {
            let y = (top + i as f32 * row_h - st.scroll[col]).round();
            if y + row_h < top || y > bottom {
                continue;
            }
            let (entry, kind) = if col == 0 {
                (&st.available[i], PackRow::Available)
            } else if i < st.selected.len() {
                let last = i + 1 == st.selected.len();
                (&st.selected[i], PackRow::Selected { first: i == 0, last })
            } else {
                (st.builtin.as_ref().unwrap(), PackRow::Builtin)
            };
            let click = pack_row(ui, entry, kind, x + 2.0 * s, y + s, cw - 4.0 * s, row_h - 2.0 * s);
            if click != PackClick::None {
                clicks.push((i, click));
            }
        }
        ui.set_clip(None);
    }
    for (i, click) in clicks {
        match click {
            PackClick::Add => {
                let e = st.available.remove(i);
                st.selected.insert(0, e);
            }
            PackClick::Remove => {
                let e = st.selected.remove(i);
                st.available.push(e);
                st.available.sort_by_key(|e| e.title.to_lowercase());
            }
            PackClick::Up => st.selected.swap(i, i - 1),
            PackClick::Down => st.selected.swap(i, i + 1),
            PackClick::None => {}
        }
    }

    let (bw, bh) = (150.0 * s, 20.0 * s);
    let y = (h - 28.0 * s).round();
    let mut act = Action::None;
    if ui.button(t("packs.folder"), (w * 0.5 - 154.0 * s).round(), y, bw, bh, true) {
        act = Action::OpenPackFolder;
    }
    if ui.button(t("gui.done"), (w * 0.5 + 4.0 * s).round(), y, bw, bh, true) {
        act = Action::Back;
    }
    act
}
