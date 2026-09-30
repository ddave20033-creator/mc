//! Hunger (Minecraft's food level, saturation and exhaustion), thirst, and the effects from
//! drinking dirty water (poison and nausea).
//!
//! Thirst works like hunger: every action that tires you (sprinting, jumping, fighting,
//! healing) also dries you out a little, and it slowly goes down with time on its own.

pub const MAX_FOOD: f32 = 20.0;
pub const MAX_THIRST: f32 = 20.0;
/// Seconds it takes to eat or drink something (Minecraft: 32 ticks).
pub const USE_TIME: f32 = 1.6;
/// Thirst points lost per second just by being alive: 1 per 2 minutes, so a full bar lasts
/// 40 minutes (two game days) while resting. Food does not drain at rest at all
/// (Minecraft), so water always runs out first.
const THIRST_DRAIN: f32 = 1.0 / 120.0;
/// Part of the exhaustion from moving and fighting that also counts toward thirst. With the
/// time drain, mixed play empties the thirst bar in about 20-25 minutes, a full food bar
/// (with saturation) lasts about 30-35.
const THIRST_FROM_EXHAUSTION: f32 = 0.5;

/// Exhaustion costs (Minecraft's values).
pub mod cost {
    /// Per block sprinted.
    pub const SPRINT: f32 = 0.1;
    /// Per block swum.
    pub const SWIM: f32 = 0.01;
    pub const JUMP: f32 = 0.05;
    pub const SPRINT_JUMP: f32 = 0.2;
    pub const ATTACK: f32 = 0.1;
    pub const HURT: f32 = 0.1;
    pub const MINE: f32 = 0.005;
}

pub struct Needs {
    pub food: f32,
    pub saturation: f32,
    pub exhaustion: f32,
    pub thirst: f32,
    /// Like exhaustion, for thirst: every 4 points take one point of thirst.
    pub dehydration: f32,
    /// Seconds of poison left (1 damage every 1.25 s, never below half a heart).
    pub poison: f32,
    /// Seconds of nausea left (the view sways).
    pub nausea: f32,
    /// Full length of the current poison and nausea, for their progress bars.
    pub poison_total: f32,
    pub nausea_total: f32,
    regen_timer: f32,
    starve_timer: f32,
    thirst_timer: f32,
    poison_timer: f32,
}

/// What `Needs::update` asks the game to do to the player's health.
#[derive(Default)]
pub struct NeedsEffect {
    pub heal: f32,
    /// (amount, death message key, may kill)
    pub damage: Vec<(f32, &'static str, bool)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EffectKind {
    Poison,
    Nausea,
}

impl EffectKind {
    /// Translation key of the name.
    pub fn key(self) -> &'static str {
        match self {
            EffectKind::Poison => "effect.poison",
            EffectKind::Nausea => "effect.nausea",
        }
    }
}

pub struct Effect {
    pub kind: EffectKind,
    /// Seconds left and the full length.
    pub left: f32,
    pub total: f32,
}

/// What eating or drinking something can do to you: with this chance, poison and nausea
/// for so many seconds (0: none).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sickness {
    pub chance: f32,
    pub poison: f32,
    pub nausea: f32,
}

/// Food or drink.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Consumable {
    pub food: f32,
    pub saturation: f32,
    pub thirst: f32,
    /// Unboiled water, meat raw or burnt: it can make you sick.
    pub sick: Option<Sickness>,
    /// Drunk (bottle comes back empty) rather than eaten.
    pub drink: bool,
}

impl Needs {
    pub fn new() -> Self {
        Self {
            food: MAX_FOOD,
            saturation: 5.0,
            exhaustion: 0.0,
            thirst: MAX_THIRST,
            dehydration: 0.0,
            poison: 0.0,
            nausea: 0.0,
            poison_total: 0.0,
            nausea_total: 0.0,
            regen_timer: 0.0,
            starve_timer: 0.0,
            thirst_timer: 0.0,
            poison_timer: 0.0,
        }
    }

    pub fn exhaust(&mut self, amount: f32) {
        self.exhaustion += amount;
        self.dehydration += amount * THIRST_FROM_EXHAUSTION;
    }

    /// Sprinting needs more than 3 drumsticks and 3 drops.
    pub fn can_sprint(&self) -> bool {
        self.food > 6.0 && self.thirst > 6.0
    }

    pub fn wants(&self, c: &Consumable) -> bool {
        (c.food > 0.0 && self.food < MAX_FOOD) || (c.thirst > 0.0 && self.thirst < MAX_THIRST)
    }

    pub fn consume(&mut self, c: &Consumable) {
        self.food = (self.food + c.food).min(MAX_FOOD);
        self.saturation = (self.saturation + c.saturation).min(self.food);
        self.thirst = (self.thirst + c.thirst).min(MAX_THIRST);
    }

