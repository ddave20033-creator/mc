//! The options screen (in tabs) and the key binds screen.

use super::{backdrop, Action};
use crate::app::lang::{on_off, t};
use crate::app::settings::Settings;
use crate::ui::*;

/// What the options screen remembers between frames.
#[derive(Default)]
pub struct OptionsState {
    pub tab: usize,
    /// The key binds list, scrolled this far down (pixels).
    pub scroll: f32,
    /// The key bind (index in `keys::BINDS`) waiting for a key press.
    pub listening: Option<usize>,
}

/// Settings tabs.
const OPTION_TABS: [&str; 4] = ["opt.tab.graphics", "opt.tab.controls", "opt.tab.sound", "opt.tab.interface"];

/// Options, sorted into tabs. Each row: the setting's name on the left, its control on the
/// right; the hovered row's description shows at the bottom.
/// `max_msaa`: the most anti-aliasing samples the GPU supports.
pub fn options(
    ui: &mut Ui,
    st: &mut Settings,
    in_game: bool,
    os: &mut OptionsState,
    max_msaa: u32,
) -> Action {
    let (w, h, s) = (ui.w, ui.h, ui.s);
    if in_game {
        ui.gradient(0.0, 0.0, w, h, rgba(0, 0, 0, 150), rgba(0, 0, 0, 120));
    } else {
        backdrop(ui, 1.4);
    }

    let row_h = 22.0 * s;
    let (pw, ph) = (330.0 * s, 58.0 * s + 8.0 * row_h + 52.0 * s);
    let (px, py) = ((w * 0.5 - pw * 0.5).round(), ((h - ph) * 0.5).round());
    ui.panel(px, py, pw, ph);
    ui.text_centered(t("opt.title"), w * 0.5, py + 9.0 * s, s, WHITE, true);

    // Tabs (Q and E go to the one before and after).
    let step = ui.nav_tab();
    if step != 0 {
        os.tab = (os.tab as i32 + step).rem_euclid(OPTION_TABS.len() as i32) as usize;
    }
    let n = OPTION_TABS.len() as f32;
    let tab_w = ((pw - 24.0 * s - (n - 1.0) * 4.0 * s) / n).floor();
    let tab_y = py + 24.0 * s;
    let tab_h = 18.0 * s;
    for (i, key) in OPTION_TABS.iter().enumerate() {
        let x = (px + 12.0 * s + i as f32 * (tab_w + 4.0 * s)).round();
        let active = os.tab == i;
        let hovered = ui.hit(x, tab_y, tab_w, tab_h);
        let bg = if active {
            rgba(255, 255, 255, 40)
        } else if hovered {
            rgba(255, 255, 255, 22)
        } else {
            rgba(255, 255, 255, 8)
        };
        ui.rect(x, tab_y, tab_w, tab_h, bg, 3.0 * s);
        if active {
            let th = (s * 0.67).max(1.0).round();
            ui.rect(
                x + 4.0 * s,
                tab_y + tab_h - th - s,
                tab_w - 8.0 * s,
                th,
                ACCENT,
                th * 0.5,
            );
        }
        let color = if active {
            WHITE
        } else if hovered {
            HOVER_TEXT
        } else {
            rgba(170, 170, 178, 255)
        };
        let ty = tab_y + (tab_h - 7.0 * s) * 0.5;
        ui.text_centered(t(key), x + tab_w * 0.5, ty, s, color, true);
        if hovered && ui.pressed && !active {
            os.tab = i;
        }
    }
    let line = (s * 0.5).max(1.0);
    ui.solid(
        px + 12.0 * s,
        tab_y + tab_h + 4.0 * s,
        pw - 24.0 * s,
        line,
        rgba(255, 255, 255, 30),
    );

    let (cw, ch) = (150.0 * s, 20.0 * s);
    let cx = (px + pw - 12.0 * s - cw).round();
    let lx = (px + 16.0 * s).round();
    let list_top = (tab_y + tab_h + 10.0 * s).round();
    let row = |i: usize| (list_top + i as f32 * row_h).round();
    let mut hint: Option<&str> = None;
    // The name of row `i`; remembers its description while the mouse is over the row.
    let mut name = |ui: &mut Ui, i: usize, key: &'static str, desc: &'static str| {
        let y = row(i);
        if ui.hit(px, y - s, pw, row_h) {
            hint = Some(desc);
            let hl = rgba(255, 255, 255, 10);
            ui.rect(
                px + 8.0 * s,
                y - s,
                pw - 16.0 * s,
                ch + 2.0 * s,
                hl,
                3.0 * s,
            );
        }
        let color = rgba(220, 220, 226, 255);
        ui.text(t(key), lx, y + (ch - 7.0 * s) * 0.5, s, color, true);
        y
    };
    let mut act = Action::None;

    match os.tab {
        0 => {
            let y = name(ui, 0, "opt.l.render", "opt.d.render");
            let label = format!("{} chunk", st.render_distance.round() as i32);
            ui.slider(
                "rd",
                &label,
                &mut st.render_distance,
                4.0,
                64.0,
                cx,
                y,
                cw,
                ch,
            );
            st.render_distance = st.render_distance.round();

            let y = name(ui, 1, "opt.l.fov", "opt.d.fov");
            let label = match st.fov.round() as i32 {
                70 => t("opt.v.normal").to_string(),
                110 => "Quake Pro".to_string(),
                v => format!("{v}"),
            };
            ui.slider("fov", &label, &mut st.fov, 30.0, 110.0, cx, y, cw, ch);
            st.fov = st.fov.round();

            let y = name(ui, 2, "opt.l.shadows", "opt.d.shadows");
            if ui.button(on_off(st.shadows), cx, y, cw, ch, true) {
                st.shadows = !st.shadows;
            }
            let y = name(ui, 3, "opt.l.clouds", "opt.d.clouds");
            if ui.button(on_off(st.clouds), cx, y, cw, ch, true) {
                st.clouds = !st.clouds;
            }
            let y = name(ui, 4, "opt.l.fullscreen", "opt.d.fullscreen");
            if ui.button(on_off(st.fullscreen), cx, y, cw, ch, true) {
                act = Action::ToggleFullscreen;
            }
            // Max FPS in steps of 10; the far end is no limit.
            let y = name(ui, 5, "opt.l.fps_limit", "opt.d.fps_limit");
            let mut v = if st.fps_limit == 0 { 260.0 } else { st.fps_limit as f32 };
            let label = if v >= 255.0 {
                t("opt.v.unlimited").to_string()
            } else {
                format!("{} FPS", v.round() as i32)
            };
            ui.slider("fps", &label, &mut v, 30.0, 260.0, cx, y, cw, ch);
            let v = (v / 10.0).round() * 10.0;
            st.fps_limit = if v >= 260.0 { 0 } else { v as u32 };

            let y = name(ui, 6, "opt.l.aa", "opt.d.aa");
            // Always on: 2x, 4x, 8x (as far as the GPU goes).
            let shown = st.msaa.min(max_msaa);
            let label = if shown <= 1 {
                t("opt.off").to_string()
            } else {
                format!("{shown}x MSAA")
            };
            if ui.button(&label, cx, y, cw, ch, max_msaa > 2) {
                st.msaa = if shown >= max_msaa { 2 } else { shown * 2 };
                act = Action::AntialiasingChanged;
            }
            let y = name(ui, 7, "opt.l.packs", "opt.d.packs");
            if ui.button(t("opt.v.packs"), cx, y, cw, ch, true) {
                act = Action::ResourcePacks;
            }
        }
        1 => {
            let y = name(ui, 0, "opt.l.sens", "opt.d.sens");
            let label = format!("{}%", st.sensitivity.round() as i32);
            ui.slider(
                "sens",
                &label,
                &mut st.sensitivity,
                10.0,
                200.0,
                cx,
                y,
                cw,
                ch,
            );
            st.sensitivity = st.sensitivity.round();

            let y = name(ui, 1, "opt.l.bobbing", "opt.d.bobbing");
            if ui.button(on_off(st.view_bobbing), cx, y, cw, ch, true) {
                st.view_bobbing = !st.view_bobbing;
            }

            let y = name(ui, 2, "opt.l.keys", "opt.d.keys");
            if ui.button(t("opt.v.keys"), cx, y, cw, ch, true) {
                os.scroll = 0.0;
                os.listening = None;
                act = Action::KeyBinds;
            }
        }
        2 => {
            // Volumes, in steps of 5 percent.
            let sliders: [(&str, &'static str, &'static str, &mut f32); 3] = [
                ("vol", "opt.l.volume", "opt.d.volume", &mut st.volume),
                ("vol_weapons", "opt.l.volume_weapons", "opt.d.volume_weapons", &mut st.volume_weapons),
                ("vol_other", "opt.l.volume_other", "opt.d.volume_other", &mut st.volume_other),
            ];
            for (i, (id, key, desc, v)) in sliders.into_iter().enumerate() {
                let y = name(ui, i, key, desc);
                let label = if v.round() <= 0.0 {
                    t("opt.off").to_string()
                } else {
                    format!("{}%", v.round() as i32)
                };
                ui.slider(id, &label, v, 0.0, 100.0, cx, y, cw, ch);
                *v = (*v / 5.0).round() * 5.0;
            }
        }
        _ => {
            let y = name(ui, 0, "opt.l.language", "opt.d.language");
            if ui.button(t("opt.v.language"), cx, y, cw, ch, true) {
                act = Action::Language;
            }
            let y = name(ui, 1, "opt.l.gui", "opt.d.gui");
            let max_scale = Settings::max_gui_scale(w, h);
            let label = if st.gui_scale == 0 {
                t("opt.v.auto").to_string()
            } else {
                st.gui_scale.to_string()
            };
            if ui.button(&label, cx, y, cw, ch, true) {
                st.gui_scale = if st.gui_scale >= max_scale {
                    0
                } else {
                    st.gui_scale + 1
                };
            }
            let y = name(ui, 2, "opt.l.dark_ui", "opt.d.dark_ui");
            if ui.button(on_off(st.dark_ui), cx, y, cw, ch, true) {
                st.dark_ui = !st.dark_ui;
            }
            let y = name(ui, 3, "opt.l.fps", "opt.d.fps");
            if ui.button(on_off(st.show_fps), cx, y, cw, ch, true) {
                st.show_fps = !st.show_fps;
            }
            let y = name(ui, 4, "opt.l.fp_body", "opt.d.fp_body");
            if ui.button(on_off(st.first_person_body), cx, y, cw, ch, true) {
                st.first_person_body = !st.first_person_body;
            }
        }
    }

    // Description of the hovered setting.
    let hint_y = row(8) + 2.0 * s;
    ui.solid(
        px + 12.0 * s,
        hint_y - 3.0 * s,
        pw - 24.0 * s,
        line,
        rgba(255, 255, 255, 30),
    );
    if let Some(key) = hint {
        let fs = (s - 1.0).max(1.0);
        let color = rgba(180, 180, 190, 255);
        ui.text_centered(t(key), w * 0.5, hint_y + 2.0 * s, fs, color, true);
    }

    let dw = 200.0 * s;
    let done_x = (w * 0.5 - dw * 0.5).round();
    if ui.button(t("gui.done"), done_x, py + ph - 28.0 * s, dw, ch, true) {
        act = Action::Back;
    }
    act
}

