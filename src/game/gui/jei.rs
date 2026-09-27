//! JEI ("Just Enough Items"): every item in a scrollable grid beside the inventory screens,
//! with a search box under it. In survival a click shows how the item is made (its crafting
//! grids, or what smelts into it and in which furnace), and a click on an ingredient there
//! shows how that is made; in creative a click takes the item, a right click shows the
//! recipe.

use super::*;

/// What the JEI panel remembers: how far it is scrolled, what is searched for (and whether
/// the search box has the keyboard), and the item whose recipe is shown (which of its ways).
#[derive(Default)]
pub(in crate::game) struct Jei {
    pub(in crate::game) focused: bool,
    search: String,
    scroll: f32,
    item: Option<ItemId>,
    page: usize,
}

/// One way to get an item.
enum Way {
    Craft([Vec<ItemId>; 9], u8),
    Smelt(ItemId, u8),
}

fn ways(item: ItemId) -> Vec<Way> {
    let mut v: Vec<Way> = recipes_for(item)
        .into_iter()
        .map(|(g, n)| Way::Craft(g, n))
        .collect();
    v.extend(smelted_from(item).into_iter().map(|(i, t)| Way::Smelt(i, t)));
    v
}

impl Game {
    /// Keyboard input while the JEI search box has it.
    pub(in crate::game) fn jei_key(&mut self, code: KeyCode, text: Option<&str>) {
        const MAX: usize = 24;
        let jei = &mut self.jei;
        match code {
            KeyCode::Escape | KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Tab => {
                jei.focused = false
            }
            KeyCode::Backspace => {
                jei.search.pop();
                jei.scroll = 0.0;
            }
            _ => {
                for c in text.unwrap_or("").chars() {
                    if !c.is_control() && jei.search.chars().count() < MAX {
                        jei.search.push(c);
                        jei.scroll = 0.0;
                    }
                }
            }
        }
    }

    /// The JEI panel at the right edge of the screen, in the room right of `left` (screen x).
    /// In creative the item under the mouse is reported like a creative slot. Returns whether
    /// the mouse is over it.
    pub(super) fn jei_panel(
        &mut self,
        left: f32,
        hovered: &mut Option<SlotRef>,
        hovered_stack: &mut Option<Stack>,
    ) -> bool {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        let cell = SLOT * s;
        let right = w - 6.0 * s;
        let cols = (((right - left - 14.0 * s) / cell).floor().max(0.0) as usize).min(9);
        if cols < 5 {
            return false;
        }
        let pw = cols as f32 * cell + 8.0 * s;
        let (x0, y0, y1) = (right - pw, 6.0 * s, h - 6.0 * s);
        self.ui.rect_full(
            x0,
            y0,
            pw,
            y1 - y0,
            rgba(12, 13, 18, 200),
            rgba(12, 13, 18, 225),
            5.0 * s,
            3.0 * s,
        );
        let over = self.ui.hit(x0, y0, pw, y1 - y0);
        let creative = self.creative();
        let fs = (s * 0.75).round().max(1.0);
        let gx = x0 + 4.0 * s;
        let mut top = y0 + 4.0 * s;
        if let Some(item) = self.jei.item {
            top = self.jei_recipe(item, gx, top, pw - 8.0 * s);
        } else {
            let hint = t(if creative { "jei.hint_creative" } else { "jei.hint_survival" });
            let lines = self.ui.wrap(hint, pw - 8.0 * s, fs);
            for line in &lines {
                self.ui
                    .text(line, gx, top + s, fs, rgba(170, 170, 178, 255), true);
                top += 9.0 * fs;
            }
            top += 3.0 * s;
        }

        // The search box along the bottom.
        let (bh, by) = (12.0 * s, y1 - 16.0 * s);
        self.jei_search_box(gx, by, pw - 8.0 * s, bh);

        // The items.
        let q = search_fold(self.jei.search.trim());
        let items: Vec<ItemId> = creative_items(Tab::All, "")
            .into_iter()
            .flatten()
            .filter(|&id| q.is_empty() || search_fold(&name(id)).contains(&q) || search_fold(&key(id)).contains(&q))
            .collect();
        let gy = top;
        let visible = (((by - 3.0 * s - gy) / cell).floor().max(0.0)) as usize;
        let rows = items.len().div_ceil(cols);
        let max_scroll = rows.saturating_sub(visible) as f32;
        if over && !self.jei.focused {
            self.jei.scroll -= self.ui.scroll;
        }
        self.jei.scroll = self.jei.scroll.clamp(0.0, max_scroll);
        let first = self.jei.scroll.round() as usize;
        for r in 0..visible {
            for c in 0..cols {
                let Some(&id) = items.get((first + r) * cols + c) else {
                    continue;
                };
                let (x, y) = (gx + c as f32 * cell, gy + r as f32 * cell);
                let st = Stack::one(id);
                if !self.draw_slot(x, y, Some(st)) {
                    continue;
                }
                if creative && !self.ui.right_pressed {
                    // Taken like from the creative grid.
                    *hovered = Some(SlotRef::Creative(id));
                    *hovered_stack = Some(st);
                } else {
                    if self.cursor.is_none() {
                        self.tooltip_for(&st);
                    }
                    if self.ui.pressed || self.ui.right_pressed {
                        self.jei.item = Some(id);
                        self.jei.page = 0;
                    }
                }
            }
        }
        // Where in the list: a thin bar along the right edge.
        if max_scroll > 0.0 && visible > 0 {
            let track = visible as f32 * cell;
            let bar = (track * visible as f32 / rows as f32).max(6.0 * s);
            let at = gy + (track - bar) * (self.jei.scroll / max_scroll);
            self.ui
                .solid(x0 + pw - 3.0 * s, at, 2.0 * s, bar, rgba(200, 200, 210, 160));
        }
        if over && self.ui.pressed && !self.ui.hit(gx, by, pw - 8.0 * s, bh) {
            self.jei.focused = false;
        }
        over
    }

