//! The inventory's crafting tab: what can be made down the left (what the inventory holds
//! enough for first), the chosen one's ingredients on the right, and the button that makes it.
//! Making takes a moment (a bar fills on the button); shift-click queues as many as the
//! inventory holds enough for. Recipes bigger than 2x2 need a crafting table nearby.

use super::*;
use crate::item::{craft_have, craft_list, craft_pay, ListedRecipe};
use crate::app::lang::tf;
use crate::ui::{lerp_color, with_alpha, WHITE};
use crate::world::CRAFTING_TABLE;
use glam::IVec3;

/// The list: columns and rows of recipe slots seen at once.
const COLS: usize = 4;
const ROWS: usize = 8;
/// How far (in blocks) a crafting table counts as nearby.
const TABLE_REACH: i32 = 4;

/// The open tab and what is being made.
#[derive(Default)]
pub(super) struct CraftMenu {
    /// The crafting tab is open (else the inventory one).
    pub(super) open: bool,
    /// The chosen result.
    selected: Option<ItemId>,
    /// The list's scroll, in rows.
    scroll: f32,
    pub(super) job: Option<Job>,
}

/// Something being made: which recipe, how many more after this one, and how far along.
pub(super) struct Job {
    recipe: usize,
    left: u32,
    time: f32,
}

/// One line of the list: a result, the recipe it is made with here, whether that can be made
/// now and whether a table is missing for it.
struct Entry {
    recipe: usize,
    ready: bool,
    no_table: bool,
}

/// Seconds making one takes: a little more for more ingredients.
fn craft_time(r: &ListedRecipe) -> f32 {
    let n: u32 = r.needs.iter().map(|(_, n)| *n as u32).sum();
    (0.35 + 0.1 * n as f32).min(1.4)
}

impl Game {
    /// A crafting table within reach of the player.
    fn near_crafting_table(&self) -> bool {
        let at = self.me.body.pos.floor().as_ivec3();
        let r = TABLE_REACH;
        (-r..=r).any(|x| {
            (-2..=3).any(|y| (-r..=r).any(|z| self.terrain.world.geti(at + IVec3::new(x, y, z)) == CRAFTING_TABLE))
        })
    }

    /// Each result once: the first of its recipes that can be made now (else its first), the
    /// ones that can be made first.
    fn craft_entries(&self, table: bool) -> Vec<Entry> {
        let list = craft_list();
        let slots = self.me.items.inventory.slots;
        let mut seen: Vec<ItemId> = Vec::new();
        let mut out = Vec::new();
        for (i, r) in list.iter().enumerate() {
            if seen.contains(&r.result.item) {
                continue;
            }
            seen.push(r.result.item);
            let ways = (i..list.len()).filter(|&j| list[j].result.item == r.result.item);
            let can = |j: usize| (table || !list[j].table) && craft_pay(&mut slots.clone(), &list[j].needs);
            let entry = match ways.clone().find(|&j| can(j)) {
                Some(j) => Entry { recipe: j, ready: true, no_table: false },
                None => {
                    // (one whose ingredients are there but whose table is not says so)
                    let blocked = ways.clone().find(|&j| craft_pay(&mut slots.clone(), &list[j].needs));
                    let j = blocked.unwrap_or(i);
                    Entry { recipe: j, ready: false, no_table: !table && list[j].table }
                }
            };
            out.push(entry);
        }
        out.sort_by_key(|e| !e.ready);
        out
    }

    /// How many times the inventory can pay for `needs` (up to 64).
    fn craftable_count(&self, needs: &[(Vec<ItemId>, u8)]) -> u32 {
        let mut slots = self.me.items.inventory.slots;
        let mut n = 0;
        while n < 64 && craft_pay(&mut slots, needs) {
            n += 1;
        }
        n
    }

    /// Goes on making what is queued: each one done takes its ingredients and puts what it
    /// makes into the inventory. Runs while the inventory is open.
    pub(in crate::client) fn update_craft_job(&mut self, dt: f32) {
        let Some(job) = &mut self.inv_ui.craft.job else { return };
        let r = &craft_list()[job.recipe];
        job.time += dt;
        if job.time < craft_time(r) {
            return;
        }
        job.time = 0.0;
        job.left = job.left.saturating_sub(1);
        let done = job.left == 0;
        if craft_pay(&mut self.me.items.inventory.slots, &r.needs) {
            self.give(r.result);
            self.me.hand.swing();
            self.audio.play(crate::audio::Sound::GearClick, None, 0.5);
            if done || !craft_pay(&mut self.me.items.inventory.slots.clone(), &r.needs) {
                self.inv_ui.craft.job = None;
            }
        } else {
            self.inv_ui.craft.job = None;
        }
    }

