//! Item screens: inventory, crafting table, furnace, chest and creative inventory.
//! Slot interaction follows Minecraft: left click picks up / places / swaps, right click
//! splits / places one, shift-click moves between sections, 1-9 swaps with the hotbar,
//! dragging a held stack spreads it over slots and double-click collects matching items.

use super::*;
use crate::entity::SMELT_TIME;
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
    Furnace(usize),
    Chest(usize),
    Creative(ItemId),
    Trash,
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
    matches!(
        r,
        SlotRef::Inv(_) | SlotRef::Craft(_) | SlotRef::Chest(_) | SlotRef::Furnace(0 | 1)
    )
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

/// Creative items matching a search: by display name (current language) or by item key.
fn creative_items(query: &str) -> Vec<ItemId> {
    let q = search_fold(query.trim());
    all_items()
        .into_iter()
        .filter(|&id| {
            q.is_empty()
                || search_fold(&name(id)).contains(&q)
                || search_fold(&key(id)).contains(&q)
        })
        .collect()
}

/// Draws an item icon with its stack count and durability bar. `size` is the icon size in pixels.
pub fn draw_stack(ui: &mut Ui, x: f32, y: f32, size: f32, st: &Stack) {
    let c = Vec2::new(x + size * 0.5, y + size * 0.5);
    match icon(st.item) {
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
    if max > 0 && st.damage > 0 {
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
        self.search_focused = false;
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
        self.search_focused = false;
        if let Screen::Container(c) = self.screen {
            if Self::container_pos(c).is_some() {
                // LAN player: the last changes go to the host before closing.
                self.net_container_sync();
                self.net_container_closed();
            }
        }
        self.stash_table(true);
        let mut back: Vec<Stack> = self.craft.iter_mut().filter_map(|s| s.take()).collect();
        back.extend(self.cursor.take());
        for s in back {
            self.give(s);
        }
        self.resume();
    }

    /// The block a container screen belongs to.
    pub(super) fn container_pos(c: Container) -> Option<IVec3> {
        match c {
            Container::Crafting(p) | Container::Furnace(p) | Container::Chest(p) => Some(p),
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
            SlotRef::Furnace(i) => match c {
                Container::Furnace(p) => {
                    self.block_entities.furnaces.get_mut(&p).map(|f| match i {
                        0 => &mut f.input,
                        1 => &mut f.fuel,
                        _ => &mut f.output,
                    })
                }
                _ => None,
            },
            SlotRef::Chest(i) => match c {
                Container::Chest(p) => {
                    let (a, b) = self.chest_halves(p);
                    let (q, i) = if i < 27 { (a, i) } else { (b?, i - 27) };
                    self.block_entities.chests.get_mut(&q).map(|ch| &mut ch[i])
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn craft_result(&self, c: Container) -> Option<Stack> {
        let n = Self::craft_size(c);
        craft(&self.craft[..n * n], n)
    }

    fn consume_craft_inputs(&mut self, c: Container) {
        let n = Self::craft_size(c);
        for s in &mut self.craft[..n * n] {
            take(s, 1);
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
            (Container::Furnace(p), SlotRef::Inv(i)) => {
                let f = self.block_entities.furnaces.get_mut(&p);
                match f {
                    Some(f) if smelt(stack.item).is_some() => {
                        add_to(std::slice::from_mut(&mut f.input), stack)
                    }
                    Some(f) if fuel_time(stack.item).is_some() => {
                        add_to(std::slice::from_mut(&mut f.fuel), stack)
                    }
                    _ => self.move_within_inventory(i, stack),
                }
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
            SlotRef::Furnace(2) => {
                // Output: take only.
                let cursor = &mut self.cursor;
                let Some(out) = (match c {
                    Container::Furnace(p) => self
                        .block_entities
                        .furnaces
                        .get_mut(&p)
                        .map(|f| &mut f.output),
                    _ => None,
                }) else {
                    return;
                };
                let Some(st) = *out else { return };
                if shift {
                    *out = None;
                    self.give(st);
                    return;
                }
                match cursor {
                    None => *cursor = out.take(),
                    Some(cur) if cur.stacks_with(&st) => {
                        let n = (max_stack(st.item) - cur.count).min(st.count);
                        cur.count += n;
                        take(out, n);
                    }
                    _ => {}
                }
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
        if max > 0 && st.damage > 0 {
            text = format!(
                "{text}  ({})",
                tf("gui.durability", &[&(max - st.damage), &max])
            );
        }
        self.ui.set_tooltip(&text);
    }

    /// Main inventory (3 rows) and hotbar at GUI offset (ox, oy) = top-left of the main rows.
    fn inventory_slots(&mut self, px: f32, py: f32, oy: f32, hovered: &mut Option<SlotRef>) {
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

    fn panel(&mut self, pw: f32, ph: f32) -> (f32, f32) {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        self.ui
            .gradient(0.0, 0.0, w, h, rgba(0, 0, 0, 150), rgba(0, 0, 0, 110));
        let (px, py) = (((w - pw * s) * 0.5).round(), ((h - ph * s) * 0.5).round());
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

    /// Furnace flame (14x14); the top part burns away as the fuel runs out.
    fn flame(&mut self, x: f32, y: f32, k: f32) {
        const MASK: [&str; 14] = [
            "......#.......",
            "......##......",
            ".....###......",
            ".....####.....",
            "....#####.....",
            "....######....",
            "...#######.#..",
            "...#########..",
            "..###########.",
            "..###########.",
            ".#############",
            ".#############",
            "..###########.",
            "...#########..",
        ];
        let s = self.ui.s;
        let lit_from = (14.0 * (1.0 - k.clamp(0.0, 1.0))).round() as usize;
        for (row, line) in MASK.iter().enumerate() {
            let lit = k > 0.0 && row >= lit_from;
            let t = row as f32 / 13.0;
            let color = if lit {
                [1.0, 0.95 - 0.5 * t, 0.35 - 0.3 * t, 1.0]
            } else {
                self.theme().idle
            };
            for (col, ch) in line.bytes().enumerate() {
                if ch == b'#' {
                    self.ui
                        .solid(x + col as f32 * s, y + row as f32 * s, s, s, color);
                }
            }
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
        // (min, max, textures, rotation, pivot) in model pixels, feet at y = 0, facing +z.
        let mut parts = vec![
            (
                Vec3::new(-4.0, 0.0, -2.0),
                Vec3::new(0.0, 12.0, 2.0),
                LEG,
                body,
                Vec3::ZERO,
            ),
            (
                Vec3::new(0.0, 0.0, -2.0),
                Vec3::new(4.0, 12.0, 2.0),
                LEG,
                body,
                Vec3::ZERO,
            ),
            (
                Vec3::new(-4.0, 12.0, -2.0),
                Vec3::new(4.0, 24.0, 2.0),
                BODY,
                body,
                Vec3::ZERO,
            ),
            (
                Vec3::new(-8.0, 12.0, -2.0),
                Vec3::new(-4.0, 24.0, 2.0),
                ARM,
                body,
                Vec3::ZERO,
            ),
            (
                Vec3::new(4.0, 12.0, -2.0),
                Vec3::new(8.0, 24.0, 2.0),
                ARM,
                body,
                Vec3::ZERO,
            ),
        ];
        // Painter's order: farthest first, head always last (it sits on top of everything).
        parts.sort_by(|a, b| {
            let za = (a.3 * ((a.0 + a.1) * 0.5)).z;
            let zb = (b.3 * ((b.0 + b.1) * 0.5)).z;
            za.total_cmp(&zb)
        });
        parts.push((
            Vec3::new(-4.0, 24.0, -4.0),
            Vec3::new(4.0, 32.0, 4.0),
            HEAD,
            head,
            neck,
        ));
        let light = Vec3::new(0.35, 0.55, 0.76).normalize();
        for (lo, hi, tex, rot, pivot) in parts {
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
                let n = rot * n;
                if n.z <= 0.01 {
                    continue;
                }
                let shade = 0.55 + 0.45 * n.dot(light).max(0.0);
                let p = corners.map(|c| {
                    let q = rot * (c - pivot) + pivot;
                    Vec2::new(cx + q.x * u, feet - q.y * u)
                });
                self.ui.tex_quad(p, tex[ti], shade);
            }
        }
    }

    /// Draws the open item screen and handles clicks.
    pub(super) fn container_screen(&mut self, c: Container) {
        let s = self.ui.s;
        let mut hovered: Option<SlotRef> = None;
        let mut hovered_stack: Option<Stack> = None;
        let (panel_w, panel_h) = match c {
            Container::Creative => (195.0, 160.0),
            Container::Chest(p) if self.chest_halves(p).1.is_some() => (176.0, 222.0),
            Container::Chest(_) => (176.0, 168.0),
            _ => (176.0, 166.0),
        };
        let (px, py) = self.panel(panel_w, panel_h);
        if matches!(c, Container::Inventory | Container::Creative) {
            self.draw_effects_list(px, py, panel_w * s);
        }
        let at = |gx: f32, gy: f32| (px + gx * s, py + gy * s);

        match c {
            Container::Inventory | Container::Crafting(_) => {
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
            Container::Furnace(p) => {
                let (tx, ty) = at(8.0, 6.0);
                self.label(t("gui.furnace"), tx, ty);
                let f = self
                    .block_entities
                    .furnaces
                    .get(&p)
                    .cloned()
                    .unwrap_or_default();
                for (i, (gx, gy, size)) in
                    [(55.0, 16.0, SLOT), (55.0, 52.0, SLOT), (111.0, 30.0, 26.0)]
                        .into_iter()
                        .enumerate()
                {
                    let (x, y) = at(gx, gy);
                    let content = [f.input, f.fuel, f.output][i];
                    if self.draw_slot_sized(x, y, size, content) {
                        hovered = Some(SlotRef::Furnace(i));
                    }
                }
                // Flame (fuel left) and arrow (smelting progress).
                let burn = if f.burn > 0.0 && f.burn_total > 0.0 {
                    f.burn / f.burn_total
                } else {
                    0.0
                };
                let (fx, fy) = at(57.0, 36.0);
                self.flame(fx, fy, burn);
                let (ax, ay) = at(80.0, 35.5);
                self.arrow(ax, ay, 24.0, f.cook / SMELT_TIME);
                let (lx, ly) = at(8.0, 72.0);
                self.label(t("gui.inventory"), lx, ly);
                self.inventory_slots(px, py, 84.0, &mut hovered);
            }
            Container::Chest(p) => {
                let slots = self.chest_slots(p);
                let double = slots.len() > 27;
                let (tx, ty) = at(8.0, 6.0);
                self.label(
                    t(if double {
                        "gui.large_chest"
                    } else {
                        "gui.chest"
                    }),
                    tx,
                    ty,
                );
                for (i, st) in slots.iter().enumerate() {
                    let (x, y) = at(8.0 + (i % 9) as f32 * SLOT, 18.0 + (i / 9) as f32 * SLOT);
                    if self.draw_slot(x, y, *st) {
                        hovered = Some(SlotRef::Chest(i));
                    }
                }
                // Three more rows push the inventory down.
                let extra = if double { 3.0 * SLOT } else { 0.0 };
                let (lx, ly) = at(8.0, 74.0 + extra);
                self.label(t("gui.inventory"), lx, ly);
                self.inventory_slots(px, py, 86.0 + extra, &mut hovered);
            }
            Container::Creative => {
                let (tx, ty) = at(8.0, 6.0);
                self.label(t("gui.creative"), tx, ty);
                let fs = (s * 0.75).round().max(1.0);
                let title_end = tx + self.ui.text_width(t("gui.creative"), fs);
                self.search_box(px, py, title_end);
                let all = creative_items(&self.creative_search);
                if all.is_empty() {
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
                for r in 0..=visible {
                    for cidx in 0..9 {
                        let i = (first + r) * 9 + cidx;
                        let x = px + (9.0 + cidx as f32 * SLOT) * s;
                        let y = (py + (18.0 + (r as f32 - frac) * SLOT) * s).round();
                        let content = all.get(i).map(|&id| Stack::new(id, 1));
                        if self.draw_slot(x, y, content) {
                            if let Some(&id) = all.get(i) {
                                hovered = Some(SlotRef::Creative(id));
                                hovered_stack = content;
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
                    th.scroll_thumb,
                );
                // Hotbar + trash
                for i in 0..9 {
                    let (x, y) = at(9.0 + i as f32 * SLOT, 134.0);
                    if self.draw_slot(x, y, self.inventory.slots[i]) {
                        hovered = Some(SlotRef::Inv(i));
                    }
                }
                let (x, y) = at(173.0, 134.0);
                if self.draw_slot(x, y, None) {
                    hovered = Some(SlotRef::Trash);
                    self.ui.set_tooltip(t("gui.trash"));
                }
                let (lx, ly) = at(174.0 + 4.0, 138.0);
                self.ui.text("x", lx, ly, s, rgba(200, 60, 60, 255), false);
            }
        }

        // Tooltip for the hovered stack.
        if self.cursor.is_none() {
            if let Some(r) = hovered {
                let st = match r {
                    SlotRef::Creative(_) => hovered_stack,
                    SlotRef::CraftOut => self.craft_result(c),
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
                    SlotRef::CraftOut => self.craft_result(c),
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
                    _ => self.click_slot(c, r, right, shift && !right),
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
                            | SlotRef::Furnace(2)
                    )
                {
                    let mut hot = self.inventory.slots[d].take();
                    if let Some(slot) = self.slot_mut(c, r) {
                        std::mem::swap(slot, &mut hot);
                    }
                    self.inventory.slots[d] = hot;
                }
            }
        } else if self.ui.pressed || self.ui.right_pressed {
            // Clicking outside the window throws the held stack.
            let inside = self.ui.hit(px, py, panel_w * s, panel_h * s);
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

        // Stack on the mouse cursor.
        if let Some(st) = self.cursor {
            let m = self.ui.mouse;
            draw_stack(&mut self.ui, m.x - 8.0 * s, m.y - 8.0 * s, 16.0 * s, &st);
        }
    }
}
