//! LAN multiplayer networking: the message protocol, TCP connections, the host's server
//! and LAN discovery.
//!
//! The host runs the world (blocks, fluids, mobs, dropped items, furnaces) and sends what
//! changes; players generate the untouched terrain themselves from the seed. Every message
//! is a little-endian binary frame: a u32 length, then a tag byte and the fields.
//!
//! Discovery works like Minecraft's "Open to LAN": the host announces its game on a UDP
//! broadcast every 1.5 seconds and the multiplayer screen lists what it hears.

mod conn;

pub use conn::{local_ip, Conn, Finder, Server, DEFAULT_PORT};

use crate::entity::Grilled;
use crate::item::{Slot, Stack};
use glam::{IVec3, Vec3};

/// Bumped whenever the messages change; host and players must match.
pub const PROTOCOL: u16 = 21;

// ---------------------------------------------------------------------------- data

/// "No block" in `Pose::open`.
pub const NO_BLOCK: IVec3 = IVec3::new(0, i32::MIN, 0);

/// A player's look, sent 20 times a second (animation values included, so the others
/// see exactly what the player sees of their own model).
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Pose {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub body_yaw: f32,
    pub limb_swing: f32,
    pub limb_amount: f32,
    /// Attack swing progress 0..1.
    pub attack: f32,
    pub crouch: f32,
    pub held: u16,
    pub skin: u8,
    pub flags: u8,
    /// The block being mined and how far (0 = not mining), for the crack overlay.
    pub mining: IVec3,
    pub mine_progress: f32,
    /// Block entity this player has open (chest lids open for everyone), `NO_BLOCK` if none.
    pub open: IVec3,
    /// What the player is busy with (`status`), shown in a bubble above their head.
    pub status: u8,
    /// The attachments on the held gun (`gun_mod` bits).
    pub gun_mods: u8,
    /// What they wear (`item::armor_code`).
    pub armor: u16,
    /// Holding the guide book open (`book` bits): open, and the page turns so far.
    pub book: u8,
    /// The spread it is open at (its left page / 2), and `book::HUNGARIAN`.
    pub book_page: u8,
    /// In spectator mode: flies through blocks, and only other spectators see them.
    pub spectator: bool,
}

/// `Pose::book`: the book is held open; the last page turn went back; the number of page
/// turns so far (low 6 bits, wrapping), so the others turn a page when it changes.
/// `Pose::book_page`: the book is read in Hungarian (the rest is the spread).
pub mod book {
    pub const OPEN: u8 = 0x80;
    pub const BACK: u8 = 0x40;
    pub const TURNS: u8 = 0x3f;
    pub const HUNGARIAN: u8 = 0x80;
}

/// `Pose::status`: typing in the chat, in the pause menu, away (the game window is not in
/// front) or looking at an item screen.
pub mod status {
    pub const NONE: u8 = 0;
    pub const TYPING: u8 = 1;
    pub const MENU: u8 = 2;
    pub const AFK: u8 = 3;
    pub const INVENTORY: u8 = 4;
    pub const READING: u8 = 5;
}

