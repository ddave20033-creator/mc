//! Where the player looks (`Look`: the view, the field of view, the body and limbs following),
//! and the third-person and shoulder cameras (F5): where the camera goes, keeping it out of
//! walls, and picking blocks under the crosshair when it is not at the eye.

use crate::entity::player::raycast;
use crate::util::{ray_box, wrap_angle};
use crate::world::{is_solid, World};
use crate::model::player::LimbSmoother;
use glam::{IVec3, Mat4, Vec3};
use std::f32::consts::PI;

const RADIUS: f32 = 0.18;

/// Where the player looks and how the view and their model follow: the camera (F5), the
/// field of view, the body turning after the head, the limbs swinging.
pub(in crate::client) struct Look {
    pub(in crate::client) yaw: f32,
    pub(in crate::client) pitch: f32,
    /// F5 view mode and the third-person camera's state.
    pub(in crate::client) camera: Rig,
    /// The field of view easing toward the setting's (zoomed, widened while sprinting).
    pub(in crate::client) fov: f32,
    /// Field of view for simplifying detail too small for the screen (setting and zoom only).
    pub(in crate::client) detail_fov: f32,
    /// This frame's view bobbing (camera space), taken off a held gun again.
    pub(in crate::client) view_bob: Mat4,
    /// Last frame's camera matrix (for name tags).
    pub(in crate::client) view_proj: Mat4,
    /// The body's turn, following the head's (`yaw`).
    pub(in crate::client) body_yaw: f32,
    pub(in crate::client) limb_swing: f32,
    pub(in crate::client) limb_amount: f32,
    /// Smoothed arm and leg rotations of the player model.
    pub(in crate::client) limbs: LimbSmoother,
    /// How far into running the player is (0..1, eased: the gun carried across the chest on
    /// the player model).
    pub(in crate::client) tp_sprint: f32,
    /// Swing of the lantern in this player's hand (third person and body model).
    pub(in crate::client) lantern_swing: crate::model::lantern::SmoothSwing,
    /// Where the torch in this player's hand burns (seen last frame), for its particles.
    pub(in crate::client) held_torch_tip: Option<Vec3>,
}

impl Look {
    /// Looking north, first person; `fov`: the field of view setting.
    pub(in crate::client) fn new(fov: f32) -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            camera: Rig::default(),
            fov,
            detail_fov: fov,
            view_bob: Mat4::IDENTITY,
            view_proj: Mat4::IDENTITY,
            body_yaw: 0.0,
            limb_swing: 0.0,
            limb_amount: 0.0,
            limbs: LimbSmoother::default(),
            tp_sprint: 0.0,
            lantern_swing: Default::default(),
            held_torch_tip: None,
        }
    }

    /// Where the player looks.
    pub(in crate::client) fn dir(&self) -> Vec3 {
        crate::entity::player::look_dir(self.yaw, self.pitch)
    }

    /// The mouse turns the view (`sens`: radians a pixel); up and down only so far.
    pub(in crate::client) fn turn(&mut self, delta: glam::Vec2, sens: f32) {
        self.yaw += delta.x * sens;
        self.pitch = (self.pitch - delta.y * sens).clamp(-1.55, 1.55);
    }

    /// The body follows the head when moving (`speed`: horizontally), otherwise lags within
    /// 50 degrees (like Minecraft); with a gun in hand it turns with the head, all of it at
    /// once.
    pub(in crate::client) fn follow_body(&mut self, dt: f32, speed: f32, gun: bool, chopping: bool) {
        let diff = wrap_angle(self.yaw - self.body_yaw);
        let lag = 50f32.to_radians();
        // (The shoulder views, 3 and 4, orbit the body while stationary.)
        let orbiting = matches!(self.camera.mode, 3 | 4);
        if gun {
            // Turned with the look at once, all of it (the torso never twisted off the legs).
            self.body_yaw = self.yaw;
        } else if chopping {
            // Chopping: the feet step round after the swing (the upper body turns at once).
            self.body_yaw += diff * (crate::util::damp(8.0, dt));
        } else if speed > 0.1 {
            self.body_yaw += diff * (crate::util::damp(10.0, dt));
        } else if !orbiting && diff.abs() > lag {
            // In regular views the torso follows the head so a large turn still looks natural.
            let excess = diff - diff.signum() * lag;
            self.body_yaw += excess * (crate::util::damp(12.0, dt));
        }
    }

    /// Limb swing follows horizontal movement (in the air too, like Minecraft; less flying).
    pub(in crate::client) fn swing_limbs(&mut self, dt: f32, speed: f32, flying: bool) {
        let fly = if flying { 0.3 } else { 1.0 };
        let target = (speed / 4.3).min(1.0) * fly;
        self.limb_amount += (target - self.limb_amount) * (crate::util::damp(10.0, dt));
    }

    /// Keep the model's head within a natural turn while the shoulder camera orbits freely.
    pub(in crate::client) fn visual_head_yaw(&self) -> f32 {
        self.body_yaw + head_turn(self.yaw, self.body_yaw)
    }
}

