//! The pages of the books in hands drawn onto block texture layers: this player's (the
//! open spread, the page turning, the spreads around it, the chapter tabs) and those the
//! other players hold near, each page into one of a few sheets of layers, a few a frame; the
//! pages that move drawn again while being read.

use super::draw::{DARK, LIGHT, Look, draw_page, draw_tabs};
use super::layout::{PAGE_PX, contents_entry};
use crate::client::Game;
use crate::item::GUIDE_BOOK;
use crate::model::book::{BookHit, BookView, SHEET_LAYERS, TAB_LAYERS};
use crate::textures::{TILE, tex};

/// Pages drawn onto the textures in one frame at most (the rest wait for the next ones), and
/// how many of them may be redrawn only to move their animations on.
const DRAWS_PER_FRAME: usize = 4;
const ANIM_DRAWS_PER_FRAME: usize = 2;
/// Pages that move are drawn again this many times a second while being read.
const ANIM_FPS: f32 = 20.0;
/// The other players' books are drawn up to this far away (blocks).
const SEE_PAGES: f32 = 24.0;

/// A page as drawn: which, in which language and theme, and the contents entry lit up.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct PageId {
    page: usize,
    hu: bool,
    dark: bool,
    hover: Option<usize>,
}

/// One page's texture layers, and what is drawn on them.
#[derive(Default)]
pub(super) struct Sheet {
    pub(super) page: Option<PageId>,
    /// The animation frame drawn.
    frame: u32,
    /// The last `Book::frame` it was shown.
    used: u64,
}

/// The pages shown on a book open at `spread`, while a page turns (`turn`, see `BookView`):
/// the left and right pages under it, and the turning page's faces.
fn pages_shown(spread: usize, turn: f32) -> [Option<usize>; 4] {
    let s = spread as isize;
    let p = |k: isize| (s + k >= 0).then_some((s + k) as usize);
    if turn > 0.0 {
        [p(-2), p(1), p(-1), p(0)]
    } else if turn < 0.0 {
        [p(0), p(3), p(1), p(2)]
    } else {
        [p(0), p(1), None, None]
    }
}

/// A texture `cols` layers wide (a page's 2 x 3, the tabs' 4 x 1) cut into its `n` layers,
/// with their mip levels (averaged in linear light, like the block textures'; cut-out
/// alpha kept).
pub(in crate::client) fn sheet_levels(px: &[[u8; 4]], w: usize, cols: usize, n: usize) -> Vec<Vec<u8>> {
    let mut base = vec![0u8; TILE * TILE * 4 * n];
    for l in 0..n {
        let (cx, cy) = ((l % cols) * TILE, (l / cols) * TILE);
        for y in 0..TILE {
            for x in 0..TILE {
                let p = px[(cy + y) * w + cx + x];
                base[((l * TILE + y) * TILE + x) * 4..][..4].copy_from_slice(&p);
            }
        }
    }
    let linear: [f32; 256] = std::array::from_fn(|v| (v as f32 / 255.0).powf(2.2));
    let mut levels = vec![base];
    let mut size = TILE;
    while size > 1 {
        let prev = levels.last().unwrap();
        let ns = size / 2;
        let mut next = vec![0u8; ns * ns * 4 * n];
        for l in 0..n {
            for y in 0..ns {
                for x in 0..ns {
                    let o = ((l * ns + y) * ns + x) * 4;
                    let at = |dx: usize, dy: usize| ((l * size + y * 2 + dy) * size + x * 2 + dx) * 4;
                    let (mut rgb, mut wsum) = ([0.0f32; 3], 0.0);
                    for i in [at(0, 0), at(1, 0), at(0, 1), at(1, 1)] {
                        let a = prev[i + 3] as f32 / 255.0;
                        for c in 0..3 {
                            rgb[c] += linear[prev[i + c] as usize] * a;
                        }
                        wsum += a;
                    }
                    for c in 0..3 {
                        let v = if wsum > 0.0 { rgb[c] / wsum } else { 0.0 };
                        next[o + c] = (v.powf(1.0 / 2.2) * 255.0).round() as u8;
                    }
                    next[o + 3] = if wsum / 4.0 > 0.4 { 255 } else { 0 };
                }
            }
        }
        levels.push(next);
        size = ns;
    }
    levels
}

