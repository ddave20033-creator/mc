//! The geometry built on the CPU each frame: particles, crack overlays, the first-person
//! hand (and body), the player model, dropped items, falling blocks and trees, mobs, the
//! other players, chest lids and doors, and what lies on furnaces and crafting tables.

use crate::client::{Container, Game, Screen, multi};
use crate::client::gui::SlotRef;
use crate::entity::block_entity::{
    build_chest_items, build_chest_lid, build_door, build_furnace_items, build_glow,
    build_table_items, build_table_made, chest_side,
};
use crate::entity::player::look_dir;
use crate::item::ItemId;
use crate::model::crack_overlay;
use crate::model::player::{PlayerPose, build_player, hand_pivot, limb_targets};
use crate::world::*;
use crate::world::mesh::Vertex;
use crate::world::textures::tex;
use glam::{IVec3, Mat4, Vec3};

use super::frame::View;

/// Geometry built on the CPU this frame, by render range. Kept from frame to frame (emptied,
/// not freed), so its lists do not grow anew to hundreds of thousands of vertices each frame.
#[derive(Default)]
pub(super) struct Scene {
    pub(super) particles: Vec<Vertex>,
    pub(super) overlay: Vec<Vertex>,
    pub(super) viewmodel: Vec<Vertex>,
    /// Entities: they always cast shadows, but are drawn only in third person (in first
    /// person they are copied into `particles` to be seen). This player's model comes first.
    pub(super) entity: Vec<Vertex>,
    pub(super) entity_visible: bool,
    pub(super) player_vertex_count: usize,
    /// Blended flames and glass (no depth writes, no shadows).
    pub(super) translucent: Vec<Vertex>,
    /// The first-person gun's glass, and its scope's eyepiece (showing the scope's view).
    pub(super) viewmodel_glass: Vec<Vertex>,
    pub(super) lens: Vec<Vertex>,
    /// Where the scope looks (a direction) and its field of view (radians), while its view
    /// shows on the eyepiece.
    pub(super) scope: Option<(Vec3, Vec3, Vec3, f32, f32)>,
    /// (scratch: the mobs and the other players, before they go into `entity`)
    pub(super) mobs: Vec<Vertex>,
}

impl Scene {
    pub(super) fn clear(&mut self) {
        for v in [
            &mut self.particles,
            &mut self.overlay,
            &mut self.viewmodel,
            &mut self.entity,
            &mut self.translucent,
            &mut self.viewmodel_glass,
            &mut self.lens,
            &mut self.mobs,
        ] {
            v.clear();
        }
        self.entity_visible = false;
        self.player_vertex_count = 0;
        self.scope = None;
    }
}

