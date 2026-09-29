//! LAN multiplayer: opening a world to LAN, joining one, and keeping everything in sync.
//!
//! The host runs the world as usual and is the authority for blocks, fluids, mobs, dropped
//! items, falling blocks and block entities. Each player moves, fights, eats and manages
//! their own inventory, and tells the host what they do to the world; the host applies it
//! with the world's rules and sends the result to everyone. A player's inventory, position,
//! health and hunger are kept by the host between visits (`saves/<world>/players/`).

mod client;
mod host;
mod lan_ui;

use super::*;

use crate::item::{armor_code, GunKind};
use crate::lang::tf;
use crate::model::player::{hand_pivot, limb_targets};
use crate::net::{
    container, pose_flags, Conn, Finder, ItemNet, Msg, PlayerState, Pose, Server, NO_BLOCK,
    PROTOCOL,
};
use crate::save::{rle, unrle};
use crate::util::lerp_angle;
use std::path::PathBuf;

/// Seconds between pose and entity updates (20 per second, Minecraft's tick rate).
const TICK: f32 = 0.05;
/// How far around a player mobs are sent; items and falling blocks a bit less.
const MOB_RANGE: f32 = 96.0;
const ITEM_RANGE: f32 = 64.0;
/// `Msg::Open` at this height means the player closed their container.
const CLOSED_Y: i32 = i32::MIN;
/// How far away other players' names show.
const NAME_RANGE: f32 = 48.0;
/// The host's player id.
pub(super) const HOST_ID: u8 = 0;

/// Another player, as drawn here.
pub(super) struct RemotePlayer {
    pub id: u8,
    pub name: String,
    /// Smoothed pose that is drawn, and the latest one received.
    pub pose: Pose,
    target: Pose,
    has_pose: bool,
    limbs: LimbSmoother,
    /// Swing of a lantern in their hand.
    lantern: crate::model::lantern::SmoothSwing,
    /// The guide book in their hands: its page turning, and how it is shown here.
    pub(in crate::game) book: crate::model::book::TurnAnim,
    pub(in crate::game) book_view: Option<crate::model::book::BookView>,
    /// When they last fired (game time), for their gun's slide.
    pub(in crate::game) shot_at: Option<f32>,
}

impl RemotePlayer {
    /// Alive and in the world according to the latest pose received (a spectator is not:
    /// mobs, items, beds and weapons leave them alone).
    pub(in crate::game) fn alive(&self) -> bool {
        self.has_pose && self.target.flags & pose_flags::DEAD == 0 && !self.target.spectator
    }

    /// Dead for now (not a spectator): they come back.
    pub(in crate::game) fn dead(&self) -> bool {
        self.has_pose && self.target.flags & pose_flags::DEAD != 0 && !self.target.spectator
    }

    /// Drawn: alive in the smoothed pose that is shown (spectators are invisible).
    pub(in crate::game) fn shown(&self) -> bool {
        self.has_pose && self.pose.flags & pose_flags::DEAD == 0 && !self.pose.spectator
    }
}

/// A connected player, on the host.
struct Peer {
    id: u8,
    name: String,
    conn: Conn,
    joined: bool,
    /// Latest saved state (inventory, health...) from the player.
    state: Option<PlayerState>,
    pose: Option<Pose>,
    /// Block entity the player has open, and what was last sent of it.
    open: Option<IVec3>,
    sent_container: Option<Vec<u8>>,
    /// Chests others have open (their contents show in them), as this player last got them.
    seen_chests: FastMap<IVec3, Vec<u8>>,
    leaving: bool,
}

pub(super) struct Host {
    server: Server,
    peers: Vec<Peer>,
    tick: f32,
    time_tick: f32,
    /// Crafting table grids as the players last got them (items lie on the tables).
    tables_sent: FastMap<IVec3, [Slot; 9]>,
    /// Furnaces as the players last got them: what changes at once (`furnace_key`), the
    /// whole message, and when it was sent.
    furnaces_sent: FastMap<IVec3, (Vec<u8>, Vec<u8>, f32)>,
    /// "192.168.1.20:25565", shown in the pause menu.
    pub address: String,
}

