//! Furnaces: smelting in the mouth, fuel in the firebox, and meat grilled on the top.

use crate::item::inventory::{add_to, take};
use crate::item::{
    fuel_time, icon, meat, meat_with_sides, smelt, smelt_tier, Icon, ItemId, Slot, Stack, BUCKET,
    LAVA_BUCKET, MEAT_SIDES,
};
use crate::world::textures::tex;
use glam::Vec3;

/// Seconds an item takes to smelt in a furnace (the better ones are faster, see
/// `Furnace::smelt_time`).
pub const SMELT_TIME: f32 = 10.0;
/// Seconds a side of meat needs on the fire, and after how long it burns.
pub const GRILL_TIME: f32 = 10.0;
pub const BURN_TIME: f32 = 20.0;
/// Seconds a flip takes (the meat rises, turns over and settles back).
pub const FLIP_TIME: f32 = 0.55;

/// A piece of meat lying on one corner of a furnace's top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grilled {
    /// The raw meat (`item::meat` gives what it becomes).
    pub raw: ItemId,
    /// Seconds each side (0: the sprite's front, 1: its back) has spent on the fire.
    pub cook: [f32; 2],
    /// The side lying on the fire.
    pub down: u8,
    /// Seconds left of a flip in progress.
    pub flip: f32,
}

/// How done one side of a piece of meat is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Doneness {
    Raw,
    Cooked,
    Burnt,
}

pub fn doneness(t: f32) -> Doneness {
    if t >= BURN_TIME {
        Doneness::Burnt
    } else if t >= GRILL_TIME {
        Doneness::Cooked
    } else {
        Doneness::Raw
    }
}

impl Grilled {
    /// Meat put on the fire, its sides as the item says (see `item::MEAT_SIDES`), with the
    /// less done side down. Fully burnt meat does not go back on.
    pub fn new(item: ItemId) -> Option<Self> {
        let m = meat(item)?;
        let sides = MEAT_SIDES[m.iter().position(|&i| i == item)?];
        if sides == [2, 2] {
            return None;
        }
        let time = |d: u8| [0.0, GRILL_TIME, BURN_TIME][d as usize];
        Some(Self {
            raw: m[0],
            cook: sides.map(time),
            down: 1,
            flip: 0.0,
        })
    }

    /// The item it is when taken off: the variant with its two sides as they are.
    pub fn item(&self) -> ItemId {
        meat_with_sides(self.raw, self.cook.map(|t| doneness(t) as u8))
    }

    /// The texture of one side for how done it is: the raw, cooked or burnt meat's own (they
    /// have the same shape, so both sides of the piece match).
    pub fn side_layer(&self, side: usize) -> u32 {
        let m = meat(self.raw).unwrap_or([self.raw; 6]);
        let item = match doneness(self.cook[side]) {
            Doneness::Raw => m[0],
            Doneness::Cooked => m[2],
            Doneness::Burnt => m[4],
        };
        match icon(item) {
            Icon::Flat(l) => l,
            Icon::Block(_) => tex::STONE,
        }
    }
}

/// Parts of a furnace a player can put things into: the four corners of the top (0..4, the
/// grill for meat), and the front's upper (things to smelt) and lower (fuel) halves.
pub mod part {
    pub const INPUT: u8 = 4;
    pub const FUEL: u8 = 5;
}

/// Grill corner `i` (along x, then z) as a box in the block (0..1).
pub fn grill_box(i: usize) -> (Vec3, Vec3) {
    let x = (i % 2) as f32 * 0.5 + 0.035;
    let z = (i / 2) as f32 * 0.5 + 0.035;
    (Vec3::new(x, 1.0, z), Vec3::new(x + 0.43, 1.07, z + 0.43))
}

#[derive(Default, Clone, PartialEq)]
pub struct Furnace {
    pub input: Slot,
    pub fuel: Slot,
    /// What is smelted, staying in the mouth (with what is still to smelt) until taken.
    pub output: Slot,
    /// Seconds of fuel left.
    pub burn: f32,
    pub burn_total: f32,
    /// Seconds spent smelting the current item.
    pub cook: f32,
    /// Meat on the four corners of the top.
    pub grill: [Option<Grilled>; 4],
    /// 1 furnace, 2 blast furnace, 3 advanced furnace (`furnace_tier` of its block, kept
    /// up to date by the game; 0 counts as 1).
    pub tier: u8,
}

