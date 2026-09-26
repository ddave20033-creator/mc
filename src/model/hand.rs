//! The first-person hand: the arm or the held item, with Minecraft's swing, equip, bob,
//! eating and sword-blocking animations. Built on the CPU each frame in world space.

use super::player::ARM as ARM_LAYERS;
use super::{emit_box, emit_held};
use crate::item::{icon, Icon, ItemId, NONE};
use crate::util::vertex_light;
use crate::world::mesh::{flags, Vertex};
use crate::world::TORCH;
use glam::{Mat4, Vec2, Vec3};
use std::f32::consts::{PI, TAU};
fn t(x: f32, y: f32, z: f32) -> Mat4 {
    Mat4::from_translation(Vec3::new(x, y, z))
}
fn rx(d: f32) -> Mat4 {
    Mat4::from_rotation_x(d.to_radians())
}
fn ry(d: f32) -> Mat4 {
    Mat4::from_rotation_y(d.to_radians())
}
fn rz(d: f32) -> Mat4 {
    Mat4::from_rotation_z(d.to_radians())
}

/// Arm swing duration in seconds (a bit snappier than Minecraft's 6 ticks).
const SWING_TIME: f32 = 0.24;
/// Fastest swing, used when actions follow each other very quickly.
const MIN_SWING_TIME: f32 = 0.09;

pub struct HandAnim {
    swing: f32,
    swinging: bool,
    /// Duration of the current swing; shorter when actions come in quick succession.
    swing_time: f32,
    /// Animation clock and the time of the last swing request (to measure the action rhythm).
    clock: f32,
    last_request: f32,
    equip: f32,
    walk_dist: f32,
    bob: f32,
    sway: Vec2,
    pub held: ItemId,
    /// Hand height, 1 = normal; lowered out of view when the body's arms take over
    /// (First Person Model's dynamic hands).
    pub lower: f32,
    /// Blocking with a sword: held across the view (Minecraft 1.8).
    pub blocking: bool,
    /// Seconds spent eating or drinking the held item (None when not).
    pub eating: Option<f32>,
    /// Hold a lantern by its handle in the fist (with the first-person body option); otherwise
    /// it is held like any other item, as in plain Minecraft.
    pub fancy_lantern: bool,
    lantern_swing: crate::model::lantern::SmoothSwing,
    /// Blend 0..1 from the normal hold to the blocking pose.
    block: f32,
}

impl HandAnim {
    pub fn new() -> Self {
        Self {
            swing: 0.0,
            swinging: false,
            swing_time: SWING_TIME,
            clock: 0.0,
            last_request: -1.0,
            equip: 0.0,
            walk_dist: 0.0,
            bob: 0.0,
            sway: Vec2::ZERO,
            held: NONE,
            lower: 1.0,
            blocking: false,
            block: 0.0,
            eating: None,
            fancy_lantern: false,
            lantern_swing: crate::model::lantern::SmoothSwing::default(),
        }
    }

    /// One swing per action (breaking or placing a block, hitting, throwing). The swing
    /// speed follows the rhythm of the actions, so fast breaking/placing gets fast swings
    /// that finish before the next one instead of one slow swing that ignores them.
    pub fn swing(&mut self) {
        let interval = self.clock - self.last_request;
        self.last_request = self.clock;
        self.swing_time = (interval * 0.9).clamp(MIN_SWING_TIME, SWING_TIME);
        // Restart unless the current swing has barely started (avoids a visible jump back).
        if !self.swinging || self.swing > 0.35 {
            self.swing = 0.0;
            self.swinging = true;
        }
    }

    /// Starts a swing only if the arm is at rest (continuous mining keeps one smooth loop).
    pub fn keep_swinging(&mut self) {
        if !self.swinging {
            self.swing_time = SWING_TIME;
            self.swing = 0.0;
            self.swinging = true;
        }
    }

    /// Current attack swing progress (0 when idle).
    pub fn attack(&self) -> f32 {
        if self.swinging {
            self.swing
        } else {
            0.0
        }
    }

    /// Switch to a new held item with the lower-and-raise animation.
    pub fn equip(&mut self, item: ItemId) {
        if item != self.held {
            self.held = item;
            self.equip = 0.0;
            self.block = 0.0;
            self.lantern_swing = crate::model::lantern::SmoothSwing::default();
        }
    }

