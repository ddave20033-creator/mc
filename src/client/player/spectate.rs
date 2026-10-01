//! Spectator mode: flying through blocks without touching anything, and watching another
//! LAN player through their eyes (picked from a menu, like Minecraft's spectator menu).

use crate::client::{Game, MAX_AIR, Screen};
use crate::client::player::GameMode;
use crate::entity::player::MoveInput;
use crate::keys::Bind;
use crate::lang::{t, tf};
use crate::sim::clock::TICK_SECS;
use crate::ui::{WHITE, chat, rgba};
use glam::Vec3;

impl Game {
    /// Switches this player's game mode (the /gamemode command).
    pub(in crate::client) fn set_game_mode(&mut self, mode: GameMode) {
        let was = self.me.mode;
        self.me.mode = mode;
        self.session.spectating = None;
        match mode {
            GameMode::Survival => {
                self.me.body.flying = false;
                self.me.body.noclip = false;
            }
            GameMode::Creative => {
                self.me.vitals.fire = 0.0;
                self.me.body.noclip = false;
                // Out of spectator mode in the air: keep flying instead of falling.
                self.me.body.flying |= was == GameMode::Spectator;
            }
            GameMode::Spectator => {
                if self.me.vitals.sleep.is_some() {
                    self.wake_up();
                }
                self.me.vitals.fire = 0.0;
                self.me.vitals.air = MAX_AIR;
                self.me.vitals.hurt_time = 0.0;
                self.me.aim.let_go();
                self.me.body.flying = true;
                self.me.body.noclip = true;
            }
        }
    }

