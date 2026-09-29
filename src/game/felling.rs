//! Felling trees with an axe. Each chop is a swing of its own, as made in Blockbench
//! (`chop_rig`): the axe drawn back and swung round level. The axe's edge is followed along
//! its real path through the world as it swings, and only where it meets a trunk does it
//! bite in: it stops there, chips fly, and the cut (`mesh::Notch`) is made on that side at
//! that height, a little deeper with every chop (how much deeper, the axe decides). Deep enough, the trunk breaks there and the
//! tree above it (its trunk, branches and leaves) falls over as one, away from the player,
//! slowly at first and faster as it tips, until it hits the ground and breaks up into what
//! it drops.

use super::*;
use crate::item::{inventory, tool_of, Tier, ToolKind};
use crate::model::chop_rig::{self, ChopPose, Swing, EDGE, HIT, STROKE};
use crate::world::mesh::{notch_at, set_notch, Notch};

/// How deep (of the trunk's width) the cut goes before the trunk breaks.
const FELL_DEPTH: f32 = 0.75;
/// The most blocks a falling tree takes with it (a trunk in a wall of logs stays).
const MAX_BLOCKS: usize = 900;

/// A tree falling over: its blocks turning about the hinge left by the cut.
pub(super) struct FallingTree {
    /// Where it turns about (on the trunk's far side from the cut, at the cut's height), and
    /// the axis it turns about (level, across the way it falls).
    pivot: Vec3,
    axis: Vec3,
    /// How far over it is (radians from upright) and how fast it is going over.
    angle: f32,
    speed: f32,
    /// How high it reaches over the hinge (its fall is slower, the taller it is).
    height: f32,
    /// Its blocks: their lower corner from the pivot as it stood, and the block.
    blocks: Vec<(Vec3, u8)>,
    /// The part of the cut block above the cut (its lower corner from the pivot, the block
    /// and the cut's height in it), which goes with the tree.
    stub: (Vec3, u8, f32),
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
fn is_trunk(b: u8) -> bool {
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
    if let Some(n) = notch_at(p) {
        let y = q.y - p.y as f32;
        let h = n.height.clamp(0.12, 0.88);
        if n.felled && y > h {
            return None;
        }
        let deep = n.depth.clamp(0.0, 1.0) * 2.0 * r;
        let half = (deep * 0.8).max(0.08);
        let cut = r - deep * (1.0 - (y - h).abs() / half).max(0.0);
        if rel.dot(Vec2::new(n.angle.cos(), n.angle.sin())) > cut {
            return None;
        }
    }
    Some(p)
}

impl Game {
    /// Where the chop's rig is in the world (as the player model draws it).
    pub(super) fn chop_world(&self) -> Mat4 {
        chop_rig::to_world(self.player.pos, self.visual_head_yaw(), self.pitch)
    }

    /// The trunk the player is aiming an axe at, if any: a swing can start (felling is the
    /// host's; a LAN client mines trunks like any block).
    fn chop_target(&self) -> Option<IVec3> {
        chops_needed(self.held())?;
        if self.is_client() {
            return None;
        }
        let (hit, _) = self.target?;
        // (a stump left is mined like a log)
        let stump = notch_at(hit).is_some_and(|n| n.felled);
        (is_trunk(self.terrain.world.geti(hit)) && !stump).then_some(hit)
    }

    /// Where the axe's edge first comes into a trunk's wood between the animation's times
    /// `t0` and `t1`: the time, the trunk and the point.
    fn edge_contact(&self, t0: f32, t1: f32) -> Option<(f32, IVec3, Vec3)> {
        let world = self.chop_world();
        let steps = ((t1 - t0) / 0.004).ceil().max(1.0) as usize;
        for i in 1..=steps {
            let t = t0 + (t1 - t0) * i as f32 / steps as f32;
            let axe = world * ChopPose::at(t).axe();
            for e in EDGE {
                let q = axe.transform_point3(e);
                if let Some(p) = in_trunk(&self.terrain.world, q) {
                    return Some((t, p, q));
                }
            }
        }
        None
    }

    /// Chopping with an axe, instead of mining: a swing at a time while the button is held,
    /// the edge followed along its path; where it meets a trunk it bites in. True while it is
    /// going on (the normal mining is left out, and the hand is drawn by the rig).
    pub(super) fn update_chopping(&mut self, active: bool, dt: f32) -> bool {
        if let Some(mut sw) = self.chop {
            let before = sw.anim_time();
            sw.clock += dt;
            let after = sw.anim_time();
            if sw.hit.is_none() && after > STROKE && before < HIT {
                if let Some((t, p, point)) = self.edge_contact(before.max(STROKE), after.min(HIT)) {
                    // Stuck where it bit in.
                    sw.hit = Some(t);
                    sw.clock = t;
                    self.chop_hit(p, point);
                }
            }
            self.chop = (!sw.done()).then_some(sw);
        }
        let target = if active { self.chop_target() } else { None };
        if self.chop.is_none() && (target.is_none() || !self.left_down) {
            self.hand.hidden = false;
            return target.is_some();
        }
        self.mining = None;
        if self.chop.is_none() {
            self.chop = Some(Swing::default());
        }
        self.hand.hidden = true;
        true
    }

