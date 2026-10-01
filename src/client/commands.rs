//! Chat commands.

use super::*;
use crate::item::{from_key, key, max_stack, Stack};
use crate::lang::tf;

/// "x y z" relative to `here` where written with `~` ("~" alone, or "~2"); missing
/// coordinates stay at `here`.
fn parse_pos(args: &[&str], here: Vec3) -> Option<Vec3> {
    let mut p = here;
    for (i, s) in args.iter().take(3).enumerate() {
        p[i] = match s.strip_prefix('~') {
            Some("") => here[i],
            Some(rest) => here[i] + rest.parse::<f32>().ok()?,
            None => s.parse::<f32>().ok()?,
        };
    }
    Some(p)
}

impl Game {
    pub(super) fn run_command(&mut self, line: &str) {
        let Some(cmd) = line.strip_prefix('/') else {
            self.chat_line(line);
            return;
        };
        let args: Vec<&str> = cmd.split_whitespace().collect();
        // The time is the world's: its server sets it (and answers).
        if args.first() == Some(&"time") && self.me.cheats {
            self.send(crate::net::Msg::Command(line.to_string()));
            return;
        }
        let needs_cheats = !matches!(
            args.first(),
            Some(&"help") | Some(&"?") | Some(&"seed") | Some(&"save")
        );
        if needs_cheats && !self.me.cheats {
            self.say(t("cmd.no_cheats"), chat::RED);
            return;
        }
        match args.as_slice() {
            ["help"] | ["?"] => {
                self.say(t("cmd.help"), chat::GRAY);
                for (name, usage) in chat::COMMANDS {
                    if *name != "gm" {
                        self.say(*usage, chat::GRAY);
                    }
                }
            }
            ["gamemode" | "gm", m] => {
                let mode = match *m {
                    "survival" | "s" | "0" => Some(GameMode::Survival),
                    "creative" | "c" | "1" => Some(GameMode::Creative),
                    "spectator" | "sp" | "3" => Some(GameMode::Spectator),
                    _ => None,
                };
                match mode {
                    Some(mode) => {
                        self.set_game_mode(mode);
                        let name = Self::mode_name(mode);
                        self.say(tf("cmd.gamemode", &[&name]), chat::WHITE);
                    }
                    None => self.say(tf("cmd.bad_mode", &[m]), chat::RED),
                }
            }
            ["spectate"] => self.spectate_command(None),
            ["spectate", name @ ..] => {
                let name = name.join(" ");
                self.spectate_command(Some(&name));
            }
            ["give", item, rest @ ..] => match from_key(item) {
                Some(id) => {
                    let count = rest
                        .first()
                        .and_then(|c| c.parse::<u32>().ok())
                        .unwrap_or(1)
                        .clamp(1, 64 * 36);
                    let mut left = count;
                    while left > 0 {
                        let n = left.min(max_stack(id) as u32);
                        self.give(Stack::new(id, n as u8));
                        left -= n;
                    }
                    self.say(tf("cmd.give", &[&count, &key(id)]), chat::WHITE);
                }
                None => self.say(tf("cmd.bad_item", &[item]), chat::RED),
            },
            ["tp", coords @ ..] if coords.len() == 3 => match parse_pos(coords, self.me.body.pos) {
                Some(p) => {
                    self.me.body.pos = p;
                    self.me.body.start_tick();
                    self.me.body.vel = Vec3::ZERO;
                    self.me.vitals.fall_peak = p.y;
                    let f = |v: f32| format!("{v:.1}");
                    self.say(tf("cmd.tp", &[&f(p.x), &f(p.y), &f(p.z)]), chat::WHITE);
                }
                None => self.say(t("cmd.bad_coords"), chat::RED),
            },
            ["summon", kind, coords @ ..] => match MobKind::from_key(kind) {
                Some(kind) => match parse_pos(coords, self.me.body.pos) {
                    Some(p) => {
                        self.spawn_mob(kind, p);
                        self.say(tf("cmd.summon", &[&kind.name()]), chat::WHITE);
                    }
                    None => self.say(t("cmd.bad_coords"), chat::RED),
                },
                None => self.say(tf("cmd.bad_entity", &[kind]), chat::RED),
            },
            ["effect", "clear"] => {
                self.me.vitals.needs.poison = 0.0;
                self.me.vitals.needs.nausea = 0.0;
                self.say(t("cmd.effect_clear"), chat::WHITE);
            }
            ["effect", "give", name, rest @ ..] => {
                use crate::entity::survival::EffectKind;
                let kind = match *name {
                    "poison" => Some(EffectKind::Poison),
                    "nausea" => Some(EffectKind::Nausea),
                    _ => None,
                };
                let secs = rest
                    .first()
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(30.0);
                match kind {
                    Some(k) => {
                        self.me.vitals.needs.add_effect(k, secs.clamp(1.0, 3600.0));
                        self.say(tf("cmd.effect", &[&t(k.key())]), chat::WHITE);
                    }
                    None => self.say(tf("cmd.bad_effect", &[name]), chat::RED),
                }
            }
            ["spawn"] => {
                self.spawn_player();
                self.say(t("cmd.spawn"), chat::WHITE);
            }
            ["kill"] => {
                self.me.vitals.health = 0.0;
                self.die("death.kill");
            }
            ["save"] => {
                self.save_world();
                // (the world itself is its server's to save)
                if self.session.local.is_some() {
                    self.send(crate::net::Msg::Command(line.to_string()));
                }
                self.say(t("cmd.saved"), chat::WHITE);
            }
            ["seed"] => {
                let seed = self.terrain.gen.seed;
                self.say(tf("cmd.seed", &[&seed]), chat::WHITE);
            }
            _ => self.say(t("cmd.unknown"), chat::RED),
        }
    }
}
