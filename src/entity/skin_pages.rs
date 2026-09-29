//! Entity skins drawn in more detail than one texture layer holds (the wolf, the pig and the
//! sheep: Minecraft's 64 unit wide atlases at `px` texels per unit, 512 texels wide at 8).
//! Each box's faces are cut out of the atlas and packed onto 128x128 layers ("pages"), with
//! `border` texels round each repeating its edge (the wolf's) or packed tight (8 unit faces
//! then fill a page's half exactly, which keeps the mip levels apart too); a face too big for
//! a page is cut into pieces (whole units), each drawn as its own quad. The same for any
//! resolution a pack draws the atlas at: the pages always hold `px` texels per unit.

use std::sync::OnceLock;

/// A texture layer's size.
pub const PAGE: u32 = 128;

/// One of a model's boxes: its texture offset and size, in units (Minecraft's box UV).
pub type BoxUv = ([f32; 2], [f32; 3]);

/// Part of a box's face on a page: its rectangle in the atlas (units: left, top, right,
/// bottom), the page, and the page texel of its top left corner (any border round it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub rect: (f32, f32, f32, f32),
    pub page: u32,
    pub x: u32,
    pub y: u32,
}

impl Piece {
    /// Its size on the page in texels (without the border).
    pub fn size(&self, px: u32) -> (u32, u32) {
        let (l, t, r, b) = self.rect;
        (((r - l) * px as f32).round() as u32, ((b - t) * px as f32).round() as u32)
    }
}

pub struct SkinPages {
    pub boxes: &'static [BoxUv],
    /// Texels per unit on the pages.
    pub px: u32,
    /// Pages reserved for it (texture layers from its first one).
    pub pages: u32,
    /// Texels round each piece repeating its edge.
    pub border: u32,
    layout: OnceLock<Vec<[Vec<Piece>; 6]>>,
}

/// Face `f` of a box's texture (in `emit_faces`' order: bottom, top, the -X side, -Z, +X,
/// +Z): its corners' (u, v) as the face runs, in units (Minecraft's box UV).
pub fn face_uv(uv: [f32; 2], s: [f32; 3], f: usize) -> (f32, f32, f32, f32) {
    let (u, w) = (uv[0], uv[1]);
    let (dx, dy, dz) = (s[0], s[1], s[2]);
    let (u0, u1, u2, u3, u4, u5) = (u, u + dz, u + dz + dx, u + dz + dx + dx, u + dz + dx + dz, u + dz + dx + dz + dx);
    let (w0, w1, w2) = (w, w + dz, w + dz + dy);
    [(u1, w0, u2, w1), (u2, w1, u3, w0), (u0, w1, u1, w2), (u1, w1, u2, w2), (u2, w1, u4, w2), (u4, w1, u5, w2)][f]
}

impl SkinPages {
    pub const fn new(boxes: &'static [BoxUv], px: u32, pages: u32, border: u32) -> Self {
        SkinPages { boxes, px, pages, border, layout: OnceLock::new() }
    }

    /// Face `f` of box `b` as a rectangle in the atlas (left, top, right, bottom; units).
    pub fn rect(&self, b: usize, f: usize) -> (f32, f32, f32, f32) {
        let (a, c, e, g) = face_uv(self.boxes[b].0, self.boxes[b].1, f);
        (a.min(e), c.min(g), a.max(e), c.max(g))
    }

    /// The pieces face `f` of box `b` is on (one unless it is too big for a page).
    pub fn pieces(&self, b: usize, f: usize) -> &[Piece] {
        &self.layout()[b][f]
    }

    /// Every piece of every face.
    pub fn all(&self) -> impl Iterator<Item = &Piece> {
        self.layout().iter().flat_map(|faces| faces.iter().flatten())
    }

    /// The page texture coordinates (0..1) of atlas point (u, v) (units) on a piece.
    pub fn page_uv(&self, p: &Piece, u: f32, v: f32) -> [f32; 2] {
        let k = self.px as f32;
        [
            (p.x as f32 + (u - p.rect.0) * k) / PAGE as f32,
            (p.y as f32 + (v - p.rect.1) * k) / PAGE as f32,
        ]
    }

    /// The atlas point (units) shown at texel (x, y) of a page (a border repeats the
    /// piece's edge), or None where nothing is.
    pub fn atlas_at(&self, page: u32, x: u32, y: u32) -> Option<(f32, f32)> {
        let k = self.px as f32;
        let e = self.border;
        self.all().find_map(|p| {
            let (w, h) = p.size(self.px);
            let inside = p.page == page && x + e >= p.x && x < p.x + w + e && y + e >= p.y && y < p.y + h + e;
            inside.then(|| {
                let fx = (x as f32 + 0.5 - p.x as f32).clamp(0.5, w as f32 - 0.5);
                let fy = (y as f32 + 0.5 - p.y as f32).clamp(0.5, h as f32 - 0.5);
                (p.rect.0 + fx / k, p.rect.1 + fy / k)
            })
        })
    }

