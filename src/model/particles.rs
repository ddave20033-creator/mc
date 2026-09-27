//! Particles: block debris, torch flames and smoke, food crumbs and the puff of a dying mob.

use crate::util::{vertex_light, Rng};
use crate::world::mesh::{flags, Vertex};
use crate::world::textures::{tex, SMOKE_FRAMES};
use crate::world::{face_texture, is_solid, World};
use glam::Vec3;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Block debris: falls and bounces, shows a quarter of the block texture.
    Debris,
    /// Torch flame: glows, hovers and shrinks away.
    Flame,
    /// Torch smoke: rises and drifts through the smoke sprites.
    Smoke,
}

pub struct Particle {
    kind: Kind,
    pos: Vec3,
    vel: Vec3,
    life: f32,
    max_life: f32,
    layer: u32,
    uv0: [f32; 2],
    size: f32,
    tint: [u8; 3],
    light: [u8; 2],
}

pub struct Particles {
    list: Vec<Particle>,
    rng: Rng,
}

impl Particles {
    pub fn new() -> Self {
        Self {
            list: Vec::new(),
            rng: Rng::new(12345),
        }
    }

    fn rand(&mut self) -> f32 {
        self.rng.next()
    }

    /// Burst of debris from a broken (or hit) block.
    pub fn burst(&mut self, world: &World, p: glam::IVec3, b: u8, count: usize, tint: [u8; 3]) {
        let layer = face_texture(b, 0);
        let sky = world.sky_estimate(p.as_vec3() + Vec3::splat(0.5));
        let blk = world.block_light_estimate(p.as_vec3() + Vec3::splat(0.5));
        for _ in 0..count {
            let off = Vec3::new(self.rand(), self.rand(), self.rand());
            let pos = p.as_vec3() + off * 0.8 + Vec3::splat(0.1);
            let vel = (off - Vec3::splat(0.5)) * 4.0 + Vec3::Y * (1.5 + self.rand() * 2.0);
            let uv0 = [self.rand() * 0.75, self.rand() * 0.75];
            let size = 0.06 + self.rand() * 0.06;
            let life = 0.5 + self.rand() * 0.7;
            self.list.push(Particle {
                kind: Kind::Debris,
                pos,
                vel,
                life,
                max_life: life,
                layer,
                uv0,
                size,
                tint,
                light: [sky, blk],
            });
        }
    }

    /// Chips of block `b` flying out of a bullet hole at `pos` on a face with normal `n`.
    pub fn impact(&mut self, world: &World, pos: Vec3, n: Vec3, b: u8, tint: [u8; 3]) {
        let layer = face_texture(b, 0);
        let sky = world.sky_estimate(pos + n * 0.3);
        let blk = world.block_light_estimate(pos + n * 0.3);
        for _ in 0..8 {
            let spread = Vec3::new(self.rand(), self.rand(), self.rand()) - Vec3::splat(0.5);
            let vel = n * (2.0 + self.rand() * 2.5) + spread * 3.0 + Vec3::Y * 1.0;
            let uv0 = [self.rand() * 0.75, self.rand() * 0.75];
            let size = 0.03 + self.rand() * 0.04;
            let life = 0.4 + self.rand() * 0.5;
            self.list.push(Particle {
                kind: Kind::Debris,
                pos,
                vel,
                life,
                max_life: life,
                layer,
                uv0,
                size,
                tint,
                light: [sky, blk],
            });
        }
    }

    /// Minecraft's torch flame particle at `pos` (just above the glowing tip).
    pub fn flame(&mut self, pos: Vec3) {
        let life = 0.4 + self.rand() * 0.4;
        let jitter = Vec3::new(self.rand() - 0.5, 0.0, self.rand() - 0.5) * 0.02;
        let size = 0.05 + self.rand() * 0.02;
        self.list.push(Particle {
            kind: Kind::Flame,
            pos: pos + jitter,
            vel: Vec3::Y * 0.04,
            life,
            max_life: life,
            layer: tex::FLAME_PARTICLE,
            uv0: [0.0, 0.0],
            size,
            tint: [255; 3],
            light: [15, 15],
        });
    }

