//! Crafting recipes (2x2 and 3x3 grids, mirrored patterns too) and the furnace: fuels and
//! smelting results.

use super::*;

/// Seconds a fuel item burns (one item smelts in 10 s).
pub fn fuel_time(id: ItemId) -> Option<f32> {
    Some(match id {
        COAL | CHARCOAL => 80.0,
        LAVA_BUCKET => 1000.0,
        STICK => 5.0,
        _ if id == COAL_BLOCK as ItemId => 800.0,
        _ if [
            OAK_LOG,
            BIRCH_LOG,
            SPRUCE_LOG,
            PLANKS,
            CRAFTING_TABLE,
            CHEST,
            OAK_STAIRS,
        ]
        .iter()
        .any(|&b| b as ItemId == id) =>
        {
            15.0
        }
        _ if id == OAK_DOOR as ItemId => 10.0,
        _ if [OAK_SAPLING, BIRCH_SAPLING, SPRUCE_SAPLING, WOOL]
            .iter()
            .any(|&b| b as ItemId == id) =>
        {
            5.0
        }
        _ => match tool_of(id) {
            Some((_, Tier::Wood)) => 10.0,
            _ => return None,
        },
    })
}

pub fn smelt(id: ItemId) -> Option<ItemId> {
    Some(match id {
        CLAY_BALL => BRICK,
        PORKCHOP => COOKED_PORKCHOP,
        MUTTON => COOKED_MUTTON,
        WATER_BOTTLE => PURIFIED_WATER,
        _ => match id as u8 {
            _ if id >= 256 => return None,
            IRON_ORE => IRON_INGOT,
            GOLD_ORE => GOLD_INGOT,
            SAND => GLASS as ItemId,
            COBBLE => STONE as ItemId,
            OAK_LOG | BIRCH_LOG | SPRUCE_LOG => CHARCOAL,
            _ => return None,
        },
    })
}

pub struct Recipe {
    /// Pattern rows; each char is a key into `keys`, ' ' is empty.
    pattern: &'static [&'static str],
    keys: Vec<(char, Vec<ItemId>)>,
    result: Stack,
}

fn logs() -> Vec<ItemId> {
    vec![OAK_LOG as ItemId, BIRCH_LOG as ItemId, SPRUCE_LOG as ItemId]
}

fn b(id: u8) -> Vec<ItemId> {
    vec![id as ItemId]
}

