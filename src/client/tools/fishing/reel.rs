//! Fishing played: holding the rod, the wheel and the reel's gears, casting, the bobber flying
//! and floating, the bites, the fight on the line, and a fish landed or lost.

use super::*;

impl Game {
    pub(super) fn holding_rod(&self) -> bool {
        self.held() == FISHING_ROD && !self.spectator() && self.me.body.spawned && self.screen != Screen::Dead
    }

    fn rod_stack(&self) -> Option<Stack> {
        self.me.items.held_stack().filter(|s| s.item == FISHING_ROD)
    }

    /// The gear the held rod's reel is in.
    fn rod_gear(&self) -> u8 {
        self.rod_stack().map_or(3, |s| rod_gear(&s))
    }

    /// Where the line leaves the rod (as it was drawn last frame, from the eyes or from
    /// outside), or about there.
    pub(super) fn rod_tip(&self) -> Vec3 {
        let tip = if self.me.look.first_person { self.tools.fishing.tip_fp } else { self.tools.fishing.tip_tp };
        let eye = self.eye();
        tip.or(self.tools.fishing.tip_tp)
            .filter(|t| t.distance(eye) < 4.0)
            .unwrap_or_else(|| eye + self.me.look.dir() * 1.8 + Vec3::Y * 0.6)
    }

    /// The mouse wheel with the rod in hand: Shift shifts the reel's gear; with the line out
    /// (or drawing back to cast) it turns the reel. Returns whether the wheel was used for it
    /// (otherwise it goes through the hotbar).
    pub(in crate::client) fn fishing_scroll(&mut self) -> bool {
        if !self.holding_rod() {
            self.tools.fishing.scroll = 0.0;
            return false;
        }
        let f = &mut self.tools.fishing;
        f.scroll += self.input.scroll;
        let whole = f.scroll.trunc();
        f.scroll -= whole;
        let n = whole as i32;
        if self.bind_down(Bind::Sneak) {
            // Away from you: up a gear.
            if n != 0 {
                let slot = self.me.items.hotbar_slot;
                if let Some(s) = self.me.items.inventory.slots[slot].as_mut() {
                    let before = rod_gear(s);
                    let g = (before as i32 + n).clamp(1, ROD_GEARS as i32) as u8;
                    set_rod_gear(s, g);
                    if g != before {
                        let pitch = 0.85 + 0.08 * g as f32;
                        self.audio.play_pitched(Sound::GearClick, None, 0.55, pitch);
                        self.tools.fishing.gear_flash = 0.4;
                    }
                }
            }
            return true;
        }
        if self.tools.fishing.line.is_some() || self.tools.fishing.charge.is_some() || self.tools.fishing.cast.is_some() {
            // Toward you (the wheel turned down) reels in.
            self.tools.fishing.notches -= n;
            return true;
        }
        self.tools.fishing.scroll = 0.0;
        false
    }

    /// Something the right button opens is under the crosshair (a rod does not cast then, and
    /// nothing is eaten or drunk).
    pub(in crate::client) fn opens_target(&self) -> bool {
        self.me.aim.target.is_some_and(|(hit, _)| opens_on_use(self.terrain.world.geti(hit)))
    }