    /// Minecraft's smoke particle: dark gray, rising and drifting, sprites from large to small.
    pub fn smoke(&mut self, pos: Vec3, sky: u8, blk: u8) {
        let life = 0.7 + self.rand() * 0.8;
        let drift = Vec3::new(self.rand() - 0.5, 0.0, self.rand() - 0.5) * 0.12;
        let gray = (40.0 + self.rand() * 50.0) as u8;
        let size = 0.05 + self.rand() * 0.03;
        let vel = drift + Vec3::Y * (0.35 + self.rand() * 0.2);
        self.list.push(Particle {
            kind: Kind::Smoke,
            pos,
            vel,
            life,
            max_life: life,
            layer: tex::SMOKE,
            uv0: [0.0, 0.0],
            size,
            tint: [gray; 3],
            light: [sky, blk],
        });
    }

    /// Bits of the food being eaten, flying out of the mouth.
    pub fn crumbs(&mut self, pos: Vec3, layer: u32, count: usize, sky: u8, blk: u8) {
        for _ in 0..count {
            let off = Vec3::new(self.rand() - 0.5, self.rand() * 0.3, self.rand() - 0.5);
            let uv0 = [0.2 + self.rand() * 0.5, 0.2 + self.rand() * 0.5];
            let size = 0.03 + self.rand() * 0.03;
            let life = 0.4 + self.rand() * 0.4;
            self.list.push(Particle {
                kind: Kind::Debris,
                pos: pos + off * 0.2,
                vel: off * 2.0 + Vec3::Y * 1.2,
                life,
                max_life: life,
                layer,
                uv0,
                size,
                tint: [255; 3],
                light: [sky, blk],
            });
        }
    }

    /// Minecraft's "poof" when a mob dies: light gray smoke puffing out of its body.
    pub fn poof(&mut self, center: Vec3, sky: u8, blk: u8) {
        for _ in 0..20 {
            let dir = Vec3::new(self.rand() - 0.5, self.rand() - 0.3, self.rand() - 0.5);
            let pos = center + dir * 0.8;
            let life = 0.4 + self.rand() * 0.5;
            let gray = (190.0 + self.rand() * 60.0) as u8;
            let size = 0.08 + self.rand() * 0.06;
            self.list.push(Particle {
                kind: Kind::Smoke,
                pos,
                vel: dir * 1.2 + Vec3::Y * 0.3,
                life,
                max_life: life,
                layer: tex::SMOKE,
                uv0: [0.0, 0.0],
                size,
                tint: [gray; 3],
                light: [sky, blk],
            });
        }
    }

    pub fn update(&mut self, dt: f32, world: &World) {
        for p in &mut self.list {
            p.life -= dt;
            if p.kind != Kind::Debris {
                p.pos += p.vel * dt;
                continue;
            }
            p.vel.y -= 18.0 * dt;
            let next = p.pos + p.vel * dt;
            if is_solid(world.get(
                next.x.floor() as i32,
                next.y.floor() as i32,
                next.z.floor() as i32,
            )) {
                p.vel *= Vec3::new(0.5, -0.2, 0.5);
            } else {
                p.pos = next;
            }
            p.vel.x *= 1.0 - dt * 2.0;
            p.vel.z *= 1.0 - dt * 2.0;
        }
        self.list.retain(|p| p.life > 0.0);
    }

    pub fn build(&self, out: &mut Vec<Vertex>, right: Vec3, up: Vec3) {
        for p in &self.list {
            // Age 0..1 over the particle's life.
            let t = (1.0 - p.life / p.max_life).clamp(0.0, 1.0);
            let (size, layer, s, fl) = match p.kind {
                Kind::Debris => (p.size, p.layer, 0.25, 0),
                Kind::Flame => (p.size * (1.0 - t * t * 0.5), p.layer, 1.0, flags::EMISSIVE),
                Kind::Smoke => {
                    let frame =
                        SMOKE_FRAMES - 1 - ((t * SMOKE_FRAMES as f32) as u32).min(SMOKE_FRAMES - 1);
                    (p.size, p.layer + frame, 1.0, 0)
                }
            };
            let (r, u) = (right * size, up * size);
            let corners = [p.pos - r - u, p.pos + r - u, p.pos + r + u, p.pos - r + u];
            let (u0, v0) = (p.uv0[0], p.uv0[1]);
            let uvs = [[u0, v0 + s], [u0 + s, v0 + s], [u0 + s, v0], [u0, v0]];
            let mut light = vertex_light(p.light[0], p.light[1]);
            light[3] = 6;
            let v: [Vertex; 4] = std::array::from_fn(|i| Vertex {
                pos: corners[i].to_array(),
                uv: uvs[i],
                layer: layer as f32,
                light,
                tint: [p.tint[0], p.tint[1], p.tint[2], fl],
            });
            out.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
        }
    }
}