pub(super) struct Client {
    conn: Conn,
    pub(super) id: u8,
    tick: f32,
    /// Where the host says each dropped item is; they glide there.
    item_targets: FastMap<u32, Vec3>,
    /// The open container as last sent to or received from the host.
    container_known: Option<Vec<u8>>,
}

pub(super) enum Net {
    Host(Host),
    Client(Client),
}

fn color_bytes(c: Color) -> [u8; 4] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn color_from(b: [u8; 4]) -> Color {
    b.map(|v| v as f32 / 255.0)
}

impl Game {
    /// The host's state (read only), when hosting.
    fn host_ref(&self) -> Option<&Host> {
        match &self.net {
            Some(Net::Host(h)) => Some(h),
            _ => None,
        }
    }

    pub(super) fn is_client(&self) -> bool {
        matches!(self.net, Some(Net::Client(_)))
    }

    /// LAN player: sends a message to the host.
    pub(super) fn send(&self, m: Msg) {
        if let Some(Net::Client(c)) = &self.net {
            c.conn.send(&m);
        }
    }

    /// Host: sends a message to one player.
    pub(super) fn send_to(&self, id: u8, m: &Msg) {
        if let Some(Net::Host(h)) = &self.net {
            if let Some(p) = h.peers.iter().find(|p| p.id == id && p.joined) {
                p.conn.send(m);
            }
        }
    }

    /// Host: sends a message to every player (but `except`).
    pub(super) fn broadcast(&self, m: &Msg, except: Option<u8>) {
        if let Some(Net::Host(h)) = &self.net {
            for p in h.peers.iter().filter(|p| p.joined && Some(p.id) != except) {
                p.conn.send(m);
            }
        }
    }

    pub(super) fn entity_id(&mut self) -> u32 {
        self.next_entity_id = self.next_entity_id.wrapping_add(1).max(1);
        self.next_entity_id
    }

    /// Adds a dropped item to the world (a LAN player hands it to the host).
    pub(super) fn add_item(&mut self, mut it: ItemEntity) {
        if self.is_client() {
            self.send(Msg::DropItem {
                pos: it.pos,
                vel: it.vel,
                stack: it.stack,
                delay: it.pickup_delay,
            });
            return;
        }
        it.id = self.entity_id();
        self.items.push(it);
    }

    /// Feet of all living players: this one and the others on the LAN.
    pub(super) fn player_positions(&self) -> Vec<Vec3> {
        let mut v = Vec::new();
        if self.player.spawned && self.screen != Screen::Dead && !self.spectator() {
            v.push(self.player.pos);
        }
        v.extend(
            self.remotes
                .iter()
                .filter(|r| r.alive())
                .map(|r| r.target.pos),
        );
        v
    }

    /// Another player is lying in the bed whose head is at `head`.
    pub(super) fn remote_in_bed(&self, head: IVec3) -> bool {
        self.remotes.iter().any(|r| {
            r.alive()
                && r.target.flags & pose_flags::SLEEPING != 0
                && r.target.pos.floor().as_ivec3() == head
        })
    }

    /// Living players on the LAN other than this one: (how many, how many are asleep).
    pub(super) fn remotes_asleep(&self) -> (usize, usize) {
        let alive = self.remotes.iter().filter(|r| r.alive());
        let asleep = alive
            .clone()
            .filter(|r| r.target.flags & pose_flags::SLEEPING != 0)
            .count();
        (alive.count(), asleep)
    }

    /// The other players who are alive, and where they are.
    pub(super) fn remote_positions(&self) -> Vec<(u8, Vec3)> {
        self.remotes
            .iter()
            .filter(|r| r.alive())
            .map(|r| (r.id, r.target.pos))
            .collect()
    }

    pub(super) fn remote_pos(&self, id: u8) -> Option<Vec3> {
        self.remotes.iter().find(|r| r.id == id).map(|r| r.pose.pos)
    }

