//! What LAN play shows: names above the other players, the player list (Tab), and the
//! multiplayer and connection screens.

use crate::client::{Game, Screen};
use crate::client::multi::{NAME_RANGE, OWNER_ID};
use crate::lang::{t, tf};
use crate::net::Finder;
use crate::ui::{Color, Ui, WHITE, rgba, screens, with_alpha};
use crate::ui::screens::Action;
use crate::world::is_opaque;
use glam::{Vec2, Vec3};
use std::f32::consts::TAU;

impl Game {
    /// Names above the other players' heads.
    pub(in crate::client) fn draw_name_tags(&mut self) {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        let cam = self.eye();
        let time = self.clock.time;
        for r in &self.session.remotes {
            let p = r.pose;
            if !r.shown() || Some(r.id) == self.session.spectating {
                continue;
            }
            let top = p.pos + Vec3::Y * (2.1 - 0.3 * p.crouch);
            // Up to 48 blocks away (fading out over the last 8), and not through walls.
            let dist = top.distance(cam);
            if dist > NAME_RANGE || !self.line_of_sight(cam, top) {
                continue;
            }
            let fade = ((NAME_RANGE - dist) / 8.0).clamp(0.0, 1.0);
            let clip = self.me.look.view_proj * top.extend(1.0);
            if clip.w < 0.1 {
                continue;
            }
            let ndc = clip.truncate() / clip.w;
            if ndc.x.abs() > 1.2 || ndc.y.abs() > 1.2 {
                continue;
            }
            let (x, y) = ((ndc.x * 0.5 + 0.5) * w, (ndc.y * 0.5 + 0.5) * h);
            // The tag is a fixed size in the world (about a quarter block tall, like
            // Minecraft's), so it shrinks with distance instead of covering far players.
            let focal = 1.0 / (self.me.look.fov.to_radians() * 0.5).tan();
            let px_per_block = h * 0.5 * focal / clip.w;
            let fs = (0.27 * px_per_block / 9.0).clamp(0.5, 2.0 * s);
            let tw = self.ui.text_width(&r.name, fs);
            let (bx, by) = (x - tw * 0.5 - 2.0 * fs, y - 9.0 * fs);
            let bg = rgba(0, 0, 0, (100.0 * fade) as u8);
            self.ui.rect(bx, by, tw + 4.0 * fs, 9.0 * fs, bg, fs);
            // Sneaking players' names are dimmed, like Minecraft.
            let alpha = if p.crouch > 0.5 { 0.5 } else { 1.0 } * fade;
            self.ui
                .text_centered(&r.name, x, by + fs, fs, with_alpha(WHITE, alpha), false);
            status_bubble(&mut self.ui, x, by - 1.5 * fs, fs * 0.28, p.status, time, fade);
        }
    }

    /// No solid block between `a` and `b` (checked every fifth of a block).
    pub(in crate::client) fn line_of_sight(&self, a: Vec3, b: Vec3) -> bool {
        let d = b - a;
        let steps = (d.length() * 5.0).ceil() as i32;
        (1..steps).all(|i| {
            let p = a + d * (i as f32 / steps as f32);
            !is_opaque(self.terrain.world.get(
                p.x.floor() as i32,
                p.y.floor() as i32,
                p.z.floor() as i32,
            ))
        })
    }

