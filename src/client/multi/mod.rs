//! The game's side of playing in a world: it is always a player connected to the world's
//! server (`sim::server`), the one this game runs for its own world or a LAN game's.
//!
//! The server is the authority for blocks, fluids, mobs, dropped items, falling blocks and
//! block entities. This game moves, fights, eats and manages the player's own inventory, and
//! tells the server what the player does to the world; it shows that at once and takes what
//! the server sends (`client`). The other players are drawn from the poses they send.

mod client;
mod lan_ui;
mod testbed;

use crate::client::{AUTOSAVE_SECONDS, Container, Game, Screen};
use crate::entity::ItemEntity;
use crate::entity::player::look_dir;
use crate::item::{GunKind, ItemId, Slot, armor_code};
use crate::model::players::player::{LimbSmoother, PlayerPose, build_player, hand_pivot, limb_targets};
use crate::net::{Conn, Msg, NO_BLOCK, Pose, container, pose_flags};
use crate::world::save::PlayerSave;
use crate::ui::{Color, chat};
use crate::util::lerp_angle;
use crate::world::{AIR, Block, FastMap, World, facing_dir, facing_of, is_chest};
use crate::world::mesh::Vertex;
use glam::{IVec3, Vec3};

/// `Msg::Open` at this height means the player closed their container.
const CLOSED_Y: i32 = i32::MIN;
/// How far away other players' names show.
const NAME_RANGE: f32 = 48.0;
/// The world's owner's player id (the one whose game runs its server).
pub(super) const OWNER_ID: u8 = 0;

/// Another player, as drawn here.
pub(super) struct RemotePlayer {
    pub id: u8,
    pub name: String,
    /// Smoothed pose that is drawn, and the latest one received.
    pub pose: Pose,
    target: Pose,
    /// The glide from the pose drawn to the latest (an even pace from one to the next).
    glide: crate::util::Glide,
    has_pose: bool,
    limbs: LimbSmoother,
    /// Swing of a lantern in their hand.
    lantern: crate::model::items::lantern::SmoothSwing,
    /// The guide book in their hands: its page turning, and how it is shown here.
    pub(super) book: crate::model::items::book::TurnAnim,
    pub(super) book_view: Option<crate::model::items::book::BookView>,
    /// When they last fired (game time), for their gun's slide.
    pub(super) shot_at: Option<f32>,
}

impl RemotePlayer {
    /// Alive and in the world according to the latest pose received (a spectator is not:
    /// mobs, items, beds and weapons leave them alone).
    pub(super) fn alive(&self) -> bool {
        self.has_pose && self.target.flags & pose_flags::DEAD == 0 && !self.target.spectator
    }

    /// Dead for now (not a spectator): they come back.
    pub(super) fn dead(&self) -> bool {
        self.has_pose && self.target.flags & pose_flags::DEAD != 0 && !self.target.spectator
    }

    /// Drawn: alive in the smoothed pose that is shown (spectators are invisible).
    fn shown(&self) -> bool {
        self.has_pose && self.pose.flags & pose_flags::DEAD == 0 && !self.pose.spectator
    }
}

pub(super) struct Client {
    conn: Conn,
    id: u8,
    tick: f32,
    /// Where the server says each dropped item is; they glide there.
    item_targets: FastMap<u32, (Vec3, crate::util::Glide)>,
    /// The open container as last sent to or received from the server.
    container_known: Option<Vec<u8>>,
    /// Dropped items flying to whoever picked them up (item, player).
    collecting: FastMap<u32, u8>,
    /// Where the server says each falling block is (in the order of `Level::falling`), and
    /// when it last said.
    falling_targets: Vec<Vec3>,
    falling_at: f32,
}