impl Game {
    /// Each frame before the models are built: which pages the books in hands show (this
    /// player's and the others'), drawn onto texture layers where they are not yet.
    pub(in crate::client) fn update_book_views(&mut self, dt: f32) {
        use crate::net::book::HUNGARIAN;
        self.book.frame += 1;
        let frame = self.book.frame;
        let dark = self.settings.dark_ui;
        // What is wanted: a page, and the animation frame it should show (None: any).
        let mut wants: Vec<(PageId, Option<u32>)> = Vec::new();
        let anim = (self.clock.time * ANIM_FPS) as u32;
        // Pages drawn ahead (last: they must not push out pages that are shown).
        let mut ahead: Vec<(PageId, Option<u32>)> = Vec::new();

        let own = if self.holding_book() {
            let hu = self.book_hu();
            let (turn, spread) = match &self.book.turn {
                Some(t) => {
                    let k = t.t / t.time;
                    (if t.to > t.from { k.max(1e-3) } else { -k.max(1e-3) }, t.to)
                }
                None => (0.0, self.book.spread),
            };
            let hover = match (self.book.hit, spread, self.book.layouts[hu as usize].as_ref()) {
                (Some(BookHit::Page(true, at)), 0, Some(l)) if self.book.turn.is_none() => {
                    contents_entry(l, at.y * PAGE_PX.1)
                }
                _ => None,
            };
            let reading = self.book.read > 0.3;
            let pages = pages_shown(spread, turn).map(|p| {
                p.filter(|&p| p < self.book.pages(hu)).map(|page| {
                    let id = PageId {
                        page,
                        hu,
                        dark,
                        hover: hover.filter(|_| page == 1),
                    };
                    let animated = reading
                        && self.book.layouts[hu as usize].as_ref().is_some_and(|l| l.animated(page));
                    wants.push((id, animated.then_some(anim)));
                    id
                })
            });
            // The spreads before and after are drawn ahead, so turning to them shows them
            // at once.
            if self.book.turn.is_none() {
                for page in [spread + 2, spread + 3, spread.wrapping_sub(2), spread.wrapping_sub(1)] {
                    if page < self.book.pages(hu) {
                        ahead.push((PageId { page, hu, dark, hover: None }, None));
                    }
                }
            }
            Some((turn, pages))
        } else {
            None
        };
        let me = self.me.body.pos;
        let mut remote = Vec::new();
        for i in 0..self.session.remotes.len() {
            let r = &mut self.session.remotes[i];
            let turn = r.book.update(r.pose.book, dt);
            let near = r.pose.pos.distance(me) < SEE_PAGES;
            let shown = r.book.show(r.pose.flags & crate::net::pose_flags::SHOWING != 0, dt);
            let (Some(turn), true, true) = (turn, r.pose.held == GUIDE_BOOK, near) else {
                r.book_view = None;
                continue;
            };
            let hu = r.pose.book_page & HUNGARIAN != 0;
            let spread = (r.pose.book_page & !HUNGARIAN) as usize * 2;
            remote.push((i, turn, hu, spread, shown));
        }
        let mut remote_pages = Vec::new();
        for (i, turn, hu, spread, shown) in remote {
            self.book_layout(hu);
            let count = self.book.pages(hu);
            let pages = pages_shown(spread, turn).map(|p| {
                p.filter(|&p| p < count).map(|page| {
                    let id = PageId { page, hu, dark, hover: None };
                    wants.push((id, None));
                    id
                })
            });
            remote_pages.push((i, turn, pages, shown));
        }

        wants.extend(ahead);
        // Give each wanted page its sheet; draw what is missing (a few per frame), before
        // moving animations on.
        let mut draws = 0;
        let mut anim_draws = 0;
        let mut placed: Vec<(PageId, u32)> = Vec::new();
        for (id, want) in wants {
            if placed.iter().any(|p| p.0 == id) {
                continue;
            }
            let slot = match self.book.sheets.iter().position(|s| s.page == Some(id)) {
                Some(i) => {
                    let stale = want.is_some_and(|f| f != self.book.sheets[i].frame);
                    if stale && anim_draws < ANIM_DRAWS_PER_FRAME && draws < DRAWS_PER_FRAME {
                        anim_draws += 1;
                        draws += 1;
                        self.draw_sheet(i, id, want.unwrap_or(0));
                    }
                    Some(i)
                }
                None if draws < DRAWS_PER_FRAME => {
                    let free = (0..self.book.sheets.len())
                        .filter(|&i| self.book.sheets[i].used < frame)
                        .min_by_key(|&i| self.book.sheets[i].used);
                    if let Some(i) = free {
                        draws += 1;
                        self.draw_sheet(i, id, want.unwrap_or(0));
                    }
                    free
                }
                None => None,
            };
            if let Some(i) = slot {
                self.book.sheets[i].used = frame;
                placed.push((id, tex::BOOK_SHEETS + i as u32 * SHEET_LAYERS));
            }
        }
        let shown: Vec<(PageId, u32)> = placed;
        let pages_of = |pages: [Option<PageId>; 4]| {
            pages.map(|id| id.and_then(|id| shown.iter().find(|p| p.0 == id).map(|p| p.1)))
        };
        // The chapter tabs on this player's book, drawn again when what they show changes.
        let tabs = if own.is_some() {
            let hu = self.book_hu();
            let hover = match self.book.hit {
                Some(BookHit::Tab(i)) => Some(i),
                _ => None,
            };
            let want = (hu, dark, self.book_chapter(hu), hover);
            if self.book.tabs != Some(want) {
                self.draw_tabs(want);
            }
            self.book.tabs.map(|_| tex::BOOK_TABS)
        } else {
            None
        };
        let show = self.book.show;
        self.book.view = own.map(|(turn, pages)| BookView {
            turn,
            pages: pages_of(pages),
            tabs,
            show,
        });
        for (i, turn, pages, show) in remote_pages {
            self.session.remotes[i].book_view = Some(BookView {
                turn,
                pages: pages_of(pages),
                tabs: None,
                show,
            });
        }
    }