/// Key binds (from the Controls tab): the actions in groups, each with its key and a button
/// putting that one back to its default. Click a key, then press the new one (Escape
/// cancels).
pub fn key_binds(ui: &mut Ui, st: &mut Settings, in_game: bool, os: &mut OptionsState) -> Action {
    use crate::app::keys::{display, BINDS, CATEGORIES};
    let (w, h, s) = (ui.w, ui.h, ui.s);
    if in_game {
        ui.gradient(0.0, 0.0, w, h, rgba(0, 0, 0, 150), rgba(0, 0, 0, 120));
    } else {
        backdrop(ui, 1.4);
    }
    ui.text_centered(t("keys.title"), w * 0.5, 10.0 * s, s, WHITE, true);

    let lw = (360.0 * s).min(w - 24.0 * s).round();
    let lx = ((w - lw) * 0.5).round();
    let (top, bottom) = ((28.0 * s).round(), (h - 50.0 * s).round());
    let row_h = (22.0 * s).round();
    let head_h = (26.0 * s).round();
    let (ch, kw, rw) = (20.0 * s, 120.0 * s, 46.0 * s);
    let rx = (lx + lw - 10.0 * s - rw).round();
    let kx = (rx - 4.0 * s - kw).round();

    ui.rect(lx, top, lw, bottom - top, rgba(0, 0, 0, 110), 3.0 * s);
    let content: f32 = CATEGORIES
        .iter()
        .map(|(_, binds)| head_h + binds.len() as f32 * row_h)
        .sum::<f32>()
        + 6.0 * s;
    let max_scroll = (content - (bottom - top)).max(0.0);
    if ui.hit(lx, top, lw, bottom - top) {
        os.scroll -= ui.scroll * row_h;
    }
    os.scroll = os.scroll.clamp(0.0, max_scroll);

    if ui.pressed {
        os.listening = None;
    }
    let defaults = crate::app::keys::KeyMap::default();
    let mut hint: Option<String> = None;
    ui.set_clip(Some([lx, top, lw, bottom - top]));
    let mut y = (top + 4.0 * s - os.scroll).round();
    for (title, binds) in CATEGORIES {
        // Group heading with a line under it.
        if y + head_h > top && y < bottom {
            ui.text(t(title), lx + 10.0 * s, y + 9.0 * s, s, ACCENT, true);
            ui.solid(
                lx + 10.0 * s,
                y + 19.0 * s,
                lw - 20.0 * s,
                (s * 0.5).max(1.0),
                rgba(255, 255, 255, 40),
            );
        }
        y += head_h;
        for &bind in binds.iter() {
            let i = bind as usize;
            let ry = y;
            y += row_h;
            if ry + row_h < top || ry > bottom {
                continue;
            }
            let conflict = st.keys.conflicts(i);
            if ui.hit(lx, ry - s, lw, row_h) {
                ui.rect(lx + 4.0 * s, ry - s, lw - 8.0 * s, ch + 2.0 * s, rgba(255, 255, 255, 10), 3.0 * s);
                hint = Some(if conflict {
                    let others: Vec<&str> = (0..BINDS.len())
                        .filter(|&j| j != i && st.keys.0[j] == st.keys.0[i])
                        .map(|j| t(key_label(BINDS[j].1)))
                        .collect();
                    format!("{} {}", t("keys.conflict"), others.join(", "))
                } else {
                    t("opt.d.key").to_string()
                });
            }
            let name_color = if conflict { rgba(255, 110, 100, 255) } else { rgba(220, 220, 226, 255) };
            ui.text(t(key_label(BINDS[i].1)), lx + 16.0 * s, ry + (ch - 7.0 * s) * 0.5, s, name_color, true);
            let listening = os.listening == Some(i);
            let label = if listening {
                format!("> {} <", t("opt.v.press_key"))
            } else {
                display(st.keys.0[i])
            };
            if ui.button(&label, kx, ry, kw, ch, true) {
                os.listening = Some(i);
            }
            if listening {
                ui.rect(kx, ry, kw, ch, rgba(255, 220, 90, 40), 3.0 * s);
            } else if conflict {
                ui.rect(kx, ry, kw, ch, rgba(255, 60, 60, 70), 3.0 * s);
            }
            let changed = st.keys.0[i] != defaults.0[i];
            if ui.button(t("keys.reset"), rx, ry, rw, ch, changed) {
                st.keys.0[i] = defaults.0[i];
                os.listening = None;
            }
        }
    }
    ui.set_clip(None);

    // Scroll bar
    if max_scroll > 0.0 {
        let list_h = bottom - top;
        let bar_h = (list_h * list_h / (list_h + max_scroll)).max(12.0 * s);
        let bar_y = top + (list_h - bar_h) * os.scroll / max_scroll;
        let bx = (lx + lw - 5.0 * s).round();
        ui.rect(bx, bar_y, 2.0 * s, bar_h, rgba(255, 255, 255, 90), s);
    }

    // The hovered row's hint, or a reminder of how it works.
    let fs = (s - 1.0).max(1.0);
    let hint = hint.unwrap_or_else(|| t("opt.d.key").to_string());
    ui.text_centered(&hint, w * 0.5, bottom + 6.0 * s, fs, rgba(180, 180, 190, 255), true);

    let (bw, bh) = (150.0 * s, 20.0 * s);
    let by = (h - 28.0 * s).round();
    let mut act = Action::None;
    if ui.button(t("keys.reset_all"), (w * 0.5 - 154.0 * s).round(), by, bw, bh, st.keys != defaults) {
        st.keys = defaults;
        os.listening = None;
    }
    if ui.button(t("gui.done"), (w * 0.5 + 4.0 * s).round(), by, bw, bh, true) {
        act = Action::Back;
    }
    act
}