/// Playing in a world: the connection to its server (and the server itself when it is the
/// game's own), the other players in it, and what came from it to wait for the world.
/// (The fields drop in their order: the connection closes before the server stops.)
pub(super) struct Session {
    /// The connection to the world's server (the game's own, or a LAN game's), while in a
    /// world.
    pub(super) net: Option<Client>,
    /// The server this game runs for the world it plays (joined through `net`), and its LAN
    /// address once it is open to the LAN.
    pub(super) local: Option<crate::sim::server::Local>,
    pub(super) lan_address: Option<String>,
    pub(super) remotes: Vec<RemotePlayer>,
    /// Spectator mode: the player whose eyes the camera is in.
    pub(super) spectating: Option<u8>,
    /// The player as the server keeps them, till the world around them has loaded.
    pub(super) pending_player: Option<PlayerSave>,
    /// Till the player's state is sent to be saved again.
    autosave: f32,
}

impl Session {
    /// This game's own world, not open to LAN, nobody else in it: pausing stops it (the
    /// server stands still too).
    pub(in crate::client) fn stands_still(&self) -> bool {
        self.local.is_some() && self.lan_address.is_none() && self.remotes.is_empty()
    }

    pub(super) fn new() -> Self {
        Self {
            net: None,
            local: None,
            lan_address: None,
            remotes: Vec::new(),
            spectating: None,
            pending_player: None,
            autosave: AUTOSAVE_SECONDS,
        }
    }

    /// A new world (on the same connection, or none): all but the connection made anew (the
    /// other players and what was waiting for the last world go).
    pub(super) fn forget_world(&mut self) {
        *self = Self {
            net: self.net.take(),
            local: self.local.take(),
            lan_address: self.lan_address.take(),
            ..Self::new()
        };
    }

    /// This player's id on the server (0 out of a world).
    pub(super) fn my_id(&self) -> u8 {
        self.net.as_ref().map_or(0, |c| c.id)
    }
}

fn color_bytes(c: Color) -> [u8; 4] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn color_from(b: [u8; 4]) -> Color {
    b.map(|v| v as f32 / 255.0)
}

impl Game {
    /// Sends a message to the server.
    pub(super) fn send(&self, m: Msg) {
        if let Some(c) = &self.session.net {
            // The server checks what a player does against where they stand and what they
            // hold: it gets the pose as it is right now first.
            let checked = matches!(
                m,
                Msg::Place { .. }
                    | Msg::Break { .. }
                    | Msg::AttackMob { .. }
                    | Msg::AttackPlayer { .. }
                    | Msg::Shot { .. }
                    | Msg::Grenade { .. }
                    | Msg::DropItem { .. }
                    | Msg::SpawnMob { .. }
                    | Msg::Notch { .. }
                    | Msg::Stump { .. }
                    | Msg::CutLog { .. }
            );
            if checked && self.me.body.spawned {
                c.conn.send(&Msg::Pose(self.my_pose()));
            }
            c.conn.send(&m);
        }
    }

    /// Drops an item into the world (the server puts it there).
    pub(super) fn add_item(&mut self, it: ItemEntity) {
        self.send(Msg::DropItem {
            pos: it.pos,
            vel: it.vel,
            stack: it.stack,
            delay: it.pickup_delay,
        });
    }

    /// Another player is lying in the bed whose head is at `head`.
    pub(super) fn remote_in_bed(&self, head: IVec3) -> bool {
        self.session.remotes.iter().any(|r| {
            r.alive()
                && r.target.flags & pose_flags::SLEEPING != 0
                && r.target.pos.floor().as_ivec3() == head
        })
    }

    /// Living players on the LAN other than this one: (how many, how many are asleep).
    pub(super) fn remotes_asleep(&self) -> (usize, usize) {
        let alive = self.session.remotes.iter().filter(|r| r.alive());
        let asleep = alive
            .clone()
            .filter(|r| r.target.flags & pose_flags::SLEEPING != 0)
            .count();
        (alive.count(), asleep)
    }

    pub(super) fn remote_pos(&self, id: u8) -> Option<Vec3> {
        self.session.remotes.iter().find(|r| r.id == id).map(|r| r.pose.pos)
    }