    /// This player's pose, as the others should see it.
    fn my_pose(&self) -> Pose {
        let mut flags = 0;
        if self.fire > 0.0 {
            flags |= pose_flags::BURNING;
        }
        if self.blocking {
            flags |= pose_flags::BLOCKING;
        }
        if self.hurt_time > 0.0 {
            flags |= pose_flags::HURT;
        }
        if self.screen == Screen::Dead || !self.player.spawned {
            flags |= pose_flags::DEAD;
        }
        if self.creative() {
            flags |= pose_flags::CREATIVE;
        }
        if self.sleep.is_some() {
            flags |= pose_flags::SLEEPING;
        }
        if self.holding_gun() && self.guns.aim > 0.5 {
            flags |= pose_flags::AIMING;
        }
        if self.book_showing() {
            flags |= pose_flags::SHOWING;
        }
        Pose {
            pos: self.player.pos,
            yaw: self.visual_head_yaw(),
            pitch: self.pitch,
            body_yaw: self.body_yaw,
            limb_swing: self.limb_swing,
            limb_amount: self.limb_amount,
            attack: self.hand.attack(),
            crouch: self.player.crouch,
            held: self.held(),
            skin: self.effective_skin(),
            flags,
            mining: self.mining.map(|(p, _)| p).unwrap_or(NO_BLOCK),
            mine_progress: match self.mining {
                Some((_, prog)) if !self.creative() => prog.min(1.0),
                _ => 0.0,
            },
            open: match self.screen {
                Screen::Container(c) => Self::container_pos(c).unwrap_or(NO_BLOCK),
                _ => NO_BLOCK,
            },
            status: self.my_status(),
            gun_mods: self.held_gun_mods(),
            gun_state: if self.holding_gun() { self.hand.gun_anim().pack() } else { 0 },
            armor: armor_code(&self.inventory.armor),
            book: self.book_pose().0,
            book_page: self.book_pose().1,
            spectator: self.spectator(),
            sprint: self.tp_sprint,
            gun_dirt: self.held_gun_dirt(),
            brush: self.bench_brush_pose(),
            drawer: matches!(self.screen, Screen::Container(Container::GunStation(_))) && self.bench_in_drawer,
            held_data: self.inventory.slots[self.hotbar_slot].map_or(0, |s| s.data),
            gun_extra: if self.holding_gun() { self.hand.gun_anim().pack_extra() } else { 0 },
            grenade: self.grenades.hold.map_or(0, |h| (h.t * 100.0).round().min(65000.0) as u16 + 1),
            rod: self.rod_anim(),
            bench_hold: match (self.screen, self.cursor, self.bench_hold_at) {
                (Screen::Container(Container::GunStation(_)), Some(st), Some(at)) => Some((st, at)),
                _ => None,
            },
        }
    }

    /// What this player is busy with, for the bubble above their head.
    fn my_status(&self) -> u8 {
        use crate::net::status;
        if !self.focused {
            return status::AFK;
        }
        if self.screen == Screen::Playing && self.book_status() {
            return status::READING;
        }
        match self.screen {
            Screen::Chat => status::TYPING,
            Screen::Container(_) => status::INVENTORY,
            Screen::Spectate => status::MENU,
            Screen::Paused
            | Screen::Options { in_game: true }
            | Screen::ResourcePacks { in_game: true }
            | Screen::KeyBinds { in_game: true } => status::MENU,
            _ => status::NONE,
        }
    }

    /// What the other players have open, and where they stand.
    pub(super) fn remote_open_blocks(&self) -> Vec<(IVec3, Vec3)> {
        self.remotes
            .iter()
            .filter(|r| r.has_pose && r.target.open != NO_BLOCK)
            .map(|r| (r.target.open, r.target.pos))
            .collect()
    }

    /// The other players holding a gun station's brush: the station (its left half) and where
    /// the brush is.
    /// Gun stations another player looks into the drawer of (it is out for everyone).
    pub(super) fn remote_drawers(&self) -> Vec<IVec3> {
        self.remotes
            .iter()
            .filter(|r| r.has_pose && r.target.drawer && r.target.open != NO_BLOCK)
            .map(|r| r.target.open)
            .collect()
    }

    /// What the other players hold over a gun station's table (the station, the stack, where
    /// it shows).
    pub(super) fn remote_bench_holds(&self) -> Vec<(IVec3, crate::item::Stack, Vec3)> {
        self.remotes
            .iter()
            .filter(|r| r.has_pose && r.target.open != NO_BLOCK)
            .filter_map(|r| r.target.bench_hold.map(|(st, at)| (r.target.open, st, at)))
            .collect()
    }