    /// The rod in hand: casting, the bobber, bites and the fight. Putting it away cuts the
    /// line.
    pub(in crate::client) fn update_fishing(&mut self, dt: f32, control: bool) {
        let notches = std::mem::take(&mut self.tools.fishing.notches);
        {
            let f = &mut self.tools.fishing;
            f.gear_flash = (f.gear_flash - dt).max(0.0);
            f.alarm = (f.alarm - dt).max(0.0);
            if let Some((_, t, _)) = &mut f.toast {
                *t -= dt;
            }
            if f.toast.as_ref().is_some_and(|t| t.1 <= 0.0) {
                f.toast = None;
            }
            f.flying = f.flying.map(|(p, t, s, w)| (p, t + dt, s, w)).filter(|v| v.1 < 0.6);
            f.snapped = f.snapped.map(|(t, p)| (t + dt, p)).filter(|v| v.0 < 0.8);
            f.lift = f.lift.map(|t| t + dt).filter(|&t| t < LIFT_TIME);
        }
        if !self.holding_rod() {
            let f = &mut self.tools.fishing;
            f.line = None;
            f.charge = None;
            f.cast = None;
            f.lift = None;
            f.fight_k = 0.0;
            f.tension = 0.0;
            f.hud_in = 0.0;
            f.alarm = 0.0;
            f.hang = None;
            f.hang_tip = None;
            f.bob_draw = None;
            return;
        }
        // Drawing back, and letting go to cast.
        if control && self.input.right_pressed {
            let can_cast = self.tools.fishing.cast.is_none() && !self.opens_target() && self.me.aim.action_cooldown <= 0.0;
            match &mut self.tools.fishing.line {
                Some(line) if line.fight.is_none() => line.auto_reel = true,
                Some(_) => {}
                None if can_cast => {
                    self.tools.fishing.charge = Some(0.0);
                }
                None => {}
            }
        }
        if let Some(c) = self.tools.fishing.charge {
            if !control {
                self.tools.fishing.charge = None;
            } else if self.input.right_down {
                self.tools.fishing.charge = Some(c + dt);
            } else {
                self.tools.fishing.charge = None;
                self.tools.fishing.cast = Some(0.0);
                self.tools.fishing.cast_power = smooth(c / CHARGE_TIME).max(0.05);
                self.audio.play(Sound::FishCast, None, 0.35 + 0.45 * self.tools.fishing.cast_power);
            }
        }
        if let Some(t) = self.tools.fishing.cast {
            let at = WHIP_FORWARD * CAST_TIME;
            if t < at && t + dt >= at {
                self.launch_bobber();
            }
            self.tools.fishing.cast = Some(t + dt).filter(|&t| t < CAST_TIME);
        }
        self.update_line(dt, notches);
        // What the rod shows.
        let f = &mut self.tools.fishing;
        let (fighting, want) = match &f.line {
            Some(Line { fight: Some(fi), .. }) => (true, fi.tension),
            Some(l) if l.dragged > 0.0 => (false, 0.2),
            Some(Line { bite: Bite::Strike(_), state: Bobber::Floating, .. }) => (false, 0.3),
            _ => (false, 0.0),
        };
        f.fight_k += ((fighting as i32 as f32) - f.fight_k) * (crate::util::damp(4.0, dt));
        f.tension += (want - f.tension) * (crate::util::damp(10.0, dt));
        self.smooth_fishing(dt);
    }