    /// The tabs over the inventory window ("Inventory", "Crafting"); true when the mouse is on
    /// them.
    pub(super) fn inventory_tabs(&mut self, px: f32, py: f32) -> bool {
        let s = self.ui.s;
        let th = self.theme();
        let fs = (s * 0.75).round().max(1.0);
        let mut over = false;
        let mut x = px;
        for (i, label) in [t("gui.inventory"), t("gui.crafting")].into_iter().enumerate() {
            let open = (i == 1) == self.inv_ui.craft.open;
            let w = (self.ui.text_width(label, fs) + 14.0 * s).round();
            let h = INV_TAB_H * s;
            let y = py - h - if open { 2.0 * s } else { 0.0 };
            let hovered = self.ui.hit(x, y, w, py - y);
            over |= hovered;
            self.ui.rect(x - s, y - s, w + 2.0 * s, py - y + 3.0 * s, th.border, 3.0 * s);
            self.ui.rect(x, y, w, py - y + 2.0 * s, if open { th.bevel_hi } else { th.border }, 2.0 * s);
            let fill = if open {
                th.fill_top
            } else if hovered {
                th.slot_light
            } else {
                th.tab_idle
            };
            self.ui.rect(x + s, y + s, w - 2.0 * s, py - y + if open { 2.0 * s } else { -s }, fill, 2.0 * s);
            let ty = (y + (h - 7.0 * fs) * 0.5 + if open { s } else { 0.0 }).round();
            self.label(label, (x + 7.0 * s).round(), ty);
            if hovered && self.ui.pressed && !open {
                self.inv_ui.craft.open = i == 1;
            }
            x += w + 3.0 * s;
        }
        over
    }

