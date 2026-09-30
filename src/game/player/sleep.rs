//! Beds: using one sets where the player comes back to life, and at night they lie down in
//! it. When every player is asleep for a few seconds the night is skipped, like Minecraft.

use crate::game::*;
use crate::lang::tf;

/// Night, when beds can be slept in (Minecraft: ticks 12542..23459 of 24000).
pub fn is_night(time_of_day: f32) -> bool {
    (12542.0 / 24000.0..23459.0 / 24000.0).contains(&time_of_day)
}

/// Seconds everyone has to be asleep before the morning comes (Minecraft: 100 ticks).
const SKIP_AFTER: f32 = 5.0;

/// Lying in a bed.
#[derive(Clone, Copy, Debug)]
pub(in crate::game) struct Sleep {
    /// The bed's head half.
    pub bed: IVec3,
    pub facing: u8,
    /// Seconds in bed.
    pub time: f32,
}

impl Game {
    /// Right click on a bed: it becomes the respawn point, and at night the player lies down.
    pub(in crate::game) fn use_bed(&mut self, hit: IVec3) {
        self.action_cooldown = 0.25;
        let b = self.terrain.world.geti(hit);
        let head = if bed_head(b) {
            hit
        } else {
            hit + bed_other_half(b)
        };
        if !is_bed(self.terrain.world.geti(head)) {
            return;
        }
        self.bed_spawn = Some(head);
        self.say(t("bed.spawn_set"), chat::WHITE);
        if !is_night(self.time_of_day) {
            self.say(t("bed.no_sleep"), chat::WHITE);
            return;
        }
        if self.remote_in_bed(head) {
            self.say(t("bed.occupied"), chat::WHITE);
            return;
        }
        self.sleep = Some(Sleep {
            bed: head,
            facing: bed_facing(b),
            time: 0.0,
        });
        self.mining = None;
        self.using = None;
        self.player.flying = false;
        self.player.vel = Vec3::ZERO;
    }

    /// The player's frame while in bed (instead of moving): they lie still on it until they
    /// sneak, the morning comes or the bed is gone. Health and hunger go on.
    pub(in crate::game) fn update_sleep(&mut self, dt: f32, control: bool) {
        let Some(s) = self.sleep.as_mut() else {
            return;
        };
        s.time += dt;
        let s = *s;
        let b = self.terrain.world.geti(s.bed);
        let leave = control && self.bind_down(Bind::Sneak);
        if !(is_bed(b) && bed_head(b)) || !is_night(self.time_of_day) || leave {
            self.wake_up();
            return;
        }
        self.player.pos = s.bed.as_vec3() + Vec3::new(0.5, BED_HEIGHT, 0.5);
        self.player.vel = Vec3::ZERO;
        self.fall_peak = self.player.pos.y;
        let foot = -facing_dir(s.facing).as_vec3();
        self.body_yaw = foot.z.atan2(foot.x);
        self.limb_amount = 0.0;
        self.blocking = false;
        self.hand.blocking = false;
        self.target = None;
        self.mob_target = None;
        self.player_target = None;
        self.mining = None;
        let on_ground = self.player.on_ground;
        self.update_health(dt, on_ground);
        if self.screen == Screen::Dead {
            self.sleep = None;
        }
    }

    /// Gets out of bed onto a free spot beside it (or on top of it if there is none).
    pub(in crate::game) fn wake_up(&mut self) {
        let Some(s) = self.sleep.take() else {
            return;
        };
        let pos = self
            .bed_stand_pos(s.bed)
            .unwrap_or(s.bed.as_vec3() + Vec3::new(0.5, BED_HEIGHT, 0.5));
        self.player.pos = pos;
        self.player.vel = Vec3::ZERO;
        self.fall_peak = pos.y;
    }

    /// The camera in bed: the eyes of the model lying on the pillow.
    pub(in crate::game) fn sleep_eye(&self) -> Option<Vec3> {
        let s = self.sleep?;
        let head = facing_dir(s.facing).as_vec3();
        Some(s.bed.as_vec3() + Vec3::new(0.5, BED_HEIGHT + 0.3, 0.5) + head * 0.25)
    }

    /// Host and single player: the night is skipped once everyone has been asleep a while.
    pub(in crate::game) fn update_sleepers(&mut self, dt: f32) {
        if self.is_client() {
            return;
        }
        let (remotes, remotes_asleep) = self.remotes_asleep();
        // A spectator does not need to sleep for the night to pass.
        let here = self.player.spawned && self.screen != Screen::Dead && !self.spectator();
        let players = remotes + here as usize;
        let asleep = remotes_asleep + (here && self.sleep.is_some()) as usize;
        if players == 0 || asleep < players || !is_night(self.time_of_day) {
            self.asleep_for = 0.0;
            return;
        }
        self.asleep_for += dt;
        if self.asleep_for >= SKIP_AFTER {
            self.asleep_for = 0.0;
            self.time_of_day = 0.0;
            self.broadcast(&crate::net::Msg::Time(self.time_of_day), None);
        }
    }

    /// LAN: how many of the players are in bed, while this player waits in one.
    pub(in crate::game) fn sleep_status(&self) -> Option<String> {
        self.sleep?;
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
    pub(in crate::game) fn bed_stand_pos(&self, head: IVec3) -> Option<Vec3> {
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
    pub(in crate::game) fn home_pos(&self) -> Vec3 {
        self.bed_spawn
            .and_then(|b| self.bed_stand_pos(b))
            .unwrap_or_else(|| self.spawn_pos())
    }

    /// Back to life at home. A bed that is gone or blocked is forgotten (with a message).
    pub(in crate::game) fn spawn_at_home(&mut self, tell: bool) {
        self.spawn_player();
        let Some(bed) = self.bed_spawn else {
            return;
        };
        match self.bed_stand_pos(bed) {
            Some(pos) => {
                self.player.pos = pos;
                self.fall_peak = pos.y;
            }
            None => {
                self.bed_spawn = None;
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
