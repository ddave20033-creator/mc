//! Gun stations: what lies on their table and in their drawer, and the last change there.

use crate::item::Stack;

/// Something lying on a gun station's table: where on it (blocks from the middle of its top,
/// `x` to the right, `z` toward the front), and turned how far about the up axis (radians).
/// `id` tells it apart while things on the table move (the animations follow it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BenchItem {
    pub id: u16,
    pub stack: Stack,
    pub x: f32,
    pub z: f32,
    pub turn: f32,
}

/// What happened last on a gun station's table, for everyone to play out (`bench_event`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BenchEvent {
    /// Counts up with every change; 0: none yet.
    pub serial: u16,
    pub kind: u8,
    /// The gun it happened to (by id: taken apart, put together, an attachment on or off).
    pub gun: u16,
    /// The attachment put on or taken off (`gun_mod` bit).
    pub bit: u8,
    /// What was on the table before and is not now (the gun taken apart, the parts put
    /// together, the attachment put on), as it lay.
    pub gone: Vec<BenchItem>,
    /// What came of it (the parts taken apart, the attachment taken off), by id.
    pub made: Vec<u16>,
}

/// `BenchEvent::kind`.
pub mod bench_event {
    pub const NONE: u8 = 0;
    pub const STRIP: u8 = 1;
    pub const ASSEMBLE: u8 = 2;
    pub const FIT: u8 = 3;
    pub const UNFIT: u8 = 4;
    /// Rounds pushed into a magazine (`gun`), `bit` of them.
    pub const LOAD: u8 = 5;
    /// A gun's magazine taken out of it (it lies beside it: `made`), or one put into it
    /// (`gone`: as it lay).
    pub const MAG_OUT: u8 = 6;
    pub const MAG_IN: u8 = 7;
}

/// A gun station: what lies on its table, the boxes of rounds in the three places for them in
/// its drawer (the rounds in each; None: taken out), the rifle station's magazine loader in
/// the middle of its drawer (there or not, and the magazine on it), and the last change there.
#[derive(Clone, Debug, PartialEq)]
pub struct GunBench {
    pub items: Vec<BenchItem>,
    pub next_id: u16,
    pub boxes: [Option<u16>; 3],
    pub loader: bool,
    pub loader_mag: Option<Stack>,
    pub event: BenchEvent,
    /// The rifle station's grenade crate on its shelf: how many frag and smoke grenades.
    pub grenades: [u8; 2],
}

impl Default for GunBench {
    /// A new one: three empty boxes in the drawer.
    fn default() -> Self {
        GunBench { items: Vec::new(), next_id: 0, boxes: [Some(0); 3], loader: false, loader_mag: None, event: BenchEvent::default(), grenades: [0; 2] }
    }
}

/// Seconds the rifle station's loader takes to push each round into the magazine on it (its
/// "feed" animation's length).
pub const LOADER_ROUND: f32 = 0.35;

impl GunBench {
    /// The box the loader takes its next round from (for the magazine on it): which, if any.
    pub fn loader_source(&self) -> Option<usize> {
        use crate::item::{box_ammo, gun_rounds, magazine_capacity, magazine_gun};
        let mag = self.loader_mag.filter(|_| self.loader)?;
        let kind = magazine_gun(mag.item)?;
        if gun_rounds(&mag) >= magazine_capacity(mag.item).unwrap_or(0) {
            return None;
        }
        self.boxes.iter().position(|b| b.and_then(box_ammo) == Some(kind.ammo()))
    }

    /// Puts a stack on the table; returns its id.
    pub fn add(&mut self, stack: Stack, x: f32, z: f32, turn: f32) -> u16 {
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let id = self.next_id;
        self.items.push(BenchItem { id, stack, x, z, turn });
        id
    }

    pub fn get(&self, id: u16) -> Option<&BenchItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn take(&mut self, id: u16) -> Option<BenchItem> {
        let i = self.items.iter().position(|i| i.id == id)?;
        Some(self.items.remove(i))
    }
}