pub mod pose_flags {
    pub const BURNING: u8 = 1;
    pub const BLOCKING: u8 = 2;
    pub const HURT: u8 = 4;
    pub const DEAD: u8 = 8;
    pub const CREATIVE: u8 = 16;
    /// Lying in a bed (the pose's position is on top of the bed's head half).
    pub const SLEEPING: u8 = 32;
    /// Aiming a gun down its sights.
    pub const AIMING: u8 = 64;
    /// Holding the guide book turned around to show it.
    pub const SHOWING: u8 = 128;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MobNet {
    pub id: u32,
    pub kind: u8,
    pub pos: Vec3,
    pub body_yaw: f32,
    pub head_yaw: f32,
    pub pitch: f32,
    pub limb_swing: f32,
    pub limb_amount: f32,
    pub hurt: bool,
    /// Seconds since dying, or negative while alive.
    pub death: f32,
    /// A sheep without its wool.
    pub sheared: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemNet {
    pub id: u32,
    pub pos: Vec3,
    pub stack: Stack,
    pub age: f32,
}

/// Everything about a player that the host keeps between visits.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerState {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub health: f32,
    pub needs: [f32; 7],
    /// Game mode (`mode`): survival, creative or spectator.
    pub mode: u8,
    pub flying: bool,
    pub slot: u8,
    pub inventory: Vec<Slot>,
    /// The head of the bed the player last used (where they come back to life).
    pub bed: Option<IVec3>,
}

/// `PlayerState::mode` (a saved state's old creative flag reads as 0 or 1).
pub mod mode {
    pub const SURVIVAL: u8 = 0;
    pub const CREATIVE: u8 = 1;
    pub const SPECTATOR: u8 = 2;
}

/// What hurt a player (`Msg::Hurt`, `Msg::AttackPlayer`): armor takes some kinds better.
pub mod hurt {
    pub const MELEE: u8 = 0;
    pub const BULLET: u8 = 1;
    pub const BLAST: u8 = 2;
}

/// Block entity kinds in `Msg::Container`.
pub mod container {
    pub const CHEST: u8 = 0;
    pub const TABLE: u8 = 1;
}

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    // Player -> host
    Hello {
        proto: u16,
        name: String,
    },
    Pose(Pose),
    /// A block the player placed or removed with an item (buckets): the host applies it
    /// with the world's rules.
    Place {
        p: IVec3,
        b: u8,
    },
    /// A block the player mined: the host drops what it drops.
    Break {
        p: IVec3,
        held: u16,
        creative: bool,
    },
    AttackMob {
        id: u32,
        dmg: f32,
        knock: f32,
    },
    AttackPlayer {
        id: u8,
        dmg: f32,
        knock: f32,
        kind: u8,
    },
    SpawnMob {
        kind: u8,
        pos: Vec3,
    },
    /// A shot from a gun, for the others to see: the bullets (their speed and direction)
    /// leaving `eye`, the muzzle flash and the case. From a player the host sends it on to
    /// the rest with `id` set to the shooter.
    Shot {
        id: u8,
        kind: u8,
        mods: u8,
        eye: Vec3,
        seed: f32,
        bullets: Vec<Vec3>,
    },
    /// A grenade thrown (`kind`: 0 frag, 1 smoke), the same way as `Shot`; `seed` names it.
    Grenade {
        id: u8,
        kind: u8,
        pos: Vec3,
        vel: Vec3,
        seed: u32,
    },
    /// Host -> players: that grenade went off here.
    Blast {
        pos: Vec3,
        seed: u32,
    },
    /// Used shears on a mob (the host drops its wool).
    Shear {
        id: u32,
    },
    DropItem {
        pos: Vec3,
        vel: Vec3,
        stack: Stack,
        delay: f32,
    },
    /// Opened the block entity at `p` (the host answers with its contents).
    Open {
        p: IVec3,
    },
    Command(String),
    Save(PlayerState),
    /// Used a part of a furnace (`block_entity::part`): a left click (`take`) takes out
    /// what is there, a right click puts in or turns meat over. The player already took
    /// `offered` from their hand; the host gives back what did not go in, and what came out.
    FurnaceUse {
        p: IVec3,
        part: u8,
        take: bool,
        offered: Slot,
    },

