//! Felling trees with an axe. Each chop is a swing of its own, as made in Blockbench
//! (`chop_rig`): the axe drawn back and swung round level. The axe's edge is followed along
//! its real path through the world as it swings, and only where it meets a trunk does it
//! bite in: it stops there, chips fly, and the cut (`mesh::Notch`) is made on that side at
//! that height, a little deeper with every chop (how much deeper, the axe decides). Deep enough, the trunk breaks there and the
//! tree above it (its trunk, branches and leaves) falls over as one, away from the player,
//! slowly at first and faster as it tips, until it hits the ground and breaks up into what
//! it drops.
//!
//! What is left of a felled tree is its stump (the cut block, and the trunk under it where
//! it was cut higher up). An axe takes it out in one stroke, whatever the axe: raised over
//! the head and brought straight down into it (`chop_rig::Kind::Stump`); when the axe is
//! pulled out again the stump comes apart into its logs.

use crate::game::*;
use crate::item::{inventory, tool_of};
use crate::model::chop_rig::{self, ChopPose, Kind, Swing, EDGE};
use crate::sim::felling::*;
use crate::world::mesh::Notch;

/// What the axe is stuck in, to come apart when it is pulled out: a stump, or a lying trunk
/// (which, and where it is cut).
#[derive(Clone, Copy, Debug)]
pub(in crate::game) enum Struck {
    Stump(IVec3),
    Log(u32, bool),
}

/// What the axe's edge came into.
enum Contact {
    Trunk(IVec3),
    Log(u32),
    Ground,
}

impl Game {
    /// A stroke of the axe (survival): it tires the player (`exhaust`) and wears the axe in
    /// hand; one that breaks bursts apart at `at`.
    pub(in crate::game) fn wear_axe(&mut self, exhaust: f32, at: IVec3) {
        self.needs.exhaust(exhaust);
        let slot = self.hotbar_slot;
        if tool_of(self.held()).is_some() && inventory::damage(&mut self.inventory.slots[slot], 1) {
            self.particles.burst(&self.terrain.world, at, STONE, 12, [255; 3]);
        }
    }

    /// Where the chop's rig is in the world (as the player model draws it).
    pub(in crate::game) fn chop_world(&self) -> Mat4 {
        chop_rig::to_world(self.player.pos, self.visual_head_yaw(), self.pitch)
    }

    /// The trunk the player is aiming an axe at, if any, and the swing it takes: a standing
    /// trunk is chopped level, a stump struck from above (felling is the host's; a LAN
    /// client mines trunks like any block).
    fn chop_target(&self) -> Option<Kind> {
        chops_needed(self.held())?;
        // (a lying trunk is cut up struck from above too)
        if self.log_aim.is_some() {
            return Some(Kind::Stump);
        }
        let (hit, _) = self.target?;
        let w = &self.terrain.world;
        if !is_trunk(w.geti(hit)) {
            return None;
        }
        Some(if stump_of(w, hit).is_some() { Kind::Stump } else { Kind::Chop })
    }

    /// Where the axe's edge first comes into a trunk's wood (standing, or lying) between the
    /// animation's times `t0` and `t1`: the time, what it came into and the point. Struck
    /// down, the ground stops it too.
    fn edge_contact(&self, kind: Kind, t0: f32, t1: f32) -> Option<(f32, Contact, Vec3)> {
        let world = self.chop_world();
        let w = &self.terrain.world;
        let steps = ((t1 - t0) / 0.004).ceil().max(1.0) as usize;
        for i in 1..=steps {
            let t = t0 + (t1 - t0) * i as f32 / steps as f32;
            let axe = world * ChopPose::at(kind, t).axe();
            for e in EDGE {
                let q = axe.transform_point3(e);
                if let Some(p) = in_trunk(w, q) {
                    return Some((t, Contact::Trunk(p), q));
                }
                if kind == Kind::Stump {
                    if let Some(l) = self.level.lying_logs.iter().find(|l| l.contains(q).is_some()) {
                        return Some((t, Contact::Log(l.id), q));
                    }
                    let b = w.geti(q.floor().as_ivec3());
                    if is_solid(b) && !is_leaves(b) && !is_log(b) {
                        return Some((t, Contact::Ground, q));
                    }
                }
            }
        }
        None
    }

