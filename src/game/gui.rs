//! Item screens: the inventory and the creative inventory, and the slot handling shared
//! with the chest and crafting table views (`station`).
//! Slot interaction follows Minecraft: left click picks up / places / swaps, right click
//! splits / places one, shift-click moves between sections, 1-9 swaps with the hotbar,
//! dragging a held stack spreads it over slots and double-click collects matching items.
//! A stack can also be dragged out of a slot and let go over another one.

mod gun_station;
pub(super) use gun_station::Pick as BenchPick;
mod jei;
pub(super) use jei::Jei;

use super::*;
use crate::item::inventory::{add_to, click, take};
use crate::item::*;
use crate::lang::tf;
use crate::ui::rgba;

/// GUI pixel size of a slot (16 px icon + 1 px border each side).
const SLOT: f32 = 18.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum SlotRef {
    Inv(usize),
    Craft(usize),
    CraftOut,
    Chest(usize),
    Creative(ItemId),
    Trash,
    /// What the player wears (helmet, chestplate, leggings, boots, vest).
    Armor(usize),
}

/// Colors of the item screens; light is classic Minecraft, dark is the optional dark mode.
struct Theme {
    border: Color,
    bevel_hi: Color,
    bevel_lo: Color,
    fill_top: Color,
    fill_bottom: Color,
    slot: Color,
    slot_shadow: Color,
    slot_light: Color,
    slot_inner: Color,
    hover: Color,
    label: Color,
    label_shadow: bool,
    idle: Color,
    progress: Color,
    preview_top: Color,
    preview_bottom: Color,
    scroll_track: Color,
    scroll_thumb: Color,
    /// Creative tabs that are not open.
    tab_idle: Color,
}

const LIGHT: Theme = Theme {
    border: rgba(16, 16, 20, 255),
    bevel_hi: rgba(255, 255, 255, 255),
    bevel_lo: rgba(86, 86, 92, 255),
    fill_top: rgba(198, 198, 204, 255),
    fill_bottom: rgba(198, 198, 204, 255),
    slot: rgba(55, 55, 60, 255),
    slot_shadow: rgba(35, 35, 38, 255),
    slot_light: rgba(120, 120, 128, 255),
    slot_inner: rgba(78, 78, 84, 255),
    hover: rgba(255, 255, 255, 70),
    label: rgba(64, 64, 70, 255),
    label_shadow: false,
    idle: rgba(139, 139, 145, 255),
    progress: rgba(255, 255, 255, 255),
    preview_top: rgba(28, 30, 38, 255),
    preview_bottom: rgba(12, 12, 16, 255),
    scroll_track: rgba(90, 90, 96, 255),
    scroll_thumb: rgba(220, 220, 226, 255),
    tab_idle: rgba(160, 160, 167, 255),
};

const DARK: Theme = Theme {
    border: rgba(6, 6, 9, 255),
    bevel_hi: rgba(74, 78, 96, 255),
    bevel_lo: rgba(12, 13, 18, 255),
    fill_top: rgba(42, 44, 56, 255),
    fill_bottom: rgba(32, 34, 44, 255),
    slot: rgba(22, 23, 30, 255),
    slot_shadow: rgba(12, 12, 17, 255),
    slot_light: rgba(62, 65, 80, 255),
    slot_inner: rgba(30, 32, 41, 255),
    hover: rgba(98, 214, 120, 60),
    label: rgba(214, 218, 232, 255),
    label_shadow: true,
    idle: rgba(76, 80, 98, 255),
    progress: rgba(120, 226, 140, 255),
    preview_top: rgba(18, 19, 26, 255),
    preview_bottom: rgba(6, 6, 9, 255),
    scroll_track: rgba(22, 23, 30, 255),
    scroll_thumb: rgba(112, 118, 140, 255),
    tab_idle: rgba(26, 27, 36, 255),
};

/// Mouse drag with a held stack: left spreads it evenly, right drops one per slot.
pub(super) struct Drag {
    right: bool,
    /// The cursor stack when the drag started.
    stack: Stack,
    /// Slots visited so far with their contents before the drag.
    slots: Vec<(SlotRef, Slot)>,
}

/// Whether `st` can be dropped onto a slot holding `cur`.
fn fits(cur: Slot, st: &Stack) -> bool {
    match cur {
        None => true,
        Some(c) => c.stacks_with(st) && c.count < max_stack(c.item),
    }
}

/// Slots that accept items from the cursor.
fn droppable(r: SlotRef) -> bool {
    matches!(r, SlotRef::Inv(_) | SlotRef::Craft(_) | SlotRef::Chest(_))
}

/// Lowercase without Hungarian accents, so "gyemant" finds "Gyémánt".
fn search_fold(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' | 'ö' | 'ő' => 'o',
            'ú' | 'ü' | 'ű' => 'u',
            '_' => ' ',
            c => c,
        })
        .collect()
}

/// Tabs along the top of the creative inventory, like Minecraft's: the item categories, and
/// the player's own inventory at the right end.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Tab {
    Blocks,
    Functional,
    /// Tools and weapons (guns, their ammunition and parts, grenades) together.
    Tools,
    /// Armor and the bulletproof vest.
    Armor,
    Food,
    Mobs,
    Materials,
    /// Every item, the other tabs one after the other.
    All,
    Inventory,
}

pub(super) const TABS: [Tab; 9] = [
    Tab::Blocks,
    Tab::Functional,
    Tab::Tools,
    Tab::Armor,
    Tab::Food,
    Tab::Mobs,
    Tab::Materials,
    Tab::All,
    Tab::Inventory,
];

/// Tab size in GUI pixels (the open one is taller and joins the window), and the distance
/// between the category tabs.
const TAB_W: f32 = 21.0;
const TAB_H: f32 = 24.0;
const TAB_STEP: f32 = 21.5;