    // Host -> player
    Welcome {
        id: u8,
        seed: u32,
        world: String,
        time: f32,
        spawn: (i32, i32),
        creative: bool,
        cheats: bool,
        state: Option<PlayerState>,
    },
    Refuse(String),
    Chunk {
        pos: (i32, i32),
        rle: Vec<u8>,
    },
    /// All chunks are sent: the player can start loading.
    Ready,
    Blocks(Vec<(IVec3, u8)>),
    Time(f32),
    Join {
        id: u8,
        name: String,
    },
    Leave {
        id: u8,
    },
    Poses(Vec<(u8, Pose)>),
    Entities {
        mobs: Vec<MobNet>,
        items: Vec<ItemNet>,
        falling: Vec<(Vec3, u8)>,
    },
    Give(Stack),
    /// Someone broke a block here: debris flies (the block change comes separately).
    BreakFx {
        p: IVec3,
        block: u8,
    },
    /// Hit by another player (or their grenade).
    Hurt {
        dmg: f32,
        from: Vec3,
        knock: f32,
        kind: u8,
    },

    // Both ways
    /// Contents of a chest or crafting table.
    Container {
        p: IVec3,
        kind: u8,
        slots: Vec<Slot>,
    },
    /// What everyone sees of a furnace: its fire, the meat on top (by corner) and what was
    /// put into the front.
    Furnace {
        p: IVec3,
        burn: f32,
        /// Seconds spent smelting the item in its mouth.
        cook: f32,
        input: Slot,
        fuel: Slot,
        /// What is smelted (it stays in the mouth).
        output: Slot,
        grill: Vec<(u8, Grilled)>,
    },
    Chat {
        text: String,
        color: [u8; 4],
    },
    /// One player's custom Minecraft skin PNG; id is assigned by the host.
    Skin {
        id: u8,
        png: Vec<u8>,
    },
}

// ---------------------------------------------------------------------------- codec

struct W(Vec<u8>);

impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend(v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend(v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.0.extend(v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend(v.to_le_bytes());
    }
    fn bool(&mut self, v: bool) {
        self.u8(v as u8);
    }
    fn bytes(&mut self, v: &[u8]) {
        self.u32(v.len() as u32);
        self.0.extend_from_slice(v);
    }
    fn str(&mut self, v: &str) {
        self.bytes(v.as_bytes());
    }
    fn vec3(&mut self, v: Vec3) {
        self.f32(v.x);
        self.f32(v.y);
        self.f32(v.z);
    }
    fn ivec3(&mut self, v: IVec3) {
        self.i32(v.x);
        self.i32(v.y);
        self.i32(v.z);
    }
    fn stack(&mut self, s: Stack) {
        self.u16(s.item);
        self.u8(s.count);
        self.u16(s.damage);
        self.u16(s.data);
    }
    fn slot(&mut self, s: Slot) {
        match s {
            Some(s) => {
                self.bool(true);
                self.stack(s);
            }
            None => self.bool(false),
        }
    }
    fn slots(&mut self, v: &[Slot]) {
        self.u32(v.len() as u32);
        for s in v {
            self.slot(*s);
        }
    }
    fn pose(&mut self, p: &Pose) {
        self.vec3(p.pos);
        for v in [
            p.yaw,
            p.pitch,
            p.body_yaw,
            p.limb_swing,
            p.limb_amount,
            p.attack,
            p.crouch,
        ] {
            self.f32(v);
        }
        self.u16(p.held);
        self.u8(p.skin);
        self.u8(p.flags);
        self.ivec3(p.mining);
        self.f32(p.mine_progress);
        self.ivec3(p.open);
        self.u8(p.status);
        self.u8(p.gun_mods);
        self.u16(p.armor);
        self.u8(p.book);
        self.u8(p.book_page);
        self.bool(p.spectator);
    }
    fn state(&mut self, s: &PlayerState) {
        self.vec3(s.pos);
        self.f32(s.yaw);
        self.f32(s.pitch);
        self.f32(s.health);
        for v in s.needs {
            self.f32(v);
        }
        self.u8(s.mode);
        self.bool(s.flying);
        self.u8(s.slot);
        self.slots(&s.inventory);
        match s.bed {
            Some(b) => {
                self.bool(true);
                self.ivec3(b);
            }
            None => self.bool(false),
        }
    }
}

struct R<'a> {
    d: &'a [u8],
    o: usize,
}