    /// This player's pose, as the others should see it.
    fn my_pose(&self) -> Pose {
        let mut flags = 0;
        if self.me.vitals.fire > 0.0 {
            flags |= pose_flags::BURNING;
        }
        if self.me.aim.blocking {
            flags |= pose_flags::BLOCKING;
        }
        if self.me.vitals.hurt_time > 0.0 {
            flags |= pose_flags::HURT;
        }
        if self.screen == Screen::Dead || !self.me.body.spawned {
            flags |= pose_flags::DEAD;
        }
        if self.creative() {
            flags |= pose_flags::CREATIVE;
        }
        if self.me.vitals.sleep.is_some() {
            flags |= pose_flags::SLEEPING;
        }
        if self.holding_gun() && self.tools.guns.aim > 0.5 {
            flags |= pose_flags::AIMING;
        }
        if self.book_showing() {
            flags |= pose_flags::SHOWING;
        }
        Pose {
            // (where it is drawn, as its limbs are: sent 20 times a second from frames, the
            // place after the last tick would move on in uneven steps)
            pos: self.me.body.between_pos(self.clock.between),
            yaw: self.me.look.visual_head_yaw(),
            pitch: self.me.look.pitch,
            body_yaw: self.me.look.body_yaw,
            limb_swing: self.me.look.limb_swing,
            limb_amount: self.me.look.limb_amount,
            attack: self.me.hand.attack(),
            crouch: self.me.body.drawn_crouch(self.clock.between),
            held: self.held(),
            skin: self.effective_skin(),
            flags,
            mining: self.me.aim.mining.map(|(p, _)| p).unwrap_or(NO_BLOCK),
            mine_progress: match self.me.aim.mining {
                Some((_, prog)) if !self.creative() => prog.min(1.0),
                _ => 0.0,
            },
            open: match self.screen {
                Screen::Container(c) => Self::container_pos(c).unwrap_or(NO_BLOCK),
                _ => NO_BLOCK,
            },
            status: self.my_status(),
            gun_mods: self.held_gun_mods(),
            gun_state: if self.holding_gun() { self.me.hand.gun_anim().pack() } else { 0 },
            armor: armor_code(&self.me.items.inventory.armor),
            book: self.book_pose().0,
            book_page: self.book_pose().1,
            spectator: self.spectator(),
            sprint: self.me.look.tp_sprint,
            gun_dirt: self.held_gun_dirt(),
            brush: self.bench_brush_pose(),
            drawer: matches!(self.screen, Screen::Container(Container::GunStation(_))) && self.bench_ui.in_drawer,
            held_data: self.me.items.held_stack().map_or(0, |s| s.data),
            gun_extra: if self.holding_gun() { self.me.hand.gun_anim().pack_extra() } else { 0 },
            grenade: self.tools.grenades.hold.map_or(0, |h| (h.t * 100.0).round().min(65000.0) as u16 + 1),
            rod: self.rod_anim(),
            chop: self.me.aim.chop,
            bench_hold: match (self.screen, self.me.items.cursor, self.bench_ui.hold_at) {
                (Screen::Container(Container::GunStation(_)), Some(st), Some(at)) => Some((st, at)),
                _ => None,
            },
        }
    }