impl Tab {
    fn name(self) -> &'static str {
        t(match self {
            Tab::Blocks => "gui.tab.blocks",
            Tab::Functional => "gui.tab.functional",
            Tab::Tools => "gui.tab.tools",
            Tab::Armor => "gui.tab.armor",
            Tab::All => "gui.tab.all",
            Tab::Food => "gui.tab.food",
            Tab::Mobs => "gui.tab.mobs",
            Tab::Materials => "gui.tab.materials",
            Tab::Inventory => "gui.inventory",
        })
    }

    fn icon(self) -> ItemId {
        match self {
            Tab::Blocks => GRASS as ItemId,
            Tab::Functional => CRAFTING_TABLE as ItemId,
            Tab::Tools => PISTOL,
            Tab::Armor => armor_id(2, 1),
            Tab::All => GUIDE_BOOK,
            Tab::Food => COOKED_PORKCHOP,
            Tab::Mobs => PIG_SPAWN_EGG,
            Tab::Materials => IRON_INGOT,
            Tab::Inventory => CHEST as ItemId,
        }
    }

    fn of(id: ItemId) -> Tab {
        if let Some(b) = block_of(id) {
            // Blocks that do something: stations, storage, lights, beds and doors.
            if matches!(
                b,
                CRAFTING_TABLE
                    | FURNACE
                    | BLAST_FURNACE
                    | ADV_FURNACE
                    | CHEST
                    | GUN_STATION
                    | RIFLE_BENCH
                    | BED
                    | OAK_DOOR
                    | TORCH
                    | LANTERN
            ) {
                Tab::Functional
            } else {
                Tab::Blocks
            }
        } else if armor_of(id).is_some() {
            Tab::Armor
        } else if tool_of(id).is_some()
            || matches!(
                id,
                BUCKET | WATER_BUCKET | LAVA_BUCKET | SHEARS | GLASS_BOTTLE | GUIDE_BOOK | FISHING_ROD
            )
            || GunKind::of(id).is_some()
            || matches!(
                id,
                BULLET
                    | MAGNUM_ROUND
                    | PISTOL_FRAME..=PISTOL_MAGAZINE
                    | SCOPE..=LASER_SIGHT
                    | FLASHLIGHT
                    | SPEEDLOADER
                    | REVOLVER_FRAME..=REVOLVER_HAMMER
                    | RIFLE_ROUND
                    | AK_MAGAZINE..=AK_COVER
                    | MAG_LOADER
                    | FRAG_GRENADE
                    | SMOKE_GRENADE
            )
        {
            Tab::Tools
        } else if consumable(id).is_some() || meat(id).is_some() {
            Tab::Food
        } else if key(id).ends_with("_spawn_egg") || id == TARGET_DUMMY {
            Tab::Mobs
        } else {
            Tab::Materials
        }
    }

    /// The items of the tab in the order they are shown, in groups; each group starts on a
    /// new row.
    fn groups(self) -> Vec<Vec<ItemId>> {
        let b = |ids: &[u8]| ids.iter().map(|&b| b as ItemId).collect::<Vec<_>>();
        let tools = |kind| {
            TIER_ORDER.map(|tier| tool_id(kind, tier)).to_vec()
        };
        match self {
            Tab::Blocks => vec![
                b(&[GRASS, SNOWY_GRASS, DIRT, SAND, GRAVEL, CLAY, SNOW, ICE]),
                b(&[STONE, COBBLE, STONE_BRICKS, SANDSTONE, BRICKS, OBSIDIAN, BEDROCK]),
                b(&[OAK_LOG, BIRCH_LOG, SPRUCE_LOG, PLANKS, OAK_STAIRS, GLASS, GLOWSTONE, WOOL]),
                b(&[
                    OAK_LEAVES,
                    BIRCH_LEAVES,
                    SPRUCE_LEAVES,
                    OAK_SAPLING,
                    BIRCH_SAPLING,
                    SPRUCE_SAPLING,
                    TALL_GRASS,
                    POPPY,
                    DANDELION,
                    DEAD_BUSH,
                    CACTUS,
                ]),
                b(&[
                    COAL_ORE,
                    COPPER_ORE,
                    IRON_ORE,
                    GOLD_ORE,
                    DIAMOND_ORE,
                ]),
                b(&[
                    COAL_BLOCK,
                    COPPER_BLOCK,
                    IRON_BLOCK,
                    GOLD_BLOCK,
                    DIAMOND_BLOCK,
                ]),
            ],
            Tab::Functional => vec![
                b(&[CRAFTING_TABLE, FURNACE, BLAST_FURNACE, ADV_FURNACE, GUN_STATION, RIFLE_BENCH]),
                b(&[CHEST, BED, OAK_DOOR, TORCH, LANTERN]),
            ],
            Tab::Tools => vec![
                tools(ToolKind::Pickaxe),
                tools(ToolKind::Axe),
                tools(ToolKind::Shovel),
                tools(ToolKind::Sword),
                vec![SHEARS, BUCKET, WATER_BUCKET, LAVA_BUCKET, GLASS_BOTTLE, GUIDE_BOOK, FISHING_ROD],
                // The pistol, its ammunition and the extended magazine, grenades, attachments
                // and pistol parts (the last one is the magazine).
                vec![PISTOL, REVOLVER, BULLET, MAGNUM_ROUND, EXTENDED_MAGAZINE, SPEEDLOADER],
                vec![AK47, RIFLE_ROUND, AK_MAGAZINE, MAG_LOADER],
                vec![FRAG_GRENADE, SMOKE_GRENADE],
                vec![SCOPE, SILENCER, LASER_SIGHT, FLASHLIGHT],
                vec![
                    PISTOL_FRAME,
                    PISTOL_BARREL,
                    PISTOL_SPRING,
                    PISTOL_SLIDE,
                    PISTOL_MAGAZINE,
                ],
                REVOLVER_PARTS.to_vec(),
                AK_PARTS[..4].to_vec(),
            ],
            // A row per material, then the vest.
            Tab::Armor => {
                let mut rows: Vec<Vec<ItemId>> = (0..MATERIALS)
                    .map(|m| (0..4).map(|p| armor_id(m, p)).collect())
                    .collect();
                rows.push(vec![BULLETPROOF_VEST]);
                rows
            }
            Tab::Food => vec![
                meat(PORKCHOP).unwrap().to_vec(),
                meat(MUTTON).unwrap().to_vec(),
                vec![WATER_BOTTLE, PURIFIED_WATER],
                vec![RAW_FISH, COOKED_FISH],
            ],
            Tab::Mobs => vec![vec![PIG_SPAWN_EGG, SHEEP_SPAWN_EGG, WOLF_SPAWN_EGG], vec![TARGET_DUMMY]],
            Tab::Materials => vec![
                vec![STICK, BONE, COAL, CHARCOAL, CLAY_BALL, BRICK],
                vec![COPPER_INGOT, IRON_NUGGET, IRON_INGOT, GOLD_INGOT, DIAMOND],
                vec![STEEL_INGOT, CERAMIC_PLATE],
            ],
            Tab::All | Tab::Inventory => Vec::new(),
        }
    }
}

/// The creative grid: a tab's groups, each starting on a new row (the gaps are `None`), and
/// at the end whatever item of the tab the groups do not list. A search looks through every
/// item instead, by display name (current language) or by item key.
fn creative_items(tab: Tab, query: &str) -> Vec<Option<ItemId>> {
    let q = search_fold(query.trim());
    if !q.is_empty() {
        return all_items()
            .into_iter()
            .filter(|&id| search_fold(&name(id)).contains(&q) || search_fold(&key(id)).contains(&q))
            .map(Some)
            .collect();
    }
    if tab == Tab::All {
        // Every tab's grid, one after the other.
        let mut grid = Vec::new();
        for t in TABS.into_iter().filter(|&t| !matches!(t, Tab::All | Tab::Inventory)) {
            grid.resize(grid.len().next_multiple_of(9), None);
            grid.extend(creative_items(t, ""));
        }
        return grid;
    }
    let listed: Vec<ItemId> = TABS.iter().flat_map(|t| t.groups().concat()).collect();
    let mut groups = tab.groups();
    groups.push(
        all_items()
            .into_iter()
            .filter(|id| !listed.contains(id) && Tab::of(*id) == tab)
            .collect(),
    );
    let mut grid = Vec::new();
    for g in groups.into_iter().filter(|g| !g.is_empty()) {
        grid.resize(grid.len().next_multiple_of(9), None);
        grid.extend(g.into_iter().map(Some));
    }
    grid
}

/// Draws an item icon with its stack count and durability bar. `size` is the icon size in pixels.
pub fn draw_stack(ui: &mut Ui, x: f32, y: f32, size: f32, st: &Stack) {
    let c = Vec2::new(x + size * 0.5, y + size * 0.5);
    // A gun, magazine or part as it is (its rounds, attachments, dirt), once drawn.
    let state = super::icons::state_icon(st);
    match icon(st.item) {
        _ if state.is_some() => ui.block_sprite(c, size * 0.5, state.unwrap_or(0), [255; 3]),
        Icon::Block(b) if is_stairs(b) => {
            // Two boxes seen from the same corner as the cube icons: the step in front,
            // the tall part behind it.
            let r = size * 0.47;
            let k = 0.866 * r;
            let at = |x: f32, y: f32, z: f32| {
                Vec2::new(
                    c.x + k * (x + z - 1.0),
                    c.y + r * (1.0 - y) - r * 0.5 * (1.0 + z - x),
                )
            };
            let layer = face_texture(b, 2);
            for (lo, hi) in [([0.0, 0.0, 0.0], [1.0, 0.5, 1.0]), ([0.0, 0.5, 0.5], [1.0, 1.0, 1.0])] {
                let [x0, y0, z0] = lo;
                let [x1, y1, z1] = hi;
                let top = [at(x0, y1, z1), at(x1, y1, z1), at(x1, y1, z0), at(x0, y1, z0)];
                let front = [at(x0, y1, z0), at(x1, y1, z0), at(x1, y0, z0), at(x0, y0, z0)];
                let side = [at(x1, y1, z0), at(x1, y1, z1), at(x1, y0, z1), at(x1, y0, z0)];
                ui.tex_quad(top, layer, 1.0);
                ui.tex_quad(front, layer, 0.8);
                ui.tex_quad(side, layer, 0.62);
            }
        }
        Icon::Block(b) if is_log(b) => {
            // Round, upright, seen from the same corner as the cube icons: the bark toward
            // us, the rings on top.
            let r = size * 0.47;
            let k = 0.866 * r;
            let at = |x: f32, y: f32, z: f32| {
                Vec2::new(
                    c.x + k * (x + z - 1.0),
                    c.y + r * (1.0 - y) - r * 0.5 * (1.0 + z - x),
                )
            };
            const SIDES: usize = 12;
            let (rad, rim) = (log_radius(OAK_LOG), crate::world::mesh::LOG_END_RIM);
            let ang = |i: usize| i as f32 / SIDES as f32 * std::f32::consts::TAU;
            let rim_at = |i: usize| (0.5 + ang(i).cos() * rad, 0.5 + ang(i).sin() * rad);
            let (bark, rings) = (face_texture(b, 0), face_texture(b, 2));
            for i in 0..SIDES {
                let mid = (ang(i) + ang(i + 1)) * 0.5;
                let (nc, ns) = (mid.cos(), mid.sin());
                // Only the sides facing us (toward +x and -z).
                if nc - ns <= 0.0 {
                    continue;
                }
                let ((x0, z0), (x1, z1)) = (rim_at(i), rim_at(i + 1));
                let (u0, u1) = ((i % 4) as f32 / 4.0, (i % 4 + 1) as f32 / 4.0);
                let shade = 0.71 + 0.09 * (-ns - nc);
                ui.tex_quad_uv(
                    [at(x0, 1.0, z0), at(x1, 1.0, z1), at(x1, 0.0, z1), at(x0, 0.0, z0)],
                    [[u0, 0.0], [u1, 0.0], [u1, 1.0], [u0, 1.0]],
                    bark,
                    shade,
                );
            }
            let f = rim / rad;
            let uv = |x: f32, z: f32| [0.5 + (x - 0.5) * f, 0.5 + (z - 0.5) * f];
            for i in 0..SIDES {
                let ((x0, z0), (x1, z1)) = (rim_at(i), rim_at(i + 1));
                let p = [at(0.5, 1.0, 0.5), at(x0, 1.0, z0), at(x1, 1.0, z1), at(x1, 1.0, z1)];
                ui.tex_quad_uv(p, [uv(0.5, 0.5), uv(x0, z0), uv(x1, z1), uv(x1, z1)], rings, 1.0);
            }
        }
        Icon::Block(b) => {
            let tint = icon_tint(b);
            let top = if tint_kind(b, 2) != TintKind::None {
                tint
            } else {
                [255; 3]
            };
            let side = if tint_kind(b, 0) != TintKind::None {
                tint
            } else {
                [255; 3]
            };
            ui.block_icon_faces(
                c,
                size * 0.47,
                face_texture(b, 2),
                face_texture(b, 5),
                face_texture(b, 0),
                top,
                side,
            );
        }
        Icon::Flat(layer) => {
            let tint = block_of(st.item).map(icon_tint).unwrap_or([255; 3]);
            ui.block_sprite(c, size * 0.5, layer, tint);
        }
    }
    let px = (size / 16.0).max(1.0);
    let max = max_damage(st.item);
    // A gun's dirt shows on the gun itself, not as a bar.
    let dirt = GunKind::of(st.item).is_some()
        || (PISTOL_FRAME..=PISTOL_MAGAZINE).contains(&st.item)
        || st.item == EXTENDED_MAGAZINE
        || REVOLVER_PARTS.contains(&st.item)
        || AK_PARTS.contains(&st.item);
    if max > 0 && st.damage > 0 && !dirt {
        let f = 1.0 - st.damage as f32 / max as f32;
        let (bx, by, bw) = (x + 2.0 * px, y + size - 3.0 * px, size - 4.0 * px);
        ui.solid(bx, by, bw, 2.0 * px, rgba(0, 0, 0, 255));
        let col = [(1.0 - f).min(1.0) * 2.0, f * 2.0, 0.0, 1.0].map(|v: f32| v.min(1.0));
        ui.solid(bx, by, (bw * f).round().max(px), px, col);
    }
    if st.count > 1 {
        let text = st.count.to_string();
        let fs = (px * 0.75).round().max(1.0);
        let tw = ui.text_width(&text, fs);
        ui.text(
            &text,
            x + size - tw + px * 0.5,
            y + size - 7.0 * fs + px * 0.5,
            fs,
            WHITE,
            true,
        );
    }
}

