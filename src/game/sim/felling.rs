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
use crate::item::{inventory, tool_of, Tier, ToolKind};
use crate::model::chop_rig::{self, ChopPose, Kind, Swing, EDGE};
use crate::world::mesh::{stump_heights, Notch};

/// How deep (of the trunk's width) the cut goes before the trunk breaks.
const FELL_DEPTH: f32 = 0.75;
/// The most blocks a falling tree takes with it.
const MAX_BLOCKS: usize = 900;

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

/// A tree falling over: its blocks turning about the hinge left by the cut.
pub(in crate::game) struct FallingTree {
    /// Where it turns about (on the trunk's far side from the cut, at the cut's height), and
    /// the axis it turns about (level, across the way it falls).
    pivot: Vec3,
    axis: Vec3,
    /// How far over it is (radians from upright) and how fast it is going over.
    angle: f32,
    speed: f32,
    /// How high it reaches over the hinge (its fall is slower, the taller it is).
    height: f32,
    /// Its blocks: their lower corner from the pivot as it stood, and the block; the first
    /// `trunk` of them its trunk from the cut up (which stays lying where it falls).
    blocks: Vec<(Vec3, Block)>,
    trunk: usize,
    /// The middle of the stump's top (where the trunk lies from).
    stump: Vec3,
    /// The part of the cut block above the cut (its lower corner from the pivot, the block
    /// and the cut's height in it), which goes with the tree.
    stub: (Vec3, Block, f32),
    leaf_tint: [u8; 3],
    /// What it was felled with (for the drops), and whether in creative (none).
    tool: ItemId,
    creative: bool,
}

impl FallingTree {
    fn turn(&self) -> Mat4 {
        Mat4::from_translation(self.pivot) * Mat4::from_axis_angle(self.axis, self.angle)
    }

    /// Where a block's middle is now.
    fn at(&self, turn: &Mat4, o: Vec3) -> Vec3 {
        turn.transform_point3(o + Vec3::splat(0.5))
    }
}

/// The trunk of a standing tree (an upright log, not a branch), which an axe fells.
fn is_trunk(b: Block) -> bool {
    is_log(b) && !is_branch(b) && log_axis(b) == 1
}

/// Chops it takes an axe to fell a tree (None: not an axe): each chop cuts this much of the
/// way through, a weak axe little, a strong one more.
fn chops_needed(held: ItemId) -> Option<f32> {
    match tool_of(held)? {
        (ToolKind::Axe, tier) => Some(match tier {
            Tier::Wood => 10.0,
            Tier::Stone => 8.0,
            Tier::Copper => 7.0,
            Tier::Iron => 6.0,
            Tier::Diamond => 5.0,
            Tier::Gold => 4.0,
        }),
        _ => None,
    }
}

/// Whether the point `q` is in the wood of an upright trunk (not in the air round it, nor in
/// a cut already taken out of it), and which trunk.
fn in_trunk(w: &World, q: Vec3) -> Option<IVec3> {
    let p = q.floor().as_ivec3();
    let b = w.geti(p);
    if !is_trunk(b) {
        return None;
    }
    let rel = Vec2::new(q.x - (p.x as f32 + 0.5), q.z - (p.z as f32 + 0.5));
    let r = log_radius(b);
    if rel.length() > r {
        return None;
    }
    if let Some(n) = w.notch(p) {
        let y = q.y - p.y as f32;
        let h = n.height.clamp(0.12, 0.88);
        let deep = n.depth.clamp(0.0, 1.0) * 2.0 * r;
        if n.felled {
            // A stump: flat, the hinge standing on its far side.
            let (flat, hinge) = stump_heights(n, r);
            let far = rel.dot(Vec2::new(n.angle.cos(), n.angle.sin())) <= r - deep;
            return (y <= flat || (y <= hinge && far)).then_some(p);
        }
        let half = (deep * 0.8).max(0.08);
        let cut = r - deep * (1.0 - (y - h).abs() / half).max(0.0);
        if rel.dot(Vec2::new(n.angle.cos(), n.angle.sin())) > cut {
            return None;
        }
    }
    Some(p)
}