    /// What this player is busy with, for the bubble above their head.
    fn my_status(&self) -> u8 {
        use crate::net::status;
        if !self.input.focused {
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
        self.session.remotes
            .iter()
            .filter(|r| r.has_pose && r.target.open != NO_BLOCK)
            .map(|r| (r.target.open, r.target.pos))
            .collect()
    }

    /// The other players holding a gun station's brush: the station (its left half) and where
    /// the brush is.
    /// Gun stations another player looks into the drawer of (it is out for everyone).
    pub(super) fn remote_drawers(&self) -> Vec<IVec3> {
        self.session.remotes
            .iter()
            .filter(|r| r.has_pose && r.target.drawer && r.target.open != NO_BLOCK)
            .map(|r| r.target.open)
            .collect()
    }

    /// What the other players hold over a gun station's table (the station, the stack, where
    /// it shows).
    pub(super) fn remote_bench_holds(&self) -> Vec<(IVec3, crate::item::Stack, Vec3)> {
        self.session.remotes
            .iter()
            .filter(|r| r.has_pose && r.target.open != NO_BLOCK)
            .filter_map(|r| r.target.bench_hold.map(|(st, at)| (r.target.open, st, at)))
            .collect()
    }

    pub(super) fn remote_brushes(&self) -> Vec<(IVec3, Vec3)> {
        self.session.remotes
            .iter()
            .filter(|r| r.has_pose && r.target.open != NO_BLOCK)
            .filter_map(|r| r.target.brush.map(|b| (r.target.open, b)))
            .collect()
    }

    /// Chests the other players have open (their lids open here too).
    pub(super) fn remote_open_chests(&self) -> Vec<IVec3> {
        self.session.remotes
            .iter()
            .filter(|r| r.has_pose && r.target.open != NO_BLOCK)
            .map(|r| r.target.open)
            .filter(|p| is_chest(self.terrain.world.geti(*p)))
            .collect()
    }

    /// Torches and lanterns in the other players' hands: (id, where the light is, item).
    pub(super) fn remote_held_lights(&self) -> Vec<(u8, Vec3, ItemId)> {
        self.session.remotes
            .iter()
            .filter(|r| r.shown())
            .filter(|r| crate::model::players::player::gives_light(r.pose.held))
            .map(|r| {
                // Like this player's own: just below the eyes, where the hand holds it up.
                let eye = 1.62 - 0.35 * r.pose.crouch;
                (r.id, r.pose.pos + Vec3::Y * (eye - 0.35), r.pose.held)
            })
            .collect()
    }

    /// Crack overlays of the blocks the other players are mining.
    pub(super) fn remote_cracks(&self) -> Vec<(IVec3, f32)> {
        self.session.remotes
            .iter()
            .filter(|r| r.has_pose && r.target.mine_progress > 0.02 && r.target.mining != NO_BLOCK)
            .map(|r| (r.target.mining, r.target.mine_progress))
            .collect()
    }

    /// Debris from a block another player broke.
    fn break_fx(&mut self, p: IVec3, block: Block) {
        if block == AIR {
            return;
        }
        let tint = self.block_tint(p, block);
        self.level.particles.burst(&self.terrain.world, p, block, 28, tint);
    }

    /// Hit by another player (or blown about by a grenade they threw, or bitten by a wolf):
    /// damage and knockback.
    fn hit_by_player(&mut self, dmg: f32, from: Vec3, knock: f32, kind: u8) {
        if kind == crate::net::hurt::BLAST {
            self.blast_hit(dmg, from, knock);
            return;
        }
        let dmg = self.armor_hit(dmg, kind);
        let before = self.me.vitals.health;
        let cause = if kind == crate::net::hurt::WOLF { "death.wolf" } else { "death.player" };
        self.damage(dmg, cause);
        if self.me.vitals.health < before {
            let away = (self.me.body.pos - from) * Vec3::new(1.0, 0.0, 1.0);
            let away = away.try_normalize().unwrap_or(Vec3::X);
            self.me.body.vel.x = away.x * 6.0 * knock;
            self.me.body.vel.z = away.z * 6.0 * knock;
            self.me.body.vel.y = self.me.body.vel.y.max(5.0);
        }
    }

    // ------------------------------------------------------------------ every frame

    /// Network work for this frame: messages in and out, the other players' poses.
    pub(super) fn net_tick(&mut self, dt: f32) {
        self.poll_joining();
        self.client_tick(dt);
        // Other players glide toward their latest pose (the rod's fight and the like still
        // ease toward theirs).
        let k = crate::util::damp(15.0, dt);
        let mut chops = Vec::new();
        for r in &mut self.session.remotes {
            let g = r.glide.step(dt);
            let (p, t) = (&mut r.pose, r.target);
            // (their axe biting in: heard where it is, about an arm ahead of them)
            if let (Some(None), Some(Some(_))) = (p.chop.map(|s| s.hit), t.chop.map(|s| s.hit)) {
                let ahead = Vec3::new(p.yaw.cos(), 0.0, p.yaw.sin());
                chops.push(p.pos + Vec3::Y + ahead);
            }
            p.pos = if p.pos.distance_squared(t.pos) > 64.0 {
                t.pos
            } else {
                p.pos.lerp(t.pos, g)
            };
            p.yaw = lerp_angle(p.yaw, t.yaw, g);
            p.body_yaw = lerp_angle(p.body_yaw, t.body_yaw, g);
            p.pitch += (t.pitch - p.pitch) * g;
            p.limb_swing += (t.limb_swing - p.limb_swing) * g;
            p.limb_amount += (t.limb_amount - p.limb_amount) * g;
            p.crouch += (t.crouch - p.crouch) * g;
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
            // (an axe's swing goes on smoothly between the poses; a new one starts over)
            p.chop = match (p.chop, t.chop) {
                (Some(mut s), Some(n)) if s.kind == n.kind && n.clock + 0.2 >= s.clock => {
                    s.clock = (s.clock + dt).clamp(n.clock - 0.1, n.clock + 0.1);
                    s.hit = n.hit;
                    Some(s)
                }
                (_, n) => n,
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
        for at in chops {
            self.audio.play(crate::audio::Sound::AxeChop, Some(at), 1.0);
        }
    }

    fn set_remote_pose(&mut self, id: u8, pose: Pose) {
        if let Some(r) = self.session.remotes.iter_mut().find(|r| r.id == id) {
            if !r.has_pose {
                r.pose = pose;
            }
            // (the server passes on the latest pose every tick, whether it changed or not: the
            // same again does not start the glide over, which would slow it)
            let t = &r.target;
            if pose.pos != t.pos || pose.yaw != t.yaw || pose.body_yaw != t.body_yaw || pose.pitch != t.pitch || pose.limb_swing != t.limb_swing {
                r.glide.restart();
            }
            r.target = pose;
            r.has_pose = true;
        }
    }

    /// Another player left: their model and skin go.
    fn remove_remote(&mut self, id: u8) {
        self.session.remotes.retain(|r| r.id != id);
        self.gfx.remove_skin(id);
    }

    fn add_remote(&mut self, id: u8, name: String) {
        self.session.remotes.retain(|r| r.id != id);
        self.session.remotes.push(RemotePlayer {
            id,
            name,
            pose: Pose::default(),
            target: Pose::default(),
            glide: Default::default(),
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
fn remote_gun(p: &Pose, time: f32, shot_at: Option<f32>) -> crate::model::guns::pistol_view::GunAnim {
    let mut g = crate::model::guns::pistol_view::GunAnim::unpack(
        p.gun_state,
        shot_at.map(|at| time - at).filter(|&t| (0.0..1.0).contains(&t)),
    );
    g.unpack_extra(p.gun_extra);
    let gun = crate::item::Stack { data: p.held_data, ..crate::item::Stack::one(p.held) };
    match GunKind::of(p.held) {
        Some(kind) if !kind.uses_magazine() => g.cyl = p.held_data,
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
        chop: p.chop,
    }
}

impl Game {
    /// The other players holding a gun (and not in bed): id, the gun, its attachments,
    /// their eye and where they look.
    pub(super) fn remote_guns(&self) -> Vec<(u8, GunKind, u8, Vec3, Vec3)> {
        self.session.remotes
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
        let r = self.session.remotes.iter().find(|r| r.id == id && r.shown())?;
        let pose = standing_pose(&r.pose, self.clock.time, r.shot_at);
        let point = crate::model::guns::gun_view::rest_point_in_gun_space(kind, point);
        Some(crate::model::players::player::gun_point(&pose, kind, point))
    }
}

impl Game {
    /// The other players fishing: where the tip of their rod is and what it is doing (for
    /// their line and bobber).
    pub(super) fn remote_rods(&self) -> Vec<(Vec3, crate::model::items::angler::RodAnim)> {
        self.session.remotes
            .iter()
            .filter(|r| r.shown() && r.pose.flags & pose_flags::SLEEPING == 0)
            .filter_map(|r| {
                let rod = r.pose.rod.filter(|_| r.pose.held == crate::item::FISHING_ROD)?;
                let pose = standing_pose(&r.pose, self.clock.time, r.shot_at);
                Some((crate::model::players::player::rod_tip(&pose)?, rod))
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
                crate::model::players::player::lying(p.pos, head)
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
            let lantern = if crate::model::players::player::hangs(pose.held) {
                Some(r.lantern.update(
                    crate::model::items::lantern::ON_MODEL,
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
                crate::model::players::player::lay_down(&mut out[start..], feet, turn);
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
        self.session.remotes
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

    /// A chat line typed by this player (the server sends it to everyone, this player too).
    pub(super) fn chat_line(&mut self, text: &str) {
        self.send(Msg::Chat {
            text: text.to_string(),
            color: color_bytes(chat::WHITE),
        });
    }

    /// Contents of the block entity at `p` as a message (the crafting table open here: its
    /// live grid).
    fn container_msg(&self, p: IVec3) -> Option<Msg> {
        let (kind, mut slots) = self.level.block_entities.container(&self.terrain.world, p)?;
        if matches!(self.screen, Screen::Container(Container::Crafting(q)) if q == p) {
            slots = self.me.items.craft.to_vec();
        }
        Some(Msg::Container { p, kind, slots })
    }

    /// Stores received contents (the crafting table open here: into its live grid).
    fn apply_container(&mut self, p: IVec3, kind: u8, slots: &[Slot]) {
        let open_here = matches!(self.screen, Screen::Container(Container::Crafting(q)) if q == p);
        if kind == container::TABLE && open_here {
            self.me.items.craft = std::array::from_fn(|i| slots.get(i).copied().flatten());
        } else {
            self.level.block_entities.apply_container(&self.terrain.world, p, kind, slots);
        }
    }

    /// A furnace as the server has it (only for showing it; the server runs it).
    #[allow(clippy::too_many_arguments)]
    fn apply_furnace(
        &mut self,
        p: IVec3,
        burn: f32,
        cook: f32,
        [input, fuel, output]: [Slot; 3],
        grill: Vec<(u8, crate::entity::Grilled)>,
    ) {
        let f = self.level.block_entities.furnaces.entry(p).or_default();
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

    /// This player opened a container: the server sends its contents.
    pub(super) fn net_container_opened(&mut self, p: IVec3) {
        if let Some(c) = &mut self.session.net {
            c.container_known = None;
            c.conn.send(&Msg::Open { p });
        }
    }

    pub(super) fn net_container_closed(&mut self) {
        if let Some(c) = &mut self.session.net {
            c.container_known = None;
            c.conn.send(&Msg::Open {
                p: IVec3::new(0, CLOSED_Y, 0),
            });
        }
    }

    /// The server's copy of the open chest or crafting table has come (a change made before
    /// it would be lost under it); the other screens are always ready.
    pub(super) fn container_ready(&self, c: Container) -> bool {
        !matches!(c, Container::Chest(_) | Container::Crafting(_))
            || self.session.net.as_ref().is_none_or(|n| n.container_known.is_some())
    }

    /// Sends the open container if this player changed it.
    pub(super) fn net_container_sync(&mut self) {
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
        if let Some(c) = &mut self.session.net {
            // Nothing known yet: wait for the server's copy instead of overwriting it.
            if c.container_known.as_ref().is_some_and(|k| *k != bytes) {
                c.conn.send(&msg);
                c.container_known = Some(bytes);
            }
        }
    }
}
