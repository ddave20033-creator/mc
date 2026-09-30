//! What a player sends, handled with the world's rules. A player is believed what they may
//! do (`checks`): only where they stand, with items there are, dealing what their weapon can.

use super::checks::{self, known_item, valid_slot, valid_stack};
use super::peers::{CLOSED_Y, RED, WHITE};
use super::Server;
use crate::entity::ItemEntity;
use crate::net::{pose_flags, Msg};
use crate::world::*;
use glam::IVec3;

impl Server {
    pub(super) fn handle(&mut self, id: u8, m: Msg) {
        let joined = self.peers.iter().any(|p| p.id == id && p.joined);
        if !joined {
            if let Msg::Hello { proto, name, view } = m {
                self.welcome(id, proto, name, view);
            }
            return;
        }
        let pose = self.peers.iter().find(|p| p.id == id).and_then(|p| p.pose);
        // What a player does has to be where they stand (their latest pose: a player sends it
        // right before anything checked here).
        let feet = pose.map(|p| p.pos);
        let near_block = |p: IVec3| feet.is_some_and(|f| checks::block_near(f, p, checks::BLOCK_REACH));
        let near_hand = |at: glam::Vec3| feet.is_some_and(|f| checks::point_near(f, at, checks::HAND_REACH));
        match m {
            Msg::Pose(mut pose) => {
                // (a custom skin is in the player's own slot)
                pose.skin = if pose.skin >= 4 { 4 + id } else { pose.skin.min(3) };
                if !known_item(pose.held) {
                    pose.held = crate::item::NONE;
                }
                if let Some(p) = self.peer(id) {
                    p.pose = Some(pose);
                }
            }
            Msg::Place { p, b } => {
                if near_block(p) {
                    self.place_world(p, valid(b));
                }
                // The player guessed the result; make sure it matches.
                let actual = self.world.geti(p);
                self.send_to(id, &Msg::Blocks(vec![(p, actual)]));
            }
            Msg::Break { p, held, creative } => {
                let b = self.world.geti(p);
                if b != AIR && near_block(p) && known_item(held) {
                    // (mined as in creative, without drops, only by a player in creative)
                    let creative = creative && pose.is_some_and(|p| p.flags & pose_flags::CREATIVE != 0);
                    self.break_world(p, held, creative);
                    self.broadcast(&Msg::BreakFx { p, block: b }, Some(id));
                }
                let actual = self.world.geti(p);
                self.send_to(id, &Msg::Blocks(vec![(p, actual)]));
            }
            Msg::DropItem { pos, vel, stack, delay } => {
                if valid_stack(&stack) && near_hand(pos) {
                    self.add_item(ItemEntity::new(pos, vel, stack, delay.max(0.0)));
                }
            }
            Msg::Open { p } => {
                let open = if p.y == CLOSED_Y || !near_block(p) {
                    None
                } else {
                    // Make sure the block entity exists.
                    let b = self.world.geti(p);
                    if let Some(bench) = self.level.block_entities.benches.get(&p).filter(|_| is_gun_bench(b)) {
                        // What lies on the gun station, as it is now.
                        let msg = Msg::Bench { p, bench: bench.clone() };
                        self.send_to(id, &msg);
                    }
                    if is_chest(b) {
                        let (a, other) = self.chest_halves(p);
                        for q in std::iter::once(a).chain(other) {
                            self.level.block_entities.chests.entry(q).or_insert_with(|| Box::new([None; 27]));
                        }
                    }
                    Some(p)
                };
                if let Some(peer) = self.peer(id) {
                    peer.open = open;
                    peer.sent_container = None;
                }
            }
            Msg::Container { p, kind, slots } => self.player_container(id, p, kind, slots),
            Msg::Chat { text, .. } => {
                let name = self.player_name(id).unwrap_or_default();
                self.announce(format!("<{name}> {text}"), WHITE);
            }
            Msg::Command(line) => self.command(id, &line),
            Msg::Save(mut state) => {
                for s in &mut state.inventory {
                    if !valid_slot(s) {
                        *s = None;
                    }
                }
                if let Some(p) = self.peer(id) {
                    p.state = Some(state);
                }
            }
            Msg::Pause(on) => {
                if let Some(p) = self.peer(id) {
                    p.paused = on;
                }
            }
            Msg::Skin { png, .. } => {
                if id < crate::world::textures::tex::CUSTOM_SKIN_SLOTS && crate::world::textures::decode_skin_png(&png).is_ok() {
                    self.skins.insert(id, png.clone());
                    self.broadcast(&Msg::Skin { id, png }, Some(id));
                }
            }
            _ => {}
        }
    }

