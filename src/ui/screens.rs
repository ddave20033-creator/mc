use crate::lang::{on_off, t};
use crate::settings::Settings;
use crate::ui::*;
use glam::Vec2;
use glam::Vec3;
use std::collections::HashSet;

pub enum Action {
    None,
    Singleplayer,
    Options,
    Credits,
    Quit,
    Resume,
    ToTitle,
    Back,
    ToggleFullscreen,
    /// Options: the anti-aliasing setting changed.
    AntialiasingChanged,
    Respawn,
    Language,
    Multiplayer,
    SkinMenu,
    SelectSkin(u8),
    UploadSkin,
    /// Pause menu: open this world to the LAN.
    OpenLan,
    /// Open a web page in the browser (clicked link).
    OpenLink(&'static str),
    /// Options: the resource pack screen.
    ResourcePacks,
    /// Options: the key binds screen.
    KeyBinds,
    /// Resource pack screen: show `resourcepacks/` in the file explorer.
    OpenPackFolder,
}

pub fn death(ui: &mut Ui, message: &str) -> Action {
    let (w, h, s) = (ui.w, ui.h, ui.s);
    ui.gradient(0.0, 0.0, w, h, rgba(120, 0, 0, 110), rgba(60, 0, 0, 170));
    let title = t("death.title");
    let size = s * 2.0;
    ui.text_centered(
        title,
        w * 0.5,
        (h / 4.0 - 10.0 * s).round(),
        size,
        WHITE,
        true,
    );
    ui.text_centered(
        message,
        w * 0.5,
        (h / 4.0 + 12.0 * s).round(),
        s,
        rgba(230, 230, 230, 255),
        true,
    );
    let (bw, bh) = (200.0 * s, 20.0 * s);
    let x = (w * 0.5 - bw * 0.5).round();
    let y = (h / 4.0 + 44.0 * s).round();
    let mut act = Action::None;
    if ui.button(t("death.respawn"), x, y, bw, bh, true) {
        act = Action::Respawn;
    }
    if ui.button(t("death.title_screen"), x, y + 24.0 * s, bw, bh, true) {
        act = Action::ToTitle;
    }
    act
}

pub const SPLASHES: &[&str] = &[
    "Now with Vulkan!",
    "100% Rust!",
    "Memory safe!",
    "Blazingly fast!",
    "Fearless concurrency!",
    "Borrow checked!",
    "Zero-cost abstractions!",
    "Magyarul is!",
    "Written from scratch!",
    "Also try Minecraft!",
    "Procedural everything!",
    "Ambient occlusion!",
    "cargo run --release",
    "No unsafe (mostly)!",
];

const LOGO: &str = "RUSTCRAFT";

fn hash01(a: i32, b: i32) -> f32 {
    let mut h = (a as u32).wrapping_mul(0x9E37_79B1) ^ (b as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 13;
    (h & 0xFFFF) as f32 / 65535.0
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
        rgba(8, 10, 18, a(140.0)),
        rgba(8, 10, 18, a(30.0)),
    );
    ui.gradient(
        0.0,
        h * 0.5,
        w,
        h * 0.5 + 1.0,
        rgba(8, 10, 18, a(30.0)),
        rgba(8, 10, 18, a(170.0)),
    );
    ui.vignette(rgba(0, 0, 0, a(180.0)));
}

fn logo_units(ui: &Ui) -> f32 {
    LOGO.chars().map(|c| ui.font.glyph(c).adv).sum::<f32>() - 1.0
}

/// Pixel-art 3D logo built from font bitmaps. Returns (width, height) in screen pixels.
fn draw_logo(ui: &mut Ui, cx: f32, y: f32, px: f32) -> (f32, f32) {
    let mut pixels: Vec<(i32, i32, bool)> = Vec::new();
    let mut pen = 0i32;
    for (i, ch) in LOGO.chars().enumerate() {
        let g = *ui.font.glyph(ch);
        for (row, bits) in g.bits.iter().enumerate() {
            for col in g.minx..8 {
                if bits & (1 << col) != 0 {
                    pixels.push((pen + (col - g.minx) as i32, row as i32, i < 4));
                }
            }
        }
        pen += g.adv as i32;
    }
    let set: HashSet<(i32, i32)> = pixels.iter().map(|&(x, y, _)| (x, y)).collect();
    let total_w = (pen - 1) as f32 * px;
    let total_h = 7.0 * px;
    let x0 = (cx - total_w * 0.5).round();
    let y = y.round();

    // Soft glow behind the logo
    ui.rect_full(
        x0 - px,
        y,
        total_w + 2.0 * px,
        total_h,
        rgba(0, 0, 0, 80),
        rgba(0, 0, 0, 80),
        3.0 * px,
        3.0 * px,
    );

    // Extruded 3D sides
    let depth = 4;
    for k in (1..=depth).rev() {
        let off = (k as f32 * px * 0.3).round();
        let f = 1.0 - k as f32 / (depth as f32 + 2.0);
        for &(c, r, rust) in &pixels {
            let base: [f32; 3] = if rust {
                [92.0, 42.0, 22.0]
            } else {
                [52.0, 52.0, 60.0]
            };
            let col = [
                base[0] * f / 255.0,
                base[1] * f / 255.0,
                base[2] * f / 255.0,
                1.0,
            ];
            ui.solid(
                x0 + c as f32 * px + off,
                y + r as f32 * px + off,
                px,
                px,
                col,
            );
        }
    }

    // Front face with stone-like per-pixel variation and bevels
    let bevel = (px * 0.22).max(1.0).round();
    for &(c, r, rust) in &pixels {
        let n = hash01(c, r);
        let base: [f32; 3] = if rust {
            [222.0, 116.0, 62.0]
        } else {
            [200.0, 202.0, 210.0]
        };
        let v = (0.84 + 0.22 * n) * (1.12 - r as f32 * 0.04);
        let col = [
            (base[0] * v / 255.0).min(1.0),
            (base[1] * v / 255.0).min(1.0),
            (base[2] * v / 255.0).min(1.0),
            1.0,
        ];
        let (px0, py0) = (x0 + c as f32 * px, y + r as f32 * px);
        ui.solid(px0, py0, px, px, col);
        if !set.contains(&(c, r - 1)) {
            ui.solid(px0, py0, px, bevel, rgba(255, 255, 255, 80));
        }
        if !set.contains(&(c, r + 1)) {
            ui.solid(px0, py0 + px - bevel, px, bevel, rgba(0, 0, 0, 70));
        }
    }
    (total_w, total_h)
}

#[derive(Default)]
pub struct PreviewRotation {
    pub angle: f32,
    dragging: bool,
    last_x: f32,
}

pub fn main_menu(ui: &mut Ui, splash: &str, skin: u8, preview: &mut PreviewRotation) -> Action {
    let (w, h, s) = (ui.w, ui.h, ui.s);
    backdrop(ui, 1.0);

    // Logo
    let px = ((w * 0.8).min(270.0 * s) / logo_units(ui)).floor().max(2.0);
    let logo_y = (30.0 * s).round();
    let (lw, lh) = draw_logo(ui, w * 0.5, logo_y, px);

    // Edition badge
    let label = t("menu.edition");
    let tw = ui.text_width(label, s);
    let (pw, ph) = ((tw + 16.0 * s).round(), (12.0 * s).round());
    let (bx, by) = (
        (w * 0.5 - pw * 0.5).round(),
        (logo_y + lh + 2.0 * s).round(),
    );
    ui.rect_full(
        bx,
        by + s,
        pw,
        ph,
        rgba(0, 0, 0, 110),
        rgba(0, 0, 0, 110),
        ph * 0.5,
        3.0 * s,
    );
    ui.rect(bx, by, pw, ph, rgba(255, 190, 90, 210), ph * 0.5);
    let bw = (s * 0.67).max(1.0).round();
    ui.rect_full(
        bx + bw,
        by + bw,
        pw - 2.0 * bw,
        ph - 2.0 * bw,
        rgba(40, 30, 22, 240),
        rgba(20, 16, 14, 240),
        ph * 0.5,
        0.0,
    );
    ui.text(
        label,
        w * 0.5 - tw * 0.5,
        by + (ph - 7.0 * s) * 0.5,
        s,
        rgba(255, 206, 120, 255),
        true,
    );

    // Splash text
    let pulse = (ui.time * std::f32::consts::TAU).sin().abs() * 0.1;
    let units = ui.text_width(splash, 1.0);
    let size = s * (1.8 - pulse) * 100.0 / (units + 32.0);
    let center = Vec2::new((w * 0.5 + lw * 0.47).min(w - 60.0 * s), logo_y + lh * 0.72);
    ui.text_rotated(
        splash,
        center,
        size,
        -20f32.to_radians(),
        rgba(255, 255, 0, 255),
    );

    // Buttons
    let (bw, bh) = (200.0 * s, 20.0 * s);
    let x = (w * 0.5 - bw * 0.5).round();
    let y0 = (h / 4.0 + 48.0 * s).max(by + ph + 14.0 * s).round();
    let mut act = Action::None;
    if ui.button(t("menu.singleplayer"), x, y0, bw, bh, true) {
        act = Action::Singleplayer;
    }
    if ui.button(t("menu.multiplayer"), x, y0 + 24.0 * s, bw, bh, true) {
        act = Action::Multiplayer;
    }
    if ui.button(t("menu.credits"), x, y0 + 48.0 * s, bw, bh, true) {
        act = Action::Credits;
    }
    let half = 98.0 * s;
    if ui.button(t("menu.options"), x, y0 + 84.0 * s, half, bh, true) {
        act = Action::Options;
    }
    if ui.button(t("menu.quit"), x + bw - half, y0 + 84.0 * s, half, bh, true) {
        act = Action::Quit;
    }

    // The preview fits in the free area to the right of the menu buttons.
    let px = x + bw + 12.0 * s;
    let pw = (w - px - 12.0 * s).min(112.0 * s);
    if pw >= 55.0 * s {
        let ph = (115.0 * s).min(h - y0 + 20.0 * s - bh - 14.0 * s);
        let py = (y0 - 14.0 * s).max(by + ph * 0.04);
        ui.rect_full(
            px + 14.0 * s,
            py + 8.0 * s,
            pw - 28.0 * s,
            ph - 16.0 * s,
            rgba(10, 16, 28, 22),
            rgba(10, 16, 28, 42),
            24.0 * s,
            24.0 * s,
        );
        ui.rect_full(
            px + pw * 0.28,
            py + ph * 0.86,
            pw * 0.44,
            7.0 * s,
            rgba(0, 0, 0, 55),
            rgba(0, 0, 0, 20),
            4.0 * s,
            5.0 * s,
        );
        draw_menu_player(
            ui,
            [px + 2.0 * s, py + 2.0 * s, pw - 4.0 * s, ph - 4.0 * s],
            skin,
            preview,
        );
        let sy = py + ph + 5.0 * s;
        let hover = ui.hit(px, sy, pw, bh);
        let label = t("menu.skin");
        ui.text_centered(
            label,
            px + pw * 0.5,
            sy + 5.5 * s,
            s,
            if hover { HOVER_TEXT } else { WHITE },
            true,
        );
        if hover {
            ui.rect(
                px + pw * 0.38,
                sy + bh - 2.0 * s,
                pw * 0.24,
                s * 0.6,
                ACCENT,
                s * 0.3,
            );
        }
        if hover && ui.pressed {
            ui.clicked = true;
            act = Action::SkinMenu;
        }
    }

    // Footer
    let fy = h - 10.0 * s;
    ui.text(VERSION, 2.0 * s, fy, s, rgba(255, 255, 255, 190), true);
    let right = t("menu.madewith");
    let rw = ui.text_width(right, s);
    ui.text(
        right,
        w - rw - 2.0 * s,
        fy,
        s,
        rgba(255, 255, 255, 190),
        true,
    );
    act
}

/// Projects the same textured model used in the world into the menu's UI layer.
/// Painter sorting and backface removal keep it clean without touching the world depth buffer.
fn draw_menu_player(ui: &mut Ui, rect: [f32; 4], skin: u8, preview: &mut PreviewRotation) {
    use crate::model::player::{build_player, limb_targets, PlayerPose};
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
                    center.x + p.x * scale,
                    center.y - (p.y - 0.9) * scale + p.z * scale * 0.06,
                ],
                uv: v.uv,
                color: [1.0; 4],
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