impl R<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.d.get(self.o..self.o.checked_add(n)?)?;
        self.o += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn f32(&mut self) -> Option<f32> {
        let v = f32::from_le_bytes(self.take(4)?.try_into().ok()?);
        v.is_finite().then_some(v)
    }
    fn bool(&mut self) -> Option<bool> {
        Some(self.u8()? != 0)
    }
    fn bytes(&mut self) -> Option<Vec<u8>> {
        let n = self.u32()? as usize;
        Some(self.take(n)?.to_vec())
    }
    fn str(&mut self) -> Option<String> {
        String::from_utf8(self.bytes()?).ok()
    }
    fn vec3(&mut self) -> Option<Vec3> {
        Some(Vec3::new(self.f32()?, self.f32()?, self.f32()?))
    }
    fn ivec3(&mut self) -> Option<IVec3> {
        Some(IVec3::new(self.i32()?, self.i32()?, self.i32()?))
    }
    fn stack(&mut self) -> Option<Stack> {
        Some(Stack {
            item: self.u16()?,
            count: self.u8()?,
            damage: self.u16()?,
            data: self.u16()?,
        })
    }
    fn slot(&mut self) -> Option<Slot> {
        Some(if self.bool()? {
            Some(self.stack()?)
        } else {
            None
        })
    }
    fn slots(&mut self) -> Option<Vec<Slot>> {
        let n = self.u32()? as usize;
        if n > 1024 {
            return None;
        }
        (0..n).map(|_| self.slot()).collect()
    }
    /// A list of `n` items read by `f`, with a sanity limit.
    fn list<T>(&mut self, f: impl Fn(&mut Self) -> Option<T>) -> Option<Vec<T>> {
        let n = self.u32()? as usize;
        if n > 1 << 20 {
            return None;
        }
        (0..n).map(|_| f(self)).collect()
    }
    fn pose(&mut self) -> Option<Pose> {
        Some(Pose {
            pos: self.vec3()?,
            yaw: self.f32()?,
            pitch: self.f32()?,
            body_yaw: self.f32()?,
            limb_swing: self.f32()?,
            limb_amount: self.f32()?,
            attack: self.f32()?,
            crouch: self.f32()?,
            held: self.u16()?,
            skin: self.u8()?,
            flags: self.u8()?,
            mining: self.ivec3()?,
            mine_progress: self.f32()?,
            open: self.ivec3()?,
            status: self.u8()?,
            gun_mods: self.u8()?,
            armor: self.u16()?,
            book: self.u8()?,
            book_page: self.u8()?,
            spectator: self.bool()?,
        })
    }
    fn state(&mut self) -> Option<PlayerState> {
        Some(PlayerState {
            pos: self.vec3()?,
            yaw: self.f32()?,
            pitch: self.f32()?,
            health: self.f32()?,
            needs: [
                self.f32()?,
                self.f32()?,
                self.f32()?,
                self.f32()?,
                self.f32()?,
                self.f32()?,
                self.f32()?,
            ],
            mode: self.u8()?,
            flying: self.bool()?,
            slot: self.u8()?,
            inventory: self.slots()?,
            bed: if self.bool()? { Some(self.ivec3()?) } else { None },
        })
    }
}

