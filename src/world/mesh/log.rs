//! Round logs and branches, and the axe's cuts in upright trunks.

use super::*;

/// How far out a round log's end reaches in its end texture (0.5 is the edge): just into
/// the bark ring round the wood.
pub const LOG_END_RIM: f32 = 0.47;

/// A stump's heights in its block (a trunk `radius` thick): its flat top, and the top of the
/// hinge left standing on its far side (the cut's middle).
pub fn stump_heights(notch: Notch, radius: f32) -> (f32, f32) {
    let h = notch.height.clamp(0.12, 0.88);
    let deep = notch.depth.clamp(0.0, 1.0) * 2.0 * radius;
    let low = (h - (deep * 0.8).max(0.08)).max(0.02);
    ((low + h) * 0.5, h)
}

impl Builder {
    /// A log or a branch, round: a many-sided cylinder along its axis (logs thick, branches
    /// thin), its bark around it. An end joining the same kind of log goes on into it; one
    /// meeting a log across (a branch out of a trunk, a branch turning up) reaches on into
    /// its middle, so the joint is closed; a free end is capped with the rings.
    pub(super) fn round_log(&mut self, r: &Region, x: i32, y: i32, z: i32, b: u8) {
        let axis = log_axis(b);
        let radius = log_radius(b);
        let sides = if is_branch(b) { 8 } else { 12 };
        // Block-local position from (along the axis, u, v across it).
        let (u_axis, v_axis) = match axis {
            0 => (2, 1),
            1 => (0, 2),
            _ => (0, 1),
        };
        let at = |t: f32, u: f32, v: f32| {
            let mut p = [0.5f32; 3];
            p[axis] = t;
            p[u_axis] = 0.5 + u;
            p[v_axis] = 0.5 + v;
            p
        };
        let mut step = [0i32; 3];
        step[axis] = 1;
        // Another log comes into this one across (a branch goes on from here another way).
        let elbow = (0..3).filter(|&k| k != axis).any(|k| {
            [-1i32, 1].into_iter().any(|dir| {
                let mut d = [0i32; 3];
                d[k] = dir;
                let nb = r.get(x + d[0], y + d[1], z + d[2]);
                is_log(nb) && log_axis(nb) == k && log_radius(nb) <= radius + 0.01
            })
        });
        let mut ends = [(0.0f32, true), (1.0f32, true)];
        for (k, dir) in [-1i32, 1].into_iter().enumerate() {
            let nb = r.get(x + step[0] * dir, y + step[1] * dir, z + step[2] * dir);
            let t = if dir < 0 { 0.0 } else { 1.0 };
            ends[k] = if is_log(nb) && log_axis(nb) == axis {
                // Goes on into the next one (a thicker one here shows its end ring).
                (t, radius > log_radius(nb) + 0.01)
            } else if is_log(nb) && radius <= log_radius(nb) + 0.01 {
                (t + 0.5 * dir as f32, false)
            } else if is_branch(b) && elbow {
                // A branch turning (up, or aside): it stops just past the middle, where the
                // one going on from here closes round it, not out into the air.
                (0.5 + radius * dir as f32, true)
            } else {
                (t, true)
            };
        }
        let (s, bl) = r.light(x, y, z);
        let side_layer = face_texture(b, if axis == 1 { 0 } else { 2 });
        let end_layer = face_texture(b, if axis == 1 { 2 } else { 0 });
        let (t0, t1) = (ends[0].0, ends[1].0);
        // (no side faces straight along an axis: two logs crossing never have sides in the
        // same plane, which would flicker)
        let angle = |i: usize| i as f32 / sides as f32 * std::f32::consts::TAU;
        let around = if is_branch(b) { 1.0 } else { 3.0 };
        let (wx, wz) = ((x + self.ox) as f32, (z + self.oz) as f32);
        let world = move |p: [f32; 3]| [wx + p[0], y as f32 + p[1], wz + p[2]];
        let face_of = |n: [f32; 3]| {
            let a = n.map(f32::abs);
            let k = if a[0] >= a[1] && a[0] >= a[2] { 0 } else if a[1] >= a[2] { 1 } else { 2 };
            (k * 2 + (n[k] < 0.0) as usize) as u8
        };
        // A corner of the bark at `a` round: its smooth normal (see world.vert).
        let round_n = |a: f32| (16 + axis * 64 + ((a / std::f32::consts::TAU * 64.0).round() as usize % 64)) as u8;
        let emit = |b: &mut Self, ps: [[f32; 3]; 4], uvs: [[f32; 2]; 4], n: [f32; 3], layer: u32, ns: Option<[u8; 4]>| {
            // Wound to face `n`.
            let d = |a: [f32; 3], c: [f32; 3]| [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let (e1, e2) = (d(ps[0], ps[1]), d(ps[0], ps[2]));
            let cross = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            let flip = cross[0] * n[0] + cross[1] * n[1] + cross[2] * n[2] < 0.0;
            let base = b.verts.len() as u32;
            let face = face_of(n);
            for i in 0..4 {
                b.push(Vertex {
                    pos: world(ps[i]),
                    uv: uvs[i],
                    layer: layer as f32,
                    light: [255, (s * 17) as u8, (bl * 17) as u8, ns.map_or(face, |ns| ns[i])],
                    tint: [255, 255, 255, 0],
                });
            }
            let idx = if flip { [base, base + 2, base + 1, base, base + 3, base + 2] } else { [base, base + 1, base + 2, base, base + 2, base + 3] };
            b.opaque.extend_from_slice(&idx);
        };
        if axis == 1 && !is_branch(b) {
            if let Some(notch) = self.notch_at(IVec3::new(x + self.ox, y, z + self.oz)) {
                self.notched_log(notch, radius, sides, around, (t0, ends[0].1), (t1, ends[1].1), side_layer, end_layer, &emit, &round_n, &at);
                return;
            }
        }
        for i in 0..sides {
            let (a0, a1) = (angle(i), angle(i + 1));
            let (c0, s0, c1, s1) = (a0.cos() * radius, a0.sin() * radius, a1.cos() * radius, a1.sin() * radius);
            let mid = (a0 + a1) * 0.5;
            let mut n = [0.0f32; 3];
            n[u_axis] = mid.cos();
            n[v_axis] = mid.sin();
            let (u0, u1) = (i as f32 / sides as f32 * around, (i + 1) as f32 / sides as f32 * around);
            emit(
                self,
                [at(t0, c0, s0), at(t0, c1, s1), at(t1, c1, s1), at(t1, c0, s0)],
                [[u0, 1.0 - t0], [u1, 1.0 - t0], [u1, 1.0 - t1], [u0, 1.0 - t1]],
                n,
                side_layer,
                Some([round_n(a0), round_n(a1), round_n(a1), round_n(a0)]),
            );
            // The end caps: a slice of the rings each.
            for (k, &(t, capped)) in ends.iter().enumerate() {
                if !capped {
                    continue;
                }
                let mut n = [0.0f32; 3];
                n[axis] = if k == 0 { -1.0 } else { 1.0 };
                // The whole end (rings and the bark round them) at any thickness.
                let k = LOG_END_RIM / radius;
                let uv = |u: f32, v: f32| [0.5 + u * k, 0.5 + v * k];
                emit(
                    self,
                    [at(t, 0.0, 0.0), at(t, c0, s0), at(t, c1, s1), at(t, 0.0, 0.0)],
                    [uv(0.0, 0.0), uv(c0, s0), uv(c1, s1), uv(0.0, 0.0)],
                    n,
                    end_layer,
                    None,
                );
            }
        }
    }

    /// An upright trunk with an axe's cut in it (`Notch`): a wedge taken out of its side,
    /// deepest at the cut's middle, in steps like chips hewn out one after another. The
    /// trunk is drawn in slices: whole below and above the cut, and each slice of the cut
    /// the round cross-section with the part past the cut's face gone (its face and the
    /// steps between the slices bare wood).
    #[allow(clippy::too_many_arguments)]
    fn notched_log(
        &mut self,
        notch: Notch,
        radius: f32,
        sides: usize,
        around: f32,
        (t0, cap0): (f32, bool),
        (t1, cap1): (f32, bool),
        side_layer: u32,
        end_layer: u32,
        emit: &dyn Fn(&mut Self, [[f32; 3]; 4], [[f32; 2]; 4], [f32; 3], u32, Option<[u8; 4]>),
        round_n: &dyn Fn(f32) -> u8,
        at: &dyn Fn(f32, f32, f32) -> [f32; 3],
    ) {
        use std::f32::consts::TAU;
        const SLICES: usize = 6;
        let dir = [notch.angle.cos(), notch.angle.sin()];
        let across = [-dir[1], dir[0]];
        let deep = notch.depth.clamp(0.0, 1.0) * 2.0 * radius;
        let half = (deep * 0.8).max(0.08);
        let h = notch.height.clamp(0.12, 0.88);
        let (z0, z1) = ((h - half).max(t0 + 0.02), (h + half).min(t1 - 0.02));
        let ring: Vec<[f32; 2]> = (0..sides)
            .map(|i| {
                let a = i as f32 / sides as f32 * TAU;
                [a.cos() * radius, a.sin() * radius]
            })
            .collect();
        // The cross-section with everything past `c` along the cut's direction gone.
        let clip = |c: f32| -> Vec<[f32; 2]> {
            let d = |p: [f32; 2]| p[0] * dir[0] + p[1] * dir[1] - c;
            let mut out = Vec::new();
            for i in 0..ring.len() {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                let (da, db) = (d(a), d(b));
                if da <= 0.0 {
                    out.push(a);
                }
                if (da <= 0.0) != (db <= 0.0) {
                    let k = da / (da - db);
                    out.push([a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k]);
                }
            }
            out
        };
        let rim = LOG_END_RIM / radius;
        // One slice from `ya` up to `yb` of the cross-section `poly` (cut at `c`), with its
        // bottom and top ends if asked.
        let slice = |m: &mut Self, poly: &[[f32; 2]], c: f32, ya: f32, yb: f32, bottom: bool, top: bool| {
            if poly.len() < 3 || yb - ya < 1e-4 {
                return;
            }
            let on_cut = |p: [f32; 2]| (p[0] * dir[0] + p[1] * dir[1] - c).abs() < 1e-4;
            for i in 0..poly.len() {
                let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
                if on_cut(p) && on_cut(q) {
                    // The cut's face: bare wood, its grain along the trunk.
                    let u = |p: [f32; 2]| 0.5 + p[0] * across[0] + p[1] * across[1];
                    let n = [dir[0], 0.0, dir[1]];
                    emit(
                        m,
                        [at(ya, p[0], p[1]), at(ya, q[0], q[1]), at(yb, q[0], q[1]), at(yb, p[0], p[1])],
                        [[u(p), 1.0 - ya], [u(q), 1.0 - ya], [u(q), 1.0 - yb], [u(p), 1.0 - yb]],
                        n,
                        end_layer,
                        None,
                    );
                    continue;
                }
                let (ap, mut aq) = (p[1].atan2(p[0]).rem_euclid(TAU), q[1].atan2(q[0]).rem_euclid(TAU));
                if aq < ap {
                    aq += TAU;
                }
                let mid = (ap + aq) * 0.5;
                let (up, uq) = (ap / TAU * around, aq / TAU * around);
                emit(
                    m,
                    [at(ya, p[0], p[1]), at(ya, q[0], q[1]), at(yb, q[0], q[1]), at(yb, p[0], p[1])],
                    [[up, 1.0 - ya], [uq, 1.0 - ya], [uq, 1.0 - yb], [up, 1.0 - yb]],
                    [mid.cos(), 0.0, mid.sin()],
                    side_layer,
                    Some([round_n(ap), round_n(aq), round_n(aq), round_n(ap)]),
                );
            }
            let n = poly.len() as f32;
            let mid = poly.iter().fold([0.0, 0.0], |s, p| [s[0] + p[0] / n, s[1] + p[1] / n]);
            let uv = |p: [f32; 2]| [0.5 + p[0] * rim, 0.5 + p[1] * rim];
            for (y, up, on) in [(ya, -1.0, bottom), (yb, 1.0, top)] {
                if !on {
                    continue;
                }
                for i in 0..poly.len() {
                    let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
                    emit(
                        m,
                        [at(y, mid[0], mid[1]), at(y, p[0], p[1]), at(y, q[0], q[1]), at(y, mid[0], mid[1])],
                        [uv(mid), uv(p), uv(q), uv(mid)],
                        [0.0, up, 0.0],
                        end_layer,
                        None,
                    );
                }
            }
        };
        let whole = clip(f32::INFINITY);
        if notch.felled {
            // A stump: cut flat, and on the far side the hinge the tree broke off at, a ridge
            // of the wood the cut had not reached, up to the cut's middle.
            let (flat, hinge) = stump_heights(notch, radius);
            let c = radius - deep;
            slice(self, &whole, f32::INFINITY, t0, flat, cap0, true);
            slice(self, &clip(c), c, flat, hinge, false, true);
            return;
        }
        slice(self, &whole, f32::INFINITY, t0, z0, cap0, true);
        let n = SLICES;
        for k in 0..n {
            let ya = z0 + (z1 - z0) * k as f32 / n as f32;
            let yb = z0 + (z1 - z0) * (k + 1) as f32 / n as f32;
            let dy = ((ya + yb) * 0.5 - h).abs();
            let c = radius - deep * (1.0 - dy / half).max(0.0);
            slice(self, &clip(c), c, ya, yb, true, true);
        }
        slice(self, &whole, f32::INFINITY, z1, t1, true, cap1);
    }
}
