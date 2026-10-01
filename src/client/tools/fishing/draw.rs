//! Fishing seen and heard: the rod's animation, its creaking, the lines, bobbers and fish in
//! the world, and the HUD (the reel's gear, the line out, the tension bar, the messages).

use super::*;

impl Game {
    /// What the held rod is doing, for its animation (and the others).
    pub(in crate::client) fn rod_anim(&self) -> Option<RodAnim> {
        if self.held() != FISHING_ROD {
            return None;
        }
        let f = &self.tools.fishing;
        let line = f.line.as_ref();
        Some(RodAnim {
            charge: f.charge.map_or(0.0, |c| smooth(c / CHARGE_TIME)),
            cast: f.cast,
            out: line.is_some(),
            fight: f.fight_k,
            tension: f.tension,
            crank: f.crank,
            lift: f.lift,
            bobber: line.map(|l| f.bob_draw.unwrap_or(l.bobber) - Vec3::Y * l.dip_now),
        })
    }

    /// The rod creaking under a fish's pull (a looping sound).
    pub(in crate::client) fn fishing_sounds(&self) -> Vec<(u64, Sound, Vec3, f32)> {
        match &self.tools.fishing.line {
            Some(Line { fight: Some(f), .. }) if f.tension > 0.35 => {
                vec![(0xf15_4000, Sound::RodCreak, self.rod_tip(), ((f.tension - 0.35) * 1.6).min(1.0))]
            }
            _ => Vec::new(),
        }
    }

    /// The lines from the rods' tips (this player's and the others'), the bobbers, a fish
    /// near the surface on the line or flying out, a line just parted.
    pub(in crate::client) fn build_fishing(&self, out: &mut Vec<Vertex>, cam: Vec3) {
        let world = &self.terrain.world;
        let light = |p: Vec3| {
            let (sky, blk) = world.light_estimate(p + Vec3::Y * 0.3);
            vertex_light(sky, blk)
        };
        let mut rods: Vec<(Vec3, RodAnim, bool)> = self.remote_rods().into_iter().map(|(t, a)| (t, a, false)).collect();
        let own = self.rod_anim().filter(|_| self.holding_rod());
        if let Some(a) = own {
            let tip = if self.me.look.first_person { self.tools.fishing.tip_fp } else { self.tools.fishing.tip_tp };
            if let Some(tip) = tip {
                rods.push((tip, a, true));
            }
        }
        for (tip, a, mine) in rods {
            match a.bobber {
                Some(b) => {
                    let len = tip.distance(b);
                    let slack = 1.0 - (a.tension / 0.35).clamp(0.0, 1.0);
                    let flying = a.cast.is_some() && a.fight == 0.0;
                    let sag = if flying { 0.01 * len } else { (0.035 * len * slack).min(2.0) };
                    emit_line(out, tip, b + Vec3::Y * 0.1, cam, sag, light((tip + b) * 0.5));
                    let toward = Vec3::new(tip.x - b.x, 0.0, tip.z - b.z).normalize_or(Vec3::X);
                    let tilt = if a.fight > 0.5 { 0.9 } else { 0.25 * a.tension };
                    // (bigger far off, to be seen out on the water)
                    let size = 1.0 + (b.distance(cam) / 30.0).min(1.0);
                    angler::emit_bobber(out, b, toward, tilt, size, light(b));
                }
                None => {
                    // Hanging from the tip on a short line (this player's swings, see
                    // `smooth_fishing`), along the line.
                    let hang = self.tools.fishing.hang.map(|h| h.0).filter(|_| mine);
                    let sway = Vec3::new((self.clock.time * 1.3).sin(), 0.0, (self.clock.time * 1.1).cos()) * 0.02;
                    let knot = hang.unwrap_or(tip - Vec3::Y * 0.55 + sway);
                    let up = (tip - knot).normalize_or(Vec3::Y);
                    let b = knot - up * 0.1;
                    emit_line(out, tip, knot, cam, 0.0, light(tip));
                    let toward = Vec3::new(up.x, 0.0, up.z).normalize_or(Vec3::X);
                    angler::emit_bobber(out, b, toward, up.y.clamp(-1.0, 1.0).acos(), 1.0, light(tip));
                }
            }
        }
        // A fish close in on the line shows at the surface.
        if let (Some(line), true) = (&self.tools.fishing.line, own.is_some()) {
            if let Some(f) = line.fight.as_ref().filter(|f| f.dist < 6.0) {
                let sp = &SPECIES[f.species];
                let size = (f.weight / 0.5).cbrt().clamp(0.7, 3.0);
                let tip = self.rod_tip();
                let away = Vec3::new(line.bobber.x - tip.x, 0.0, line.bobber.z - tip.z).normalize_or(Vec3::X);
                let wiggle = (self.clock.time * (8.0 + 10.0 * f.pull)).sin() * 0.6;
                let bob = self.tools.fishing.bob_draw.unwrap_or(line.bobber);
                let pos = bob - Vec3::Y * (0.12 * size) + away * 0.12 * size;
                angler::emit_fish(out, pos, away + Vec3::Y * 0.2, size, wiggle, sp.tint, light(pos));
            }
        }
        // A landed fish flying out to the player.
        if let Some((from, t, species, weight)) = self.tools.fishing.flying {
            let k = t / 0.6;
            // (to the feet in front, not into the eyes)
            let ahead = look_dir(self.me.look.yaw, 0.0);
            let to = self.me.body.pos + Vec3::Y * 0.5 + ahead * 0.9;
            let pos = from.lerp(to, k) + Vec3::Y * (4.0 * k * (1.0 - k) * 1.5);
            let size = (weight / 0.5).cbrt().clamp(0.7, 3.0);
            let dir = (to - from).normalize_or(Vec3::X);
            let wiggle = (self.clock.time * 22.0).sin() * 0.7;
            let size = size * (1.0 - 0.6 * smooth((k - 0.7) / 0.3));
            angler::emit_fish(out, pos, dir + Vec3::Y * (1.0 - 2.0 * k), size, wiggle, SPECIES[species].tint, light(pos));
        }
        // The loose end of a parted line, whipping back and falling.
        if let (Some((t, at)), Some(_)) = (self.tools.fishing.snapped, own) {
            let tip = self.rod_tip();
            let k = t / 0.8;
            let reach = (1.0 - k) * 3.0 + 0.4;
            let dir = (at - tip).normalize_or(Vec3::X);
            let end = tip + dir * reach * (1.0 - 0.6 * k) - Vec3::Y * (1.5 * k * k + 0.3);
            emit_line(out, tip, end, cam, 0.3 + 0.8 * k, light(tip));
        }
    }

