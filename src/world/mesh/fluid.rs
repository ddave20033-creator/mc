//! Water and lava: surfaces sloping toward their neighbours, animated as they flow.

use super::*;

impl Builder {
    /// Fluid block. Vertices carry the previous surface height and the change time in `uv`
    /// (the shader animates between them and derives texture coordinates from position),
    /// and the flow direction in `tint`.
    pub(super) fn fluid(&mut self, r: &Region, x: i32, y: i32, z: i32, b: u8) {
        let lava = is_lava(b);
        let same = |q: u8| if lava { is_lava(q) } else { is_water(q) };
        let above_same = same(r.get(x, y + 1, z));
        let now_get = |x: i32, y: i32, z: i32| r.get(x, y, z);
        let h = surface_heights(&now_get, x, y, z, lava);

        // Previous state, if anything around this block changed recently.
        let mut start = -1.0e6f32;
        for dy in 0..=1 {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    if let Some(&(_, t)) = r.old.get(&(x + dx, y + dy, z + dz)) {
                        start = start.max(t);
                    }
                }
            }
        }
        // Only changes from that latest tick are "in progress"; earlier ones already finished animating.
        let recent = |x: i32, y: i32, z: i32| {
            r.old
                .get(&(x, y, z))
                .filter(|o| o.1 > start - 0.01)
                .map(|o| o.0)
        };
        let h_old = if start > -1.0e5 {
            let old_get =
                |x: i32, y: i32, z: i32| recent(x, y, z).unwrap_or_else(|| r.get(x, y, z));
            surface_heights(&old_get, x, y, z, lava)
        } else {
            h
        };

        // A block that just filled with fluid grows out of whatever fed it: downward from
        // above, or sideways out of the lowest-level horizontal neighbour.
        let is_new = recent(x, y, z).is_some_and(|o| !same(o));
        let mut feeder: Option<(i32, i32)> = None;
        if is_new && !above_same {
            let mut best = u8::MAX;
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let q = r.get(x + dx, y, z + dz);
                if same(q) {
                    let l = fluid_level(q);
                    let eff = if l >= FALLING { 0 } else { l };
                    if eff < best {
                        best = eff;
                        feeder = Some((dx, dz));
                    }
                }
            }
        }

        // Flow direction: away from the feeder for new blocks, otherwise downhill.
        let gx = (h[1][0] + h[1][1]) - (h[0][0] + h[0][1]);
        let gz = (h[0][1] + h[1][1]) - (h[0][0] + h[1][0]);
        let len = (gx * gx + gz * gz).sqrt();
        let (fx, fz) = match feeder {
            Some((dx, dz)) => (-dx as f32, -dz as f32),
            None if len > 0.01 => (-gx / len, -gz / len),
            None => (0.0, 0.0),
        };
        let falling = above_same || fluid_level(b) >= FALLING;
        let tint = [
            ((fx * 0.5 + 0.5) * 255.0) as u8,
            ((fz * 0.5 + 0.5) * 255.0) as u8,
            if falling { 255 } else { 0 },
        ];

        let fl = flags::FLUID | if lava { flags::EMISSIVE } else { flags::WATER };
        let layer = if lava { tex::LAVA } else { tex::WATER } as f32;
        let own = r.light(x, y, z);
        for (face, n) in FACE_N.iter().enumerate() {
            let nb = r.get(x + n[0], y + n[1], z + n[2]);
            let visible = if face == 2 {
                !above_same
            } else {
                !same(nb) && !is_opaque(nb)
            };
            if !visible {
                continue;
            }
            let l = r.light(x + n[0], y + n[1], z + n[2]);
            let light = [
                255,
                (l.0.max(own.0) * 17) as u8,
                (l.1.max(own.1) * 17) as u8,
                face as u8,
            ];
            let base = self.verts.len() as u32;
            for &(su, sv) in &CORNERS {
                let p = corner_pos(face, su, sv);
                let (cx, cz) = (p[0] as usize, p[2] as usize);
                let top = p[1] > 0.5;
                let (py, mut old_y) = if top {
                    (h[cx][cz], h_old[cx][cz])
                } else {
                    (0.0, 0.0)
                };
                let mut vlayer = layer;
                if is_new && above_same && !top {
                    // Falling: the column extends downward from the top.
                    old_y = 1.0;
                } else if let (true, Some((dx, dz))) = (is_new, feeder) {
                    // Corners on the far side start on the feeder's edge (the shader slides them
                    // out along the flow; the +0.25 layer offset marks them) at that edge's height.
                    let far = (dx != 0 && cx as i32 != (dx + 1) / 2)
                        || (dz != 0 && cz as i32 != (dz + 1) / 2);
                    if far {
                        vlayer += 0.25;
                        if top {
                            let (mx, mz) = if dx != 0 { (1 - cx, cz) } else { (cx, 1 - cz) };
                            old_y = h_old[mx][mz];
                        }
                    }
                }
                let (wx, wy, wz) = (
                    (x + self.ox) as f32 + p[0],
                    y as f32 + py,
                    (z + self.oz) as f32 + p[2],
                );
                let uv = [y as f32 + old_y, start];
                self.push(Vertex {
                    pos: [wx, wy, wz],
                    uv,
                    layer: vlayer,
                    light,
                    tint: [tint[0], tint[1], tint[2], fl],
                });
            }
            let list = if lava {
                &mut self.opaque
            } else {
                &mut self.water
            };
            list.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
}

/// How high a fluid of this level stands in its block (0..1).
pub fn fluid_height(level: u8) -> f32 {
    if level == 0 || level >= FALLING {
        0.875
    } else {
        (8 - level) as f32 / 9.0
    }
}

/// Surface height (0..1) at the four top corners, indexed [x][z].
fn surface_heights(
    get: &impl Fn(i32, i32, i32) -> u8,
    x: i32,
    y: i32,
    z: i32,
    lava: bool,
) -> [[f32; 2]; 2] {
    let same = |q: u8| if lava { is_lava(q) } else { is_water(q) };
    if same(get(x, y + 1, z)) {
        return [[1.0; 2]; 2];
    }
    std::array::from_fn(|cx| {
        std::array::from_fn(|cz| corner_height(get, x, y, z, cx as i32, cz as i32, lava))
    })
}

fn corner_height(
    get: &impl Fn(i32, i32, i32) -> u8,
    x: i32,
    y: i32,
    z: i32,
    cx: i32,
    cz: i32,
    lava: bool,
) -> f32 {
    let same = |q: u8| if lava { is_lava(q) } else { is_water(q) };
    let (mut sum, mut w) = (0.0f32, 0.0f32);
    for dx in [cx - 1, cx] {
        for dz in [cz - 1, cz] {
            let (bx, bz) = (x + dx, z + dz);
            if same(get(bx, y + 1, bz)) {
                return 1.0;
            }
            let b = get(bx, y, bz);
            if same(b) {
                let l = fluid_level(b);
                let wt = if l == 0 { 10.0 } else { 1.0 };
                sum += fluid_height(l) * wt;
                w += wt;
            } else if !is_solid(b) {
                w += 1.0;
            }
        }
    }
    if w > 0.0 {
        sum / w
    } else {
        0.0
    }
}