    /// Tab: everyone in the world, top center (the world's owner first, marked).
    pub(in crate::client) fn draw_player_list(&mut self) {
        let my_id = self.session.net.as_ref().map_or(OWNER_ID, |c| c.id);
        let mut players: Vec<(u8, String)> = self
            .session.remotes
            .iter()
            .map(|r| (r.id, r.name.clone()))
            .collect();
        players.push((my_id, self.settings.name.clone()));
        players.sort_by_key(|(id, _)| *id);

        let (w, s) = (self.ui.w, self.ui.s);
        let title = tf("lan.players", &[&players.len()]);
        let host_tag = t("lan.host_tag");
        let label = |id: u8, name: &str| {
            if id == OWNER_ID {
                format!("{name} {host_tag}")
            } else {
                name.to_string()
            }
        };
        // Two columns once the list gets long, like Minecraft.
        let cols = if players.len() > 10 { 2 } else { 1 };
        let rows = players.len().div_ceil(cols);
        let col_w = players
            .iter()
            .map(|(id, n)| self.ui.text_width(&label(*id, n), s))
            .fold(self.ui.text_width(&title, s) / cols as f32, f32::max)
            + 16.0 * s;
        let row_h = 10.0 * s;
        let (bw, bh) = (
            col_w * cols as f32 + 8.0 * s,
            20.0 * s + rows as f32 * row_h,
        );
        let (bx, by) = ((w * 0.5 - bw * 0.5).round(), 10.0 * s);
        self.ui.rect(bx, by, bw, bh, rgba(0, 0, 0, 150), 3.0 * s);
        self.ui.text_centered(
            &title,
            w * 0.5,
            by + 4.0 * s,
            s,
            rgba(255, 255, 170, 255),
            true,
        );
        for (i, (id, name)) in players.iter().enumerate() {
            let (c, r) = (i / rows, i % rows);
            let x = bx + 4.0 * s + c as f32 * col_w;
            let y = by + 16.0 * s + r as f32 * row_h;
            // Each name on its own light stripe; this player's own is highlighted.
            let mine = *id == my_id;
            let stripe = if mine {
                rgba(255, 255, 255, 50)
            } else {
                rgba(255, 255, 255, 22)
            };
            self.ui.solid(x, y - s, col_w - 4.0 * s, row_h - s, stripe);
            let color = if mine {
                rgba(255, 255, 170, 255)
            } else {
                WHITE
            };
            self.ui
                .text(&label(*id, name), x + 3.0 * s, y, s, color, true);
        }
    }

    pub(in crate::client) fn open_multiplayer(&mut self) {
        self.menus.finder = Some(Finder::start());
        self.menus.mp_selected = None;
        self.screen = Screen::Multiplayer;
    }

    pub(in crate::client) fn multiplayer_screen(&mut self) -> Action {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        screens::backdrop(&mut self.ui, 1.5);
        if let Some(f) = &mut self.menus.finder {
            f.update();
        }
        self.ui
            .text_centered(t("mp.title"), w * 0.5, 12.0 * s, s, WHITE, true);

        let lw = 300.0 * s;
        let lx = (w * 0.5 - lw * 0.5).round();
        let gray = rgba(170, 170, 170, 255);

        // Player name
        let mut y = 28.0 * s;
        self.ui.text(t("mp.name"), lx, y + 6.0 * s, s, gray, true);
        let nx = lx + 90.0 * s;
        let mut name = std::mem::take(&mut self.settings.name);
        self.ui.text_field(
            "mp_name",
            &mut name,
            nx,
            y,
            lw - 90.0 * s,
            20.0 * s,
            "Steve",
            16,
        );
        name.retain(|c| c.is_alphanumeric() || c == '_');
        self.settings.name = name;
        y += 28.0 * s;

        // LAN games
        self.ui.text(t("mp.lan_games"), lx, y, s, gray, true);
        y += 11.0 * s;
        let row_h = 30.0 * s;
        let list_h = (h - y - 118.0 * s).max(row_h * 2.0);
        self.ui.solid(lx, y, lw, list_h, rgba(0, 0, 0, 110));
        let games = self
            .menus.finder
            .as_ref()
            .map(|f| f.games.clone())
            .unwrap_or_default();
        let mut join = None;
        if games.is_empty() {
            let dots = ".".repeat(1 + (self.clock.time * 2.0) as usize % 3);
            let text = if self.menus.finder.as_ref().is_some_and(|f| f.error) {
                t("mp.search_failed").to_string()
            } else {
                format!("{}{dots}", t("mp.searching"))
            };
            self.ui
                .text_centered(&text, w * 0.5, y + list_h * 0.5 - 4.0 * s, s, gray, true);
        }
        for (i, g) in games.iter().enumerate() {
            let ry = y + 2.0 * s + i as f32 * row_h;
            if ry + row_h > y + list_h {
                break;
            }
            let selected = self.menus.mp_selected == Some(g.addr);
            let hovered = self.ui.hit(lx, ry, lw, row_h - 2.0 * s);
            if selected || hovered {
                let a = if selected { 60 } else { 25 };
                self.ui.rect(
                    lx + 2.0 * s,
                    ry,
                    lw - 4.0 * s,
                    row_h - 2.0 * s,
                    rgba(255, 255, 255, a),
                    3.0 * s,
                );
            }
            if hovered && self.ui.pressed {
                let double = self.menus.mp_selected == Some(g.addr) && self.menus.last_click.0 == i && self.clock.time - self.menus.last_click.1 < 0.35;
                self.menus.mp_selected = Some(g.addr);
                self.menus.last_click = (i, self.clock.time);
                if double && g.compatible {
                    join = Some(g.addr.to_string());
                }
            }
            self.ui
                .text(&g.world, lx + 8.0 * s, ry + 5.0 * s, s, WHITE, true);
            let (info, color) = if g.compatible {
                (format!("{} - {}", g.host, g.addr), gray)
            } else {
                (t("mp.other_version").to_string(), rgba(255, 110, 110, 255))
            };
            let fs = (s - 1.0).max(1.0);
            self.ui
                .text(&info, lx + 8.0 * s, ry + 17.0 * s, fs, color, true);
        }
        y += list_h + 8.0 * s;

        // Direct connection
        self.ui.text(t("mp.direct"), lx, y + 6.0 * s, s, gray, true);
        let mut addr = std::mem::take(&mut self.menus.mp_address);
        let field_w = lw - 90.0 * s - 84.0 * s;
        self.ui.text_field(
            "mp_addr",
            &mut addr,
            nx,
            y,
            field_w,
            20.0 * s,
            "192.168.1.10",
            64,
        );
        self.menus.mp_address = addr;
        let can_direct = !self.menus.mp_address.trim().is_empty();
        if self.ui.button(
            t("mp.connect"),
            lx + lw - 80.0 * s,
            y,
            80.0 * s,
            20.0 * s,
            can_direct,
        ) {
            join = Some(self.menus.mp_address.clone());
        }
        y += 30.0 * s;

        // Buttons
        let bw = (lw - 8.0 * s) / 2.0;
        let chosen = self
            .menus.mp_selected
            .and_then(|a| games.iter().find(|g| g.addr == a))
            .filter(|g| g.compatible);
        if self
            .ui
            .button(t("mp.join"), lx, y, bw, 20.0 * s, chosen.is_some())
        {
            join = chosen.map(|g| g.addr.to_string());
        }
        if self
            .ui
            .button(t("gui.back"), lx + bw + 8.0 * s, y, bw, 20.0 * s, true)
        {
            self.menus.finder = None;
            self.settings.save();
            self.screen = Screen::MainMenu;
        }
        if let Some(addr) = join {
            if !self.settings.name.is_empty() {
                self.join_server(&addr);
            }
        }
        Action::None
    }

