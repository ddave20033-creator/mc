//! The pixel font: the 8x8 bitmap glyphs (ASCII, Latin-1 and the Hungarian letters) packed
//! into the font texture, their sizes, measuring and wrapping text; and the smooth font
//! drawn from a system TrueType font where there is one.

use super::*;

#[derive(Clone, Copy, Default)]
pub struct Glyph {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    /// Visible width in font pixels.
    pub w: f32,
    /// Advance in font pixels (width + 1 spacing).
    pub adv: f32,
    pub bits: [u8; 8],
    pub minx: u32,
    /// Accent row drawn one pixel above the cell (capital letters with accents).
    pub top: u8,
    /// Its quad in font units from the pen and the top of the line (x, y, width, height).
    pub quad: [f32; 4],
}

/// ASCII + Latin-1 + Hungarian o/u with double acute.
fn font_chars() -> Vec<(char, [u8; 8])> {
    let mut v = Vec::new();
    for c in (32u32..128).chain(160..256) {
        let ch = char::from_u32(c).unwrap();
        v.push((
            ch,
            BASIC_FONTS
                .get(ch)
                .or_else(|| LATIN_FONTS.get(ch))
                .unwrap_or([0; 8]),
        ));
    }
    v.push(('\u{151}', [0x6C, 0x36, 0x00, 0x1E, 0x33, 0x33, 0x1E, 0x00]));
    v.push(('\u{171}', [0x6C, 0x36, 0x00, 0x33, 0x33, 0x33, 0x7E, 0x00]));
    // font8x8 squashes accented capitals to lowercase height; use the plain capital
    // and put the accent in an extra row above the cell instead (see accent_row).
    for (ch, base) in [
        ('\u{c1}', 'A'),
        ('\u{c9}', 'E'),
        ('\u{cd}', 'I'),
        ('\u{d3}', 'O'),
        ('\u{d6}', 'O'),
        ('\u{da}', 'U'),
        ('\u{dc}', 'U'),
        ('\u{150}', 'O'),
        ('\u{170}', 'U'),
    ] {
        let g = BASIC_FONTS.get(base).unwrap();
        v.retain(|e| e.0 != ch);
        v.push((ch, g));
    }
    v
}

/// Accent pixels for the extra row above accented capitals (bit 0 = leftmost pixel).
fn accent_row(ch: char) -> u8 {
    match ch {
        '\u{c1}' | '\u{c9}' | '\u{cd}' | '\u{d3}' | '\u{da}' => 0x18,
        '\u{d6}' | '\u{dc}' => 0x12,
        '\u{150}' | '\u{170}' => 0x36,
        _ => 0,
    }
}

/// 8x8 bitmap font baked into a 16-column atlas with 1px padding; glyphs are cropped to be proportional.
pub struct Font {
    pub glyphs: Vec<Glyph>,
    pub atlas: Vec<u8>,
    index: HashMap<char, usize>,
    /// The glyph of each character up to U+00FF (most text), without hashing it; the rest are
    /// looked up in `index`. Characters with none show as '?'.
    latin: [u16; 256],
}

/// `Font::latin` from the glyph index.
fn latin_table(index: &HashMap<char, usize>) -> [u16; 256] {
    let unknown = index[&'?'] as u16;
    std::array::from_fn(|c| index.get(&char::from(c as u8)).map_or(unknown, |&i| i as u16))
}