    pub(super) fn remote_brushes(&self) -> Vec<(IVec3, Vec3)> {
        self.remotes
            .iter()
            .filter(|r| r.has_pose && r.target.open != NO_BLOCK)
            .filter_map(|r| r.target.brush.map(|b| (r.target.open, b)))
            .collect()
    }

    /// Chests the other players have open (their lids open here too).
    pub(super) fn remote_open_chests(&self) -> Vec<IVec3> {
        self.remotes
            .iter()
            .filter(|r| r.has_pose && r.target.open != NO_BLOCK)
            .map(|r| r.target.open)
            .filter(|p| is_chest(self.terrain.world.geti(*p)))
            .collect()
    }

    /// Torches and lanterns in the other players' hands: (id, where the light is, item).
    pub(super) fn remote_held_lights(&self) -> Vec<(u8, Vec3, ItemId)> {
        self.remotes
            .iter()
            .filter(|r| r.shown())
            .filter(|r| crate::model::player::held_up(r.pose.held))
            .map(|r| {
                // Like this player's own: just below the eyes, where the hand holds it up.
                let eye = 1.62 - 0.35 * r.pose.crouch;
                (r.id, r.pose.pos + Vec3::Y * (eye - 0.35), r.pose.held)
            })
            .collect()
    }

    /// Crack overlays of the blocks the other players are mining.
    pub(super) fn remote_cracks(&self) -> Vec<(IVec3, f32)> {
        self.remotes
            .iter()
            .filter(|r| r.has_pose && r.target.mine_progress > 0.02 && r.target.mining != NO_BLOCK)
            .map(|r| (r.target.mining, r.target.mine_progress))
            .collect()
    }

    /// Debris from a block broken by someone: shown here if `local`, and the host sends it to
    /// the other players (but `except`, who broke it).
    pub(super) fn break_fx(&mut self, p: IVec3, block: u8, local: bool, except: Option<u8>) {
        if block == AIR {
            return;
        }
        if local {
            let tint = self.block_tint(p, block);
            self.particles
                .burst(&self.terrain.world, p, block, 28, tint);
        }
        if self.is_host() {
            self.broadcast(&Msg::BreakFx { p, block }, except);
        }
    }

    pub(super) fn is_host(&self) -> bool {
        matches!(self.net, Some(Net::Host(_)))
    }

    /// Hit by another player (or blown about by a grenade they threw, or bitten by a wolf):
    /// damage and knockback.
    pub(super) fn hit_by_player(&mut self, dmg: f32, from: Vec3, knock: f32, kind: u8) {
        if kind == crate::net::hurt::BLAST {
            self.blast_hit(dmg, from, knock);
            return;
        }
        let dmg = self.armor_hit(dmg, kind);
        let before = self.health;
        let cause = if kind == crate::net::hurt::WOLF { "death.wolf" } else { "death.player" };
        self.damage(dmg, cause);
        if self.health < before {
            let away = (self.player.pos - from) * Vec3::new(1.0, 0.0, 1.0);
            let away = away.try_normalize().unwrap_or(Vec3::X);
            self.player.vel.x = away.x * 6.0 * knock;
            self.player.vel.z = away.z * 6.0 * knock;
            self.player.vel.y = self.player.vel.y.max(5.0);
        }
    }

    // ------------------------------------------------------------------ every frame