    /// The axe's edge has bitten into the trunk at `p` at `point`: the cut is made there, on
    /// the side of the trunk it came in from, at the height it hit; each chop takes as much
    /// more wood out as the axe cuts (a chop a little off the cut moves it that way, weighed
    /// by how much is cut already). Chips fly, and cut through far enough the tree falls.
    fn chop_hit(&mut self, p: IVec3, point: Vec3) {
        let b = self.terrain.world.geti(p);
        let creative = self.creative();
        let chops = if creative { 4.0 } else { chops_needed(self.held()).unwrap_or(10.0) };
        let middle = p.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
        // Where it bit in: round the trunk from its middle toward the point hit (or the
        // player, hit straight on), and how high.
        let side = Vec2::new(point.x - middle.x, point.z - middle.z);
        let side = if side.length() > 0.05 { side } else { Vec2::new(self.player.pos.x - middle.x, self.player.pos.z - middle.z) };
        let (angle, height) = (side.y.atan2(side.x), (point.y - p.y as f32).clamp(0.25, 0.75));
        let step = FELL_DEPTH / chops;
        let notch = match notch_at(p) {
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
        let tint = self.block_tint(p, b);
        for _ in 0..3 {
            self.particles.impact(&self.terrain.world, at, out, b, tint);
        }
        let layer = face_texture(b, 2);
        let (sky, blk) = self.terrain.world.light_estimate(at);
        self.particles.crumbs(at, layer, 6, sky, blk);
        if !creative {
            self.needs.exhaust(crate::entity::survival::cost::MINE * 0.5);
            let slot = self.hotbar_slot;
            if inventory::damage(&mut self.inventory.slots[slot], 1) {
                self.particles.burst(&self.terrain.world, p, STONE, 12, [255; 3]);
            }
        }
        let notch = Notch { depth, ..notch };
        if depth >= FELL_DEPTH - 1e-3 {
            self.fell_tree(p, notch);
        } else {
            set_notch(p, Some(notch));
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
        let leaves = match kind {
            BIRCH_LOG => BIRCH_LEAVES,
            SPRUCE_LOG => SPRUCE_LEAVES,
            _ => OAK_LEAVES,
        };
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
                let b = w.geti(r);
                if is_log(b) && log_base(b) == kind {
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
        // The stump stays; the rest of its block goes with the tree.
        wood.retain(|&q| q != p);
        let mut blocks: Vec<(Vec3, u8)> = Vec::with_capacity(wood.len() + leaf.len());
        for &q in wood.iter().chain(&leaf) {
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
        set_notch(p, Some(Notch { felled: true, ..notch }));
        self.falling_trees.push(FallingTree {
            pivot,
            axis: Vec3::Y.cross(toward).normalize(),
            angle: 0.02,
            speed: 0.15,
            height,
            blocks,
            stub: (p.as_vec3() - pivot, b0, notch.height),
            leaf_tint,
            tool,
            creative,
        });
    }

    /// The falling trees go over (like a pole tipping about its foot) until one of them
    /// hits something solid (leaves do not stop it) or lies flat; then it breaks up into
    /// its drops with a crash.
    pub(super) fn update_falling_trees(&mut self, dt: f32) {
        const GRAVITY: f32 = 28.0;
        let mut landed = Vec::new();
        for (i, t) in self.falling_trees.iter_mut().enumerate() {
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
            let t = self.falling_trees.swap_remove(i);
            self.tree_lands(t);
        }
    }

    /// A fallen tree breaks up where it lies: its wood and leaves drop what they would when
    /// mined, in a burst of bark and leaves.
    fn tree_lands(&mut self, t: FallingTree) {
        let turn = t.turn();
        for (n, &(o, b)) in t.blocks.iter().enumerate() {
            let at = t.at(&turn, o);
            let q = at.floor().as_ivec3();
            let tint = if is_leaves(b) { t.leaf_tint } else { [255; 3] };
            if !is_leaves(b) || n % 3 == 0 {
                self.particles.burst(&self.terrain.world, q, b, if is_leaves(b) { 3 } else { 6 }, tint);
            }
            if t.creative {
                continue;
            }
            let r = self.random();
            let held = if is_leaves(b) { NONE } else { t.tool };
            for s in crate::item::drops(b, held, r) {
                // Up out of whatever it came down into.
                let mut at = at;
                while is_solid(self.terrain.world.geti(at.floor().as_ivec3())) && at.y < t.pivot.y + 30.0 {
                    at.y += 1.0;
                }
                self.spawn_drop(at, s);
            }
        }
    }

    /// The falling trees' blocks where they are now: the logs round, the leaves.
    pub(super) fn build_falling_trees(&self, out: &mut Vec<Vertex>) {
        use crate::model::{emit_box, emit_item};
        let fl = crate::world::mesh::flags::ENTITY;
        for t in &self.falling_trees {
            let turn = t.turn();
            let mid = t.at(&turn, Vec3::new(0.0, t.height * 0.5, 0.0));
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