impl Font {
    pub fn new() -> Self {
        let mut atlas = vec![0u8; (FONT_TEX_W * FONT_TEX_H) as usize];
        let chars = font_chars();
        let mut glyphs = Vec::with_capacity(chars.len());
        let mut index = HashMap::new();
        for (i, &(ch, bits)) in chars.iter().enumerate() {
            let i = i as u32;
            index.insert(ch, i as usize);
            let (ox, oy) = ((i % 16) * 10 + 1, (i / 16) * 10 + 1);
            let (mut minx, mut maxx) = (8u32, 0u32);
            let top = accent_row(ch);
            for x in 0..8u32 {
                if top & (1 << x) != 0 {
                    atlas[((oy - 1) * FONT_TEX_W + ox + x) as usize] = 255;
                }
            }
            for (y, row) in bits.iter().enumerate() {
                for x in 0..8u32 {
                    if row & (1 << x) != 0 {
                        atlas[((oy + y as u32) * FONT_TEX_W + ox + x) as usize] = 255;
                        minx = minx.min(x);
                        maxx = maxx.max(x);
                    }
                }
            }
            if minx > maxx {
                glyphs.push(Glyph {
                    w: 0.0,
                    adv: 4.0,
                    bits,
                    ..Default::default()
                });
            } else {
                let w = (maxx - minx + 1) as f32;
                let lift = (top != 0) as u32 as f32;
                glyphs.push(Glyph {
                    u0: (ox + minx) as f32 / FONT_TEX_W as f32,
                    u1: (ox + maxx + 1) as f32 / FONT_TEX_W as f32,
                    v0: (oy - (top != 0) as u32) as f32 / FONT_TEX_H as f32,
                    v1: (oy + 8) as f32 / FONT_TEX_H as f32,
                    w,
                    adv: w + 1.0,
                    bits,
                    minx,
                    top,
                    quad: [0.0, -lift, w, 8.0 + lift],
                });
            }
        }
        let latin = latin_table(&index);
        Self {
            glyphs,
            atlas,
            index,
            latin,
        }
    }

    /// The smooth typeface (`SMOOTH_FONT`), anti-aliased: capitals about as tall as the pixel
    /// font's, its baseline where the pixel font's is. The pixel font where it cannot be read.
    pub fn smooth() -> Self {
        use ab_glyph::{point, Font as _, FontVec, PxScale, ScaleFont as _};
        let Some(font) = std::fs::read(SMOOTH_FONT).ok().and_then(|d| FontVec::try_from_vec(d).ok()) else {
            return Self::new();
        };
        // Its size: a capital H 6.4 units tall.
        let probe = PxScale::from(100.0);
        let cap = font
            .outline_glyph(font.glyph_id('H').with_scale_and_position(probe, point(0.0, 0.0)))
            .map_or(70.0, |g| g.px_bounds().height());
        let scale = PxScale::from(100.0 * 6.4 * FONT_RES / cap);
        let scaled = font.as_scaled(scale);
        let mut atlas = vec![0u8; (FONT_TEX_W * FONT_TEX_H) as usize];
        let mut glyphs = Vec::new();
        let mut index = HashMap::new();
        let chars = (32u32..127)
            .chain(160..256)
            .filter_map(char::from_u32)
            .chain(['\u{150}', '\u{151}', '\u{170}', '\u{171}', '\u{2013}', '\u{2014}', '\u{2022}', '\u{2026}', '\u{2190}', '\u{2192}']);
        const PAD: u32 = 2;
        let (mut cx, mut cy, mut row_h) = (1u32, 1u32, 0u32);
        for ch in chars {
            let id = font.glyph_id(ch);
            let adv = scaled.h_advance(id) / FONT_RES;
            let mut g = Glyph { adv, bits: [0; 8], ..Default::default() };
            // The baseline 7 units down the line.
            let placed = id.with_scale_and_position(scale, point(0.0, 7.0 * FONT_RES));
            if let Some(outline) = font.outline_glyph(placed) {
                let b = outline.px_bounds();
                let (bw, bh) = (b.width() as u32 + 2 * PAD, b.height() as u32 + 2 * PAD);
                if cx + bw >= FONT_TEX_W {
                    cx = 1;
                    cy += row_h + 1;
                    row_h = 0;
                }
                if cy + bh >= FONT_TEX_H {
                    break;
                }
                outline.draw(|x, y, c| {
                    let (ax, ay) = (cx + PAD + x, cy + PAD + y);
                    if ax < FONT_TEX_W && ay < FONT_TEX_H {
                        let i = (ay * FONT_TEX_W + ax) as usize;
                        atlas[i] = atlas[i].max((c.clamp(0.0, 1.0) * 255.0) as u8);
                    }
                });
                g.u0 = cx as f32 / FONT_TEX_W as f32;
                g.v0 = cy as f32 / FONT_TEX_H as f32;
                g.u1 = (cx + bw) as f32 / FONT_TEX_W as f32;
                g.v1 = (cy + bh) as f32 / FONT_TEX_H as f32;
                g.quad = [
                    (b.min.x - PAD as f32) / FONT_RES,
                    (b.min.y - PAD as f32) / FONT_RES,
                    bw as f32 / FONT_RES,
                    bh as f32 / FONT_RES,
                ];
                g.w = b.width() / FONT_RES;
                cx += bw + 1;
                row_h = row_h.max(bh);
            }
            index.insert(ch, glyphs.len());
            glyphs.push(g);
        }
        let latin = latin_table(&index);
        Self { glyphs, atlas, index, latin }
    }