    pub fn update(
        &mut self,
        dt: f32,
        mining: bool,
        walk_speed: f32,
        on_ground: bool,
        look_delta: Vec2,
    ) {
        self.clock += dt;
        if self.swinging {
            self.swing += dt / self.swing_time;
            if self.swing >= 1.0 {
                if mining {
                    // Holding the button on a slow block: loop at the normal pace again once
                    // the quick actions have stopped.
                    if self.clock - self.last_request > SWING_TIME {
                        self.swing_time = SWING_TIME;
                    }
                    self.swing = (self.swing - 1.0).min(0.5);
                } else {
                    self.swing = 0.0;
                    self.swinging = false;
                }
            }
        } else if mining {
            self.keep_swinging();
        }
        self.equip = (self.equip + dt * 4.0).min(1.0);
        // The sword swings into the blocking pose and back in about 0.15 s.
        let target = if self.blocking { 1.0 } else { 0.0 };
        let step = dt / 0.15;
        self.block += (target - self.block).clamp(-step, step);
        // Minecraft: bob approaches min(0.1, speed per tick); walk distance grows at 0.6x speed.
        let target = if on_ground {
            (walk_speed / 20.0).min(0.1)
        } else {
            0.0
        };
        self.bob += (target - self.bob) * (1.0 - (-8.0 * dt).exp());
        self.walk_dist += walk_speed * dt * 0.6;
        let sway_target = (look_delta * 0.03).clamp(Vec2::splat(-2.5), Vec2::splat(2.5));
        self.sway += (sway_target - self.sway) * (1.0 - (-12.0 * dt).exp());
    }

    /// View bobbing, applied in camera space to both the world and the hand (like Minecraft).
    pub fn bob_matrix(&self) -> Mat4 {
        let f = -self.walk_dist * PI;
        let b = self.bob;
        t(f.sin() * b * 0.5, -(f.cos() * b).abs(), 0.0)
            * rz(f.sin() * b * 3.0)
            * rx((((-self.walk_dist) * PI - 0.2).cos() * b).abs() * 5.0)
    }

    /// Walking phase used by the hand and camera, in radians.
    pub fn walk_phase(&self) -> f32 {
        self.walk_dist * PI
    }

    /// Minecraft's `ItemInHandRenderer.renderPlayerArm` pose (camera space, blocks) for the
    /// right arm. `s`/`sq`: attack progress and its square root, `eq`: equip progress.
    fn arm_pose(base: Mat4, s: f32, sq: f32, eq: f32) -> Mat4 {
        base * t(
            -0.3 * (sq * PI).sin() + 0.64,
            0.4 * (sq * TAU).sin() - 0.6 - (1.0 - eq) * 0.6,
            -0.4 * (s * PI).sin() - 0.72,
        ) * ry(45.0)
            * ry((sq * PI).sin() * 70.0)
            * rz((s * s * PI).sin() * -20.0)
            * t(-1.0, 3.6, 3.5)
            * rz(120.0)
            * rx(200.0)
            * ry(-135.0)
            * t(5.6, 0.0, 0.0)
    }

    /// The arm model part within that pose: model pixels to the world.
    fn arm_part(pose: Mat4) -> Mat4 {
        pose * t(-5.0 / 16.0, 2.0 / 16.0, 0.0)
            * Mat4::from_scale(Vec3::splat(1.0 / 16.0))
            * rx(180.0)
    }

