//! The binary form of the messages: here the little-endian writer (`W`) and reader (`R`)
//! with the shared pieces (slots, poses, gun stations, player states) and the limits on what
//! a peer may send; `Msg::encode` in `encode`, `Msg::decode` in `decode`.

use super::{ItemNet, MobNet, Msg, PlayerState, Pose};
use crate::entity::{BenchEvent, BenchItem, GunBench, Grilled};
use crate::item::{Slot, Stack};
use crate::sim::felling::{FallingTree, LyingLog};
use crate::world::block::valid;
use crate::world::mesh::Notch;
use glam::{IVec3, Vec3};

mod decode;
mod encode;
#[cfg(test)]
mod tests;

// Limits on what is read from a peer: anything larger means a broken or hostile peer, and the
// message (or the connection, for a frame) is refused rather than allocated.

/// Largest frame read from a connection, in bytes.
pub(super) const MAX_FRAME: usize = 16 << 20;
/// Most slots in one list of slots (a container or an inventory).
const MAX_SLOTS: usize = 1024;
/// Most entries in any other list (blocks, poses, mobs, items, bench items).
const MAX_LIST: usize = 1 << 20;
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
    /// (a value that is not a number, or infinite, goes as 0: the other end would refuse the
    /// whole message)
    fn f32(&mut self, v: f32) {
        self.0.extend(if v.is_finite() { v } else { 0.0 }.to_le_bytes());
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
        match p.chop {
            Some(s) => {
                self.u8(1 + s.kind as u8);
                self.f32(s.clock);
                self.f32(s.hit.unwrap_or(-1.0));
            }
            None => self.u8(0),
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
            chop: self.swing()?,
        })
    }
    fn swing(&mut self) -> Option<Option<crate::model::players::chop_rig::Swing>> {
        use crate::model::players::chop_rig::{Kind, Swing};
        let kind = match self.u8()? {
            0 => return Some(None),
            1 => Kind::Chop,
            _ => Kind::Stump,
        };
        let clock = self.f32()?;
        let hit = self.f32()?;
        Some(Some(Swing { kind, clock, hit: (hit >= 0.0).then_some(hit) }))
    }
    fn rod(&mut self) -> Option<crate::model::items::angler::RodAnim> {
        let time = |v: f32| (v >= 0.0).then_some(v);
        Some(crate::model::items::angler::RodAnim {
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

/// A chat line cut to `MAX_CHAT_CHARS` characters.
fn chat_text(mut text: String) -> String {
    if let Some((i, _)) = text.char_indices().nth(MAX_CHAT_CHARS) {
        text.truncate(i);
    }
    text
}
