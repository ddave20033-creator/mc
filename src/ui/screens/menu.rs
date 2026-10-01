//! The title screen with the turnable player preview, the skin screen and the credits.

use super::{backdrop, Action};
use crate::app::lang::t;
use crate::ui::*;
use glam::{Vec2, Vec3};

#[derive(Default)]
pub struct PreviewRotation {
    pub angle: f32,
    dragging: bool,
    last_x: f32,
}

/// The title screen: a dark sidebar on the left (sliding in) with the logo, the menu and the
/// version; the blurred world on the right, the player's character standing in it, with the
/// skin button under it.
pub fn main_menu(ui: &mut Ui, skin: u8, preview: &mut PreviewRotation) -> Action {
    let (w, h, s) = (ui.w, ui.h, ui.s);
    backdrop(ui, 0.6);

    // The sidebar.
    let sw = (w * 0.3).clamp(220.0 * s, 270.0 * s).min(w - 16.0 * s).round();
    let a = ui.appear();
    let slide = ui.style(1.0, Vec2::new(-((1.0 - a) * 24.0 * s).round(), 0.0));
    ui.solid(0.0, 0.0, sw, h, rgba(12, 12, 16, 214));
    ui.hgradient(sw, 0.0, 40.0 * s, h, rgba(11, 11, 26, 205), rgba(12, 12, 16, 0));

    let (bx, bw, bh) = ((20.0 * s).round(), (sw - 40.0 * s).round(), (22.0 * s).round());
    let logo_y = (h * 0.16).round().max(16.0 * s);
    ui.logo(crate::textures::tex::LOGO, sw * 0.5, logo_y, bw);

    // The menu.
    let gap = (27.0 * s).round();
    let y0 = (logo_y + bw / 8.0 + 34.0 * s).max(h * 0.36).round();
    let mut act = Action::None;
    if ui.button_primary(t("menu.singleplayer"), bx, y0, bw, bh, true) {
        act = Action::Singleplayer;
    }
    if ui.button(t("menu.multiplayer"), bx, y0 + gap, bw, bh, true) {
        act = Action::Multiplayer;
    }
    if ui.button(t("menu.options"), bx, y0 + 2.0 * gap, bw, bh, true) {
        act = Action::Options;
    }
    if ui.button(t("menu.credits"), bx, y0 + 3.0 * gap, bw, bh, true) {
        act = Action::Credits;
    }
    if ui.button_ex(t("menu.quit"), bx, y0 + 4.0 * gap + 8.0 * s, bw, bh, true, ButtonKind::Danger) {
        act = Action::Quit;
    }
    ui.text(VERSION, bx, h - 14.0 * s, s, rgba(120, 124, 140, 190), false);
    ui.restore(slide);

    // The character, in the space to the right of the sidebar.
    let area = (sw + 16.0 * s, w - 16.0 * s);
    let pw = (area.1 - area.0).min(150.0 * s);
    if pw >= 70.0 * s {
        let ph = (h * 0.55).min(210.0 * s);
        let px = ((area.0 + area.1) * 0.5 - pw * 0.5).round();
        let py = ((h - ph) * 0.5 - 6.0 * s).round();
        let a = ui.appear();
        let rise = ui.style(a, Vec2::new(0.0, ((1.0 - a) * 10.0 * s).round()));
        // A soft shadow under the feet.
        let (gx, gy, gw) = (px + pw * 0.28, py + ph * 0.87, pw * 0.44);
        ui.rect_full(gx, gy, gw, 5.0 * s, rgba(0, 0, 0, 110), rgba(0, 0, 0, 70), 3.0 * s, 5.0 * s);
        draw_menu_player(ui, [px, py, pw, ph], skin, preview);
        let (sbw, sbh) = ((90.0 * s).round(), (20.0 * s).round());
        if ui.button(t("menu.skin"), (px + pw * 0.5 - sbw * 0.5).round(), (py + ph + 6.0 * s).round(), sbw, sbh, true) {
            act = Action::SkinMenu;
        }
        ui.restore(rise);
    }
    act
}