    /// The search box: click to type, right click to clear.
    fn jei_search_box(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let s = self.ui.s;
        let hovered = self.ui.hit(x, y, w, h);
        if hovered && self.ui.pressed {
            self.jei.focused = true;
            self.search_focused = false;
        }
        if hovered && self.ui.right_pressed {
            self.jei.search.clear();
            self.jei.scroll = 0.0;
            self.jei.focused = true;
        }
        let border = if self.jei.focused {
            rgba(98, 214, 120, 255)
        } else if hovered {
            rgba(150, 150, 160, 255)
        } else {
            rgba(70, 70, 80, 255)
        };
        self.ui.solid(x, y, w, h, border);
        self.ui
            .solid(x + s, y + s, w - 2.0 * s, h - 2.0 * s, rgba(22, 23, 30, 255));
        let fs = (s * 0.75).round().max(1.0);
        let (tx, ty) = (x + 3.0 * s, (y + (h - 7.0 * fs) * 0.5).round());
        if self.jei.search.is_empty() && !self.jei.focused {
            self.ui
                .text(t("gui.search"), tx, ty, fs, rgba(125, 125, 132, 255), false);
        } else {
            let mut shown = self.jei.search.as_str();
            while self.ui.text_width(shown, fs) > w - 8.0 * s {
                let mut it = shown.chars();
                it.next();
                shown = it.as_str();
            }
            let tw = self.ui.text(shown, tx, ty, fs, WHITE, true);
            if self.jei.focused && (self.time * 2.5) as i32 % 2 == 0 {
                self.ui.text("_", tx + tw + fs * 0.5, ty, fs, WHITE, true);
            }
        }
    }

