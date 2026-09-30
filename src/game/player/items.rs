//! Using items: throwing, placing blocks, buckets, bottles, eating and drinking, and opening
//! containers.

use crate::game::*;
use crate::entity::player::raycast_fluid;
use crate::item::inventory::take;
use crate::item::*;
impl Game {
    /// Q: throw the held item (Ctrl+Q: the whole stack).
    pub(in crate::game) fn drop_held(&mut self, all: bool) {
        let slot = self.hotbar_slot;
        let Some(s) = self.inventory.slots[slot] else {
            return;
        };
        let n = if all { s.count } else { 1 };
        take(&mut self.inventory.slots[slot], n);
        self.throw(Stack { count: n, ..s });
        self.hand.swing();
    }

    /// Throws a stack in the look direction.
    pub(in crate::game) fn throw(&mut self, stack: Stack) {
        let dir = look_dir(self.yaw, self.pitch);
        let pos = self.eye() - Vec3::Y * 0.3 + dir * 0.3;
        self.add_item(ItemEntity::new(pos, dir * 6.0 + Vec3::Y * 1.5, stack, 1.5));
    }

    /// Into the selected hotbar slot if it is empty (a filled bottle or bucket replacing the
    /// used one), otherwise into the inventory.
    pub(in crate::game) fn put_in_hand(&mut self, stack: Stack) {
        let slot = &mut self.inventory.slots[self.hotbar_slot];
        if slot.is_none() {
            *slot = Some(stack);
        } else {
            self.give(stack);
        }
    }

    /// The mob the crosshair is on, as an index into `mobs` (if it is still there).
    pub(in crate::game) fn target_mob(&self) -> Option<usize> {
        let id = self.mob_target?;
        self.level.mobs.iter().position(|m| m.id == id)
    }

    /// The inventory as it is saved: items in the 2x2 grid or on the cursor count as carried
    /// (an open crafting table keeps its own grid).
    pub(in crate::game) fn carried_slots(&self) -> [Slot; crate::item::inventory::SIZE] {
        let at_table = matches!(self.screen, Screen::Container(Container::Crafting(_)));
        let mut slots = self.inventory.slots;
        let grid = if at_table { &[][..] } else { &self.craft[..] };
        // A magazine on its way into a gun (a reload going on) is still the player's.
        let reloading = self.guns.reload.is_some() && !self.creative();
        let held = [self.cursor, self.craft_out, self.guns.plan.new_mag.filter(|_| reloading)];
        for s in grid.iter().chain(held.iter()).flatten() {
            let _ = inventory::add_to(&mut slots, *s);
        }
        slots
    }

    /// Puts a stack into the inventory; drops what does not fit.
    pub(in crate::game) fn give(&mut self, stack: Stack) {
        if let Some(left) = self.inventory.add(stack) {
            self.throw(left);
        }
    }