/// Projects the same textured model used in the world into the menu's UI layer.
/// Painter sorting and backface removal keep it clean without touching the world depth buffer.
fn draw_menu_player(ui: &mut Ui, rect: [f32; 4], skin: u8, preview: &mut PreviewRotation) {
    use crate::model::players::player::{build_player, limb_targets, PlayerPose};
    use crate::world::mesh::Vertex;
    use std::f32::consts::PI;

    let [x, y, w, h] = rect;
    let scale = (w / 1.35).min(h / 2.2);
    let center = Vec2::new(x + w * 0.5, y + h * 0.52);
    // A drag starts on the character itself, not anywhere in its surrounding panel.
    let body_hit = ui.hit(
        center.x - 0.48 * scale,
        center.y - 1.0 * scale,
        0.96 * scale,
        1.9 * scale,
    );
    if ui.pressed && body_hit {
        preview.dragging = true;
        preview.last_x = ui.mouse.x;
    }
    if !ui.mouse_down {
        preview.dragging = false;
    }
    if preview.dragging && ui.mouse_down {
        preview.angle =
            (preview.angle + (ui.mouse.x - preview.last_x) * 0.012).rem_euclid(2.0 * PI);
        preview.last_x = ui.mouse.x;
    }
    let turn = preview.angle - 0.25;
    let pose = PlayerPose {
        pos: Vec3::ZERO,
        body_yaw: -PI / 2.0 + turn,
        head_yaw: -PI / 2.0 + turn,
        pitch: 0.0,
        limb_swing: ui.time * 2.1,
        limb_amount: 0.08,
        sprint: 0.0,
        attack: 0.0,
        crouch: 0.0,
        held: crate::item::NONE,
        skin,
        book: None,
        time: ui.time,
        hurt: false,
        first_person: false,
        burning: false,
        blocking: false,
        hide_arms: false,
        hide_right_arm: false,
        lantern: None,
        gun_mods: 0,
        gun_dirt: 0,
        held_data: 0,
        gun: Default::default(),
        armor: 0,
        grenade: None,
        rod: None,
        chop: None,
    };
    let mut model = Vec::new();
    build_player(&mut model, &mut Vec::new(), &pose, &limb_targets(&pose), 15, 15);
    let camera = Vec3::new(0.0, 1.2, -5.0);
    let mut faces: Vec<(f32, [UiVertex; 3])> = Vec::new();
    for tri in model.chunks_exact(3) {
        let a = Vec3::from(tri[0].pos);
        let b = Vec3::from(tri[1].pos);
        let c = Vec3::from(tri[2].pos);
        let normal = (b - a).cross(c - a);
        let mid = (a + b + c) / 3.0;
        if normal.dot(camera - mid) <= 0.0 {
            continue;
        }
        let verts = std::array::from_fn(|i| {
            let v: &Vertex = &tri[i];
            let p = Vec3::from(v.pos);
            let shade = (0.77
                + normal
                    .normalize_or_zero()
                    .dot(Vec3::new(-0.35, 0.7, -0.6))
                    .max(0.0)
                    * 0.23)
                .min(1.0);
            UiVertex {
                pos: [
                    center.x + p.x * scale + ui.offset.x,
                    center.y - (p.y - 0.9) * scale + p.z * scale * 0.06 + ui.offset.y,
                ],
                uv: v.uv,
                color: [1.0, 1.0, 1.0, ui.fade],
                rect: [v.layer, shade, 0.0, 0.0],
                mode: 3.0,
            }
        });
        faces.push((mid.z, verts));
    }
    faces.sort_by(|a, b| b.0.total_cmp(&a.0));
    ui.set_clip(Some(rect));
    for (_, vertices) in faces {
        ui.verts.extend(vertices);
    }
    ui.set_clip(None);
}

