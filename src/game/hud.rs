//! The HUD: hotbar, hearts, hunger and thirst, air, effects, crosshair, item names, hints,
//! the chat and the F3 debug and performance screens.

use super::*;

/// Frames shown in the F3 frame time graph.
pub(super) const FRAME_GRAPH: usize = 240;
impl Game {
    fn draw_heart(ui: &mut Ui, x: f32, y: f32, px: f32, fill: u8, flash: bool, poison: bool) {
        const SHAPE: [&str; 8] = [
            ".ooo.ooo.",
            "oxxxoxxxo",
            "oxxxxxxxo",
            "oxxxxxxxo",
            ".oxxxxxo.",
            "..oxxxo..",
            "...oxo...",
            "....o....",
        ];
        let outline = if flash {
            rgba(255, 255, 255, 255)
        } else {
            rgba(20, 0, 0, 255)
        };
        for (r, row) in SHAPE.iter().enumerate() {
            for (c, ch) in row.chars().enumerate() {
                let color = match ch {
                    'o' => outline,
                    'x' => {
                        let red = fill == 2 || (fill == 1 && c <= 4);
                        if !red {
                            rgba(60, 12, 12, 255)
                        } else if poison {
                            if r == 1 && (c == 1 || c == 2) {
                                rgba(200, 230, 150, 255)
                            } else if r >= 4 {
                                rgba(100, 120, 20, 255)
                            } else {
                                rgba(140, 160, 34, 255)
                            }
                        } else if r == 1 && (c == 1 || c == 2) {
                            rgba(255, 190, 190, 255)
                        } else if r >= 4 {
                            rgba(190, 20, 24, 255)
                        } else {
                            rgba(230, 36, 40, 255)
                        }
                    }
                    _ => continue,
                };
                ui.solid(x + c as f32 * px, y + r as f32 * px, px, px, color);
            }
        }
    }

    /// A pixel icon from rows of characters: 'o' outline, 'x' fill, 'h' highlight, 'b' bone
    /// (never emptied). `fill` 2 = full, 1 = half (right half filled), 0 = empty.
    fn draw_icon(
        ui: &mut Ui,
        x: f32,
        y: f32,
        px: f32,
        shape: &[&str],
        colors: [Color; 4],
        fill: u8,
    ) {
        let [outline, full, light, empty] = colors;
        for (r, row) in shape.iter().enumerate() {
            for (c, ch) in row.chars().enumerate() {
                let filled = fill == 2 || (fill == 1 && c >= 4);
                let color = match ch {
                    'o' => outline,
                    'b' => rgba(236, 228, 214, 255),
                    'x' if filled => full,
                    'h' if filled => light,
                    'x' | 'h' => empty,
                    _ => continue,
                };
                ui.solid(x + c as f32 * px, y + r as f32 * px, px, px, color);
            }
        }
    }

    fn draw_food(ui: &mut Ui, x: f32, y: f32, px: f32, fill: u8) {
        const SHAPE: [&str; 9] = [
            "....ooo..",
            "...ohhxo.",
            "..ohxxxxo",
            "..oxxxxxo",
            "..oxxxxo.",
            ".obooo...",
            "obo......",
            "oo.......",
            ".........",
        ];
        let colors = [
            rgba(40, 20, 10, 255),
            rgba(180, 100, 40, 255),
            rgba(230, 150, 80, 255),
            rgba(60, 36, 20, 255),
        ];
        Self::draw_icon(ui, x, y, px, &SHAPE, colors, fill);
    }

    fn draw_drop(ui: &mut Ui, x: f32, y: f32, px: f32, fill: u8) {
        const SHAPE: [&str; 9] = [
            "....o....",
            "...oxo...",
            "...oxo...",
            "..oxxxo..",
            ".ohxxxxo.",
            ".ohxxxxo.",
            ".oxxxxxo.",
            "..oxxxo..",
            "...ooo...",
        ];
        let colors = [
            rgba(10, 24, 60, 255),
            rgba(50, 120, 230, 255),
            rgba(160, 210, 255, 255),
            rgba(24, 36, 64, 255),
        ];
        Self::draw_icon(ui, x, y, px, &SHAPE, colors, fill);
    }