/// The camera's F5 view mode (0 first person, 1 behind, 2 in front, 3 and 4 over the right
/// and left shoulder) and how it eases between positions and around walls.
#[derive(Default)]
pub(in crate::client) struct Rig {
    pub mode: u8,
    /// Offset from the eye in local coordinates (right, up, forward), easing toward the mode's.
    pub local: Vec3,
    /// How far from the eye the camera has room to be (walls pull it in).
    pub clear_distance: f32,
    /// Too little room for the mode's position: first person until there is.
    fallback: bool,
    transition: bool,
}

impl Rig {
    /// F5: the next view mode.
    pub fn cycle(&mut self) {
        let next = (self.mode + 1) % 5;
        self.transition = animate_switch(self.mode, next);
        self.mode = next;
    }

    /// Offset from the eye to the camera this frame; zero (and reset) outside the world.
    pub fn update(
        &mut self,
        world: &World,
        eye: Vec3,
        aim_dir: Vec3,
        in_world: bool,
        dt: f32,
    ) -> Vec3 {
        if !in_world {
            let mode = self.mode;
            *self = Self {
                mode,
                ..Self::default()
            };
            return Vec3::ZERO;
        }
        let aim_dir = if self.mode == FIXED_FRONT { Vec3::X } else { aim_dir };
        let desired = desired_local(self.mode);
        let room = clearance(world, eye, world_offset(desired, aim_dir));
        let was_fallback = self.fallback;
        if self.mode == 0 {
            self.fallback = false;
        } else if self.fallback {
            self.fallback = room < 1.25;
        } else {
            self.fallback = room < 0.85;
        }
        if self.fallback != was_fallback {
            self.transition = true;
        }
        let goal = if self.fallback { Vec3::ZERO } else { desired };
        if self.transition {
            let blend = crate::util::damp(11.0, dt);
            self.local += (goal - self.local) * blend;
            if self.local.distance(goal) < 0.02 {
                self.local = goal;
                self.transition = false;
            }
        } else {
            self.local = goal;
        }
        let local_world = world_offset(self.local, aim_dir);
        let safe_offset = clamp_offset(world, eye, local_world);
        self.clear_distance = recover_distance(
            self.clear_distance,
            safe_offset.length(),
            self.transition,
            dt,
        );
        local_world.normalize_or_zero() * self.clear_distance
    }

    /// The camera's position for picking blocks from a shoulder view (`dir`: the look).
    pub fn shoulder_camera(&self, world: &World, eye: Vec3, dir: Vec3) -> Option<Vec3> {
        if self.mode < 3 || self.local.length() <= 0.22 {
            return None;
        }
        let offset = clamp_offset(world, eye, world_offset(self.local, dir));
        Some(eye + offset.normalize_or_zero() * offset.length().min(self.clear_distance))
    }
}

/// Only the two moves involving first person and the shoulder-to-shoulder move glide.
pub(in crate::client) fn animate_switch(from: u8, to: u8) -> bool {
    matches!((from, to), (0, 1) | (3, 4) | (4, 0))
}

/// A head cannot follow a camera all the way behind the torso. Ease it back to
/// forward before the signed angle wraps at 180 degrees, avoiding a side-to-side snap.
pub(in crate::client) fn head_turn(camera_yaw: f32, body_yaw: f32) -> f32 {
    let turn = wrap_angle(camera_yaw - body_yaw);
    let magnitude = turn.abs();
    let full = 55f32.to_radians();
    let fade = 115f32.to_radians();
    let amount = if magnitude <= full {
        magnitude
    } else if magnitude <= fade {
        full
    } else {
        full * ((PI - magnitude) / (PI - fade)).clamp(0.0, 1.0)
    };
    turn.signum() * amount
}

