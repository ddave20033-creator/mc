//! The trunk of a felled tree, lying where it fell (its leaves and branches came off as it
//! hit the ground): a row of pieces, each one block of the trunk it was. An axe cuts it up,
//! struck straight down into it (`chop_rig::Kind::Stump`): each stroke takes a piece of one
//! to three blocks (as it happens) off the end nearer where it is aimed, which drops the logs
//! it is made of; aimed at, a pale ring round the bark shows where it will come off. The last
//! block left is not cut: it comes apart by itself.

use crate::game::*;
use crate::game::felling::Struck;
use crate::item::{tool_of, ToolKind};

/// How far from the eye a lying trunk can be aimed at.
const AIM_REACH: f32 = 5.0;

/// A trunk lying on the ground.
pub(in crate::game) struct LyingLog {
    pub id: u32,
    /// Its base end's middle, and the way it lies (level) from there.
    pub base: Vec3,
    pub dir: Vec3,
    /// Its pieces from the base: the trunk's blocks they were.
    pub pieces: Vec<u8>,
    /// How many blocks the next stroke takes off (1 to 3, rolled after each).
    pub next: usize,
}

impl LyingLog {
    fn len(&self) -> f32 {
        self.pieces.len() as f32
    }

    fn radius(&self) -> f32 {
        self.pieces.first().map_or(0.44, |&b| log_radius(b))
    }

    /// How far along it (from its base) the point `q` is, if it is in its wood.
    pub fn contains(&self, q: Vec3) -> Option<f32> {
        let s = (q - self.base).dot(self.dir);
        if !(0.0..=self.len()).contains(&s) {
            return None;
        }
        let off = q - (self.base + self.dir * s);
        (off.length() <= self.radius()).then_some(s)
    }

    /// The middle of the piece `i`.
    fn piece_middle(&self, i: usize) -> Vec3 {
        self.base + self.dir * (i as f32 + 0.5)
    }

    /// Which end a stroke at `s` along it takes the next piece off: the nearer one.
    fn from_base_at(&self, s: f32) -> bool {
        s < self.len() * 0.5
    }

    /// The pieces the next stroke takes off from that end (the rest too, if only one would be
    /// left).
    fn taken(&self, from_base: bool) -> std::ops::Range<usize> {
        let n = self.pieces.len();
        let k = self.next.clamp(1, 3).min(n);
        let k = if n - k <= 1 { n } else { k };
        if from_base {
            0..k
        } else {
            n - k..n
        }
    }
}

/// A lying trunk aimed at with an axe: which, and which end a stroke would cut from.
#[derive(Clone, Copy, Debug)]
pub(in crate::game) struct LogAim {
    pub id: u32,
    pub from_base: bool,
}

/// Lying trunks saved with a world: `x,y,z,dx,dz,piece;piece;...` a line.
pub fn logs_text(logs: &[LyingLog]) -> String {
    logs.iter()
        .map(|l| {
            let pieces: Vec<String> = l.pieces.iter().map(|b| b.to_string()).collect();
            format!("{},{},{},{},{},{}\n", l.base.x, l.base.y, l.base.z, l.dir.x, l.dir.z, pieces.join(";"))
        })
        .collect()
}

pub fn parse_logs(text: &str) -> Vec<LyingLog> {
    let mut out = Vec::new();
    for line in text.lines() {
        let v: Vec<&str> = line.trim().split(',').collect();
        if v.len() != 6 {
            continue;
        }
        let f: Vec<f32> = v[..5].iter().filter_map(|x| x.parse().ok()).collect();
        let pieces: Vec<u8> = v[5].split(';').filter_map(|x| x.parse().ok()).filter(|&b| is_log(b)).collect();
        if f.len() != 5 || pieces.is_empty() {
            continue;
        }
        let dir = Vec3::new(f[3], 0.0, f[4]).normalize_or_zero();
        if dir == Vec3::ZERO {
            continue;
        }
        let id = out.len() as u32 + 1;
        out.push(LyingLog { id, base: Vec3::new(f[0], f[1], f[2]), dir, pieces, next: 1 + id as usize % 3 });
    }
    out
}

impl Game {
    fn log_index(&self, id: u32) -> Option<usize> {
        self.level.lying_logs.iter().position(|l| l.id == id)
    }

