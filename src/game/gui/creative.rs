//! The creative inventory: its tabs and what each lists, the search, and the hotbar under it.

use super::*;


/// Lowercase without Hungarian accents, so "gyemant" finds "Gyémánt".
pub(super) fn search_fold(s: &str) -> String {
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
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(in crate::game) enum Tab {
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

pub(in crate::game) const TABS: [Tab; 9] = [
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
pub(super) const TAB_W: f32 = 21.0;
pub(super) const TAB_H: f32 = 24.0;
pub(super) const TAB_STEP: f32 = 21.5;

impl Tab {
    pub(super) fn name(self) -> &'static str {
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

    pub(super) fn icon(self) -> ItemId {
        match self {
            Tab::Blocks => GRASS as ItemId,
            Tab::Functional => CRAFTING_TABLE as ItemId,
            Tab::Tools => PISTOL,
            Tab::Armor => STEEL_CHESTPLATE,
            Tab::All => GUIDE_BOOK,
            Tab::Food => COOKED_PORKCHOP,
            Tab::Mobs => PIG_SPAWN_EGG,
            Tab::Materials => IRON_INGOT,
            Tab::Inventory => CHEST as ItemId,
        }
    }

    /// The category tab and the group the items' lines put them in (`content::Creative`).
    fn place(c: Creative) -> Option<(Tab, u8)> {
        Some(match c {
            Creative::None => return None,
            Creative::Blocks(g) => (Tab::Blocks, g),
            Creative::Functional(g) => (Tab::Functional, g),
            Creative::Tools(g) => (Tab::Tools, g),
            Creative::Armor(g) => (Tab::Armor, g),
            Creative::Food(g) => (Tab::Food, g),
            Creative::Mobs(g) => (Tab::Mobs, g),
            Creative::Materials(g) => (Tab::Materials, g),
        })
    }

    /// The category tab an item is in (one that is not listed: `All`).
    #[cfg(test)]
    pub(super) fn of(id: ItemId) -> Tab {
        Tab::place(creative(id)).map_or(Tab::All, |(tab, _)| tab)
    }

    /// The items of the tab in the order they are shown, in groups; each group starts on a
    /// new row. In a group, in the order of `all_items` (the blocks' table, then the items'
    /// files).
    pub(super) fn groups(self) -> Vec<Vec<ItemId>> {
        let mut groups: std::collections::BTreeMap<u8, Vec<ItemId>> = Default::default();
        for id in all_items() {
            match Tab::place(creative(id)) {
                Some((tab, g)) if tab == self => groups.entry(g).or_default().push(id),
                _ => {}
            }
        }
        groups.into_values().collect()
    }
}

/// The creative grid: a tab's groups, each starting on a new row (the gaps are `None`). A
/// search looks through every item instead, by display name (current language) or by item
/// key.
pub(super) fn creative_items(tab: Tab, query: &str) -> Vec<Option<ItemId>> {
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
    let mut grid = Vec::new();
    for g in tab.groups() {
        grid.resize(grid.len().next_multiple_of(9), None);
        grid.extend(g.into_iter().map(Some));
    }
    grid
}

/// `creative_items`, remembered: the grid is drawn every frame an inventory is open, and
/// working it out goes through every item (by name, with a search). Made again for another
/// tab, search or language.
pub(in crate::game) fn creative_grid(tab: Tab, query: &str) -> std::rc::Rc<Vec<Option<ItemId>>> {
    use std::cell::RefCell;
    use std::rc::Rc;
    pub(super) type Key = (Tab, String, bool);
    thread_local! {
        static MADE: RefCell<Vec<(Key, Rc<Vec<Option<ItemId>>>)>> = const { RefCell::new(Vec::new()) };
    }
    let key = (tab, query.to_string(), crate::lang::is_hungarian());
    MADE.with_borrow_mut(|made| {
        if let Some((_, grid)) = made.iter().find(|(k, _)| *k == key) {
            return grid.clone();
        }
        let grid = Rc::new(creative_items(tab, query));
        // (the few last ones: the tab and the JEI's list are drawn in the same frame)
        if made.len() >= 8 {
            made.remove(0);
        }
        made.push((key, grid.clone()));
        grid
    })
}

impl Game {
    /// The search changed: the list starts from the top again.
    pub(super) fn scroll_creative_to_top(&mut self) {
        self.inv_ui.creative_scroll = 0.0;
        self.inv_ui.creative_scroll_anim = 0.0;
    }

    /// Keyboard input while the creative search box is focused.
    pub(in crate::game) fn search_key(&mut self, code: KeyCode, text: Option<&str>) {
        use crate::ui::{edit_line, LineEdit};
        match edit_line(&mut self.inv_ui.creative_search, code, text, 24) {
            LineEdit::Escape => self.close_container(),
            LineEdit::Done => self.inv_ui.search_focused = false,
            LineEdit::Changed => self.scroll_creative_to_top(),
            LineEdit::None => {}
        }
    }

    /// Search box in the top right corner of the creative inventory, right-aligned with the
    /// scroll bar and starting after the title (`title_end`, screen x). Click to type,
    /// right-click to clear.
    pub(super) fn search_box(&mut self, px: f32, py: f32, title_end: f32) {
        let s = self.ui.s;
        let x = (px + 95.0 * s).max((title_end + 4.0 * s).round());
        let (y, w, h) = (py + 4.0 * s, px + 187.0 * s - x, 11.0 * s);
        let hovered = self.ui.hit(x, y, w, h);
        if self.ui.pressed {
            self.inv_ui.search_focused = hovered;
        }
        if hovered && self.ui.right_pressed {
            self.inv_ui.creative_search.clear();
            self.scroll_creative_to_top();
            self.inv_ui.search_focused = true;
        }
        let th = self.theme();
        let border = if self.inv_ui.search_focused {
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
        if self.inv_ui.creative_search.is_empty() && !self.inv_ui.search_focused {
            self.ui
                .text(t("gui.search"), tx, ty, fs, rgba(125, 125, 132, 255), false);
        } else {
            // Show the end of a long query.
            let mut shown = self.inv_ui.creative_search.as_str();
            while self.ui.text_width(shown, fs) > room - 2.0 * fs {
                let mut it = shown.chars();
                it.next();
                shown = it.as_str();
            }
            let tw = self.ui.text(shown, tx, ty, fs, WHITE, true);
            if self.inv_ui.search_focused && (self.time * 2.5) as i32 % 2 == 0 {
                self.ui.text("_", tx + tw + fs * 0.5, ty, fs, WHITE, true);
            }
        }
    }

    /// The creative tabs above the window at (px, py): an icon for each category and one for
    /// the player's own inventory. A click opens a tab (and clears the search). Returns
    /// whether the mouse is over them.
    pub(super) fn creative_tabs(&mut self, px: f32, py: f32, panel_w: f32) -> bool {
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
            let open = i == self.inv_ui.creative_tab;
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
                    self.inv_ui.creative_tab = i;
                    self.inv_ui.creative_search.clear();
                    self.inv_ui.search_focused = false;
                    self.inv_ui.scroll_drag = false;
                    self.scroll_creative_to_top();
                }
            }
        }
        over
    }

    /// The hotbar along the bottom of every creative tab, with the slot that destroys items.
    pub(super) fn creative_hotbar(&mut self, px: f32, py: f32, hovered: &mut Option<SlotRef>) {
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
}
