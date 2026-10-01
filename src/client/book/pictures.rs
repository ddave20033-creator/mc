//! The pictures on the guide book's pages, with their small looping animations: the
//! furnaces, a furnace's front and top, the guns, the pickaxes over the ores they mine, the
//! gun station and rows of items. Their words come with them (`pages`).

use super::draw::{Draw, phase};
use crate::item::*;
use crate::ui::{rgba, with_alpha};
use crate::textures::tex;
use crate::world::*;
use glam::Vec2;
use std::f32::consts::PI;

/// Pictures drawn on the pages.
#[derive(Clone)]
pub(super) enum Pic {
    /// A furnace's blocks as they stand (FURNACE, BLAST_FURNACE or ADV_FURNACE).
    Furnace(Block),
    /// A furnace's front: the upper half takes what to smelt, the lower half the fuel (their
    /// labels).
    Front([String; 2]),
    /// The top of a furnace with meat on its corners (labelled raw, half, cooked, burnt).
    Grill([String; 4]),
    /// A gun, big.
    Gun(GunKind),
    /// The pickaxes from the first to the last, each over the hardest ore it mines.
    Tiers,
    /// The gun station.
    Station,
    /// A row of items.
    Items(Vec<ItemId>),
}

impl Pic {
    /// How tall it is (book units).
    pub(super) fn height(&self) -> f32 {
        match self {
            Pic::Furnace(FURNACE) => 36.0,
            Pic::Furnace(BLAST_FURNACE) => 52.0,
            Pic::Furnace(_) => 58.0,
            Pic::Front(_) => 56.0,
            Pic::Grill(_) => 62.0,
            Pic::Gun(_) => 66.0,
            Pic::Tiers => 44.0,
            Pic::Station => 50.0,
            Pic::Items(_) => 24.0,
        }
    }
}