/// Translation key of a key bind's name.
fn key_label(id: &str) -> &'static str {
    match id {
        "forward" => "key.forward",
        "back" => "key.back",
        "left" => "key.left",
        "right" => "key.right",
        "jump" => "key.jump",
        "sneak" => "key.sneak",
        "sprint" => "key.sprint",
        "zoom" => "key.zoom",
        "inventory" => "key.inventory",
        "drop" => "key.drop",
        "reload" => "key.reload",
        "inspect" => "key.inspect",
        "gunlight" => "key.gunlight",
        "chat" => "key.chat",
        "command" => "key.command",
        "playerlist" => "key.playerlist",
        "fly" => "key.fly",
        "perspective" => "key.perspective",
        "hidehud" => "key.hidehud",
        "debug" => "key.debug",
        "fullscreen" => "key.fullscreen",
        "hotbar1" => "key.hotbar1",
        "hotbar2" => "key.hotbar2",
        "hotbar3" => "key.hotbar3",
        "hotbar4" => "key.hotbar4",
        "hotbar5" => "key.hotbar5",
        "hotbar6" => "key.hotbar6",
        "hotbar7" => "key.hotbar7",
        "hotbar8" => "key.hotbar8",
        "hotbar9" => "key.hotbar9",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_bind_has_its_own_name() {
        for (_, id, _) in crate::app::keys::BINDS {
            assert_eq!(key_label(id), format!("key.{id}"));
        }
    }
}
