//! Entity updates as changes: the host keeps, for each player, the mobs and dropped items
//! as that player last got them, and sends only what is new, what changed (beyond a little)
//! and the ids of what is gone. Everything goes over TCP, so nothing is lost on the way; an
//! entry is still sent again every `REFRESH` seconds, so anything the player counts on by
//! itself (an item's age, a dying mob's seconds) never drifts far.

use super::{ItemNet, MobNet, Msg};
use crate::world::block::Block;
use crate::world::{FastMap, FastSet};
use glam::Vec3;

/// Seconds after which an entry is sent again even if it did not change.
pub const REFRESH: f32 = 5.0;
/// How far a mob or item may move (blocks), or turn or swing its limbs (radians), before
/// the player gets it again.
const POS_EPS: f32 = 0.01;
const ANGLE_EPS: f32 = 0.01;
/// Seconds a dying mob's death time may run apart (the player counts it on itself).
const DEATH_EPS: f32 = 0.5;

/// Whether the player's copy `was` of a mob is too far from `now` to keep.
pub fn mob_changed(was: &MobNet, now: &MobNet) -> bool {
    let near = |a: f32, b: f32| (a - b).abs() <= ANGLE_EPS;
    was.kind != now.kind
        || was.pos.distance_squared(now.pos) > POS_EPS * POS_EPS
        || !near(was.body_yaw, now.body_yaw)
        || !near(was.head_yaw, now.head_yaw)
        || !near(was.pitch, now.pitch)
        || !near(was.limb_swing, now.limb_swing)
        || !near(was.limb_amount, now.limb_amount)
        || was.hurt != now.hurt
        || (was.death >= 0.0) != (now.death >= 0.0)
        || (was.death - now.death).abs() > DEATH_EPS
        || was.health != now.health
        || was.sheared != now.sheared
        || was.taken != now.taken
        || was.last_hit != now.last_hit
        || was.flags != now.flags
        || was.collar != now.collar
}

/// Whether the player's copy of a dropped item is too far from `now` (its age the player
/// counts on itself).
pub fn item_changed(was: &ItemNet, now: &ItemNet) -> bool {
    was.stack != now.stack || was.pos.distance_squared(now.pos) > POS_EPS * POS_EPS
}

/// What one player has of the entities near them (kept by the host).
#[derive(Default)]
pub struct EntitySync {
    /// Each entry as last sent, and when (host time).
    mobs: FastMap<u32, (MobNet, f32)>,
    items: FastMap<u32, (ItemNet, f32)>,
    falling: Vec<(Vec3, Block)>,
    started: bool,
}

impl EntitySync {
    /// The message that brings the player from what they have to `mobs`, `items` and
    /// `falling` (everything near them now), or None if nothing changed. The first one lists
    /// everything (`full`).
    pub fn update(
        &mut self,
        now: f32,
        mobs: &[MobNet],
        items: &[ItemNet],
        falling: &[(Vec3, Block)],
    ) -> Option<Msg> {
        let full = !self.started;
        self.started = true;
        let (mobs, gone_mobs) = changes(&mut self.mobs, mobs, now, |m| m.id, mob_changed);
        let (items, gone_items) = changes(&mut self.items, items, now, |i| i.id, item_changed);
        let falling_changed = self.falling != falling;
        if falling_changed {
            self.falling = falling.to_vec();
        }
        let nothing = mobs.is_empty()
            && items.is_empty()
            && gone_mobs.is_empty()
            && gone_items.is_empty()
            && !falling_changed;
        if nothing && !full {
            return None;
        }
        Some(Msg::Entities {
            full,
            mobs,
            items,
            gone_mobs,
            gone_items,
            falling: falling.to_vec(),
        })
    }
}