    /// Chopping with an axe, instead of mining: a swing at a time while the button is held,
    /// the edge followed along its path; where it meets a trunk it bites in. True while it is
    /// going on (the normal mining is left out, and the hand is drawn by the rig).
    pub(in crate::game) fn update_chopping(&mut self, active: bool, dt: f32) -> bool {
        if let Some(mut sw) = self.chop {
            let times = sw.kind.times();
            let before = sw.anim_time();
            sw.clock += dt;
            let after = sw.anim_time();
            if sw.hit.is_none() && after > times.stroke && before < times.hit {
                if let Some((t, p, point)) = self.edge_contact(sw.kind, before.max(times.stroke), after.min(times.hit)) {
                    // Stuck where it bit in.
                    sw.hit = Some(t);
                    sw.clock = t;
                    match (sw.kind, p) {
                        (Kind::Chop, Contact::Trunk(p)) => self.chop_hit(p, point),
                        (Kind::Stump, Contact::Trunk(p)) => self.stump_hit(p, point),
                        (_, Contact::Log(id)) => self.log_hit(id, point),
                        (_, _) => self.ground_hit(point),
                    }
                }
            }
            // Pulled out of the stump (or the lying trunk), it comes apart.
            if sw.pulling() {
                self.come_apart();
            }
            self.chop = (!sw.done()).then_some(sw);
        }
        if self.chop.is_none() {
            self.come_apart();
            self.log_cut = None;
        }
        let target = if active { self.chop_target() } else { None };
        if self.chop.is_none() && (target.is_none() || !self.input.left_down) {
            self.hand.hidden = false;
            return target.is_some();
        }
        self.mining = None;
        if let (None, Some(kind)) = (self.chop, target) {
            self.chop = Some(Swing { kind, ..Swing::default() });
            self.log_cut = self.log_aim.map(|a| (a.id, a.from_base));
        }
        self.hand.hidden = true;
        true
    }

    /// What the axe was stuck in comes apart as it is pulled out.
    fn come_apart(&mut self) {
        match self.struck.take() {
            Some(Struck::Stump(p)) => self.break_stump(p),
            Some(Struck::Log(id, from_base)) => self.cut_log(id, from_base),
            None => {}
        }
    }

    /// Struck down into the stump at `p`: the edge stuck in it, chips flying up; it comes
    /// apart when the axe is pulled out (`break_stump`).
    fn stump_hit(&mut self, p: IVec3, point: Vec3) {
        let b = self.terrain.world.geti(p);
        self.chips(p, b, point, Vec3::Y);
        // (a standing trunk in the way only gives chips)
        if stump_of(&self.terrain.world, p).is_some() {
            self.struck = Some(Struck::Stump(p));
        }
    }

    /// The edge struck into the ground: a puff of it.
    fn ground_hit(&mut self, point: Vec3) {
        let q = point.floor().as_ivec3();
        let b = self.terrain.world.geti(q);
        let tint = self.block_tint(q, b);
        self.particles.impact(&self.terrain.world, point, Vec3::Y, b, tint);
    }

    /// Chips of the trunk at `p` flying out at `at` (the way `out`).
    pub(in crate::game) fn chips(&mut self, p: IVec3, b: Block, at: Vec3, out: Vec3) {
        let tint = self.block_tint(p, b);
        for _ in 0..3 {
            self.particles.impact(&self.terrain.world, at, out, b, tint);
        }
        let layer = face_texture(b, 2);
        let (sky, blk) = self.terrain.world.light_estimate(at);
        self.particles.crumbs(at, layer, 6, sky, blk);
    }

    /// The stump the axe was struck into comes apart: the cut block and the trunk under it
    /// drop their logs (the ground under it left bare, the grass to grow back), the axe worn
    /// by the one stroke.
    fn break_stump(&mut self, p: IVec3) {
        let Some(column) = stump_of(&self.terrain.world, p) else { return };
        let held = self.held();
        let creative = self.creative();
        if self.is_client() {
            self.send(crate::net::Msg::Stump { p });
            if !creative {
                self.wear_axe(crate::entity::survival::cost::MINE, p);
            }
            return;
        }
        for q in column {
            let b = self.terrain.world.geti(q);
            self.break_world(q, held, creative);
            self.break_fx(q, b, false, None);
        }
        if !creative {
            self.wear_axe(crate::entity::survival::cost::MINE, p);
        }
    }

    /// Lets a tree still going over land at once (the world is being left: its drops are
    /// not lost with it).
    pub(in crate::game) fn land_falling_trees(&mut self) {
        for t in std::mem::take(&mut self.level.falling_trees) {
            self.tree_lands(t);
        }
        self.come_apart();
        self.chop = None;
        self.log_cut = None;
    }

