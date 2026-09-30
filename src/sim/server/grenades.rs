//! Grenades on the server: its copy of each decides where a frag grenade goes off, which
//! blocks the blast takes, and who gets hurt; everyone's game is told (`Msg::Blast`).

use super::Server;
use crate::item::{drops, tool_id, Tier, ToolKind};
use crate::net::{hurt, Msg};
use crate::sim::grenade::{blast_blocks, blast_hurt, fly, Grenade, GrenadeKind, SMOKE_TIME};
use crate::world::*;
use glam::Vec3;

impl Server {
    /// A player threw a grenade: it flies here too (the one that decides), and the others see
    /// it thrown.
    pub(super) fn thrown_grenade(&mut self, id: u8, kind: u8, pos: Vec3, vel: Vec3, seed: u32, fuse: f32) {
        self.grenades.push(Grenade::new(GrenadeKind::from_u8(kind), pos, vel, seed, fuse));
        self.broadcast(&Msg::Grenade { id, kind, pos, vel, seed, fuse }, Some(id));
    }

    /// Grenades fly, bounce and go off; a smoke grenade smokes for a while and is gone.
    pub(super) fn update_grenades(&mut self, dt: f32) {
        let mut blasts = Vec::new();
        for g in &mut self.grenades {
            fly(g, dt, &self.world);
            g.fuse -= dt;
            match g.kind {
                GrenadeKind::Frag if g.fuse <= 0.0 => blasts.push((g.pos, g.seed)),
                GrenadeKind::Smoke if g.fuse <= 0.0 => {
                    let left = g.smoke.get_or_insert(SMOKE_TIME);
                    *left -= dt;
                }
                _ => {}
            }
        }
        self.grenades.retain(|g| !(g.kind == GrenadeKind::Frag && g.fuse <= 0.0) && g.smoke.is_none_or(|s| s > 0.0));
        for (pos, seed) in blasts {
            self.explode(pos, seed);
            self.broadcast(&Msg::Blast { pos, seed }, None);
        }
    }

    /// A frag grenade goes off at `pos`: the blocks of its ragged ball go (about a third of
    /// them drop), and everyone around gets hurt.
    fn explode(&mut self, pos: Vec3, seed: u32) {
        let blocks = blast_blocks(&self.world, pos, seed);
        let pick = tool_id(ToolKind::Pickaxe, Tier::Diamond);
        for q in &blocks {
            let b = self.world.geti(*q);
            self.set_block(*q, AIR);
            if self.random() < 0.3 {
                let r = self.random();
                for s in drops(b, pick, r) {
                    self.spawn_drop(q.as_vec3() + Vec3::splat(0.5), s);
                }
            }
        }
        for q in &blocks {
            self.block_updated(*q);
        }
        let world = &self.world;
        for m in self.level.mobs.iter_mut().filter(|m| m.alive()) {
            if let Some((dmg, knock)) = blast_hurt(world, pos, m.center()) {
                m.hurt(dmg, Some(pos), knock);
            }
        }
        let players: Vec<(u8, f32, f32)> = self
            .peers
            .iter()
            .filter_map(|p| p.alive_pose().map(|pose| (p.id, pose.pos)))
            .filter_map(|(id, at)| blast_hurt(world, pos, at + Vec3::Y * 0.9).map(|(d, k)| (id, d, k)))
            .collect();
        for (id, dmg, knock) in players {
            self.send_to(id, &Msg::Hurt { dmg, from: pos, knock, kind: hurt::BLAST });
        }
    }
}