    pub(in crate::game) fn use_item(&mut self) {
        let held = self.held();
        // The grenade crate on a rifle station: grenades in, or one out.
        if self.input.right_pressed && self.crate_click() {
            return;
        }
        // With a gun the right mouse button aims: no opening or placing with it.
        if GunKind::of(held).is_some() {
            return;
        }
        // A grenade is readied and thrown by holding the button (`update_grenade_hold`).
        if crate::game::grenades::GrenadeKind::of(held).is_some() {
            return;
        }
        // A fishing rod casts by holding the button (`update_fishing`), unless there is
        // something to open.
        if held == FISHING_ROD {
            if !self.opens_target() || self.fishing.line.is_some() {
                return;
            }
        }
        // A magazine in hand does nothing (it is loaded at the gun station).
        if magazine_capacity(held).is_some() {
            return;
        }
        // Armor in hand: put it on (swapping with what is worn).
        if let Some((piece, _)) = armor_of(held) {
            if self.input.right_pressed {
                let slot = self.hotbar_slot;
                std::mem::swap(&mut self.inventory.slots[slot], &mut self.inventory.armor[piece]);
                self.audio.play(crate::audio::Sound::ArmorEquip, None, 0.8);
                self.hand.swing();
            }
            return;
        }
        let sneaking = self.sneaking();
        if let Some(i) = self.target_mob() {
            if held == SHEARS && self.level.mobs[i].can_shear() {
                self.shear(i);
                return;
            }
            if self.input.right_pressed && self.use_on_wolf(i) {
                return;
            }
        }
        if let Some((hit, _)) = self.target {
            let hb = self.terrain.world.geti(hit);
            // Opening things (tables, chests, doors, beds...) takes a fresh click: holding the
            // button (blocking with a sword, placing blocks) and looking at one does nothing.
            if !sneaking && opens_on_use(hb) && !self.input.right_pressed {
                return;
            }
            // Furnaces have no screen: meat goes on top, the rest into the front.
            if let Some((p, k)) = self.furnace_part.filter(|(p, _)| *p == hit) {
                if self.input.right_pressed && self.use_furnace(p, k, false) {
                    return;
                }
            }
            if !sneaking {
                if hb == CRAFTING_TABLE {
                    // Whatever was left on the table is still there.
                    self.craft = self
                        .level.block_entities
                        .tables
                        .get(&hit)
                        .copied()
                        .unwrap_or([None; 9]);
                    self.open_container(Container::Crafting(hit));
                    return;
                }
                if let Some(main) = bench_main(hit, hb, |q| self.terrain.world.geti(q)) {
                    self.open_gun_station(main);
                    return;
                }
                if hb == GUN_STATION {
                    // An old one-block station: it becomes the two-block one (reaching to the
                    // right, facing the player) where there is room, and opens.
                    let d = look_dir(self.yaw, 0.0);
                    let facing = (facing_of(d.x, d.z) + 2) & 3;
                    let w = &self.terrain.world;
                    let right = hit + chest_right(facing);
                    let front = facing_dir(facing);
                    let room = is_replaceable(w.geti(right))
                        && !self.player.intersects(right)
                        && !is_solid(w.geti(hit + front))
                        && !is_solid(w.geti(right + front));
                    if room {
                        self.edit_block(hit, gun_bench_id(facing, false));
                        self.edit_block(right, gun_bench_id(facing, true));
                        self.open_gun_station(hit);
                    }
                    return;
                }
                if is_furnace(hb) {
                    return;
                }
                if is_door(hb) {
                    self.toggle_door(hit);
                    return;
                }
                if is_bed(hb) {
                    self.use_bed(hit);
                    return;
                }
                if is_chest(hb) {
                    let (a, b) = self.chest_halves(hit);
                    for q in std::iter::once(a).chain(b) {
                        self.level.block_entities
                            .chests
                            .entry(q)
                            .or_insert_with(|| Box::new([None; 27]));
                    }
                    self.open_container(Container::Chest(hit));
                    return;
                }
            }
        }
        match held {
            BUCKET => self.fill_bucket(),
            WATER_BUCKET | LAVA_BUCKET => self.empty_bucket(held),
            PIG_SPAWN_EGG => self.use_spawn_egg(MobKind::Pig),
            SHEEP_SPAWN_EGG => self.use_spawn_egg(MobKind::Sheep),
            WOLF_SPAWN_EGG => self.use_spawn_egg(MobKind::Wolf),
            TARGET_DUMMY => self.use_spawn_egg(MobKind::Dummy),
            GLASS_BOTTLE => self.fill_bottle(),
            _ if block_of(held).is_some() => self.place_block(held),
            _ => {}
        }
    }

