//! Crafting recipes (2x2 and 3x3 grids, mirrored patterns too), and what the furnaces make of
//! what (each item's `fuel` and `smelt` are its line's in `content`).

use super::*;

pub struct Recipe {
    /// Pattern rows; each char is a key into `keys`, ' ' is empty.
    pattern: &'static [&'static str],
    keys: Vec<(char, Vec<ItemId>)>,
    result: Stack,
}

fn logs() -> Vec<ItemId> {
    vec![OAK_LOG as ItemId, BIRCH_LOG as ItemId, SPRUCE_LOG as ItemId]
}

fn b(id: Block) -> Vec<ItemId> {
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
            // The blast furnace takes copper, the advanced furnace iron.
            Recipe {
                pattern: &["CCC", "CFC", "BBB"],
                keys: vec![
                    ('C', vec![COPPER_INGOT]),
                    ('F', b(FURNACE)),
                    ('B', b(BRICKS)),
                ],
                result: Stack::one(BLAST_FURNACE as ItemId),
            },
            Recipe {
                pattern: &["III", "IFI", "SSS"],
                keys: vec![
                    ('I', vec![IRON_INGOT]),
                    ('F', b(BLAST_FURNACE)),
                    ('S', b(STONE_BRICKS)),
                ],
                result: Stack::one(ADV_FURNACE as ItemId),
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
            // Like Minecraft's: three sticks on the diagonal, the line (wool spun thin) hanging
            // down from the tip.
            Recipe {
                pattern: &["  S", " SW", "S W"],
                keys: vec![('S', vec![STICK]), ('W', b(WOOL))],
                result: Stack::one(FISHING_ROD),
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
            // The guide book: wool bound on a plank.
            Recipe {
                pattern: &["W", "P"],
                keys: vec![('W', b(WOOL)), ('P', b(PLANKS))],
                result: Stack::one(GUIDE_BOOK),
            },
            // Guns: the station, the pistol's parts (put together at the station) and bullets.
            Recipe {
                pattern: &["III", "N N"],
                keys: vec![('I', vec![IRON_INGOT]), ('N', vec![IRON_NUGGET])],
                result: Stack::one(GUN_STATION as ItemId),
            },
            // The rifle station: a gun station made longer, with steel and a wooden top.
            Recipe {
                pattern: &["PPP", "SGS"],
                keys: vec![('P', b(PLANKS)), ('S', vec![STEEL_INGOT]), ('G', vec![GUN_STATION as ItemId])],
                result: Stack::one(RIFLE_BENCH as ItemId),
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
            // The revolver's parts (put together at the station): a steel frame with a wooden
            // grip, a steel barrel, a mainspring of nuggets, a steel cylinder and an iron
            // hammer; and the speedloader.
            Recipe {
                pattern: &["SSS", "S P"],
                keys: vec![('S', vec![STEEL_INGOT]), ('P', b(PLANKS))],
                result: Stack::one(REVOLVER_FRAME),
            },
            Recipe {
                pattern: &["SS"],
                keys: vec![('S', vec![STEEL_INGOT])],
                result: Stack::one(REVOLVER_BARREL),
            },
            Recipe {
                pattern: &["N  ", " N ", "  N"],
                keys: vec![('N', vec![IRON_NUGGET])],
                result: Stack::one(REVOLVER_SPRING),
            },
            Recipe {
                pattern: &[" S ", "S S", " S "],
                keys: vec![('S', vec![STEEL_INGOT])],
                result: Stack::one(REVOLVER_CYLINDER),
            },
            Recipe {
                pattern: &["IN"],
                keys: vec![('I', vec![IRON_INGOT]), ('N', vec![IRON_NUGGET])],
                result: Stack::one(REVOLVER_HAMMER),
            },
            // Magnum rounds: a copper jacket, an iron core and the powder.
            Recipe {
                pattern: &["O", "N", "C"],
                keys: vec![('O', vec![COPPER_INGOT]), ('N', vec![IRON_NUGGET]), ('C', vec![COAL, CHARCOAL])],
                result: Stack::new(MAGNUM_ROUND, 4),
            },
            Recipe {
                pattern: &["N N", " I "],
                keys: vec![('N', vec![IRON_NUGGET]), ('I', vec![IRON_INGOT])],
                result: Stack::one(SPEEDLOADER),
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
            Recipe {
                pattern: &["N", "C"],
                keys: vec![('N', vec![IRON_NUGGET]), ('C', vec![COAL, CHARCOAL])],
                result: Stack::new(BULLET, 4),
            },
            // The AK-47's parts: steel (from the blast furnace) and wood.
            Recipe {
                pattern: &["SSS", "PSP"],
                keys: vec![('S', vec![STEEL_INGOT]), ('P', b(PLANKS))],
                result: Stack::one(AK_RECEIVER),
            },
            Recipe {
                pattern: &["PSS"],
                keys: vec![('S', vec![STEEL_INGOT]), ('P', b(PLANKS))],
                result: Stack::one(AK_GAS_TUBE),
            },
            Recipe {
                pattern: &["SN", "SS"],
                keys: vec![('S', vec![STEEL_INGOT]), ('N', vec![IRON_NUGGET])],
                result: Stack::one(AK_BOLT),
            },
            Recipe {
                pattern: &["SS", "NN"],
                keys: vec![('S', vec![STEEL_INGOT]), ('N', vec![IRON_NUGGET])],
                result: Stack::one(AK_COVER),
            },
            Recipe {
                pattern: &["S", "N", "S"],
                keys: vec![('S', vec![STEEL_INGOT]), ('N', vec![IRON_NUGGET])],
                result: Stack::one(AK_MAGAZINE),
            },
            // The magazine loader: steel, a crank of iron, a spring.
            Recipe {
                pattern: &["NIN", "SSS"],
                keys: vec![('N', vec![IRON_NUGGET]), ('I', vec![IRON_INGOT]), ('S', vec![STEEL_INGOT])],
                result: Stack::one(MAG_LOADER),
            },
            // Rifle rounds: a copper jacket, a steel core and more powder than the pistol's.
            Recipe {
                pattern: &["O", "S", "C"],
                keys: vec![('O', vec![COPPER_INGOT]), ('S', vec![STEEL_INGOT]), ('C', vec![COAL, CHARCOAL])],
                result: Stack::new(RIFLE_ROUND, 6),
            },
            // Grenades: a steel body (from the blast furnace), the filling and the fuse.
            Recipe {
                pattern: &[" N ", "SCS", " S "],
                keys: vec![
                    ('N', vec![IRON_NUGGET]),
                    ('S', vec![STEEL_INGOT]),
                    ('C', vec![COAL, CHARCOAL]),
                ],
                result: Stack::new(FRAG_GRENADE, 2),
            },
            Recipe {
                pattern: &[" N ", "CSC", " C "],
                keys: vec![
                    ('N', vec![IRON_NUGGET]),
                    ('S', vec![STEEL_INGOT]),
                    ('C', vec![COAL, CHARCOAL]),
                ],
                result: Stack::new(SMOKE_GRENADE, 2),
            },
            // The target dummy: a wool sack on a post with a crossbar, on plank feet.
            Recipe {
                pattern: &[" W ", "SWS", "PSP"],
                keys: vec![('W', b(WOOL)), ('S', vec![STICK]), ('P', b(PLANKS))],
                result: Stack::one(TARGET_DUMMY),
            },
        ];
        // Storage blocks and back.
        for (block, item) in [
            (COAL_BLOCK, COAL),
            (COPPER_BLOCK, COPPER_INGOT),
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
        // Armor (steel from the blast furnace), and the bulletproof vest: steel, ceramic plates
        // from the advanced furnace, and wool.
        for m in 0..MATERIALS {
            let keys = || vec![('X', vec![material_item(m)])];
            for (piece, pattern) in [
                &["XXX", "X X"][..],
                &["X X", "XXX", "XXX"][..],
                &["XXX", "X X", "X X"][..],
                &["X X", "X X"][..],
            ]
            .into_iter()
            .enumerate()
            {
                v.push(Recipe {
                    pattern,
                    keys: keys(),
                    result: Stack::one(armor_id(m, piece)),
                });
            }
        }
        v.push(Recipe {
            pattern: &["S S", "PWP", "PSP"],
            keys: vec![
                ('S', vec![STEEL_INGOT]),
                ('P', vec![CERAMIC_PLATE]),
                ('W', b(WOOL)),
            ],
            result: Stack::one(BULLETPROOF_VEST),
        });
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

/// How `result` is crafted (its first recipe), for the guide book: the pattern's rows, each
/// cell with the items that can go there (none: empty), and what comes out.
pub fn recipe_view(result: ItemId) -> Option<(Vec<Vec<Vec<ItemId>>>, Stack)> {
    let r = recipes().iter().find(|r| r.result.item == result)?;
    let rows = r
        .pattern
        .iter()
        .map(|row| {
            row.chars()
                .map(|ch| {
                    r.keys
                        .iter()
                        .find(|(k, _)| *k == ch)
                        .map(|(_, items)| items.clone())
                        .unwrap_or_default()
                })
                .collect()
        })
        .collect();
    Some((rows, r.result))
}

/// Everything that can be crafted, once each, in the order of the recipes.
pub fn recipe_results() -> Vec<ItemId> {
    let mut out: Vec<ItemId> = Vec::new();
    for r in recipes() {
        if !out.contains(&r.result.item) {
            out.push(r.result.item);
        }
    }
    out
}

/// The ways `item` is crafted: each recipe's 3x3 grid (the items a cell takes, empty for
/// none) and how many it makes.
pub fn recipes_for(item: ItemId) -> Vec<([Vec<ItemId>; 9], u8)> {
    recipes()
        .iter()
        .filter(|r| r.result.item == item)
        .map(|r| {
            let mut grid: [Vec<ItemId>; 9] = Default::default();
            for (y, row) in r.pattern.iter().enumerate() {
                for (x, ch) in row.chars().enumerate() {
                    if let Some((_, ids)) = r.keys.iter().find(|k| k.0 == ch) {
                        grid[y * 3 + x] = ids.clone();
                    }
                }
            }
            (grid, r.result.count)
        })
        .collect()
}

/// What smelts into `item`, and the furnace it needs (`smelt_tier`).
pub fn smelted_from(item: ItemId) -> Vec<(ItemId, u8)> {
    all_items()
        .into_iter()
        .filter(|&i| smelt(i) == Some(item))
        .map(|i| (i, smelt_tier(i)))
        .collect()
}

/// The filled part of a crafting grid: the bounding box of its non-empty cells, and their
/// items row by row (`w` to a row).
#[derive(Clone, Copy, PartialEq, Eq)]
struct Filled {
    w: usize,
    h: usize,
    cells: [Option<ItemId>; 9],
}

impl Filled {
    fn of(grid: &[Slot], size: usize) -> Option<Filled> {
        let (mut x0, mut x1, mut y0, mut y1) = (usize::MAX, 0, usize::MAX, 0);
        for i in (0..size * size).filter(|&i| grid[i].is_some()) {
            let (x, y) = (i % size, i / size);
            (x0, x1, y0, y1) = (x0.min(x), x1.max(x), y0.min(y), y1.max(y));
        }
        if x0 == usize::MAX {
            return None;
        }
        let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
        // (no recipe is bigger than 3x3)
        if w > 3 || h > 3 {
            return Some(Filled { w, h, cells: [None; 9] });
        }
        let mut cells = [None; 9];
        for y in 0..h {
            for x in 0..w {
                cells[y * w + x] = grid[(y0 + y) * size + x0 + x].map(|s| s.item);
            }
        }
        Some(Filled { w, h, cells })
    }

    /// Whether `r` makes this, as written or mirrored horizontally.
    fn matches(&self, r: &Recipe) -> bool {
        let (w, h) = (self.w, self.h);
        [false, true].into_iter().any(|mirror| {
            (0..h).all(|y| {
                (0..w).all(|x| {
                    let px = if mirror { w - 1 - x } else { x };
                    let ch = r.pattern[y].as_bytes()[px] as char;
                    match (ch, self.cells[y * w + x]) {
                        (' ', None) => true,
                        (' ', Some(_)) | (_, None) => false,
                        (c, Some(item)) => r.keys.iter().any(|(k, items)| *k == c && items.contains(&item)),
                    }
                })
            })
        })
    }
}

/// The recipes by their size: `[h - 1][w - 1]`, each list in the order of `recipes`.
fn recipes_by_size() -> &'static [[Vec<&'static Recipe>; 3]; 3] {
    static R: std::sync::OnceLock<[[Vec<&'static Recipe>; 3]; 3]> = std::sync::OnceLock::new();
    R.get_or_init(|| {
        let mut by: [[Vec<&'static Recipe>; 3]; 3] = Default::default();
        for r in recipes() {
            by[r.pattern.len() - 1][r.pattern[0].len() - 1].push(r);
        }
        by
    })
}

/// Result of the items in a crafting grid (`size` x `size`, row-major): only the items count,
/// where they are against each other (anywhere in the grid), a recipe's pattern as written or
/// mirrored. The grid shown is asked about every frame: the last one asked about is kept with
/// what it makes.
pub fn craft(grid: &[Slot], size: usize) -> Option<Stack> {
    thread_local! {
        static LAST: std::cell::Cell<Option<(Filled, Option<Stack>)>> = const { std::cell::Cell::new(None) };
    }
    let f = Filled::of(grid, size)?;
    if let Some((last, out)) = LAST.get() {
        if last == f {
            return out;
        }
    }
    let out = if f.w > 3 || f.h > 3 {
        None
    } else {
        recipes_by_size()[f.h - 1][f.w - 1].iter().find(|r| f.matches(r)).map(|r| r.result)
    };
    LAST.set(Some((f, out)));
    out
}

/// A recipe as the craft menu lists it: what it makes, what it takes (each ingredient: the
/// items that will do, and how many) and whether it needs a crafting table (bigger than 2x2).
pub struct ListedRecipe {
    pub result: Stack,
    pub needs: Vec<(Vec<ItemId>, u8)>,
    pub table: bool,
}

/// Every recipe for the craft menu, in the order of `recipes`.
pub fn craft_list() -> &'static [ListedRecipe] {
    static L: std::sync::OnceLock<Vec<ListedRecipe>> = std::sync::OnceLock::new();
    L.get_or_init(|| {
        recipes()
            .iter()
            .map(|r| {
                let needs = r
                    .keys
                    .iter()
                    .map(|(k, items)| {
                        let n = r.pattern.iter().map(|row| row.chars().filter(|c| c == k).count()).sum::<usize>();
                        (items.clone(), n as u8)
                    })
                    .filter(|(_, n)| *n > 0)
                    .collect();
                ListedRecipe { result: r.result, needs, table: r.pattern.len() > 2 || r.pattern[0].len() > 2 }
            })
            .collect()
    })
}

/// How many of an ingredient's items `slots` hold.
pub fn craft_have(slots: &[Slot], items: &[ItemId]) -> u32 {
    slots.iter().flatten().filter(|s| items.contains(&s.item)).map(|s| s.count as u32).sum()
}

/// Takes what `needs` asks for out of `slots` (from the largest stacks first); false, with
/// `slots` as they were, if they do not hold it all.
pub fn craft_pay(slots: &mut [Slot], needs: &[(Vec<ItemId>, u8)]) -> bool {
    let before = slots.to_vec();
    for (items, n) in needs {
        let mut left = *n;
        while left > 0 {
            let best = slots
                .iter()
                .enumerate()
                .filter_map(|(i, s)| s.filter(|s| items.contains(&s.item)).map(|s| (i, s.count)))
                .max_by_key(|&(_, c)| c);
            let Some((i, c)) = best else {
                slots.copy_from_slice(&before);
                return false;
            };
            let k = c.min(left);
            inventory::take(&mut slots[i], k);
            left -= k;
        }
    }
    true
}

#[cfg(test)]
mod craft_list_tests {
    use super::*;

    #[test]
    fn planks_from_any_log_and_a_table_from_four_planks() {
        let planks = craft_list().iter().find(|l| l.result.item == PLANKS as ItemId).unwrap();
        assert!(!planks.table);
        let mut slots: Vec<Slot> = vec![Some(Stack::new(BIRCH_LOG as ItemId, 2)), None];
        assert!(craft_pay(&mut slots, &planks.needs));
        assert_eq!(slots[0].unwrap().count, 1);
        let table = craft_list().iter().find(|l| l.result.item == CRAFTING_TABLE as ItemId).unwrap();
        assert_eq!(table.needs, vec![(vec![PLANKS as ItemId], 4)]);
        let mut few: Vec<Slot> = vec![Some(Stack::new(PLANKS as ItemId, 3))];
        assert!(!craft_pay(&mut few, &table.needs));
        assert_eq!(few[0].unwrap().count, 3);
    }

    #[test]
    fn three_by_three_recipes_need_a_table() {
        let pick = tool_id(ToolKind::Pickaxe, Tier::Wood);
        assert!(craft_list().iter().find(|l| l.result.item == pick).unwrap().table);
    }
}