    /// Network work for this frame (host or player).
    pub(super) fn net_tick(&mut self, dt: f32) {
        match self.net {
            Some(Net::Host(_)) => self.host_tick(dt),
            Some(Net::Client(_)) => self.client_tick(dt),
            None => {}
        }
        // Other players glide toward their latest pose.
        let k = 1.0 - (-15.0 * dt).exp();
        for r in &mut self.remotes {
            let (p, t) = (&mut r.pose, r.target);
            p.pos = if p.pos.distance_squared(t.pos) > 64.0 {
                t.pos
            } else {
                p.pos.lerp(t.pos, k)
            };
            p.yaw = lerp_angle(p.yaw, t.yaw, k);
            p.body_yaw = lerp_angle(p.body_yaw, t.body_yaw, k);
            p.pitch += (t.pitch - p.pitch) * k;
            p.limb_swing += (t.limb_swing - p.limb_swing) * k;
            p.limb_amount += (t.limb_amount - p.limb_amount) * k;
            p.crouch += (t.crouch - p.crouch) * k;
            p.attack = t.attack;
            p.held = t.held;
            p.flags = t.flags;
            p.status = t.status;
            p.gun_mods = t.gun_mods;
            p.gun_state = t.gun_state;
            // (the held gun as it is now: its dirt, its magazine's rounds or cylinder, and
            // what it is doing)
            p.gun_dirt = t.gun_dirt;
            p.held_data = t.held_data;
            p.gun_extra = t.gun_extra;
            p.armor = t.armor;
            p.book = t.book;
            p.book_page = t.book_page;
            p.spectator = t.spectator;
            // (a readied grenade goes on smoothly between the poses)
            p.grenade = match (p.grenade, t.grenade) {
                (_, 0) => 0,
                (0, g) => g,
                (g, n) => g.saturating_add((dt * 100.0).round() as u16).clamp(n.saturating_sub(10), n.saturating_add(10)),
            };
            // (a fishing rod's swings go on smoothly between the poses, its bobber glides)
            p.rod = match (p.rod, t.rod) {
                (Some(mut r), Some(n)) => {
                    let run = |a: Option<f32>, b: Option<f32>| match (a, b) {
                        (Some(a), Some(b)) => Some((a + dt).clamp(b - 0.1, b + 0.1)),
                        (_, b) => b,
                    };
                    r.cast = run(r.cast, n.cast);
                    r.lift = run(r.lift, n.lift);
                    r.charge += (n.charge - r.charge) * k;
                    r.fight += (n.fight - r.fight) * k;
                    r.tension += (n.tension - r.tension) * k;
                    r.crank = lerp_angle(r.crank, n.crank, k);
                    r.bobber = match (r.bobber, n.bobber) {
                        (Some(a), Some(b)) if a.distance_squared(b) < 25.0 => Some(a.lerp(b, k)),
                        (_, b) => b,
                    };
                    r.out = n.out;
                    Some(r)
                }
                (_, n) => n,
            };
        }
    }

    fn set_remote_pose(&mut self, id: u8, pose: Pose) {
        if let Some(r) = self.remotes.iter_mut().find(|r| r.id == id) {
            if !r.has_pose {
                r.pose = pose;
            }
            r.target = pose;
            r.has_pose = true;
        }
    }

    /// Another player left: their model and skin go.
    fn remove_remote(&mut self, id: u8) {
        self.remotes.retain(|r| r.id != id);
        self.custom_skins.remove(&id);
        self.skin_pngs.remove(&id);
    }

    fn add_remote(&mut self, id: u8, name: String) {
        self.remotes.retain(|r| r.id != id);
        self.remotes.push(RemotePlayer {
            id,
            name,
            pose: Pose::default(),
            target: Pose::default(),
            has_pose: false,
            limbs: LimbSmoother::default(),
            lantern: Default::default(),
            book: Default::default(),
            book_view: None,
            shot_at: None,
        });
    }
}

/// What another player's held gun is doing, as they sent it: its slide, reload and aim
/// (`Pose::gun_state`, `gun_extra`), and from the gun itself (`held_data`) the rounds in its
/// magazine or what is in each chamber of its cylinder.
fn remote_gun(p: &Pose, time: f32, shot_at: Option<f32>) -> crate::model::pistol_view::GunAnim {
    let mut g = crate::model::pistol_view::GunAnim::unpack(
        p.gun_state,
        shot_at.map(|at| time - at).filter(|&t| (0.0..1.0).contains(&t)),
    );
    g.unpack_extra(p.gun_extra);
    let gun = crate::item::Stack { data: p.held_data, ..crate::item::Stack::one(p.held) };
    match GunKind::of(p.held) {
        Some(GunKind::Revolver) => g.cyl = p.held_data,
        Some(kind) => {
            g.mag = crate::item::gun_has_mag(&gun)
                .then(|| (crate::item::gun_rounds(&gun), kind.magazine_size(crate::item::gun_mods(&gun))));
            // (the magazine a reload brings is sent as the pistol's, 12 or 20: another gun's
            // holds what its own does)
            if kind != GunKind::Pistol {
                g.new_mag = g.new_mag.map(|(n, _)| (n, kind.magazine_size(0)));
            }
        }
        None => {}
    }
    g
}

