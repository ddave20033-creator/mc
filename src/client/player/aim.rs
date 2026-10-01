//! What the player aims at (a block, a mob, another player, a lying trunk, a furnace's part)
//! and what their hands are doing with it: mining, chopping, blocking with a sword, eating or
//! drinking, and the wait before the next use.

use crate::client::*;
use crate::entity::player::raycast;
use crate::item::break_time;

/// What the crosshair is on and what the hands do.
#[derive(Default)]
pub(in crate::client) struct Aim {
    /// The block under the crosshair and the face it is looked at on (its normal).
    pub(in crate::client) target: Option<(IVec3, IVec3)>,
    /// Where the look ray meets the targeted block.
    pub(in crate::client) target_point: Vec3,
    /// The furnace part under the crosshair: a corner of the top (0..4) or the front's
    /// upper or lower half (`block_entity::part`).
    pub(in crate::client) furnace_part: Option<(IVec3, u8)>,
    /// Something was just taken out of a furnace with the left button, still held: it does
    /// not start mining the furnace.
    pub(in crate::client) furnace_hold: bool,
    /// The mob the crosshair is on (its id; `target_mob` finds it), when it is closer than any
    /// block. An id, not an index: a mob removed earlier in the frame must not shift it.
    pub(in crate::client) mob_target: Option<u32>,
    /// The other player the crosshair is on.
    pub(in crate::client) player_target: Option<u8>,
    /// The lying trunk aimed at with an axe (`shown::logs`), and the cut in it going on.
    pub(in crate::client) log_aim: Option<crate::client::shown::logs::LogAim>,
    pub(in crate::client) log_cut: Option<(u32, bool)>,
    /// The block being mined and how far (0..1).
    pub(in crate::client) mining: Option<(IVec3, f32)>,
    /// Till the next mining particles.
    pub(in crate::client) dig_timer: f32,
    /// Till the hands may use something again.
    pub(in crate::client) action_cooldown: f32,
    /// A chop with an axe going on (`shown::felling`).
    pub(in crate::client) chop: Option<crate::model::chop_rig::Swing>,
    /// What the axe is stuck in, to come apart when it is pulled out.
    pub(in crate::client) struck: Option<crate::client::shown::felling::Struck>,
    /// Blocking with a sword (right mouse button held).
    pub(in crate::client) blocking: bool,
    /// Eating or drinking: the item and seconds spent so far.
    pub(in crate::client) using: Option<(ItemId, f32)>,
}

impl Aim {
    /// Nothing aimed at, and the hands let go of what they did (in bed, as a spectator).
    pub(in crate::client) fn let_go(&mut self) {
        self.target = None;
        self.furnace_part = None;
        self.mob_target = None;
        self.player_target = None;
        self.mining = None;
        self.blocking = false;
        self.using = None;
    }
}

impl Game {
    /// What the crosshair is on (`control`: nothing while a screen is open), looking from the
    /// eye where it is drawn (what is aimed at is what is seen): a block, and a mob or another
    /// player in front of it (entity reach: 3 blocks), a furnace's part, a lying trunk.
    pub(in crate::client) fn update_aim(&mut self, control: bool) {
        let eye = self.eye();
        self.me.aim.target = if control {
            let dir = self.me.look.dir();
            match self.me.look.camera.shoulder_camera(&self.terrain.world, eye, dir) {
                Some(cam) => {
                    super::camera::shoulder_target(&self.terrain.world, eye, dir, cam, 5.0)
                }
                None => raycast(&self.terrain.world, eye, dir, 5.0),
            }
        } else {
            None
        };
        if let Some((hit, _)) = self.me.aim.target {
            let dir = self.me.look.dir();
            self.me.aim.target_point = crate::entity::player::ray_boxes(&self.terrain.world, eye, dir, hit, 6.0)
                .map(|(t, _)| eye + dir * t)
                .unwrap_or(hit.as_vec3() + Vec3::splat(0.5));
        }
        self.aim_furnace();
        // A mob in front of the targeted block takes the crosshair (entity reach: 3 blocks).
        self.me.aim.mob_target = None;
        self.me.aim.player_target = None;
        if control {
            let dir = self.me.look.dir();
            let block_dist = self
                .me.aim.target
                .and_then(|(hit, _)| {
                    let min = hit.as_vec3();
                    crate::util::ray_box(eye, dir, min, min + Vec3::ONE, 5.0)
                })
                .unwrap_or(f32::INFINITY);
            let reach = if self.creative() { 5.0 } else { 3.0 };
            let mob = self
                .level.mobs
                .iter()
                .enumerate()
                .filter_map(|(i, m)| m.ray_hit(eye, dir, reach).map(|d| (i, d)))
                .filter(|&(_, d)| d < block_dist)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            // Another LAN player in front of the mob and the block takes it instead.
            let player = self
                .pick_player(eye, dir, reach)
                .filter(|&(_, d)| d < block_dist && mob.is_none_or(|(_, md)| d < md));
            self.me.aim.player_target = player.map(|(id, _)| id);
            self.me.aim.mob_target = mob.filter(|_| player.is_none()).map(|(i, _)| self.level.mobs[i].id);
            if self.me.aim.mob_target.is_some() || self.me.aim.player_target.is_some() {
                self.me.aim.target = None;
            }
        }
        // A felled trunk lying there, aimed at with an axe.
        self.aim_lying_logs(control && self.me.aim.mob_target.is_none() && self.me.aim.player_target.is_none());
    }

    /// Mining the block aimed at while the left button is held (`can`: nothing else is done
    /// with it): its cracks grow, crumbs fly; returns the block once it breaks.
    pub(in crate::client) fn update_mining(&mut self, dt: f32, can: bool) -> Option<IVec3> {
        let mut breaking = None;
        if can && self.input.left_down && self.me.aim.action_cooldown <= 0.0 {
            if let Some((hit, _)) = self.me.aim.target {
                let b = self.terrain.world.geti(hit);
                let time =
                    break_time(b, self.held()).map(|t| if self.creative() { 0.0 } else { t });
                if let Some(time) = time {
                    let progress = match self.me.aim.mining {
                        Some((p, prog)) if p == hit => prog,
                        _ => {
                            self.me.hand.keep_swinging();
                            0.0
                        }
                    } + dt / time.max(1e-4);
                    self.me.aim.mining = Some((hit, progress));
                    self.me.aim.dig_timer -= dt;
                    if self.me.aim.dig_timer <= 0.0 && time > 0.0 {
                        self.me.aim.dig_timer = 0.24;
                        let tint = self.block_tint(hit, b);
                        self.level.particles.burst(&self.terrain.world, hit, b, 2, tint);
                    }
                    if progress >= 1.0 || time == 0.0 {
                        breaking = Some(hit);
                    }
                } else {
                    self.me.aim.mining = None;
                }
            } else {
                self.me.aim.mining = None;
            }
        } else if !self.input.left_down {
            self.me.aim.mining = None;
            self.me.aim.dig_timer = 0.0;
        }
        breaking
    }
}