    /// What is drawn follows the game smoothly: the reel's handle, the bobber, the bobber
    /// swinging from the tip, and the HUD's values.
    fn smooth_fishing(&mut self, dt: f32) {
        let ease = |rate: f32| crate::util::damp(rate, dt);
        let gear = self.rod_gear() as f32;
        let tip = self.rod_tip();
        let f = &mut self.tools.fishing;
        f.crank += (f.crank_to - f.crank) * ease(14.0);

        // The bobber out on the line.
        match &f.line {
            Some(l) => {
                let to = l.bobber;
                f.bob_draw = match f.bob_draw {
                    Some(b) if l.state != Bobber::Flying && b.distance(to) < 4.0 => Some(b.lerp(to, ease(9.0))),
                    _ => Some(to),
                };
                f.hang = None;
            }
            None => f.bob_draw = None,
        }

        // Hanging from the tip: a pendulum on a short line, swung by the rod's moves.
        if f.line.is_none() {
            const HANG: f32 = 0.55;
            let (mut p, mut v) = f.hang.filter(|(p, _)| p.distance(tip) < 3.0).unwrap_or((tip - Vec3::Y * HANG, Vec3::ZERO));
            let steps = 4;
            let h = dt.min(0.05) / steps as f32;
            // (the tip's jolts smoothed out first, so it sways instead of flailing)
            let anchor = match f.hang_tip {
                Some(a) if a.distance(tip) < 3.0 => a.lerp(tip, ease(10.0)),
                _ => tip,
            };
            f.hang_tip = Some(anchor);
            for _ in 0..steps {
                v.y -= 12.0 * h;
                v *= (-4.5 * h).exp();
                let before = p;
                p += v * h;
                let d = p - anchor;
                if d.length() > HANG {
                    p = anchor + d.normalize() * HANG;
                }
                v = ((p - before) / h).clamp_length_max(2.5);
            }
            f.hang = Some((p, v));
        }

        // The HUD.
        let fight = f.line.as_ref().and_then(|l| l.fight.as_ref());
        let tension = fight.map_or(f.tension, |fi| fi.tension);
        // (the reel's panel only once the bobber is out on the water)
        let in_water = f.line.as_ref().is_some_and(|l| l.state == Bobber::Floating || l.fight.is_some());
        f.hud_in += ((in_water as i32 as f32) - f.hud_in) * ease(8.0);
        f.hud_gear += (gear - f.hud_gear) * ease(16.0);
        if (f.hud_gear - gear).abs() > 5.0 {
            f.hud_gear = gear;
        }
        // (a stiff, well damped spring: it keeps up, with a little life in it)
        let (x, v) = f.hud_tension;
        let a = (tension - x) * 260.0 - v * 26.0;
        let v = v + a * dt.min(0.05);
        f.hud_tension = (x + v * dt.min(0.05), v);
        let len = f.line.as_ref().map_or(0.0, |l| l.length);
        f.hud_len += (len - f.hud_len) * ease(10.0);
        f.hud_fight += ((fight.is_some() as i32 as f32) - f.hud_fight) * ease(7.0);
        f.hud_fresh += (fight.map_or(f.hud_fresh, |fi| fi.fresh()) - f.hud_fresh) * ease(6.0);
        let charging = f.charge.map(|c| smooth(c / CHARGE_TIME));
        f.hud_charge = match charging {
            Some(k) => k,
            None => f.hud_charge * (1.0 - ease(10.0)),
        };
    }

    /// The bobber leaves the rod's tip at the whip's furthest forward, as hard as it was
    /// drawn back, toward where the crosshair is.
    fn launch_bobber(&mut self) {
        let k = self.tools.fishing.cast_power;
        let (lo, hi) = CAST_SPEED;
        let speed = lo + (hi - lo) * k.powf(0.8);
        let look = self.me.look.dir();
        // Up a little over the look (a cast goes up and out).
        let flat = Vec3::new(look.x, 0.0, look.z).normalize_or(Vec3::X);
        let up = (look.y + 0.35).clamp(-0.3, 0.9);
        let dir = (flat + Vec3::Y * up).normalize();
        let from = self.rod_tip();
        self.tools.fishing.line = Some(Line {
            bobber: from,
            vel: dir * speed + self.me.body.vel * 0.5,
            state: Bobber::Flying,
            length: 0.0,
            bite: Bite::Wait(0.0),
            fight: None,
            auto_reel: false,
            dip: 0.0,
            dip_now: 0.0,
            dragged: 0.0,
        });
        self.audio.play(Sound::LineZip, None, 0.5);
        self.me.aim.action_cooldown = 0.3;
    }

    /// A new wait for a fish: shorter in deeper water.
    fn bite_wait(&mut self, at: Vec3) -> f32 {
        let d = depth(&self.terrain.world, at) as f32;
        (6.0 + 16.0 * self.random()) * (1.0 - 0.08 * d)
    }

    fn toast(&mut self, text: String, color: Color) {
        self.tools.fishing.toast = Some((text, 3.0, color));
    }