fn recipes() -> &'static Vec<Recipe> {
    static R: std::sync::OnceLock<Vec<Recipe>> = std::sync::OnceLock::new();
    R.get_or_init(|| {
        let mut v = vec![
            Recipe {
                pattern: &["L"],
                keys: vec![('L', logs())],
                result: Stack::new(PLANKS as ItemId, 4),
            },
            Recipe {
                pattern: &["P", "P"],
                keys: vec![('P', b(PLANKS))],
                result: Stack::new(STICK, 4),
            },
            Recipe {
                pattern: &["PP", "PP"],
                keys: vec![('P', b(PLANKS))],
                result: Stack::one(CRAFTING_TABLE as ItemId),
            },
            Recipe {
                pattern: &["CCC", "C C", "CCC"],
                keys: vec![('C', b(COBBLE))],
                result: Stack::one(FURNACE as ItemId),
            },
            Recipe {
                pattern: &["PPP", "P P", "PPP"],
                keys: vec![('P', b(PLANKS))],
                result: Stack::one(CHEST as ItemId),
            },
            Recipe {
                pattern: &["PP", "PP", "PP"],
                keys: vec![('P', b(PLANKS))],
                result: Stack::new(OAK_DOOR as ItemId, 3),
            },
            Recipe {
                pattern: &["WWW", "PPP"],
                keys: vec![('W', b(WOOL)), ('P', b(PLANKS))],
                result: Stack::one(BED as ItemId),
            },
            Recipe {
                pattern: &[" I", "I "],
                keys: vec![('I', vec![IRON_INGOT])],
                result: Stack::one(SHEARS),
            },
            Recipe {
                pattern: &["P  ", "PP ", "PPP"],
                keys: vec![('P', b(PLANKS))],
                result: Stack::new(OAK_STAIRS as ItemId, 4),
            },
            Recipe {
                pattern: &["C", "S"],
                keys: vec![('C', vec![COAL, CHARCOAL]), ('S', vec![STICK])],
                result: Stack::new(TORCH as ItemId, 4),
            },
            Recipe {
                pattern: &["I I", " I "],
                keys: vec![('I', vec![IRON_INGOT])],
                result: Stack::one(BUCKET),
            },
            Recipe {
                pattern: &["I"],
                keys: vec![('I', vec![IRON_INGOT])],
                result: Stack::new(IRON_NUGGET, 9),
            },
            Recipe {
                pattern: &["NNN", "NNN", "NNN"],
                keys: vec![('N', vec![IRON_NUGGET])],
                result: Stack::one(IRON_INGOT),
            },
            Recipe {
                pattern: &["NNN", "NTN", "NNN"],
                keys: vec![('N', vec![IRON_NUGGET]), ('T', b(TORCH))],
                result: Stack::one(LANTERN as ItemId),
            },
            Recipe {
                pattern: &["G G", " G "],
                keys: vec![('G', b(GLASS))],
                result: Stack::new(GLASS_BOTTLE, 3),
            },
            Recipe {
                pattern: &["SS", "SS"],
                keys: vec![('S', b(SAND))],
                result: Stack::one(SANDSTONE as ItemId),
            },
            Recipe {
                pattern: &["SS", "SS"],
                keys: vec![('S', b(STONE))],
                result: Stack::new(STONE_BRICKS as ItemId, 4),
            },
            Recipe {
                pattern: &["BB", "BB"],
                keys: vec![('B', vec![BRICK])],
                result: Stack::one(BRICKS as ItemId),
            },
            Recipe {
                pattern: &["CC", "CC"],
                keys: vec![('C', vec![CLAY_BALL])],
                result: Stack::one(CLAY as ItemId),
            },
            // Guns: the station, the pistol's parts (put together at the station) and bullets.
            Recipe {
                pattern: &["III", "N N"],
                keys: vec![('I', vec![IRON_INGOT]), ('N', vec![IRON_NUGGET])],
                result: Stack::one(GUN_STATION as ItemId),
            },
            Recipe {
                pattern: &["III", "I  "],
                keys: vec![('I', vec![IRON_INGOT])],
                result: Stack::one(PISTOL_FRAME),
            },
            Recipe {
                pattern: &["III"],
                keys: vec![('I', vec![IRON_INGOT])],
                result: Stack::one(PISTOL_BARREL),
            },
            Recipe {
                pattern: &["N", "N", "N"],
                keys: vec![('N', vec![IRON_NUGGET])],
                result: Stack::one(PISTOL_SPRING),
            },
            Recipe {
                pattern: &["NNN", "NNN"],
                keys: vec![('N', vec![IRON_NUGGET])],
                result: Stack::one(PISTOL_SLIDE),
            },
            Recipe {
                pattern: &["I", "I"],
                keys: vec![('I', vec![IRON_INGOT])],
                result: Stack::one(PISTOL_MAGAZINE),
            },
            // Pistol attachments.
            Recipe {
                pattern: &["GIG"],
                keys: vec![('G', b(GLASS)), ('I', vec![IRON_INGOT])],
                result: Stack::one(SCOPE),
            },
            Recipe {
                pattern: &["I", "W", "I"],
                keys: vec![('I', vec![IRON_INGOT]), ('W', b(WOOL))],
                result: Stack::one(SILENCER),
            },
            Recipe {
                pattern: &["M", "I"],
                keys: vec![('M', vec![PISTOL_MAGAZINE]), ('I', vec![IRON_INGOT])],
                result: Stack::one(EXTENDED_MAGAZINE),
            },
            Recipe {
                pattern: &["NTG"],
                keys: vec![('N', vec![IRON_NUGGET]), ('T', b(TORCH)), ('G', b(GLASS))],
                result: Stack::one(LASER_SIGHT),
            },
            // Ammunition of the other guns.
            Recipe {
                pattern: &["N", "N", "C"],
                keys: vec![('N', vec![IRON_NUGGET]), ('C', vec![COAL, CHARCOAL])],
                result: Stack::new(RIFLE_ROUND, 6),
            },
            Recipe {
                pattern: &["I", "C"],
                keys: vec![('I', vec![IRON_INGOT]), ('C', vec![COAL, CHARCOAL])],
                result: Stack::new(MAGNUM_ROUND, 3),
            },
            Recipe {
                pattern: &["I", "I", "C"],
                keys: vec![('I', vec![IRON_INGOT]), ('C', vec![COAL, CHARCOAL])],
                result: Stack::new(BMG_ROUND, 2),
            },
            Recipe {
                pattern: &["NCN"],
                keys: vec![('N', vec![IRON_NUGGET]), ('C', vec![COAL, CHARCOAL])],
                result: Stack::new(SHOTGUN_SHELL, 4),
            },
            Recipe {
                pattern: &["N", "C"],
                keys: vec![('N', vec![IRON_NUGGET]), ('C', vec![COAL, CHARCOAL])],
                result: Stack::new(BULLET, 4),
            },
        ];
        // Storage blocks and back.
        for (block, item) in [
            (COAL_BLOCK, COAL),
            (IRON_BLOCK, IRON_INGOT),
            (GOLD_BLOCK, GOLD_INGOT),
            (DIAMOND_BLOCK, DIAMOND),
        ] {
            v.push(Recipe {
                pattern: &["XXX", "XXX", "XXX"],
                keys: vec![('X', vec![item])],
                result: Stack::one(block as ItemId),
            });
            v.push(Recipe {
                pattern: &["X"],
                keys: vec![('X', b(block))],
                result: Stack::new(item, 9),
            });
        }
        // Tools.
        for tier in TIERS {
            let m = tier.material();
            let keys = || vec![('M', vec![m]), ('S', vec![STICK])];
            v.push(Recipe {
                pattern: &["MMM", " S ", " S "],
                keys: keys(),
                result: Stack::one(tool_id(ToolKind::Pickaxe, tier)),
            });
            v.push(Recipe {
                pattern: &["MM", "MS", " S"],
                keys: keys(),
                result: Stack::one(tool_id(ToolKind::Axe, tier)),
            });
            v.push(Recipe {
                pattern: &["M", "S", "S"],
                keys: keys(),
                result: Stack::one(tool_id(ToolKind::Shovel, tier)),
            });
            v.push(Recipe {
                pattern: &["M", "M", "S"],
                keys: keys(),
                result: Stack::one(tool_id(ToolKind::Sword, tier)),
            });
        }
        v
    })
}

