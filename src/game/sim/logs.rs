//! The trunk of a felled tree, lying where it fell (its leaves and branches came off as it
//! hit the ground): a row of pieces, each one block of the trunk it was. An axe cuts it up,
//! struck straight down into it (`chop_rig::Kind::Stump`): each stroke takes a piece of one
//! to three blocks (as it happens) off the end nearer where it is aimed, which drops the logs
//! it is made of; aimed at, a pale ring round the bark shows where it will come off. The last
//! block left is not cut: it comes apart by itself.

use crate::game::*;
use crate::game::felling::Struck;
use crate::sim::felling::*;
use crate::item::{tool_of, ToolKind};

/// How far from the eye a lying trunk can be aimed at.
const AIM_REACH: f32 = 5.0;

/// A lying trunk aimed at with an axe: which, and which end a stroke would cut from.
#[derive(Clone, Copy, Debug)]
pub(in crate::game) struct LogAim {
    pub id: u32,
    pub from_base: bool,
}

impl Game {
    fn log_index(&self, id: u32) -> Option<usize> {
        self.level.lying_logs.iter().position(|l| l.id == id)
    }

    /// A felled trunk comes to lie on the ground: from `start` (the cut's middle, where the
    /// tree came down) along `dir` (level), as long as its `pieces`, on whatever is under it.
    /// Pieces that would go into something solid break off there and drop what they are.
    pub(in crate::game) fn lay_log(&mut self, start: Vec3, dir: Vec3, pieces: Vec<Block>, tool: ItemId, creative: bool) {
        let Some((base, dir, free)) = log_rest(&self.terrain.world, start, dir, &pieces) else { return };
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
        if !control || !matches!(tool_of(self.held()), Some((ToolKind::Axe, _))) {
            return;
        }
        // (a swing going on keeps to the trunk it began on)
        let eye = self.eye();
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
            if let Some((t, s)) = l.ray_hit(eye, dir, block_dist.min(best.map_or(f32::MAX, |b| b.0))) {
                best = Some((t, LogAim { id: l.id, from_base: l.from_base_at(s) }));
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
        if self.is_client() {
            // The server cuts it for real (and drops the logs); here it shows at once.
            self.send(crate::net::Msg::CutLog { id, from_base });
        }
        let next = 1 + (self.random() * 3.0) as usize;
        let l = &mut self.level.lying_logs[i];
        let taken = l.taken(from_base);
        let at: Vec<Vec3> = taken.clone().map(|j| l.piece_middle(j)).collect();
        let off: Vec<Block> = l.pieces.drain(taken.clone()).collect();
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
            if !creative && !self.is_client() {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The first point stepped along the ray that is in the wood (as the aim used to find it).
    fn stepped(l: &LyingLog, o: Vec3, d: Vec3, max: f32) -> Option<(f32, f32)> {
        let mut t = 0.0;
        while t < max {
            if let Some(s) = l.contains(o + d * t) {
                return Some((t, s));
            }
            t += 0.002;
        }
        None
    }

    #[test]
    fn a_ray_meets_a_lying_trunk_where_stepping_along_it_does() {
        let mut rng = crate::util::Rng::new(7);
        let mut hits = 0;
        for k in 0..400 {
            let dir = Vec3::new(rng.next() - 0.5, (rng.next() - 0.5) * 0.3, rng.next() - 0.5).normalize();
            let l = LyingLog { id: k, base: Vec3::new(0.3, 64.4, -0.7), dir, pieces: vec![crate::world::OAK_LOG; 1 + (k % 5) as usize], next: 1 };
            let o = l.base + Vec3::new(rng.next() - 0.5, rng.next() * 0.6 + 0.8, rng.next() - 0.5) * 6.0;
            let target = l.base + dir * (rng.next() * l.len());
            let d = (target - o).normalize();
            let exact = l.ray_hit(o, d, 8.0);
            let step = stepped(&l, o, d, 8.0);
            match (exact, step) {
                (Some((t, s)), Some((ts, ss))) => {
                    hits += 1;
                    assert!(t <= ts + 1e-4 && ts - t < 0.003, "{t} {ts}");
                    assert!((s - ss).abs() < 0.01, "{s} {ss}");
                }
                (None, None) => {}
                // (only a ray grazing the wood between two steps)
                (Some((t, _)), None) => assert!(l.contains(o + d * (t + 1e-3)).is_none() || t > 7.99),
                (None, Some(_)) => panic!("stepping found a hit the exact test missed"),
            }
        }
        assert!(hits > 100, "{hits}");
    }
}