    /// The axe's edge has bitten into the trunk at `p` at `point`: the cut is made there, on
    /// the side of the trunk it came in from, at the height it hit; each chop takes as much
    /// more wood out as the axe cuts (a chop a little off the cut moves it that way, weighed
    /// by how much is cut already). Chips fly, and cut through far enough the tree falls.
    fn chop_hit(&mut self, p: IVec3, point: Vec3) {
        let b = self.terrain.world.geti(p);
        if self.terrain.world.notch(p).is_some_and(|n| n.felled) {
            // (a stump is not chopped level: only chips)
            let out = Vec2::new(self.player.pos.x - p.x as f32 - 0.5, self.player.pos.z - p.z as f32 - 0.5).normalize_or_zero();
            self.chips(p, b, point, Vec3::new(out.x, 0.3, out.y));
            return;
        }
        let creative = self.creative();
        let chops = if creative { 4.0 } else { chops_needed(self.held()).unwrap_or(10.0) };
        let middle = p.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
        // Where it bit in: round the trunk from its middle toward the point hit (or the
        // player, hit straight on), and how high.
        let side = Vec2::new(point.x - middle.x, point.z - middle.z);
        let side = if side.length() > 0.05 { side } else { Vec2::new(self.player.pos.x - middle.x, self.player.pos.z - middle.z) };
        let (angle, height) = (side.y.atan2(side.x), (point.y - p.y as f32).clamp(0.25, 0.75));
        let step = FELL_DEPTH / chops;
        let notch = match self.terrain.world.notch(p) {
            Some(n) => {
                let k = step / (n.depth + step);
                let turn = (angle - n.angle + PI).rem_euclid(TAU) - PI;
                Notch { angle: n.angle + turn * k, height: n.height + (height - n.height) * k, ..n }
            }
            None => Notch { angle, height, depth: 0.0, felled: false },
        };
        let depth = notch.depth + step;
        // Chips out of the cut, toward the player.
        let out = Vec3::new(notch.angle.cos(), 0.0, notch.angle.sin());
        let r = log_radius(b) * (1.0 - notch.depth * 1.6).max(0.2);
        let at = middle + out * r + Vec3::Y * notch.height;
        self.chips(p, b, at, out);
        if !creative {
            self.wear_axe(crate::entity::survival::cost::MINE * 0.5, p);
        }
        let notch = Notch { depth, ..notch };
        if self.is_client() {
            // The server fells it (cut through, it answers with the stump and the tree
            // falling); meanwhile the cut shows.
            self.terrain.world.set_notch(p, Some(notch));
            self.send(crate::net::Msg::Notch { p, notch: Some(notch) });
        } else if depth >= FELL_DEPTH - 1e-3 {
            self.fell_tree(p, notch);
        } else {
            self.terrain.world.set_notch(p, Some(notch));
        }
        self.terrain.block_changed(p, false);
    }

    /// The trunk at `p` breaks at the cut: everything of its tree above it comes away and
    /// falls over (`sim::felling::fell`). Below the cut its stump is left.
    fn fell_tree(&mut self, p: IVec3, notch: Notch) {
        let (tool, creative) = (self.held(), self.creative());
        let leaves = leaves_of(log_base(self.terrain.world.geti(p)));
        let (mut tree, leaf, wood) = fell(&self.terrain.world, p, notch, tool, creative, |q| self.block_tint(q, leaves));
        for &q in leaf.iter().chain(&wood) {
            self.set_block(q, AIR);
        }
        for &q in &wood {
            self.block_updated(q);
        }
        self.terrain.world.set_notch(p, Some(Notch { felled: true, ..notch }));
        self.level.next_tree_id += 1;
        tree.id = self.level.next_tree_id;
        self.level.falling_trees.push(tree);
    }

    /// The falling trees go over (like a pole tipping about its foot) until one of them
    /// hits something solid (leaves do not stop it) or lies flat; then it breaks up into
    /// its drops with a crash.
    pub(in crate::game) fn update_falling_trees(&mut self, dt: f32) {
        let mut landed = Vec::new();
        for (i, t) in self.level.falling_trees.iter_mut().enumerate() {
            if t.step(dt, &self.terrain.world) {
                landed.push(i);
            }
        }
        for i in landed.into_iter().rev() {
            let t = self.level.falling_trees.swap_remove(i);
            self.tree_lands(t);
        }
    }

    /// The server says a falling tree is down: it breaks up in a burst of bark and leaves.
    pub(in crate::game) fn tree_landed(&mut self, id: u32) {
        let Some(i) = self.level.falling_trees.iter().position(|t| t.id == id) else { return };
        let t = self.level.falling_trees.swap_remove(i);
        self.crash_fx(&t);
    }

    /// The bark and leaves flying where a fallen tree breaks up.
    fn crash_fx(&mut self, t: &FallingTree) {
        let turn = t.turn();
        for (n, &(o, b)) in t.blocks.iter().enumerate().skip(t.trunk) {
            let q = t.at(&turn, o).floor().as_ivec3();
            let tint = if is_leaves(b) { t.leaf_tint } else { [255; 3] };
            if !is_leaves(b) || n % 3 == 0 {
                self.particles.burst(&self.terrain.world, q, b, if is_leaves(b) { 3 } else { 6 }, tint);
            }
        }
    }

