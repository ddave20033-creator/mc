//! Mining: how long a block takes to break with a held item, whether it drops, what it drops
//! and how much the tool wears.

use super::*;

/// How a block is mined (its line in the blocks' table).
fn mining(b: Block) -> Option<Mine> {
    crate::content::blocks::def(b).mine
}

/// Can `held` mine `b` so that it drops?
pub fn can_harvest(b: Block, held: ItemId) -> bool {
    let Some(m) = mining(b).filter(|&m| breaks(m, held)) else { return false };
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

/// Whether holding `held` breaks `b` at all (a log: only an axe).
fn breaks(m: Mine, held: ItemId) -> bool {
    !m.tool_only || tool_of(held).is_some_and(|(kind, _)| Some(kind) == m.tool)
}

/// Whether `b` is broken only with a tool that `held` is not (a log by hand).
pub fn wrong_tool(b: Block, held: ItemId) -> bool {
    mining(b).is_some_and(|m| !breaks(m, held))
}

/// Seconds to break `b` holding `held` (None = unbreakable, or not with this).
pub fn break_time(b: Block, held: ItemId) -> Option<f32> {
    let m = mining(b).filter(|&m| breaks(m, held))?;
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

    /// A tree's wood breaks only with an axe; its branches snap off by hand too, into sticks.
    #[test]
    fn logs_need_an_axe_branches_give_sticks() {
        let axe = tool_id(ToolKind::Axe, Tier::Wood);
        let pick = tool_id(ToolKind::Pickaxe, Tier::Diamond);
        for log in [OAK_LOG, BIRCH_LOG_X, SPRUCE_LOG_Z] {
            assert_eq!(break_time(log, NONE), None);
            assert_eq!(break_time(log, pick), None);
            assert!(wrong_tool(log, NONE) && drops(log, NONE, 0.5).is_empty());
            assert!(break_time(log, axe).is_some() && !wrong_tool(log, axe));
            assert_eq!(drops(log, axe, 0.5), vec![Stack::one(item_of_block(log).unwrap())]);
        }
        for branch in [OAK_BRANCH, BIRCH_BRANCH_X, SPRUCE_BRANCH_Z] {
            assert!(break_time(branch, NONE).is_some_and(|t| t < 1.0) && !wrong_tool(branch, NONE));
            for r in [0.0, 0.99] {
                let d = drops(branch, NONE, r);
                assert!(d.len() == 1 && d[0].item == STICK && (1..=2).contains(&d[0].count), "{r}");
            }
        }
    }
}