    /// How `item` is made, at the top of the panel from `y` (width `w`): its name, a close
    /// button, the recipe (a crafting grid or a furnace), and arrows through its ways.
    /// Returns where the list goes on below it.
    fn jei_recipe(&mut self, item: ItemId, x: f32, y: f32, w: f32) -> f32 {
        let s = self.ui.s;
        let cell = SLOT * s;
        let fs = (s * 0.75).round().max(1.0);
        // The name, and a close button.
        let mut title = name(item);
        while self.ui.text_width(&title, fs) > w - 12.0 * s && title.chars().count() > 3 {
            title.pop();
        }
        self.ui.text(&title, x, y + s, fs, rgba(255, 220, 150, 255), true);
        if self.ui.button("x", x + w - 9.0 * s, y, 9.0 * s, 9.0 * s, true) {
            self.jei.item = None;
            return y + 12.0 * s;
        }
        let ways = ways(item);
        let mut top = y + 11.0 * s;
        if ways.is_empty() {
            let lines = self.ui.wrap(t("jei.none"), w, fs);
            for line in &lines {
                self.ui
                    .text(line, x, top, fs, rgba(170, 170, 178, 255), true);
                top += 9.0 * fs;
            }
            return top + 4.0 * s;
        }
        let page = self.jei.page % ways.len();
        // Several ways: arrows to go through them.
        if ways.len() > 1 {
            let label = format!("{}/{}", page + 1, ways.len());
            let lw = self.ui.text_width(&label, fs);
            let cx = x + w * 0.5;
            if self.ui.button("<", cx - lw * 0.5 - 12.0 * s, top, 9.0 * s, 9.0 * s, true) {
                self.jei.page = (page + ways.len() - 1) % ways.len();
            }
            if self.ui.button(">", cx + lw * 0.5 + 3.0 * s, top, 9.0 * s, 9.0 * s, true) {
                self.jei.page = (page + 1) % ways.len();
            }
            self.ui
                .text_centered(&label, cx, top + 1.0 * s, fs, WHITE, true);
            top += 11.0 * s;
        }
        // Ingredients with several choices show each in turn.
        let tick = self.time as usize;
        let mut clicked = None;
        match &ways[page] {
            Way::Craft(grid, count) => {
                for (i, ids) in grid.iter().enumerate() {
                    let (cx, cy) = (x + (i % 3) as f32 * cell, top + (i / 3) as f32 * cell);
                    let content = (!ids.is_empty()).then(|| Stack::one(ids[tick % ids.len()]));
                    if self.draw_slot(cx, cy, content) {
                        if let Some(st) = content {
                            self.tooltip_for(&st);
                            if self.ui.pressed {
                                clicked = Some(st.item);
                            }
                        }
                    }
                }
                let ax = x + 3.0 * cell + 3.0 * s;
                self.arrow(ax, top + cell - 7.5 * s, 20.0, 1.0);
                let (ox, oy) = (ax + 23.0 * s, top + cell);
                if self.draw_slot(ox, oy, Some(Stack::new(item, *count))) {
                    self.tooltip_for(&Stack::one(item));
                }
                top += 3.0 * cell;
            }
            Way::Smelt(input, tier) => {
                if self.draw_slot(x, top, Some(Stack::one(*input))) {
                    self.tooltip_for(&Stack::one(*input));
                    if self.ui.pressed {
                        clicked = Some(*input);
                    }
                }
                let (furnace, label) = match tier {
                    3 => (ADV_FURNACE, "jei.advanced"),
                    2 => (BLAST_FURNACE, "jei.blast"),
                    _ => (FURNACE, "jei.furnace"),
                };
                let fx = x + cell + 2.0 * s;
                draw_stack(&mut self.ui, fx + s, top + s, 16.0 * s, &Stack::one(furnace as ItemId));
                self.arrow(fx + cell, top + 1.5 * s, 20.0, 1.0);
                let ox = fx + cell + 23.0 * s;
                if self.draw_slot(ox, top, Some(Stack::one(item))) {
                    self.tooltip_for(&Stack::one(item));
                }
                self.ui.text(
                    t(label),
                    x,
                    top + cell + 2.0 * s,
                    fs,
                    rgba(210, 210, 215, 255),
                    true,
                );
                top += cell + 11.0 * s;
            }
        }
        if let Some(id) = clicked {
            self.jei.item = Some(id);
            self.jei.page = 0;
        }
        // A line under the recipe.
        self.ui
            .solid(x, top + 3.0 * s, w, s, rgba(80, 80, 90, 200));
        top + 7.0 * s
    }
}