impl Game {
    /// The search changed: the list starts from the top again.
    fn scroll_creative_to_top(&mut self) {
        self.creative_scroll = 0.0;
        self.creative_scroll_anim = 0.0;
    }

    /// Keyboard input while the creative search box is focused.
    pub(super) fn search_key(&mut self, code: KeyCode, text: Option<&str>) {
        const MAX: usize = 24;
        match code {
            KeyCode::Escape => self.close_container(),
            KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Tab => self.search_focused = false,
            KeyCode::Backspace => {
                self.creative_search.pop();
                self.scroll_creative_to_top();
            }
            _ => {
                for c in text.unwrap_or("").chars() {
                    if !c.is_control() && self.creative_search.chars().count() < MAX {
                        self.creative_search.push(c);
                        self.scroll_creative_to_top();
                    }
                }
            }
        }
    }

    /// Search box in the top right corner of the creative inventory, right-aligned with the
    /// scroll bar and starting after the title (`title_end`, screen x). Click to type,
    /// right-click to clear.
    fn search_box(&mut self, px: f32, py: f32, title_end: f32) {
        let s = self.ui.s;
        let x = (px + 95.0 * s).max((title_end + 4.0 * s).round());
        let (y, w, h) = (py + 4.0 * s, px + 187.0 * s - x, 11.0 * s);
        let hovered = self.ui.hit(x, y, w, h);
        if self.ui.pressed {
            self.search_focused = hovered;
        }
        if hovered && self.ui.right_pressed {
            self.creative_search.clear();
            self.scroll_creative_to_top();
            self.search_focused = true;
        }
        let th = self.theme();
        let border = if self.search_focused {
            rgba(98, 214, 120, 255)
        } else if hovered {
            th.slot_light
        } else {
            th.slot_shadow
        };
        self.ui.solid(x, y, w, h, border);
        self.ui
            .solid(x + s, y + s, w - 2.0 * s, h - 2.0 * s, th.slot);

        // Magnifying glass.
        let icon = Vec2::new(x + 5.5 * s, y + 5.0 * s);
        let grey = rgba(170, 170, 178, 255);
        self.ui.ring(icon, 2.2 * s, s, grey);
        self.ui
            .solid(icon.x + 1.6 * s, icon.y + 1.6 * s, s, s, grey);
        self.ui
            .solid(icon.x + 2.4 * s, icon.y + 2.4 * s, s, s, grey);

        let fs = (s * 0.75).round().max(1.0);
        let (tx, ty) = (x + 10.0 * s, (y + (h - 7.0 * fs) * 0.5).round());
        let room = w - 13.0 * s;
        if self.creative_search.is_empty() && !self.search_focused {
            self.ui
                .text(t("gui.search"), tx, ty, fs, rgba(125, 125, 132, 255), false);
        } else {
            // Show the end of a long query.
            let mut shown = self.creative_search.as_str();
            while self.ui.text_width(shown, fs) > room - 2.0 * fs {
                let mut it = shown.chars();
                it.next();
                shown = it.as_str();
            }
            let tw = self.ui.text(shown, tx, ty, fs, WHITE, true);
            if self.search_focused && (self.time * 2.5) as i32 % 2 == 0 {
                self.ui.text("_", tx + tw + fs * 0.5, ty, fs, WHITE, true);
            }
        }
    }

    pub(super) fn open_container(&mut self, c: Container) {
        if let Some(p) = Self::container_pos(c) {
            // LAN player: ask the host for the contents.
            self.net_container_opened(p);
        }
        self.drag = None;
        self.press_pick = None;
        self.search_focused = false;
        self.jei.focused = false;
        self.open_station(c);
        self.screen = Screen::Container(c);
        self.set_grab(false);
        self.keys.clear();
    }

    /// Stores the crafting grid of an open crafting table back into the table.
    pub(super) fn stash_table(&mut self, clear: bool) {
        if let Screen::Container(Container::Crafting(p)) = self.screen {
            if self.craft.iter().any(|s| s.is_some()) {
                self.block_entities.tables.insert(p, self.craft);
            } else {
                self.block_entities.tables.remove(&p);
            }
            if clear {
                self.craft = [None; 9];
            }
        }
    }

    /// Closes an item screen: a crafting table keeps its grid, the 2x2 grid and the cursor
    /// go back to the inventory.
    pub(super) fn close_container(&mut self) {
        self.drag = None;
        self.press_pick = None;
        self.search_focused = false;
        if let Screen::Container(c) = self.screen {
            if Self::container_pos(c).is_some() {
                // LAN player: the last changes go to the host before closing.
                self.net_container_sync();
                self.net_container_closed();
            }
        }
        self.stash_table(true);
        if let Screen::Container(Container::GunStation(_)) = self.screen {
            self.close_gun_station();
        }
        let mut back: Vec<Stack> = self.craft.iter_mut().filter_map(|s| s.take()).collect();
        back.extend(self.cursor.take());
        back.extend(self.craft_out.take());
        self.craft_fx = None;
        for s in back {
            self.give(s);
        }
        self.resume();
    }

    /// The block a container screen belongs to.
    pub(super) fn container_pos(c: Container) -> Option<IVec3> {
        match c {
            Container::Crafting(p) | Container::Chest(p) | Container::GunStation(p) => Some(p),
            Container::Inventory | Container::Creative => None,
        }
    }

    /// Contents of a chest: 27 slots, or 54 for a double chest (left half first).
    pub(super) fn chest_slots(&self, p: IVec3) -> Vec<Slot> {
        let (a, b) = self.chest_halves(p);
        let get = |q: IVec3| {
            self.block_entities
                .chests
                .get(&q)
                .map_or([None; 27], |c| **c)
        };
        let mut out = get(a).to_vec();
        if let Some(b) = b {
            out.extend(get(b));
        }
        out
    }

    /// Stores `slots` (as `chest_slots` returns them) into the chest's halves.
    pub(super) fn set_chest_slots(&mut self, p: IVec3, slots: &[Slot]) {
        let (a, b) = self.chest_halves(p);
        for (q, part) in std::iter::once(a).chain(b).zip(slots.chunks(27)) {
            let c = self
                .block_entities
                .chests
                .entry(q)
                .or_insert_with(|| Box::new([None; 27]));
            for (s, v) in c.iter_mut().zip(part) {
                *s = *v;
            }
        }
    }

    fn craft_size(c: Container) -> usize {
        if matches!(c, Container::Crafting(_)) {
            3
        } else {
            2
        }
    }

