//! Item screens: the inventory and the creative inventory, and the slot handling shared
//! with the chest and crafting table views (`station`).
//! Slot interaction follows Minecraft: left click picks up / places / swaps, right click
//! splits / places one, shift-click moves between sections, 1-9 swaps with the hotbar,
//! dragging a held stack spreads it over slots and double-click collects matching items.
//! A stack can also be dragged out of a slot and let go over another one.

mod gun_station;
pub(super) use gun_station::BenchUi;
mod jei;
pub(super) use jei::Jei;
pub(super) mod hud;
pub(crate) mod icons;
pub(super) mod station;
mod craft_menu;
mod creative;
mod draw;
mod preview;
mod slots;

// (their items are used across the item screens as if they were here)
use creative::*;
use slots::*;
pub(super) use creative::{creative_grid, Tab, TABS};
pub(super) use draw::draw_stack;
pub(super) use slots::Drag;

use crate::client::{Container, Game};
use crate::item::*;
use crate::item::inventory::take;
use crate::app::lang::t;
use crate::ui::rgba;
use crate::world::is_rifle_bench;
use glam::Vec3;

/// The inventory screens: the creative tabs, list and search, the JEI panel, dragging and
/// clicking slots, and what the mouse is on at an open chest or table.
pub(super) struct InventoryUi {
    /// Creative list scroll in rows: the target set by the wheel, and the eased position.
    creative_scroll: f32,
    creative_scroll_anim: f32,
    /// Dragging the creative scroll bar.
    scroll_drag: bool,
    /// Creative inventory search text; typing goes to it while it is focused.
    creative_search: String,
    pub(super) search_focused: bool,
    /// The JEI panel beside the inventory screens.
    pub(super) jei: Jei,
    /// The open tab of the creative inventory (index into `TABS`); kept between openings.
    creative_tab: usize,
    /// Slot drag in progress (Minecraft-style stack spreading).
    pub(super) drag: Option<Drag>,
    /// The slot a stack was just picked up from with the button still held: letting go
    /// over another slot puts it there.
    press_pick: Option<SlotRef>,
    /// Time and slot of the last left click, for double-click collecting.
    slot_click: (f32, Option<SlotRef>),
    /// What the mouse points at in the open chest or on the open table, and the corners of
    /// its highlighted slot.
    pub(super) station_hover: Option<SlotRef>,
    pub(super) station_frame: Option<[Vec3; 4]>,
    /// The mouse is over the open chest or table, or the inventory under it: a click there
    /// does not throw the held stack.
    station_inside: bool,
    /// The inventory's crafting tab.
    craft: craft_menu::CraftMenu,
}

impl InventoryUi {
    pub(super) fn new() -> Self {
        Self {
            creative_scroll: 0.0,
            creative_scroll_anim: 0.0,
            scroll_drag: false,
            creative_search: String::new(),
            search_focused: false,
            jei: Default::default(),
            creative_tab: 0,
            drag: None,
            press_pick: None,
            slot_click: (-1.0, None),
            station_hover: None,
            station_frame: None,
            station_inside: false,
            craft: Default::default(),
        }
    }

    /// Out of a world: what the mouse was doing at its screens goes (the creative tab, list
    /// and search, and the JEI panel stay as they were).
    pub(super) fn forget_world(&mut self) {
        *self = Self {
            creative_scroll: self.creative_scroll,
            creative_scroll_anim: self.creative_scroll_anim,
            creative_search: std::mem::take(&mut self.creative_search),
            jei: std::mem::take(&mut self.jei),
            creative_tab: self.creative_tab,
            ..Self::new()
        };
    }
}

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

impl Game {