/// Other players' models (into the entity and particle ranges, like mobs).
/// Another player's model, standing as their pose says (`shot_at`: when they last fired).
fn standing_pose(p: &Pose, time: f32, shot_at: Option<f32>) -> PlayerPose {
    PlayerPose {
        pos: p.pos,
        body_yaw: p.body_yaw,
        head_yaw: p.yaw,
        pitch: p.pitch,
        limb_swing: p.limb_swing,
        limb_amount: p.limb_amount,
        attack: p.attack,
        crouch: p.crouch,
        sprint: p.sprint,
        held: p.held,
        skin: p.skin,
        time,
        hurt: p.flags & pose_flags::HURT != 0,
        first_person: false,
        burning: p.flags & pose_flags::BURNING != 0,
        blocking: p.flags & pose_flags::BLOCKING != 0,
        hide_arms: false,
        hide_right_arm: false,
        lantern: None,
        gun_mods: p.gun_mods,
        gun_dirt: p.gun_dirt,
        held_data: p.held_data,
        gun: remote_gun(p, time, shot_at),
        armor: p.armor,
        book: None,
        grenade: (p.grenade > 0).then(|| (p.grenade - 1) as f32 / 100.0),
        rod: p.rod,
    }
}

impl Game {
    /// The other players holding a gun (and not in bed): id, the gun, its attachments,
    /// their eye and where they look.
    pub(super) fn remote_guns(&self) -> Vec<(u8, GunKind, u8, Vec3, Vec3)> {
        self.remotes
            .iter()
            .filter(|r| r.shown() && r.pose.flags & pose_flags::SLEEPING == 0)
            .filter_map(|r| {
                let kind = GunKind::of(r.pose.held)?;
                let eye = r.pose.pos + Vec3::Y * (1.62 - 0.35 * r.pose.crouch);
                Some((r.id, kind, r.pose.gun_mods, eye, look_dir(r.pose.yaw, r.pose.pitch)))
            })
            .collect()
    }

    /// Where a point of another player's gun (Blockbench model: bone and point) is on their
    /// model.
    pub(super) fn remote_gun_point(&self, id: u8, kind: GunKind, point: (usize, Vec3)) -> Option<Vec3> {
        let r = self.remotes.iter().find(|r| r.id == id && r.shown())?;
        let pose = standing_pose(&r.pose, self.time, r.shot_at);
        let point = crate::model::gun_view::rest_point_in_gun_space(kind, point);
        Some(crate::model::player::gun_point(&pose, kind, point))
    }
}

impl Game {
    /// The other players fishing: where the tip of their rod is and what it is doing (for
    /// their line and bobber).
    pub(super) fn remote_rods(&self) -> Vec<(Vec3, crate::model::angler::RodAnim)> {
        self.remotes
            .iter()
            .filter(|r| r.shown() && r.pose.flags & pose_flags::SLEEPING == 0)
            .filter_map(|r| {
                let rod = r.pose.rod.filter(|_| r.pose.held == crate::item::FISHING_ROD)?;
                let pose = standing_pose(&r.pose, self.time, r.shot_at);
                Some((crate::model::player::rod_tip(&pose)?, rod))
            })
            .collect()
    }
}

