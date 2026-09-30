//! Felling on the server: a player's axe cuts into a trunk (their game works out the cut and
//! sends it); cut through, the tree falls over here and in everyone's game, and where it
//! lands its trunk lies, to be cut up; a stump struck comes apart.

use super::Server;
use crate::item::{ItemId, Stack, STICK};
use crate::net::Msg;
use crate::sim::felling::*;
use crate::world::mesh::Notch;
use crate::world::*;
use glam::{IVec3, Vec3};

impl Server {
    /// Player `id` cut into the trunk at `p` (with `held`, in creative or not): the cut is
    /// kept and shown to the others; deep enough, the tree falls.
    pub(super) fn player_notch(&mut self, id: u8, p: IVec3, notch: Notch, held: ItemId, creative: bool) {
        if !is_trunk(self.world.geti(p)) || self.world.notch(p).is_some_and(|n| n.felled) {
            self.send_to(id, &Msg::Notch { p, notch: self.world.notch(p) });
            return;
        }
        let notch = Notch {
            depth: notch.depth.clamp(0.0, FELL_DEPTH),
            height: notch.height.clamp(0.25, 0.75),
            felled: false,
            ..notch
        };
        if notch.depth >= FELL_DEPTH - 1e-3 {
            self.fell_tree(p, notch, held, creative);
        } else {
            self.world.set_notch(p, Some(notch));
            self.broadcast(&Msg::Notch { p, notch: Some(notch) }, Some(id));
        }
    }

    /// The trunk at `p` breaks at the cut: its tree falls over (`sim::felling::fell`).
    fn fell_tree(&mut self, p: IVec3, notch: Notch, tool: ItemId, creative: bool) {
        let leaves = leaves_of(log_base(self.world.geti(p)));
        let gen = self.gen.clone();
        let tint = |q: IVec3| match tint_kind(leaves, 0) {
            TintKind::Foliage => gen.tints(q.x, q.z).1,
            TintKind::Spruce => SPRUCE_TINT,
            TintKind::Birch => BIRCH_TINT,
            _ => [255; 3],
        };
        let (mut tree, leaf, wood) = fell(&self.world, p, notch, tool, creative, tint);
        for &q in leaf.iter().chain(&wood) {
            self.set_block(q, AIR);
        }
        for &q in &wood {
            self.block_updated(q);
        }
        let stump = Notch { felled: true, ..notch };
        self.world.set_notch(p, Some(stump));
        self.broadcast(&Msg::Notch { p, notch: Some(stump) }, None);
        self.level.next_tree_id += 1;
        tree.id = self.level.next_tree_id;
        self.broadcast(&Msg::TreeFalls(Box::new(tree.clone())), None);
        self.level.falling_trees.push(tree);
    }

    /// The falling trees go over until they hit the ground; then they break up.
    pub(super) fn update_falling_trees(&mut self, dt: f32) {
        let mut landed = Vec::new();
        for (i, t) in self.level.falling_trees.iter_mut().enumerate() {
            if t.step(dt, &self.world) {
                landed.push(i);
            }
        }
        for i in landed.into_iter().rev() {
            let t = self.level.falling_trees.swap_remove(i);
            self.tree_lands(t);
        }
    }

    /// A tree still going over lands at once (the world is being saved).
    pub(super) fn land_now(&mut self, t: FallingTree) {
        self.tree_lands(t);
    }

    /// A fallen tree breaks up where it lies: its leaves and branches shatter, leaving little
    /// (a sapling or two from the leaves, a few sticks from the branches); its trunk stays
    /// lying there, to be cut up.
    fn tree_lands(&mut self, t: FallingTree) {
        self.broadcast(&Msg::TreeLands { id: t.id }, None);
        let turn = t.turn();
        let pieces: Vec<Block> = t.blocks[..t.trunk].iter().map(|&(_, b)| b).collect();
        let fell = t.fell_toward();
        self.lay_log(t.stump + fell * 0.5, fell, pieces, t.tool, t.creative);
        if t.creative {
            return;
        }
        let (mut leaves, mut branches) = (Vec::new(), Vec::new());
        for &(o, b) in t.blocks.iter().skip(t.trunk) {
            let mut at = t.at(&turn, o);
            // (up out of whatever it came down into)
            while is_solid(self.world.geti(at.floor().as_ivec3())) && at.y < t.pivot.y + 30.0 {
                at.y += 1.0;
            }
            if is_leaves(b) {
                leaves.push(at);
            } else {
                branches.push(at);
            }
        }
        let sapling = sapling_of(log_base(t.stub.1)) as ItemId;
        let r = self.random();
        let saplings = if leaves.is_empty() || r < 0.4 { 0 } else if r < 0.85 { 1 } else { 2 };
        let r = self.random();
        let sticks = if branches.is_empty() { (r < 0.5) as usize } else { 1 + (r * 3.0) as usize };
        self.drop_among(&leaves, sapling, saplings);
        let from = if branches.is_empty() { leaves } else { branches };
        self.drop_among(&from, STICK, sticks);
    }