    // Tabs
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
    use crate::keys::{display, BINDS, CATEGORIES};
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
    let defaults = crate::keys::KeyMap::default();
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
        _ => "key.hotbar9",
    }
}

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
    ui.gradient(0.0, 0.0, w, h, rgba(0, 0, 0, 150), rgba(0, 0, 0, 110));
    ui.vignette(rgba(0, 0, 0, 120));
    let (bw, bh) = (200.0 * s, 20.0 * s);
    let x = (w * 0.5 - bw * 0.5).round();
    let y = (h / 4.0 + 8.0 * s).round();
    ui.text_centered(t("pause.title"), w * 0.5, y - 26.0 * s, s, WHITE, true);
    let mut act = Action::None;
    if ui.button(t("pause.resume"), x, y, bw, bh, true) {
        act = Action::Resume;
    }
    let mut row = y + 24.0 * s;
    match lan {
        PauseLan::Available => {
            if ui.button(t("pause.lan"), x, row, bw, bh, true) {
                act = Action::OpenLan;
            }
            row += 24.0 * s;
        }
        PauseLan::Open(addr) => {
            let label = format!("{} {addr}", t("pause.lan_open"));
            ui.button(&label, x, row, bw, bh, false);
            row += 24.0 * s;
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
    if ui.button(quit, x, row + 24.0 * s, bw, bh, true) {
        act = Action::ToTitle;
    }
    act
}

pub fn loading(ui: &mut Ui, progress: f32) {
    let (w, h, s) = (ui.w, ui.h, ui.s);
    backdrop(ui, 1.6);
    let cy = (h * 0.5).round();
    ui.text_centered(
        t("loading.generating"),
        w * 0.5,
        cy - 24.0 * s,
        s,
        WHITE,
        true,
    );
    let (bw, bh) = (200.0 * s, 6.0 * s);
    let x = (w * 0.5 - bw * 0.5).round();
    ui.rect(x, cy, bw, bh, rgba(255, 255, 255, 36), bh * 0.5);
    let p = progress.clamp(0.0, 1.0);
    if p > 0.0 {
        ui.rect_full(
            x,
            cy,
            (bw * p).max(bh),
            bh,
            rgba(140, 240, 150, 255),
            with_alpha(ACCENT, 1.0),
            bh * 0.5,
            0.0,
        );
    }
    let pct = format!("{}%", (p * 100.0).round() as i32);
    ui.text_centered(
        &pct,
        w * 0.5,
        cy + bh + 8.0 * s,
        s,
        rgba(220, 220, 220, 255),
        true,
    );
    let tips = [
        t("tip.1"),
        t("tip.2"),
        t("tip.3"),
        t("tip.4"),
        t("tip.5"),
        t("tip.6"),
    ];
    let tip = tips[(ui.time / 3.0) as usize % tips.len()];
    ui.text_centered(
        tip,
        w * 0.5,
        h - 24.0 * s,
        s,
        rgba(180, 180, 180, 255),
        true,
    );
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
            ui.text_centered("RustCraft", cx, y, 2.0 * s, gold, true);
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
        ui.begin(1920.0, 1080.0, 3.0, 0.016, 3.0);
        main_menu(&mut ui, "Test", 1, &mut preview);
        let first: Vec<_> = ui
            .verts
            .iter()
            .filter(|v| v.mode == 3.0)
            .map(|v| v.pos)
            .collect();
        assert!(first.len() > 60);
        assert!(first.iter().all(|p| p[0] > 1260.0 && p[0] < 1920.0));

        ui.mouse = Vec2::new(1500.0, 500.0);
        ui.begin(1920.0, 1080.0, 3.0, 0.016, 3.0);
        main_menu(&mut ui, "Test", 1, &mut preview);
        let hover: Vec<_> = ui
            .verts
            .iter()
            .filter(|v| v.mode == 3.0)
            .map(|v| v.pos)
            .collect();
        assert_eq!(first, hover);

        ui.mouse_down = true;
        ui.pressed = true;
        ui.begin(1920.0, 1080.0, 3.0, 0.016, 3.0);
        main_menu(&mut ui, "Test", 1, &mut preview);
        ui.mouse = Vec2::new(1570.0, 500.0);
        ui.pressed = false;
        ui.begin(1920.0, 1080.0, 3.0, 0.016, 3.0);
        main_menu(&mut ui, "Test", 1, &mut preview);
        let turned: Vec<_> = ui
            .verts
            .iter()
            .filter(|v| v.mode == 3.0)
            .map(|v| v.pos)
            .collect();
        assert_ne!(first, turned);
    }
}