    /// The bobber flies, floats or lies; the reel pulls it in; fish bite; the fight goes on.
    fn update_line(&mut self, dt: f32, notches: i32) {
        let Some(mut line) = self.tools.fishing.line.take() else { return };
        let tip = self.rod_tip();
        let gear = self.rod_gear();
        let g = (gear - 1) as usize;
        let world = &self.terrain.world;
        // Where the bobber goes on its own.
        match line.state {
            Bobber::Flying => {
                line.vel.y -= GRAVITY * dt;
                line.vel *= (-0.25 * dt).exp();
                let mut p = line.bobber;
                let mut landed = false;
                for axis in [1, 0, 2] {
                    let mut q = p;
                    q[axis] += line.vel[axis] * dt;
                    if solid_at(world, q) {
                        if axis == 1 && line.vel.y < 0.0 {
                            landed = true;
                        }
                        line.vel[axis] = 0.0;
                    } else {
                        p = q;
                    }
                }
                line.bobber = p;
                if let Some(s) = water_surface(world, p).filter(|&s| p.y <= s) {
                    line.bobber.y = s;
                    let hit = line.vel.length();
                    line.vel = Vec3::ZERO;
                    line.state = Bobber::Floating;
                    line.dip_now = -0.12;
                    let wait = self.bite_wait(p);
                    line.bite = Bite::Wait(wait);
                    self.audio.play(Sound::BobberPlop, Some(p), (0.4 + hit / 20.0).min(1.0));
                    let (sky, blk) = self.terrain.world.light_estimate(p + Vec3::Y);
                    self.level.particles.splash(Vec3::new(p.x, s, p.z), 10, (hit / 25.0).min(1.0), sky, blk);
                } else if landed && line.vel.length() < 0.5 {
                    line.state = Bobber::Ground;
                    line.vel = Vec3::ZERO;
                } else if landed {
                    // Skidding along the ground.
                    line.vel.x *= 0.5;
                    line.vel.z *= 0.5;
                }
                // The line runs off the reel as it flies (to the end of it).
                let d = line.bobber.distance(tip);
                if d > MAX_LINE {
                    let out = (line.bobber - tip).normalize();
                    line.bobber = tip + out * MAX_LINE;
                    line.vel -= out * line.vel.dot(out).max(0.0);
                }
                line.length = line.length.max(d.min(MAX_LINE));
            }
            Bobber::Floating => {
                let world = &self.terrain.world;
                match water_surface(world, line.bobber) {
                    Some(s) if line.fight.is_none() => line.bobber.y = s,
                    Some(_) => {}
                    None if line.fight.is_none() => {
                        line.state = Bobber::Flying;
                        line.vel = Vec3::ZERO;
                    }
                    None => {}
                }
            }
            Bobber::Ground => {
                if !solid_at(&self.terrain.world, line.bobber - Vec3::Y * 0.1) {
                    line.state = Bobber::Flying;
                    line.vel = Vec3::ZERO;
                }
            }
        }

        // The reel.
        let fighting = line.fight.is_some();
        if notches != 0 {
            self.tools.fishing.crank_to += notches as f32 * 0.9;
            let pitch = 0.9 + 0.06 * gear as f32;
            if notches > 0 {
                self.audio.play_pitched(Sound::ReelClick, None, 0.45, pitch);
            } else {
                self.audio.play_pitched(Sound::LineZip, None, 0.25, pitch);
            }
        }
        if line.auto_reel && !fighting {
            let n = 14.0 * dt;
            line.length -= n;
            self.tools.fishing.crank_to += n * 1.5;
            if (self.clock.time * 8.0).fract() < dt * 8.0 {
                self.audio.play_pitched(Sound::ReelClick, None, 0.35, 1.2);
            }
        }
        if !fighting && notches != 0 {
            line.length -= notches as f32 * PER_NOTCH[g];
            // Reeling scares a nibbling fish off now and then.
            if notches > 0 {
                if let Bite::Nibble(..) = line.bite {
                    if self.random() < 0.5 {
                        line.bite = Bite::Wait(3.0 + 6.0 * self.random());
                    }
                }
            }
        }
        line.length = line.length.clamp(0.0, MAX_LINE);
        // Pulled in along the water or the ground when the line is shorter than the way to it.
        line.dragged = (line.dragged - dt).max(0.0);
        if !fighting && line.state != Bobber::Flying {
            let to = tip - line.bobber;
            let d = to.length();
            if d > line.length + 0.05 {
                let step = ((d - line.length) * (crate::util::damp(7.0, dt))).min(9.0 * dt);
                let flat = Vec3::new(to.x, 0.0, to.z);
                let mut q = line.bobber + flat.normalize_or_zero() * step.min(flat.length());
                let world = &self.terrain.world;
                if let Some(s) = water_surface(world, q + Vec3::Y * 0.3) {
                    q.y = s;
                    line.state = Bobber::Floating;
                } else if let Some(top) = ground_top(world, q + Vec3::Y * 0.3) {
                    q.y = top;
                    line.state = Bobber::Ground;
                } else if flat.length() < 0.3 || d > 0.0 {
                    // Over an edge or up to the rod: straight at it.
                    q = line.bobber + to / d * step;
                }
                line.bobber = q;
                line.dragged = 0.15;
                if line.state == Bobber::Floating && self.random() < dt * 12.0 {
                    let (sky, blk) = self.terrain.world.light_estimate(q + Vec3::Y);
                    self.level.particles.splash(q, 1, 0.1, sky, blk);
                }
            }
            let near = line.bobber.distance(tip);
            if near < 1.5 || line.length < 0.3 {
                // Reeled all the way in.
                self.audio.play_pitched(Sound::ReelClick, None, 0.5, 0.8);
                self.tools.fishing.line = None;
                return;
            }
        }

        // Fish.
        if line.state == Bobber::Floating && !fighting {
            let open = open_water(&self.terrain.world, line.bobber);
            match line.bite {
                Bite::Wait(secs) if open => {
                    let secs = secs - dt;
                    line.bite = if secs <= 0.0 { Bite::Nibble(1 + (self.random() * 3.0) as u8, 0.3) } else { Bite::Wait(secs) };
                }
                Bite::Wait(_) => {}
                Bite::Nibble(left, secs) => {
                    let secs = secs - dt;
                    if secs > 0.0 {
                        line.bite = Bite::Nibble(left, secs);
                    } else if left > 0 {
                        line.bite = Bite::Nibble(left - 1, 0.7 + 1.3 * self.random());
                        line.dip_now = 0.06 + 0.04 * self.random();
                        let p = line.bobber;
                        self.audio.play(Sound::FishNibble, Some(p), 0.6);
                        let (sky, blk) = self.terrain.world.light_estimate(p + Vec3::Y);
                        self.level.particles.splash(p, 3, 0.05, sky, blk);
                    } else {
                        // The bite: pulled right under.
                        line.bite = Bite::Strike(STRIKE_TIME);
                        let p = line.bobber;
                        self.audio.play(Sound::FishBite, Some(p), 1.0);
                        let (sky, blk) = self.terrain.world.light_estimate(p + Vec3::Y);
                        self.level.particles.splash(p, 14, 0.5, sky, blk);
                        self.tools.fishing.alarm = STRIKE_TIME;
                        self.toast(t("fish.bite").to_string(), rgba(255, 230, 120, 255));
                    }
                }
                Bite::Strike(secs) => {
                    if notches > 0 {
                        // Hooked!
                        let d = line.length.max(line.bobber.distance(tip));
                        let mut r = || self.rng.next();
                        line.fight = Some(Fight::new(d, &mut r));
                        line.bite = Bite::Wait(0.0);
                        self.tools.fishing.alarm = 0.0;
                        self.tools.fishing.toast = None;
                        let p = line.bobber;
                        self.audio.play(Sound::FishSplash, Some(p), 0.9);
                    } else if secs - dt <= 0.0 {
                        line.bite = Bite::Wait(self.bite_wait(line.bobber));
                        self.toast(t("fish.missed").to_string(), rgba(220, 220, 220, 255));
                    } else {
                        line.bite = Bite::Strike(secs - dt);
                    }
                }
            }
        }
        line.dip = match (line.bite, &line.fight) {
            (_, Some(_)) => 0.14,
            (Bite::Strike(_), _) => 0.3,
            _ => 0.0,
        };
        // A twitch jumps down at once and floats back up.
        line.dip_now += (line.dip - line.dip_now) * (crate::util::damp(6.0, dt));

        // The fight.
        if let Some(mut fight) = line.fight.take() {
            let mut r = || self.rng.next();
            let (reeled, end) = fight.step(dt, notches, gear, &mut r);
            if reeled > 0.0 {
                self.tools.fishing.crank_to += reeled * 0.5;
            }
            line.length = fight.dist;
            // It swims: across (turning about the angler) and at the length of the line.
            let origin = Vec3::new(tip.x, line.bobber.y, tip.z);
            let flat = Vec3::new(line.bobber.x - tip.x, 0.0, line.bobber.z - tip.z);
            let turn = fight.side * dt;
            let dir = Mat4::from_rotation_y(turn).transform_vector3(flat).normalize_or(Vec3::X);
            let drop = (tip.y - line.bobber.y).max(0.0);
            let across = (fight.dist * fight.dist - drop * drop).max(0.0).sqrt();
            let mut q = origin + dir * across;
            let world = &self.terrain.world;
            if let Some(s) = water_surface(world, q + Vec3::Y * 0.3) {
                q.y = s;
                line.state = Bobber::Floating;
            } else if let Some(top) = ground_top(world, q + Vec3::Y * 0.3) {
                // Dragged up the bank.
                q.y = top;
                line.state = Bobber::Ground;
            } else {
                q.y = line.bobber.y;
            }
            if solid_at(world, q + Vec3::Y * 0.1) {
                // Into a wall: it turns the other way.
                fight.side_to = -fight.side_to;
                fight.side = -fight.side * 0.5;
                q = Vec3::new(line.bobber.x, q.y, line.bobber.z);
            }
            line.bobber = q;
            // Thrashing at the surface, most in a surge and the more the bigger it is.
            let rate = if fight.surging() { 1.6 } else { 0.25 };
            if line.state == Bobber::Floating && self.random() < dt * rate {
                let big = (fight.weight / 8.0).min(1.0);
                self.audio.play(Sound::FishSplash, Some(q), 0.5 + 0.5 * big);
                let (sky, blk) = self.terrain.world.light_estimate(q + Vec3::Y);
                self.level.particles.splash(q, 6 + (big * 10.0) as usize, 0.3 + 0.5 * big, sky, blk);
            }
            match end {
                Some(FightEnd::Landed) => {
                    self.land_fish(&fight, line.bobber);
                    return;
                }
                Some(FightEnd::Snapped) => {
                    self.snap_line(line.bobber);
                    return;
                }
                Some(FightEnd::Escaped) => {
                    line.bite = Bite::Wait(self.bite_wait(line.bobber));
                    self.toast(t("fish.escaped").to_string(), rgba(220, 220, 220, 255));
                }
                None => line.fight = Some(fight),
            }
        }
        self.tools.fishing.line = Some(line);
    }