    /// `n` of `what` dropped, each at one of `from` (as it happens).
    fn drop_among(&mut self, from: &[Vec3], what: ItemId, n: usize) {
        for _ in 0..n {
            if from.is_empty() {
                return;
            }
            let at = from[(self.random() * from.len() as f32) as usize % from.len()];
            self.spawn_drop(at, Stack::one(what));
        }
    }

    /// A felled trunk comes to lie on the ground (`sim::felling::log_rest`); pieces that would
    /// go into something solid break off there and drop what they are.
    fn lay_log(&mut self, start: Vec3, dir: Vec3, pieces: Vec<Block>, tool: ItemId, creative: bool) {
        let Some((base, dir, free)) = log_rest(&self.world, start, dir, &pieces) else { return };
        let (lying, broken) = pieces.split_at(free);
        let lying = lying.to_vec();
        for (i, &b) in broken.iter().enumerate() {
            let at = base + dir * ((free + i) as f32 + 0.5);
            self.broadcast(&Msg::BreakFx { p: at.floor().as_ivec3(), block: b }, None);
            if !creative {
                let r = self.random();
                for s in crate::item::drops(b, tool, r) {
                    let mut at = at;
                    while is_solid(self.world.geti(at.floor().as_ivec3())) && at.y < base.y + 30.0 {
                        at.y += 1.0;
                    }
                    self.spawn_drop(at, s);
                }
            }
        }
        if !lying.is_empty() {
            self.level.next_log_id += 1;
            let id = self.level.next_log_id;
            let next = 1 + (self.random() * 3.0) as usize;
            self.level.lying_logs.push(LyingLog { id, base, dir, pieces: lying, next });
            self.send_logs();
        }
    }

    /// Everyone gets the lying trunks as they are now.
    pub(super) fn send_logs(&self) {
        self.broadcast(&Msg::Logs(self.level.lying_logs.clone()), None);
    }

    /// Player `id` struck the lying trunk `log` with an axe (`held`, in creative or not): the
    /// next piece comes off the end it was struck nearer and drops its logs.
    pub(super) fn cut_log(&mut self, id: u8, log: u32, from_base: bool, held: ItemId, creative: bool) {
        let feet = self.peers.iter().find(|p| p.id == id).and_then(|p| p.pose).map(|p| p.pos);
        let Some(i) = self.level.lying_logs.iter().position(|l| l.id == log) else {
            return self.send_logs();
        };
        let l = &self.level.lying_logs[i];
        let mid = l.base + l.dir * (l.len() * 0.5);
        if feet.is_none_or(|f| f.distance(mid) > l.len() * 0.5 + 8.0) {
            return self.send_logs();
        }
        let next = 1 + (self.random() * 3.0) as usize;
        let l = &mut self.level.lying_logs[i];
        let taken = l.taken(from_base);
        let at: Vec<Vec3> = taken.clone().map(|j| l.piece_middle(j)).collect();
        let off: Vec<Block> = l.pieces.drain(taken.clone()).collect();
        if taken.start == 0 {
            l.base += l.dir * taken.end as f32;
        }
        l.next = next;
        if self.level.lying_logs[i].pieces.is_empty() {
            self.level.lying_logs.remove(i);
        }
        for (&b, &p) in off.iter().zip(&at) {
            self.broadcast(&Msg::BreakFx { p: p.floor().as_ivec3(), block: b }, Some(id));
            if !creative {
                let r = self.random();
                for s in crate::item::drops(b, held, r) {
                    self.spawn_drop(p, s);
                }
            }
        }
        self.send_logs();
    }

    /// Player `id` struck the stump at `p` with an axe (`held`, in creative or not): the cut
    /// block and the trunk under it come apart into their logs.
    pub(super) fn break_stump(&mut self, id: u8, p: IVec3, held: ItemId, creative: bool) {
        let Some(column) = stump_of(&self.world, p) else { return };
        for q in column {
            let b = self.world.geti(q);
            self.break_world(q, held, creative);
            self.broadcast(&Msg::BreakFx { p: q, block: b }, Some(id));
        }
    }
}