/// What a player did at a furnace: how many of the offered items went in, and what they
/// get back.
#[derive(Default, Debug, PartialEq)]
pub struct UseResult {
    pub used: u8,
    pub give: Vec<Stack>,
}

impl Furnace {
    /// What `item` smelts into here: a furnace of too low a tier does not smelt it.
    pub fn smelts(&self, item: ItemId) -> Option<ItemId> {
        smelt(item).filter(|_| smelt_tier(item) <= self.tier.max(1))
    }

    /// Seconds one item takes: 10 in a furnace, 7 in a blast furnace, 4 in an advanced one.
    pub fn smelt_time(&self) -> f32 {
        match self.tier {
            3 => 4.0,
            2 => 7.0,
            _ => SMELT_TIME,
        }
    }

    fn can_smelt(&self) -> bool {
        let Some(out) = self.input.and_then(|s| self.smelts(s.item)) else {
            return false;
        };
        match self.output {
            None => true,
            Some(o) => o.item == out && o.count < crate::item::max_stack(out),
        }
    }

    /// Something to heat: meat on the grill or an item to smelt.
    fn needs_heat(&self) -> bool {
        self.grill.iter().any(|g| g.is_some()) || self.can_smelt()
    }

    /// Advances the furnace. Returns true while it is burning.
    pub fn update(&mut self, dt: f32) -> bool {
        for g in self.grill.iter_mut().flatten() {
            g.flip = (g.flip - dt).max(0.0);
        }
        let smeltable = self.can_smelt();
        if self.burn <= 0.0 && self.needs_heat() {
            if let Some(f) = self.fuel.and_then(|s| fuel_time(s.item).map(|t| (s, t))) {
                self.burn = f.1;
                self.burn_total = f.1;
                if f.0.item == LAVA_BUCKET {
                    self.fuel = Some(Stack::one(BUCKET));
                } else {
                    take(&mut self.fuel, 1);
                }
            }
        }
        if self.burn > 0.0 {
            self.burn -= dt;
            // The side on the fire cooks (not while it is up in the air being turned).
            for g in self.grill.iter_mut().flatten() {
                if g.flip <= 0.0 {
                    g.cook[g.down as usize] += dt;
                }
            }
            if smeltable {
                self.cook += dt;
                if self.cook >= self.smelt_time() {
                    self.cook = 0.0;
                    let out = self.smelts(self.input.unwrap().item).unwrap();
                    take(&mut self.input, 1);
                    match &mut self.output {
                        Some(o) => o.count += 1,
                        None => self.output = Some(Stack::one(out)),
                    }
                }
            } else {
                self.cook = 0.0;
            }
            true
        } else {
            self.cook = (self.cook - dt * 2.0).max(0.0);
            false
        }
    }

    /// Whether `item` can go into `part`: meat on an empty grill corner, things to smelt
    /// (not meat, that goes on top) in the front's upper half, fuel in the lower half.
    pub fn accepts(&self, part: u8, item: ItemId) -> bool {
        let fits = |slot: Slot| match slot {
            None => true,
            Some(s) => s.item == item && s.count < crate::item::max_stack(item),
        };
        match part {
            0..=3 => self.grill[part as usize].is_none() && Grilled::new(item).is_some(),
            part::INPUT => meat(item).is_none() && self.smelts(item).is_some() && fits(self.input),
            part::FUEL => fuel_time(item).is_some() && fits(self.fuel),
            _ => false,
        }
    }

    /// Something to take from (or turn over at) `part`.
    pub fn has(&self, part: u8) -> bool {
        match part {
            0..=3 => self.grill[part as usize].is_some(),
            part::INPUT => self.input.is_some() || self.output.is_some(),
            part::FUEL => self.fuel.is_some(),
            _ => false,
        }
    }

