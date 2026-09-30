//! The trunk of a felled tree, lying where it fell (its leaves and branches came off as it
//! hit the ground): a row of pieces, each one block of the trunk it was. An axe cuts it up,
//! struck straight down into it (`chop_rig::Kind::Stump`): aimed at, it shows how long it is
//! and where the stroke would cut it (at a piece's end, a pale ring round the bark), and the
//! shorter part comes off as the logs it is made of.

use super::*;
use super::felling::Struck;
use crate::item::{inventory, tool_of, ToolKind};

/// How far from the eye a lying trunk can be aimed at.
const AIM_REACH: f32 = 5.0;

/// A trunk lying on the ground.
pub(super) struct LyingLog {
    pub id: u32,
    /// Its base end's middle, and the way it lies (level) from there.
    pub base: Vec3,
    pub dir: Vec3,
    /// Its pieces from the base: the trunk's blocks they were.
    pub pieces: Vec<u8>,
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

    /// Where a stroke at `s` along it would cut it: the end of a piece (1 .. len-1), or the
    /// whole of it when it is a single piece (1).
    fn cut_at(&self, s: f32) -> usize {
        let n = self.pieces.len();
        if n <= 1 {
            1
        } else {
            (s.round() as usize).clamp(1, n - 1)
        }
    }

    /// The two parts a cut at `k` leaves (from the base).
    pub fn parts(&self, k: usize) -> (usize, usize) {
        let n = self.pieces.len();
        (k.min(n), n - k.min(n))
    }
}

/// A lying trunk aimed at with an axe: which, and where a stroke would cut it.
#[derive(Clone, Copy, Debug)]
pub(super) struct LogAim {
    pub id: u32,
    pub cut: usize,
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
        out.push(LyingLog { id: out.len() as u32 + 1, base: Vec3::new(f[0], f[1], f[2]), dir, pieces });
    }
    out
}

impl Game {
    fn log_index(&self, id: u32) -> Option<usize> {
        self.lying_logs.iter().position(|l| l.id == id)
    }