impl Msg {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = W(Vec::with_capacity(64));
        match self {
            Msg::Hello { proto, name } => {
                w.u8(0);
                w.u16(*proto);
                w.str(name);
            }
            Msg::Pose(p) => {
                w.u8(1);
                w.pose(p);
            }
            Msg::Place { p, b } => {
                w.u8(2);
                w.ivec3(*p);
                w.u8(*b);
            }
            Msg::Break { p, held, creative } => {
                w.u8(3);
                w.ivec3(*p);
                w.u16(*held);
                w.bool(*creative);
            }
            Msg::AttackMob { id, dmg, knock } => {
                w.u8(4);
                w.u32(*id);
                w.f32(*dmg);
                w.f32(*knock);
            }
            Msg::AttackPlayer { id, dmg, knock, kind } => {
                w.u8(5);
                w.u8(*id);
                w.f32(*dmg);
                w.f32(*knock);
                w.u8(*kind);
            }
            Msg::SpawnMob { kind, pos } => {
                w.u8(6);
                w.u8(*kind);
                w.vec3(*pos);
            }
            Msg::Shear { id } => {
                w.u8(11);
                w.u32(*id);
            }
            Msg::FurnaceUse {
                p,
                part,
                take,
                offered,
            } => {
                w.u8(12);
                w.ivec3(*p);
                w.u8(*part);
                w.bool(*take);
                w.slot(*offered);
            }
            Msg::DropItem {
                pos,
                vel,
                stack,
                delay,
            } => {
                w.u8(7);
                w.vec3(*pos);
                w.vec3(*vel);
                w.stack(*stack);
                w.f32(*delay);
            }
            Msg::Open { p } => {
                w.u8(8);
                w.ivec3(*p);
            }
            Msg::Command(line) => {
                w.u8(9);
                w.str(line);
            }
            Msg::Save(s) => {
                w.u8(10);
                w.state(s);
            }
            Msg::Welcome {
                id,
                seed,
                world,
                time,
                spawn,
                creative,
                cheats,
                state,
            } => {
                w.u8(20);
                w.u8(*id);
                w.u32(*seed);
                w.str(world);
                w.f32(*time);
                w.i32(spawn.0);
                w.i32(spawn.1);
                w.bool(*creative);
                w.bool(*cheats);
                match state {
                    Some(s) => {
                        w.bool(true);
                        w.state(s);
                    }
                    None => w.bool(false),
                }
            }
            Msg::Refuse(reason) => {
                w.u8(21);
                w.str(reason);
            }
            Msg::Chunk { pos, rle } => {
                w.u8(22);
                w.i32(pos.0);
                w.i32(pos.1);
                w.bytes(rle);
            }
            Msg::Ready => w.u8(23),
            Msg::Blocks(list) => {
                w.u8(24);
                w.u32(list.len() as u32);
                for (p, b) in list {
                    w.ivec3(*p);
                    w.u8(*b);
                }
            }
            Msg::Time(t) => {
                w.u8(25);
                w.f32(*t);
            }
            Msg::Join { id, name } => {
                w.u8(26);
                w.u8(*id);
                w.str(name);
            }
            Msg::Leave { id } => {
                w.u8(27);
                w.u8(*id);
            }
            Msg::Poses(list) => {
                w.u8(28);
                w.u32(list.len() as u32);
                for (id, p) in list {
                    w.u8(*id);
                    w.pose(p);
                }
            }
            Msg::Entities {
                mobs,
                items,
                falling,
            } => {
                w.u8(29);
                w.u32(mobs.len() as u32);
                for m in mobs {
                    w.u32(m.id);
                    w.u8(m.kind);
                    w.vec3(m.pos);
                    for v in [m.body_yaw, m.head_yaw, m.pitch, m.limb_swing, m.limb_amount] {
                        w.f32(v);
                    }
                    w.bool(m.hurt);
                    w.f32(m.death);
                    w.bool(m.sheared);
                }
                w.u32(items.len() as u32);
                for it in items {
                    w.u32(it.id);
                    w.vec3(it.pos);
                    w.stack(it.stack);
                    w.f32(it.age);
                }
                w.u32(falling.len() as u32);
                for (p, b) in falling {
                    w.vec3(*p);
                    w.u8(*b);
                }
            }
            Msg::Give(s) => {
                w.u8(30);
                w.stack(*s);
            }
            Msg::BreakFx { p, block } => {
                w.u8(32);
                w.ivec3(*p);
                w.u8(*block);
            }
            Msg::Hurt {
                dmg,
                from,
                knock,
                kind,
            } => {
                w.u8(31);
                w.f32(*dmg);
                w.vec3(*from);
                w.f32(*knock);
                w.u8(*kind);
            }
            Msg::Container { p, kind, slots } => {
                w.u8(40);
                w.ivec3(*p);
                w.u8(*kind);
                w.slots(slots);
            }
            Msg::Furnace {
                p,
                burn,
                cook,
                input,
                fuel,
                output,
                grill,
            } => {
                w.u8(43);
                w.ivec3(*p);
                w.f32(*burn);
                w.f32(*cook);
                w.slot(*input);
                w.slot(*fuel);
                w.slot(*output);
                w.u32(grill.len() as u32);
                for (corner, g) in grill {
                    w.u8(*corner);
                    w.u16(g.raw);
                    w.f32(g.cook[0]);
                    w.f32(g.cook[1]);
                    w.u8(g.down);
                    w.f32(g.flip);
                }
            }
            Msg::Chat { text, color } => {
                w.u8(41);
                w.str(text);
                w.0.extend_from_slice(color);
            }
            Msg::Skin { id, png } => {
                w.u8(42);
                w.u8(*id);
                w.bytes(png);
            }
            Msg::Grenade {
                id,
                kind,
                pos,
                vel,
                seed,
            } => {
                w.u8(45);
                w.u8(*id);
                w.u8(*kind);
                w.vec3(*pos);
                w.vec3(*vel);
                w.u32(*seed);
            }
            Msg::Blast { pos, seed } => {
                w.u8(46);
                w.vec3(*pos);
                w.u32(*seed);
            }
            Msg::Shot {
                id,
                kind,
                mods,
                eye,
                seed,
                bullets,
            } => {
                w.u8(44);
                w.u8(*id);
                w.u8(*kind);
                w.u8(*mods);
                w.vec3(*eye);
                w.f32(*seed);
                w.u32(bullets.len() as u32);
                for v in bullets {
                    w.vec3(*v);
                }
            }
        }
        w.0
    }

    pub fn decode(d: &[u8]) -> Option<Msg> {
        let mut r = R { d, o: 0 };
        let m = match r.u8()? {
            0 => Msg::Hello {
                proto: r.u16()?,
                name: r.str()?,
            },
            1 => Msg::Pose(r.pose()?),
            2 => Msg::Place {
                p: r.ivec3()?,
                b: r.u8()?,
            },
            3 => Msg::Break {
                p: r.ivec3()?,
                held: r.u16()?,
                creative: r.bool()?,
            },
            4 => Msg::AttackMob {
                id: r.u32()?,
                dmg: r.f32()?,
                knock: r.f32()?,
            },
            5 => Msg::AttackPlayer {
                id: r.u8()?,
                dmg: r.f32()?,
                knock: r.f32()?,
                kind: r.u8()?,
            },
            6 => Msg::SpawnMob {
                kind: r.u8()?,
                pos: r.vec3()?,
            },
            7 => Msg::DropItem {
                pos: r.vec3()?,
                vel: r.vec3()?,
                stack: r.stack()?,
                delay: r.f32()?,
            },
            8 => Msg::Open { p: r.ivec3()? },
            9 => Msg::Command(r.str()?),
            10 => Msg::Save(r.state()?),
            11 => Msg::Shear { id: r.u32()? },
            12 => Msg::FurnaceUse {
                p: r.ivec3()?,
                part: r.u8()?,
                take: r.bool()?,
                offered: r.slot()?,
            },
            20 => Msg::Welcome {
                id: r.u8()?,
                seed: r.u32()?,
                world: r.str()?,
                time: r.f32()?,
                spawn: (r.i32()?, r.i32()?),
                creative: r.bool()?,
                cheats: r.bool()?,
                state: if r.bool()? { Some(r.state()?) } else { None },
            },
            21 => Msg::Refuse(r.str()?),
            22 => Msg::Chunk {
                pos: (r.i32()?, r.i32()?),
                rle: r.bytes()?,
            },
            23 => Msg::Ready,
            24 => Msg::Blocks(r.list(|r| Some((r.ivec3()?, r.u8()?)))?),
            25 => Msg::Time(r.f32()?),
            26 => Msg::Join {
                id: r.u8()?,
                name: r.str()?,
            },
            27 => Msg::Leave { id: r.u8()? },
            28 => Msg::Poses(r.list(|r| Some((r.u8()?, r.pose()?)))?),
            29 => Msg::Entities {
                mobs: r.list(|r| {
                    Some(MobNet {
                        id: r.u32()?,
                        kind: r.u8()?,
                        pos: r.vec3()?,
                        body_yaw: r.f32()?,
                        head_yaw: r.f32()?,
                        pitch: r.f32()?,
                        limb_swing: r.f32()?,
                        limb_amount: r.f32()?,
                        hurt: r.bool()?,
                        death: r.f32()?,
                        sheared: r.bool()?,
                    })
                })?,
                items: r.list(|r| {
                    Some(ItemNet {
                        id: r.u32()?,
                        pos: r.vec3()?,
                        stack: r.stack()?,
                        age: r.f32()?,
                    })
                })?,
                falling: r.list(|r| Some((r.vec3()?, r.u8()?)))?,
            },
            30 => Msg::Give(r.stack()?),
            32 => Msg::BreakFx {
                p: r.ivec3()?,
                block: r.u8()?,
            },
            31 => Msg::Hurt {
                dmg: r.f32()?,
                from: r.vec3()?,
                knock: r.f32()?,
                kind: r.u8()?,
            },
            40 => Msg::Container {
                p: r.ivec3()?,
                kind: r.u8()?,
                slots: r.slots()?,
            },
            43 => Msg::Furnace {
                p: r.ivec3()?,
                burn: r.f32()?,
                cook: r.f32()?,
                input: r.slot()?,
                fuel: r.slot()?,
                output: r.slot()?,
                grill: r.list(|r| {
                    let corner = r.u8()?;
                    let g = Grilled {
                        raw: r.u16()?,
                        cook: [r.f32()?, r.f32()?],
                        down: r.u8()? & 1,
                        flip: r.f32()?,
                    };
                    (corner < 4).then_some((corner, g))
                })?,
            },
            41 => Msg::Chat {
                text: r.str()?,
                color: r.take(4)?.try_into().ok()?,
            },
            42 => {
                let id = r.u8()?;
                let png = r.bytes()?;
                if png.len() > 1_000_000 {
                    return None;
                }
                Msg::Skin { id, png }
            }
            45 => Msg::Grenade {
                id: r.u8()?,
                kind: r.u8()?,
                pos: r.vec3()?,
                vel: r.vec3()?,
                seed: r.u32()?,
            },
            46 => Msg::Blast {
                pos: r.vec3()?,
                seed: r.u32()?,
            },
            44 => Msg::Shot {
                id: r.u8()?,
                kind: r.u8()?,
                mods: r.u8()?,
                eye: r.vec3()?,
                seed: r.f32()?,
                bullets: {
                    let v = r.list(|r| r.vec3())?;
                    if v.len() > 32 {
                        return None;
                    }
                    v
                },
            },
            _ => return None,
        };
        // Trailing bytes mean a version mismatch or corruption.
        (r.o == d.len()).then_some(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(m: Msg) {
        let enc = m.encode();
        assert_eq!(Msg::decode(&enc), Some(m));
        // Cut short: never panics, just fails.
        for n in 0..enc.len() {
            assert!(Msg::decode(&enc[..n]).is_none() || n == enc.len());
        }
    }

    #[test]
    fn messages_roundtrip() {
        let stack = Stack {
            item: 300,
            count: 1,
            damage: 12,
            data: 0x0305,
        };
        let state = PlayerState {
            pos: Vec3::new(1.0, 70.5, -3.25),
            yaw: 1.0,
            pitch: -0.2,
            health: 17.0,
            needs: [20.0, 5.0, 0.0, 18.0, 1.0, 0.0, 3.0],
            mode: mode::SPECTATOR,
            flying: false,
            slot: 3,
            inventory: vec![Some(stack), None, Some(Stack::new(3, 64))],
            bed: Some(IVec3::new(4, 70, -12)),
        };
        roundtrip(Msg::Hello {
            proto: PROTOCOL,
            name: "Albí".into(),
        });
        roundtrip(Msg::Pose(Pose {
            pos: Vec3::ONE,
            held: 5,
            skin: 2,
            flags: pose_flags::HURT,
            status: status::TYPING,
            gun_mods: 0b1001,
            armor: 0x1234,
            book: book::OPEN | 5,
            book_page: book::HUNGARIAN | 7,
            spectator: true,
            ..Default::default()
        }));
        roundtrip(Msg::Grenade {
            id: 1,
            kind: 1,
            pos: Vec3::new(3.0, 64.0, 1.0),
            vel: Vec3::new(10.0, 2.0, -4.0),
            seed: 0xdead_beef,
        });
        roundtrip(Msg::Blast {
            pos: Vec3::new(3.0, 64.0, 1.0),
            seed: 7,
        });
        roundtrip(Msg::Hurt {
            dmg: 4.5,
            from: Vec3::ONE,
            knock: 0.4,
            kind: hurt::BULLET,
        });
        roundtrip(Msg::Shot {
            id: 3,
            kind: 4,
            mods: 2,
            eye: Vec3::new(1.0, 70.5, -3.0),
            seed: 0.25,
            bullets: vec![Vec3::X * 120.0, Vec3::new(0.1, 0.2, 119.0)],
        });
        roundtrip(Msg::Skin {
            id: 2,
            png: vec![137, 80, 78, 71, 0, 255],
        });
        roundtrip(Msg::Welcome {
            id: 2,
            seed: 99,
            world: "Új világ".into(),
            time: 0.3,
            spawn: (-5, 7),
            creative: false,
            cheats: true,
            state: Some(state.clone()),
        });
        roundtrip(Msg::Save(state));
        roundtrip(Msg::Shear { id: 77 });
        roundtrip(Msg::Blocks(vec![
            (IVec3::new(1, 2, 3), 7),
            (IVec3::new(-9, 0, 4), 0),
        ]));
        roundtrip(Msg::Entities {
            mobs: vec![MobNet {
                id: 4,
                kind: 0,
                pos: Vec3::X,
                body_yaw: 1.0,
                head_yaw: 1.2,
                pitch: 0.0,
                limb_swing: 3.0,
                limb_amount: 0.4,
                hurt: true,
                death: -1.0,
                sheared: true,
            }],
            items: vec![ItemNet {
                id: 9,
                pos: Vec3::Y,
                stack,
                age: 2.0,
            }],
            falling: vec![(Vec3::Z, 4)],
        });
        roundtrip(Msg::Container {
            p: IVec3::new(3, 4, 5),
            kind: container::CHEST,
            slots: vec![None, Some(stack), None],
        });
        roundtrip(Msg::Furnace {
            p: IVec3::new(-3, 64, 9),
            burn: 12.5,
            cook: 4.0,
            input: None,
            fuel: Some(Stack::new(257, 7)),
            output: Some(Stack::new(259, 3)),
            grill: vec![(
                2,
                Grilled {
                    raw: 268,
                    cook: [10.5, 3.0],
                    down: 1,
                    flip: 0.2,
                },
            )],
        });
        roundtrip(Msg::FurnaceUse {
            p: IVec3::new(1, 2, 3),
            part: 3,
            take: true,
            offered: Some(Stack::one(268)),
        });
        roundtrip(Msg::Chat {
            text: "<Albi> szia".into(),
            color: [255, 255, 255, 255],
        });
    }
}