pub fn skin_menu(ui: &mut Ui, selected: u8, custom_available: bool, error: &str) -> Action {
    let (w, h, s) = (ui.w, ui.h, ui.s);
    backdrop(ui, 1.1);
    let (pw, ph) = (236.0 * s, 188.0 * s);
    let (x, y) = ((w - pw) * 0.5, (h - ph) * 0.5);
    ui.panel(x, y, pw, ph);
    ui.text_centered(t("menu.skin"), w * 0.5, y + 11.0 * s, 1.3 * s, WHITE, true);
    if !error.is_empty() {
        ui.text_centered(
            error,
            w * 0.5,
            y + 25.0 * s,
            0.65 * s,
            rgba(255, 135, 125, 255),
            true,
        );
    }
    let names = [
        t("skin.classic"),
        t("skin.forest"),
        t("skin.red"),
        t("skin.night"),
    ];
    let mut action = Action::None;
    for (i, name) in names.iter().enumerate() {
        let label = if selected == i as u8 {
            format!("> {name}")
        } else {
            name.to_string()
        };
        if ui.button(
            &label,
            x + 14.0 * s,
            y + (35.0 + i as f32 * 24.0) * s,
            pw - 28.0 * s,
            20.0 * s,
            true,
        ) {
            action = Action::SelectSkin(i as u8);
        }
    }
    let custom = if selected == 4 {
        format!("> {}", t("skin.custom"))
    } else {
        t("skin.custom").to_string()
    };
    if ui.button(
        &custom,
        x + 14.0 * s,
        y + 132.0 * s,
        (pw - 32.0 * s) * 0.5,
        20.0 * s,
        custom_available,
    ) {
        action = Action::SelectSkin(4);
    }
    if ui.button(
        t("skin.upload"),
        x + 18.0 * s + (pw - 32.0 * s) * 0.5,
        y + 132.0 * s,
        (pw - 32.0 * s) * 0.5,
        20.0 * s,
        true,
    ) {
        action = Action::UploadSkin;
    }
    if ui.button(
        t("gui.back"),
        x + 14.0 * s,
        y + 160.0 * s,
        pw - 28.0 * s,
        20.0 * s,
        true,
    ) {
        action = Action::Back;
    }
    action
}

pub const FAITHFUL_URL: &str = "https://faithfulpack.net";