    /// Glass bottle on water: fills it (lake water, not safe to drink until boiled).
    pub(in crate::game) fn fill_bottle(&mut self) {
        let dir = look_dir(self.yaw, self.pitch);
        let Some((hit, _)) = raycast_fluid(&self.terrain.world, self.eye(), dir, 5.0) else {
            return;
        };
        if !is_water(self.terrain.world.geti(hit)) {
            return;
        }
        let slot = self.hotbar_slot;
        if !self.creative() {
            take(&mut self.inventory.slots[slot], 1);
        }
        self.put_in_hand(Stack::one(WATER_BOTTLE));
        self.hand.swing();
        self.action_cooldown = 0.25;
    }

    /// Holding the right mouse button with food or drink: eat or drink it in 1.6 seconds
    /// (not while aiming at a container, which opens instead).
    pub(in crate::game) fn update_using(&mut self, dt: f32, control: bool) {
        use crate::entity::survival::USE_TIME;
        let held = self.held();
        let c = consumable(held);
        let sneaking = self.sneaking();
        let at_container = !sneaking && self.opens_target();
        let ok = control
            && self.input.right_down
            && !self.creative()
            && !at_container
            && c.as_ref().is_some_and(|c| self.needs.wants(c));
        let Some(c) = c.filter(|_| ok) else {
            self.using = None;
            self.hand.eating = None;
            return;
        };
        let before = match self.using {
            Some((item, t)) if item == held => t,
            _ => 0.0,
        };
        let now = before + dt;
        let mouth = self.eye() + look_dir(self.yaw, self.pitch) * 0.35 - Vec3::Y * 0.15;
        if !c.drink && now > 0.35 && (now / 0.2).floor() != (before / 0.2).floor() {
            // Bits of food fly off every 4 ticks, like Minecraft's eating particles.
            if let Icon::Flat(layer) = icon(held) {
                let (sky, blk) = (
                    self.terrain.world.sky_estimate(mouth),
                    self.terrain.world.block_light_estimate(mouth),
                );
                self.particles.crumbs(mouth, layer, 5, sky, blk);
            }
        }
        if now < USE_TIME {
            self.using = Some((held, now));
            self.hand.eating = Some(now);
            return;
        }
        // Done: restore food/thirst, use up the item, the bottle comes back empty.
        self.using = None;
        self.hand.eating = None;
        self.needs.consume(&c);
        let slot = self.hotbar_slot;
        take(&mut self.inventory.slots[slot], 1);
        if c.drink {
            self.put_in_hand(Stack::one(GLASS_BOTTLE));
        }
        if let Some(s) = c.sick.filter(|s| self.random() < s.chance) {
            // Lake water, raw or burnt meat: a stomach bug (poison for a few seconds, the
            // view sways for longer).
            use crate::entity::survival::EffectKind;
            if s.poison > 0.0 {
                self.needs.add_effect(EffectKind::Poison, s.poison);
            }
            if s.nausea > 0.0 {
                self.needs.add_effect(EffectKind::Nausea, s.nausea);
            }
        }
        self.slot_name_timer = 0.0;
    }

    pub(in crate::game) fn fill_bucket(&mut self) {
        let dir = look_dir(self.yaw, self.pitch);
        let Some((hit, _)) = raycast_fluid(&self.terrain.world, self.eye(), dir, 5.0) else {
            return;
        };
        let b = self.terrain.world.geti(hit);
        if !is_fluid(b) || fluid_level(b) != 0 {
            return;
        }
        let filled = if is_water(b) {
            WATER_BUCKET
        } else {
            LAVA_BUCKET
        };
        self.edit_block(hit, AIR);
        let slot = self.hotbar_slot;
        if !self.creative() {
            take(&mut self.inventory.slots[slot], 1);
        }
        // Creative only takes one filled bucket unless the empty one was used up.
        if self.inventory.slots[slot].is_none()
            || !self.creative()
            || self.inventory.count(filled) == 0
        {
            self.put_in_hand(Stack::one(filled));
        }
        self.hand.swing();
        self.action_cooldown = 0.25;
    }