/// The entries of `now` that are new or changed (or not sent for `REFRESH` seconds), and
/// the ids in `sent` that are no longer there; `sent` becomes what the player has after.
fn changes<T: Copy>(
    sent: &mut FastMap<u32, (T, f32)>,
    now: &[T],
    time: f32,
    id: impl Fn(&T) -> u32,
    changed: impl Fn(&T, &T) -> bool,
) -> (Vec<T>, Vec<u32>) {
    let mut out = Vec::new();
    for e in now {
        match sent.get_mut(&id(e)) {
            Some((was, at)) if !changed(was, e) && time - *at < REFRESH => {}
            Some(entry) => {
                *entry = (*e, time);
                out.push(*e);
            }
            None => {
                sent.insert(id(e), (*e, time));
                out.push(*e);
            }
        }
    }
    let mut gone = Vec::new();
    // (everything in `now` is in `sent` by now: with as many entries, nothing is gone)
    if sent.len() != now.len() {
        let here: FastSet<u32> = now.iter().map(&id).collect();
        sent.retain(|k, _| {
            let keep = here.contains(k);
            if !keep {
                gone.push(*k);
            }
            keep
        });
    }
    gone.sort_unstable();
    (out, gone)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::Stack;
    use std::collections::BTreeMap;

    /// A player's copy, changed by the messages the way the game's is (`sync_entities`).
    #[derive(Default)]
    struct View {
        mobs: BTreeMap<u32, MobNet>,
        items: BTreeMap<u32, ItemNet>,
        falling: Vec<(Vec3, Block)>,
    }

    impl View {
        fn apply(&mut self, m: &Msg) {
            let Msg::Entities { full, mobs, items, gone_mobs, gone_items, falling } = m else {
                panic!("not entities");
            };
            if *full {
                self.mobs.clear();
                self.items.clear();
            }
            for id in gone_mobs {
                assert!(self.mobs.remove(id).is_some(), "mob {id} gone twice");
            }
            for id in gone_items {
                assert!(self.items.remove(id).is_some(), "item {id} gone twice");
            }
            self.mobs.extend(mobs.iter().map(|m| (m.id, *m)));
            self.items.extend(items.iter().map(|i| (i.id, *i)));
            self.falling = falling.clone();
        }
    }

    fn mob(id: u32, x: f32) -> MobNet {
        MobNet {
            id,
            kind: 1,
            pos: Vec3::new(x, 64.0, 0.0),
            body_yaw: 0.0,
            head_yaw: 0.0,
            pitch: 0.0,
            limb_swing: x,
            limb_amount: 0.5,
            hurt: false,
            death: -1.0,
            health: 10.0,
            sheared: false,
            taken: 0.0,
            last_hit: 0.0,
            flags: 0,
            collar: 0,
        }
    }

    fn item(id: u32, y: f32, count: u8) -> ItemNet {
        ItemNet { id, pos: Vec3::new(0.0, y, 3.0), stack: Stack::new(5, count), age: 0.0 }
    }

    /// The player's copy matches the host's (within the tolerances), entry by entry.
    fn same(view: &View, mobs: &[MobNet], items: &[ItemNet], falling: &[(Vec3, Block)]) {
        assert_eq!(view.mobs.len(), mobs.len());
        for m in mobs {
            let v = view.mobs.get(&m.id).expect("mob missing");
            assert!(!mob_changed(v, m), "mob {} differs: {v:?} / {m:?}", m.id);
        }
        assert_eq!(view.items.len(), items.len());
        for i in items {
            let v = view.items.get(&i.id).expect("item missing");
            assert!(!item_changed(v, i), "item {} differs", i.id);
        }
        assert_eq!(view.falling, falling);
    }

    #[test]
    fn deltas_rebuild_the_hosts_state() {
        let mut sync = EntitySync::default();
        let mut view = View::default();
        let mut sizes = Vec::new();
        // A little world going on: mobs walking and one standing, items falling then resting,
        // some coming into range and going out, a falling block now and then.
        for tick in 0..400u32 {
            let t = tick as f32 * 0.05;
            let mut mobs: Vec<MobNet> = (0..20u32)
                .filter(|i| (tick / 40 + i) % 7 != 0)
                .map(|i| mob(i, if i % 2 == 0 { t * 0.5 + i as f32 } else { i as f32 }))
                .collect();
            if tick % 50 < 5 {
                mobs[0].hurt = true;
            }
            let items: Vec<ItemNet> = (0..15u32)
                .filter(|i| tick < 300 || i % 3 != 0)
                .map(|i| item(100 + i, (70.0 - t * 4.0).max(64.0 + i as f32 * 0.001), 1 + (tick > 200 && i == 4) as u8))
                .collect();
            let falling: Vec<(Vec3, Block)> =
                if tick % 60 < 10 { vec![(Vec3::new(1.0, 80.0 - t, 1.0), 12)] } else { vec![] };
            let msg = sync.update(t, &mobs, &items, &falling);
            if tick == 0 {
                assert!(matches!(msg, Some(Msg::Entities { full: true, .. })));
            }
            if let Some(m) = &msg {
                // (through the wire, as it really goes)
                let m = Msg::decode(&m.encode()).unwrap();
                view.apply(&m);
                sizes.push(m.encode().len());
            }
            same(&view, &mobs, &items, &falling);
        }
        // Resting things are not sent 20 times a second: far fewer bytes than full lists.
        let full_size = Msg::Entities {
            full: false,
            mobs: (0..17).map(|i| mob(i, 0.0)).collect(),
            items: (0..15).map(|i| item(i, 0.0, 1)).collect(),
            gone_mobs: vec![],
            gone_items: vec![],
            falling: vec![],
        }
        .encode()
        .len();
        let total: usize = sizes.iter().sum();
        assert!(total * 2 < full_size * 400, "{total} vs {}", full_size * 400);
    }

    #[test]
    fn nothing_changed_sends_nothing_until_refresh() {
        let mut sync = EntitySync::default();
        let mobs = [mob(1, 0.0)];
        assert!(sync.update(0.0, &mobs, &[], &[]).is_some());
        assert!(sync.update(0.05, &mobs, &[], &[]).is_none());
        // Moved less than the tolerance: still nothing.
        let mut m = mobs[0];
        m.pos.x += 0.004;
        assert!(sync.update(0.1, &[m], &[], &[]).is_none());
        // Every few seconds it goes again.
        let Some(Msg::Entities { full, mobs: sent, .. }) = sync.update(REFRESH + 0.1, &[m], &[], &[]) else {
            panic!("no refresh");
        };
        assert!(!full);
        assert_eq!(sent, vec![m]);
        // Gone out of range: its id goes.
        let Some(Msg::Entities { gone_mobs, .. }) = sync.update(REFRESH + 0.2, &[], &[], &[]) else {
            panic!("not gone");
        };
        assert_eq!(gone_mobs, vec![1]);
    }
}