    /// Wears the held rod; it may break.
    fn wear_rod(&mut self, n: u16) {
        if self.creative() {
            return;
        }
        let slot = self.me.items.hotbar_slot;
        if damage(&mut self.me.items.inventory.slots[slot], n) {
            self.audio.play(Sound::LineSnap, None, 0.8);
            self.say(t("fish.rod_broke"), rgba(255, 170, 120, 255));
        }
    }

    /// A fish brought in: it flies out of the water to the player and is theirs.
    fn land_fish(&mut self, fight: &Fight, at: Vec3) {
        let sp = &SPECIES[fight.species];
        let name = if crate::app::lang::is_hungarian() { sp.hu } else { sp.en };
        let msg = crate::app::lang::tf("fish.caught", &[&kilos(fight.weight), &name]);
        self.say(msg.clone(), rgba(120, 220, 255, 255));
        self.toast(msg, rgba(150, 230, 255, 255));
        self.audio.play(Sound::FishLand, Some(at), 1.0);
        let (sky, blk) = self.terrain.world.light_estimate(at + Vec3::Y);
        self.level.particles.splash(at, 18, 0.7, sky, blk);
        let count = (1.0 + fight.weight / 4.0).min(4.0) as u8;
        self.give(Stack::new(RAW_FISH, count));
        self.tools.fishing.flying = Some((at, 0.0, fight.species, fight.weight));
        self.tools.fishing.lift = Some(0.0);
        self.tools.fishing.line = None;
        self.wear_rod(1);
    }

    /// The line parts: the fish is gone with the bobber.
    fn snap_line(&mut self, at: Vec3) {
        let tip = self.rod_tip();
        self.audio.play(Sound::LineSnap, Some(tip), 1.0);
        self.tools.fishing.snapped = Some((0.0, at));
        self.tools.fishing.line = None;
        self.toast(t("fish.snap").to_string(), rgba(255, 120, 100, 255));
        self.wear_rod(3);
    }
}