    /// A LAN player's (or the server's player's) copies of the falling trees go over, as the
    /// server's do, until it says they are down.
    pub(in crate::game) fn fall_trees_here(&mut self, dt: f32) {
        for t in &mut self.level.falling_trees {
            let angle = t.angle;
            if t.step(dt, &self.terrain.world) {
                // (it waits lying there for the word)
                (t.angle, t.speed) = (angle, 0.0);
            }
            t.prev_angle = t.angle;
        }
    }

    /// A fallen tree breaks up where it lies: its leaves and branches shatter in a burst of
    /// bark and leaves, leaving little (a sapling or two from the leaves, a few sticks from
    /// the branches); its trunk stays lying there, to be cut up.
    fn tree_lands(&mut self, t: FallingTree) {
        let turn = t.turn();
        // The trunk from just past the stump, the way it fell (the ground's lie of it).
        let pieces: Vec<Block> = t.blocks[..t.trunk].iter().map(|&(_, b)| b).collect();
        let fell = t.fell_toward();
        self.lay_log(t.stump + fell * 0.5, fell, pieces, t.tool, t.creative);
        let (mut leaves, mut branches) = (Vec::new(), Vec::new());
        for (n, &(o, b)) in t.blocks.iter().enumerate().skip(t.trunk) {
            let mut at = t.at(&turn, o);
            let q = at.floor().as_ivec3();
            let tint = if is_leaves(b) { t.leaf_tint } else { [255; 3] };
            if !is_leaves(b) || n % 3 == 0 {
                self.particles.burst(&self.terrain.world, q, b, if is_leaves(b) { 3 } else { 6 }, tint);
            }
            // (up out of whatever it came down into)
            while is_solid(self.terrain.world.geti(at.floor().as_ivec3())) && at.y < t.pivot.y + 30.0 {
                at.y += 1.0;
            }
            if is_leaves(b) {
                leaves.push(at);
            } else {
                branches.push(at);
            }
        }
        if t.creative {
            return;
        }
        let sapling = sapling_of(log_base(t.stub.1)) as ItemId;
        let r = self.random();
        let saplings = if leaves.is_empty() { 0 } else if r < 0.4 { 0 } else if r < 0.85 { 1 } else { 2 };
        let r = self.random();
        let sticks = if branches.is_empty() { (r < 0.5) as usize } else { 1 + (r * 3.0) as usize };
        let drop = |g: &mut Self, from: &[Vec3], what: ItemId, n: usize| {
            for _ in 0..n {
                if from.is_empty() {
                    return;
                }
                let at = from[(g.random() * from.len() as f32) as usize % from.len()];
                g.spawn_drop(at, crate::item::Stack::one(what));
            }
        };
        drop(self, &leaves, sapling, saplings);
        let from = if branches.is_empty() { &leaves } else { &branches };
        drop(self, from, crate::item::STICK, sticks);
    }

    /// The falling trees' blocks where they are now: the logs round, the leaves.
    /// Those within `sight` of `eye`.
    pub(in crate::game) fn build_falling_trees(&self, out: &mut Vec<Vertex>, eye: Vec3, sight: f32) {
        use crate::model::{emit_box, emit_item};
        let fl = crate::world::mesh::flags::ENTITY;
        for t in &self.level.falling_trees {
            let turn = t.turned(t.prev_angle + (t.angle - t.prev_angle) * self.between);
            let mid = t.at(&turn, Vec3::new(0.0, t.height * 0.5, 0.0));
            if mid.distance(eye) > sight + t.height {
                continue;
            }
            let (sky, blk) = self.terrain.world.light_estimate(mid);
            let light = crate::util::vertex_light(sky, blk);
            // The top of the cut block, from the cut up.
            let (o, b, h) = t.stub;
            let m = turn
                * Mat4::from_translation(o + Vec3::new(0.5, (1.0 + h) * 0.5, 0.5))
                * Mat4::from_scale(Vec3::new(1.0, 1.0 - h, 1.0));
            emit_item(out, m, b, light, fl);
            for &(o, b) in &t.blocks {
                let m = turn * Mat4::from_translation(o + Vec3::splat(0.5));
                if is_leaves(b) {
                    let layers = std::array::from_fn(|f| face_texture(b, f));
                    emit_box(out, m, Vec3::splat(-0.5), Vec3::splat(0.5), layers, [t.leaf_tint; 6], light, fl);
                } else {
                    emit_item(out, m, b, light, fl);
                }
            }
        }
    }
}
