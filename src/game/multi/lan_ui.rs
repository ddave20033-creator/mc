//! What LAN play shows: names above the other players, the player list (Tab), and the
//! multiplayer and connection screens.

use super::*;
impl Game {
    /// Names above the other players' heads.
    pub(in crate::game) fn draw_name_tags(&mut self) {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        let cam = self.player.eye();
        for r in &self.remotes {
            let p = r.pose;
            if !r.shown() {
                continue;
            }
            let top = p.pos + Vec3::Y * (2.1 - 0.3 * p.crouch);
            // Up to 48 blocks away (fading out over the last 8), and not through walls.
            let dist = top.distance(cam);
            if dist > NAME_RANGE || !self.line_of_sight(cam, top) {
                continue;
            }
            let fade = ((NAME_RANGE - dist) / 8.0).clamp(0.0, 1.0);
            let clip = self.view_proj * top.extend(1.0);
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
            let focal = 1.0 / (self.fov_current.to_radians() * 0.5).tan();
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
        }
    }

    /// No solid block between `a` and `b` (checked every fifth of a block).
    pub(super) fn line_of_sight(&self, a: Vec3, b: Vec3) -> bool {
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

    /// Tab: everyone in the LAN game, top center (the host first, marked).
    pub(in crate::game) fn draw_player_list(&mut self) {
        let my_id = match &self.net {
            Some(Net::Client(c)) => c.id,
            _ => HOST_ID,
        };
        let mut players: Vec<(u8, String)> = self
            .remotes
            .iter()
            .map(|r| (r.id, r.name.clone()))
            .collect();
        players.push((my_id, self.settings.name.clone()));
        players.sort_by_key(|(id, _)| *id);

        let (w, s) = (self.ui.w, self.ui.s);
        let title = tf("lan.players", &[&players.len()]);
        let host_tag = t("lan.host_tag");
        let label = |id: u8, name: &str| {
            if id == HOST_ID {
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

    pub(in crate::game) fn open_multiplayer(&mut self) {
        self.finder = Some(Finder::start());
        self.mp_selected = None;
        self.screen = Screen::Multiplayer;
    }

    pub(in crate::game) fn multiplayer_screen(&mut self) -> Action {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        screens::backdrop(&mut self.ui, 1.5);
        if let Some(f) = &mut self.finder {
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
            .finder
            .as_ref()
            .map(|f| f.games.clone())
            .unwrap_or_default();
        let mut join = None;
        if games.is_empty() {
            let dots = ".".repeat(1 + (self.time * 2.0) as usize % 3);
            let text = if self.finder.as_ref().is_some_and(|f| f.error) {
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
            let selected = self.mp_selected == Some(i);
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
                let double = self.last_click.0 == i && self.time - self.last_click.1 < 0.35;
                self.mp_selected = Some(i);
                self.last_click = (i, self.time);
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
        let mut addr = std::mem::take(&mut self.mp_address);
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
        self.mp_address = addr;
        let can_direct = !self.mp_address.trim().is_empty();
        if self.ui.button(
            t("mp.connect"),
            lx + lw - 80.0 * s,
            y,
            80.0 * s,
            20.0 * s,
            can_direct,
        ) {
            join = Some(self.mp_address.clone());
        }
        y += 30.0 * s;

        // Buttons
        let bw = (lw - 8.0 * s) / 2.0;
        let chosen = self
            .mp_selected
            .and_then(|i| games.get(i))
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
            self.finder = None;
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
    pub(in crate::game) fn net_status_screen(&mut self) -> Action {
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
        let msg = self.net_message.clone();
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
                self.leave_server(None);
                self.open_multiplayer();
            } else {
                self.screen = Screen::MainMenu;
            }
        }
        Action::None
    }
}