/// Credits: the game, the resource pack in use (`pack` = title, description) with a link,
/// and the disclaimer. Laid out twice: once to measure the panel, once to draw.
pub fn credits(ui: &mut Ui, pack: Option<(&str, &str)>) -> Action {
    let (w, h, s) = (ui.w, ui.h, ui.s);
    backdrop(ui, 1.4);
    let small = (s * 0.75).round().max(1.0);
    let pw = 290.0 * s;
    let px = (w * 0.5 - pw * 0.5).round();
    let cx = w * 0.5;
    let gold = rgba(255, 206, 120, 255);
    let soft = rgba(205, 208, 220, 255);
    let muted = rgba(140, 143, 158, 255);
    let faithful = pack.is_some_and(|(title, _)| title.to_lowercase().contains("faithful"));
    let desc = pack
        .map(|(_, d)| ui.wrap(d, pw - 48.0 * s, small))
        .unwrap_or_default();
    let legal = ui.wrap(
        &format!("{} {}", t("credits.4"), t("credits.5")),
        pw - 32.0 * s,
        small,
    );
    let about = if pack.is_some() {
        t("credits.3_pack")
    } else {
        t("credits.3")
    };

    let mut act = Action::None;
    let mut top = 0.0;
    for pass in 0..2 {
        let draw = pass == 1;
        let mut y = top + 14.0 * s;
        let divider = |ui: &mut Ui, y: &mut f32| {
            *y += 7.0 * s;
            if draw {
                ui.solid(
                    px + 20.0 * s,
                    y.round(),
                    pw - 40.0 * s,
                    (s * 0.5).max(1.0),
                    rgba(255, 255, 255, 28),
                );
            }
            *y += 8.0 * s;
        };

        // Title
        if draw {
            ui.text_centered("Your Worlds", cx, y, 2.0 * s, gold, true);
        }
        y += 20.0 * s;
        if draw {
            ui.text_centered(t("credits.1"), cx, y, s, WHITE, true);
        }
        y += 10.0 * s;
        divider(ui, &mut y);

        // The game
        if draw {
            ui.text_centered(t("credits.2"), cx, y, s, soft, true);
            ui.text_centered(about, cx, y + 11.0 * s, s, soft, true);
        }
        y += 21.0 * s;

        // Resource pack card
        if let Some((title, _)) = pack {
            divider(ui, &mut y);
            if draw {
                ui.text_centered(t("credits.textures"), cx, y, small, muted, false);
            }
            y += 9.0 * small + 3.0 * s;
            let card_h = 8.0 * s
                + 11.0 * s
                + desc.len() as f32 * 9.0 * small
                + if faithful { 4.0 * s + 9.0 * small } else { 0.0 }
                + 7.0 * s;
            let (cx0, cw) = (px + 16.0 * s, pw - 32.0 * s);
            if draw {
                ui.rect(cx0, y, cw, card_h, rgba(255, 255, 255, 12), 4.0 * s);
                ui.rect(cx0, y, 2.0 * s, card_h, gold, s);
            }
            let mut cy = y + 8.0 * s;
            if draw {
                ui.text_centered(title, cx, cy, s, gold, true);
            }
            cy += 11.0 * s;
            for line in &desc {
                if draw {
                    ui.text_centered(line, cx, cy, small, soft, false);
                }
                cy += 9.0 * small;
            }
            if faithful {
                cy += 4.0 * s;
                let lw = ui.text_width(FAITHFUL_URL, small);
                let (lx, lh) = ((cx - lw * 0.5).round(), 8.0 * small);
                let hovered = ui.hit(lx, cy - small, lw, lh + 2.0 * small);
                if draw {
                    let c = if hovered {
                        rgba(190, 228, 255, 255)
                    } else {
                        rgba(120, 190, 255, 255)
                    };
                    ui.text(FAITHFUL_URL, lx, cy, small, c, false);
                    if hovered {
                        ui.solid(lx, cy + 7.5 * small, lw, small.max(1.0), c);
                        ui.set_tooltip(t("credits.open_link"));
                    }
                    if hovered && ui.pressed {
                        act = Action::OpenLink(FAITHFUL_URL);
                    }
                }
            }
            y += card_h;
        }
        divider(ui, &mut y);

        // Disclaimer
        for line in &legal {
            if draw {
                ui.text_centered(line, cx, y, small, muted, false);
            }
            y += 9.0 * small;
        }
        y += 8.0 * s;

        let bw = 150.0 * s;
        if draw
            && ui.button(
                t("gui.done"),
                (cx - bw * 0.5).round(),
                y.round(),
                bw,
                20.0 * s,
                true,
            )
        {
            act = Action::Back;
        }
        y += 20.0 * s + 12.0 * s;

        if !draw {
            let total = y - top;
            top = ((h - total) * 0.5).max(4.0 * s).round();
            ui.panel(px, top, pw, total);
        }
    }
    act
}

#[cfg(test)]
mod menu_player_tests {
    use super::*;

    #[test]
    fn preview_rotates_only_while_dragging_the_character() {
        let mut ui = Ui::new();
        let mut preview = PreviewRotation::default();
        // (long after the menu came in: no entrance animation moving things)
        let frame = |ui: &mut Ui, preview: &mut PreviewRotation| {
            ui.begin(1920.0, 1080.0, 3.0, 0.016, 3.0);
            ui.age = 10.0;
            main_menu(ui, 1, preview);
            // (the character's textured faces, not the logo's)
            let logo = crate::textures::tex::LOGO as f32;
            ui.verts.iter().filter(|v| v.mode == 3.0 && v.rect[0] < logo).map(|v| v.pos).collect::<Vec<_>>()
        };
        let first = frame(&mut ui, &mut preview);
        assert!(first.len() > 60);
        // To the right of the sidebar.
        assert!(first.iter().all(|p| p[0] > 700.0 && p[0] < 1920.0));
        let cx = first.iter().map(|p| p[0]).sum::<f32>() / first.len() as f32;

        // Hovering beside it does nothing.
        ui.mouse = Vec2::new(cx + 400.0, 500.0);
        assert_eq!(first, frame(&mut ui, &mut preview));

        // Dragging it turns it.
        ui.mouse = Vec2::new(cx, 500.0);
        ui.mouse_down = true;
        ui.pressed = true;
        frame(&mut ui, &mut preview);
        ui.mouse = Vec2::new(cx + 70.0, 500.0);
        ui.pressed = false;
        assert_ne!(first, frame(&mut ui, &mut preview));
    }
}
