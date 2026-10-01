//! The rules of the item screens: opening and closing a container, clicking, shift-clicking,
//! dragging and double-clicking slots, and crafting (one or as many as the grid makes).

use super::*;


/// Mouse drag with a held stack: left spreads it evenly, right drops one per slot.
pub(in crate::client) struct Drag {
    pub(super) right: bool,
    /// The cursor stack when the drag started.
    pub(super) stack: Stack,
    /// Slots visited so far with their contents before the drag.
    pub(super) slots: Vec<(SlotRef, Slot)>,
}

/// Whether `st` can be dropped onto a slot holding `cur`.
pub(super) fn fits(cur: Slot, st: &Stack) -> bool {
    match cur {
        None => true,
        Some(c) => c.stacks_with(st) && c.count < max_stack(c.item),
    }
}

/// Slots that accept items from the cursor.
pub(super) fn droppable(r: SlotRef) -> bool {
    matches!(r, SlotRef::Inv(_) | SlotRef::Craft(_) | SlotRef::Chest(_))
}

impl Game {
    pub(in crate::client) fn open_container(&mut self, c: Container) {
        if let Some(p) = Self::container_pos(c) {
            // (the server sends its contents)
            self.net_container_opened(p);
        }
        self.inv_ui.drag = None;
        self.inv_ui.press_pick = None;
        self.inv_ui.search_focused = false;
        self.inv_ui.jei.focused = false;
        self.open_station(c);
        self.screen = Screen::Container(c);
        self.set_grab(false);
        self.input.keys.clear();
    }

    /// Stores the crafting grid of an open crafting table back into the table.
    pub(in crate::client) fn stash_table(&mut self, clear: bool) {
        if let Screen::Container(Container::Crafting(p)) = self.screen {
            if self.craft.iter().any(|s| s.is_some()) {
                self.level.block_entities.tables.insert(p, self.craft);
            } else {
                self.level.block_entities.tables.remove(&p);
            }
            if clear {
                self.craft = [None; 9];
            }
        }
    }

    /// Closes an item screen: a crafting table keeps its grid, the 2x2 grid and the cursor
    /// go back to the inventory.
    pub(in crate::client) fn close_container(&mut self) {
        self.inv_ui.drag = None;
        self.inv_ui.press_pick = None;
        self.inv_ui.search_focused = false;
        if let Screen::Container(c) = self.screen {
            if Self::container_pos(c).is_some() {
                // (the last changes go to the server before closing)
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
    pub(in crate::client) fn container_pos(c: Container) -> Option<IVec3> {
        match c {
            Container::Crafting(p) | Container::Chest(p) | Container::GunStation(p) => Some(p),
            Container::Inventory | Container::Creative => None,
        }
    }

    /// Contents of a chest: 27 slots, or 54 for a double chest (left half first).
    pub(in crate::client) fn chest_slots(&self, p: IVec3) -> Vec<Slot> {
        self.level.block_entities.chest_slots(&self.terrain.world, p)
    }

    /// Stores `slots` (as `chest_slots` returns them) into the chest's halves.
    pub(in crate::client) fn set_chest_slots(&mut self, p: IVec3, slots: &[Slot]) {
        self.level.block_entities.set_chest_slots(&self.terrain.world, p, slots);
    }

    pub(super) fn craft_size(c: Container) -> usize {
        if matches!(c, Container::Crafting(_)) {
            3
        } else {
            2
        }
    }

    pub(super) fn slot_mut(&mut self, c: Container, r: SlotRef) -> Option<&mut Slot> {
        match r {
            SlotRef::Inv(i) => Some(&mut self.inventory.slots[i]),
            SlotRef::Craft(i) => Some(&mut self.craft[i]),
            SlotRef::Chest(i) => match c {
                Container::Chest(p) => {
                    let (a, b) = self.chest_halves(p);
                    let (q, i) = if i < 27 { (a, i) } else { (b?, i - 27) };
                    self.level.block_entities.chests.get_mut(&q).map(|ch| &mut ch[i])
                }
                _ => None,
            },
            SlotRef::Armor(i) => Some(&mut self.inventory.armor[i]),
            _ => None,
        }
    }

    pub(in crate::client) fn craft_result(&self, c: Container) -> Option<Stack> {
        let n = Self::craft_size(c);
        craft(&self.craft[..n * n], n)
    }

    pub(super) fn consume_craft_inputs(&mut self, c: Container) {
        let n = Self::craft_size(c);
        for s in &mut self.craft[..n * n] {
            take(s, 1);
        }
    }

    /// What the result slot shows: at a table what was crafted into its middle, in the
    /// inventory what the 2x2 grid makes.
    pub(super) fn craft_out_stack(&self, c: Container) -> Option<Stack> {
        if matches!(c, Container::Crafting(_)) {
            self.craft_out
        } else {
            self.craft_result(c)
        }
    }

    /// A left click at an open table away from the slots: crafts from the grid as many as
    /// fit in one stack (64, or one of what does not stack), into the middle of the table.
    /// With more on the grid, the next click makes the next stack once this one is taken.
    pub(super) fn craft_batch(&mut self, c: Container) {
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
    pub(super) fn move_within_inventory(&mut self, i: usize, stack: Stack) -> Option<Stack> {
        if i < 9 {
            add_to(&mut self.inventory.slots[9..], stack)
        } else {
            add_to(&mut self.inventory.slots[..9], stack)
        }
    }

    /// Shift-click: move a stack to the "other" section.
    pub(super) fn quick_move(&mut self, c: Container, r: SlotRef) {
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

    pub(super) fn click_slot(&mut self, c: Container, r: SlotRef, right: bool, shift: bool) {
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
    pub(super) fn apply_drag(&mut self, c: Container, d: &Drag) {
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
    pub(super) fn collect_all(&mut self, c: Container) {
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
}
