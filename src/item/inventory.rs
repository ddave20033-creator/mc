//! Player inventory and Minecraft-style slot click handling.

use crate::item::{max_damage, max_stack, Slot, Stack};

pub const SIZE: usize = 36;

pub struct Inventory {
    /// 0..9 hotbar, 9..36 main inventory.
    pub slots: [Slot; SIZE],
    /// Worn: helmet, chestplate, leggings, boots, bulletproof vest.
    pub armor: [Slot; crate::item::ARMOR_SLOTS],
}

impl Inventory {
    pub fn new() -> Self {
        Self {
            slots: [None; SIZE],
            armor: [None; crate::item::ARMOR_SLOTS],
        }
    }

    /// Adds a stack, filling matching stacks first, then empty slots (hotbar first).
    /// Returns what did not fit.
    pub fn add(&mut self, stack: Stack) -> Option<Stack> {
        add_to(&mut self.slots, stack)
    }

    pub fn count(&self, item: u16) -> u32 {
        self.slots
            .iter()
            .flatten()
            .filter(|s| s.item == item)
            .map(|s| s.count as u32)
            .sum()
    }
}

/// Adds `stack` into `slots` (merging first). Returns the leftover.
pub fn add_to(slots: &mut [Slot], mut stack: Stack) -> Option<Stack> {
    let max = max_stack(stack.item);
    for s in slots.iter_mut().flatten() {
        if s.stacks_with(&stack) && s.count < max {
            let n = (max - s.count).min(stack.count);
            s.count += n;
            stack.count -= n;
            if stack.count == 0 {
                return None;
            }
        }
    }
    for s in slots.iter_mut() {
        if s.is_none() {
            let n = stack.count.min(max);
            *s = Some(Stack { count: n, ..stack });
            stack.count -= n;
            if stack.count == 0 {
                return None;
            }
        }
    }
    Some(stack)
}

/// Removes `n` items from a slot.
pub fn take(slot: &mut Slot, n: u8) {
    if let Some(s) = slot {
        if s.count <= n {
            *slot = None;
        } else {
            s.count -= n;
        }
    }
}

/// Applies wear to a tool; returns true if it broke.
pub fn damage(slot: &mut Slot, amount: u16) -> bool {
    if let Some(s) = slot {
        let max = max_damage(s.item);
        if max == 0 || amount == 0 {
            return false;
        }
        s.damage += amount;
        if s.damage >= max {
            *slot = None;
            return true;
        }
    }
    false
}

/// Minecraft click on a normal slot with the mouse cursor stack.
/// Left: pick up / put down / merge / swap. Right: pick up half / put one.
pub fn click(slot: &mut Slot, cursor: &mut Slot, right: bool) {
    match (slot.as_mut(), cursor.as_mut()) {
        (None, None) => {}
        (Some(s), None) => {
            if right {
                let half = s.count.div_ceil(2);
                *cursor = Some(Stack { count: half, ..*s });
                take(slot, half);
            } else {
                *cursor = slot.take();
            }
        }
        (None, Some(c)) => {
            if right {
                *slot = Some(Stack { count: 1, ..*c });
                take(cursor, 1);
            } else {
                *slot = cursor.take();
            }
        }
        (Some(s), Some(c)) => {
            if s.stacks_with(c) {
                let room = max_stack(s.item) - s.count;
                let n = if right {
                    1.min(room)
                } else {
                    c.count.min(room)
                };
                s.count += n;
                take(cursor, n);
            } else {
                std::mem::swap(slot, cursor);
            }
        }
    }
}
