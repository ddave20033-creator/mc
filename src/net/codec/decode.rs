//! `Msg::decode`: a message back from its bytes, refused (`None`) if anything in it is out
//! of bounds, unknown or left over.

use super::*;

impl Msg {
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
                b: valid(r.u16()?),
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
            48 => Msg::TakeDown { id: r.u32()? },
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
            24 => Msg::Blocks(r.list(|r| Some((r.ivec3()?, valid(r.u16()?))))?),
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
                        health: r.f32()?,
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
                falling: r.list(|r| Some((r.vec3()?, valid(r.u16()?))))?,
            },
            30 => Msg::Give(r.stack()?),
            32 => Msg::BreakFx {
                p: r.ivec3()?,
                block: valid(r.u16()?),
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
            50 => Msg::Pause(r.u8()? != 0),
            51 => Msg::Fx { kind: r.u8()?, pos: r.vec3()? },
            52 => Msg::Notch {
                p: r.ivec3()?,
                notch: if r.bool()? {
                    Some(Notch { angle: r.f32()?, height: r.f32()?, depth: r.f32()?, felled: r.bool()? })
                } else {
                    None
                },
            },
            53 => Msg::Stump { p: r.ivec3()? },
            58 => Msg::Edit(r.list(|r| Some((r.ivec3()?, valid(r.u16()?))))?),
            54 => Msg::CutLog { id: r.u32()?, from_base: r.bool()? },
            55 => Msg::TreeFalls(Box::new(FallingTree {
                id: r.u32()?,
                pivot: r.vec3()?,
                axis: r.vec3()?,
                height: r.f32()?,
                blocks: r.list(|r| Some((r.vec3()?, valid(r.u16()?))))?,
                trunk: r.u32()? as usize,
                stump: r.vec3()?,
                stub: (r.vec3()?, valid(r.u16()?), r.f32()?),
                leaf_tint: [r.u8()?, r.u8()?, r.u8()?],
                angle: 0.02,
                prev_angle: 0.02,
                speed: 0.15,
                tool: crate::item::NONE,
                creative: false,
            })),
            56 => Msg::TreeLands { id: r.u32()? },
            59 => Msg::Collect { item: r.u32()?, by: r.u8()? },
            57 => Msg::Logs(r.list(|r| {
                Some(LyingLog {
                    id: r.u32()?,
                    base: r.vec3()?,
                    dir: r.vec3()?,
                    pieces: r.list(|r| Some(valid(r.u16()?)))?,
                    next: r.u8()? as usize,
                })
            })?),
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