    /// A felled trunk comes to lie on the ground: from `start` (the cut's middle, where the
    /// tree came down) along `dir` (level), as long as its `pieces`, on whatever is under it.
    /// Pieces that would go into something solid break off there and drop what they are.
    pub(in crate::game) fn lay_log(&mut self, start: Vec3, dir: Vec3, pieces: Vec<u8>, tool: ItemId, creative: bool) {
        let dir = Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();
        if dir == Vec3::ZERO || pieces.is_empty() {
            return;
        }
        let w = &self.terrain.world;
        let r = log_radius(pieces[0]);
        let solid = |q: Vec3| {
            let b = w.geti(q.floor().as_ivec3());
            is_solid(b) && !is_leaves(b)
        };
        // The ground under a point: the top of the first solid block below it.
        let ground = |at: Vec3| {
            let mut y = at.y.floor();
            for _ in 0..12 {
                if solid(Vec3::new(at.x, y - 0.5, at.z)) {
                    return y;
                }
                y -= 1.0;
            }
            y
        };
        // It rests on the highest ground under its first pieces.
        let reach = pieces.len().min(4);
        let mut floor = f32::MIN;
        for i in 0..reach {
            let at = start + dir * (i as f32 + 0.5) + Vec3::Y * 0.5;
            floor = floor.max(ground(at));
        }
        let base = Vec3::new(start.x, floor + r, start.z);
        // It goes as far as it is free of the world.
        let free = pieces
            .iter()
            .enumerate()
            .take_while(|&(i, _)| {
                let m = base + dir * (i as f32 + 0.5);
                !solid(m) && !solid(m + Vec3::Y * (r * 0.5))
            })
            .count();
        // (a single block left lying comes apart at once)
        let free = if free == 1 { 0 } else { free };
        let (lying, broken) = pieces.split_at(free);
        let lying = lying.to_vec();
        for (i, &b) in broken.iter().enumerate() {
            let at = base + dir * ((free + i) as f32 + 0.5);
            let q = at.floor().as_ivec3();
            self.particles.burst(&self.terrain.world, q, b, 6, [255; 3]);
            if !creative {
                let r = self.random();
                for s in crate::item::drops(b, tool, r) {
                    let mut at = at;
                    while is_solid(self.terrain.world.geti(at.floor().as_ivec3())) && at.y < base.y + 30.0 {
                        at.y += 1.0;
                    }
                    self.spawn_drop(at, s);
                }
            }
        }
        if !lying.is_empty() {
            self.level.next_log_id += 1;
            let id = self.level.next_log_id;
            let next = 1 + (self.random() * 3.0) as usize;
            self.level.lying_logs.push(LyingLog { id, base, dir, pieces: lying, next });
        }
    }

    /// The lying trunk aimed at with an axe (before any block further off): it takes the
    /// crosshair from the block then.
    pub(in crate::game) fn aim_lying_logs(&mut self, control: bool) {
        self.log_aim = None;
        if !control || self.is_client() || !matches!(tool_of(self.held()), Some((ToolKind::Axe, _))) {
            return;
        }
        // (a swing going on keeps to the trunk it began on)
        let eye = self.player.eye();
        let dir = look_dir(self.yaw, self.pitch);
        let block_dist = self
            .target
            .and_then(|(hit, _)| crate::util::ray_box(eye, dir, hit.as_vec3(), hit.as_vec3() + Vec3::ONE, AIM_REACH))
            .unwrap_or(AIM_REACH);
        let mut best: Option<(f32, LogAim)> = None;
        for l in &self.level.lying_logs {
            let mid = l.base + l.dir * (l.len() * 0.5);
            if mid.distance(eye) > AIM_REACH + l.len() * 0.5 + 1.0 {
                continue;
            }
            let mut t = 0.0;
            while t < block_dist.min(best.map_or(f32::MAX, |b| b.0)) {
                if let Some(s) = l.contains(eye + dir * t) {
                    best = Some((t, LogAim { id: l.id, from_base: l.from_base_at(s) }));
                    break;
                }
                t += 0.02;
            }
        }
        if let Some((_, aim)) = best {
            self.log_aim = Some(aim);
            self.target = None;
            self.mining = None;
        }
    }

    /// The axe struck down into the lying trunk `id`: chips fly from where it went in.
    pub(in crate::game) fn log_hit(&mut self, id: u32, point: Vec3) {
        let Some(i) = self.log_index(id) else { return };
        let b = self.level.lying_logs[i].pieces[0];
        self.chips(point.floor().as_ivec3(), b, point, Vec3::Y);
        let from_base = match self.log_cut {
            Some((c_id, from_base)) if c_id == id => from_base,
            _ => {
                let l = &self.level.lying_logs[i];
                l.from_base_at((point - l.base).dot(l.dir))
            }
        };
        self.struck = Some(Struck::Log(id, from_base));
    }

