//! The revolver's cylinder: each pull of the trigger turns the next chamber under the hammer
//! (a live round fires and leaves its case there; an empty chamber or a fired case only
//! clicks), and the reload (the R key): the cylinder swings out, the ejector throws out what
//! is in it when there are fired cases (the cases fall and bounce, the live rounds drop to the
//! ground to be picked up), then it is loaded from a loaded speedloader carried, or one round
//! at a time from the magnum rounds carried (shooting stops the loading), and swung shut.

use crate::client::*;
use crate::audio::Sound;
use crate::item::*;
use crate::model::revolver_view::{
    LOAD_END, LOAD_SEAT, RELOAD_CLOSE, RELOAD_EJECT, RELOAD_END, RELOAD_OPEN, RELOAD_RELEASE,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    /// Swinging out (and the cases thrown out).
    Open,
    /// The speedloader brought to it and taken away.
    Loader,
    /// Rounds pushed in one at a time.
    Rounds,
    /// Swinging shut.
    Close,
}

/// A revolver reload going on.
#[derive(Clone, Copy, Debug)]
pub(in crate::client) struct Cylinder {
    phase: Phase,
    /// Seconds into the phase (into the round being loaded, loading rounds).
    t: f32,
    /// It empties the cylinder (there are fired cases in it); that is done.
    ejects: bool,
    ejected: bool,
    /// The chambers the speedloader fills, and the inventory slot it is in; they are in.
    loader: u8,
    loader_slot: Option<usize>,
    released: bool,
    /// The round being loaded is in.
    seated: bool,
    /// Asked to stop loading (a shot, or the reload key again): it is swung shut after the
    /// round being loaded.
    stop: bool,
}

impl Cylinder {
    /// Where the reload animation is (seconds) and, loading a round, how far into it.
    pub(in crate::client) fn anim(&self) -> (f32, Option<f32>) {
        match self.phase {
            Phase::Open => (self.t.min(RELOAD_OPEN), None),
            Phase::Loader => ((RELOAD_OPEN + self.t).min(RELOAD_CLOSE), None),
            Phase::Rounds => (RELOAD_OPEN, Some(self.t.min(LOAD_END))),
            Phase::Close => ((RELOAD_CLOSE + self.t).min(RELOAD_END), None),
        }
    }

    pub(in crate::client) fn ejects(&self) -> bool {
        self.ejects
    }

    pub(in crate::client) fn loader(&self) -> u8 {
        self.loader
    }
}

fn chambers_with(g: &Stack, what: u8) -> impl Iterator<Item = usize> + '_ {
    (0..6).filter(move |&k| revolver_chamber(g, k) == what)
}

/// The empty chambers in the order they come under the hammer (from the one after it).
fn empties_in_turn(g: &Stack) -> Vec<usize> {
    let mut k = revolver_index(g);
    let mut out = Vec::new();
    for _ in 0..6 {
        k = revolver_next(k);
        if revolver_chamber(g, k) == chamber::EMPTY {
            out.push(k);
        }
    }
    out
}

impl Game {
    fn held_revolver(&self) -> Option<Stack> {
        self.held_gun().filter(|(_, k)| *k == GunKind::Revolver).map(|(s, _)| s)
    }

    fn held_revolver_mut(&mut self) -> Option<&mut Stack> {
        let slot = self.hotbar_slot;
        self.inventory.slots[slot].as_mut().filter(|s| s.item == REVOLVER)
    }

    /// Rounds to load with (always, in creative).
    fn has_bullets(&self) -> bool {
        self.creative() || self.inventory.count(MAGNUM_ROUND) > 0
    }