    /// "Connecting..." and "Disconnected" screens.
    pub(in crate::client) fn net_status_screen(&mut self) -> Action {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        screens::backdrop(&mut self.ui, 1.6);
        let cy = (h * 0.5 - 20.0 * s).round();
        let title = if self.screen == Screen::Connecting {
            t("mp.connecting")
        } else {
            t("mp.disconnected")
        };
        self.ui
            .text_centered(title, w * 0.5, cy - 14.0 * s, s, WHITE, true);
        let msg = self.menus.net_message.clone();
        self.ui.text_centered(
            &msg,
            w * 0.5,
            cy + 2.0 * s,
            s,
            rgba(190, 190, 190, 255),
            true,
        );
        let bw = 200.0 * s;
        let label = if self.screen == Screen::Connecting {
            t("gui.cancel")
        } else {
            t("mp.back_to_menu")
        };
        if self.ui.button(
            label,
            (w * 0.5 - bw * 0.5).round(),
            cy + 24.0 * s,
            bw,
            20.0 * s,
            true,
        ) {
            if self.screen == Screen::Connecting {
                self.cancel_connecting();
            } else {
                self.screen = Screen::MainMenu;
            }
        }
        Action::None
    }
}

/// The bubble above a player's name showing what they are busy with (`net::status`): an
/// animated icon, no text. Its pointer ends at (x, bottom); `u` is the size of one of the
/// 56 x 46 units the bubble is drawn in (the same design as tools/status_icons).
fn status_bubble(ui: &mut Ui, x: f32, bottom: f32, u: f32, status: u8, time: f32, fade: f32) {
    use crate::net::status;
    let accent = match status {
        status::TYPING => rgba(77, 163, 255, 255),
        status::MENU => rgba(255, 181, 71, 255),
        status::AFK => rgba(167, 139, 250, 255),
        status::INVENTORY => rgba(74, 222, 128, 255),
        status::READING => rgba(251, 191, 36, 255),
        _ => return,
    };
    // The inactive slots of the grid: the accent sunk into the bubble.
    let dim = with_alpha(crate::ui::lerp_color(accent, rgba(18, 19, 24, 255), 0.62), fade);
    let (bg, accent) = (rgba(18, 19, 24, (255.0 * fade) as u8), with_alpha(accent, fade));
    // One loop of the animation every 1.12 s; `ease` goes 0..1..0 over it.
    let t = (time / 1.12).fract();
    let ease = |t: f32| 0.5 - 0.5 * (t * TAU).cos();
    let circle = |ui: &mut Ui, cx: f32, cy: f32, r: f32, c: Color| {
        ui.rect(cx - r, cy - r, 2.0 * r, 2.0 * r, c, r)
    };

    let (bw, bh) = (56.0 * u, 46.0 * u);
    let (x0, y0) = (x - bw * 0.5, bottom - 7.0 * u - bh);
    ui.rect(x0, y0, bw, bh, bg, 16.0 * u);
    let tip = Vec2::new(x, bottom);
    ui.quad(
        [
            Vec2::new(x - 7.0 * u, y0 + bh),
            Vec2::new(x + 7.0 * u, y0 + bh),
            tip,
            tip,
        ],
        bg,
    );

    let (cx, cy) = (x, y0 + bh * 0.5);
    match status {
        status::TYPING => {
            // Three dots bouncing one after the other.
            for i in 0..3 {
                let phase = (t - i as f32 * 0.15).rem_euclid(1.0);
                let lift = if phase < 0.5 { (phase * TAU).sin() } else { 0.0 };
                let dx = (i as f32 - 1.0) * 11.0 * u;
                circle(ui, cx + dx, cy - lift * 5.0 * u, 4.0 * u, accent);
            }
        }
        status::MENU => {
            // Pause bars, breathing slightly.
            let k = 0.9 + 0.1 * ease(t);
            let (w, h) = (6.0 * u * k, 20.0 * u * k);
            for side in [-1.0, 1.0] {
                let bx = cx + side * (2.0 * u + w * 0.5) - w * 0.5;
                ui.rect(bx, cy - h * 0.5, w, h, accent, 2.0 * u);
            }
        }
        status::AFK => {
            // Crescent moon with two stars twinkling in turn.
            let (mx, my, r) = (cx - 3.0 * u, cy + u, 10.0 * u);
            circle(ui, mx, my, r, accent);
            circle(ui, mx + 6.0 * u, my - 4.0 * u, r, bg);
            for (k, (sx, sy, size)) in [(11.0, -9.0, 5.0), (15.0, 3.0, 3.8)].into_iter().enumerate() {
                let r = size * u * (0.45 + 0.55 * ease(t + k as f32 * 0.5));
                let (px, py, n) = (cx + sx * u, cy + sy * u, r * 0.28);
                let v = Vec2::new;
                ui.quad([v(px, py - r), v(px + n, py), v(px, py + r), v(px - n, py)], accent);
                ui.quad([v(px - r, py), v(px, py - n), v(px + r, py), v(px, py + n)], accent);
            }
        }
        status::READING => {
            // An open book; a page turns over from the right to the left.
            let v = Vec2::new;
            let (pw, ph, sag) = (15.0 * u, 18.0 * u, 3.0 * u);
            let top = cy - ph * 0.5;
            for side in [-1.0, 1.0] {
                let edge = cx + side * pw;
                ui.quad(
                    [v(cx, top + sag), v(edge, top), v(edge, top + ph), v(cx, top + ph + sag)],
                    dim,
                );
            }
            // The turning page: flat on the right, standing up, flat on the left.
            let k = ease(t * 0.5);
            let edge = cx + pw * (1.0 - 2.0 * k);
            let lift = (k * std::f32::consts::PI).sin() * 5.0 * u;
            ui.quad(
                [
                    v(cx, top + sag),
                    v(edge, top - lift),
                    v(edge, top + ph - lift),
                    v(cx, top + ph + sag),
                ],
                accent,
            );
        }
        _ => {
            // A 3x3 grid of slots; the lit one goes along them.
            let (sz, gap) = (7.0 * u, 2.5 * u);
            let lit = (t * 9.0) as usize % 9;
            for i in 0..9 {
                let sx = cx - 1.5 * sz - gap + (i % 3) as f32 * (sz + gap);
                let sy = cy - 1.5 * sz - gap + (i / 3) as f32 * (sz + gap);
                let c = if i == lit { accent } else { dim };
                ui.rect(sx, sy, sz, sz, c, 1.8 * u);
            }
        }
    }
}
