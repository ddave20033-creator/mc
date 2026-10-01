//! The guide book laid out on its pages: the page's size and margins, what the book says
//! (`pages`) cut into lines and pieces, and the pieces put on pages (a heading stays with
//! what follows it, a chapter starts a page).

use super::pages::{El, FrontMatter, content, front_matter};
use super::pictures::Pic;
use crate::item::{ItemId, recipe_view};
use crate::ui::Font;

/// A page's size and margins in book units.
const PAGE_W: f32 = 168.0;
const PAGE_H: f32 = 216.0;
pub(super) const MARGIN_X: f32 = 13.0;
pub(super) const MARGIN_TOP: f32 = 13.0;
const MARGIN_BOTTOM: f32 = 21.0;
/// Pixels per book unit on a page's texture, and the texture's size: 2 x 3 block texture
/// layers (the page fills 255 x 328 of it).
pub(super) const U: f32 = 1.52;
pub(super) const SHEET_W: usize = 256;
pub(super) const SHEET_H: usize = 384;
/// The part of the texture the page fills.
pub(super) const PAGE_PX: (f32, f32) = (PAGE_W * U, PAGE_H * U);
/// Space for a bullet's dash (book units).
pub(super) const INDENT: f32 = 8.0;
/// Space for the item before a row's words (book units).
pub(super) const ROW_TEXT: f32 = 22.0;

/// Sizes on a page's texture: pixels per book unit, the body text's font scale and line
/// height, the titles', and the space inside the margins.
pub(super) struct Metrics {
    pub(super) u: f32,
    pub(super) fs: f32,
    pub(super) lh: f32,
    pub(super) tfs: f32,
    pub(super) tlh: f32,
    pub(super) w: f32,
    pub(super) h: f32,
}

pub(super) fn metrics() -> Metrics {
    let u = U;
    let (fs, tfs) = (1.0, 2.0);
    Metrics {
        u,
        fs,
        lh: fs * 10.0,
        tfs,
        tlh: tfs * 10.0,
        w: (PAGE_W - 2.0 * MARGIN_X) * u,
        h: (PAGE_H - MARGIN_TOP - MARGIN_BOTTOM) * u,
    }
}

/// A piece of a page, laid out.
#[derive(Clone)]
pub(super) enum Piece {
    Cover,
    Contents,
    Break,
    /// A chapter's title, and its short name for its tab.
    Title(Vec<String>, String),
    Head(String),
    /// A line of a paragraph; a bullet's lines are indented, its first after a dash.
    Line { text: String, indent: bool, dash: bool },
    Gap(f32),
    Recipe(ItemId),
    /// What an item smelts into, and where how fast.
    Smelt(ItemId, String),
    Row(ItemId, Vec<String>),
    Pic(Pic),
    Stat(String, f32, String),
}

impl Piece {
    /// It moves (the pictures, the recipes' and furnaces' arrows).
    fn animated(&self) -> bool {
        matches!(self, Piece::Cover | Piece::Pic(_) | Piece::Recipe(_) | Piece::Smelt(..))
    }

    /// How tall it is on the page (pixels of the page's texture).
    pub(super) fn height(&self, m: &Metrics) -> f32 {
        let u = m.u;
        match self {
            Piece::Cover | Piece::Contents => m.h,
            Piece::Break => 0.0,
            Piece::Title(lines, _) => lines.len() as f32 * m.tlh + 9.0 * u,
            Piece::Head(_) => m.lh + 4.0 * u,
            Piece::Line { .. } => m.lh,
            Piece::Gap(g) => *g,
            Piece::Recipe(id) => {
                let rows = recipe_view(*id).map_or(1, |(r, _)| r.len());
                rows as f32 * 18.0 * u + m.lh + 8.0 * u
            }
            Piece::Smelt(..) => (20.0 * u).max(2.0 * m.lh + 2.0 * u),
            Piece::Row(_, lines) => (19.0 * u).max(lines.len() as f32 * m.lh + 4.0 * u),
            Piece::Pic(p) => p.height() * u,
            Piece::Stat(..) => m.lh.max(7.0 * u) + 2.0 * u,
        }
    }
}

/// What the book says as pieces of pages: paragraphs cut into lines that fit.
fn pieces(font: &Font, m: &Metrics, el: El) -> Vec<Piece> {
    let u = m.u;
    match el {
        El::Chapter(t, short) => vec![Piece::Break, Piece::Title(font.wrap(&t, m.w, m.tfs), short)],
        El::Break => vec![Piece::Break],
        El::Head(t) => vec![Piece::Head(t)],
        El::Text(t) => {
            let mut v: Vec<Piece> = font.wrap(&t, m.w, m.fs).into_iter().map(|text| Piece::Line { text, indent: false, dash: false }).collect();
            v.push(Piece::Gap(4.0 * u));
            v
        }
        El::Bullet(t) => {
            let mut v: Vec<Piece> = font
                .wrap(&t, m.w - INDENT * u, m.fs)
                .into_iter()
                .enumerate()
                .map(|(i, text)| Piece::Line { text, indent: true, dash: i == 0 })
                .collect();
            v.push(Piece::Gap(3.0 * u));
            v
        }
        El::Recipe(id) => vec![Piece::Recipe(id)],
        El::Smelt(id, at) => vec![Piece::Smelt(id, at)],
        El::Row(id, t) => vec![Piece::Row(id, font.wrap(&t, m.w - ROW_TEXT * u, m.fs))],
        El::Picture(p) => vec![Piece::Pic(p)],
        El::Stat(a, k, b) => vec![Piece::Stat(a, k, b)],
    }
}