    fn slot_mut(&mut self, c: Container, r: SlotRef) -> Option<&mut Slot> {
        match r {
            SlotRef::Inv(i) => Some(&mut self.inventory.slots[i]),
            SlotRef::Craft(i) => Some(&mut self.craft[i]),
            SlotRef::Chest(i) => match c {
                Container::Chest(p) => {
                    let (a, b) = self.chest_halves(p);
                    let (q, i) = if i < 27 { (a, i) } else { (b?, i - 27) };
                    self.block_entities.chests.get_mut(&q).map(|ch| &mut ch[i])
                }
                _ => None,
            },
            SlotRef::Armor(i) => Some(&mut self.inventory.armor[i]),
            _ => None,
        }
    }

    pub(super) fn craft_result(&self, c: Container) -> Option<Stack> {
        let n = Self::craft_size(c);
        craft(&self.craft[..n * n], n)
    }

    fn consume_craft_inputs(&mut self, c: Container) {
        let n = Self::craft_size(c);
        for s in &mut self.craft[..n * n] {
            take(s, 1);
        }
    }

    /// What the result slot shows: at a table what was crafted into its middle, in the
    /// inventory what the 2x2 grid makes.
    fn craft_out_stack(&self, c: Container) -> Option<Stack> {
        if matches!(c, Container::Crafting(_)) {
            self.craft_out
        } else {
            self.craft_result(c)
        }
    }

    /// A left click at an open table away from the slots: crafts from the grid as many as
    /// fit in one stack (64, or one of what does not stack), into the middle of the table.
    /// With more on the grid, the next click makes the next stack once this one is taken.
    fn craft_batch(&mut self, c: Container) {
        let before = self.craft;
        let mut made = self.craft_out;
        while let Some(res) = self.craft_result(c) {
            match &mut made {
                None => made = Some(res),
                Some(m)
                    if m.stacks_with(&res)
                        && m.count as u16 + res.count as u16 <= max_stack(m.item) as u16 =>
                {
                    m.count += res.count
                }
                _ => break,
            }
            self.consume_craft_inputs(c);
        }
        if made != self.craft_out {
            self.craft_out = made;
            self.craft_fx = Some((0.0, before));
            self.hand.swing();
        }
    }

    /// Shift-click inside the inventory: from the hotbar (slot `i` < 9) into the main part,
    /// or the other way round. Returns what does not fit.
    fn move_within_inventory(&mut self, i: usize, stack: Stack) -> Option<Stack> {
        if i < 9 {
            add_to(&mut self.inventory.slots[9..], stack)
        } else {
            add_to(&mut self.inventory.slots[..9], stack)
        }
    }

    /// Shift-click: move a stack to the "other" section.
    fn quick_move(&mut self, c: Container, r: SlotRef) {
        // In the inventory armor goes on (into its empty slot), and comes off.
        if let (Container::Inventory | Container::Creative, SlotRef::Inv(i)) = (c, r) {
            let piece = self.inventory.slots[i].and_then(|s| armor_of(s.item)).map(|a| a.0);
            if let Some(p) = piece.filter(|&p| self.inventory.armor[p].is_none()) {
                self.inventory.armor[p] = self.inventory.slots[i].take();
                self.audio.play(crate::audio::Sound::ArmorEquip, None, 0.8);
                return;
            }
        }
        let Some(stack) = self.slot_mut(c, r).and_then(|s| s.take()) else {
            return;
        };
        let left = match (c, r) {
            (Container::Chest(p), SlotRef::Inv(_)) => {
                let mut slots = self.chest_slots(p);
                let left = add_to(&mut slots, stack);
                self.set_chest_slots(p, &slots);
                left
            }
            (_, SlotRef::Inv(i)) => self.move_within_inventory(i, stack),
            _ => self.inventory.add(stack),
        };
        if let Some(left) = left {
            match self.slot_mut(c, r) {
                Some(s) if s.is_none() => *s = Some(left),
                _ => self.give(left),
            }
        }
    }

    fn click_slot(&mut self, c: Container, r: SlotRef, right: bool, shift: bool) {
        match r {
            SlotRef::CraftOut if matches!(c, Container::Crafting(_)) => {
                // At a table, what was crafted lies in the middle: a click puts it straight
                // into the inventory (what does not fit stays there).
                if let Some(st) = self.craft_out {
                    self.craft_out = self.inventory.add(st);
                }
            }
            SlotRef::CraftOut => {
                let Some(res) = self.craft_result(c) else {
                    return;
                };
                if shift {
                    // Craft as many as fit. Each result must fit completely, otherwise the part
                    // that did fit would be added without consuming the ingredients.
                    let mut n = 0;
                    while let Some(res) = self.craft_result(c) {
                        let mut slots = self.inventory.slots;
                        if add_to(&mut slots, res).is_some() || n > 64 {
                            break;
                        }
                        self.inventory.slots = slots;
                        self.consume_craft_inputs(c);
                        n += 1;
                    }
                    return;
                }
                match &mut self.cursor {
                    None => self.cursor = Some(res),
                    Some(cur)
                        if cur.stacks_with(&res)
                            && cur.count + res.count <= max_stack(res.item) =>
                    {
                        cur.count += res.count
                    }
                    _ => return,
                }
                self.consume_craft_inputs(c);
            }
            SlotRef::Creative(id) => {
                if shift {
                    // A full stack straight into the first empty slot (hotbar first).
                    if let Some(slot) = self.inventory.slots.iter_mut().find(|s| s.is_none()) {
                        *slot = Some(Stack::new(id, max_stack(id)));
                    }
                    return;
                }
                // Like Minecraft: a click takes one; clicking again with the same item adds one
                // more, and clicking with anything else puts the held stack away.
                match &mut self.cursor {
                    None => self.cursor = Some(Stack::one(id)),
                    Some(cur) if cur.item == id && cur.damage == 0 => {
                        cur.count = (cur.count + 1).min(max_stack(id));
                    }
                    Some(_) => self.cursor = None,
                }
            }
            SlotRef::Trash => self.cursor = None,
            // Only the piece that goes there.
            SlotRef::Armor(i)
                if self
                    .cursor
                    .is_some_and(|c| armor_of(c.item).map(|a| a.0) != Some(i)) => {}
            SlotRef::Armor(i) if !shift => {
                let mut cursor = self.cursor.take();
                click(&mut self.inventory.armor[i], &mut cursor, false);
                self.cursor = cursor;
                self.audio.play(crate::audio::Sound::ArmorEquip, None, 0.8);
            }
            _ => {
                if shift {
                    self.quick_move(c, r);
                } else {
                    let mut cursor = self.cursor.take();
                    if let Some(slot) = self.slot_mut(c, r) {
                        click(slot, &mut cursor, right);
                    }
                    self.cursor = cursor;
                }
            }
        }
    }

    /// Recomputes a drag from scratch: restores the visited slots, then spreads the stack.
    fn apply_drag(&mut self, c: Container, d: &Drag) {
        for (r, orig) in &d.slots {
            if let Some(s) = self.slot_mut(c, *r) {
                *s = *orig;
            }
        }
        let n = d.slots.len() as u8;
        let per = if d.right {
            1
        } else {
            (d.stack.count / n.max(1)).max(1)
        };
        let mut left = d.stack.count;
        for (r, orig) in &d.slots {
            let have = orig.map_or(0, |s| s.count);
            let put = per.min(max_stack(d.stack.item) - have).min(left);
            if put == 0 {
                continue;
            }
            left -= put;
            if let Some(s) = self.slot_mut(c, *r) {
                *s = Some(Stack {
                    count: have + put,
                    ..d.stack
                });
            }
        }
        self.cursor = (left > 0).then_some(Stack {
            count: left,
            ..d.stack
        });
    }

    /// Double-click: pull matching items from every slot into the cursor stack.
    fn collect_all(&mut self, c: Container) {
        let Some(mut cur) = self.cursor else { return };
        let max = max_stack(cur.item);
        let n = Self::craft_size(c);
        let mut refs: Vec<SlotRef> = (0..n * n).map(SlotRef::Craft).collect();
        if let Container::Chest(p) = c {
            refs.extend((0..self.chest_slots(p).len()).map(SlotRef::Chest));
        }
        refs.extend((9..36).chain(0..9).map(SlotRef::Inv));
        for r in refs {
            if cur.count >= max {
                break;
            }
            if let Some(slot) = self.slot_mut(c, r) {
                let k = match slot {
                    Some(s) if s.stacks_with(&cur) => (max - cur.count).min(s.count),
                    _ => 0,
                };
                cur.count += k;
                take(slot, k);
            }
        }
        self.cursor = Some(cur);
    }

