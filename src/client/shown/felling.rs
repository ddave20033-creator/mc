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

use crate::client::*;
use crate::item::{inventory, tool_of};
use crate::model::chop_rig::{self, Aim, ChopPose, Kind, Swing, EDGE};
use crate::sim::felling::*;

/// What the axe is stuck in, to come apart when it is pulled out: a stump, or a lying trunk
/// (which, and where it is cut).
#[derive(Clone, Copy, Debug)]
pub(in crate::client) enum Struck {
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
    pub(in crate::client) fn wear_axe(&mut self, exhaust: f32, at: IVec3) {
        self.me.vitals.needs.exhaust(exhaust);
        let slot = self.me.items.hotbar_slot;
        if tool_of(self.held()).is_some() && inventory::damage(&mut self.me.items.inventory.slots[slot], 1) {
            self.level.particles.burst(&self.terrain.world, at, STONE, 12, [255; 3]);
        }
    }

    /// Where the chop's rig is in the world (as the player model draws it, where the camera
    /// is).
    pub(in crate::client) fn chop_world(&self) -> Mat4 {
        chop_rig::to_world(self.me.body.drawn_pos(self.clock.between), self.me.look.visual_head_yaw())
    }

    /// How the swing is aimed (as the player model draws it).
    pub(in crate::client) fn chop_aim(&self) -> Aim {
        Aim::new(self.me.look.pitch, self.me.look.visual_head_yaw(), self.me.look.body_yaw)
    }

