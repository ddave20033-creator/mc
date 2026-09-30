//! Chat commands.

use super::*;
use crate::item::{from_key, key, max_stack, Stack};
use crate::lang::tf;

fn parse_time(v: &str) -> Option<f32> {
    Some(match v {
        "day" => 1000.0,
        "noon" => 6000.0,
        "sunset" => 12000.0,
        "night" => 13000.0,
        "midnight" => 18000.0,
        "sunrise" => 23000.0,
        _ => v.parse::<f32>().ok()?,
    })
}

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
        // LAN player: the time belongs to the host's world.
        if self.is_client() && args.first() == Some(&"time") && self.cheats {
            self.send(crate::net::Msg::Command(line.to_string()));
            return;
        }
        let needs_cheats = !matches!(
            args.first(),
            Some(&"help") | Some(&"?") | Some(&"seed") | Some(&"save")
        );
        if needs_cheats && !self.cheats {
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
            ["time", "set", v] => match parse_time(v) {
                Some(ticks) => {
                    self.time_of_day = (ticks / 24000.0).rem_euclid(1.0);
                    self.say(tf("cmd.time_set", &[&(ticks as i32)]), chat::WHITE);
                }
                None => self.say(tf("cmd.bad_time", &[v]), chat::RED),
            },
            ["time", "add", v] => match v.parse::<f32>() {
                Ok(ticks) => {
                    self.time_of_day = (self.time_of_day + ticks / 24000.0).rem_euclid(1.0);
                    self.say(tf("cmd.time_add", &[&(ticks as i32)]), chat::WHITE);
                }
                Err(_) => self.say(tf("cmd.bad_number", &[v]), chat::RED),
            },
            ["time"] | ["time", "query", ..] => {
                let ticks = (self.time_of_day * 24000.0).round() as i32;
                self.say(tf("cmd.time_query", &[&ticks]), chat::WHITE);
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
            ["tp", coords @ ..] if coords.len() == 3 => match parse_pos(coords, self.player.pos) {
                Some(p) => {
                    self.player.pos = p;
                    self.player.start_tick();
                    self.player.vel = Vec3::ZERO;
                    self.fall_peak = p.y;
                    let f = |v: f32| format!("{v:.1}");
                    self.say(tf("cmd.tp", &[&f(p.x), &f(p.y), &f(p.z)]), chat::WHITE);
                }
                None => self.say(t("cmd.bad_coords"), chat::RED),
            },
            ["summon", kind, coords @ ..] => match MobKind::from_key(kind) {
                Some(kind) => match parse_pos(coords, self.player.pos) {
                    Some(p) => {
                        self.spawn_mob(kind, p);
                        self.say(tf("cmd.summon", &[&kind.name()]), chat::WHITE);
                    }
                    None => self.say(t("cmd.bad_coords"), chat::RED),
                },
                None => self.say(tf("cmd.bad_entity", &[kind]), chat::RED),
            },
            ["effect", "clear"] => {
                self.needs.poison = 0.0;
                self.needs.nausea = 0.0;
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
                        self.needs.add_effect(k, secs.clamp(1.0, 3600.0));
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
                self.health = 0.0;
                self.die("death.kill");
            }
            ["save"] => {
                self.save_world();
                // (the world itself is its server's to save)
                if self.local.is_some() {
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