/// `inside`: the player a spectator watches through their eyes (not drawn).
pub(super) fn build_remote_players(
    remotes: &mut [RemotePlayer],
    world: &World,
    time: f32,
    out: &mut Vec<Vertex>,
    glass: &mut Vec<Vertex>,
    dt: f32,
    inside: Option<u8>,
) {
    {
        for r in remotes.iter_mut() {
            let p = r.pose;
            if !r.shown() || Some(r.id) == inside {
                continue;
            }
            // In a bed: the pose's position is on top of the head half, and the body faces
            // the foot end.
            let bed = (p.flags & pose_flags::SLEEPING != 0).then(|| {
                let d = -look_dir(p.body_yaw, 0.0);
                let head = facing_dir(facing_of(d.x, d.z)).as_vec3();
                crate::model::player::lying(p.pos, head)
            });
            let standing = standing_pose(&p, time, r.shot_at);
            let pose = match bed {
                Some((feet, yaw, _)) => PlayerPose {
                    pos: feet,
                    body_yaw: yaw,
                    head_yaw: yaw,
                    pitch: 0.0,
                    limb_amount: 0.0,
                    ..standing
                },
                None => standing,
            };
            // Their open book, at their page (see `update_book_views`).
            let pose = PlayerPose {
                book: r.book_view.filter(|_| bed.is_none()),
                ..pose
            };
            let limbs = r.limbs.update(limb_targets(&pose), dt);
            let lantern = if pose.held == LANTERN as ItemId {
                Some(r.lantern.update(
                    crate::model::lantern::ON_MODEL,
                    hand_pivot(&pose, &limbs),
                    dt,
                ))
            } else {
                r.lantern = Default::default();
                None
            };
            let pose = PlayerPose { lantern, ..pose };
            let c = p.pos + Vec3::Y;
            let start = out.len();
            build_player(
                out,
                glass,
                &pose,
                &limbs,
                world.sky_estimate(c),
                world.block_light_estimate(c),
            );
            if let Some((feet, _, turn)) = bed {
                crate::model::player::lay_down(&mut out[start..], feet, turn);
            }
        }
    }
}

impl Game {
    /// The other player the crosshair is on, and how far away.
    pub(super) fn pick_player(&self, eye: Vec3, dir: Vec3, reach: f32) -> Option<(u8, f32)> {
        self.pick_other_player(eye, dir, reach, None)
    }