    /// Pixel icon of an effect (9x9 at `px` per pixel).
    fn draw_effect_icon(ui: &mut Ui, x: f32, y: f32, px: f32, kind: EffectKind) {
        match kind {
            EffectKind::Poison => {
                // A green drop with bubbles.
                const SHAPE: [&str; 9] = [
                    "....o..o.",
                    "...oxo.oo",
                    "..oxxxo..",
                    ".oxhxxxo.",
                    ".oxhxxxo.",
                    "oxxxxxxxo",
                    "oxxxxxxxo",
                    ".oxxxxxo.",
                    "..ooooo..",
                ];
                let colors = [
                    rgba(20, 44, 12, 255),
                    rgba(78, 147, 49, 255),
                    rgba(170, 220, 120, 255),
                    rgba(0, 0, 0, 0),
                ];
                Self::draw_icon(ui, x, y, px, &SHAPE, colors, 2);
            }
            EffectKind::Nausea => {
                // A dizzy swirl.
                const SHAPE: [&str; 9] = [
                    "..xxxxx..",
                    ".x.....x.",
                    "x..xxx..x",
                    "x.x...x.x",
                    "x.x.h.x.x",
                    "x.x.hh..x",
                    "x..x....x",
                    ".x..xxxx.",
                    "..xx.....",
                ];
                let colors = [
                    rgba(0, 0, 0, 0),
                    rgba(150, 70, 140, 255),
                    rgba(220, 160, 210, 255),
                    rgba(0, 0, 0, 0),
                ];
                Self::draw_icon(ui, x, y, px, &SHAPE, colors, 2);
            }
        }
    }

    /// Square frame of thickness `t` whose bright part shrinks clockwise (from the top left
    /// corner) as `frac` goes from 1 to 0; the rest stays as a dim track.
    fn progress_frame(ui: &mut Ui, x: f32, y: f32, size: f32, t: f32, frac: f32, color: Color) {
        let edge = size - t;
        // Each edge as a rectangle of the lit length `len`, walking clockwise.
        let rect = |k: usize, len: f32| match k {
            0 => (x, y, len, t),
            1 => (x + edge, y, t, len),
            2 => (x + size - len, y + edge, len, t),
            _ => (x, y + size - len, t, len),
        };
        let mut left = frac.clamp(0.0, 1.0) * 4.0 * edge;
        for k in 0..4 {
            let (rx, ry, rw, rh) = rect(k, edge);
            ui.solid(rx, ry, rw, rh, rgba(255, 255, 255, 40));
            let lit = left.min(edge);
            if lit > 0.0 {
                let (rx, ry, rw, rh) = rect(k, lit);
                ui.solid(rx, ry, rw, rh, color);
            }
            left -= lit;
        }
    }

    fn effect_color(kind: EffectKind) -> Color {
        match kind {
            EffectKind::Poison => rgba(120, 200, 70, 255),
            EffectKind::Nausea => rgba(200, 110, 190, 255),
        }
    }

    /// "m:ss" for an effect's time left.
    fn effect_time(left: f32) -> String {
        let secs = left.ceil() as i32;
        format!("{}:{:02}", secs / 60, secs % 60)
    }

    /// Top right corner of the HUD: one small square per effect, its frame running down.
    fn draw_effects_hud(&mut self) {
        let s = self.ui.s;
        let size = 18.0 * s;
        let mut x = self.ui.w - size - 4.0 * s;
        for e in self.needs.effects() {
            let y = 4.0 * s;
            self.ui
                .rect(x, y, size, size, rgba(14, 16, 22, 185), 2.0 * s);
            let color = Self::effect_color(e.kind);
            Self::progress_frame(&mut self.ui, x, y, size, s, e.left / e.total, color);
            let icon = 9.0 * (s * 1.3).floor().max(1.0);
            let ip = icon / 9.0;
            Self::draw_effect_icon(
                &mut self.ui,
                x + (size - icon) * 0.5,
                y + (size - icon) * 0.5,
                ip,
                e.kind,
            );
            x -= size + 3.0 * s;
        }
    }

