//! The rules of the item screens: opening and closing a container, clicking, shift-clicking,
//! dragging and double-clicking slots.

use crate::client::{Container, Game, Screen};
use crate::client::gui::SlotRef;
use crate::item::{Slot, Stack, armor_of, max_stack};
use crate::item::inventory::{add_to, click, take};
use glam::IVec3;

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
    matches!(r, SlotRef::Inv(_) | SlotRef::Chest(_))
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
        if let Container::Crafting(_) = c {
            // (at a table the crafting tab comes up first)
            self.inv_ui.craft.open = true;
        }
        self.open_station(c);
        self.screen = Screen::Container(c);
        self.set_grab(false);
        self.input.keys.clear();
    }

    /// Closes an item screen: what is on the cursor goes back into the inventory.
    pub(in crate::client) fn close_container(&mut self) {
        self.inv_ui.drag = None;
        self.inv_ui.press_pick = None;
        self.inv_ui.search_focused = false;
        self.inv_ui.craft.job = None;
        if let Screen::Container(c) = self.screen {
            if Self::container_pos(c).is_some() {
                // (the last changes go to the server before closing)
                self.net_container_sync();
                self.net_container_closed();
            }
        }
        if let Screen::Container(Container::GunStation(_)) = self.screen {
            self.close_gun_station();
        }
        if let Some(st) = self.me.items.cursor.take() {
            self.give(st);
        }
        self.resume();
    }

    /// The block a container screen belongs to, as the server is told (`Msg::Open`): a chest
    /// or a gun station (a crafting table's screen is the player's own inventory).
    pub(in crate::client) fn container_pos(c: Container) -> Option<IVec3> {
        match c {
            Container::Chest(p) | Container::GunStation(p) => Some(p),
            Container::Inventory | Container::Crafting(_) | Container::Creative => None,
        }
    }

    /// Contents of a chest: 27 slots, or 54 for a double chest (left half first).
    pub(super) fn chest_slots(&self, p: IVec3) -> Vec<Slot> {
        self.level.block_entities.chest_slots(&self.terrain.world, p)
    }

    /// Stores `slots` (as `chest_slots` returns them) into the chest's halves.
    fn set_chest_slots(&mut self, p: IVec3, slots: &[Slot]) {
        self.level.block_entities.set_chest_slots(&self.terrain.world, p, slots);
    }

    pub(super) fn slot_mut(&mut self, c: Container, r: SlotRef) -> Option<&mut Slot> {
        match r {
            SlotRef::Inv(i) => Some(&mut self.me.items.inventory.slots[i]),
            SlotRef::Chest(i) => match c {
                Container::Chest(p) => {
                    let (a, b) = self.chest_halves(p);
                    let (q, i) = if i < 27 { (a, i) } else { (b?, i - 27) };
                    self.level.block_entities.chests.get_mut(&q).map(|ch| &mut ch[i])
                }
                _ => None,
            },
            SlotRef::Armor(i) => Some(&mut self.me.items.inventory.armor[i]),
            _ => None,
        }
    }

    /// Shift-click inside the inventory: from the hotbar (slot `i` < 9) into the main part,
    /// or the other way round. Returns what does not fit.
    fn move_within_inventory(&mut self, i: usize, stack: Stack) -> Option<Stack> {
        if i < 9 {
            add_to(&mut self.me.items.inventory.slots[9..], stack)
        } else {
            add_to(&mut self.me.items.inventory.slots[..9], stack)
        }
    }

    /// Shift-click: move a stack to the "other" section.
    fn quick_move(&mut self, c: Container, r: SlotRef) {
        // In the inventory armor goes on (into its empty slot), and comes off.
        if let (Container::Inventory | Container::Crafting(_) | Container::Creative, SlotRef::Inv(i)) = (c, r) {
            let piece = self.me.items.inventory.slots[i].and_then(|s| armor_of(s.item)).map(|a| a.0);
            if let Some(p) = piece.filter(|&p| self.me.items.inventory.armor[p].is_none()) {
                self.me.items.inventory.armor[p] = self.me.items.inventory.slots[i].take();
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
            _ => self.me.items.inventory.add(stack),
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
            SlotRef::Creative(id) => {
                if shift {
                    // A full stack straight into the first empty slot (hotbar first).
                    if let Some(slot) = self.me.items.inventory.slots.iter_mut().find(|s| s.is_none()) {
                        *slot = Some(Stack::new(id, max_stack(id)));
                    }
                    return;
                }
                // Like Minecraft: a click takes one; clicking again with the same item adds one
                // more, and clicking with anything else puts the held stack away.
                match &mut self.me.items.cursor {
                    None => self.me.items.cursor = Some(Stack::one(id)),
                    Some(cur) if cur.item == id && cur.damage == 0 => {
                        cur.count = (cur.count + 1).min(max_stack(id));
                    }
                    Some(_) => self.me.items.cursor = None,
                }
            }
            SlotRef::Trash => self.me.items.cursor = None,
            // Only the piece that goes there.
            SlotRef::Armor(i)
                if self
                    .me.items.cursor
                    .is_some_and(|c| armor_of(c.item).map(|a| a.0) != Some(i)) => {}
            SlotRef::Armor(i) if !shift => {
                let mut cursor = self.me.items.cursor.take();
                click(&mut self.me.items.inventory.armor[i], &mut cursor, false);
                self.me.items.cursor = cursor;
                self.audio.play(crate::audio::Sound::ArmorEquip, None, 0.8);
            }
            _ => {
                if shift {
                    self.quick_move(c, r);
                } else {
                    let mut cursor = self.me.items.cursor.take();
                    if let Some(slot) = self.slot_mut(c, r) {
                        click(slot, &mut cursor, right);
                    }
                    self.me.items.cursor = cursor;
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
        self.me.items.cursor = (left > 0).then_some(Stack {
            count: left,
            ..d.stack
        });
    }

    /// Double-click: pull matching items from every slot into the cursor stack.
    pub(super) fn collect_all(&mut self, c: Container) {
        let Some(mut cur) = self.me.items.cursor else { return };
        let max = max_stack(cur.item);
        let mut refs: Vec<SlotRef> = Vec::new();
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
        self.me.items.cursor = Some(cur);
    }
}