    pub(in crate::game) fn empty_bucket(&mut self, held: ItemId) {
        let Some((hit, prev)) = self.target else {
            return;
        };
        let w = &self.terrain.world;
        let at = if is_replaceable(w.geti(hit)) {
            hit
        } else {
            prev
        };
        if !is_replaceable(w.geti(at)) && !fluid_breaks(w.geti(at)) {
            return;
        }
        self.edit_block(at, if held == WATER_BUCKET { WATER } else { LAVA });
        if !self.creative() {
            self.inventory.slots[self.hotbar_slot] = Some(Stack::one(BUCKET));
        }
        self.hand.swing();
        self.action_cooldown = 0.25;
    }

    pub(in crate::game) fn place_block(&mut self, held: ItemId) {
        let Some((hit, prev)) = self.target else {
            return;
        };
        let Some(base) = block_of(held) else { return };
        let w = &self.terrain.world;
        let at = if w.geti(hit) == TALL_GRASS { hit } else { prev };
        let cur = w.geti(at);
        if !is_replaceable(cur) || at.y < 0 || at.y >= HEIGHT as i32 {
            return;
        }
        // Directional blocks face the player; stairs and doors take the look direction.
        let d = look_dir(self.yaw, 0.0);
        let look = facing_of(d.x, d.z);
        let facing = (look + 2) & 3;
        // The face of the clicked block the new one goes against (outward).
        let normal = if at == hit { IVec3::Y } else { at - hit };
        let b = match def(base).place {
            Place::Door => return self.place_door(at, look),
            Place::Bed => return self.place_bed(at, look),
            Place::BigFurnace => return self.place_big_furnace(at, base, facing),
            Place::GunBench => return self.place_gun_bench(at, facing, false),
            Place::RifleBench => return self.place_gun_bench(at, facing, true),
            Place::Torch => {
                let support = hit - at;
                if at == hit || support == IVec3::NEG_Y {
                    TORCH
                } else if support.y == 0 {
                    let Some(wall) = wall_torch_for_support(support) else {
                        return;
                    };
                    wall
                } else {
                    return;
                }
            }
            Place::Lantern => {
                // Hangs when placed against the underside of a block (or when there is nothing
                // to stand on), otherwise stands.
                let hang = hit - at == IVec3::Y;
                if hang || !is_opaque(w.geti(at - IVec3::Y)) {
                    LANTERN_HANGING
                } else {
                    LANTERN
                }
            }
            Place::Chest => {
                let sneaking = self.sneaking();
                self.chest_to_place(at, hit, facing, sneaking)
            }
            Place::Facing => base + facing as Block,
            Place::Stairs => {
                // Upside down against the underside of a block or the top half of a side.
                let upper = self.target_point.y - at.y as f32 > 0.5;
                let upside_down = normal == IVec3::NEG_Y || (normal.y == 0 && upper);
                stairs_id(look, upside_down)
            }
            Place::Log => {
                let axis = if normal.x != 0 {
                    0
                } else if normal.z != 0 {
                    2
                } else {
                    1
                };
                let on = w.geti(hit);
                if at != hit && is_log(on) && !self.sneaking() {
                    // Grows out of the wood clicked: up a trunk it is trunk; out of a side, or
                    // on from a branch, a branch; on along a lying log, the same log.
                    if is_branch(on) || (axis != 1 && log_axis(on) != axis) {
                        branch_with_axis(base, axis)
                    } else {
                        log_with_axis(base, axis)
                    }
                } else {
                    // Lies along the clicked face's normal, like Minecraft (sneaking: always).
                    log_with_axis(base, axis)
                }
            }
            Place::Plain => base,
        };
        if is_solid(b) && (self.player.intersects(at) || self.drawer_room(at)) {
            return;
        }
        if needs_support(b) && !Self::supported(w, at, b) {
            return;
        }
        self.edit_block(at, b);
        if !self.creative() {
            let slot = self.hotbar_slot;
            take(&mut self.inventory.slots[slot], 1);
        }
        self.hand.swing();
        self.action_cooldown = 0.2;
    }