    /// A player uses `part` holding `held`: with `take` (a left click) whatever is there
    /// comes out (a piece of meat, what is smelted before what is still smelting, the fuel);
    /// otherwise (a right click) meat goes onto an empty corner or the meat there is turned
    /// over, and the front takes the held stack into its half.
    pub fn use_part(&mut self, part: u8, held: Slot, take: bool) -> UseResult {
        let mut r = UseResult::default();
        match part {
            0..=3 => {
                let g = &mut self.grill[part as usize];
                match g {
                    Some(m) if take => {
                        r.give.push(Stack::one(m.item()));
                        *g = None;
                    }
                    Some(m) => {
                        if m.flip <= 0.0 {
                            m.down ^= 1;
                            m.flip = FLIP_TIME;
                        }
                    }
                    None if !take => {
                        if let Some(new) = held.and_then(|h| Grilled::new(h.item)) {
                            *g = Some(new);
                            r.used = 1;
                        }
                    }
                    None => {}
                }
            }
            part::INPUT | part::FUEL if take => {
                let out = if part == part::FUEL {
                    self.fuel.take()
                } else {
                    self.output.take().or_else(|| self.input.take())
                };
                r.give.extend(out);
            }
            part::INPUT | part::FUEL => {
                if let Some(h) = held.filter(|h| self.accepts(part, h.item)) {
                    let slot = if part == part::INPUT {
                        &mut self.input
                    } else {
                        &mut self.fuel
                    };
                    let left = add_to(std::slice::from_mut(slot), h);
                    r.used = h.count - left.map_or(0, |l| l.count);
                }
            }
            _ => {}
        }
        r
    }