    fn theme(&self) -> &'static Theme {
        if self.settings.dark_ui {
            &DARK
        } else {
            &LIGHT
        }
    }

    /// Draws one slot and returns whether the mouse is over it.
    fn draw_slot(&mut self, x: f32, y: f32, content: Option<Stack>) -> bool {
        self.draw_slot_sized(x, y, SLOT, content)
    }

    /// Slot of `gui` GUI pixels (18 normal, 26 for result slots); the item is centered.
    fn draw_slot_sized(&mut self, x: f32, y: f32, gui: f32, content: Option<Stack>) -> bool {
        let s = self.ui.s;
        let size = gui * s;
        let hovered = self.ui.hit(x, y, size, size);
        let th = self.theme();
        self.ui.solid(x, y, size, size, th.slot);
        self.ui.solid(x, y, size - s, s, th.slot_shadow);
        self.ui.solid(x, y, s, size - s, th.slot_shadow);
        self.ui
            .solid(x + s, y + size - s, size - s, s, th.slot_light);
        self.ui
            .solid(x + size - s, y + s, s, size - s, th.slot_light);
        self.ui
            .solid(x + s, y + s, size - 2.0 * s, size - 2.0 * s, th.slot_inner);
        if hovered {
            self.ui
                .solid(x + s, y + s, size - 2.0 * s, size - 2.0 * s, th.hover);
        }
        if let Some(st) = content {
            let o = (gui - 16.0) * 0.5 * s;
            draw_stack(&mut self.ui, x + o, y + o, 16.0 * s, &st);
        }
        hovered
    }

    fn tooltip_for(&mut self, st: &Stack) {
        let mut text = name(st.item);
        let max = max_damage(st.item);
        if let Some(kind) = GunKind::of(st.item) {
            let size = kind.magazine_size(gun_mods(st));
            let rounds = if gun_has_mag(st) {
                tf("gun.magazine", &[&gun_ready_rounds(st), &size])
            } else {
                crate::lang::t("gun.no_mag").split('!').next().unwrap_or("").to_string()
            };
            text = format!("{text}  ({rounds})");
        } else if st.item == AMMO_BOX {
            text = match box_ammo(st.data) {
                Some(kind) => format!("{text}  ({}/{} {})", box_rounds(st), AMMO_BOX_ROUNDS, name(kind)),
                None => format!("{text}  (0/{AMMO_BOX_ROUNDS})"),
            };
        } else if let Some(cap) = magazine_capacity(st.item) {
            text = format!(
                "{text}  ({}, {})",
                tf("gun.magazine", &[&gun_rounds(st), &cap]),
                crate::lang::t("gun.mag_hint")
            );
        } else if max > 0
            && st.damage > 0
            && !(PISTOL_FRAME..=PISTOL_SLIDE).contains(&st.item)
            && !REVOLVER_PARTS.contains(&st.item)
            && !AK_PARTS.contains(&st.item)
        {
            text = format!(
                "{text}  ({})",
                tf("gui.durability", &[&(max - st.damage), &max])
            );
        }
        self.ui.set_tooltip(&text);
    }

    /// Main inventory (3 rows) and hotbar at GUI offset (ox, oy) = top-left of the main rows.
    pub(super) fn inventory_slots(
        &mut self,
        px: f32,
        py: f32,
        oy: f32,
        hovered: &mut Option<SlotRef>,
    ) {
        let s = self.ui.s;
        for i in 9..36 {
            let (cx, cy) = ((i - 9) % 9, (i - 9) / 9);
            let (x, y) = (
                px + (8.0 + cx as f32 * SLOT) * s,
                py + (oy + cy as f32 * SLOT) * s,
            );
            if self.draw_slot(x, y, self.inventory.slots[i]) {
                *hovered = Some(SlotRef::Inv(i));
            }
        }
        for i in 0..9 {
            let (x, y) = (px + (8.0 + i as f32 * SLOT) * s, py + (oy + 58.0) * s);
            if self.draw_slot(x, y, self.inventory.slots[i]) {
                *hovered = Some(SlotRef::Inv(i));
            }
        }
    }

    /// The armor slots at these places (helmet, chestplate, leggings, boots, vest), each
    /// showing a faint picture of what goes there while empty.
    fn armor_slots(&mut self, spots: [(f32, f32); ARMOR_SLOTS], hovered: &mut Option<SlotRef>) {
        let s = self.ui.s;
        for (i, (x, y)) in spots.into_iter().enumerate() {
            if self.draw_slot(x, y, self.inventory.armor[i]) {
                *hovered = Some(SlotRef::Armor(i));
            }
            if self.inventory.armor[i].is_none() {
                let hint = if i == VEST_SLOT { BULLETPROOF_VEST } else { armor_id(2, i) };
                draw_stack(&mut self.ui, x + s, y + s, 16.0 * s, &Stack::one(hint));
                let th = self.theme();
                self.ui
                    .solid(x + s, y + s, 16.0 * s, 16.0 * s, with_alpha(th.slot_inner, 0.8));
            }
        }
    }

    fn panel(&mut self, pw: f32, ph: f32) -> (f32, f32) {
        self.panel_below(pw, ph, 0.0)
    }

    /// A window centered together with `top` GUI pixels above it (the creative tabs).
    fn panel_below(&mut self, pw: f32, ph: f32, top: f32) -> (f32, f32) {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        self.ui
            .gradient(0.0, 0.0, w, h, rgba(0, 0, 0, 150), rgba(0, 0, 0, 110));
        let (px, py) = (
            ((w - pw * s) * 0.5).round(),
            ((h - (ph + top) * s) * 0.5 + top * s).round(),
        );
        let (bw, bh) = (pw * s, ph * s);
        self.ui.rect_full(
            px - s,
            py + s,
            bw + 2.0 * s,
            bh + 2.0 * s,
            rgba(0, 0, 0, 90),
            rgba(0, 0, 0, 90),
            5.0 * s,
            6.0 * s,
        );
        let th = self.theme();
        self.ui.rect(
            px - s,
            py - s,
            bw + 2.0 * s,
            bh + 2.0 * s,
            th.border,
            4.0 * s,
        );
        self.ui.rect(px, py, bw, bh, th.bevel_hi, 3.0 * s);
        self.ui.rect(
            px + 2.0 * s,
            py + 2.0 * s,
            bw - 2.0 * s,
            bh - 2.0 * s,
            th.bevel_lo,
            3.0 * s,
        );
        self.ui.rect_full(
            px + 2.0 * s,
            py + 2.0 * s,
            bw - 4.0 * s,
            bh - 4.0 * s,
            th.fill_top,
            th.fill_bottom,
            2.0 * s,
            0.0,
        );
        (px, py)
    }

    /// Minecraft-style progress arrow `w` GUI pixels long (15 tall), filled left to right by `k`.
    fn arrow(&mut self, x: f32, y: f32, w: f32, k: f32) {
        let s = self.ui.s;
        let head = 8.0;
        let fill = (w * k.clamp(0.0, 1.0)).round();
        let th = self.theme();
        for col in 0..w as i32 {
            let c = col as f32;
            let (y0, h) = if c < w - head {
                (5.0, 5.0)
            } else {
                let d = w - 1.0 - c;
                (7.0 - d, 1.0 + 2.0 * d)
            };
            let color = if c < fill { th.progress } else { th.idle };
            self.ui.solid(x + c * s, y + y0 * s, s, h * s, color);
        }
    }

    fn label(&mut self, text: &str, x: f32, y: f32) {
        let size = (self.ui.s * 0.75).round().max(1.0);
        let th = self.theme();
        self.ui.text(text, x, y, size, th.label, th.label_shadow);
    }

    /// Player figure for the inventory screen, inside the dark box at (x, y, w, h).
    /// Like Minecraft, the body turns a little toward the mouse and the head follows it.
    fn player_preview(&mut self, x: f32, y: f32, w: f32, h: f32) {
        use crate::model::player::{ARM, BODY, HEAD, LEG};
        use crate::world::textures::tex;
        use glam::Mat3;
        let s = self.ui.s;
        let th = self.theme();
        self.ui
            .gradient(x, y, w, h, th.preview_top, th.preview_bottom);
        let u = s * 1.8;
        let (cx, feet) = (x + w * 0.5, y + h - 5.0 * s);
        let m = self.ui.mouse;
        let f = ((m.x - cx) / s / 40.0).atan();
        let g = ((m.y - (feet - 28.0 * u)) / s / 40.0).atan();
        let body = Mat3::from_rotation_y((f * 20.0).to_radians());
        let head = Mat3::from_rotation_y((f * 40.0).to_radians())
            * Mat3::from_rotation_x((g * 20.0).to_radians());
        let neck = Vec3::new(0.0, 24.0, 0.0);
        let plain = [255u8; 3];
        let v = Vec3::new;
        // What is worn (as on the player model), in the colors of its material.
        let (worn, vest) = unpack_armor(armor_code(&self.inventory.armor));
        let look = |m: usize| match m {
            0 => ([tex::ARMOR_WOOL; 6], [196, 184, 160]),
            1 => ([tex::ARMOR_METAL; 6], [226, 146, 96]),
            2 => ([tex::ARMOR_METAL; 6], [176, 184, 198]),
            _ => ([tex::ARMOR_METAL; 6], [120, 228, 232]),
        };
        type Part = (Vec3, Vec3, [u32; 6], Mat3, Vec3, [u8; 3]);
        let on = |m: Option<usize>, boxes: &[(Vec3, Vec3)], rot: Mat3, pivot: Vec3| -> Vec<Part> {
            m.map(|m| {
                let (t, c) = look(m);
                boxes.iter().map(|&(lo, hi)| (lo, hi, t, rot, pivot, c)).collect()
            })
            .unwrap_or_default()
        };
        // Each part of the body (min, max, textures, rotation, pivot, tint; model pixels, feet
        // at y = 0, facing +z) with what is worn over it, drawn right after it.
        let leg = |x0: f32, x1: f32| -> (Part, Vec<Part>) {
            let mut over = on(worn[2], &[(v(x0 - 0.5, 3.5, -2.5), v(x1 + 0.5, 12.3, 2.5))], body, Vec3::ZERO);
            over.extend(on(worn[3], &[(v(x0 - 0.6, -0.4, -2.6), v(x1 + 0.6, 3.7, 2.6))], body, Vec3::ZERO));
            ((v(x0, 0.0, -2.0), v(x1, 12.0, 2.0), LEG, body, Vec3::ZERO, plain), over)
        };
        let arm = |x0: f32, x1: f32| -> (Part, Vec<Part>) {
            let over = on(worn[1], &[(v(x0 - 0.6, 20.0, -2.6), v(x1 + 0.6, 24.6, 2.6))], body, Vec3::ZERO);
            ((v(x0, 12.0, -2.0), v(x1, 24.0, 2.0), ARM, body, Vec3::ZERO, plain), over)
        };
        let torso = {
            let mut over = on(worn[2], &[(v(-4.5, 11.6, -2.5), v(4.5, 14.2, 2.5))], body, Vec3::ZERO);
            over.extend(on(worn[1], &[(v(-4.6, 13.2, -2.6), v(4.6, 24.5, 2.6))], body, Vec3::ZERO));
            if vest {
                over.push((v(-4.9, 13.8, -3.0), v(4.9, 24.6, 3.0), [tex::VEST; 6], body, Vec3::ZERO, [118, 124, 92]));
                for (x0, x1) in [(-3.8, -1.5), (-1.1, 1.1), (1.5, 3.8)] {
                    over.push((v(x0, 14.4, 3.0), v(x1, 17.4, 3.7), [tex::VEST; 6], body, Vec3::ZERO, [94, 100, 74]));
                }
            }
            ((v(-4.0, 12.0, -2.0), v(4.0, 24.0, 2.0), BODY, body, Vec3::ZERO, plain), over)
        };
        let mut groups = vec![leg(-4.0, 0.0), leg(0.0, 4.0), torso, arm(-8.0, -4.0), arm(4.0, 8.0)];
        // Painter's order: farthest first, head always last (it sits on top of everything).
        groups.sort_by(|a, b| {
            let za = (a.0 .3 * ((a.0 .0 + a.0 .1) * 0.5)).z;
            let zb = (b.0 .3 * ((b.0 .0 + b.0 .1) * 0.5)).z;
            za.total_cmp(&zb)
        });
        // The helmet over the head: the cap, the guard at the back and the cheek guards
        // (drawn after the head, without their faces turned in toward it).
        let helmet = on(
            worn[0],
            &[
                (v(-4.6, 24.5, -4.6), v(4.6, 28.6, -1.2)),
                (v(-4.6, 28.6, -4.6), v(4.6, 32.7, 4.6)),
                (v(-4.6, 25.5, -1.2), v(-3.6, 28.6, 4.6)),
                (v(3.6, 25.5, -1.2), v(4.6, 28.6, 4.6)),
            ],
            head,
            neck,
        );
        let mut parts: Vec<(Part, Option<Vec3>)> = Vec::new();
        for (base, over) in groups {
            parts.push((base, None));
            parts.extend(over.into_iter().map(|p| (p, None)));
        }
        parts.push(((v(-4.0, 24.0, -4.0), v(4.0, 32.0, 4.0), HEAD, head, neck, plain), None));
        let head_center = v(0.0, 28.0, 0.0);
        parts.extend(helmet.into_iter().map(|p| (p, Some(head_center))));
        let light = Vec3::new(0.35, 0.55, 0.76).normalize();
        for ((lo, hi, tex, rot, pivot, tint), inner) in parts {
            let (x0, y0, z0, x1, y1, z1) = (lo.x, lo.y, lo.z, hi.x, hi.y, hi.z);
            let v = Vec3::new;
            // Corners TL, TR, BR, BL as seen from outside; texture index as in crate::model
            // (+X, -X, +Y, -Y, back, front), with the front facing the viewer (+z).
            let faces = [
                (
                    v(0.0, 0.0, 1.0),
                    5,
                    [v(x0, y1, z1), v(x1, y1, z1), v(x1, y0, z1), v(x0, y0, z1)],
                ),
                (
                    v(0.0, 0.0, -1.0),
                    4,
                    [v(x1, y1, z0), v(x0, y1, z0), v(x0, y0, z0), v(x1, y0, z0)],
                ),
                (
                    v(1.0, 0.0, 0.0),
                    0,
                    [v(x1, y1, z1), v(x1, y1, z0), v(x1, y0, z0), v(x1, y0, z1)],
                ),
                (
                    v(-1.0, 0.0, 0.0),
                    1,
                    [v(x0, y1, z0), v(x0, y1, z1), v(x0, y0, z1), v(x0, y0, z0)],
                ),
                (
                    v(0.0, 1.0, 0.0),
                    2,
                    [v(x0, y1, z0), v(x1, y1, z0), v(x1, y1, z1), v(x0, y1, z1)],
                ),
                (
                    v(0.0, -1.0, 0.0),
                    3,
                    [v(x0, y0, z1), v(x1, y0, z1), v(x1, y0, z0), v(x0, y0, z0)],
                ),
            ];
            for (n, ti, corners) in faces {
                // A face turned in toward what it covers is hidden inside it.
                if let Some(c) = inner {
                    let middle = (corners[0] + corners[2]) * 0.5;
                    if n.dot(c - middle) > 0.0 {
                        continue;
                    }
                }
                let n = rot * n;
                if n.z <= 0.01 {
                    continue;
                }
                let shade = 0.55 + 0.45 * n.dot(light).max(0.0);
                let p = corners.map(|c| {
                    let q = rot * (c - pivot) + pivot;
                    Vec2::new(cx + q.x * u, feet - q.y * u)
                });
                self.ui.tex_quad_tint(p, tex[ti], shade, tint);
            }
        }
    }

    /// The creative tabs above the window at (px, py): an icon for each category and one for
    /// the player's own inventory. A click opens a tab (and clears the search). Returns
    /// whether the mouse is over them.
    fn creative_tabs(&mut self, px: f32, py: f32, panel_w: f32) -> bool {
        let s = self.ui.s;
        let th = self.theme();
        let mut over = false;
        for (i, &tab) in TABS.iter().enumerate() {
            // The inventory tab sits apart at the right end, like in Minecraft.
            let gx = if tab == Tab::Inventory {
                panel_w - TAB_W
            } else {
                i as f32 * TAB_STEP
            };
            let x = px + (gx - 1.0) * s;
            let open = i == self.creative_tab;
            let y = py - (TAB_H + if open { 3.0 } else { 0.0 }) * s;
            let w = TAB_W * s;
            let hovered = self.ui.hit(x, y, w, py - y);
            if open {
                // Joins the window: its fill runs over the window's top edge.
                self.ui.rect(x, y, w, (TAB_H + 5.0) * s, th.border, 3.0 * s);
                self.ui.rect(
                    x + s,
                    y + s,
                    w - 2.0 * s,
                    (TAB_H + 3.0) * s,
                    th.bevel_hi,
                    2.0 * s,
                );
                self.ui.rect_full(
                    x + 2.0 * s,
                    y + 2.0 * s,
                    w - 4.0 * s,
                    (TAB_H + 3.0) * s,
                    th.fill_top,
                    th.fill_top,
                    2.0 * s,
                    0.0,
                );
            } else {
                self.ui.rect(x, y, w, TAB_H * s, th.border, 3.0 * s);
                let fill = if hovered { th.slot_light } else { th.tab_idle };
                self.ui.rect_full(
                    x + s,
                    y + s,
                    w - 2.0 * s,
                    (TAB_H - 1.0) * s,
                    fill,
                    fill,
                    2.0 * s,
                    0.0,
                );
            }
            let icon_y = y + (if open { 6.0 } else { 4.0 }) * s;
            draw_stack(
                &mut self.ui,
                x + ((TAB_W - 16.0) * 0.5 * s).round(),
                icon_y,
                16.0 * s,
                &Stack::one(tab.icon()),
            );
            if hovered {
                over = true;
                if self.cursor.is_none() {
                    self.ui.set_tooltip(tab.name());
                }
                if self.ui.pressed && !open {
                    self.creative_tab = i;
                    self.creative_search.clear();
                    self.search_focused = false;
                    self.scroll_drag = false;
                    self.scroll_creative_to_top();
                }
            }
        }
        over
    }

    /// The hotbar along the bottom of every creative tab, with the slot that destroys items.
    fn creative_hotbar(&mut self, px: f32, py: f32, hovered: &mut Option<SlotRef>) {
        let s = self.ui.s;
        for i in 0..9 {
            let (x, y) = (px + (9.0 + i as f32 * SLOT) * s, py + 134.0 * s);
            if self.draw_slot(x, y, self.inventory.slots[i]) {
                *hovered = Some(SlotRef::Inv(i));
            }
        }
        let (x, y) = (px + 173.0 * s, py + 134.0 * s);
        if self.draw_slot(x, y, None) {
            *hovered = Some(SlotRef::Trash);
            self.ui.set_tooltip(t("gui.trash"));
        }
        let (lx, ly) = (px + 178.0 * s, py + 138.0 * s);
        self.ui.text("x", lx, ly, s, rgba(200, 60, 60, 255), false);
    }

    /// Draws the open item screen and handles clicks.
    pub(super) fn container_screen(&mut self, c: Container) {
        if let Container::GunStation(p) = c {
            let hovered = self.gun_station_screen(p);
            let inside = self.station_inside;
            self.slot_input(c, hovered, None, inside);
            return;
        }
        if matches!(c, Container::Chest(_) | Container::Crafting(_)) {
            let mut hovered = self.station_screen(c);
            // JEI beside the inventory strip, most useful at a crafting table.
            let mut hovered_stack = None;
            let strip_right = (self.ui.w + 176.0 * self.ui.s) * 0.5;
            let over_jei = self.jei_panel(strip_right, &mut hovered, &mut hovered_stack);
            let inside = self.station_inside || over_jei;
            self.slot_input(c, hovered, hovered_stack, inside);
            return;
        }
        let s = self.ui.s;
        let mut hovered: Option<SlotRef> = None;
        let mut hovered_stack: Option<Stack> = None;
        let (panel_w, panel_h) = match c {
            Container::Creative => (195.0, 160.0),
            _ => (176.0, 166.0),
        };
        let (px, py) = if c == Container::Creative {
            self.panel_below(panel_w, panel_h, TAB_H + 4.0)
        } else {
            self.panel(panel_w, panel_h)
        };
        let over_tabs = c == Container::Creative && self.creative_tabs(px, py, panel_w);
        if matches!(c, Container::Inventory | Container::Creative) {
            self.draw_effects_list(px, py, panel_w * s);
        }
        let at = |gx: f32, gy: f32| (px + gx * s, py + gy * s);

        match c {
            Container::Chest(_) | Container::Crafting(_) | Container::GunStation(_) => {}
            Container::Inventory => {
                let n = Self::craft_size(c);
                // Grid origin, result slot frame (26 px) origin, arrow x and width.
                let (grid_x, grid_y, out_x, out_y, arrow_x, arrow_w) = if n == 3 {
                    (30.0, 17.0, 120.0, 31.0, 90.0, 24.0)
                } else {
                    (86.0, 18.0, 144.0, 23.0, 125.0, 17.0)
                };
                let title = t("gui.crafting");
                let (tx, ty) = at(if n == 3 { 30.0 } else { 86.0 }, 6.0);
                self.label(title, tx, ty);
                if n == 2 {
                    let (ax, ay) = at(26.0, 8.0);
                    self.player_preview(ax, ay, 51.0 * s, 70.0 * s);
                    // What is worn: the four pieces down the left, the vest by the figure.
                    let spots: [(f32, f32); ARMOR_SLOTS] = std::array::from_fn(|i| {
                        if i == VEST_SLOT {
                            at(77.0, 60.0)
                        } else {
                            at(7.0, 8.0 + i as f32 * SLOT)
                        }
                    });
                    self.armor_slots(spots, &mut hovered);
                }
                for i in 0..n * n {
                    let (x, y) = at(
                        grid_x + (i % n) as f32 * SLOT,
                        grid_y + (i / n) as f32 * SLOT,
                    );
                    if self.draw_slot(x, y, self.craft[i]) {
                        hovered = Some(SlotRef::Craft(i));
                    }
                }
                let (ax, ay) = at(arrow_x, out_y + 5.5);
                self.arrow(ax, ay, arrow_w, 0.0);
                let result = self.craft_result(c);
                let (ox, oy) = at(out_x, out_y);
                if self.draw_slot_sized(ox, oy, 26.0, result) {
                    hovered = Some(SlotRef::CraftOut);
                }
                if n == 3 {
                    let (lx, ly) = at(8.0, 73.0);
                    self.label(t("gui.inventory"), lx, ly);
                }
                self.inventory_slots(px, py, 84.0, &mut hovered);
            }
            Container::Creative if TABS[self.creative_tab] == Tab::Inventory => {
                // The player's own inventory: the figure, the main rows and the hotbar.
                let (ax, ay) = at(9.0, 6.0);
                self.player_preview(ax, ay, 51.0 * s, 66.0 * s);
                let (lx, ly) = at(66.0, 6.0);
                self.label(t("gui.inventory"), lx, ly);
                // What is worn, beside the figure: helmet, chestplate and the vest over it in
                // one column, leggings and boots in the next.
                let spots = [
                    at(64.0, 17.0),
                    at(64.0, 35.0),
                    at(82.0, 17.0),
                    at(82.0, 35.0),
                    at(64.0, 53.0),
                ];
                self.armor_slots(spots, &mut hovered);
                for i in 9..36 {
                    let (cx, cy) = ((i - 9) % 9, (i - 9) / 9);
                    let (x, y) = at(9.0 + cx as f32 * SLOT, 76.0 + cy as f32 * SLOT);
                    if self.draw_slot(x, y, self.inventory.slots[i]) {
                        hovered = Some(SlotRef::Inv(i));
                    }
                }
                self.creative_hotbar(px, py, &mut hovered);
            }
            Container::Creative => {
                let tab = TABS[self.creative_tab];
                let title = if self.creative_search.trim().is_empty() {
                    tab.name()
                } else {
                    t("gui.tab.search")
                };
                let (tx, ty) = at(8.0, 6.0);
                self.label(title, tx, ty);
                let fs = (s * 0.75).round().max(1.0);
                let title_end = tx + self.ui.text_width(title, fs);
                self.search_box(px, py, title_end);
                let all = creative_items(tab, &self.creative_search);
                if all.iter().all(|i| i.is_none()) {
                    let (cx, cy) = at(9.0 + 4.5 * SLOT, 18.0 + 2.6 * SLOT);
                    self.ui.text_centered(
                        t("gui.no_results"),
                        cx,
                        cy,
                        fs,
                        rgba(150, 150, 158, 255),
                        true,
                    );
                }
                let rows = all.len().div_ceil(9);
                let visible = 6;
                let max_scroll = rows.saturating_sub(visible) as f32;
                let (sx, sy) = at(175.0, 18.0);
                let track_h = visible as f32 * SLOT * s;
                let bar_h = 15.0 * s;
                // Grabbing the scroll bar (or clicking its track) follows the mouse directly.
                if self.ui.pressed && self.ui.hit(sx, sy, 12.0 * s, track_h) {
                    self.scroll_drag = true;
                }
                if !self.left_down {
                    self.scroll_drag = false;
                }
                if self.scroll_drag && max_scroll > 0.0 {
                    let k =
                        ((self.ui.mouse.y - sy - bar_h * 0.5) / (track_h - bar_h)).clamp(0.0, 1.0);
                    self.creative_scroll = k * max_scroll;
                    self.creative_scroll_anim = self.creative_scroll;
                }
                // The wheel moves the target one row per notch; the list glides there.
                self.creative_scroll =
                    (self.creative_scroll - self.ui.scroll).clamp(0.0, max_scroll);
                let ease = 1.0 - (-18.0 * self.ui.dt).exp();
                self.creative_scroll_anim +=
                    (self.creative_scroll - self.creative_scroll_anim) * ease;
                if (self.creative_scroll - self.creative_scroll_anim).abs() < 0.002 {
                    self.creative_scroll_anim = self.creative_scroll;
                }
                let scroll = self.creative_scroll_anim.clamp(0.0, max_scroll);
                let first = scroll.floor() as usize;
                let frac = scroll - first as f32;
                // Partly scrolled rows are cut off at the edges of the grid.
                let (gx, gy) = at(9.0, 18.0);
                self.ui.set_clip(Some([gx, gy, 9.0 * SLOT * s, track_h]));
                // Only the part of a cut-off row inside the grid can be clicked.
                let in_grid = self.ui.hit(gx, gy, 9.0 * SLOT * s, track_h);
                for r in 0..=visible {
                    for cidx in 0..9 {
                        let i = (first + r) * 9 + cidx;
                        let x = px + (9.0 + cidx as f32 * SLOT) * s;
                        let y = (py + (18.0 + (r as f32 - frac) * SLOT) * s).round();
                        let id = all.get(i).copied().flatten();
                        let content = id.map(|id| Stack::new(id, 1));
                        if self.draw_slot(x, y, content) && in_grid {
                            if let Some(id) = id {
                                hovered = Some(SlotRef::Creative(id));
                                hovered_stack = content;
                            } else {
                                // Like Minecraft: a held stack put into an empty spot of
                                // the item grid is gone.
                                hovered = Some(SlotRef::Trash);
                            }
                        }
                    }
                }
                self.ui.set_clip(None);
                // Scroll bar
                let th = self.theme();
                self.ui.solid(sx, sy, 12.0 * s, track_h, th.scroll_track);
                let k = if max_scroll > 0.0 {
                    scroll / max_scroll
                } else {
                    0.0
                };
                self.ui.solid(
                    sx + s,
                    (sy + k * (track_h - bar_h)).round(),
                    10.0 * s,
                    bar_h,
                    // Greyed out when everything fits and there is nothing to scroll.
                    if max_scroll > 0.0 {
                        th.scroll_thumb
                    } else {
                        th.idle
                    },
                );
                self.creative_hotbar(px, py, &mut hovered);
            }
        }

        // JEI: every item beside the inventory (and the creative "inventory" tab).
        let jei = matches!(c, Container::Inventory)
            || (c == Container::Creative && TABS[self.creative_tab] == Tab::Inventory);
        let over_jei = jei && self.jei_panel(px + panel_w * s, &mut hovered, &mut hovered_stack);
        let panel = (px, py, panel_w * s, panel_h * s);
        let inside = over_tabs || over_jei || self.ui.hit(panel.0, panel.1, panel.2, panel.3);
        self.slot_input(c, hovered, hovered_stack, inside);
    }

    /// Tooltips, clicks, drags and number keys on the slot under the mouse (`hovered`;
    /// `hovered_stack` for creative items), and the stack on the cursor. A click that is not
    /// `inside` (the window, or the chest or table and the inventory) throws the held stack.
    fn slot_input(
        &mut self,
        c: Container,
        hovered: Option<SlotRef>,
        hovered_stack: Option<Stack>,
        inside: bool,
    ) {
        let s = self.ui.s;
        // A stack dragged out of a slot and let go over another slot goes there.
        if let Some(from) = self.press_pick {
            if !self.left_down {
                self.press_pick = None;
                if let Some(r) = hovered.filter(|&r| r != from && droppable(r)) {
                    if self.cursor.is_some() {
                        self.click_slot(c, r, false, false);
                    }
                }
            }
        }

        // Tooltip for the hovered stack.
        if self.cursor.is_none() {
            if let Some(r) = hovered {
                let st = match r {
                    SlotRef::Creative(_) => hovered_stack,
                    SlotRef::CraftOut => self.craft_out_stack(c),
                    SlotRef::Trash => None,
                    _ => self.slot_mut(c, r).and_then(|s| *s),
                };
                if let Some(st) = st {
                    self.tooltip_for(&st);
                }
            }
        }

        // Clicks
        let shift = self.ui.shift;
        if let Some(mut d) = self.drag.take() {
            // Dragging: add newly entered slots and respread.
            if let Some(r) = hovered {
                let cur = self.slot_mut(c, r).and_then(|s| *s);
                if droppable(r)
                    && !d.slots.iter().any(|e| e.0 == r)
                    && (d.slots.len() as u8) < d.stack.count
                    && fits(cur, &d.stack)
                {
                    d.slots.push((r, cur));
                    self.apply_drag(c, &d);
                }
            }
            let held = if d.right {
                self.right_down
            } else {
                self.left_down
            };
            if held {
                self.drag = Some(d);
            }
        } else if let Some(r) = hovered {
            // Middle click (creative): a full stack of the hovered item on the cursor.
            let middle = self.middle_pressed && !self.cursor_grabbed;
            if middle && self.creative() && self.cursor.is_none() {
                let st = match r {
                    SlotRef::Creative(id) => Some(Stack::one(id)),
                    SlotRef::CraftOut => self.craft_out_stack(c),
                    SlotRef::Trash => None,
                    _ => self.slot_mut(c, r).and_then(|s| *s),
                };
                self.cursor = st.map(|st| Stack::new(st.item, max_stack(st.item)));
            }
            if self.ui.pressed || self.ui.right_pressed {
                let right = !self.ui.pressed;
                let double = !right
                    && !shift
                    && self.slot_click.1 == Some(r)
                    && self.time - self.slot_click.0 < 0.3;
                if !right {
                    self.slot_click = (self.time, Some(r));
                }
                let cur = self.slot_mut(c, r).and_then(|s| *s);
                match self.cursor {
                    Some(_) if double && droppable(r) => self.collect_all(c),
                    Some(st) if !shift && droppable(r) && fits(cur, &st) => {
                        let d = Drag {
                            right,
                            stack: st,
                            slots: vec![(r, cur)],
                        };
                        self.apply_drag(c, &d);
                        self.drag = Some(d);
                    }
                    _ => {
                        let empty = self.cursor.is_none();
                        self.click_slot(c, r, right, shift && !right);
                        if empty && !right && !shift && self.cursor.is_some() {
                            self.press_pick = Some(r);
                        }
                    }
                }
            }
            if let Some(d) = self.digit {
                // Number key: swap with that hotbar slot.
                // Swapping a hotbar slot with itself is a no-op (and would otherwise lose the item).
                if r != SlotRef::Inv(d)
                    && !matches!(
                        r,
                        SlotRef::CraftOut
                            | SlotRef::Creative(_)
                            | SlotRef::Trash
                            | SlotRef::Armor(_)
                    )
                {
                    let mut hot = self.inventory.slots[d].take();
                    if let Some(slot) = self.slot_mut(c, r) {
                        std::mem::swap(slot, &mut hot);
                    }
                    self.inventory.slots[d] = hot;
                }
            }
        } else if self.ui.pressed
            && self.cursor.is_none()
            && matches!(c, Container::Crafting(_))
            && self.in_station()
        {
            // At a table with nothing in hand: a click anywhere but on a slot crafts.
            self.craft_batch(c);
        } else if self.ui.pressed || self.ui.right_pressed {
            // Clicking outside the window (or away from the chest or table and the inventory)
            // throws the held stack: all of it with the left button, one with the right.
            if !inside {
                if let Some(st) = self.cursor {
                    if self.ui.right_pressed {
                        let one = Stack { count: 1, ..st };
                        take(&mut self.cursor, 1);
                        self.throw(one);
                    } else {
                        self.cursor = None;
                        self.throw(st);
                    }
                }
            }
        }

        // Stack on the mouse cursor (over a gun station's table it is shown there, in 3D).
        // (only what may lie there: anything else stays a picture on the mouse)
        let on_bench = matches!(c, Container::GunStation(_))
            && (self.bench_spot.is_some() || self.bench_drawer_spot.is_some())
            && self.cursor.is_some_and(|st| {
                let rifle = matches!(c, Container::GunStation(p) if is_rifle_bench(self.terrain.world.geti(p)));
                gun_station::belongs_on_bench(st.item, rifle)
            });
        if let (Some(st), false) = (self.cursor, on_bench) {
            let m = self.ui.mouse;
            draw_stack(&mut self.ui, m.x - 8.0 * s, m.y - 8.0 * s, 16.0 * s, &st);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_category_tab_has_items() {
        for tab in TABS.into_iter().filter(|&t| t != Tab::Inventory) {
            assert!(!creative_items(tab, "").is_empty(), "{tab:?} is empty");
        }
        // Every item is in exactly one category tab, and the listed ones are in the tab `of`
        // gives; the "all" tab has them all.
        let mut shown: Vec<ItemId> = TABS
            .iter()
            .filter(|&&t| t != Tab::All)
            .flat_map(|&tab| creative_items(tab, "").into_iter().flatten())
            .collect();
        let mut everything: Vec<ItemId> = creative_items(Tab::All, "").into_iter().flatten().collect();
        everything.sort();
        for tab in TABS {
            for id in tab.groups().concat() {
                assert_eq!(Tab::of(id), tab, "{}", key(id));
            }
        }
        shown.sort();
        let mut all = all_items();
        all.sort();
        assert_eq!(shown, all);
        assert_eq!(everything, all);
        assert_eq!(Tab::of(PISTOL), Tab::Tools);
        assert_eq!(Tab::of(FRAG_GRENADE), Tab::Tools);
        assert_eq!(Tab::of(BULLETPROOF_VEST), Tab::Armor);
        assert_eq!(Tab::of(tool_id(ToolKind::Sword, Tier::Iron)), Tab::Tools);
        assert_eq!(Tab::of(tool_id(ToolKind::Pickaxe, Tier::Iron)), Tab::Tools);
        assert_eq!(Tab::of(BURNT_MUTTON), Tab::Food);
        assert_eq!(Tab::of(SHEEP_SPAWN_EGG), Tab::Mobs);
        assert_eq!(Tab::of(DIAMOND), Tab::Materials);
        assert_eq!(Tab::of(FURNACE as ItemId), Tab::Functional);
    }
}