    /// A blast furnace (with its chimney) or an advanced furnace (two wide, two tall), its
    /// furnace block at `at` facing the player: where there is room for all of it.
    fn place_big_furnace(&mut self, at: IVec3, base: Block, facing: u8) {
        let cells = furnace_cells(base, facing, false);
        let w = &self.terrain.world;
        let room = cells.iter().all(|&(o, _)| {
            let q = at + o;
            q.y < HEIGHT as i32 && is_replaceable(w.geti(q)) && !self.player.intersects(q) && !self.drawer_room(q)
        });
        if !room {
            return;
        }
        // The furnace block first: it gets the furnace's contents.
        for (o, b) in cells {
            self.edit_block(at + o, b);
        }
        if !self.creative() {
            let slot = self.hotbar_slot;
            take(&mut self.inventory.slots[slot], 1);
        }
        self.hand.swing();
        self.action_cooldown = 0.2;
    }

    /// A gun station, two blocks wide (a rifle station three): its left block at `at`, the
    /// others beside it (to the right seen from its front), its front (the drawer) facing the
    /// player. The cells in front of it must be free, for the drawer.
    fn place_gun_bench(&mut self, at: IVec3, facing: u8, rifle: bool) {
        let w = &self.terrain.world;
        let main = if rifle { rifle_bench_id(facing) } else { gun_bench_id(facing, false) };
        let cells = bench_cells(at, main);
        let front = facing_dir(facing);
        let room = cells.iter().all(|&q| {
            is_replaceable(w.geti(q)) && !self.player.intersects(q) && !self.drawer_room(q) && !is_solid(w.geti(q + front))
        });
        if !room {
            return;
        }
        for (i, &q) in cells.iter().enumerate() {
            let b = match (rifle, i) {
                (_, 0) => main,
                (true, _) => RIFLE_BENCH_PART,
                (false, _) => gun_bench_id(facing, true),
            };
            self.edit_block(q, b);
        }
        if !self.creative() {
            let slot = self.hotbar_slot;
            take(&mut self.inventory.slots[slot], 1);
        }
        self.hand.swing();
        self.action_cooldown = 0.2;
    }

    /// Whether `q` is in front of a gun station, where its drawer slides out (nothing solid
    /// may be put there).
    pub(in crate::game) fn drawer_room(&self, q: IVec3) -> bool {
        let w = &self.terrain.world;
        (0..4u8).any(|f| {
            let at = q - facing_dir(f);
            let b = w.geti(at);
            // (a rifle station's other blocks have the facing of its left one)
            is_gun_bench(b) && bench_main(at, b, |p| w.geti(p)).and_then(|m| facing(w.geti(m))) == Some(f)
        })
    }

    /// A door at `at` (lower half) and above it, for a player looking toward `facing`.
    fn place_door(&mut self, at: IVec3, facing: u8) {
        let w = &self.terrain.world;
        let top = at + IVec3::Y;
        let below = w.geti(at - IVec3::Y);
        if top.y >= HEIGHT as i32
            || !is_replaceable(w.geti(top))
            || !is_solid(below)
            || is_door(below)
            || self.player.intersects(at)
            || self.player.intersects(top)
            || self.drawer_room(at)
            || self.drawer_room(top)
        {
            return;
        }
        let hinge_right = self.door_hinge_right(at, facing);
        self.edit_block(at, door_id(facing, false, false, hinge_right));
        self.edit_block(top, door_id(facing, false, true, hinge_right));
        if !self.creative() {
            let slot = self.hotbar_slot;
            take(&mut self.inventory.slots[slot], 1);
        }
        self.hand.swing();
        self.action_cooldown = 0.2;
    }