    /// `pick_player`, leaving out `except` (the one the ray starts from).
    pub(super) fn pick_other_player(
        &self,
        eye: Vec3,
        dir: Vec3,
        reach: f32,
        except: Option<u8>,
    ) -> Option<(u8, f32)> {
        self.remotes
            .iter()
            .filter(|r| r.alive() && Some(r.id) != except)
            .filter_map(|r| {
                let p = r.pose.pos;
                let half = Vec3::new(0.4, 0.0, 0.4);
                crate::util::ray_box(
                    eye,
                    dir,
                    p - half - Vec3::Y * 0.1,
                    p + half + Vec3::Y * 1.9,
                    reach,
                )
                .map(|d| (r.id, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// A chat line typed by this player.
    pub(super) fn chat_line(&mut self, text: &str) {
        match &self.net {
            Some(Net::Client(_)) => self.send(Msg::Chat {
                text: text.to_string(),
                color: color_bytes(chat::WHITE),
            }),
            Some(Net::Host(_)) => {
                let line = format!("<{}> {text}", self.settings.name);
                self.broadcast(
                    &Msg::Chat {
                        text: line.clone(),
                        color: color_bytes(chat::WHITE),
                    },
                    None,
                );
                self.say(line, chat::WHITE);
            }
            None => {
                let line = format!("<{}> {text}", self.settings.name);
                self.say(line, chat::WHITE);
            }
        }
    }

    /// Host: a chat message to everyone (and here).
    fn announce(&mut self, text: String, color: Color) {
        self.broadcast(
            &Msg::Chat {
                text: text.clone(),
                color: color_bytes(color),
            },
            None,
        );
        self.say(text, color);
    }

    /// Contents of the block entity at `p` as a message (a LAN player's open crafting table
    /// uses its live grid).
    fn container_msg(&self, p: IVec3) -> Option<Msg> {
        let b = self.terrain.world.geti(p);
        // Whoever has the table open here (host or player) works on the live grid.
        let table_open = matches!(self.screen, Screen::Container(Container::Crafting(q)) if q == p);
        let (kind, slots) = if is_chest(b) {
            // A double chest sends both halves (54 slots).
            self.block_entities.chests.get(&p)?;
            (container::CHEST, self.chest_slots(p))
        } else if b == CRAFTING_TABLE {
            let grid = if table_open {
                self.craft
            } else {
                self.block_entities
                    .tables
                    .get(&p)
                    .copied()
                    .unwrap_or([None; 9])
            };
            (container::TABLE, grid.to_vec())
        } else {
            return None;
        };
        Some(Msg::Container { p, kind, slots })
    }

    /// Stores received contents.
    fn apply_container(&mut self, p: IVec3, kind: u8, slots: &[Slot]) {
        let get = |i: usize| slots.get(i).copied().flatten();
        match kind {
            container::CHEST => {
                let n = if self.chest_halves(p).1.is_some() {
                    54
                } else {
                    27
                };
                let all: Vec<Slot> = (0..n).map(get).collect();
                self.set_chest_slots(p, &all);
            }
            container::TABLE => {
                let grid: [Slot; 9] = std::array::from_fn(get);
                let open_here =
                    matches!(self.screen, Screen::Container(Container::Crafting(q)) if q == p);
                if open_here {
                    self.craft = grid;
                } else if grid.iter().any(|s| s.is_some()) {
                    self.block_entities.tables.insert(p, grid);
                } else {
                    self.block_entities.tables.remove(&p);
                }
            }
            _ => {}
        }
    }

    /// What of a furnace changes all at once and must reach the players right away: its
    /// contents, whether it burns, how done each side of the meat is and whether it is being
    /// turned over. (The seconds in between go out now and then; players count them on.)
    fn furnace_key(f: &crate::entity::Furnace) -> Vec<u8> {
        use crate::entity::block_entity::doneness;
        let contents = crate::entity::Furnace {
            burn: 0.0,
            cook: 0.0,
            grill: [None; 4],
            ..f.clone()
        };
        let mut key = Self::furnace_msg(IVec3::ZERO, &contents).encode();
        key.push((f.burn > 0.0) as u8);
        for g in &f.grill {
            key.push(match g {
                None => 255,
                Some(g) => {
                    let side = |t: f32| doneness(t) as u8;
                    side(g.cook[0]) * 16 + side(g.cook[1]) * 4 + g.down * 2 + (g.flip > 0.0) as u8
                }
            });
        }
        key
    }

    /// A furnace as everyone sees it.
    pub(super) fn furnace_msg(p: IVec3, f: &crate::entity::Furnace) -> Msg {
        Msg::Furnace {
            p,
            burn: f.burn,
            cook: f.cook,
            input: f.input,
            fuel: f.fuel,
            output: f.output,
            grill: f
                .grill
                .iter()
                .enumerate()
                .filter_map(|(i, g)| g.map(|g| (i as u8, g)))
                .collect(),
        }
    }

    /// LAN player: the host's furnace (only for showing it; the host runs it).
    #[allow(clippy::too_many_arguments)]
    fn apply_furnace(
        &mut self,
        p: IVec3,
        burn: f32,
        cook: f32,
        [input, fuel, output]: [Slot; 3],
        grill: Vec<(u8, crate::entity::Grilled)>,
    ) {
        let f = self.block_entities.furnaces.entry(p).or_default();
        f.burn = burn;
        f.cook = cook;
        f.input = input;
        f.fuel = fuel;
        f.output = output;
        f.grill = [None; 4];
        for (i, g) in grill {
            f.grill[i as usize & 3] = Some(g);
        }
    }

    /// LAN player opened a container: the host sends its contents.
    pub(super) fn net_container_opened(&mut self, p: IVec3) {
        if let Some(Net::Client(c)) = &mut self.net {
            c.container_known = None;
            c.conn.send(&Msg::Open { p });
        }
    }

    pub(super) fn net_container_closed(&mut self) {
        if let Some(Net::Client(c)) = &mut self.net {
            c.container_known = None;
            c.conn.send(&Msg::Open {
                p: IVec3::new(0, CLOSED_Y, 0),
            });
        }
    }

    /// LAN player: sends the open container if this player changed it.
    pub(super) fn net_container_sync(&mut self) {
        if !self.is_client() {
            return;
        }
        let Screen::Container(c) = self.screen else {
            return;
        };
        let Some(p) = Self::container_pos(c) else {
            return;
        };
        let Some(msg) = self.container_msg(p) else {
            return;
        };
        let bytes = msg.encode();
        if let Some(Net::Client(c)) = &mut self.net {
            // Nothing known yet: wait for the host's copy instead of overwriting it.
            if c.container_known.as_ref().is_some_and(|k| *k != bytes) {
                c.conn.send(&msg);
                c.container_known = Some(bytes);
            }
        }
    }
}