    fn layout(&self) -> &Vec<[Vec<Piece>; 6]> {
        self.layout.get_or_init(|| {
            let px = self.px as f32;
            let e = self.border;
            let max = (PAGE - 2 * e) as f32;
            // Cut each face into pieces a page holds (in whole units where it can).
            let mut todo: Vec<(usize, usize, Piece)> = Vec::new();
            for b in 0..self.boxes.len() {
                for f in 0..6 {
                    let (l, t, r, bt) = self.rect(b, f);
                    let split = |a: f32, z: f32| -> Vec<(f32, f32)> {
                        let n = ((z - a) * px / max).ceil().max(1.0);
                        let mut step = ((z - a) / n).ceil();
                        if step * px > max {
                            step = (z - a) / n;
                        }
                        let mut out = Vec::new();
                        let mut s = a;
                        while s < z - 1e-4 {
                            let e = (s + step).min(z);
                            out.push((s, e));
                            s = e;
                        }
                        if out.is_empty() {
                            out.push((a, z));
                        }
                        out
                    };
                    for &(y0, y1) in &split(t, bt) {
                        for &(x0, x1) in &split(l, r) {
                            let p = Piece { rect: (x0, y0, x1, y1), page: 0, x: 0, y: 0 };
                            todo.push((b, f, p));
                        }
                    }
                }
            }
            let dims = |p: &Piece| {
                let (w, h) = p.size(self.px);
                (w + 2 * e, h + 2 * e)
            };
            todo.sort_by_key(|(b, f, p)| {
                let (w, h) = dims(p);
                (std::cmp::Reverse(h), std::cmp::Reverse(w), *b, *f)
            });
            let mut out: Vec<[Vec<Piece>; 6]> = (0..self.boxes.len()).map(|_| Default::default()).collect();
            // Shelves on each page: (top, height, filled to).
            let mut pages: Vec<Vec<(u32, u32, u32)>> = Vec::new();
            for (b, f, mut p) in todo {
                let (w, h) = dims(&p);
                let mut spot = None;
                'find: for (pi, shelves) in pages.iter_mut().enumerate() {
                    for s in shelves.iter_mut() {
                        if s.1 >= h && s.2 + w <= PAGE {
                            spot = Some((pi as u32, s.2, s.0));
                            s.2 += w;
                            break 'find;
                        }
                    }
                    let top: u32 = shelves.iter().map(|s| s.1).sum();
                    if top + h <= PAGE {
                        shelves.push((top, h, w));
                        spot = Some((pi as u32, 0, top));
                        break;
                    }
                }
                let (page, x, y) = spot.unwrap_or_else(|| {
                    pages.push(vec![(0, h, w)]);
                    ((pages.len() - 1) as u32, 0, 0)
                });
                (p.page, p.x, p.y) = (page, x + e, y + e);
                out[b][f].push(p);
            }
            for faces in &mut out {
                for pieces in faces.iter_mut() {
                    pieces.sort_by(|a, b| (a.rect.1, a.rect.0).partial_cmp(&(b.rect.1, b.rect.0)).unwrap());
                }
            }
            out
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every skin's faces fit its pages without overlapping, borders included.
    #[test]
    fn skins_fit_their_pages_without_overlapping() {
        use crate::entity::mob::{pig_skin, sheep_skin, wolf_skin};
        for skin in [&wolf_skin::SKIN, &pig_skin::SKIN, &sheep_skin::SKIN, &sheep_skin::WOOL] {
            let used = skin.all().map(|p| p.page + 1).max().unwrap_or(0);
            assert!(used <= skin.pages, "needs {used} pages, has {}", skin.pages);
            let mut placed: Vec<(u32, u32, u32, u32, u32)> = Vec::new();
            for b in 0..skin.boxes.len() {
                for f in 0..6 {
                    // The pieces cover the face exactly.
                    let (l, t, r, bt) = skin.rect(b, f);
                    let area: f32 = skin.pieces(b, f).iter().map(|p| (p.rect.2 - p.rect.0) * (p.rect.3 - p.rect.1)).sum();
                    assert!((area - (r - l) * (bt - t)).abs() < 1e-3, "box {b} face {f}");
                    for p in skin.pieces(b, f) {
                        let (w, h) = p.size(skin.px);
                        let e = skin.border;
                        assert!(p.x + w + e <= PAGE && p.y + h + e <= PAGE, "box {b} face {f}");
                        let me = (p.page, p.x - e, p.y - e, p.x + w + e, p.y + h + e);
                        for o in &placed {
                            let apart = o.0 != me.0 || o.3 <= me.1 || me.3 <= o.1 || o.4 <= me.2 || me.4 <= o.2;
                            assert!(apart, "box {b} face {f} overlaps");
                        }
                        placed.push(me);
                    }
                }
            }
        }
    }

    #[test]
    fn a_page_texel_maps_back_to_its_atlas_point() {
        let skin = &crate::entity::mob::pig_skin::SKIN;
        let p = skin.pieces(2, 2)[0];
        let (u, v) = skin.atlas_at(p.page, p.x + 4, p.y + 12).unwrap();
        assert!((u - (p.rect.0 + 4.5 / 8.0)).abs() < 1e-4 && (v - (p.rect.1 + 12.5 / 8.0)).abs() < 1e-4);
        let [s, t] = skin.page_uv(&p, u, v);
        assert!((s * 128.0 - (p.x as f32 + 4.5)).abs() < 1e-3 && (t * 128.0 - (p.y as f32 + 12.5)).abs() < 1e-3);
    }
}
