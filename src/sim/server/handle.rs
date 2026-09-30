//! What a player sends, handled with the world's rules. A player is believed what they may
//! do (`checks`): only where they stand, with items there are, dealing what their weapon can.

use super::checks::{self, known_item, valid_slot, valid_stack};
use super::peers::{CLOSED_Y, RED, WHITE};
use super::Server;
use crate::entity::mob::{Foe, MobKind};
use crate::entity::ItemEntity;
use crate::net::{hurt, pose_flags, Msg};

/// Seconds after a shot its bullets may still hit (they fly a few hundred blocks at most).
const BULLET_TIME: f32 = 10.0;
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
        let owner = self.peers.iter().any(|p| p.id == id && p.owner);
        // What a player does has to be where they stand (their latest pose: a player sends it
        // right before anything checked here).
        let feet = pose.map(|p| p.pos);
        let near_block = |p: IVec3| feet.is_some_and(|f| checks::block_near(f, p, checks::BLOCK_REACH));
        let near_hand = |at: glam::Vec3| feet.is_some_and(|f| checks::point_near(f, at, checks::HAND_REACH));
        let held = pose.map_or(crate::item::NONE, |p| p.held);
        let from = feet.unwrap_or_default();
        match m {
            Msg::Pose(mut pose) => {
                // (a custom skin is in the player's own slot)
                pose.skin = if pose.skin >= 4 { 4 + id } else { pose.skin.min(3) };
                if !known_item(pose.held) {
                    pose.held = crate::item::NONE;
                }
                // (inside the world's bounds: far out, positions stop being exact)
                pose.pos = pose.pos.clamp(glam::Vec3::new(-3.0e7, -64.0, -3.0e7), glam::Vec3::new(3.0e7, 512.0, 3.0e7));
                if let Some(p) = self.peer(id) {
                    p.pose = Some(pose);
                }
            }
            Msg::Place { p, b } => {
                if !self.world.is_loaded(p.x, p.z) {
                    return;
                }
                if near_block(p) && may_place(self.world.geti(p), b, held) {
                    self.place_world(p, b);
                }
                // The player guessed the result; make sure it matches.
                let actual = self.world.geti(p);
                self.send_to(id, &Msg::Blocks(vec![(p, actual)]));
            }
            Msg::Break { p, held, creative } => {
                if !self.world.is_loaded(p.x, p.z) {
                    return;
                }
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
            Msg::AttackMob { id: mob, dmg, knock } => {
                let (melee, bullet) = self.damage_caps(id, held);
                let Some((dmg, knock)) = checks::clamp_hit(dmg, knock, melee.max(bullet)) else { return };
                // (within reach of the hand, or of the bullets of their last shot)
                let reach = if bullet > melee { 400.0 } else { 8.0 };
                if let Some(m) = self.level.mobs.iter_mut().find(|m| m.id == mob && m.pos.distance(from) < reach) {
                    if m.hurt(dmg, Some(from), knock) {
                        self.attacked(Foe::Mob(mob), id);
                    }
                }
            }
            Msg::AttackPlayer { id: target, dmg, knock, kind } => {
                // (a player hits with their hand or their bullets; blasts and bites are the
                // server's own)
                let (melee, bullet) = self.damage_caps(id, held);
                let cap = match kind {
                    hurt::MELEE => melee,
                    hurt::BULLET => bullet,
                    _ => return,
                };
                let Some((dmg, knock)) = checks::clamp_hit(dmg, knock, cap) else { return };
                self.attacked(Foe::Player(target), id);
                self.send_to(target, &Msg::Hurt { dmg, from, knock, kind });
            }
            Msg::Grenade { kind, pos, vel, seed, fuse, .. } => {
                if kind <= 1 && near_hand(pos) {
                    self.thrown_grenade(id, kind, pos, vel, seed, fuse.clamp(0.0, 10.0));
                }
            }
            Msg::SpawnMob { kind, pos } => {
                // A spawn egg puts it in front of the player; farther only by a command.
                let allowed = self.cheats || feet.is_some_and(|f| checks::point_near(f, pos, checks::SPAWN_REACH));
                if let (true, Some(kind)) = (allowed, MobKind::from_u8(kind)) {
                    self.spawn_mob(kind, pos);
                }
            }
            Msg::UseOnMob { id: mob, item } => {
                // (what they hold, near enough to reach)
                let near = |m: &crate::entity::mob::Mob| m.pos.distance(from) < 8.0;
                if let Some(i) = self.level.mobs.iter().position(|m| m.id == mob && near(m)).filter(|_| item == held) {
                    self.wolf_used(i, item, id);
                }
            }
            Msg::BreakDummy { id: mob } => {
                let creative = pose.is_some_and(|p| p.flags & pose_flags::CREATIVE != 0);
                if let Some(i) = self.level.mobs.iter().position(|m| m.id == mob && m.pos.distance(from) < 8.0) {
                    self.break_dummy(i, !creative);
                }
            }
            Msg::Shear { id: mob } => {
                let near = |m: &crate::entity::mob::Mob| m.pos.distance(from) < 8.0;
                if let Some(i) = self.level.mobs.iter().position(|m| m.id == mob && near(m)).filter(|_| held == crate::item::SHEARS) {
                    self.shear_mob(i);
                }
            }
            Msg::FurnaceUse { p, part, take, offered } => {
                if !valid_slot(&offered) {
                    return;
                }
                if near_block(p) {
                    self.use_furnace(id, p, part, take, offered);
                } else if let Some(st) = offered {
                    // (too far: what they offered goes back)
                    self.send_to(id, &Msg::Give(st));
                }
            }
            Msg::Bench { p, bench } => {
                // A player changed what lies on a gun station: the others see it too.
                let valid = bench.items.iter().all(|i| valid_stack(&i.stack)) && bench.loader_mag.as_ref().is_none_or(valid_stack);
                if valid && near_block(p) && is_gun_bench(self.world.geti(p)) {
                    self.set_bench(p, bench.clone());
                    self.broadcast(&Msg::Bench { p, bench }, Some(id));
                }
            }
            Msg::Shot { kind, mods, eye, seed, bullets, .. } => {
                // (the gun they hold)
                let Some(gun) = crate::item::GUN_KINDS.get(kind as usize).filter(|g| crate::item::GunKind::of(held) == Some(**g)) else {
                    return;
                };
                if !near_hand(eye) {
                    return;
                }
                // Its bullets may hit for a while (even after the gun is put away).
                let (damage, until) = (gun.stats().damage, self.time as f32 + BULLET_TIME);
                if let Some(p) = self.peer(id) {
                    p.shot_damage = (damage, until);
                }
                self.broadcast(&Msg::Shot { id, kind, mods, eye, seed, bullets }, Some(id));
            }
            Msg::Notch { p, notch: Some(n) } => {
                let creative = pose.is_some_and(|p| p.flags & pose_flags::CREATIVE != 0);
                if near_block(p) && known_item(held) && self.world.is_loaded(p.x, p.z) {
                    self.player_notch(id, p, n.angle, n.height, held, creative);
                } else {
                    // (their game made the cut already: as it really is)
                    self.send_to(id, &Msg::Notch { p, notch: self.world.notch(p) });
                }
            }
            Msg::Stump { p } => {
                let creative = pose.is_some_and(|p| p.flags & pose_flags::CREATIVE != 0);
                if near_block(p) && known_item(held) && self.world.is_loaded(p.x, p.z) {
                    self.break_stump(id, p, held, creative);
                }
            }
            Msg::CutLog { id: log, from_base } => {
                let creative = pose.is_some_and(|p| p.flags & pose_flags::CREATIVE != 0);
                if known_item(held) {
                    self.cut_log(id, log, from_base, held, creative);
                }
            }
            Msg::Edit(list) if owner => {
                for (p, b) in list {
                    self.set_block(p, b);
                    let entities = &mut self.level.block_entities;
                    if is_furnace(b) {
                        entities.furnaces.entry(p).or_default();
                    } else if is_chest(b) {
                        entities.chests.entry(p).or_insert_with(|| Box::new([None; 27]));
                    }
                }
            }
            Msg::DropItem { pos, vel, stack, delay } => {
                // (the world's owner anywhere: the testbed's scripts)
                if valid_stack(&stack) && (near_hand(pos) || owner) {
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
                // (the owner paused: a good moment to save)
                if on && owner {
                    self.save_all();
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

    /// The most damage player `id` can deal now: with what they hold (a critical hit, a
    /// bullet of the gun) or with the bullets of their last shot (`melee`, `bullet`).
    fn damage_caps(&mut self, id: u8, held: crate::item::ItemId) -> (f32, f32) {
        let now = self.time as f32;
        let shot = self.peer(id).map_or(0.0, |p| if now <= p.shot_damage.1 { p.shot_damage.0 } else { 0.0 });
        (checks::melee_cap(held), checks::gun_cap(held).max(shot))
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
        let owner = self.peers.iter().any(|p| p.id == id && p.owner);
        if args.as_slice() == ["save"] && owner {
            self.save_all();
            return;
        }
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
        if answer.is_ok() {
            self.broadcast(&Msg::Time(self.time_of_day), None);
        }
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

/// Whether a player holding `held` may put `b` where `current` is: over something replaceable
/// (never over a block with things in it), a block of what they hold (a door's, bed's or big
/// furnace's other part too), a bucket's fluid; or a door opened or closed, a bucket scooping
/// up a fluid, and the old one-block gun station turned into the two-block one.
fn may_place(current: Block, b: Block, held: crate::item::ItemId) -> bool {
    use crate::item::{item_of_block, BUCKET};
    if is_door(current) && base(current) == base(b) {
        return true;
    }
    if b == AIR {
        return held == BUCKET && is_fluid(current);
    }
    if is_gun_bench(b) {
        return current == GUN_STATION || is_replaceable(current);
    }
    is_replaceable(current) && item_of_block(b) == Some(held)
}