/// The stump that `p` is part of, if it is one: its cut block (left by a felled tree) on top
/// and the trunk under it down to what it stands on, top first.
fn stump_of(w: &World, p: IVec3) -> Option<Vec<IVec3>> {
    let b = w.geti(p);
    if !is_trunk(b) {
        return None;
    }
    let same = |q: IVec3| is_trunk(w.geti(q)) && log_base(w.geti(q)) == log_base(b);
    // Up to the cut (a trunk still standing above is no stump).
    let mut top = p;
    while !w.notch(top).is_some_and(|n| n.felled) {
        top += IVec3::Y;
        if !same(top) || top.y - p.y > 64 {
            return None;
        }
    }
    let mut column = vec![top];
    let mut q = top - IVec3::Y;
    while same(q) && w.notch(q).is_none() && top.y - q.y < 64 {
        column.push(q);
        q -= IVec3::Y;
    }
    Some(column)
}

/// All the cuts in trunks, for saving (`x,y,z,angle,height,depth,felled` a line).
pub fn notches_text(w: &World) -> String {
    w.notches
        .iter()
        .map(|(p, n)| format!("{},{},{},{},{},{},{}\n", p.x, p.y, p.z, n.angle, n.height, n.depth, n.felled as u8))
        .collect()
}

/// The cuts saved with a world (any there were before are gone).
pub fn load_notches(w: &mut World, text: &str) {
    w.notches.clear();
    for line in text.lines() {
        let v: Vec<&str> = line.trim().split(',').collect();
        if v.len() != 7 {
            continue;
        }
        let i = |k: usize| v[k].parse::<i32>().ok();
        let f = |k: usize| v[k].parse::<f32>().ok();
        if let (Some(x), Some(y), Some(z), Some(angle), Some(height), Some(depth)) = (i(0), i(1), i(2), f(3), f(4), f(5)) {
            w.set_notch(IVec3::new(x, y, z), Some(Notch { angle, height, depth, felled: v[6] == "1" }));
        }
    }
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
        if self.is_client() {
            return None;
        }
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
        if depth >= FELL_DEPTH - 1e-3 {
            self.fell_tree(p, notch);
        } else {
            self.terrain.world.set_notch(p, Some(notch));
        }
        self.terrain.block_changed(p, false);
    }

    /// The trunk at `p` breaks at the cut: everything of its tree above it (the trunk, the
    /// branches, and the leaves round them that are no other tree's) comes away and falls
    /// over, away from the cut's side. Below the cut its stump is left.
    fn fell_tree(&mut self, p: IVec3, notch: Notch) {
        let w = &self.terrain.world;
        let b0 = w.geti(p);
        let kind = log_base(b0);
        let leaves = leaves_of(kind);
        const SIDES: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z];
        let mut wood = vec![p];
        let mut seen: HashSet<IVec3> = HashSet::from([p]);
        let mut i = 0;
        while i < wood.len() && wood.len() < MAX_BLOCKS {
            let q = wood[i];
            i += 1;
            for d in SIDES {
                let r = q + d;
                if r.y < p.y || seen.contains(&r) {
                    continue;
                }
                // Only what a tree is made of: its trunk straight up from the cut, and its
                // branches within a tree's reach. Logs laid by a player (a wall, a house) or
                // the trunk of another tree the branches touch stay.
                let b = w.geti(r);
                let own = if is_branch(b) {
                    (r.x - p.x).abs().max((r.z - p.z).abs()) <= crate::world::trees::REACH
                } else {
                    is_trunk(b) && r.x == p.x && r.z == p.z
                };
                if own && log_base(b) == kind {
                    seen.insert(r);
                    wood.push(r);
                }
            }
        }
        // The leaves within a few steps of its wood, but none next to another tree's wood.
        let mut leaf = Vec::new();
        let mut front = wood.clone();
        for _ in 0..5 {
            let mut next = Vec::new();
            for q in front {
                for d in SIDES {
                    let r = q + d;
                    if seen.contains(&r) || w.geti(r) != leaves {
                        continue;
                    }
                    seen.insert(r);
                    let foreign = (-1..=1).any(|dy| {
                        (-1..=1).any(|dz| {
                            (-1..=1).any(|dx| {
                                let s = r + IVec3::new(dx, dy, dz);
                                is_log(w.geti(s)) && !seen.contains(&s)
                            })
                        })
                    });
                    if !foreign {
                        leaf.push(r);
                        next.push(r);
                    }
                }
            }
            front = next;
        }
        // It falls away from the side the cut was made on, about a hinge on the far side.
        let toward = Vec3::new(-notch.angle.cos(), 0.0, -notch.angle.sin());
        let pivot = p.as_vec3() + Vec3::new(0.5, notch.height, 0.5) + toward * log_radius(b0) * 0.7;
        // The stump stays; the rest of its block goes with the tree. Its trunk from the cut up
        // first (it will lie where it falls), then the rest of its wood and its leaves.
        wood.retain(|&q| q != p);
        let trunk: Vec<IVec3> = (1..)
            .map(|dy| p + IVec3::Y * dy)
            .take_while(|&q| seen.contains(&q) && is_trunk(w.geti(q)) && log_base(w.geti(q)) == kind)
            .collect();
        let mut blocks: Vec<(Vec3, Block)> = Vec::with_capacity(wood.len() + leaf.len());
        for &q in trunk.iter().chain(wood.iter().filter(|q| !trunk.contains(q))).chain(&leaf) {
            blocks.push((q.as_vec3() - pivot, w.geti(q)));
        }
        let height = blocks.iter().map(|(o, _)| o.y + 1.0).fold(1.0f32, f32::max);
        let leaf_tint = match leaf.first() {
            Some(&q) => self.block_tint(q, leaves),
            None => [255; 3],
        };
        let tool = self.held();
        let creative = self.creative();
        for &q in leaf.iter().chain(&wood) {
            self.set_block(q, AIR);
        }
        for &q in &wood {
            self.block_updated(q);
        }
        self.terrain.world.set_notch(p, Some(Notch { felled: true, ..notch }));
        self.level.falling_trees.push(FallingTree {
            pivot,
            axis: Vec3::Y.cross(toward).normalize(),
            angle: 0.02,
            speed: 0.15,
            height,
            blocks,
            trunk: trunk.len(),
            stump: p.as_vec3() + Vec3::new(0.5, notch.height, 0.5),
            stub: (p.as_vec3() - pivot, b0, notch.height),
            leaf_tint,
            tool,
            creative,
        });
    }

    /// The falling trees go over (like a pole tipping about its foot) until one of them
    /// hits something solid (leaves do not stop it) or lies flat; then it breaks up into
    /// its drops with a crash.
    pub(in crate::game) fn update_falling_trees(&mut self, dt: f32) {
        const GRAVITY: f32 = 28.0;
        let mut landed = Vec::new();
        for (i, t) in self.level.falling_trees.iter_mut().enumerate() {
            let pull = 1.5 * GRAVITY / t.height.max(1.5) * t.angle.sin().max(0.04);
            t.speed += pull * dt;
            t.angle += t.speed * dt;
            let turn = t.turn();
            let w = &self.terrain.world;
            // Its wood stops on anything solid but leaves (its own leaves and other trees'
            // crash through).
            let hit = t.angle > 1.65
                || t.blocks.iter().any(|&(o, b)| {
                    if is_leaves(b) || o.y < 0.6 {
                        return false;
                    }
                    let g = w.geti(t.at(&turn, o).floor().as_ivec3());
                    is_solid(g) && !is_leaves(g)
                });
            if hit {
                landed.push(i);
            }
        }
        for i in landed.into_iter().rev() {
            let t = self.level.falling_trees.swap_remove(i);
            self.tree_lands(t);
        }
    }

    /// A fallen tree breaks up where it lies: its leaves and branches shatter in a burst of
    /// bark and leaves, leaving little (a sapling or two from the leaves, a few sticks from
    /// the branches); its trunk stays lying there, to be cut up.
    fn tree_lands(&mut self, t: FallingTree) {
        let turn = t.turn();
        // The trunk from just past the stump, the way it fell (the ground's lie of it).
        let pieces: Vec<Block> = t.blocks[..t.trunk].iter().map(|&(_, b)| b).collect();
        let fell = turn.transform_vector3(Vec3::Y);
        let fell = Vec3::new(fell.x, 0.0, fell.z).normalize_or_zero();
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
            let turn = t.turn();
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