    /// The trunk the player is aiming an axe at, if any, and the swing it takes: a standing
    /// trunk is chopped level, a stump struck from above.
    fn chop_target(&self) -> Option<Kind> {
        chops_needed(self.held())?;
        // (a lying trunk is cut up struck from above too)
        if self.me.aim.log_aim.is_some() {
            return Some(Kind::Stump);
        }
        let (hit, _) = self.me.aim.target?;
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
        let (world, aim) = (self.chop_world(), self.chop_aim());
        let w = &self.terrain.world;
        let steps = ((t1 - t0) / 0.004).ceil().max(1.0) as usize;
        for i in 1..=steps {
            let t = t0 + (t1 - t0) * i as f32 / steps as f32;
            let axe = world * ChopPose::at(kind, t).aimed(aim).axe();
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
    /// going on (the normal mining is left out, and the hand is drawn by the rig; after the
    /// last swing, until the axe has settled back in the hands).
    pub(in crate::client) fn update_chopping(&mut self, active: bool, dt: f32) -> bool {
        if let Some(mut sw) = self.me.aim.chop {
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
            self.me.aim.chop = (!sw.done()).then_some(sw);
        }
        // (between swings: a swing over is only settling back)
        let idle = self.me.aim.chop.is_none_or(|s| s.over());
        if idle {
            self.come_apart();
            self.me.aim.log_cut = None;
        }
        let target = if active { self.chop_target() } else { None };
        if idle && (target.is_none() || !self.input.left_down) {
            self.me.hand.hidden = self.me.aim.chop.is_some();
            return target.is_some() || self.me.aim.chop.is_some();
        }
        self.me.aim.mining = None;
        if let (true, Some(kind)) = (idle, target) {
            self.me.aim.chop = Some(Swing { kind, ..Swing::default() });
            self.me.aim.log_cut = self.me.aim.log_aim.map(|a| (a.id, a.from_base));
        }
        self.me.hand.hidden = true;
        true
    }

    /// What the axe was stuck in comes apart as it is pulled out.
    fn come_apart(&mut self) {
        match self.me.aim.struck.take() {
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
            self.me.aim.struck = Some(Struck::Stump(p));
        }
    }

    /// The edge struck into the ground: a puff of it.
    fn ground_hit(&mut self, point: Vec3) {
        let q = point.floor().as_ivec3();
        let b = self.terrain.world.geti(q);
        let tint = self.block_tint(q, b);
        self.level.particles.impact(&self.terrain.world, point, Vec3::Y, b, tint);
    }

    /// The axe bit into the trunk at `p`: its knock, and chips flying out at `at` (the way
    /// `out`).
    pub(in crate::client) fn chips(&mut self, p: IVec3, b: Block, at: Vec3, out: Vec3) {
        self.audio.play(crate::audio::Sound::AxeChop, Some(at), 1.0);
        let tint = self.block_tint(p, b);
        for _ in 0..3 {
            self.level.particles.impact(&self.terrain.world, at, out, b, tint);
        }
        let layer = face_texture(b, 2);
        let (sky, blk) = self.terrain.world.light_estimate(at);
        self.level.particles.crumbs(at, layer, 6, sky, blk);
    }

    /// The stump the axe was struck into comes apart: the cut block and the trunk under it
    /// drop their logs (the ground under it left bare, the grass to grow back), the axe worn
    /// by the one stroke.
    fn break_stump(&mut self, p: IVec3) {
        if stump_of(&self.terrain.world, p).is_none() {
            return;
        }
        let creative = self.creative();
        self.send(crate::net::Msg::Stump { p });
        if !creative {
            self.wear_axe(crate::entity::survival::cost::MINE, p);
        }
    }

    /// The axe's edge has bitten into the trunk at `p` at `point`: the cut is made there, on
    /// the side of the trunk it came in from, at the height it hit; each chop takes as much
    /// more wood out as the axe cuts (a chop a little off the cut moves it that way, weighed
    /// by how much is cut already). Chips fly, and cut through far enough the tree falls.
    fn chop_hit(&mut self, p: IVec3, point: Vec3) {
        let b = self.terrain.world.geti(p);
        if self.terrain.world.notch(p).is_some_and(|n| n.felled) {
            // (a stump is not chopped level: only chips)
            let out = Vec2::new(self.me.body.pos.x - p.x as f32 - 0.5, self.me.body.pos.z - p.z as f32 - 0.5).normalize_or_zero();
            self.chips(p, b, point, Vec3::new(out.x, 0.3, out.y));
            return;
        }
        let creative = self.creative();
        let middle = p.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
        // Where it bit in: round the trunk from its middle toward the point hit (or the
        // player, hit straight on), and how high.
        let side = Vec2::new(point.x - middle.x, point.z - middle.z);
        let side = if side.length() > 0.05 { side } else { Vec2::new(self.me.body.pos.x - middle.x, self.me.body.pos.z - middle.z) };
        let (angle, height) = (side.y.atan2(side.x), point.y - p.y as f32);
        let before = self.terrain.world.notch(p);
        let notch = deepen(before, angle, height, chop_step(self.held(), creative));
        // Chips out of the cut, toward the player.
        let out = Vec3::new(notch.angle.cos(), 0.0, notch.angle.sin());
        let r = log_radius(b) * (1.0 - before.map_or(0.0, |n| n.depth) * 1.6).max(0.2);
        let at = middle + out * r + Vec3::Y * notch.height;
        self.chips(p, b, at, out);
        if !creative {
            self.wear_axe(crate::entity::survival::cost::MINE * 0.5, p);
        }
        // The server cuts it (and fells it, cut through: it answers with the stump and the
        // tree falling); meanwhile the cut shows as it will be.
        self.terrain.world.set_notch(p, Some(notch));
        self.send(crate::net::Msg::Notch { p, notch: Some(notch) });
        self.terrain.block_changed(p, false);
    }

    /// The server says a tree was cut through: it creaks and starts to go over.
    pub(in crate::client) fn tree_falls(&mut self, t: FallingTree) {
        self.audio.play(crate::audio::Sound::TreeCreak, Some(t.stump), 1.0);
        self.level.falling_trees.retain(|f| f.id != t.id);
        self.level.falling_trees.push(t);
    }

    /// The server says a falling tree is down: it crashes and breaks up in a burst of bark
    /// and leaves.
    pub(in crate::client) fn tree_landed(&mut self, id: u32) {
        let Some(i) = self.level.falling_trees.iter().position(|t| t.id == id) else { return };
        let t = self.level.falling_trees.swap_remove(i);
        let middle = t.at(&t.turn(), Vec3::new(0.0, t.height * 0.5, 0.0));
        self.audio.play(crate::audio::Sound::TreeCrash, Some(middle), 1.0);
        self.crash_fx(&t);
    }

    /// The bark and leaves flying where a fallen tree breaks up.
    fn crash_fx(&mut self, t: &FallingTree) {
        let turn = t.turn();
        for (n, &(o, b)) in t.blocks.iter().enumerate().skip(t.trunk) {
            let q = t.at(&turn, o).floor().as_ivec3();
            let tint = if is_leaves(b) { t.leaf_tint } else { [255; 3] };
            if !is_leaves(b) || n % 3 == 0 {
                self.level.particles.burst(&self.terrain.world, q, b, if is_leaves(b) { 3 } else { 6 }, tint);
            }
        }
    }

    /// A tick of this game's copies of the falling trees: they go over as the server's do
    /// (the same steps, through the same world), until it says they are down.
    pub(in crate::client) fn fall_trees_here(&mut self, dt: f32) {
        for t in &mut self.level.falling_trees {
            let angle = t.angle;
            if t.step(dt, &self.terrain.world) {
                // (it waits lying there for the word)
                (t.angle, t.prev_angle, t.speed) = (angle, angle, 0.0);
            }
        }
    }

    /// The falling trees' blocks where they are now: the logs round, the leaves.
    /// Those within `sight` of `eye`.
    pub(in crate::client) fn build_falling_trees(&self, out: &mut Vec<Vertex>, eye: Vec3, sight: f32) {
        use crate::model::{emit_box, emit_item};
        let fl = crate::world::mesh::flags::ENTITY;
        for t in &self.level.falling_trees {
            let turn = t.turned(t.prev_angle + (t.angle - t.prev_angle) * self.clock.between);
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