/// The book laid out in one language.
pub(super) struct Layout {
    /// Each page's pieces with their heights on it.
    pub(super) pages: Vec<Vec<(f32, Piece)>>,
    /// Chapter titles and the page each starts on.
    pub(super) chapters: Vec<(String, usize)>,
    /// The chapters' short names, for their tabs.
    pub(super) shorts: Vec<String>,
    /// The cover's and the contents page's words.
    pub(super) front: FrontMatter,
}

impl Layout {
    /// Something on the page moves.
    pub(super) fn animated(&self, page: usize) -> bool {
        self.pages.get(page).is_some_and(|p| p.iter().any(|(_, piece)| piece.animated()))
    }
}

/// Lays the book out on pages: the cover, the contents, then the chapters. `keys` are the
/// names of the inventory and reload keys.
pub(super) fn layout(font: &Font, hu: bool, keys: &(String, String)) -> Layout {
    let m = metrics();
    let flat: Vec<Piece> = content(hu, keys).into_iter().flat_map(|e| pieces(font, &m, e)).collect();
    let mut pages: Vec<Vec<(f32, Piece)>> = vec![vec![(0.0, Piece::Cover)], vec![(0.0, Piece::Contents)]];
    let mut chapters = Vec::new();
    let mut shorts = Vec::new();
    let mut cur: Vec<(f32, Piece)> = Vec::new();
    let mut y = 0.0;
    for (i, p) in flat.iter().enumerate() {
        let h = p.height(&m);
        let mut need = h;
        match p {
            Piece::Break => {
                if !cur.is_empty() {
                    pages.push(std::mem::take(&mut cur));
                    y = 0.0;
                }
                continue;
            }
            Piece::Gap(_) if cur.is_empty() => continue,
            // A heading stays with what follows it.
            Piece::Head(_) | Piece::Title(..) => {
                if let Some(next) = flat.get(i + 1) {
                    need += next.height(&m);
                }
            }
            _ => {}
        }
        if y + need > m.h && !cur.is_empty() {
            pages.push(std::mem::take(&mut cur));
            y = 0.0;
            if matches!(p, Piece::Gap(_)) {
                continue;
            }
        }
        if let Piece::Title(t, short) = p {
            chapters.push((t.join(" "), pages.len()));
            shorts.push(short.clone());
        }
        cur.push((y, p.clone()));
        y += h;
    }
    if !cur.is_empty() {
        pages.push(cur);
    }
    Layout { pages, chapters, shorts, front: front_matter(hu) }
}

/// Where the contents' entries are on its page: the first one's top, and each one's height
/// (pixels of the page's texture).
pub(super) fn contents_rows(m: &Metrics) -> (f32, f32) {
    (MARGIN_TOP * m.u + m.tlh + 12.0 * m.u, m.lh + 5.0 * m.u)
}

/// The contents entry at height `y` of the contents page.
pub(super) fn contents_entry(lay: &Layout, y: f32) -> Option<usize> {
    let m = metrics();
    let (top, row) = contents_rows(&m);
    let k = (y - top + 2.0 * m.u) / row;
    (k >= 0.0 && (k as usize) < lay.chapters.len()).then_some(k as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_book_fits_its_pages_in_both_languages() {
        let font = Font::new();
        let keys = ("E".to_string(), "R".to_string());
        let m = metrics();
        assert!(PAGE_PX.0 <= SHEET_W as f32 && PAGE_PX.1 <= SHEET_H as f32);
        for hu in [false, true] {
            let lay = layout(&font, hu, &keys);
            assert_eq!(lay.chapters.len(), 8, "hu={hu}");
            assert!(lay.chapters.windows(2).all(|c| c[0].1 < c[1].1));
            for (i, page) in lay.pages.iter().enumerate().skip(2) {
                assert!(!page.is_empty(), "empty page {i}");
                for (y, p) in page {
                    assert!(y + p.height(&m) <= m.h + 0.01, "page {i} runs over (hu={hu})");
                    let text = match p {
                        Piece::Line { text, indent, .. } => Some((text, if *indent { INDENT * m.u } else { 0.0 })),
                        Piece::Row(_, lines) => lines.first().map(|l| (l, ROW_TEXT * m.u)),
                        _ => None,
                    };
                    if let Some((t, indent)) = text {
                        assert!(font.text_width(t, m.fs) <= m.w - indent + 0.01, "too wide: {t}");
                    }
                    if let Piece::Recipe(id) = p {
                        assert!(recipe_view(*id).is_some(), "no recipe for {id}");
                    }
                }
            }
            // The contents fit on their page, and their rows are found again.
            let (top, row) = contents_rows(&m);
            assert!(top + row * lay.chapters.len() as f32 <= MARGIN_TOP * m.u + m.h);
            assert_eq!(contents_entry(&lay, top + row * 2.5), Some(2));
        }
    }
}