    /// Draws the chapter tabs onto their texture layers.
    fn draw_tabs(&mut self, (hu, dark, open, hover): (bool, bool, Option<usize>, Option<usize>)) {
        self.book_layout(hu);
        let Some(lay) = self.book.layouts[hu as usize].as_ref() else {
            return;
        };
        let theme = if dark { &DARK } else { &LIGHT };
        let levels = {
            let cv = draw_tabs(&self.ui.pixel_font, &self.gfx.texture_base, lay, open, hover, theme);
            sheet_levels(&cv.px, cv.w, TAB_LAYERS as usize, TAB_LAYERS as usize)
        };
        self.gfx.renderer.queue_layers(tex::BOOK_TABS, TAB_LAYERS, levels);
        self.book.tabs = Some((hu, dark, open, hover));
    }

    /// Draws a page onto sheet `i`'s texture layers.
    fn draw_sheet(&mut self, i: usize, id: PageId, frame: u32) {
        self.book_layout(id.hu);
        let Some(lay) = self.book.layouts[id.hu as usize].as_ref() else {
            return;
        };
        let look = Look {
            time: frame as f32 / ANIM_FPS,
            theme: if id.dark { &DARK } else { &LIGHT },
            hover: id.hover,
        };
        let levels = {
            let cv = draw_page(&self.ui.pixel_font, &self.gfx.texture_base, lay, id.page, &look);
            sheet_levels(&cv.px, cv.w, 2, SHEET_LAYERS as usize)
        };
        let base = tex::BOOK_SHEETS + i as u32 * SHEET_LAYERS;
        self.gfx.renderer.queue_layers(base, SHEET_LAYERS, levels);
        let s = &mut self.book.sheets[i];
        s.page = Some(id);
        s.frame = frame;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turning_page_shows_the_pages_on_both_sides() {
        // Forward from spread 2 to 4: the old left page stays, the new right one is under
        // the turning page, which shows the old right page and then the new left one.
        assert_eq!(pages_shown(4, 0.5), [Some(2), Some(5), Some(3), Some(4)]);
        // Back from 4 to 2: the new left page, the old right one, and between them the
        // turning page with the new right page on its right face and the old left on its left.
        assert_eq!(pages_shown(2, -0.5), [Some(2), Some(5), Some(3), Some(4)]);
        assert_eq!(pages_shown(0, 0.0), [Some(0), Some(1), None, None]);
    }

    #[test]
    fn a_page_texture_is_cut_into_six_layers_with_mips() {
        let w = super::super::layout::SHEET_W;
        let px = vec![[10u8, 20, 30, 255]; w * super::super::layout::SHEET_H];
        let levels = sheet_levels(&px, w, 2, SHEET_LAYERS as usize);
        assert_eq!(levels.len(), TILE.trailing_zeros() as usize + 1);
        assert_eq!(levels[0].len(), TILE * TILE * 4 * SHEET_LAYERS as usize);
        assert_eq!(levels.last().unwrap().len(), 4 * SHEET_LAYERS as usize);
        assert_eq!(&levels[3][..4], &[10, 20, 30, 255]);
    }
}
