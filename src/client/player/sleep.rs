//! Beds: using one sets where the player comes back to life, and at night they lie down in
//! it. When every player is asleep for a few seconds the night is skipped, like Minecraft.

use crate::client::{Game, Screen};
use crate::app::keys::Bind;
use crate::app::lang::{t, tf};
use crate::ui::chat;
use crate::world::*;
use glam::{IVec3, Vec3};

pub use crate::sim::is_night;

/// Lying in a bed.
#[derive(Clone, Copy, Debug)]
pub(in crate::client) struct Sleep {
    /// The bed's head half.
    bed: IVec3,
    pub facing: u8,
    /// Seconds in bed.
    pub time: f32,
}

impl Game {
    /// Right click on a bed: it becomes the respawn point, and at night the player lies down.
    pub(super) fn use_bed(&mut self, hit: IVec3) {
        self.me.aim.action_cooldown = 0.25;
        let b = self.terrain.world.geti(hit);
        let head = if bed_head(b) {
            hit
        } else {
            hit + bed_other_half(b)
        };
        if !is_bed(self.terrain.world.geti(head)) {
            return;
        }
        self.me.vitals.bed_spawn = Some(head);
        self.say(t("bed.spawn_set"), chat::WHITE);
        if !is_night(self.level.time_of_day) {
            self.say(t("bed.no_sleep"), chat::WHITE);
            return;
        }
        if self.remote_in_bed(head) {
            self.say(t("bed.occupied"), chat::WHITE);
            return;
        }
        self.me.vitals.sleep = Some(Sleep {
            bed: head,
            facing: bed_facing(b),
            time: 0.0,
        });
        self.me.aim.mining = None;
        self.me.aim.using = None;
        self.me.body.flying = false;
        self.me.body.vel = Vec3::ZERO;
    }

    /// The player's frame while in bed (instead of moving): they lie still on it until they
    /// sneak, the morning comes or the bed is gone. Health and hunger go on.
    pub(in crate::client) fn update_sleep(&mut self, dt: f32, control: bool) {
        let Some(s) = self.me.vitals.sleep.as_mut() else {
            return;
        };
        s.time += dt;
        let s = *s;
        let b = self.terrain.world.geti(s.bed);
        let leave = control && self.bind_down(Bind::Sneak);
        if !(is_bed(b) && bed_head(b)) || !is_night(self.level.time_of_day) || leave {
            self.wake_up();
            return;
        }
        self.me.body.pos = s.bed.as_vec3() + Vec3::new(0.5, BED_HEIGHT, 0.5);
        self.me.body.start_tick();
        self.me.body.vel = Vec3::ZERO;
        self.me.vitals.fall_peak = self.me.body.pos.y;
        let foot = -facing_dir(s.facing).as_vec3();
        self.me.look.body_yaw = foot.z.atan2(foot.x);
        self.me.look.limb_amount = 0.0;
        self.me.aim.let_go();
        self.me.hand.blocking = false;
        let on_ground = self.me.body.on_ground;
        self.update_health(dt, on_ground);
        if self.screen == Screen::Dead {
            self.me.vitals.sleep = None;
        }
    }

    /// Gets out of bed onto a free spot beside it (or on top of it if there is none).
    pub(in crate::client) fn wake_up(&mut self) {
        let Some(s) = self.me.vitals.sleep.take() else {
            return;
        };
        let pos = self
            .bed_stand_pos(s.bed)
            .unwrap_or(s.bed.as_vec3() + Vec3::new(0.5, BED_HEIGHT, 0.5));
        self.me.body.pos = pos;
        self.me.body.start_tick();
        self.me.body.vel = Vec3::ZERO;
        self.me.vitals.fall_peak = pos.y;
    }

    /// The camera in bed: the eyes of the model lying on the pillow.
    pub(in crate::client) fn sleep_eye(&self) -> Option<Vec3> {
        let s = self.me.vitals.sleep?;
        let head = facing_dir(s.facing).as_vec3();
        Some(s.bed.as_vec3() + Vec3::new(0.5, BED_HEIGHT + 0.3, 0.5) + head * 0.25)
    }

    /// LAN: how many of the players are in bed, while this player waits in one.
    pub(in crate::client) fn sleep_status(&self) -> Option<String> {
        self.me.vitals.sleep?;
        let (remotes, asleep) = self.remotes_asleep();
        (remotes > 0).then(|| {
            tf(
                "bed.waiting",
                &[&(asleep + 1).to_string(), &(remotes + 1).to_string()],
            )
        })
    }

    /// A block anywhere in the world: loaded, saved or (for chunks never changed) empty.
    fn block_anywhere(&self, p: IVec3) -> Block {
        let w = &self.terrain.world;
        if !(0..HEIGHT as i32).contains(&p.y) {
            return AIR;
        }
        let cp = World::chunk_pos(p.x, p.z);
        match w.chunks.get(&cp).or_else(|| w.saved.get(&cp)) {
            Some(c) => c.get(
                p.x.rem_euclid(16) as usize,
                p.y as usize,
                p.z.rem_euclid(16) as usize,
            ),
            None => AIR,
        }
    }

    /// Where to stand beside the bed whose head half is at `head`: a free spot two blocks
    /// high with ground under it, around the foot half first (Minecraft's order is similar).
    fn bed_stand_pos(&self, head: IVec3) -> Option<Vec3> {
        let b = self.block_anywhere(head);
        if !is_bed(b) {
            return None;
        }
        let foot = head + bed_other_half(b);
        let get = |q: IVec3| self.block_anywhere(q);
        let free = |q: IVec3| {
            let (at, above, below) = (get(q), get(q + IVec3::Y), get(q - IVec3::Y));
            !is_solid(at)
                && !is_lava(at)
                && !is_solid(above)
                && !is_lava(above)
                && is_solid(below)
                && !is_bed(below)
        };
        for half in [foot, head] {
            for dy in [0, 1, -1] {
                for (dx, dz) in [
                    (0, 1),
                    (0, -1),
                    (1, 0),
                    (-1, 0),
                    (1, 1),
                    (-1, 1),
                    (1, -1),
                    (-1, -1),
                ] {
                    let q = half + IVec3::new(dx, dy, dz);
                    if q.x == foot.x && q.z == foot.z || q.x == head.x && q.z == head.z {
                        continue;
                    }
                    if free(q) {
                        return Some(q.as_vec3() + Vec3::new(0.5, 0.0, 0.5));
                    }
                }
            }
        }
        None
    }

    /// Where the player comes back to life: beside their bed, else at the world spawn.
    pub(in crate::client) fn home_pos(&self) -> Vec3 {
        self.me.vitals.bed_spawn
            .and_then(|b| self.bed_stand_pos(b))
            .unwrap_or_else(|| self.spawn_pos())
    }

    /// Back to life at home. A bed that is gone or blocked is forgotten (with a message).
    pub(in crate::client) fn spawn_at_home(&mut self, tell: bool) {
        self.spawn_player();
        let Some(bed) = self.me.vitals.bed_spawn else {
            return;
        };
        match self.bed_stand_pos(bed) {
            Some(pos) => {
                self.me.body.pos = pos;
                self.me.body.start_tick();
                self.me.vitals.fall_peak = pos.y;
            }
            None => {
                self.me.vitals.bed_spawn = None;
                if tell {
                    self.say(t("bed.missing"), chat::WHITE);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nights_are_minecrafts() {
        assert!(!is_night(0.0));
        assert!(!is_night(0.25));
        assert!(!is_night(0.5));
        assert!(is_night(0.55));
        assert!(is_night(0.75));
        assert!(!is_night(0.99));
    }
}