/// Result of the items in a crafting grid (`size` x `size`, row-major).
pub fn craft(grid: &[Slot], size: usize) -> Option<Stack> {
    // Bounding box of the non-empty cells.
    let filled: Vec<(usize, usize)> = (0..size * size)
        .filter(|&i| grid[i].is_some())
        .map(|i| (i % size, i / size))
        .collect();
    if filled.is_empty() {
        return None;
    }
    let (x0, x1) = (
        filled.iter().map(|c| c.0).min()?,
        filled.iter().map(|c| c.0).max()?,
    );
    let (y0, y1) = (
        filled.iter().map(|c| c.1).min()?,
        filled.iter().map(|c| c.1).max()?,
    );
    let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
    let cell = |x: usize, y: usize| grid[(y0 + y) * size + x0 + x].map(|s| s.item);
    'recipes: for r in recipes() {
        let rh = r.pattern.len();
        let rw = r.pattern[0].len();
        if rw != w || rh != h {
            continue;
        }
        // Try the pattern as written and mirrored horizontally.
        for mirror in [false, true] {
            let ok = (0..h).all(|y| {
                (0..w).all(|x| {
                    let px = if mirror { w - 1 - x } else { x };
                    let ch = r.pattern[y].as_bytes()[px] as char;
                    match (ch, cell(x, y)) {
                        (' ', None) => true,
                        (' ', Some(_)) | (_, None) => false,
                        (c, Some(item)) => r
                            .keys
                            .iter()
                            .any(|(k, items)| *k == c && items.contains(&item)),
                    }
                })
            });
            if ok {
                return Some(r.result);
            }
        }
        continue 'recipes;
    }
    None
}