    /// Next to the inventory window: each effect with its name and time left.
    pub(super) fn draw_effects_list(&mut self, panel_x: f32, panel_y: f32, panel_w: f32) {
        let s = self.ui.s;
        let (w, h) = (100.0 * s, 24.0 * s);
        let mut x = panel_x + panel_w + 6.0 * s;
        if x + w > self.ui.w {
            // No room on the right: the left side instead.
            x = (panel_x - w - 6.0 * s).max(0.0);
        }
        let mut y = panel_y;
        for e in self.needs.effects() {
            let color = Self::effect_color(e.kind);
            self.ui.rect(x, y, w, h, rgba(14, 16, 22, 215), 3.0 * s);
            Self::progress_frame(
                &mut self.ui,
                x + 3.0 * s,
                y + 3.0 * s,
                18.0 * s,
                s,
                e.left / e.total,
                color,
            );
            Self::draw_effect_icon(&mut self.ui, x + 7.5 * s, y + 7.5 * s, s, e.kind);
            let fs = (s - 1.0).max(1.0);
            self.ui
                .text(t(e.kind.key()), x + 25.0 * s, y + 4.0 * s, fs, color, true);
            self.ui.text(
                &Self::effect_time(e.left),
                x + 25.0 * s,
                y + 14.0 * s,
                fs,
                rgba(200, 200, 200, 255),
                true,
            );
            y += h + 3.0 * s;
        }
    }

    fn draw_bubble(ui: &mut Ui, x: f32, y: f32, px: f32) {
        const SHAPE: [&str; 9] = [
            "  ooooo  ",
            " o.....o ",
            "o.xx....o",
            "o.x.....o",
            "o.......o",
            "o.......o",
            "o.......o",
            " o.....o ",
            "  ooooo  ",
        ];
        for (r, row) in SHAPE.iter().enumerate() {
            for (c, ch) in row.chars().enumerate() {
                let color = match ch {
                    'o' => rgba(20, 40, 90, 255),
                    'x' => rgba(255, 255, 255, 255),
                    '.' => rgba(90, 160, 255, 200),
                    _ => continue,
                };
                ui.solid(x + c as f32 * px, y + r as f32 * px, px, px, color);
            }
        }
    }

