//! `Msg::encode`: a message into its bytes (its tag, then its fields).

use super::*;

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
                w.u16(*b);
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
            Msg::TakeDown { id } => {
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
                    w.u16(*b);
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
                    w.f32(m.health);
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
                    w.u16(*b);
                }
            }
            Msg::Give(s) => {
                w.u8(30);
                w.stack(*s);
            }
            Msg::BreakFx { p, block } => {
                w.u8(32);
                w.ivec3(*p);
                w.u16(*block);
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
            Msg::Notch { p, notch } => {
                w.u8(52);
                w.ivec3(*p);
                w.bool(notch.is_some());
                if let Some(n) = notch {
                    w.f32(n.angle);
                    w.f32(n.height);
                    w.f32(n.depth);
                    w.bool(n.felled);
                }
            }
            Msg::Edit(list) => {
                w.u8(58);
                w.u32(list.len() as u32);
                for (p, b) in list {
                    w.ivec3(*p);
                    w.u16(*b);
                }
            }
            Msg::Stump { p } => {
                w.u8(53);
                w.ivec3(*p);
            }
            Msg::CutLog { id, from_base } => {
                w.u8(54);
                w.u32(*id);
                w.bool(*from_base);
            }
            Msg::TreeFalls(t) => {
                w.u8(55);
                w.u32(t.id);
                w.vec3(t.pivot);
                w.vec3(t.axis);
                w.f32(t.height);
                w.u32(t.blocks.len() as u32);
                for (o, b) in &t.blocks {
                    w.vec3(*o);
                    w.u16(*b);
                }
                w.u32(t.trunk as u32);
                w.vec3(t.stump);
                w.vec3(t.stub.0);
                w.u16(t.stub.1);
                w.f32(t.stub.2);
                for c in t.leaf_tint {
                    w.u8(c);
                }
            }
            Msg::Collect { item, by } => {
                w.u8(59);
                w.u32(*item);
                w.u8(*by);
            }
            Msg::TreeLands { id } => {
                w.u8(56);
                w.u32(*id);
            }
            Msg::Logs(list) => {
                w.u8(57);
                w.u32(list.len() as u32);
                for l in list {
                    w.u32(l.id);
                    w.vec3(l.base);
                    w.vec3(l.dir);
                    w.u32(l.pieces.len() as u32);
                    for b in &l.pieces {
                        w.u16(*b);
                    }
                    w.u8(l.next as u8);
                }
            }
            Msg::Fx { kind, pos } => {
                w.u8(51);
                w.u8(*kind);
                w.vec3(*pos);
            }
            Msg::Pause(on) => {
                w.u8(50);
                w.u8(*on as u8);
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
}