    /// The fullest loaded speedloader carried: its slot and rounds.
    fn loaded_speedloader(&self) -> Option<(usize, u8)> {
        self.inventory
            .slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.filter(|s| s.item == SPEEDLOADER && gun_rounds(s) > 0).map(|s| (i, gun_rounds(&s))))
            .max_by_key(|&(i, r)| (r, std::cmp::Reverse(i)))
    }

    /// The R key with the revolver: swung out to be emptied (when there are fired cases in it)
    /// and loaded (when there is room and something to load it with).
    pub(in crate::client) fn start_revolver_reload(&mut self) {
        let Some(g) = self.held_revolver() else { return };
        if let Some(c) = &mut self.guns.cylinder {
            // Loading already: the key again stops it.
            c.stop = true;
            return;
        }
        let spent = chambers_with(&g, chamber::SPENT).count();
        let room = spent + chambers_with(&g, chamber::EMPTY).count();
        let can_load = self.has_bullets() || self.loaded_speedloader().is_some();
        if room == 0 {
            return;
        }
        if spent == 0 && !can_load {
            self.gun_message(t("gun.no_ammo"));
            return;
        }
        self.guns.cylinder = Some(Cylinder {
            phase: Phase::Open,
            t: 0.0,
            ejects: spent > 0,
            ejected: false,
            loader: 0,
            loader_slot: None,
            released: false,
            seated: false,
            stop: false,
        });
        self.guns.aim = 0.0;
        self.guns.reload = Some(0.0);
        self.guns.plan.length = RELOAD_END + if can_load { room as f32 * LOAD_END } else { 0.0 };
    }

    /// A shot asked for while the cylinder is out: loading stops after the round going in.
    pub(in crate::client) fn revolver_stop_loading(&mut self) {
        if let Some(c) = &mut self.guns.cylinder {
            c.stop = true;
        }
    }

    /// The reload goes on: its steps happen with the animation.
    pub(in crate::client) fn update_revolver_reload(&mut self, dt: f32) {
        let Some(mut c) = self.guns.cylinder else { return };
        if self.held_revolver().is_none() {
            self.guns.cylinder = None;
            self.guns.reload = None;
            return;
        }
        let at = self.eye();
        let was = c.t;
        c.t += dt;
        match c.phase {
            Phase::Open => {
                if was == 0.0 {
                    self.audio.play(Sound::CylinderOpen, Some(at), 0.8);
                }
                if c.t >= RELOAD_EJECT && !c.ejected {
                    c.ejected = true;
                    if c.ejects {
                        self.audio.play(Sound::CaseMagnum, Some(at), 0.2);
                        self.revolver_empty();
                    }
                }
                if c.t >= RELOAD_OPEN {
                    c.t = 0.0;
                    c.phase = self.after_open(&mut c);
                }
            }
            Phase::Loader => {
                if RELOAD_OPEN + c.t >= RELOAD_RELEASE && !c.released {
                    c.released = true;
                    self.audio.play(Sound::SpeedloaderIn, Some(at), 0.8);
                    self.revolver_from_loader(&c);
                }
                if RELOAD_OPEN + c.t >= RELOAD_CLOSE {
                    c.t = 0.0;
                    c.loader = 0;
                    c.phase = Phase::Close;
                }
            }
            Phase::Rounds => {
                if was < LOAD_SEAT && c.t >= LOAD_SEAT && !c.seated {
                    c.seated = true;
                    self.audio.play(Sound::RoundIn, Some(at), 0.6);
                    self.revolver_seat_round();
                }
                if c.t >= LOAD_END {
                    // Turned on to the next chamber; another round, or swung shut.
                    if let Some(g) = self.held_revolver_mut() {
                        let k = revolver_next(revolver_index(g));
                        set_revolver_index(g, k);
                    }
                    c.t = 0.0;
                    c.seated = false;
                    c.phase = if !c.stop && self.next_to_load() { Phase::Rounds } else { Phase::Close };
                }
            }
            Phase::Close => {
                if c.t >= RELOAD_END - RELOAD_CLOSE {
                    self.audio.play(Sound::CylinderShut, Some(at), 0.7);
                    self.revolver_align();
                    self.guns.cylinder = None;
                    self.guns.reload = None;
                    return;
                }
            }
        }
        self.guns.cylinder = Some(c);
        self.guns.reload = Some(self.guns.reload.unwrap_or(0.0) + dt);
    }

    /// Out and emptied: loaded from a speedloader, or a round at a time, or shut again.
    fn after_open(&mut self, c: &mut Cylinder) -> Phase {
        let Some(g) = self.held_revolver() else { return Phase::Close };
        let empties = empties_in_turn(&g);
        if empties.is_empty() {
            return Phase::Close;
        }
        if let Some((slot, rounds)) = self.loaded_speedloader() {
            c.loader_slot = Some(slot);
            c.loader = empties.iter().take(rounds as usize).fold(0, |m, &k| m | 1 << k);
            return Phase::Loader;
        }
        if !c.stop && self.next_to_load() {
            return Phase::Rounds;
        }
        Phase::Close
    }

    /// Whether another round goes in: there are bullets and an empty chamber (turned under the
    /// hammer, where the next one is pushed in).
    fn next_to_load(&mut self) -> bool {
        if !self.has_bullets() {
            return false;
        }
        let Some(g) = self.held_revolver_mut() else { return false };
        for _ in 0..6 {
            let k = revolver_index(g);
            if revolver_chamber(g, k) == chamber::EMPTY {
                return true;
            }
            set_revolver_index(g, revolver_next(k));
        }
        false
    }

    /// The round in the left hand pushed into the chamber under the hammer.
    fn revolver_seat_round(&mut self) {
        if !self.creative() {
            let Some(i) = self.inventory.slots.iter().position(|s| s.is_some_and(|s| s.item == MAGNUM_ROUND)) else {
                return;
            };
            crate::item::inventory::take(&mut self.inventory.slots[i], 1);
        }
        if let Some(g) = self.held_revolver_mut() {
            let k = revolver_index(g);
            set_revolver_chamber(g, k, chamber::LIVE);
        }
    }

    /// The speedloader lets go of its rounds into the chambers it was lined up with.
    /// Only as many as the speedloader still holds (it may have been moved or swapped since it
    /// was lined up).
    fn revolver_from_loader(&mut self, c: &Cylinder) {
        let mut n = 0;
        if let Some(slot) = c.loader_slot {
            if let Some(l) = self.inventory.slots[slot].as_mut().filter(|s| s.item == SPEEDLOADER) {
                n = gun_rounds(l).min(c.loader.count_ones() as u8);
                set_gun_rounds(l, gun_rounds(l) - n);
            }
        }
        if let Some(g) = self.held_revolver_mut() {
            for k in (0..6).filter(|k| c.loader & (1 << k) != 0).take(n as usize) {
                set_revolver_chamber(g, k, chamber::LIVE);
            }
        }
    }

    /// Swung shut: turned so the next pull fires a live round (as it is done by hand).
    fn revolver_align(&mut self) {
        let Some(g) = self.held_revolver_mut() else { return };
        if gun_rounds(g) == 0 {
            return;
        }
        for _ in 0..6 {
            let k = revolver_index(g);
            if revolver_chamber(g, revolver_next(k)) == chamber::LIVE {
                return;
            }
            set_revolver_index(g, revolver_next(k));
        }
    }

    /// The ejector pushed: everything comes out of the cylinder, each from its own chamber:
    /// the fired cases fall and bounce away, the live rounds drop to the ground (in creative
    /// they are gone).
    fn revolver_empty(&mut self) {
        let Some(g) = self.held_revolver() else { return };
        let eye = self.eye();
        let look = look_dir(self.yaw, self.pitch);
        let right = look.cross(Vec3::Y).normalize_or_zero();
        // Where each chamber's head is (the first-person gun, or near the hands).
        let fallback = self.guns.eject.or(self.guns.eject_tp).unwrap_or(eye - Vec3::Y * 0.3 + look * 0.45 - right * 0.1);
        let heads = self.guns.chambers.filter(|_| self.camera.mode == 0);
        // Out of the back of the cylinder: away from the muzzle.
        let back = match (heads, self.guns.muzzle.filter(|_| self.camera.mode == 0)) {
            (Some(h), Some(m)) => (h.iter().copied().sum::<Vec3>() / 6.0 - m).normalize_or(-look),
            _ => -look,
        };
        let creative = self.creative();
        for k in 0..6 {
            let what = revolver_chamber(&g, k);
            if what == chamber::EMPTY {
                continue;
            }
            let r = |g: &mut Self| g.random() - 0.5;
            let spread = Vec3::new(r(self), r(self), r(self)) * 0.5;
            let at = heads.map_or(fallback + spread * 0.08, |h| h[k]);
            let vel = back * (1.1 + r(self) * 0.4) + spread + Vec3::Y * 0.3 + self.player.vel * 0.8;
            if what == chamber::SPENT {
                let spin = Vec3::new(r(self), r(self), r(self)) * 18.0;
                self.guns.cases.eject(at, vel, spin, GunKind::Revolver);
            } else if !creative {
                self.add_item(crate::entity::dropped::ItemEntity::new(at, vel, Stack::one(MAGNUM_ROUND), 1.0));
            }
        }
        if let Some(g) = self.held_revolver_mut() {
            for k in 0..6 {
                set_revolver_chamber(g, k, chamber::EMPTY);
            }
        }
    }

    /// The trigger pulled: the next chamber comes under the hammer. Returns whether a round
    /// fired there (otherwise the hammer only clicked on an empty chamber or a fired case).
    pub(in crate::client) fn revolver_pull(&mut self) -> bool {
        let Some(g) = self.held_revolver_mut() else { return false };
        let k = revolver_next(revolver_index(g));
        set_revolver_index(g, k);
        if revolver_chamber(g, k) == chamber::LIVE {
            set_revolver_chamber(g, k, chamber::SPENT);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_partly_loaded_cylinder_fires_in_turn() {
        let mut g = Stack::one(REVOLVER);
        set_gun_rounds(&mut g, 2);
        assert_eq!(gun_rounds(&g), 2);
        // The two rounds are the next two to come under the hammer.
        let mut k = revolver_index(&g);
        for i in 0..6 {
            k = revolver_next(k);
            assert_eq!(revolver_chamber(&g, k) == chamber::LIVE, i < 2);
        }
        assert_eq!(empties_in_turn(&g).len(), 4);
        assert_eq!(gun_mods(&g), 0);
        assert!(gun_has_mag(&g) && gun_chambered(&g));
    }
}