impl Draw<'_, '_> {
    pub(super) fn picture(&mut self, pic: &Pic, x: f32, y: f32) {
        let (m, u) = (self.m, self.m.u);
        let th = self.look.theme;
        let t = self.look.time;
        let cx = x + m.w * 0.5;
        let hgt = pic.height() * u;
        let v = Vec2::new;
        match pic {
            Pic::Furnace(base) => {
                // Iso cubes (as the item icons draw them), the furnace facing the reader's
                // left: (right, up) steps along its front and upward.
                let cells: Vec<(f32, f32, Block)> = match *base {
                    BLAST_FURNACE => vec![(0.0, 0.0, furnace_id(*base, 0, true)), (0.0, 1.0, CHIMNEY)],
                    ADV_FURNACE => vec![
                        (0.0, 0.0, furnace_id(*base, 0, true)),
                        (1.0, 0.0, adv_part_id(1, 0, true)),
                        (0.0, 1.0, adv_part_id(2, 0, true)),
                        (1.0, 1.0, adv_part_id(3, 0, true)),
                    ],
                    _ => vec![(0.0, 0.0, furnace_id(*base, 0, true))],
                };
                let wide = cells.iter().map(|c| c.0).fold(0.0f32, f32::max);
                let tall = cells.iter().map(|c| c.1).fold(0.0f32, f32::max);
                let r = 14.0 * u;
                let k = 0.866 * r;
                let top = -r - r * tall - r * 0.5 * wide;
                let c0 = v(cx - k * wide * 0.5, y + hgt * 0.5 - (top + r) * 0.5 + 2.0 * u);
                for &(ox, oy, b) in &cells {
                    let c = c0 + v(k * ox, r * 0.5 * ox - r * oy);
                    self.cv.cube(c, r, face_texture(b, 2), face_texture(b, 5), face_texture(b, 0), [255; 3], [255; 3]);
                }
                // The fire flickers in its mouth.
                let glow = 0.18 + 0.12 * (t * 9.0).sin() * (t * 5.3).cos();
                let fc = c0 + v(-k * 0.5, r * 0.5);
                self.cv.poly(
                    &[fc + v(-k * 0.3, -r * 0.1), fc + v(k * 0.3, r * 0.2), fc + v(k * 0.3, r * 0.55), fc + v(-k * 0.3, r * 0.25)],
                    rgba(255, 150, 40, (255.0 * glow.max(0.0)) as u8),
                );
                // Smoke out of the chimney or the hood's vents.
                let tops: Vec<Vec2> = match *base {
                    BLAST_FURNACE => vec![c0 + v(0.0, -r * 2.0)],
                    ADV_FURNACE => vec![c0 + v(k * 0.3, -r * 2.2), c0 + v(k * 0.9, -r * 1.9)],
                    _ => Vec::new(),
                };
                for (i, p) in tops.into_iter().enumerate() {
                    for j in 0..3 {
                        let k = phase(t + i as f32 * 0.4 + j as f32 / 3.0 * 1.8, 1.8);
                        self.smoke(p.x, p.y, k, 110, 16.0);
                    }
                }
            }
            Pic::Front(labels) => {
                let s = 44.0 * u;
                let (fx, fy) = (x + 6.0 * u, y + (hgt - s) * 0.5);
                let b = furnace_id(FURNACE, 0, true);
                self.cv.tex_quad([v(fx, fy), v(fx + s, fy), v(fx + s, fy + s), v(fx, fy + s)], face_texture(b, 5), 1.0, [255; 3], 1.0);
                // Flames dance in the firebox.
                for i in 0..6 {
                    let k = phase(t + i as f32 * 0.17, 0.7);
                    let fxx = fx + s * (0.3 + 0.07 * i as f32) + (t * 6.0 + i as f32).sin() * u;
                    let fyy = fy + s * 0.82 - k * s * 0.18;
                    let c = if k < 0.4 { rgba(255, 230, 120, 255) } else { rgba(255, 130, 40, 255) };
                    self.cv.fill(fxx, fyy, 2.0 * u, 2.0 * u, with_alpha(c, 1.0 - k));
                }
                // The line between the halves, dashed.
                let mut dx = fx - 2.0 * u;
                while dx < fx + s + 2.0 * u {
                    self.cv.fill(dx, fy + s * 0.5 - 0.5 * u, 3.0 * u, 1.0, th.gold);
                    dx += 5.0 * u;
                }
                let lx = fx + s + 16.0 * u;
                for ((half, item), label) in [(0.25, IRON_ORE as ItemId), (0.75, COAL)].into_iter().zip(labels) {
                    let ly = fy + s * half;
                    self.cv.fill(fx + s + u, ly - 0.5 * u, 12.0 * u, 1.0, th.soft);
                    self.icon(item, 1, lx, ly - 7.0 * u, 14.0 * u);
                    self.cv.text(label, lx + 17.0 * u, ly - 4.0, m.fs, th.ink, false);
                }
            }
            Pic::Grill(labels) => {
                let s = 52.0 * u;
                let (gx, gy) = (cx - s * 0.5, y + (hgt - s) * 0.5);
                self.cv.tex_quad([v(gx, gy), v(gx + s, gy), v(gx + s, gy + s), v(gx, gy + s)], face_texture(FURNACE, 2), 1.0, [255; 3], 1.0);
                let meats = meat(PORKCHOP).unwrap_or_default();
                // One piece after the other is turned over (it narrows, and the other side
                // shows).
                let turning = (t / 1.5) as usize % 4;
                let k = phase(t, 1.5);
                for (i, item) in [meats[0], meats[1], meats[2], meats[4]].into_iter().enumerate() {
                    let (qx, qy) = (gx + (i % 2) as f32 * s * 0.5, gy + (i / 2) as f32 * s * 0.5);
                    let size = s * 0.5 - 8.0 * u;
                    let (mut w, mut lift) = (size, 0.0);
                    if i == turning && k < 0.4 {
                        w = size * ((k / 0.4) * PI).cos().abs();
                        lift = ((k / 0.4) * PI).sin() * 5.0 * u;
                    }
                    let layer = match icon(item) {
                        Icon::Flat(l) => l,
                        Icon::Block(_) => tex::PORKCHOP,
                    };
                    let (ix, iy) = (qx + 4.0 * u + (size - w) * 0.5, qy + 4.0 * u - lift);
                    self.cv.tex_quad([v(ix, iy), v(ix + w, iy), v(ix + w, iy + size), v(ix, iy + size)], layer, 1.0, [255; 3], 1.0);
                    // Smoke, darker the more done.
                    let gray = [230, 200, 150, 60][i];
                    for j in 0..2 {
                        let kk = phase(t * 0.7 + i as f32 * 0.37 + j as f32 * 0.5, 1.0);
                        self.smoke(qx + s * 0.25, qy + 4.0 * u, kk, gray, 14.0);
                    }
                }
                // The labels beside the corners: on the left right-aligned, on the right after.
                let at = [
                    (gx - 4.0 * u, gy + 4.0 * u, true),
                    (gx + s + 4.0 * u, gy + 4.0 * u, false),
                    (gx - 4.0 * u, gy + s * 0.5 + 4.0 * u, true),
                    (gx + s + 4.0 * u, gy + s * 0.5 + 4.0 * u, false),
                ];
                for (l, (lx, ly, left)) in labels.iter().zip(at) {
                    let tw = self.cv.font().text_width(l, m.fs);
                    let px = if left { lx - tw } else { lx };
                    self.cv.text(l, px, ly, m.fs, th.soft, false);
                }
            }
            Pic::Gun(kind) => {
                if let Icon::Flat(layer) = icon(kind.item()) {
                    // A shot now and then: the gun kicks back a little and settles.
                    let period = 1.8;
                    let k = phase(t, period);
                    let kick = (1.0 - k / 0.25).max(0.0);
                    let r = hgt * 0.5;
                    let c = v(cx - kick * kick * 3.0 * u, y + r - kick * kick * 1.5 * u);
                    self.cv.sprite(c, r, layer, [255; 3], 1.0);
                }
            }
            Pic::Tiers => {
                let n = TIER_ORDER.len() as f32;
                let col = m.w / n;
                // A gold frame goes along the tools.
                let lit = (t / 0.9) as usize % TIER_ORDER.len();
                for (i, tier) in TIER_ORDER.into_iter().enumerate() {
                    let px = x + col * (i as f32 + 0.5);
                    if i == lit {
                        self.cv.fill(px - 11.0 * u, y, 22.0 * u, 44.0 * u, th.gold);
                        self.cv.fill(px - 10.0 * u, y + u, 20.0 * u, 42.0 * u, th.paper);
                    }
                    let lift = if i == lit { 2.0 * u } else { 0.0 };
                    self.icon(tool_id(ToolKind::Pickaxe, tier), 1, px - 8.0 * u, y + 2.0 * u - lift, 16.0 * u);
                    self.cv.fill(px - 0.5 * u, y + 20.0 * u, 1.0, 4.0 * u, th.soft);
                    // (the hardest of what it mines)
                    if let Some(&ore) = hardest_mined(tier.level()).last() {
                        self.icon(ore as ItemId, 1, px - 8.0 * u, y + 25.0 * u, 16.0 * u);
                    }
                }
            }
            Pic::Station => {
                self.icon(GUN_STATION as ItemId, 1, cx - 38.0 * u, y + 6.0 * u, 42.0 * u);
                if let Icon::Flat(layer) = icon(PISTOL) {
                    // The pistol floats over the table as it is put together there.
                    let bob = (t * 2.0).sin() * 3.0 * u;
                    self.cv.sprite(v(cx + 22.0 * u, y + 22.0 * u + bob), 18.0 * u, layer, [255; 3], 1.0);
                    for i in 0..4 {
                        let k = phase(t + i as f32 * 0.25, 1.0);
                        let a = i as f32 * 1.7 + t;
                        let (sx, sy) = (cx + 22.0 * u + a.cos() * 16.0 * u, y + 22.0 * u + a.sin() * 10.0 * u);
                        self.cv.fill(sx, sy, u * 1.5, u * 1.5, with_alpha(th.gold, 1.0 - k));
                    }
                }
            }
            Pic::Items(items) => {
                let step = 24.0 * u;
                let x0 = cx - step * items.len() as f32 * 0.5;
                for (i, &id) in items.iter().enumerate() {
                    let bob = ((t * 2.5 + i as f32 * 0.8).sin() * 1.5 * u).round();
                    let sx = x0 + i as f32 * step + 3.0 * u;
                    self.slot(sx - u, y + 2.0 * u, 18.0 * u);
                    self.icon(id, 1, sx, y + 3.0 * u + bob, 16.0 * u);
                }
            }
        }
    }
}
