//! The binary form of the messages: a little-endian writer and reader, `Msg::encode` and
//! `Msg::decode`, and the limits on what a peer may send.

use super::{ItemNet, MobNet, Msg, PlayerState, Pose};
use crate::entity::{BenchEvent, BenchItem, GunBench, Grilled};
use crate::item::{Slot, Stack};
use glam::{IVec3, Vec3};

// Limits on what is read from a peer: anything larger means a broken or hostile peer, and the
// message (or the connection, for a frame) is refused rather than allocated.

/// Largest frame read from a connection, in bytes.
pub(super) const MAX_FRAME: usize = 64 << 20;
/// Most slots in one list of slots (a container or an inventory).
const MAX_SLOTS: usize = 1024;
/// Most entries in any other list (blocks, poses, mobs, items, bench items).
const MAX_LIST: usize = 1 << 20;
/// Largest custom skin PNG, in bytes.
const MAX_SKIN_BYTES: usize = 1_000_000;
/// Most bullets in one shot.
const MAX_BULLETS: usize = 32;
/// Longest chat line kept, in characters: a full chat input (256) with room for the "<name> "
/// the host puts in front. A longer line is cut, not refused (that would drop the peer).
const MAX_CHAT_CHARS: usize = 320;

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
        self.u16(p.gun_state);
        self.u16(p.armor);
        self.u8(p.book);
        self.u8(p.book_page);
        self.bool(p.spectator);
        self.f32(p.sprint);
        self.u8(p.gun_dirt);
        match p.brush {
            Some(b) => {
                self.bool(true);
                self.vec3(b);
            }
            None => self.bool(false),
        }
        self.bool(p.drawer);
        self.u16(p.held_data);
        self.u32(p.gun_extra);
        match p.bench_hold {
            Some((st, at)) => {
                self.bool(true);
                self.stack(st);
                self.vec3(at);
            }
            None => self.bool(false),
        }
        self.u16(p.grenade);
        match p.rod {
            Some(r) => {
                self.bool(true);
                for v in [r.charge, r.cast.unwrap_or(-1.0), r.fight, r.tension, r.crank, r.lift.unwrap_or(-1.0)] {
                    self.f32(v);
                }
                self.bool(r.out);
                match r.bobber {
                    Some(b) => {
                        self.bool(true);
                        self.vec3(b);
                    }
                    None => self.bool(false),
                }
            }
            None => self.bool(false),
        }
    }
    fn bench_item(&mut self, i: &BenchItem) {
        self.u16(i.id);
        self.stack(i.stack);
        self.f32(i.x);
        self.f32(i.z);
        self.f32(i.turn);
    }
    fn bench(&mut self, b: &GunBench) {
        self.u32(b.items.len() as u32);
        for i in &b.items {
            self.bench_item(i);
        }
        self.u16(b.next_id);
        for n in b.boxes {
            self.u16(n.unwrap_or(u16::MAX));
        }
        self.bool(b.loader);
        self.bool(b.loader_mag.is_some());
        if let Some(m) = b.loader_mag {
            self.stack(m);
        }
        let e = &b.event;
        self.u16(e.serial);
        self.u8(e.kind);
        self.u16(e.gun);
        self.u8(e.bit);
        self.u32(e.gone.len() as u32);
        for i in &e.gone {
            self.bench_item(i);
        }
        self.u32(e.made.len() as u32);
        for &m in &e.made {
            self.u16(m);
        }
        self.u8(b.grenades[0]);
        self.u8(b.grenades[1]);
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
        if n > MAX_SLOTS {
            return None;
        }
        (0..n).map(|_| self.slot()).collect()
    }
    /// A list of `n` items read by `f`, with a sanity limit.
    fn list<T>(&mut self, f: impl Fn(&mut Self) -> Option<T>) -> Option<Vec<T>> {
        let n = self.u32()? as usize;
        if n > MAX_LIST {
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
            gun_state: self.u16()?,
            armor: self.u16()?,
            book: self.u8()?,
            book_page: self.u8()?,
            spectator: self.bool()?,
            sprint: self.f32()?,
            gun_dirt: self.u8()?,
            brush: if self.bool()? { Some(self.vec3()?) } else { None },
            drawer: self.bool()?,
            held_data: self.u16()?,
            gun_extra: self.u32()?,
            bench_hold: if self.bool()? { Some((self.stack()?, self.vec3()?)) } else { None },
            grenade: self.u16()?,
            rod: if self.bool()? { Some(self.rod()?) } else { None },
        })
    }
    fn rod(&mut self) -> Option<crate::model::angler::RodAnim> {
        let time = |v: f32| (v >= 0.0).then_some(v);
        Some(crate::model::angler::RodAnim {
            charge: self.f32()?,
            cast: time(self.f32()?),
            fight: self.f32()?,
            tension: self.f32()?,
            crank: self.f32()?,
            lift: time(self.f32()?),
            out: self.bool()?,
            bobber: if self.bool()? { Some(self.vec3()?) } else { None },
        })
    }
    fn bench_item(&mut self) -> Option<BenchItem> {
        Some(BenchItem {
            id: self.u16()?,
            stack: self.stack()?,
            x: self.f32()?,
            z: self.f32()?,
            turn: self.f32()?,
        })
    }
    fn bench(&mut self) -> Option<GunBench> {
        let items = self.list(|r| r.bench_item())?;
        let next_id = self.u16()?;
        let slot = |v: u16| (v != u16::MAX).then_some(v);
        let boxes = [slot(self.u16()?), slot(self.u16()?), slot(self.u16()?)];
        let loader = self.bool()?;
        let loader_mag = if self.bool()? { Some(self.stack()?) } else { None };
        Some(GunBench {
            items,
            next_id,
            boxes,
            loader,
            loader_mag,
            event: BenchEvent {
                serial: self.u16()?,
                kind: self.u8()?,
                gun: self.u16()?,
                bit: self.u8()?,
                gone: self.list(|r| r.bench_item())?,
                made: self.list(|r| r.u16())?,
            },
            grenades: [self.u8()?, self.u8()?],
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
            Msg::Hello { proto, name, view } => {
                w.u8(0);
                w.u16(*proto);
                w.str(name);
                w.u8(*view);
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
            Msg::BreakDummy { id } => {
                w.u8(48);
                w.u32(*id);
            }
            Msg::UseOnMob { id, item } => {
                w.u8(49);
                w.u32(*id);
                w.u16(*item);
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
                full,
                mobs,
                items,
                gone_mobs,
                gone_items,
                falling,
            } => {
                w.u8(29);
                w.bool(*full);
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
                    w.f32(m.taken);
                    w.f32(m.last_hit);
                    w.u8(m.flags);
                    w.u8(m.collar);
                }
                w.u32(items.len() as u32);
                for it in items {
                    w.u32(it.id);
                    w.vec3(it.pos);
                    w.stack(it.stack);
                    w.f32(it.age);
                }
                for gone in [gone_mobs, gone_items] {
                    w.u32(gone.len() as u32);
                    for id in gone {
                        w.u32(*id);
                    }
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
            Msg::Bench { p, bench } => {
                w.u8(47);
                w.ivec3(*p);
                w.bench(bench);
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
                fuse,
            } => {
                w.u8(45);
                w.u8(*id);
                w.u8(*kind);
                w.vec3(*pos);
                w.vec3(*vel);
                w.u32(*seed);
                w.f32(*fuse);
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
                view: r.u8()?,
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
            48 => Msg::BreakDummy { id: r.u32()? },
            49 => Msg::UseOnMob { id: r.u32()?, item: r.u16()? },
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
                full: r.bool()?,
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
                        taken: r.f32()?,
                        last_hit: r.f32()?,
                        flags: r.u8()?,
                        collar: r.u8()?,
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
                gone_mobs: r.list(|r| r.u32())?,
                gone_items: r.list(|r| r.u32())?,
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
            47 => Msg::Bench {
                p: r.ivec3()?,
                bench: r.bench()?,
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
                text: chat_text(r.str()?),
                color: r.take(4)?.try_into().ok()?,
            },
            42 => {
                let id = r.u8()?;
                let png = r.bytes()?;
                if png.len() > MAX_SKIN_BYTES {
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
                fuse: r.f32()?,
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
                    if v.len() > MAX_BULLETS {
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

/// A chat line cut to `MAX_CHAT_CHARS` characters.
fn chat_text(mut text: String) -> String {
    if let Some((i, _)) = text.char_indices().nth(MAX_CHAT_CHARS) {
        text.truncate(i);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{book, container, hurt, mode, pose_flags, status, PROTOCOL};

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
            view: 12,
        });
        roundtrip(Msg::Pose(Pose {
            pos: Vec3::ONE,
            held: 5,
            skin: 2,
            flags: pose_flags::HURT,
            status: status::TYPING,
            gun_mods: 0b1001,
            gun_state: 0x1c5,
            held_data: 0x0915,
            gun_extra: 0x2a_2d4b,
            armor: 0x1234,
            book: book::OPEN | 5,
            book_page: book::HUNGARIAN | 7,
            spectator: true,
            sprint: 0.5,
            rod: Some(crate::model::angler::RodAnim {
                charge: 0.25,
                cast: Some(0.1),
                out: true,
                fight: 1.0,
                tension: 0.6,
                crank: 2.0,
                lift: None,
                bobber: Some(Vec3::new(3.0, 60.5, -8.0)),
            }),
            ..Default::default()
        }));
        roundtrip(Msg::Grenade {
            id: 1,
            kind: 1,
            pos: Vec3::new(3.0, 64.0, 1.0),
            vel: Vec3::new(10.0, 2.0, -4.0),
            seed: 0xdead_beef,
            fuse: 1.75,
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
        roundtrip(Msg::BreakDummy { id: 78 });
        roundtrip(Msg::UseOnMob { id: 78, item: 380 });
        roundtrip(Msg::Blocks(vec![
            (IVec3::new(1, 2, 3), 7),
            (IVec3::new(-9, 0, 4), 0),
        ]));
        roundtrip(Msg::Entities {
            full: false,
            gone_mobs: vec![5, 6],
            gone_items: vec![],
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
                taken: 12.5,
                last_hit: 7.0,
                flags: 5,
                collar: 3,
            }],
            items: vec![ItemNet {
                id: 9,
                pos: Vec3::Y,
                stack,
                age: 2.0,
            }],
            falling: vec![(Vec3::Z, 4)],
        });
        roundtrip(Msg::Entities {
            full: true,
            mobs: vec![],
            items: vec![],
            gone_mobs: vec![],
            gone_items: vec![1, 2, 3],
            falling: vec![],
        });
        let mut bench = GunBench::default();
        let id = bench.add(stack, 0.4, -0.1, 1.2);
        bench.event = BenchEvent { serial: 3, kind: 1, gun: id, bit: 2, gone: bench.items.clone(), made: vec![5, 6] };
        roundtrip(Msg::Bench { p: IVec3::new(1, 2, 3), bench });
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

    #[test]
    fn long_chat_lines_are_cut() {
        let text: String = "é".repeat(MAX_CHAT_CHARS + 50);
        let enc = Msg::Chat { text, color: [255; 4] }.encode();
        let Some(Msg::Chat { text, .. }) = Msg::decode(&enc) else {
            panic!("not decoded");
        };
        assert_eq!(text.chars().count(), MAX_CHAT_CHARS);
    }
}