    /// Bottom right while the bobber is on the water: the reel's handle, its gears (a
    /// highlight sliding to the one it is in) and the line out; under the crosshair the cast's strength while drawing back, and the fight's
    /// tension bar. Everything moves smoothly (`smooth_fishing`).
    pub(in crate::client) fn draw_fishing_hud(&mut self) {
        if !self.holding_rod() {
            return;
        }
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        let f = &self.tools.fishing;
        let line = f.line.as_ref();
        let fight = line.and_then(|l| l.fight.as_ref());
        let fade = smooth(f.hud_in);
        let (gear_at, tension, length) = (f.hud_gear, f.hud_tension.0.max(0.0), f.hud_len);
        let (fight_k, fresh, charge) = (smooth(f.hud_fight), f.hud_fresh, f.hud_charge);
        let (crank, flash, alarm) = (f.crank, f.gear_flash, f.alarm);
        let toast = f.toast.clone();
        let (over, slack) = fight.map_or((0.0, 0.0), |fi| (fi.over / OVER_TIME, fi.slack / SLACK_TIME));
        let time = self.clock.time;
        let ui = &mut self.ui;
        let a = |c: Color, k: f32| with_alpha(c, k * fade);
        // A round dot (for arcs and caps).
        let dot = |ui: &mut crate::ui::Ui, p: Vec2, d: f32, c: Color| {
            ui.rect(p.x - d * 0.5, p.y - d * 0.5, d, d, c, d * 0.5);
        };

        let span = 1.1f32;
        // The reel's panel (only with the bobber on the water).
        if fade > 0.01 {
            // The panel.
            let (pw, ph) = (138.0 * s, 46.0 * s);
            let (x0, y0) = (w - pw - 10.0 * s, h - ph - 36.0 * s + (1.0 - fade) * 12.0 * s);
            ui.rect_full(x0, y0, pw, ph, a(rgba(0, 0, 0, 90), 1.0), a(rgba(0, 0, 0, 90), 1.0), 9.0 * s, 6.0 * s);
            ui.rect_full(x0, y0, pw, ph, a(rgba(26, 32, 42, 205), 1.0), a(rgba(12, 15, 20, 215), 1.0), 8.0 * s, 0.0);

            // The reel's handle, turning as it is wound.
            let c = Vec2::new(x0 + 26.0 * s, y0 + ph * 0.5);
            // The handle.
            let hub = 14.0 * s;
            ui.ring(c, hub, 1.5 * s, a(rgba(150, 158, 170, 220), 1.0));
            let arm = Vec2::new(crank.cos(), crank.sin());
            for i in 0..=8 {
                dot(ui, c + arm * hub * i as f32 / 8.0, 2.0 * s, a(rgba(220, 226, 234, 255), 1.0));
            }
            dot(ui, c + arm * hub, 5.0 * s, a(rgba(255, 210, 70, 255), 1.0));
            dot(ui, c, 4.0 * s, a(rgba(40, 44, 52, 255), 1.0));

            // The gears: the highlight slides to the one the reel is in (and swells a little when
            // it has just been changed).
            let gx = |g: f32| x0 + 60.0 * s + (g - 1.0) * 15.0 * s;
            let gy = y0 + 9.0 * s;
            let swell = 1.0 + 0.25 * (flash / 0.4).clamp(0.0, 1.0);
            let (gw, gh) = (12.0 * s * swell, 13.0 * s * swell);
            let hx = gx(gear_at);
            ui.rect(hx - gw * 0.5, gy + 4.0 * s - gh * 0.5, gw, gh, a(rgba(255, 206, 60, 255), 1.0), 3.0 * s);
            for g in 1..=ROD_GEARS {
                let near = (1.0 - (gear_at - g as f32).abs()).clamp(0.0, 1.0);
                let color = crate::ui::lerp_color(rgba(150, 158, 170, 255), rgba(40, 30, 6, 255), near);
                ui.text_centered(&g.to_string(), gx(g as f32) + 0.5 * s, gy, s, a(color, 1.0), false);
            }

            // The line out.
            let big = (s * 1.5).round().max(1.0);
            let dist = format!("{length:.1} m");
            let tw = ui.text_width(&dist, big);
            ui.text(&dist, x0 + pw - 10.0 * s - tw, y0 + 25.0 * s, big, a(WHITE, 1.0), true);
        }

        // Under the crosshair.
        let center = Vec2::new((w * 0.5).round(), (h * 0.5).round());
        let (bw, bh) = (120.0 * s, 8.0 * s);
        let (bx, by) = (center.x - bw * 0.5, center.y + 24.0 * s);
        // A smooth bar: thin slices blending from one colour to the next, rounded ends.
        let bar = |ui: &mut crate::ui::Ui, x: f32, y: f32, len: f32, hgt: f32, alpha: f32, color: &dyn Fn(f32) -> Color| {
            let n = ((len / s) as usize).max(2);
            let wd = len / n as f32;
            for i in 0..n {
                let k = (i as f32 + 0.5) / n as f32;
                let x1 = x + wd * i as f32;
                let rad = if i == 0 || i + 1 == n { hgt * 0.5 } else { 0.0 };
                let extra = if i + 1 < n { 0.6 } else { 0.0 };
                ui.rect(x1, y, wd + extra, hgt, with_alpha(color(k), alpha), rad.min(wd * 0.5));
            }
        };
        let shade = |k: f32| rgba(0, 0, 0, (140.0 * k) as u8);
        // Drawing back: the cast's strength.
        if charge > 0.01 && fight_k < 0.5 {
            let k = (1.0 - fight_k * 2.0).clamp(0.0, 1.0) * smooth((charge / 0.08).min(1.0));
            ui.rect_full(bx - 2.0 * s, by - 2.0 * s, bw + 4.0 * s, bh + 4.0 * s, shade(k), shade(k), (bh + 4.0 * s) * 0.5, 2.0 * s);
            let len = (bw * charge).max(bh);
            bar(ui, bx, by, len, bh, k, &|t| crate::ui::lerp_color(rgba(255, 236, 110, 255), rgba(255, 120, 40, 255), t * charge));
        }
        // The fight: the line's tension, to be kept in the green middle.
        if fight_k > 0.01 {
            let k = fight_k;
            let yb = by + (1.0 - k) * 8.0 * s;
            let x_of = |v: f32| bx + bw * (v / span).clamp(0.0, 1.0);
            ui.rect_full(bx - 3.0 * s, yb - 3.0 * s, bw + 6.0 * s, bh + 6.0 * s, shade(k), shade(k), (bh + 6.0 * s) * 0.5, 3.0 * s);
            bar(ui, bx, yb, bw, bh, 0.85 * k, &|t| tension_color(t * span));
            // The good part: a soft light frame round it.
            let (g0, g1) = (x_of(GOOD.0), x_of(GOOD.1));
            let lit = with_alpha(WHITE, 0.12 * k);
            ui.rect_full(g0 - 1.5 * s, yb - 2.0 * s, g1 - g0 + 3.0 * s, bh + 4.0 * s, lit, lit, 3.0 * s, 1.5 * s);
            // Warnings: the ends glow, faster and brighter as time runs out there.
            let glow = |v: f32| 0.5 + 0.5 * (time * (5.0 + 9.0 * v)).sin();
            if over > 0.0 {
                let x1 = x_of(TIGHT);
                let c = with_alpha(rgba(255, 235, 220, 255), k * over * glow(over));
                ui.rect_full(x1, yb, bx + bw - x1, bh, c, c, bh * 0.5, 2.0 * s);
            }
            if slack > 0.0 {
                let x1 = x_of(SLACK);
                let c = with_alpha(rgba(220, 235, 255, 255), k * slack * glow(slack));
                ui.rect_full(bx, yb, x1 - bx, bh, c, c, bh * 0.5, 2.0 * s);
            }
            // The marker: a pill riding on the bar.
            let mx = x_of(tension);
            let under = with_alpha(rgba(0, 0, 0, 255), 0.45 * k);
            ui.rect_full(mx - 4.0 * s, yb - 5.0 * s, 8.0 * s, bh + 10.0 * s, under, under, 4.0 * s, 2.0 * s);
            ui.rect(mx - 2.5 * s, yb - 4.0 * s, 5.0 * s, bh + 8.0 * s, with_alpha(WHITE, k), 2.5 * s);
            // How much fight the fish has left.
            let sy = yb + bh + 6.0 * s;
            ui.rect(bx, sy, bw, 3.0 * s, with_alpha(rgba(0, 0, 0, 255), 0.5 * k), 1.5 * s);
            if fresh > 0.005 {
                let (c0, c1) = (with_alpha(rgba(255, 176, 80, 255), k), with_alpha(rgba(235, 120, 50, 255), k));
                ui.rect_full(bx, sy, (bw * fresh).max(3.0 * s), 3.0 * s, c0, c1, 1.5 * s, 0.0);
            }
        }
        // A bite: set the hook! (a pulsing "!")
        if alarm > 0.0 {
            let pulse = 0.5 + 0.5 * (time * 10.0).sin();
            let k = (alarm / 0.3).min(1.0);
            let size = (2.0 * s + (pulse * s).round()).max(1.0);
            ui.text_centered("!", center.x + 0.5 * s, center.y - 28.0 * s - pulse * 2.0 * s, size, with_alpha(rgba(255, 220, 60, 255), k), true);
        }
        if let Some((text, left, color)) = toast {
            let k = smooth((left / 0.5).min(1.0));
            ui.text_centered(&text, center.x, h - 78.0 * s + (1.0 - k) * 6.0 * s, s, with_alpha(color, k), true);
        }
    }
}


/// The colour of a line tension `v` (0 slack .. 1.1 parting): calm blue, green in the good
/// middle, amber getting tight, red past the limit, blended smoothly.
fn tension_color(v: f32) -> Color {
    let keys = [
        (0.0, rgba(70, 120, 215, 255)),
        (SLACK, rgba(80, 170, 220, 255)),
        ((GOOD.0 + GOOD.1) * 0.5, rgba(80, 215, 110, 255)),
        (TIGHT, rgba(245, 190, 60, 255)),
        (1.1, rgba(235, 55, 40, 255)),
    ];
    for w in keys.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if v <= b {
            return crate::ui::lerp_color(ca, cb, smooth(((v - a) / (b - a)).clamp(0.0, 1.0)));
        }
    }
    keys[keys.len() - 1].1
}

/// See `angler::emit_line`.
fn emit_line(out: &mut Vec<Vertex>, from: Vec3, to: Vec3, cam: Vec3, sag: f32, light: [u8; 4]) {
    angler::emit_line(out, from, to, cam, sag, light);
}