    /// Everything in the furnace, as items (meat as it is now).
    pub fn contents(&self) -> Vec<Stack> {
        let mut out: Vec<Stack> = [self.input, self.fuel, self.output]
            .into_iter()
            .flatten()
            .collect();
        out.extend(self.grill.iter().flatten().map(|g| Stack::one(g.item())));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{
        BURNT_PORKCHOP, COAL, COOKED_PORKCHOP, HALF_BURNT_PORKCHOP, HALF_COOKED_PORKCHOP,
        IRON_INGOT, PORKCHOP, RAW_BURNT_PORKCHOP,
    };

    use crate::world::*;
    fn lit_furnace() -> Furnace {
        Furnace {
            fuel: Some(Stack::new(COAL, 4)),
            ..Default::default()
        }
    }

    fn run(f: &mut Furnace, seconds: f32) {
        for _ in 0..(seconds * 20.0).round() as usize {
            f.update(0.05);
        }
    }

    #[test]
    fn meat_cooks_one_side_at_a_time_and_burns() {
        let mut f = lit_furnace();
        let r = f.use_part(0, Some(Stack::new(PORKCHOP, 5)), false);
        assert_eq!(r.used, 1);
        run(&mut f, 10.2);
        // Only the side on the fire is done: half cooked if taken off now.
        assert_eq!(f.grill[0].unwrap().item(), HALF_COOKED_PORKCHOP);
        // Turned over (right click): nothing cooks while it is in the air, then the other
        // side does.
        f.use_part(0, None, false);
        run(&mut f, 10.8);
        let g = f.grill[0].unwrap();
        assert_eq!(g.item(), COOKED_PORKCHOP);
        assert!(g.cook.iter().all(|&t| (10.0..20.0).contains(&t)), "{g:?}");
        // Left on too long, it burns. A left click takes it off.
        run(&mut f, 10.0);
        assert_eq!(f.grill[0].unwrap().item(), HALF_BURNT_PORKCHOP);
        // Turned over again and left on: both sides burn.
        f.use_part(0, None, false);
        run(&mut f, 10.8);
        assert_eq!(f.grill[0].unwrap().item(), BURNT_PORKCHOP);
        let r = f.use_part(0, None, true);
        assert_eq!(r.give, vec![Stack::one(BURNT_PORKCHOP)]);
        assert!(f.grill[0].is_none());
    }

    #[test]
    fn every_way_meat_comes_off_is_its_own_item() {
        // One side left on until it burns, the other never cooked: burnt and raw.
        let mut f = lit_furnace();
        f.use_part(2, Some(Stack::one(PORKCHOP)), false);
        run(&mut f, 20.5);
        assert_eq!(f.grill[2].unwrap().item(), RAW_BURNT_PORKCHOP);
        // It goes back on as it was, and every variant does but the fully burnt one.
        for id in crate::item::meat(PORKCHOP).unwrap() {
            let g = Grilled::new(id);
            if id == BURNT_PORKCHOP {
                assert!(g.is_none());
            } else {
                assert_eq!(g.unwrap().item(), id);
            }
        }
    }

    #[test]
    fn half_burnt_meat_goes_back_on_cooked_side_down() {
        let mut f = lit_furnace();
        f.use_part(1, Some(Stack::one(HALF_BURNT_PORKCHOP)), false);
        assert_eq!(f.grill[1].unwrap().item(), HALF_BURNT_PORKCHOP);
        run(&mut f, 10.2);
        assert_eq!(f.grill[1].unwrap().item(), BURNT_PORKCHOP);
    }

    #[test]
    fn half_cooked_meat_goes_back_on_raw_side_down() {
        let mut f = lit_furnace();
        f.use_part(3, Some(Stack::one(HALF_COOKED_PORKCHOP)), false);
        run(&mut f, 10.2);
        assert_eq!(f.grill[3].unwrap().item(), COOKED_PORKCHOP);
    }

    #[test]
    fn furnace_tiers() {
        let tier = |t| Furnace {
            tier: t,
            ..Default::default()
        };
        // The furnace: copper, not iron (it does not even go in), gold or diamond.
        assert!(tier(1).accepts(part::INPUT, COPPER_ORE as ItemId));
        assert!(!tier(1).accepts(part::INPUT, IRON_ORE as ItemId));
        assert!(!tier(1).accepts(part::INPUT, SAND as ItemId));
        // The blast furnace: iron and glass, not gold or diamond.
        assert!(tier(2).accepts(part::INPUT, IRON_ORE as ItemId));
        assert!(tier(2).accepts(part::INPUT, SAND as ItemId));
        assert!(!tier(2).accepts(part::INPUT, GOLD_ORE as ItemId));
        // The advanced furnace: all of it, fastest.
        for ore in [IRON_ORE, GOLD_ORE, DIAMOND_ORE] {
            assert!(tier(3).accepts(part::INPUT, ore as ItemId));
        }
        let mut f = lit_furnace();
        f.tier = 3;
        f.use_part(part::INPUT, Some(Stack::one(DIAMOND_ORE as ItemId)), false);
        run(&mut f, 4.2);
        assert_eq!(f.output, Some(Stack::one(crate::item::DIAMOND)));
    }

    #[test]
    fn front_takes_smeltables_and_fuel_but_not_meat() {
        let mut f = Furnace {
            tier: 2,
            ..Default::default()
        };
        assert!(!f.accepts(part::INPUT, PORKCHOP));
        assert!(!f.accepts(part::FUEL, PORKCHOP));
        assert!(f.accepts(0, PORKCHOP));
        let r = f.use_part(part::INPUT, Some(Stack::new(IRON_ORE as ItemId, 3)), false);
        assert_eq!(r.used, 3);
        let r = f.use_part(part::FUEL, Some(Stack::new(COAL, 1)), false);
        assert_eq!(r.used, 1);
        run(&mut f, 10.2);
        // What is smelted stays inside; taking gets it first, then what is left.
        assert_eq!(f.output, Some(Stack::one(IRON_INGOT)));
        let r = f.use_part(part::INPUT, None, true);
        assert_eq!(r.give, vec![Stack::one(IRON_INGOT)]);
        let r = f.use_part(part::INPUT, None, true);
        assert_eq!(r.give, vec![Stack::new(IRON_ORE as ItemId, 2)]);
        // A right click with nothing to put in does nothing.
        let r = f.use_part(part::FUEL, None, false);
        assert_eq!(r, UseResult::default());
    }

    #[test]
    fn no_fuel_is_burnt_without_anything_to_heat() {
        let mut f = lit_furnace();
        run(&mut f, 5.0);
        assert_eq!(f.fuel, Some(Stack::new(COAL, 4)));
        assert!(f.burn <= 0.0);
    }
}