    pub(super) fn draw_hud(&mut self, underwater: bool, in_lava: bool) {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        let playing = matches!(self.screen, Screen::Playing | Screen::Chat);
        let creative = self.creative();
        {
            let ui = &mut self.ui;
            if underwater {
                ui.gradient(
                    0.0,
                    0.0,
                    w,
                    h,
                    rgba(20, 50, 140, 60),
                    rgba(10, 30, 100, 100),
                );
            }
            if in_lava {
                ui.solid(0.0, 0.0, w, h, rgba(255, 90, 10, 150));
            }
            if self.fire > 0.0 && !creative {
                let flicker = 0.8 + 0.2 * (self.time * 17.0).sin();
                ui.gradient(
                    0.0,
                    h * 0.55,
                    w,
                    h * 0.45,
                    rgba(255, 120, 20, 0),
                    rgba(255, 90, 10, (150.0 * flicker) as u8),
                );
            }
            if self.hurt_time > 0.0 {
                ui.vignette(rgba(200, 0, 0, (self.hurt_time / 0.4 * 200.0) as u8));
            }
            // In bed the view slowly darkens (Minecraft fades it over the 5 seconds).
            if let Some(sl) = self.sleep {
                let k = (sl.time / 5.0).min(1.0);
                ui.solid(0.0, 0.0, w, h, rgba(8, 10, 24, (k * 190.0) as u8));
            }
        }
        if self.sleep.is_some() && matches!(self.screen, Screen::Playing | Screen::Chat) {
            let status = self.sleep_status();
            let ui = &mut self.ui;
            let mut y = h * 0.62;
            if let Some(line) = status {
                ui.text_centered(&line, w * 0.5, y, s, WHITE, true);
                y += 12.0 * s;
            }
            ui.text_centered(t("bed.leave"), w * 0.5, y, s, rgba(200, 200, 200, 255), true);
        }
        // F1 hides the rest while playing (menus and the open chat still show).
        if self.hide_hud && self.screen == Screen::Playing {
            return;
        }
        let gun = self.holding_gun();
        {
            let ui = &mut self.ui;

            // Crosshair (a gun draws its own)
            if self.screen == Screen::Playing
                && self.camera.mode != 2
                && !gun
            {
                let center = Vec2::new((w * 0.5).round(), (h * 0.5).round());
                // Keep the aiming circle small even when the GUI scale is high.
                ui.ring(center, 3.0, 2.4, rgba(0, 0, 0, 170));
                ui.ring(center, 3.0, 1.2, rgba(255, 255, 255, 235));
            }
        }

        if matches!(self.screen, Screen::Playing | Screen::Chat) && self.sleep.is_none() {
            self.draw_gun_hud();
        }

        // Names of the other LAN players.
        if self.in_world_view() && self.camera.mode != 2 {
            self.draw_name_tags();
        }
        // Tab held in a LAN game: who is playing.
        if self.net.is_some() && self.screen == Screen::Playing && self.bind_down(Bind::PlayerList)
        {
            self.draw_player_list();
        }

        // Active effects (top right).
        if self.screen != Screen::Dead {
            self.draw_effects_hud();
        }

        // Hotbar
        let slot = 20.0 * s;
        let gap = 2.0 * s;
        let pad = 3.0 * s;
        let total = 9.0 * slot + 8.0 * gap + 2.0 * pad;
        let x0 = ((w - total) * 0.5).round();
        let y0 = (h - slot - 2.0 * pad - 4.0 * s).round();
        let show_bars = !matches!(self.screen, Screen::Container(_) | Screen::Dead);
        if show_bars {
            let ui = &mut self.ui;
            ui.rect_full(
                x0,
                y0 + 2.0 * s,
                total,
                slot + 2.0 * pad,
                rgba(0, 0, 0, 90),
                rgba(0, 0, 0, 90),
                6.0 * s,
                6.0 * s,
            );
            ui.rect(
                x0,
                y0,
                total,
                slot + 2.0 * pad,
                rgba(255, 255, 255, 30),
                6.0 * s,
            );
            ui.rect(
                x0 + 1.0,
                y0 + 1.0,
                total - 2.0,
                slot + 2.0 * pad - 2.0,
                rgba(14, 16, 22, 185),
                6.0 * s - 1.0,
            );
            let sy = y0 + pad;
            for i in 0..9 {
                let sx = (x0 + pad + i as f32 * (slot + gap)).round();
                ui.rect(sx, sy, slot, slot, rgba(255, 255, 255, 16), 3.0 * s);
            }
            // The highlight glides to the selected slot, like the creative list scrolls.
            let target = self.hotbar_slot as f32;
            let ease = 1.0 - (-18.0 * ui.dt).exp();
            self.hotbar_anim += (target - self.hotbar_anim) * ease;
            if (target - self.hotbar_anim).abs() < 0.002 {
                self.hotbar_anim = target;
            }
            let hx = (x0 + pad + self.hotbar_anim * (slot + gap)).round();
            ui.rect(
                hx - s,
                sy - s,
                slot + 2.0 * s,
                slot + 2.0 * s,
                rgba(255, 255, 255, 235),
                4.0 * s,
            );
            ui.rect(hx, sy, slot, slot, rgba(40, 44, 54, 240), 3.0 * s);
            for i in 0..9 {
                let sx = (x0 + pad + i as f32 * (slot + gap)).round();
                if let Some(st) = self.inventory.slots[i] {
                    gui::draw_stack(ui, sx + 2.0 * s, sy + 2.0 * s, 16.0 * s, &st);
                }
            }
        }

        // Hearts (survival only)
        let hearts_y = (y0 - 11.0 * s).round();
        if show_bars && !creative {
            let flash = self.hurt_time > 0.0 && ((self.hurt_time * 12.0) as i32) % 2 == 0;
            for i in 0..10 {
                let hp = self.health - i as f32 * 2.0;
                let fill = if hp >= 2.0 {
                    2
                } else if hp >= 1.0 {
                    1
                } else {
                    0
                };
                let mut y = hearts_y;
                if self.health <= 4.0 {
                    let jitter = ((self.time * 20.0) as i32 * 7 + i * 13) % 3 - 1;
                    y += jitter as f32 * s;
                }
                let poison = self.needs.poison > 0.0;
                Self::draw_heart(
                    &mut self.ui,
                    x0 + i as f32 * 8.0 * s,
                    y,
                    s,
                    fill,
                    flash,
                    poison,
                );
            }
            // Right side, like Minecraft: hunger, thirst above it, then air bubbles while
            // holding your breath. Both bars empty from the left.
            let right = x0 + total - s;
            let fill = |v: f32, i: i32| {
                let left = v - i as f32 * 2.0;
                if left >= 2.0 {
                    2
                } else if left >= 1.0 {
                    1
                } else {
                    0
                }
            };
            let thirst_y = hearts_y - 10.0 * s;
            for i in 0..10 {
                let x = right - (i + 1) as f32 * 8.0 * s;
                // Shake when nearly empty.
                let jitter = |v: f32| {
                    if v <= 6.0 {
                        (((self.time * 20.0) as i32 * 5 + i * 11) % 3 - 1) as f32 * s * 0.5
                    } else {
                        0.0
                    }
                };
                let (food, thirst) = (self.needs.food, self.needs.thirst);
                Self::draw_food(&mut self.ui, x, hearts_y + jitter(food), s, fill(food, i));
                Self::draw_drop(
                    &mut self.ui,
                    x,
                    thirst_y + jitter(thirst),
                    s,
                    fill(thirst, i),
                );
            }
            if self.air < MAX_AIR {
                let bubbles = (self.air / MAX_AIR * 10.0).ceil() as i32;
                for i in 0..bubbles {
                    let x = right - (i + 1) as f32 * 8.0 * s;
                    Self::draw_bubble(&mut self.ui, x, thirst_y - 10.0 * s, s);
                }
            }
        }

        let held = self.held();
        let ui = &mut self.ui;
        if self.slot_name_timer > 0.0 && playing && held != NONE {
            let a = self.slot_name_timer.min(0.5) / 0.5;
            let name_y = if creative {
                y0 - 14.0 * s
            } else {
                hearts_y - 22.0 * s
            };
            ui.text_centered(
                &item::name(held),
                w * 0.5,
                name_y,
                s,
                with_alpha(WHITE, a),
                true,
            );
        }

        if self.hint_timer > 0.0 && self.screen == Screen::Playing {
            let a = self.hint_timer.min(1.0);
            let text = t("hud.hint");
            let fs = (s - 1.0).max(1.0);
            let tw = ui.text_width(text, fs);
            let (bw, bh) = (tw + 12.0 * fs, 14.0 * fs);
            let bx = ((w - bw) * 0.5).round();
            ui.rect(
                bx,
                6.0 * s,
                bw,
                bh,
                rgba(10, 12, 18, (150.0 * a) as u8),
                bh * 0.5,
            );
            ui.text(
                text,
                bx + 6.0 * fs,
                6.0 * s + 3.5 * fs,
                fs,
                with_alpha(WHITE, a),
                true,
            );
        }

        self.chat.draw(ui, self.time, self.screen == Screen::Chat);

        let fs = (s - 1.0).max(1.0);
        if self.show_debug {
            let p = self.player.pos;
            let deg = self.yaw.to_degrees().rem_euclid(360.0);
            let facing = match ((deg + 45.0) / 90.0) as i32 % 4 {
                0 => "east (+X)",
                1 => "south (+Z)",
                2 => "west (-X)",
                _ => "north (-Z)",
            };
            let world = &self.terrain.world;
            let target = self
                .target
                .map(|(t, _)| {
                    format!(
                        "Looking at: {} ({}, {}, {})",
                        item::block_name(world.geti(t)),
                        t.x,
                        t.y,
                        t.z
                    )
                })
                .unwrap_or_else(|| "Looking at: -".into());
            let biome = self
                .terrain
                .gen
                .column(p.x.floor() as i32, p.z.floor() as i32)
                .biome;
            let hours = (self.time_of_day * 24.0 + 6.0) % 24.0;
            let lines = [
                crate::ui::VERSION.to_string(),
                format!("{:.0} fps", self.fps),
                format!("GPU: {}", self.gpu.device_name),
                format!(
                    "Chunks: {} drawn / {} loaded, {} pending",
                    self.renderer.drawn_chunks,
                    self.renderer.chunk_count(),
                    self.renderer.pending()
                ),
                format!("XYZ: {:.2} / {:.2} / {:.2}", p.x, p.y, p.z),
                format!("Facing: {facing}"),
                format!("Biome: {}", biome.name()),
                format!(
                    "Time: {:02}:{:02} ({} ticks)",
                    hours as i32,
                    ((hours.fract()) * 60.0) as i32,
                    (self.time_of_day * 24000.0) as i32
                ),
                target,
                format!(
                    "Mode: {:?}{}  HP: {:.0}/20  Items: {}",
                    self.game_mode,
                    if self.player.flying { " (flying)" } else { "" },
                    self.health,
                    self.items.len()
                ),
            ];
            for (i, line) in lines.iter().enumerate() {
                let y = 2.0 * s + i as f32 * 10.0 * fs;
                let tw = ui.text_width(line, fs);
                ui.solid(
                    2.0 * s,
                    y - fs,
                    tw + 3.0 * fs,
                    10.0 * fs,
                    rgba(0, 0, 0, 110),
                );
                ui.text(line, 2.0 * s + fs, y, fs, rgba(230, 230, 230, 255), true);
            }
            self.draw_performance(fs);
        } else if self.settings.show_fps {
            ui.text(
                &format!("{:.0} FPS", self.fps),
                3.0 * s,
                3.0 * s,
                fs,
                rgba(230, 230, 230, 255),
                true,
            );
        }
    }