impl Game {
    /// Particles, crack overlays, the first-person hand (and body), the player model, dropped
    /// items, falling blocks, mobs, the other LAN players, chest lids and the items on
    /// crafting tables.
    pub(super) fn build_scene(&mut self, view: &View, dt: f32) -> Scene {
        let (in_world, third_person, cam) = (view.in_world, view.third_person, view.cam);
        let mut scene = std::mem::take(&mut self.gfx.scene);
        scene.clear();
        if in_world {
            // The pages the books in hands are open at.
            self.update_book_views(dt);
        }
        self.level.particles
            .build(&mut scene.particles, view.right, view.up);
        if in_world {
            self.build_gun_effects(&mut scene.particles, cam, view.right, view.up);
            self.build_bullet_holes(&mut scene.overlay, cam);
            self.build_grenades(&mut scene.particles);
        }
        if let (Some((p, prog)), Screen::Playing) = (self.me.aim.mining, self.screen) {
            if prog > 0.02 && !self.creative() {
                crack_overlay(&mut scene.overlay, p, prog);
            }
        }
        if in_world {
            // Blocks the other LAN players are mining.
            for (p, prog) in self.remote_cracks() {
                crack_overlay(&mut scene.overlay, p, prog);
            }
        }
        let (player_sky, player_blk) = self.terrain.world.light_estimate(self.eye());
        // First Person Model's dynamic hands: with the first-person body on, the regular hand
        // shows the held item while looking ahead. Looking down past 15 degrees it sinks
        // (fully gone past 30), and past 30 degrees the body's own arms take over.
        // A torch is always held up by the body's arm instead (no switching between the two).
        let fp_body = in_world
            && !third_person
            && self.settings.first_person_body
            && self.me.vitals.sleep.is_none()
            && !self.in_station();
        let torch = self.held() == TORCH as ItemId;
        // Where the held torch burns (for its flame particles), from whichever model shows it.
        let mut held_torch_tip = None;
        // A lantern is always held by the first-person hand (hanging with the body shown), and
        // so is a pistol (the body's arm would point it at the ground when looking down), and
        // a grenade being readied (both hands on it).
        // (and so is a bucket, hanging from the fist by its handle)
        let lantern = self.held() == LANTERN as ItemId || crate::model::bucket::is_bucket(self.held());
        // (and so is a fishing rod: both hands on it)
        let rod = self.held() == crate::item::FISHING_ROD;
        let pistol = self.holding_gun() || self.tools.grenades.hold.is_some() || rod;
        // The guide book is always held open in both first-person hands.
        let book = self.held() == crate::item::GUIDE_BOOK;
        let down = -self.me.look.pitch.to_degrees();
        let lower = &mut self.me.hand.lower;
        if !fp_body || torch || lantern || pistol || book || down <= 15.0 {
            *lower = (*lower + 8.0 * dt).min(1.0);
        } else if down < 30.0 {
            // (back up from past 30 degrees it rises to it, not at once)
            *lower = (*lower + 8.0 * dt).min(15.0 / down);
        } else {
            *lower = (*lower - 3.0 * dt).max(-0.1);
        }
        // Seen from the player's own eyes, its hands shown.
        let own_view = in_world
            && !third_person
            && !self.hud.hide
            && self.screen != Screen::Dead
            && self.me.vitals.sleep.is_none()
            && !self.in_station()
            && !self.spectator();
        // Chopping: the arms and the axe where the chop's rig has them in the world (the same
        // ones the player model shows from outside), however far down the player looks (the
        // first-person body draws the rest of it).
        if let (true, Some(swing)) = (own_view, self.me.aim.chop) {
            use crate::model::chop_rig::{emit, Parts};
            let light = crate::util::vertex_light(player_sky, player_blk);
            let fl = crate::world::mesh::flags::ENTITY;
            emit(&mut scene.particles, self.chop_world(), &swing.pose().aimed(self.chop_aim()), Parts::Arms, self.held(), self.effective_skin(), [255; 3], 0, 0.0, light, fl);
        }
        if own_view && !(fp_body && (torch || (down > 35.0 && !lantern && !pistol && !book))) {
            let f = self.me.look.dir();
            let r = f.cross(Vec3::Y).normalize();
            let u = r.cross(f);
            let cam_to_world = Mat4::from_cols(
                r.extend(0.0),
                u.extend(0.0),
                (-f).extend(0.0),
                cam.extend(1.0),
            );
            // A gun is held steady against the view's bobbing: it sways on its own (see
            // `HandAnim::build_gun`), so the sights stay where they point.
            let cam_to_world = if pistol {
                cam_to_world * self.me.look.view_bob.inverse()
            } else {
                cam_to_world
            };
            self.me.hand.fancy_lantern = fp_body;
            self.me.hand.rod = self.rod_anim();
            self.me.hand.book = self.book_view().map(|v| (self.book_read(), v));
            self.me.hand.build(
                &mut scene.viewmodel,
                cam_to_world,
                player_sky,
                player_blk,
                dt,
                self.effective_skin(),
            );
            // The hand is drawn with its own 70 degree view: move its torch tip to where the
            // world's view shows the same spot, so the flame sits on the torch.
            let k = (self.me.look.fov.to_radians() * 0.5).tan() / 35f32.to_radians().tan();
            let to_world_view = |tip: Vec3| {
                let p = cam_to_world.inverse().transform_point3(tip);
                cam_to_world.transform_point3(Vec3::new(p.x * k, p.y * k, p.z))
            };
            if let Some(tip) = self.me.hand.torch_tip {
                held_torch_tip = Some(to_world_view(tip));
            }
            scene.viewmodel_glass = std::mem::take(&mut self.me.hand.glass);
            // A direction in the hand's view turned into the world's.
            let to_world_dir = |v: Vec3| {
                let d = cam_to_world.inverse().transform_vector3(v);
                cam_to_world.transform_vector3(Vec3::new(d.x * k, d.y * k, d.z)).normalize()
            };
            // Where the gun points: its barrel, or its scope's axis when it has one.
            self.tools.guns.gun_dir = self.me.hand.barrel_dir.map(to_world_dir);
            // The scope's eyepiece: a disc on its back lens showing the scope's magnified view.
            if let Some((mid, right, up, radius)) = self.me.hand.eyepiece {
                let corner = |x: f32, y: f32| mid + right * radius * x + up * radius * y;
                let quad = [(-1.0, 1.0, [0.0, 0.0]), (1.0, 1.0, [1.0, 0.0]), (1.0, -1.0, [1.0, 1.0]), (-1.0, -1.0, [0.0, 1.0])]
                    .map(|(x, y, uv)| Vertex {
                        pos: corner(x, y).to_array(),
                        uv,
                        layer: 0.0,
                        light: [255, 255, 255, 4],
                        tint: [255, 255, 255, crate::world::mesh::flags::VIEWMODEL],
                    });
                scene.lens.extend_from_slice(&[quad[0], quad[1], quad[2], quad[0], quad[2], quad[3]]);
                // It looks exactly where the gun points: along the scope's own axis (and turned
                // with it), from the hand's view into the world's.
                let dir = to_world_dir(up.cross(right));
                let up = to_world_dir(up);
                self.tools.guns.gun_dir = Some(dir);
                // Its field of view is the one it has fully aimed, however far from the eye.
                if self.me.hand.aim > 0.97 {
                    let dist = (mid - cam).length().max(1e-3);
                    self.me.hand.scope_across = (radius / dist) / 35f32.to_radians().tan();
                }
                let across = self.me.hand.scope_across;
                let half = (across * (self.me.look.fov.to_radians() * 0.5).tan()).atan();
                // (only a gun a scope fits has one)
                let zoom = crate::item::GunKind::of(self.me.hand.held).and_then(|k| k.def().scope_zoom);
                let magnify = 1.0 / zoom.unwrap_or(1.0);
                // Seen from the scope itself, not from the eye: but never from beyond a wall
                // the eye is up against (the gun would be in it), and with its near plane
                // before whatever is right in front of it. Otherwise the near plane is further
                // out than the eye's: finer depth, so the bullet holes stay on their blocks
                // far off.
                let world = &self.terrain.world;
                let from = cam + super::player::camera::clamp_offset(world, cam, to_world_view(mid) - cam);
                let free = super::player::camera::clamp_offset(world, from, dir * 0.5).length();
                let near = (free * 0.5).clamp(0.01, 0.25);
                scene.scope = Some((from, dir, up, (2.0 * half / magnify).max(0.2f32.to_radians()), near));
            }
            // The same for the pistol's muzzle flash and the spent cases.
            self.tools.guns.muzzle = self.me.hand.muzzle_tip.map(to_world_view);
            self.tools.guns.eject = self.me.hand.eject_tip.map(to_world_view);
            self.tools.guns.chambers = self.me.hand.chamber_tips.map(|c| c.map(to_world_view));
            self.tools.guns.laser_from = self.me.hand.laser_tip.map(to_world_view);
            self.tools.guns.light_from = self.me.hand.light_tip.map(to_world_view);
            self.tools.grenades.hand_fp = self.me.hand.grenade_tip.map(to_world_view);
            self.tools.fishing.tip_fp = self.me.hand.rod_tip.map(to_world_view);
            let hit = self.me.hand.book_hit;
            self.set_book_hit(hit);
        } else {
            self.tools.guns.muzzle = None;
            self.tools.guns.eject = None;
            self.tools.guns.chambers = None;
            self.tools.guns.laser_from = None;
            self.tools.guns.light_from = None;
            self.tools.guns.gun_dir = None;
            self.tools.grenades.hand_fp = None;
            self.tools.fishing.tip_fp = None;
            self.set_book_hit(None);
        }
        if in_world {
            self.build_own_laser(&mut scene.particles, cam, view.right, view.up);
        }
        // The player model (shadow only in first person); a spectator has no body.
        // Running eases the gun across the chest (and back) on the player model.
        let run = if self.me.body.sprinting { 1.0 } else { 0.0 };
        self.me.look.tp_sprint += (run - self.me.look.tp_sprint) * (crate::util::damp(dt, 8.0));
        if in_world && self.me.body.spawned && self.screen != Screen::Dead && !self.spectator() {
            // In bed: built standing, then laid down on it.
            let bed = self.me.vitals.sleep.map(|s| {
                crate::model::player::lying(self.me.body.drawn_pos(self.clock.between), facing_dir(s.facing).as_vec3())
            });
            let (pos, head_yaw, pitch) = match bed {
                Some((feet, yaw, _)) => (feet, yaw, 0.0),
                None => (self.me.body.drawn_pos(self.clock.between), self.me.look.visual_head_yaw(), self.me.look.pitch),
            };
            let pose = PlayerPose {
                pos,
                body_yaw: self.me.look.body_yaw,
                head_yaw,
                pitch,
                limb_swing: self.me.look.limb_swing,
                limb_amount: self.me.look.limb_amount,
                attack: self.me.hand.attack(),
                crouch: self.me.body.crouch,
                sprint: self.me.look.tp_sprint,
                held: self.held(),
                skin: self.effective_skin(),
                time: self.clock.time,
                hurt: self.me.vitals.hurt_time > 0.0,
                first_person: false,
                burning: self.me.vitals.fire > 0.0,
                blocking: self.me.aim.blocking,
                hide_arms: false,
                hide_right_arm: false,
                lantern: None,
                gun_mods: self.held_gun_mods(),
                gun_dirt: self.held_gun_dirt(),
                held_data: self.me.items.held_stack().map_or(0, |s| s.data),
                gun: self.me.hand.gun_anim(),
                armor: crate::item::armor_code(&self.me.items.inventory.armor),
                book: self.book_view(),
                grenade: self.tools.grenades.hold.map(|h| h.t),
                rod: self.rod_anim(),
                chop: self.me.aim.chop,
            };
            // Where the gun's muzzle and ejection port are on the player model (third person).
            if let Some(kind) = crate::item::GunKind::of(pose.held) {
                let mods = pose.gun_mods;
                use crate::model::gun_view::{eject, light, muzzle, rest_point_in_gun_space};
                let point = |q| crate::model::player::gun_point(&pose, kind, rest_point_in_gun_space(kind, q));
                self.tools.guns.muzzle_tp = Some(point(muzzle(kind, mods)));
                self.tools.guns.eject_tp = Some(point(eject(kind)));
                self.tools.guns.light_tp = Some(point(light(kind)));
            } else {
                self.tools.guns.muzzle_tp = None;
                self.tools.guns.eject_tp = None;
                self.tools.guns.light_tp = None;
            }
            let target = limb_targets(&PlayerPose {
                first_person: fp_body,
                ..pose
            });
            let limbs = self.me.look.limbs.update(target, dt);
            // Where a readied grenade is in the model's hand (thrown from there, seen from
            // outside).
            self.tools.grenades.hand_tp = pose.grenade.map(|_| crate::model::player::held_center(&pose, &limbs));
            // Where the fishing rod's tip is on the model (the line leaves from there).
            self.tools.fishing.tip_tp = crate::model::player::rod_tip(&pose);
            // A held lantern swings from the hand.
            let lantern_dir = if crate::model::player::hangs(pose.held) {
                let pivot = hand_pivot(&pose, &limbs);
                Some(
                    self.me.look.lantern_swing
                        .update(crate::model::lantern::ON_MODEL, pivot, dt),
                )
            } else {
                self.me.look.lantern_swing = Default::default();
                None
            };
            let pose = PlayerPose {
                lantern: lantern_dir,
                ..pose
            };
            let start = scene.entity.len();
            // Only seen in third person (its shadow otherwise): so is its gun's glass.
            let mut hidden_glass = Vec::new();
            let glass = if third_person { &mut scene.translucent } else { &mut hidden_glass };
            build_player(&mut scene.entity, glass, &pose, &limbs, player_sky, player_blk);
            if let Some((feet, _, turn)) = bed {
                crate::model::player::lay_down(&mut scene.entity[start..], feet, turn);
            }
            scene.player_vertex_count = scene.entity.len();
            if torch && third_person {
                held_torch_tip = Some(crate::model::player::held_torch_tip(&pose, &limbs));
            }
            // First-person body: a headless copy drawn with the particles (which cast no shadow;
            // the full model above already does). Like the First Person Model mod, it sits
            // 0.25 blocks behind the camera (0.27 while sneaking), so looking down shows the
            // chest, legs and feet instead of the top of the shoulders.
            if fp_body {
                let back = 0.25 + 0.02 * self.me.body.crouch;
                let body_fwd = look_dir(self.me.look.body_yaw, 0.0);
                let fp = PlayerPose {
                    pos: pose.pos - body_fwd * back,
                    first_person: true,
                    // A gun is always shown by the first-person view (with its left hand).
                    hide_arms: pistol || book || (!torch && !lantern && down <= 30.0),
                    hide_right_arm: lantern || pistol || book,
                    ..pose
                };
                build_player(&mut scene.particles, &mut scene.translucent, &fp, &limbs, player_sky, player_blk);
                if torch {
                    held_torch_tip = Some(crate::model::player::held_torch_tip(&fp, &limbs));
                }
            }
        }
        self.me.look.held_torch_tip = held_torch_tip;
        if in_world {
            self.build_fishing(&mut scene.particles, cam);
        }
        if in_world {
            self.build_world_entities(&mut scene, third_person, dt);
        }
        scene.entity_visible = third_person;

        // Flame quads are blended and must not write depth or cast square shadows.
        let flame = tex::TORCH_FLAME as f32;
        let Scene {
            particles,
            entity,
            translucent,
            player_vertex_count,
            ..
        } = &mut scene;
        particles.retain(|v| {
            if v.layer == flame {
                translucent.push(*v);
                false
            } else {
                true
            }
        });
        *player_vertex_count = entity[..*player_vertex_count]
            .iter()
            .filter(|v| v.layer != flame)
            .count();
        entity.retain(|v| {
            if v.layer == flame {
                if third_person {
                    translucent.push(*v);
                }
                false
            } else {
                true
            }
        });
        scene
    }