    /// Advances hunger, thirst and effects; `health` is the player's current health.
    pub fn update(&mut self, dt: f32, health: f32, max_health: f32) -> NeedsEffect {
        let mut fx = NeedsEffect::default();
        self.dehydration += dt * THIRST_DRAIN * 4.0;
        while self.exhaustion >= 4.0 {
            self.exhaustion -= 4.0;
            if self.saturation > 0.0 {
                self.saturation = (self.saturation - 1.0).max(0.0);
            } else {
                self.food = (self.food - 1.0).max(0.0);
            }
        }
        while self.dehydration >= 4.0 {
            self.dehydration -= 4.0;
            self.thirst = (self.thirst - 1.0).max(0.0);
        }

        // Natural regeneration (Minecraft 1.11+): fast while full and saturated, slow while
        // at least 9 drumsticks. Healing tires you out. Too thirsty: no healing at all.
        let can_heal = health > 0.0 && health < max_health && self.thirst >= 6.0;
        if can_heal && self.food >= MAX_FOOD && self.saturation > 0.0 {
            self.regen_timer += dt;
            if self.regen_timer >= 0.5 {
                self.regen_timer = 0.0;
                fx.heal += 1.0;
                // Healing costs food only.
                self.exhaustion += self.saturation.min(6.0);
            }
        } else if can_heal && self.food >= 18.0 {
            self.regen_timer += dt;
            if self.regen_timer >= 4.0 {
                self.regen_timer = 0.0;
                fx.heal += 1.0;
                self.exhaustion += 6.0;
            }
        } else {
            self.regen_timer = 0.0;
        }

        // Starving hurts down to half a heart (Minecraft on Normal); dying of thirst can kill.
        if self.food <= 0.0 {
            self.starve_timer += dt;
            if self.starve_timer >= 4.0 {
                self.starve_timer = 0.0;
                if health > 1.0 {
                    fx.damage.push((1.0, "death.starve", false));
                }
            }
        } else {
            self.starve_timer = 0.0;
        }
        if self.thirst <= 0.0 {
            self.thirst_timer += dt;
            if self.thirst_timer >= 3.0 {
                self.thirst_timer = 0.0;
                fx.damage.push((1.0, "death.thirst", true));
            }
        } else {
            self.thirst_timer = 0.0;
        }

        if self.poison > 0.0 {
            self.poison -= dt;
            self.poison_timer += dt;
            if self.poison_timer >= 1.25 {
                self.poison_timer = 0.0;
                if health > 1.0 {
                    fx.damage.push((1.0, "death.poison", false));
                }
            }
        } else {
            self.poison = 0.0;
            self.poison_timer = 0.0;
        }
        self.nausea = (self.nausea - dt).max(0.0);
        fx
    }

    /// Starts (or lengthens) an effect for `seconds`.
    pub fn add_effect(&mut self, kind: EffectKind, seconds: f32) {
        let (left, total) = match kind {
            EffectKind::Poison => (&mut self.poison, &mut self.poison_total),
            EffectKind::Nausea => (&mut self.nausea, &mut self.nausea_total),
        };
        if seconds > *left {
            *left = seconds;
            *total = seconds;
        }
    }

    /// Active effects, for the HUD and the inventory.
    pub fn effects(&self) -> Vec<Effect> {
        [
            (EffectKind::Poison, self.poison, self.poison_total),
            (EffectKind::Nausea, self.nausea, self.nausea_total),
        ]
        .into_iter()
        .filter(|e| e.1 > 0.0)
        .map(|(kind, left, total)| Effect {
            kind,
            left,
            total: total.max(left),
        })
        .collect()
    }

    /// Values for the save file.
    pub fn to_array(&self) -> [f32; 7] {
        [
            self.food,
            self.saturation,
            self.exhaustion,
            self.thirst,
            self.dehydration,
            self.poison,
            self.nausea,
        ]
    }

    pub fn from_array(a: [f32; 7]) -> Self {
        Self {
            food: a[0].clamp(0.0, MAX_FOOD),
            saturation: a[1].clamp(0.0, MAX_FOOD),
            exhaustion: a[2].max(0.0),
            thirst: a[3].clamp(0.0, MAX_THIRST),
            dehydration: a[4].max(0.0),
            poison: a[5].max(0.0),
            nausea: a[6].max(0.0),
            poison_total: a[5].max(0.0),
            nausea_total: a[6].max(0.0),
            ..Self::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhaustion_uses_saturation_first() {
        let mut n = Needs::new();
        n.exhaust(4.0 * 5.0);
        n.update(0.0, 20.0, 20.0);
        assert_eq!(n.saturation, 0.0);
        assert_eq!(n.food, MAX_FOOD);
        n.exhaust(4.0);
        n.update(0.0, 20.0, 20.0);
        assert_eq!(n.food, MAX_FOOD - 1.0);
        // Thirst went down too, but less.
        assert!(n.thirst < MAX_THIRST && n.thirst > MAX_THIRST - 6.0);
    }

    #[test]
    fn heals_when_fed_but_not_when_thirsty() {
        let mut n = Needs::new();
        let fx = n.update(0.6, 10.0, 20.0);
        assert_eq!(fx.heal, 1.0);
        n.thirst = 2.0;
        n.saturation = 5.0;
        let fx = n.update(0.6, 10.0, 20.0);
        assert_eq!(fx.heal, 0.0);
    }

    #[test]
    fn starving_never_kills_but_thirst_does() {
        let mut n = Needs::new();
        n.food = 0.0;
        n.saturation = 0.0;
        let fx = n.update(4.1, 1.0, 20.0);
        assert!(fx.damage.is_empty());
        n.thirst = 0.0;
        let fx = n.update(4.1, 1.0, 20.0);
        assert!(fx.damage.iter().any(|d| d.1 == "death.thirst" && d.2));
    }
}
