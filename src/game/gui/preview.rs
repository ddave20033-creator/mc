//! The player standing in the inventory screen, turned toward the mouse, in their armor.

use super::*;


impl Game {
    /// Player figure for the inventory screen, inside the dark box at (x, y, w, h).
    /// Like Minecraft, the body turns a little toward the mouse and the head follows it.
    pub(super) fn player_preview(&mut self, x: f32, y: f32, w: f32, h: f32) {
        use crate::model::player::{ARM, BODY, HEAD, LEG};
        use crate::world::textures::tex;
        use glam::Mat3;
        let s = self.ui.s;
        let th = self.theme();
        self.ui
            .gradient(x, y, w, h, th.preview_top, th.preview_bottom);
        let u = s * 1.8;
        let (cx, feet) = (x + w * 0.5, y + h - 5.0 * s);
        let m = self.ui.mouse;
        let f = ((m.x - cx) / s / 40.0).atan();
        let g = ((m.y - (feet - 28.0 * u)) / s / 40.0).atan();
        let body = Mat3::from_rotation_y((f * 20.0).to_radians());
        let head = Mat3::from_rotation_y((f * 40.0).to_radians())
            * Mat3::from_rotation_x((g * 20.0).to_radians());
        let neck = Vec3::new(0.0, 24.0, 0.0);
        let plain = [255u8; 3];
        let v = Vec3::new;
        // What is worn (as on the player model), in the colors of its material.
        let (worn, vest) = unpack_armor(armor_code(&self.inventory.armor));
        let look = |m: usize| match m {
            0 => ([tex::ARMOR_WOOL; 6], [196, 184, 160]),
            1 => ([tex::ARMOR_METAL; 6], [226, 146, 96]),
            2 => ([tex::ARMOR_METAL; 6], [176, 184, 198]),
            _ => ([tex::ARMOR_METAL; 6], [120, 228, 232]),
        };
        type Part = (Vec3, Vec3, [u32; 6], Mat3, Vec3, [u8; 3]);
        let on = |m: Option<usize>, boxes: &[(Vec3, Vec3)], rot: Mat3, pivot: Vec3| -> Vec<Part> {
            m.map(|m| {
                let (t, c) = look(m);
                boxes.iter().map(|&(lo, hi)| (lo, hi, t, rot, pivot, c)).collect()
            })
            .unwrap_or_default()
        };
        // Each part of the body (min, max, textures, rotation, pivot, tint; model pixels, feet
        // at y = 0, facing +z) with what is worn over it, drawn right after it.
        let leg = |x0: f32, x1: f32| -> (Part, Vec<Part>) {
            let mut over = on(worn[2], &[(v(x0 - 0.5, 3.5, -2.5), v(x1 + 0.5, 12.3, 2.5))], body, Vec3::ZERO);
            over.extend(on(worn[3], &[(v(x0 - 0.6, -0.4, -2.6), v(x1 + 0.6, 3.7, 2.6))], body, Vec3::ZERO));
            ((v(x0, 0.0, -2.0), v(x1, 12.0, 2.0), LEG, body, Vec3::ZERO, plain), over)
        };
        let arm = |x0: f32, x1: f32| -> (Part, Vec<Part>) {
            let over = on(worn[1], &[(v(x0 - 0.6, 20.0, -2.6), v(x1 + 0.6, 24.6, 2.6))], body, Vec3::ZERO);
            ((v(x0, 12.0, -2.0), v(x1, 24.0, 2.0), ARM, body, Vec3::ZERO, plain), over)
        };
        let torso = {
            let mut over = on(worn[2], &[(v(-4.5, 11.6, -2.5), v(4.5, 14.2, 2.5))], body, Vec3::ZERO);
            over.extend(on(worn[1], &[(v(-4.6, 13.2, -2.6), v(4.6, 24.5, 2.6))], body, Vec3::ZERO));
            if vest {
                over.push((v(-4.9, 13.8, -3.0), v(4.9, 24.6, 3.0), [tex::VEST; 6], body, Vec3::ZERO, [118, 124, 92]));
                for (x0, x1) in [(-3.8, -1.5), (-1.1, 1.1), (1.5, 3.8)] {
                    over.push((v(x0, 14.4, 3.0), v(x1, 17.4, 3.7), [tex::VEST; 6], body, Vec3::ZERO, [94, 100, 74]));
                }
            }
            ((v(-4.0, 12.0, -2.0), v(4.0, 24.0, 2.0), BODY, body, Vec3::ZERO, plain), over)
        };
        let mut groups = vec![leg(-4.0, 0.0), leg(0.0, 4.0), torso, arm(-8.0, -4.0), arm(4.0, 8.0)];
        // Painter's order: farthest first, head always last (it sits on top of everything).
        groups.sort_by(|a, b| {
            let za = (a.0 .3 * ((a.0 .0 + a.0 .1) * 0.5)).z;
            let zb = (b.0 .3 * ((b.0 .0 + b.0 .1) * 0.5)).z;
            za.total_cmp(&zb)
        });
        // The helmet over the head: the cap, the guard at the back and the cheek guards
        // (drawn after the head, without their faces turned in toward it).
        let helmet = on(
            worn[0],
            &[
                (v(-4.6, 24.5, -4.6), v(4.6, 28.6, -1.2)),
                (v(-4.6, 28.6, -4.6), v(4.6, 32.7, 4.6)),
                (v(-4.6, 25.5, -1.2), v(-3.6, 28.6, 4.6)),
                (v(3.6, 25.5, -1.2), v(4.6, 28.6, 4.6)),
            ],
            head,
            neck,
        );
        let mut parts: Vec<(Part, Option<Vec3>)> = Vec::new();
        for (base, over) in groups {
            parts.push((base, None));
            parts.extend(over.into_iter().map(|p| (p, None)));
        }
        parts.push(((v(-4.0, 24.0, -4.0), v(4.0, 32.0, 4.0), HEAD, head, neck, plain), None));
        let head_center = v(0.0, 28.0, 0.0);
        parts.extend(helmet.into_iter().map(|p| (p, Some(head_center))));
        let light = Vec3::new(0.35, 0.55, 0.76).normalize();
        for ((lo, hi, tex, rot, pivot, tint), inner) in parts {
            let (x0, y0, z0, x1, y1, z1) = (lo.x, lo.y, lo.z, hi.x, hi.y, hi.z);
            let v = Vec3::new;
            // Corners TL, TR, BR, BL as seen from outside; texture index as in crate::model
            // (+X, -X, +Y, -Y, back, front), with the front facing the viewer (+z).
            let faces = [
                (
                    v(0.0, 0.0, 1.0),
                    5,
                    [v(x0, y1, z1), v(x1, y1, z1), v(x1, y0, z1), v(x0, y0, z1)],
                ),
                (
                    v(0.0, 0.0, -1.0),
                    4,
                    [v(x1, y1, z0), v(x0, y1, z0), v(x0, y0, z0), v(x1, y0, z0)],
                ),
                (
                    v(1.0, 0.0, 0.0),
                    0,
                    [v(x1, y1, z1), v(x1, y1, z0), v(x1, y0, z0), v(x1, y0, z1)],
                ),
                (
                    v(-1.0, 0.0, 0.0),
                    1,
                    [v(x0, y1, z0), v(x0, y1, z1), v(x0, y0, z1), v(x0, y0, z0)],
                ),
                (
                    v(0.0, 1.0, 0.0),
                    2,
                    [v(x0, y1, z0), v(x1, y1, z0), v(x1, y1, z1), v(x0, y1, z1)],
                ),
                (
                    v(0.0, -1.0, 0.0),
                    3,
                    [v(x0, y0, z1), v(x1, y0, z1), v(x1, y0, z0), v(x0, y0, z0)],
                ),
            ];
            for (n, ti, corners) in faces {
                // A face turned in toward what it covers is hidden inside it.
                if let Some(c) = inner {
                    let middle = (corners[0] + corners[2]) * 0.5;
                    if n.dot(c - middle) > 0.0 {
                        continue;
                    }
                }
                let n = rot * n;
                if n.z <= 0.01 {
                    continue;
                }
                let shade = 0.55 + 0.45 * n.dot(light).max(0.0);
                let p = corners.map(|c| {
                    let q = rot * (c - pivot) + pivot;
                    Vec2::new(cx + q.x * u, feet - q.y * u)
                });
                self.ui.tex_quad_tint(p, tex[ti], shade, tint);
            }
        }
    }
}
