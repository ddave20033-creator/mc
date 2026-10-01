//! Mining: how long a block takes to break with a held item, whether it drops, what it drops
//! and how much the tool wears.

use super::*;

/// How a block is mined (its line in the blocks' table).
fn mining(b: Block) -> Option<Mine> {
    crate::content::blocks::def(b).mine
}

/// Can `held` mine `b` so that it drops?
pub fn can_harvest(b: Block, held: ItemId) -> bool {
    let Some(m) = mining(b) else { return false };
    match m.needs {
        None => true,
        Some(level) => match tool_of(held) {
            Some((kind, tier)) => Some(kind) == m.tool && tier.level() >= level,
            None => false,
        },
    }
}

/// The hardest a pickaxe of this harvest level (`Tier::level`) mines: the ores that need just
/// that level (`ORES`, the softest first); for a level no ore needs, the blocks that do.
pub fn hardest_mined(level: u8) -> Vec<Block> {
    let needs = |b: Block| mining(b).is_some_and(|m| m.tool == Some(ToolKind::Pickaxe) && m.needs == Some(level));
    let ores: Vec<Block> = ORES.into_iter().filter(|&b| needs(b)).collect();
    if !ores.is_empty() {
        return ores;
    }
    crate::content::blocks::BLOCKS.iter().map(|d| d.id).filter(|&b| needs(b)).collect()
}

/// Seconds to break `b` holding `held` (None = unbreakable).
pub fn break_time(b: Block, held: ItemId) -> Option<f32> {
    let m = mining(b)?;
    let mut speed = 1.0;
    if let Some((kind, tier)) = tool_of(held) {
        if Some(kind) == m.tool {
            speed = if kind == ToolKind::Sword {
                1.5
            } else {
                tier.speed()
            };
        }
    }
    if held == SHEARS {
        // Minecraft's shears: fast on leaves and wool.
        speed = match b {
            _ if is_leaves(b) => 15.0,
            WOOL => 5.0,
            _ => speed,
        };
    }
    let mult = if can_harvest(b, held) { 1.5 } else { 5.0 };
    Some(m.hardness * mult / speed)
}

/// How hard a block is (Minecraft hardness); None for what cannot be mined.
pub fn hardness(b: Block) -> Option<f32> {
    mining(b).map(|m| m.hardness)
}

/// Items dropped when `b` is mined with `held` (survival). `r` is a random number in 0..1.
pub fn drops(b: Block, held: ItemId, r: f32) -> Vec<Stack> {
    if !can_harvest(b, held) {
        return Vec::new();
    }
    let one = |id: ItemId| vec![Stack::one(id)];
    // Sheared leaves and plants drop themselves, like in Minecraft.
    if sheared(b, held) {
        return one(base(b) as ItemId);
    }
    match crate::content::blocks::def(b).drops {
        Drops::Nothing => Vec::new(),
        Drops::Item(item, n) => vec![Stack::new(item, n)],
        Drops::Custom(f) => f(b, held, r),
        Drops::Itself => item_of_block(b)
            .filter(|&i| block_of(i).is_some())
            .map(one)
            .unwrap_or_default(),
    }
}

/// Durability cost of breaking a block with a tool (swords wear twice as fast).
pub fn wear(held: ItemId, b: Block) -> u16 {
    if held == SHEARS {
        // Shears wear on every block they break.
        return mining(b).is_some() as u16;
    }
    match tool_of(held) {
        Some((ToolKind::Sword, _)) => 2,
        Some(_) if mining(b).is_some_and(|m| m.hardness > 0.0) => 1,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tier ladder: wood mines coal, stone copper, copper iron, iron gold and diamond,
    /// diamond obsidian.
    #[test]
    fn tier_ladder() {
        let pick = |t| tool_id(ToolKind::Pickaxe, t);
        let ok = |b: Block, t| can_harvest(b, pick(t));
        assert!(ok(COAL_ORE, Tier::Wood) && !ok(COPPER_ORE, Tier::Wood));
        assert!(ok(COPPER_ORE, Tier::Stone) && !ok(IRON_ORE, Tier::Stone));
        assert!(ok(IRON_ORE, Tier::Copper));
        for ore in [GOLD_ORE, DIAMOND_ORE] {
            assert!(!ok(ore, Tier::Copper));
            assert!(ok(ore, Tier::Iron) && ok(ore, Tier::Diamond));
        }
        assert!(!ok(OBSIDIAN, Tier::Iron) && ok(OBSIDIAN, Tier::Diamond));
        // The hardest each tier mines (what the guide book shows).
        let hardest = |t: Tier| hardest_mined(t.level());
        assert_eq!(hardest(Tier::Wood), [COAL_ORE]);
        assert_eq!(hardest(Tier::Copper), [IRON_ORE]);
        assert_eq!(hardest(Tier::Gold), [GOLD_ORE, DIAMOND_ORE]);
        assert_eq!(hardest(Tier::Diamond), [OBSIDIAN]);
        // The copper tools have their own ids: what comes after the first 20 tools is not a
        // tool.
        assert_eq!(tool_of(BULLET), None);
        for t in TIER_ORDER {
            assert_eq!(tool_of(pick(t)), Some((ToolKind::Pickaxe, t)));
        }
    }
}