    /// A felled trunk comes to lie on the ground: from `start` (the cut's middle, where the
    /// tree came down) along `dir` (level), as long as its `pieces`, on whatever is under it.
    /// Pieces that would go into something solid break off there and drop what they are.
    pub(super) fn lay_log(&mut self, start: Vec3, dir: Vec3, pieces: Vec<u8>, tool: ItemId, creative: bool) {
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
            self.next_log_id += 1;
            let id = self.next_log_id;
            self.lying_logs.push(LyingLog { id, base, dir, pieces: lying });
        }
    }

    /// The lying trunk aimed at with an axe (before any block further off): it takes the
    /// crosshair from the block then.
    pub(super) fn aim_lying_logs(&mut self, control: bool) {
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
        for l in &self.lying_logs {
            let mid = l.base + l.dir * (l.len() * 0.5);
            if mid.distance(eye) > AIM_REACH + l.len() * 0.5 + 1.0 {
                continue;
            }
            let mut t = 0.0;
            while t < block_dist.min(best.map_or(f32::MAX, |b| b.0)) {
                if let Some(s) = l.contains(eye + dir * t) {
                    best = Some((t, LogAim { id: l.id, cut: l.cut_at(s) }));
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
    pub(super) fn log_hit(&mut self, id: u32, point: Vec3) {
        let Some(i) = self.log_index(id) else { return };
        let b = self.lying_logs[i].pieces[0];
        self.chips(point.floor().as_ivec3(), b, point, Vec3::Y);
        let cut = match self.log_cut {
            Some((c_id, cut)) if c_id == id => cut,
            _ => {
                let l = &self.lying_logs[i];
                l.cut_at((point - l.base).dot(l.dir))
            }
        };
        self.struck = Some(Struck::Log(id, cut));
    }

    /// The axe pulled out of the lying trunk `id`: it comes apart at `cut`, the shorter part
    /// (the whole of a single piece) dropping its logs; the axe worn by the stroke.
    pub(super) fn cut_log(&mut self, id: u32, cut: usize) {
        let Some(i) = self.log_index(id) else { return };
        let l = &mut self.lying_logs[i];
        let n = l.pieces.len();
        let k = if n <= 1 { n } else { cut.clamp(1, n - 1) };
        // (the shorter part comes off; of two alike, the far end's)
        let (from, to) = if n <= 1 { (0, n) } else if k * 2 < n { (0, k) } else { (k, n) };
        let at: Vec<Vec3> = (from..to).map(|j| l.piece_middle(j)).collect();
        let off: Vec<u8> = l.pieces.drain(from..to).collect();
        if from == 0 && to < n {
            l.base += l.dir * to as f32;
        }
        if self.lying_logs[i].pieces.is_empty() {
            self.lying_logs.remove(i);
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
            self.needs.exhaust(crate::entity::survival::cost::MINE);
            let slot = self.hotbar_slot;
            if tool_of(held).is_some() && inventory::damage(&mut self.inventory.slots[slot], 1) {
                self.particles.burst(&self.terrain.world, at[0].floor().as_ivec3(), STONE, 12, [255; 3]);
            }
        }
    }

    /// The lying trunks (round, their pieces end to end), and where the one aimed at would
    /// be cut: a pale ring round it there.
    pub(super) fn build_lying_logs(&self, out: &mut Vec<Vertex>) {
        use crate::model::emit_item;
        let fl = crate::world::mesh::flags::ENTITY;
        let aim = self.log_aim.or(self.log_cut.map(|(id, cut)| LogAim { id, cut }));
        for l in &self.lying_logs {
            let turn = Mat4::from_quat(glam::Quat::from_rotation_arc(Vec3::Y, l.dir));
            for (i, &b) in l.pieces.iter().enumerate() {
                let mid = l.piece_middle(i);
                let (sky, blk) = self.terrain.world.light_estimate(mid + Vec3::Y * 0.3);
                let light = crate::util::vertex_light(sky, blk);
                emit_item(out, Mat4::from_translation(mid) * turn, b, light, fl);
            }
            let Some(a) = aim.filter(|a| a.id == l.id) else { continue };
            // Where it would be cut (all of it: a ring at each end).
            let marks: Vec<usize> = if l.pieces.len() <= 1 { vec![0, 1] } else { vec![a.cut.min(l.pieces.len())] };
            let layer = face_texture(l.pieces[0], 2);
            let pulse = 0.85 + 0.15 * (self.time * 6.0).sin();
            // (lit up, day or night, a little pulsing)
            let light = [255, 255, (255.0 * pulse) as u8, 0];
            for k in marks {
                let at = l.base + l.dir * k as f32;
                ring(out, Mat4::from_translation(at) * turn, l.radius() * 1.04, 0.09, layer, light, fl);
            }
        }
    }

    /// Under the crosshair, aiming at a lying trunk: how long it is, and what the stroke would
    /// cut it into.
    pub(super) fn draw_log_aim(&mut self) {
        let Some(a) = self.log_aim else { return };
        let Some(i) = self.log_index(a.id) else { return };
        let l = &self.lying_logs[i];
        let n = l.pieces.len();
        let mut line = format!("{}: {} {}", t("log.trunk"), n, t("log.blocks"));
        if n > 1 {
            let (x, y) = l.parts(a.cut);
            line += &format!("   {}: {} | {}", t("log.cut"), x, y);
        }
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        let tw = self.ui.text_width(&line, s);
        self.ui.rect(w * 0.5 - tw * 0.5 - 4.0 * s, h * 0.5 + 10.0 * s, tw + 8.0 * s, 12.0 * s, rgba(0, 0, 0, 110), 3.0 * s);
        self.ui.text_centered(&line, w * 0.5, h * 0.5 + 12.0 * s, s, rgba(255, 236, 190, 255), true);
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