    /// A player changed the container they have open. Only the slots this player changed
    /// (from what they last got) are taken, so two players working in the same chest do not
    /// undo each other.
    fn player_container(&mut self, id: u8, p: IVec3, kind: u8, slots: Vec<crate::item::Slot>) {
        let theirs = self.peers.iter().any(|peer| peer.id == id && peer.open == Some(p));
        let current = self.container_msg(p);
        let fits = matches!(&current, Some(Msg::Container { kind: k, slots: now, .. }) if *k == kind && now.len() == slots.len());
        if !theirs || !fits || !slots.iter().all(valid_slot) {
            // (what they have is not what is there: the next tick sends it again)
            if let Some(peer) = self.peer(id).filter(|peer| peer.open == Some(p)) {
                peer.sent_container = None;
            }
            return;
        }
        let base = self.peer(id).and_then(|peer| peer.sent_container.as_deref().and_then(Msg::decode));
        let merged = match (base, current) {
            (Some(Msg::Container { p: bp, kind: bk, slots: before }), Some(Msg::Container { slots: now, .. }))
                if bp == p && bk == kind && before.len() == slots.len() =>
            {
                now.iter().zip(&before).zip(&slots).map(|((cur, was), theirs)| if theirs != was { *theirs } else { *cur }).collect()
            }
            _ => slots.clone(),
        };
        self.apply_container(p, kind, &merged);
        // What the player has now: if the merge differs, the next tick sends it.
        let theirs = Msg::Container { p, kind, slots }.encode();
        if let Some(peer) = self.peer(id) {
            peer.sent_container = Some(theirs);
        }
    }

    /// A command a player typed that is the world's (the time; their own run in their game),
    /// with cheats on; the answer goes to them.
    fn command(&mut self, id: u8, line: &str) {
        use crate::lang::{t, tf};
        let args: Vec<&str> = line.trim_start_matches('/').split_whitespace().collect();
        if !self.cheats {
            self.send_to(id, &Msg::Chat { text: t("cmd.no_cheats").to_string(), color: RED });
            return;
        }
        let answer = match args.as_slice() {
            ["time", "set", v] => match parse_time(v) {
                Some(ticks) => {
                    self.time_of_day = (ticks / 24000.0).rem_euclid(1.0);
                    Ok(tf("cmd.time_set", &[&(ticks as i32)]))
                }
                None => Err(tf("cmd.bad_time", &[v])),
            },
            ["time", "add", v] => match v.parse::<f32>() {
                Ok(ticks) => {
                    self.time_of_day = (self.time_of_day + ticks / 24000.0).rem_euclid(1.0);
                    Ok(tf("cmd.time_add", &[&(ticks as i32)]))
                }
                Err(_) => Err(tf("cmd.bad_number", &[v])),
            },
            ["time"] | ["time", "query", ..] => {
                let ticks = (self.time_of_day * 24000.0).round() as i32;
                Ok(tf("cmd.time_query", &[&ticks]))
            }
            ["save"] => {
                self.save_all();
                Ok(t("cmd.saved").to_string())
            }
            _ => Err(t("cmd.unknown").to_string()),
        };
        self.broadcast(&Msg::Time(self.time_of_day), None);
        let (text, color) = match answer {
            Ok(text) => (text, WHITE),
            Err(text) => (text, RED),
        };
        self.send_to(id, &Msg::Chat { text, color });
    }
}

/// A time of day as a command gives it: a name or Minecraft ticks.
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