    /// Builds the arm (empty hand) or the held item in world space.
    pub fn build(
        &mut self,
        out: &mut Vec<Vertex>,
        cam_to_world: Mat4,
        sky: u8,
        blk: u8,
        dt: f32,
        skin: u8,
    ) {
        let light = vertex_light(sky, blk);
        let fl = flags::VIEWMODEL;
        let s = self.attack();
        let sq = s.sqrt();
        let eq = {
            let e = self.equip;
            e * e * (3.0 - 2.0 * e)
        } * self.lower;
        // Hand lags slightly behind camera rotation.
        let base = cam_to_world * rx(self.sway.y) * ry(self.sway.x);

        if self.held == NONE {
            let m = Self::arm_part(Self::arm_pose(base, s, sq, eq));
            emit_box(
                out,
                m,
                Vec3::new(-3.0, -10.0, -2.0),
                Vec3::new(1.0, 2.0, 2.0),
                ARM_LAYERS.map(|layer| crate::world::textures::skin_layer(layer, skin)),
                [[255; 3]; 6],
                light,
                fl,
            );
            return;
        }

        // Held item: arm transform + attack transform + item display transform.
        let f = (s * s * PI).sin();
        let f1 = (sq * PI).sin();
        // Eating or drinking (Minecraft's applyEatTransform): the item comes up to the mouth
        // and bobs there.
        let eat = match self.eating {
            Some(used) => {
                let total = crate::entity::survival::USE_TIME * 20.0;
                let left = (total - used * 20.0).max(0.0) + 1.0;
                let frac = left / total;
                let bob = if frac < 0.8 {
                    ((left / 4.0 * PI).cos() * 0.1).abs()
                } else {
                    0.0
                };
                let k = 1.0 - frac.min(1.0).powi(27);
                t(k * 0.6, bob - k * 0.5, 0.0) * ry(k * 90.0) * rx(k * 10.0) * rz(k * 30.0)
            }
            None => Mat4::IDENTITY,
        };
        let m = base
            * eat
            * t(-0.4 * f1, 0.2 * (sq * TAU).sin(), -0.2 * (s * PI).sin())
            * t(0.56, -0.52 - (1.0 - eq) * 0.6, -0.72)
            * ry(45.0 + f * -20.0)
            * rz(f1 * -20.0)
            * rx(f1 * -80.0)
            * ry(-45.0);
        let flat = matches!(icon(self.held), Icon::Flat(_));
        if self.held == crate::world::LANTERN as ItemId {
            // Hold the lantern by its chain in both first-person hand styles. Its body
            // follows the hand with the same gravity-driven swing as the player model.
            let grip = if self.fancy_lantern {
                base * t(-0.025, 0.125, 0.0) * rz(10.0)
            } else {
                base
            };
            let pose = Self::arm_pose(grip, s, sq, eq);
            emit_box(
                out,
                Self::arm_part(pose),
                Vec3::new(-3.0, -10.0, -2.0),
                Vec3::new(1.0, 2.0, 2.0),
                ARM_LAYERS.map(|layer| crate::world::textures::skin_layer(layer, skin)),
                [[255; 3]; 6],
                light,
                fl,
            );
            let turn = glam::Quat::from_xyzw(0.2077, -0.6488, 0.4433, 0.5825).normalize();
            let center = Vec3::splat(0.5);
            // Block model space (0..1) to the world, then the lantern's own pixel space.
            let block = pose
                * t(-0.684, 0.117, -0.439)
                * Mat4::from_translation(center)
                * Mat4::from_quat(turn)
                * Mat4::from_translation(-center);
            let pivot = block.transform_point3(Vec3::new(0.5, 11.0 / 16.0, 0.5));
            let style = crate::model::lantern::FIRST_PERSON;
            let dir = self.lantern_swing.update(style, pivot, dt);
            let forward = -cam_to_world.z_axis.truncate();
            let yaw = forward.z.atan2(forward.x);
            crate::model::lantern::emit_held_lantern(out, style, pivot, dir, yaw, light, fl);
            return;
        }
        let item = if self.held == TORCH as ItemId {
            m * t(0.08, 0.2, 0.06) * rz(-12.0) * Mat4::from_scale(Vec3::splat(0.82))
        } else if flat {
            m * t(1.13 / 16.0, 3.2 / 16.0, 1.13 / 16.0)
                * ry(-90.0)
                * rz(25.0)
                * Mat4::from_scale(Vec3::splat(0.68))
        } else {
            m * ry(45.0) * Mat4::from_scale(Vec3::splat(0.4))
        };
        let item = if self.block > 0.0 {
            // Minecraft 1.8's ItemRenderer: transformFirstPersonItem (no swing), then
            // doBlockTransformations, then the 1.8 handheld first-person display transform.
            let guard = base
                * t(0.56, -0.52 - (1.0 - eq) * 0.6, -0.72)
                * ry(45.0)
                * Mat4::from_scale(Vec3::splat(0.4))
                * t(-0.5, 0.2, 0.0)
                * ry(30.0)
                * rx(-80.0)
                * ry(60.0)
                * t(0.0, 4.0 / 16.0, 2.0 / 16.0)
                * ry(-135.0)
                * rz(25.0)
                * Mat4::from_scale(Vec3::splat(1.7));
            let k = self.block * self.block * (3.0 - 2.0 * self.block);
            blend(item, guard, k)
        } else {
            item
        };
        emit_held(out, item, self.held, light, fl);
    }
}

/// Between two rigid (uniformly scaled) transforms: position and size linearly, rotation along
/// the shortest arc, so the item turns smoothly instead of being squashed.
fn blend(a: Mat4, b: Mat4, k: f32) -> Mat4 {
    let (sa, ra, ta) = a.to_scale_rotation_translation();
    let (sb, rb, tb) = b.to_scale_rotation_translation();
    Mat4::from_scale_rotation_translation(sa.lerp(sb, k), ra.slerp(rb, k), ta.lerp(tb, k))
}

#[cfg(test)]
mod lantern_view_tests {
    use super::*;

    #[test]
    fn first_person_lantern_trails_a_moving_hand() {
        for fancy in [false, true] {
            let mut hand = HandAnim::new();
            hand.equip(crate::world::LANTERN as ItemId);
            hand.fancy_lantern = fancy;
            let dt = 1.0 / 60.0;
            let mut verts = Vec::new();
            for _ in 0..60 {
                hand.update(dt, false, 0.0, true, Vec2::ZERO);
                verts.clear();
                hand.build(&mut verts, Mat4::IDENTITY, 15, 15, dt, 0);
            }
            let center_x = |v: &[Vertex]| {
                v.iter().skip(36).map(|p| p.pos[0]).sum::<f32>() / (v.len() - 36) as f32
            };
            let rest_x = center_x(&verts);
            for step in 1..=10 {
                verts.clear();
                hand.build(
                    &mut verts,
                    Mat4::from_translation(Vec3::X * (step as f32 * 0.08)),
                    15,
                    15,
                    dt,
                    0,
                );
            }
            assert!(
                center_x(&verts) - 0.8 < rest_x - 0.008,
                "lantern did not trail the hand (fancy={fancy}, rest={}, moving={})",
                rest_x,
                center_x(&verts) - 0.8
            );
        }
    }
}