    /// The crafting tab inside the inventory window at (px, py).
    pub(super) fn craft_tab(&mut self, px: f32, py: f32) {
        let s = self.ui.s;
        let at = |gx: f32, gy: f32| (px + gx * s, py + gy * s);
        let table = self.near_crafting_table();
        let entries = self.craft_entries(table);
        let list = craft_list();
        let th = self.theme();
        let fs = (s * 0.75).round().max(1.0);

        let (tx, ty) = at(8.0, 6.0);
        self.label(t("craft.what"), tx, ty);

        // The list, scrolled by the wheel over it.
        let (gx, gy) = at(7.0, 17.0);
        let (lw, lh) = (COLS as f32 * SLOT * s, ROWS as f32 * SLOT * s);
        let rows = entries.len().div_ceil(COLS);
        let max_scroll = rows.saturating_sub(ROWS) as f32;
        if self.ui.hit(gx, gy, lw, lh) {
            self.inv_ui.craft.scroll = (self.inv_ui.craft.scroll - self.ui.scroll).clamp(0.0, max_scroll);
        }
        let first = self.inv_ui.craft.scroll.round() as usize;
        if self.inv_ui.craft.selected.is_none_or(|sel| !entries.iter().any(|e| list[e.recipe].result.item == sel)) {
            self.inv_ui.craft.selected = entries.first().map(|e| list[e.recipe].result.item);
        }
        let mut hovered_stack = None;
        for (k, e) in entries.iter().enumerate().skip(first * COLS).take(ROWS * COLS) {
            let k = k - first * COLS;
            let (x, y) = (gx + (k % COLS) as f32 * SLOT * s, gy + (k / COLS) as f32 * SLOT * s);
            let res = list[e.recipe].result;
            let chosen = self.inv_ui.craft.selected == Some(res.item);
            // (what cannot be made yet is faint)
            let hovered = if e.ready {
                self.draw_slot(x, y, Some(res))
            } else {
                let hovered = self.draw_slot(x, y, None);
                let faint = self.ui.style(0.3, glam::Vec2::ZERO);
                draw_stack(&mut self.ui, x + s, y + s, 16.0 * s, &res);
                self.ui.restore(faint);
                hovered
            };
            if hovered {
                hovered_stack = Some(res);
                if self.ui.pressed {
                    self.inv_ui.craft.selected = Some(res.item);
                }
            }
            if chosen {
                frame(&mut self.ui, x, y, SLOT * s, s, th.progress);
            }
        }
        // The scroll bar.
        let (sx, sy) = at(80.0, 17.0);
        self.ui.solid(sx, sy, 5.0 * s, lh, th.scroll_track);
        if max_scroll > 0.0 {
            let bar_h = (lh * ROWS as f32 / rows as f32).max(12.0 * s);
            let k = self.inv_ui.craft.scroll / max_scroll;
            self.ui.solid(sx + s, (sy + k * (lh - bar_h)).round(), 3.0 * s, bar_h, th.scroll_thumb);
        }
        let (dx, dy) = at(89.0, 8.0);
        self.ui.solid(dx, dy, s, 150.0 * s, th.bevel_lo);

        // The chosen one.
        let Some(e) = self.inv_ui.craft.selected.and_then(|sel| entries.iter().find(|e| list[e.recipe].result.item == sel))
        else {
            if let Some(st) = hovered_stack {
                self.tooltip_for(&st);
            }
            return;
        };
        let r = &list[e.recipe];
        let cx = px + 132.0 * s;
        let (bx, by) = (cx - 13.0 * s, py + 15.0 * s);
        if self.draw_slot_sized(bx, by, 26.0, Some(r.result)) {
            hovered_stack = Some(r.result);
        }
        let name = name(r.result.item);
        for (i, line) in self.ui.wrap(&name, 74.0 * s, fs).iter().take(2).enumerate() {
            self.ui.text_centered(line, cx, py + (44.0 + i as f32 * 8.0) * s, fs, th.label, th.label_shadow);
        }
        let (lx, ly) = at(94.0, 63.0);
        self.label(t("craft.needs"), lx, ly);

        // The ingredients, two to a row: what will do (the one held, or each in turn) and
        // how many are there of how many needed.
        let slots = self.me.items.inventory.slots;
        for (i, (items, n)) in r.needs.iter().enumerate().take(6) {
            let (x, y) = at(94.0 + (i % 2) as f32 * 38.0, 73.0 + (i / 2) as f32 * 20.0);
            let have = craft_have(&slots, items);
            let shown = items
                .iter()
                .copied()
                .find(|&it| craft_have(&slots, &[it]) > 0)
                .unwrap_or(items[(self.clock.time * 0.8) as usize % items.len()]);
            if self.draw_slot(x, y, Some(Stack::one(shown))) {
                hovered_stack = Some(Stack::one(shown));
            }
            let c = if have >= *n as u32 { rgba(110, 220, 120, 255) } else { rgba(236, 96, 96, 255) };
            let text = format!("{}/{}", have.min(999), n);
            self.ui.text(&text, x + 19.0 * s, y + 6.0 * s, fs, c, th.label_shadow);
        }

        // The button, or why it cannot be made here.
        let (bx, by, bw, bh) = (px + 94.0 * s, py + 136.0 * s, 75.0 * s, 20.0 * s);
        if e.no_table {
            for (i, line) in self.ui.wrap(t("craft.need_table"), bw, fs).iter().take(3).enumerate() {
                self.ui.text_centered(line, cx, by + (1.0 + i as f32 * 8.0) * s, fs, rgba(236, 120, 96, 255), th.label_shadow);
            }
        } else {
            let can = self.craftable_count(&r.needs);
            let job = self.inv_ui.craft.job.as_ref().filter(|j| j.recipe == e.recipe);
            let progress = job.map(|j| j.time / craft_time(r));
            let queued = job.map_or(0, |j| j.left);
            let enabled = e.ready || job.is_some();
            let hovered = self.ui.hit(bx, by, bw, bh);
            self.ui.rect(bx - s, by - s, bw + 2.0 * s, bh + 2.0 * s, th.border, 3.0 * s);
            let fill = if !enabled {
                th.idle
            } else if hovered {
                lerp_color(th.slot_light, th.progress, 0.35)
            } else {
                th.slot_light
            };
            self.ui.rect(bx, by, bw, bh, fill, 2.0 * s);
            if let Some(k) = progress {
                self.ui.rect(bx, by, (bw * k.clamp(0.0, 1.0)).max(2.0 * s), bh, with_alpha(th.progress, 0.85), 2.0 * s);
            }
            let label = if queued > 1 {
                format!("{} ({queued})", t("craft.make"))
            } else {
                t("craft.make").to_string()
            };
            let tw = self.ui.text_width(&label, s);
            let tc = if enabled { WHITE } else { rgba(150, 150, 158, 255) };
            self.ui.text(&label, (bx + (bw - tw) * 0.5).round(), (by + (bh - 7.0 * s) * 0.5).round(), s, tc, true);
            if hovered && enabled {
                self.ui.set_tooltip(&tf("craft.can", &[&can]));
            }
            if hovered && self.ui.pressed && can > 0 {
                let more = if self.ui.shift { can } else { 1 };
                match &mut self.inv_ui.craft.job {
                    Some(j) if j.recipe == e.recipe => j.left = (j.left + more).min(can.max(j.left)),
                    job => *job = Some(Job { recipe: e.recipe, left: more, time: 0.0 }),
                }
            }
        }
        if let Some(st) = hovered_stack {
            self.tooltip_for(&st);
        }
    }
}

/// A frame `t` thick around a square slot.
fn frame(ui: &mut crate::ui::Ui, x: f32, y: f32, size: f32, t: f32, c: crate::ui::Color) {
    ui.solid(x, y, size, t, c);
    ui.solid(x, y + size - t, size, t, c);
    ui.solid(x, y + t, t, size - 2.0 * t, c);
    ui.solid(x + size - t, y + t, t, size - 2.0 * t, c);
}

/// GUI pixel height of the inventory's tabs.
pub(super) const INV_TAB_H: f32 = 14.0;
