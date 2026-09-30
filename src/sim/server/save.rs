//! The world on disk: loaded when the server starts, saved every minute, when the owner
//! pauses or leaves, and when the server stops. The owner's state goes into the world's own
//! files (its level file and inventory), the LAN players' into `players/`.

use super::Server;
use crate::item::{inventory, Slot, ARMOR_SLOTS};
use crate::net::{mode, PlayerState};
use crate::save::{self, PlayerSave};
use std::sync::Arc;

impl Server {
    /// Everything of the world `self.meta` from its folder: its edited chunks, block entities,
    /// saplings, dropped items, mobs, the cuts in its trunks.
    pub(super) fn load_world(&mut self) {
        let folder = self.meta.folder.clone();
        if folder.is_empty() {
            return;
        }
        for (pos, c) in save::load_chunks(&folder) {
            self.world.saved.insert(pos, Arc::new(c));
            self.world.modified.insert(pos);
        }
        save::load_entities(
            &folder,
            &mut self.level.block_entities,
            &mut self.level.saplings,
            &mut self.level.items,
            &mut self.level.mobs,
        );
        // Ids are not saved: every loaded mob and item gets a fresh one.
        for i in 0..self.level.mobs.len() {
            let id = self.entity_id();
            self.level.mobs[i].set_id(id);
        }
        for i in 0..self.level.items.len() {
            self.level.items[i].id = self.entity_id();
        }
        save::apply_notches(&mut self.world, &save::load_notches(&folder));
        self.level.lying_logs = crate::sim::felling::parse_logs(&save::load_logs(&folder));
        self.level.next_log_id = self.level.lying_logs.len() as u32;
    }

    /// Writes the world and its players to disk (the chunks on a thread of their own).
    pub(super) fn save_all(&mut self) {
        // (a world without a folder, the benchmark's, is not kept)
        if self.meta.folder.is_empty() {
            return;
        }
        self.save_players();
        let meta = &mut self.meta;
        meta.last_played = save::now_secs();
        meta.time_of_day = self.time_of_day;
        meta.spawn = Some(self.spawn);
        meta.save();
        let folder = meta.folder.clone();
        save::save_notches(&folder, &save::notches_text(&self.world));
        save::save_logs(&folder, &crate::sim::felling::logs_text(&self.level.lying_logs));
        save::save_entities(
            &folder,
            &self.level.block_entities,
            &self.level.saplings,
            &self.level.items,
            &self.level.mobs,
        );
        let world = &self.world;
        let chunks = world
            .modified
            .iter()
            .filter_map(|p| world.chunks.get(p).or_else(|| world.saved.get(p)).map(|c| (*p, c.clone())))
            .collect();
        self.saver.save(&folder, chunks);
        // (a save that failed: the last one's chunks are written on a thread)
        if let Some(e) = save::take_save_error() {
            let text = crate::lang::tf("save.failed", &[&e]);
            for p in self.peers.iter().filter(|p| p.owner) {
                p.conn.send(&crate::net::Msg::Chat { text: text.clone(), color: super::peers::RED });
            }
        }
    }

    /// The owner's state as the world's files keep it (None: they have not played it yet).
    pub(super) fn owner_state(&self) -> Option<PlayerState> {
        let p = self.meta.player.as_ref()?;
        let mut all: Vec<Slot> = vec![None; inventory::SIZE + ARMOR_SLOTS];
        save::load_inventory(&self.meta.folder, &mut all);
        Some(PlayerState {
            pos: glam::Vec3::from(p.pos),
            yaw: p.yaw,
            pitch: p.pitch,
            health: p.health,
            needs: p.needs.unwrap_or_else(|| crate::entity::survival::Needs::new().to_array()),
            mode: if self.meta.spectator {
                mode::SPECTATOR
            } else if self.meta.creative {
                mode::CREATIVE
            } else {
                mode::SURVIVAL
            },
            flying: p.flying,
            slot: p.slot as u8,
            inventory: all,
            bed: self.meta.bed,
        })
    }

    /// Keeps the owner's state in the world's files.
    pub(super) fn keep_owner_state(&mut self, s: &PlayerState) {
        if self.meta.folder.is_empty() {
            return;
        }
        let meta = &mut self.meta;
        meta.player = Some(PlayerSave {
            pos: s.pos.to_array(),
            yaw: s.yaw,
            pitch: s.pitch,
            health: s.health,
            flying: s.flying,
            slot: s.slot as usize,
            needs: Some(s.needs),
        });
        meta.bed = s.bed;
        meta.creative = s.mode == mode::CREATIVE;
        meta.spectator = s.mode == mode::SPECTATOR;
        save::save_inventory(&meta.folder, &s.inventory);
        meta.save();
    }
}
