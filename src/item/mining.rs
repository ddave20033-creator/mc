//! Mining: how long a block takes to break with a held item, whether it drops, what it drops
//! and how much the tool wears.

use super::*;

struct Mining {
    /// Minecraft hardness (break time with bare hands = hardness * 1.5 s when harvestable).
    hardness: f32,
    /// The tool that mines this faster.
    tool: Option<ToolKind>,
    /// Only drops when mined with `tool` of at least this harvest level.
    needs: Option<u8>,
}

fn mining(b: u8) -> Option<Mining> {
    use ToolKind::*;
    let m = |hardness, tool, needs| {
        Some(Mining {
            hardness,
            tool,
            needs,
        })
    };
    match b {
        AIR | BEDROCK => None,
        _ if is_fluid(b) => None,
        _ if is_plant(b) || is_torch(b) => m(0.0, None, None),
        _ if is_lantern(b) => m(3.5, Some(Pickaxe), Some(0)),
        _ if is_leaves(b) => m(0.2, Some(Sword), None),
        GRASS | SNOWY_GRASS | GRAVEL | CLAY => m(0.6, Some(Shovel), None),
        DIRT | SAND => m(0.5, Some(Shovel), None),
        SNOW => m(0.2, Some(Shovel), Some(0)),
        ICE => m(0.5, Some(Pickaxe), None),
        GLASS | GLOWSTONE => m(0.3, None, None),
        CACTUS => m(0.4, None, None),
        _ if is_log(b) || b == PLANKS || is_stairs(b) => m(2.0, Some(Axe), None),
        _ if is_door(b) => m(3.0, Some(Axe), None),
        _ if is_bed(b) => m(0.2, None, None),
        WOOL => m(0.8, None, None),
        CRAFTING_TABLE => m(2.5, Some(Axe), None),
        _ if is_chest(b) => m(2.5, Some(Axe), None),
        STONE | STONE_BRICKS => m(1.5, Some(Pickaxe), Some(0)),
        COBBLE | BRICKS => m(2.0, Some(Pickaxe), Some(0)),
        SANDSTONE => m(0.8, Some(Pickaxe), Some(0)),
        _ if furnace_base(b).is_some() => m(3.5, Some(Pickaxe), Some(0)),
        // Harvest levels (`Tier::level`): 0 wood, 1 stone, 2 copper, 3 iron, 4 diamond.
        COAL_ORE => m(3.0, Some(Pickaxe), Some(0)),
        COPPER_ORE => m(3.0, Some(Pickaxe), Some(1)),
        IRON_ORE => m(3.0, Some(Pickaxe), Some(2)),
        GOLD_ORE | DIAMOND_ORE => m(3.0, Some(Pickaxe), Some(3)),
        COAL_BLOCK => m(5.0, Some(Pickaxe), Some(0)),
        COPPER_BLOCK => m(5.0, Some(Pickaxe), Some(1)),
        IRON_BLOCK => m(5.0, Some(Pickaxe), Some(2)),
        GUN_STATION => m(3.5, Some(Pickaxe), Some(0)),
        _ if is_gun_bench(b) => m(3.5, Some(Pickaxe), Some(0)),
        GOLD_BLOCK => m(3.0, Some(Pickaxe), Some(3)),
        DIAMOND_BLOCK => m(5.0, Some(Pickaxe), Some(3)),
        OBSIDIAN => m(50.0, Some(Pickaxe), Some(4)),
        _ => m(1.0, None, None),
    }
}

/// Can `held` mine `b` so that it drops?
pub fn can_harvest(b: u8, held: ItemId) -> bool {
    let Some(m) = mining(b) else { return false };
    match m.needs {
        None => true,
        Some(level) => match tool_of(held) {
            Some((kind, tier)) => Some(kind) == m.tool && tier.level() >= level,
            None => false,
        },
    }
}

/// Seconds to break `b` holding `held` (None = unbreakable).
pub fn break_time(b: u8, held: ItemId) -> Option<f32> {
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
pub fn hardness(b: u8) -> Option<f32> {
    mining(b).map(|m| m.hardness)
}

/// Items dropped when `b` is mined with `held` (survival). `r` is a random number in 0..1.
pub fn drops(b: u8, held: ItemId, r: f32) -> Vec<Stack> {
    if !can_harvest(b, held) {
        return Vec::new();
    }
    let one = |id: ItemId| vec![Stack::one(id)];
    // Sheared leaves and plants drop themselves, like in Minecraft.
    if held == SHEARS && (is_leaves(b) || matches!(b, TALL_GRASS | DEAD_BUSH)) {
        return one(b as ItemId);
    }
    match b {
        // A branch is wood of its tree: it gives back the log it was placed from.
        _ if is_branch(b) => one(log_base(b) as ItemId),
        GRASS | SNOWY_GRASS => one(DIRT as ItemId),
        STONE => one(COBBLE as ItemId),
        COAL_ORE => one(COAL),
        CLAY => vec![Stack::new(CLAY_BALL, 4)],
        GLASS | ICE | TALL_GRASS => Vec::new(),
        DEAD_BUSH => {
            let n = (r * 3.0) as u8;
            if n > 0 {
                vec![Stack::new(STICK, n)]
            } else {
                Vec::new()
            }
        }
        OAK_LEAVES | BIRCH_LEAVES | SPRUCE_LEAVES => {
            if r < 0.05 {
                let sapling = match b {
                    BIRCH_LEAVES => BIRCH_SAPLING,
                    SPRUCE_LEAVES => SPRUCE_SAPLING,
                    _ => OAK_SAPLING,
                };
                one(sapling as ItemId)
            } else if b == OAK_LEAVES && r > 0.98 {
                one(STICK)
            } else {
                Vec::new()
            }
        }
        _ => item_of_block(b)
            .filter(|&i| block_of(i).is_some())
            .map(one)
            .unwrap_or_default(),
    }
}

/// Durability cost of breaking a block with a tool (swords wear twice as fast).
pub fn wear(held: ItemId, b: u8) -> u16 {
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
        let ok = |b: u8, t| can_harvest(b, pick(t));
        assert!(ok(COAL_ORE, Tier::Wood) && !ok(COPPER_ORE, Tier::Wood));
        assert!(ok(COPPER_ORE, Tier::Stone) && !ok(IRON_ORE, Tier::Stone));
        assert!(ok(IRON_ORE, Tier::Copper));
        for ore in [GOLD_ORE, DIAMOND_ORE] {
            assert!(!ok(ore, Tier::Copper));
            assert!(ok(ore, Tier::Iron) && ok(ore, Tier::Diamond));
        }
        assert!(!ok(OBSIDIAN, Tier::Iron) && ok(OBSIDIAN, Tier::Diamond));
        // The copper tools have their own ids: what comes after the first 20 tools is not a
        // tool.
        assert_eq!(tool_of(BULLET), None);
        for t in TIER_ORDER {
            assert_eq!(tool_of(pick(t)), Some((ToolKind::Pickaxe, t)));
        }
    }
}