/// Local coordinates: right, up, forward. The orbit follows both mouse axes
/// immediately even while its distance or shoulder side eases.
/// Camera modes F5 never reaches, for the test pictures (`--test`): the player seen from the
/// right and a little in front, and from the left.
pub const SIDE_VIEW: u8 = 5;
pub const SIDE_LEFT: u8 = 7;
/// Another (`--test`): in front of the player, facing it, held still where it is (+X of the
/// player, looking back at it) whichever way the player turns.
pub const FIXED_FRONT: u8 = 6;

pub(in crate::client) fn desired_local(mode: u8) -> Vec3 {
    match mode {
        1 => Vec3::new(0.0, 0.0, -4.0),
        2 => Vec3::new(0.0, 0.0, 4.0),
        3 => Vec3::new(0.95, 0.2, -3.2),
        4 => Vec3::new(-0.95, 0.2, -3.2),
        // Only for the test pictures: in front, and from the right or left side (a little in
        // front), looking at the player.
        FIXED_FRONT => Vec3::new(0.0, -0.3, 3.4),
        SIDE_VIEW => Vec3::new(2.8, -0.4, 1.6),
        SIDE_LEFT => Vec3::new(-2.8, -0.4, 1.6),
        _ => Vec3::ZERO,
    }
}

pub(in crate::client) fn world_offset(local: Vec3, forward: Vec3) -> Vec3 {
    let right = forward.cross(Vec3::Y).normalize_or_zero();
    let up = right.cross(forward);
    right * local.x + up * local.y + forward * local.z
}

fn occupied(world: &World, p: Vec3) -> bool {
    let lo = (p - Vec3::splat(RADIUS)).floor().as_ivec3();
    let hi = (p + Vec3::splat(RADIUS)).floor().as_ivec3();
    for z in lo.z..=hi.z {
        for y in lo.y..=hi.y {
            for x in lo.x..=hi.x {
                if !is_solid(world.get(x, y, z)) {
                    continue;
                }
                let min = Vec3::new(x as f32, y as f32, z as f32);
                let nearest = p.clamp(min, min + Vec3::ONE);
                if p.distance_squared(nearest) < RADIUS * RADIUS {
                    return true;
                }
            }
        }
    }
    false
}

/// Available length along the eye-to-camera path, including the camera's radius.
pub(in crate::client) fn clearance(world: &World, eye: Vec3, offset: Vec3) -> f32 {
    let distance = offset.length();
    if distance < 1e-5 {
        return 0.0;
    }
    let direction = offset / distance;
    let mut t = 0.0;
    while t <= distance {
        if occupied(world, eye + direction * t) {
            return (t - 0.08).max(0.0);
        }
        t += 0.06;
    }
    if occupied(world, eye + offset) {
        return (distance - 0.08).max(0.0);
    }
    distance
}

pub(in crate::client) fn clamp_offset(world: &World, eye: Vec3, offset: Vec3) -> Vec3 {
    let distance = offset.length();
    if distance < 1e-5 {
        Vec3::ZERO
    } else {
        offset * (clearance(world, eye, offset) / distance)
    }
}

/// Move inward immediately for safety, but glide back out when an obstacle clears.
pub(in crate::client) fn recover_distance(previous: f32, safe: f32, transitioning: bool, dt: f32) -> f32 {
    if transitioning || safe <= previous {
        safe
    } else {
        (previous + 9.0 * dt).min(safe)
    }
}