    /// Draws the open item screen and handles clicks.
    pub(super) fn container_screen(&mut self, c: Container) {
        if let Container::GunStation(p) = c {
            let hovered = self.gun_station_screen(p);
            let inside = self.inv_ui.station_inside;
            self.slot_input(c, hovered, None, inside);
            return;
        }
        if matches!(c, Container::Chest(_) | Container::Crafting(_)) {
            let mut hovered = self.station_screen(c);
            // JEI beside the inventory strip, most useful at a crafting table.
            let mut hovered_stack = None;
            let strip_right = (self.ui.w + 176.0 * self.ui.s) * 0.5;
            let over_jei = self.jei_panel(strip_right, &mut hovered, &mut hovered_stack);
            let inside = self.inv_ui.station_inside || over_jei;
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
            self.panel_below(panel_w, panel_h, craft_menu::INV_TAB_H + 3.0)
        };
        let over_tabs = if c == Container::Creative {
            self.creative_tabs(px, py, panel_w)
        } else {
            self.inventory_tabs(px, py)
        };
        let crafting_tab = c == Container::Inventory && self.inv_ui.craft.open;
        if matches!(c, Container::Inventory | Container::Creative) {
            self.draw_effects_list(px, py, panel_w * s);
        }
        let at = |gx: f32, gy: f32| (px + gx * s, py + gy * s);

        match c {
            Container::Chest(_) | Container::Crafting(_) | Container::GunStation(_) => {}
            Container::Inventory if crafting_tab => self.craft_tab(px, py),
            Container::Inventory => {
                // The figure with what is worn down its left and the vest beside it.
                let (ax, ay) = at(26.0, 8.0);
                self.player_preview(ax, ay, 51.0 * s, 70.0 * s);
                let spots: [(f32, f32); ARMOR_SLOTS] = std::array::from_fn(|i| {
                    if i == VEST_SLOT {
                        at(77.0, 60.0)
                    } else {
                        at(7.0, 8.0 + i as f32 * SLOT)
                    }
                });
                self.armor_slots(spots, &mut hovered);
                self.inventory_slots(px, py, 84.0, &mut hovered);
            }
            Container::Creative if TABS[self.inv_ui.creative_tab] == Tab::Inventory => {
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
                    if self.draw_slot(x, y, self.me.items.inventory.slots[i]) {
                        hovered = Some(SlotRef::Inv(i));
                    }
                }
                self.creative_hotbar(px, py, &mut hovered);
            }
            Container::Creative => {
                let tab = TABS[self.inv_ui.creative_tab];
                let title = if self.inv_ui.creative_search.trim().is_empty() {
                    tab.name()
                } else {
                    t("gui.tab.search")
                };
                let (tx, ty) = at(8.0, 6.0);
                self.label(title, tx, ty);
                let fs = (s * 0.75).round().max(1.0);
                let title_end = tx + self.ui.text_width(title, fs);
                self.search_box(px, py, title_end);
                let all = creative_grid(tab, &self.inv_ui.creative_search);
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
                    self.inv_ui.scroll_drag = true;
                }
                if !self.input.left_down {
                    self.inv_ui.scroll_drag = false;
                }
                if self.inv_ui.scroll_drag && max_scroll > 0.0 {
                    let k =
                        ((self.ui.mouse.y - sy - bar_h * 0.5) / (track_h - bar_h)).clamp(0.0, 1.0);
                    self.inv_ui.creative_scroll = k * max_scroll;
                    self.inv_ui.creative_scroll_anim = self.inv_ui.creative_scroll;
                }
                // The wheel moves the target one row per notch; the list glides there.
                self.inv_ui.creative_scroll =
                    (self.inv_ui.creative_scroll - self.ui.scroll).clamp(0.0, max_scroll);
                let ease = crate::util::damp(18.0, self.ui.dt);
                self.inv_ui.creative_scroll_anim +=
                    (self.inv_ui.creative_scroll - self.inv_ui.creative_scroll_anim) * ease;
                if (self.inv_ui.creative_scroll - self.inv_ui.creative_scroll_anim).abs() < 0.002 {
                    self.inv_ui.creative_scroll_anim = self.inv_ui.creative_scroll;
                }
                let scroll = self.inv_ui.creative_scroll_anim.clamp(0.0, max_scroll);
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
        let jei = (matches!(c, Container::Inventory) && !crafting_tab)
            || (c == Container::Creative && TABS[self.inv_ui.creative_tab] == Tab::Inventory);
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
        if !self.container_ready(c) {
            // (the server's copy of the chest or table is still on its way)
            return;
        }
        let s = self.ui.s;
        // A stack dragged out of a slot and let go over another slot goes there.
        if let Some(from) = self.inv_ui.press_pick {
            if !self.input.left_down {
                self.inv_ui.press_pick = None;
                if let Some(r) = hovered.filter(|&r| r != from && droppable(r)) {
                    if self.me.items.cursor.is_some() {
                        self.click_slot(c, r, false, false);
                    }
                }
            }
        }

        // Tooltip for the hovered stack.
        if self.me.items.cursor.is_none() {
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
        if let Some(mut d) = self.inv_ui.drag.take() {
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
                self.input.right_down
            } else {
                self.input.left_down
            };
            if held {
                self.inv_ui.drag = Some(d);
            }
        } else if let Some(r) = hovered {
            // Middle click (creative): a full stack of the hovered item on the cursor.
            let middle = self.input.middle_pressed && !self.input.cursor_grabbed;
            if middle && self.creative() && self.me.items.cursor.is_none() {
                let st = match r {
                    SlotRef::Creative(id) => Some(Stack::one(id)),
                    SlotRef::CraftOut => self.craft_out_stack(c),
                    SlotRef::Trash => None,
                    _ => self.slot_mut(c, r).and_then(|s| *s),
                };
                self.me.items.cursor = st.map(|st| Stack::new(st.item, max_stack(st.item)));
            }
            if self.ui.pressed || self.ui.right_pressed {
                let right = !self.ui.pressed;
                let double = !right
                    && !shift
                    && self.inv_ui.slot_click.1 == Some(r)
                    && self.clock.time - self.inv_ui.slot_click.0 < 0.3;
                if !right {
                    self.inv_ui.slot_click = (self.clock.time, Some(r));
                }
                let cur = self.slot_mut(c, r).and_then(|s| *s);
                match self.me.items.cursor {
                    Some(_) if double && droppable(r) => self.collect_all(c),
                    Some(st) if !shift && droppable(r) && fits(cur, &st) => {
                        let d = Drag {
                            right,
                            stack: st,
                            slots: vec![(r, cur)],
                        };
                        self.apply_drag(c, &d);
                        self.inv_ui.drag = Some(d);
                    }
                    _ => {
                        let empty = self.me.items.cursor.is_none();
                        self.click_slot(c, r, right, shift && !right);
                        if empty && !right && !shift && self.me.items.cursor.is_some() {
                            self.inv_ui.press_pick = Some(r);
                        }
                    }
                }
            }
            if let Some(d) = self.input.digit {
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
                    let mut hot = self.me.items.inventory.slots[d].take();
                    if let Some(slot) = self.slot_mut(c, r) {
                        std::mem::swap(slot, &mut hot);
                    }
                    self.me.items.inventory.slots[d] = hot;
                }
            }
        } else if self.ui.pressed
            && self.me.items.cursor.is_none()
            && matches!(c, Container::Crafting(_))
            && self.in_station()
        {
            // At a table with nothing in hand: a click anywhere but on a slot crafts.
            self.craft_batch(c);
        } else if self.ui.pressed || self.ui.right_pressed {
            // Clicking outside the window (or away from the chest or table and the inventory)
            // throws the held stack: all of it with the left button, one with the right.
            if !inside {
                if let Some(st) = self.me.items.cursor {
                    if self.ui.right_pressed {
                        let one = Stack { count: 1, ..st };
                        take(&mut self.me.items.cursor, 1);
                        self.throw(one);
                    } else {
                        self.me.items.cursor = None;
                        self.throw(st);
                    }
                }
            }
        }

        // Stack on the mouse cursor (over a gun station's table it is shown there, in 3D).
        // (only what may lie there: anything else stays a picture on the mouse)
        let on_bench = matches!(c, Container::GunStation(_))
            && (self.bench_ui.spot.is_some() || self.bench_ui.drawer_spot.is_some())
            && self.me.items.cursor.is_some_and(|st| {
                let rifle = matches!(c, Container::GunStation(p) if is_rifle_bench(self.terrain.world.geti(p)));
                belongs_on_bench(st.item, rifle)
            });
        if let (Some(st), false) = (self.me.items.cursor, on_bench) {
            let m = self.ui.mouse;
            draw_stack(&mut self.ui, m.x - 8.0 * s, m.y - 8.0 * s, 16.0 * s, &st);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::FURNACE;

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