    /// The name of a game mode, for the chat.
    pub(in crate::client) fn mode_name(mode: GameMode) -> &'static str {
        match mode {
            GameMode::Survival => t("mode.survival_long"),
            GameMode::Creative => t("mode.creative_long"),
            GameMode::Spectator => t("mode.spectator_long"),
        }
    }

    /// The players a spectator can watch: everyone else who is in the world (not dead and
    /// not spectating themselves), in the order they joined.
    fn spectate_candidates(&self) -> Vec<(u8, String)> {
        let mut v: Vec<(u8, String)> = self
            .session.remotes
            .iter()
            .filter(|r| r.alive())
            .map(|r| (r.id, r.name.clone()))
            .collect();
        v.sort_by_key(|(id, _)| *id);
        v
    }

    pub(in crate::client) fn open_spectate_menu(&mut self) {
        self.screen = Screen::Spectate;
        self.set_grab(false);
        self.input.keys.clear();
    }

    /// The i-th player of the menu (number keys).
    pub(in crate::client) fn spectate_nth(&mut self, i: usize) {
        if let Some((id, _)) = self.spectate_candidates().get(i).cloned() {
            self.start_spectating(id);
        }
    }

    /// Puts the camera into another player's eyes.
    fn start_spectating(&mut self, id: u8) {
        let Some(name) = self
            .session.remotes
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.name.clone())
        else {
            return;
        };
        self.session.spectating = Some(id);
        self.say(tf("spectate.now", &[&name]), chat::GRAY);
        if self.screen == Screen::Spectate {
            self.resume();
        }
    }

    /// Back to flying freely, where the watched player was.
    fn stop_spectating(&mut self) {
        if self.session.spectating.take().is_some() {
            self.me.body.vel = Vec3::ZERO;
            self.say(t("spectate.stopped"), chat::GRAY);
        }
    }

    /// The /spectate command: watch a player by name (without a name: stop watching).
    pub(in crate::client) fn spectate_command(&mut self, name: Option<&str>) {
        if !self.spectator() {
            self.say(t("spectate.not_spectator"), chat::RED);
            return;
        }
        let Some(name) = name else {
            self.stop_spectating();
            return;
        };
        let found = self
            .session.remotes
            .iter()
            .filter(|r| r.alive())
            .find(|r| r.name.eq_ignore_ascii_case(name))
            .map(|r| r.id);
        match found {
            Some(id) => self.start_spectating(id),
            None => self.say(tf("spectate.no_player", &[&name]), chat::RED),
        }
    }

    /// Spectator movement: flying through blocks, or riding along in the watched player's
    /// eyes (sneak lets go of them).
    pub(in crate::client) fn update_spectator(&mut self, dt: f32, control: bool) {
        self.me.body.flying = true;
        self.me.body.noclip = true;
        self.me.aim.let_go();
        self.me.hand.blocking = false;
        self.me.vitals.fire = 0.0;
        self.me.vitals.air = MAX_AIR;
        self.me.vitals.hurt_time = 0.0;
        self.me.vitals.fall_peak = self.me.body.pos.y;
        // (bullets, cases and grenades in the world go on in `update_world`)
        self.update_guns(dt, false);

        if let Some(id) = self.session.spectating {
            if control && self.sneaking() {
                self.stop_spectating();
            } else {
                match self.session.remotes.iter().find(|r| r.id == id) {
                    Some(r) if r.alive() => {
                        let p = r.pose;
                        self.me.body.pos = p.pos;
                        self.me.body.vel = Vec3::ZERO;
                        self.me.body.crouch = p.crouch;
                        self.me.look.yaw = p.yaw;
                        self.me.look.pitch = p.pitch;
                        self.me.look.body_yaw = p.body_yaw;
                        self.me.look.limb_amount = 0.0;
                        return;
                    }
                    // Dead for now: wait where they fell until they are back.
                    Some(r) if r.dead() => return,
                    _ => {
                        self.session.spectating = None;
                        self.say(t("spectate.lost"), chat::GRAY);
                    }
                }
            }
        }

        self.me.look.body_yaw = self.me.look.yaw;
        self.me.look.limb_amount += (0.0 - self.me.look.limb_amount) * (crate::util::damp(10.0, dt));
    }

    /// A spectator's tick: flying about (not while watching someone: then it is where they are).
    pub(in crate::client) fn tick_spectator(&mut self, control: bool) {
        if self.session.spectating.is_some() {
            return;
        }
        let k = |b: Bind| control && self.bind_down(b);
        let axis = |a: bool, b: bool| (a as i32 - b as i32) as f32;
        let input = MoveInput {
            forward: axis(k(Bind::Forward), k(Bind::Back)),
            strafe: axis(k(Bind::Right), k(Bind::Left)),
            up: k(Bind::Jump),
            down: k(Bind::Sneak),
            sprint: k(Bind::Sprint) || (self.input.w_sprint && k(Bind::Forward)),
            sneak: false,
            using: false,
            aiming: false,
        };
        self.me.body.flying = true;
        self.me.body.noclip = true;
        self.me.body.update(TICK_SECS, &self.terrain.world, self.me.look.yaw, &input);
    }

    /// Instead of the hotbar: what spectator mode is doing and which keys work.
    pub(in crate::client) fn draw_spectator_hud(&mut self) {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        let watched = self
            .session.spectating
            .and_then(|id| self.session.remotes.iter().find(|r| r.id == id))
            .map(|r| r.name.clone());
        let (title, hint) = match &watched {
            Some(name) => (tf("spectate.watching", &[name]), t("spectate.hint_watching")),
            None => (t("mode.spectator_long").to_string(), t("spectate.hint")),
        };
        let fs = (s - 1.0).max(1.0);
        let tw = self.ui.text_width(&title, s).max(self.ui.text_width(hint, fs));
        let (bw, bh) = (tw + 16.0 * s, 12.0 * s + 10.0 * fs);
        let (bx, by) = (((w - bw) * 0.5).round(), (h - bh - 8.0 * s).round());
        self.ui.rect(bx, by, bw, bh, rgba(10, 12, 18, 150), 5.0 * s);
        self.ui
            .text_centered(&title, w * 0.5, by + 4.0 * s, s, rgba(170, 220, 255, 255), true);
        self.ui.text_centered(
            hint,
            w * 0.5,
            by + 6.0 * s + 9.0 * s,
            fs,
            rgba(200, 200, 200, 255),
            true,
        );
    }

    /// The spectator menu: a button for every player to watch.
    pub(in crate::client) fn spectate_screen(&mut self) {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        let players = self.spectate_candidates();
        let ui = &mut self.ui;
        ui.gradient(0.0, 0.0, w, h, rgba(0, 0, 0, 150), rgba(0, 0, 0, 110));
        ui.vignette(rgba(0, 0, 0, 120));
        let (bw, bh) = (200.0 * s, 20.0 * s);
        let x = (w * 0.5 - bw * 0.5).round();
        let rows = players.len().max(1) as f32 + 2.0;
        let mut y = ((h - rows * 24.0 * s) * 0.5).max(40.0 * s).round();
        ui.text_centered(t("spectate.title"), w * 0.5, y - 26.0 * s, s, WHITE, true);
        let mut pick = None;
        if players.is_empty() {
            // (this game's own world, not open to the LAN: nobody else can be in it)
            let msg = if self.session.local.is_some() && self.session.lan_address.is_none() {
                t("spectate.single")
            } else {
                t("spectate.none")
            };
            ui.text_centered(msg, w * 0.5, y + 6.0 * s, s, rgba(190, 190, 190, 255), true);
            y += 24.0 * s;
        }
        for (i, (id, name)) in players.iter().enumerate() {
            let label = if self.session.spectating == Some(*id) {
                format!("{}. {name}  ({})", i + 1, t("spectate.current"))
            } else if i < 9 {
                format!("{}. {name}", i + 1)
            } else {
                name.clone()
            };
            if ui.button(&label, x, y, bw, bh, true) {
                pick = Some(*id);
            }
            y += 24.0 * s;
        }
        y += 6.0 * s;
        let mut stop = false;
        if self.session.spectating.is_some() && ui.button(t("spectate.stop"), x, y, bw, bh, true) {
            stop = true;
        }
        if self.session.spectating.is_some() {
            y += 24.0 * s;
        }
        let back = ui.button(t("gui.done"), x, y, bw, bh, true);
        if let Some(id) = pick {
            self.start_spectating(id);
        } else if stop {
            self.stop_spectating();
            self.resume();
        } else if back {
            self.resume();
        }
    }
}