    pub fn glyph(&self, c: char) -> &Glyph {
        let i = match self.latin.get(c as usize) {
            Some(&i) => i as usize,
            None => self.index.get(&c).copied().unwrap_or(self.latin[b'?' as usize] as usize),
        };
        &self.glyphs[i]
    }

    /// Width of `s` at scale `size` (the last glyph's gap not counted).
    pub fn text_width(&self, s: &str, size: f32) -> f32 {
        let units: f32 = s.chars().map(|c| self.glyph(c).adv).sum();
        if units > 0.0 {
            (units - 1.0) * size
        } else {
            0.0
        }
    }

    /// `text` split into lines no wider than `max_w` at `size`, at spaces (a word wider than
    /// a line gets one of its own). The width is added up as it goes, not measured again
    /// for every word.
    pub fn wrap(&self, text: &str, max_w: f32, size: f32) -> Vec<String> {
        let units = |from: f32, s: &str| s.chars().fold(from, |u, c| u + self.glyph(c).adv);
        let space = self.glyph(' ').adv;
        let mut rows = Vec::new();
        let mut cur = String::new();
        let mut cur_units = 0.0f32;
        for word in text.split_whitespace() {
            if cur.is_empty() {
                cur.push_str(word);
                cur_units = units(0.0, word);
                continue;
            }
            let with = units(cur_units + space, word);
            let width = if with > 0.0 { (with - 1.0) * size } else { 0.0 };
            if width > max_w {
                rows.push(std::mem::replace(&mut cur, word.to_string()));
                cur_units = units(0.0, word);
            } else {
                cur.push(' ');
                cur.push_str(word);
                cur_units = with;
            }
        }
        if !cur.is_empty() {
            rows.push(cur);
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wrapping as it was: every candidate line measured whole.
    fn wrap_measuring_each(font: &Font, text: &str, max_w: f32, size: f32) -> Vec<String> {
        let mut rows = Vec::new();
        let mut cur = String::new();
        for word in text.split_whitespace() {
            let candidate = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
            if !cur.is_empty() && font.text_width(&candidate, size) > max_w {
                rows.push(std::mem::replace(&mut cur, word.to_string()));
            } else {
                cur = candidate;
            }
        }
        if !cur.is_empty() {
            rows.push(cur);
        }
        rows
    }

    #[test]
    fn wrapping_adds_up_widths_the_same_as_measuring() {
        let font = Font::new();
        let text = "A gyors barna róka átugrik a lusta kutyán, és közben 1234 ÁRVÍZTŰRŐ tükörfúrógép \
                    hums quietly: the quick brown fox jumps over the lazy dog? Igen! verylongwordthatdoesnotfit x y z";
        for size in [1.0, 1.5, 2.0, 3.0] {
            for max_w in (4..400).step_by(7) {
                let max_w = max_w as f32;
                assert_eq!(font.wrap(text, max_w, size), wrap_measuring_each(&font, text, max_w, size), "{size} {max_w}");
            }
        }
        // Characters past U+00FF and unknown ones still find their glyph.
        assert!(!std::ptr::eq(font.glyph('ő'), font.glyph('?')));
        assert_eq!(font.glyph('\u{2603}').adv, font.glyph('?').adv);
    }
}