    /// The axe pulled out of the lying trunk `id`: the next piece comes off the end it was
    /// struck nearer (`from_base`) and drops its logs (the last block left with it); the axe
    /// worn by the stroke.
    pub(in crate::game) fn cut_log(&mut self, id: u32, from_base: bool) {
        let Some(i) = self.log_index(id) else { return };
        let next = 1 + (self.random() * 3.0) as usize;
        let l = &mut self.level.lying_logs[i];
        let taken = l.taken(from_base);
        let at: Vec<Vec3> = taken.clone().map(|j| l.piece_middle(j)).collect();
        let off: Vec<u8> = l.pieces.drain(taken.clone()).collect();
        if taken.start == 0 {
            l.base += l.dir * taken.end as f32;
        }
        l.next = next;
        if self.level.lying_logs[i].pieces.is_empty() {
            self.level.lying_logs.remove(i);
        }
        let held = self.held();
        let creative = self.creative();
        for (&b, &p) in off.iter().zip(&at) {
            let q = p.floor().as_ivec3();
            self.particles.burst(&self.terrain.world, q, b, 8, [255; 3]);
            if !creative {
                let r = self.random();
                for s in crate::item::drops(b, held, r) {
                    self.spawn_drop(p, s);
                }
            }
        }
        if !creative {
            self.wear_axe(crate::entity::survival::cost::MINE, at[0].floor().as_ivec3());
        }
    }

    /// The lying trunks (round, their pieces end to end), and where the one aimed at would
    /// be cut: a pale ring round it there.
    /// Those within `sight` of `eye`.
    pub(in crate::game) fn build_lying_logs(&self, out: &mut Vec<Vertex>, eye: Vec3, sight: f32) {
        use crate::model::emit_item;
        let fl = crate::world::mesh::flags::ENTITY;
        let aim = self.log_aim.or(self.log_cut.map(|(id, from_base)| LogAim { id, from_base }));
        for l in &self.level.lying_logs {
            let length = l.pieces.len() as f32;
            if (l.base + l.dir * length * 0.5).distance(eye) > sight + length {
                continue;
            }
            let turn = Mat4::from_quat(glam::Quat::from_rotation_arc(Vec3::Y, l.dir));
            for (i, &b) in l.pieces.iter().enumerate() {
                let mid = l.piece_middle(i);
                let (sky, blk) = self.terrain.world.light_estimate(mid + Vec3::Y * 0.3);
                let light = crate::util::vertex_light(sky, blk);
                emit_item(out, Mat4::from_translation(mid) * turn, b, light, fl);
            }
            let Some(a) = aim.filter(|a| a.id == l.id) else { continue };
            // Where it will be cut (all of it going: a ring at each end).
            let n = l.pieces.len();
            let taken = l.taken(a.from_base);
            let marks: Vec<usize> = if taken.len() == n { vec![0, n] } else if a.from_base { vec![taken.end] } else { vec![taken.start] };
            let layer = face_texture(PLANKS, 2);
            let pulse = 0.85 + 0.15 * (self.time * 6.0).sin();
            // (lit up, day or night, a little pulsing)
            let light = [255, 255, (255.0 * pulse) as u8, 0];
            for k in marks {
                let at = l.base + l.dir * k as f32;
                ring(out, Mat4::from_translation(at) * turn, l.radius() * 1.04, 0.09, layer, light, fl);
            }
        }
    }

}

/// A thin band round a trunk (`m`: its middle, the trunk along +Y), `radius` out and `wide`
/// along it, in the pale wood of its cut ends.
fn ring(out: &mut Vec<Vertex>, m: Mat4, radius: f32, wide: f32, layer: u32, light: [u8; 4], fl: u8) {
    const SIDES: usize = 16;
    let at = |a: f32, y: f32| m.transform_point3(Vec3::new(a.cos() * radius, y, a.sin() * radius)).to_array();
    for i in 0..SIDES {
        let (a0, a1) = (i as f32 / SIDES as f32 * TAU, (i + 1) as f32 / SIDES as f32 * TAU);
        let n = m.transform_vector3(Vec3::new(((a0 + a1) * 0.5).cos(), 0.0, ((a0 + a1) * 0.5).sin()));
        let face = {
            let a = n.abs();
            let k = if a.x >= a.y && a.x >= a.z { 0 } else if a.y >= a.z { 1 } else { 2 };
            (k * 2 + (n[k] < 0.0) as usize) as u8
        };
        let (u0, u1) = (i as f32 / SIDES as f32, (i + 1) as f32 / SIDES as f32);
        let ps = [at(a0, -wide * 0.5), at(a1, -wide * 0.5), at(a1, wide * 0.5), at(a0, wide * 0.5)];
        let uvs = [[u0, 0.45], [u1, 0.45], [u1, 0.55], [u0, 0.55]];
        let v: [Vertex; 4] = std::array::from_fn(|j| Vertex {
            pos: ps[j],
            uv: uvs[j],
            layer: layer as f32,
            light: [light[0], light[1], light[2], face],
            tint: [255, 255, 255, fl],
        });
        out.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
        out.extend_from_slice(&[v[0], v[2], v[1], v[0], v[3], v[2]]);
    }
}