/// Pick the block under the centre of an over-the-shoulder view, but only when
/// the player's eye can reach the same block without another block in the way.
pub(in crate::client) fn shoulder_target(
    world: &World,
    eye: Vec3,
    dir: Vec3,
    cam: Vec3,
    reach: f32,
) -> Option<(IVec3, IVec3)> {
    let to_focus = eye + dir * reach - cam;
    let length = to_focus.length();
    if length < 1e-5 {
        return None;
    }
    let view_ray = to_focus / length;
    let (visible, _) = raycast(world, cam, view_ray, length)?;
    let min = visible.as_vec3();
    let distance = ray_box(cam, view_ray, min, min + Vec3::ONE, length)?;
    let hit_point = cam + view_ray * (distance + 0.01);
    let eye_ray = hit_point - eye;
    let eye_distance = eye_ray.length();
    if eye_distance < 1e-5 || eye_distance > reach + 0.05 {
        return None;
    }
    raycast(world, eye, eye_ray / eye_distance, eye_distance + 0.03)
        .filter(|(hit, _)| *hit == visible)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkData, GLASS, STONE};
    use std::sync::Arc;

    #[test]
    fn camera_keeps_clearance_from_solid_blocks_including_glass() {
        let mut world = World::new();
        world.chunks.insert((0, 0), Arc::new(ChunkData::new()));
        world.set(0, 2, 2, STONE);
        let eye = Vec3::new(0.5, 2.5, 0.5);
        let offset = Vec3::new(0.0, 0.0, 4.0);
        let clear = clearance(&world, eye, offset);
        assert!(clear > 1.0 && clear < 1.5);
        assert!(!occupied(&world, eye + clamp_offset(&world, eye, offset)));
        world.set(0, 2, 2, GLASS);
        assert!(clearance(&world, eye, offset) < 1.5);
    }

    #[test]
    fn shoulder_sides_have_opposite_offsets_and_neutral_midpoint() {
        let left = desired_local(3);
        let right = desired_local(4);
        assert!((left.x + right.x).abs() < 0.001);
        assert!((left.z - right.z).abs() < 0.001);
        assert!(((left + right) * 0.5).x.abs() < 0.001);
    }

    #[test]
    fn shoulder_pick_uses_screen_centre_and_eye_reach() {
        let mut world = World::new();
        world.chunks.insert((0, 0), Arc::new(ChunkData::new()));
        world.set(0, 2, 3, STONE);
        let eye = Vec3::new(0.5, 2.5, 0.5);
        // Far enough to the side that the camera sees past blocks right in front of the eye.
        let cam = Vec3::new(2.5, 2.5, -2.0);
        let picked = shoulder_target(&world, eye, Vec3::Z, cam, 5.0);
        assert_eq!(picked.map(|(p, _)| p), Some(IVec3::new(0, 2, 3)));
        // A block between the eye and the target, which the camera does not see: the eye
        // cannot reach what is under the crosshair, so nothing is picked.
        world.set(0, 2, 2, STONE);
        assert!(shoulder_target(&world, eye, Vec3::Z, cam, 5.0).is_none());
    }

    #[test]
    fn camera_turns_with_both_mouse_axes_during_a_shoulder_transition() {
        let midway = (desired_local(3) + desired_local(4)) * 0.5;
        let east = world_offset(midway, Vec3::X);
        let north = world_offset(midway, Vec3::Z);
        assert!((east.x + 3.2).abs() < 0.001);
        assert!((north.z + 3.2).abs() < 0.001);
        let down = world_offset(midway, Vec3::new(0.0, -0.6, 0.8));
        assert!(down.y > 1.9);
    }

    #[test]
    fn collision_recovery_cannot_pop_outward_or_enter_a_wall() {
        assert!((recover_distance(3.0, 1.0, false, 0.016) - 1.0).abs() < 0.001);
        let recovered = recover_distance(1.0, 3.0, false, 0.016);
        assert!(recovered > 1.0 && recovered < 1.2);
        assert!((recover_distance(1.0, 3.0, true, 0.016) - 3.0).abs() < 0.001);
    }

    #[test]
    fn head_is_continuous_when_camera_crosses_behind_player() {
        let before = head_turn(PI - 0.01, 0.0);
        let after = head_turn(-PI + 0.01, 0.0);
        assert!(before.abs() < 0.02);
        assert!(after.abs() < 0.02);
        assert!((head_turn(0.5, 0.0) - 0.5).abs() < 0.001);
    }

    #[test]
    fn f5_transition_policy_keeps_front_and_shoulder_entries_instant() {
        assert!(animate_switch(0, 1));
        assert!(!animate_switch(1, 2));
        assert!(!animate_switch(2, 3));
        assert!(animate_switch(3, 4));
        assert!(animate_switch(4, 0));
    }
}