    /// Right side of the F3 screen: frame rate and frame times (with a graph), and how much
    /// processor, memory, video memory and GPU this program and the whole system use.
    fn draw_performance(&mut self, fs: f32) {
        let st = self.sys_stats.get();
        let mb = |b: u64| b / (1024 * 1024);
        let gb = |b: u64| b as f32 / (1024.0 * 1024.0 * 1024.0);
        let times = &self.frame_times;
        let n = times.len().max(1) as f32;
        let avg = times.iter().sum::<f32>() / n;
        let max = times.iter().copied().fold(0.0, f32::max);
        let min = times.iter().copied().fold(f32::MAX, f32::min).min(max);
        let pct = |v: Option<f32>| v.map_or("n/a".to_string(), |v| format!("{v:.0}%"));
        let lines = [
            format!("FPS: {:.0}", self.fps),
            format!("Frame: {avg:.1} ms (min {min:.1} / max {max:.1})"),
            format!(
                "CPU ms: update {:.1} / build {:.1} / submit {:.1}, wait {:.1}",
                self.cpu_ms[0], self.cpu_ms[1], self.cpu_ms[2], self.cpu_ms[3]
            ),
            match self.renderer.gpu_ms {
                Some([sh, wo, ui]) => {
                    format!("GPU ms: shadows {sh:.1} / world {wo:.1} / UI {ui:.1}")
                }
                None => "GPU ms: n/a".to_string(),
            },
            String::new(),
            format!("CPU: {}", st.cpu_name),
            format!(
                "CPU use: {:.0}% game / {:.0}% total ({} threads)",
                st.cpu_game, st.cpu_total, st.threads
            ),
            format!(
                "RAM: {} MB game / {:.1} of {:.1} GB",
                mb(st.ram_game),
                gb(st.ram_used),
                gb(st.ram_total)
            ),
            String::new(),
            format!("GPU: {}", self.gpu.device_name),
            format!(
                "GPU use: {} game / {} total",
                pct(st.gpu_game),
                pct(st.gpu_total)
            ),
            match self.vram {
                Some((used, budget)) => format!("VRAM: {} MB / {} MB", mb(used), mb(budget)),
                None => "VRAM: n/a".to_string(),
            },
        ];
        let ui = &mut self.ui;
        let (w, s) = (ui.w, ui.s);
        let right = w - 2.0 * s;
        let mut y = 2.0 * s;
        for line in &lines {
            if !line.is_empty() {
                let tw = ui.text_width(line, fs);
                ui.solid(
                    right - tw - 3.0 * fs,
                    y - fs,
                    tw + 3.0 * fs,
                    10.0 * fs,
                    rgba(0, 0, 0, 110),
                );
                ui.text(
                    line,
                    right - tw - 1.5 * fs,
                    y,
                    fs,
                    rgba(230, 230, 230, 255),
                    true,
                );
            }
            y += 10.0 * fs;
        }

        // Frame time graph: one bar per frame, newest on the right. Lines mark 60 and 30 fps;
        // bars turn yellow and red past them.
        let bar = fs.max(1.0);
        let gw = FRAME_GRAPH as f32 * bar;
        let gh = 60.0 * fs;
        let gx = right - gw;
        let gy = y + 2.0 * fs;
        ui.solid(gx, gy, gw, gh, rgba(0, 0, 0, 110));
        let scale = gh / 50.0; // 50 ms fills the graph
        for (i, &ms) in times.iter().enumerate() {
            let bh = (ms * scale).clamp(1.0, gh);
            let color = if ms <= 1000.0 / 60.0 {
                rgba(90, 220, 110, 230)
            } else if ms <= 1000.0 / 30.0 {
                rgba(240, 210, 70, 230)
            } else {
                rgba(240, 80, 70, 230)
            };
            let x = gx + (FRAME_GRAPH - times.len() + i) as f32 * bar;
            ui.solid(x, gy + gh - bh, bar, bh, color);
        }
        for (ms, label) in [(1000.0 / 60.0, "16.7 ms"), (1000.0 / 30.0, "33.3 ms")] {
            let ly = (gy + gh - ms * scale).round();
            ui.solid(gx, ly, gw, fs.max(1.0) * 0.5, rgba(255, 255, 255, 110));
            ui.text(
                label,
                gx + 1.5 * fs,
                ly - 8.0 * fs,
                fs,
                rgba(230, 230, 230, 200),
                true,
            );
        }
    }
}