    /// A bed: the foot half at `at` and the head half one block further the way the player
    /// looks (`facing`), like Minecraft.
    fn place_bed(&mut self, at: IVec3, facing: u8) {
        let w = &self.terrain.world;
        let head = at + facing_dir(facing);
        if !is_replaceable(w.geti(head))
            || !is_solid(w.geti(at - IVec3::Y))
            || !is_solid(w.geti(head - IVec3::Y))
            || self.player.intersects(at)
            || self.player.intersects(head)
            || self.drawer_room(at)
            || self.drawer_room(head)
        {
            return;
        }
        self.edit_block(at, bed_id(facing, false));
        self.edit_block(head, bed_id(facing, true));
        if !self.creative() {
            let slot = self.hotbar_slot;
            take(&mut self.inventory.slots[slot], 1);
        }
        self.hand.swing();
        self.action_cooldown = 0.2;
    }

    /// Minecraft's door hinge: next to another door it makes a double door, otherwise the
    /// hinge goes toward the wall, or to the side of the block that was clicked.
    fn door_hinge_right(&self, at: IVec3, facing: u8) -> bool {
        let w = &self.terrain.world;
        let (left, right) = (facing_dir(facing + 3), facing_dir(facing + 1));
        let full = |q: IVec3| {
            let b = w.geti(q);
            is_solid(b) && !is_door(b) && !is_stairs(b)
        };
        let lower_door = |q: IVec3| {
            let b = w.geti(q);
            is_door(b) && !door_upper(b)
        };
        let up = IVec3::Y;
        let walls = full(at + right) as i32 + full(at + right + up) as i32
            - full(at + left) as i32
            - full(at + left + up) as i32;
        let (l, r) = (lower_door(at + left), lower_door(at + right));
        if (l && !r) || walls > 0 {
            return true;
        }
        if (r && !l) || walls < 0 {
            return false;
        }
        let d = facing_dir(facing);
        let (dx, dz) = (
            self.target_point.x - at.x as f32,
            self.target_point.z - at.z as f32,
        );
        let left_hinge = (d.x >= 0 || dz >= 0.5)
            && (d.x <= 0 || dz <= 0.5)
            && (d.z >= 0 || dx <= 0.5)
            && (d.z <= 0 || dx >= 0.5);
        !left_hinge
    }

    /// Opens or closes a door (both halves). It always swings away from the player: out
    /// into the next block when opened from the side it closes on (if there is room).
    /// The other door of a double door goes with it.
    pub(in crate::game) fn toggle_door(&mut self, p: IVec3) {
        let w = &self.terrain.world;
        let b = w.geti(p);
        let mut cells = vec![p, p + door_other_half(b)];
        // A double door: the door beside the free edge, hinged on the other side.
        let f = door_facing(b);
        let hinge_right = door_hinge_right(b);
        let q = p + facing_dir(if hinge_right { f + 3 } else { f + 1 });
        let qb = w.geti(q);
        if is_door(qb)
            && door_facing(qb) == f
            && door_hinge_right(qb) != hinge_right
            && door_upper(qb) == door_upper(b)
            && door_open(qb) == door_open(b)
        {
            cells.extend([q, q + door_other_half(qb)]);
        }
        let open = !door_open(b);
        let c = door_closed_side(b);
        let center = p.as_vec3() + Vec3::splat(0.5);
        let from_closed_side = (self.player.pos - center).dot(c.as_vec3()) > 0.0;
        let room = cells.iter().all(|&q| {
            let n = w.geti(q + c);
            !is_solid(n) || is_door(n)
        });
        let out = !from_closed_side && room;
        let changes: Vec<(IVec3, Block)> = cells
            .iter()
            .map(|&q| (q, w.geti(q)))
            .filter(|&(_, qb)| is_door(qb))
            .map(|(q, qb)| (q, door_set_open(qb, open, out)))
            .collect();
        for (q, nb) in changes {
            self.edit_block(q, nb);
        }
        self.hand.swing();
        self.action_cooldown = 0.25;
    }
}