    /// Dropped items, falling blocks, mobs, the other LAN players, chest lids and items on
    /// crafting tables near the player.
    pub(super) fn build_world_entities(&mut self, scene: &mut Scene, third_person: bool, dt: f32) {
        // Gun stations: their model, the drawer sliding out while one is used, what lies on
        // them.
        self.build_benches(if third_person { &mut scene.entity } else { &mut scene.particles }, dt);
        let world = &self.terrain.world;
        // Items and falling blocks must always be visible, so in first person they go into
        // the particle range (which is drawn normally) instead.
        let target = if third_person {
            &mut scene.entity
        } else {
            &mut scene.particles
        };
        // Only what can be seen from here: a dropped item is lost to sight past `ITEM_SIGHT`,
        // the rest past the view distance.
        const ITEM_SIGHT: f32 = 64.0;
        let sight = self.settings.render_distance * CHUNK as f32;
        let eye = self.me.body.pos;
        for it in self.level.items.iter().filter(|it| it.pos.distance_squared(eye) < ITEM_SIGHT * ITEM_SIGHT) {
            let (sky, blk) = world.light_estimate(it.pos + Vec3::Y * 0.3);
            it.build(target, self.clock.time, sky, blk);
        }
        for f in self.level.falling.iter().filter(|f| f.pos.distance_squared(eye) < sight * sight) {
            let (sky, blk) = world.light_estimate(f.pos + Vec3::Y * 0.5);
            f.build(target, sky, blk);
        }
        self.build_falling_trees(target, eye, sight);
        self.build_lying_logs(target, eye, sight);
        // Mobs always go into the entity range so they cast shadows; in first person that
        // range only draws shadows, so they are copied into the particle range to be seen too.
        let mob_verts = &mut scene.mobs;
        for m in &self.level.mobs {
            if (m.pos - self.me.body.pos).length_squared() > 128.0 * 128.0 {
                continue;
            }
            let (sky, blk) = world.light_estimate(m.center());
            m.build(mob_verts, sky, blk);
        }
        // Watching someone through their eyes: their own model would be in the way.
        let inside = self.session.spectating.filter(|_| !third_person);
        multi::build_remote_players(
            &mut self.session.remotes,
            world,
            self.clock.time,
            mob_verts,
            &mut scene.translucent,
            dt,
            inside,
        );
        let near = |p: &IVec3| (p.as_vec3() - self.me.body.pos).length_squared() < 48.0 * 48.0;
        let light = |p: IVec3| world.light_estimate(p.as_vec3() + Vec3::new(0.5, 1.2, 0.5));
        // Every chest in sight gets its lid, known contents or not (the chunk mesh has only
        // its body: a lid missing would leave it open-topped).
        let sight = (self.settings.render_distance * CHUNK as f32).powi(2);
        let in_sight = |p: &IVec3| (p.as_vec3() - self.me.body.pos).length_squared() < sight;
        for p in self.terrain.chests.values().flatten().filter(|p| in_sight(p)) {
            let b = world.geti(*p);
            if let Some(facing) = facing(b).filter(|_| is_chest(b)) {
                // Both halves of a double chest open together.
                let partner = chest_partner_offset(b);
                let lid = |q: IVec3| self.level.chest_open.get(&q).copied().unwrap_or(0.0);
                let open = lid(*p).max(partner.map_or(0.0, |d| lid(*p + d)));
                let side = chest_side(b, facing);
                let (sky, blk) = light(*p);
                build_chest_lid(target, *p, facing, side, open, sky, blk);
                if open > 0.0 {
                    // What is inside shows while it is open (lifted: under the mouse).
                    let lift = match (self.inv_ui.station_hover, &self.station) {
                        (Some(SlotRef::Chest(i)), Some(st)) => {
                            let (a, b) = self.chest_halves(st.pos);
                            if *p == a && i < 27 {
                                Some(i)
                            } else if Some(*p) == b && i >= 27 {
                                Some(i - 27)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };
                    if let Some(slots) = self.level.block_entities.chests.get(p) {
                        build_chest_items(target, *p, facing, side, &slots[..], lift, sky, blk);
                    }
                }
            }
        }
        // Doors swing open and shut over a fifth of a second.
        let world = &self.terrain.world;
        let step = dt / 0.2;
        for p in self.terrain.doors.values().flatten() {
            let b = world.geti(*p);
            if !is_door(b) {
                continue;
            }
            if !in_sight(p) {
                // (out of sight it is not drawn, but it still swings shut or open)
                let target_open = if door_open(b) { 1.0 } else { 0.0 };
                self.level.door_swing.insert(*p, target_open);
                continue;
            }
            let target_open = if door_open(b) { 1.0 } else { 0.0 };
            let s = self.level.door_swing.entry(*p).or_insert(target_open);
            *s = if *s < target_open {
                (*s + step).min(target_open)
            } else {
                (*s - step).max(target_open)
            };
            let (sky, blk) = world.light_estimate(p.as_vec3() + Vec3::splat(0.5));
            build_door(target, *p, b, *s, sky, blk);
        }
        self.level.door_swing.retain(|p, _| is_door(world.geti(*p)));
        // Meat on the furnaces, and what was put into their fronts.
        for (p, f) in self.level.block_entities.furnaces.iter().filter(|(p, _)| near(p)) {
            let b = world.geti(*p);
            if let Some(facing) = facing(b).filter(|_| is_furnace(b)) {
                let (sky, blk) = light(*p);
                // Inside it, the light in front of it (and its own fire's).
                let front = *p + facing_dir(facing);
                let (fs, fb) = world.light_estimate(front.as_vec3() + Vec3::splat(0.5));
                let inside = crate::util::vertex_light(fs, fb);
                let planes = !self.gfx.torch_particles;
                build_furnace_items(target, *p, facing, f, self.clock.time, sky, blk, inside, planes);
            }
        }
        let open_table = match self.screen {
            Screen::Container(Container::Crafting(p)) => Some(p),
            _ => None,
        };
        for (p, grid) in self.level.block_entities.tables.iter().filter(|(p, _)| near(p)) {
            if open_table != Some(*p) {
                let (sky, blk) = light(*p);
                build_table_items(target, *p, self.table_side(*p), grid, None, sky, blk);
            }
        }
        if let Some(p) = open_table {
            // The open table: its grid (lifted under the mouse), and what was crafted.
            let lift = match self.inv_ui.station_hover {
                Some(SlotRef::Craft(i)) => Some(i),
                _ => None,
            };
            let (sky, blk) = light(p);
            let side = self.table_side(p);
            build_table_items(target, p, side, &self.me.items.craft, lift, sky, blk);
            if let Some(made) = &self.me.items.craft_out {
                let (t, used) = self.me.items.craft_fx.unwrap_or((10.0, [None; 9]));
                let hovered = self.inv_ui.station_hover == Some(SlotRef::CraftOut);
                build_table_made(target, p, side, made, &used, t, hovered, sky, blk);
            }
        }
        // The highlighted slot in an open chest or on a table, or spot of a furnace.
        let glow = match self.screen {
            Screen::Container(_) if self.in_station() => self.inv_ui.station_frame,
            Screen::Playing if !self.in_station() => self.furnace_frame(),
            _ => None,
        };
        if let Some(corners) = glow {
            build_glow(&mut scene.overlay, corners);
        }
        if !third_person {
            scene.particles.extend_from_slice(&scene.mobs);
        }
        scene.entity.extend_from_slice(&scene.mobs);
    }
}
