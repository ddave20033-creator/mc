//! The guide book in the hands: held open in both hands like a map, and lifted up in front of
//! the eyes to read when looking down. The right mouse button turns a page, the left one
//! turns back (so do the arrow keys); while reading, the number keys open the chapters
//! marked on the tabs along its top.
//! The pages are drawn onto block texture layers (`content`, `canvas`): those of this
//! player's book and of the books in the other players' hands, who send the page they are
//! on, so everyone sees what is on it.

mod canvas;
mod content;

use super::*;
use crate::item::GUIDE_BOOK;
use crate::model::book::{BookHit, BookView, RIFFLE_TIME, SHEET_LAYERS, TAB_LAYERS, TURN_TIME};
use crate::world::textures::{tex, TILE};
use content::{contents_entry, draw_page, draw_tabs, layout, Layout, Look, DARK, LIGHT, PAGE_PX};

/// Pages drawn onto the textures in one frame at most (the rest wait for the next ones), and
/// how many of them may be redrawn only to move their animations on.
const DRAWS_PER_FRAME: usize = 4;
const ANIM_DRAWS_PER_FRAME: usize = 2;
/// Pages that move are drawn again this many times a second while being read.
const ANIM_FPS: f32 = 20.0;
/// The other players' books are drawn up to this far away (blocks).
const SEE_PAGES: f32 = 24.0;

/// A page turning over: from the spread whose left page is `from` to the one at `to`, in
/// `time` seconds.
struct Turn {
    from: usize,
    to: usize,
    t: f32,
    time: f32,
}

/// At most this many pages turn over on the way to a chapter far away.
const RIFFLE_TURNS: usize = 6;
/// Holding the right button this long turns the book around to show it (a shorter click
/// turns a page).
const SHOW_HOLD: f32 = 0.25;

/// A page as drawn: which, in which language and theme, and the contents entry lit up.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct PageId {
    page: usize,
    hu: bool,
    dark: bool,
    hover: Option<usize>,
}

/// One page's texture layers, and what is drawn on them.
#[derive(Default)]
struct Sheet {
    page: Option<PageId>,
    /// The animation frame drawn.
    frame: u32,
    /// The last `Book::frame` it was shown.
    used: u64,
}

pub(super) struct Book {
    /// The left page of the open spread (always even).
    spread: usize,
    turn: Option<Turn>,
    /// The spreads still to turn to, one after the other (leafing through to a chapter).
    queue: std::collections::VecDeque<usize>,
    /// How long the right button has been held (None: not held), and whether the book is
    /// turned around to show it, and how far it has turned.
    right_held: Option<f32>,
    showing: bool,
    show: f32,
    /// Page turns so far and whether the last one went back (for the others to see).
    turns: u8,
    back: bool,
    /// How far the book is lifted up to read: 0 held low .. 1 in front of the eyes.
    read: f32,
    /// The book laid out in English and in Hungarian (made when first needed).
    layouts: [Option<Layout>; 2],
    sheets: Vec<Sheet>,
    frame: u64,
    /// What the middle of the view was on last frame (see `book::book_hit`).
    hit: Option<BookHit>,
    /// What the chapter tabs are drawn with: language, theme, the chapter open and the tab
    /// aimed at.
    tabs: Option<(bool, bool, Option<usize>, Option<usize>)>,
    /// This player's book as it is shown now.
    view: Option<BookView>,
}

impl Default for Book {
    fn default() -> Self {
        Self {
            spread: 0,
            turn: None,
            queue: Default::default(),
            right_held: None,
            showing: false,
            show: 0.0,
            turns: 0,
            back: false,
            read: 0.0,
            layouts: [None, None],
            sheets: (0..tex::BOOK_SHEET_COUNT).map(|_| Sheet::default()).collect(),
            frame: 0,
            hit: None,
            tabs: None,
            view: None,
        }
    }
}

impl Book {
    fn pages(&self, hu: bool) -> usize {
        self.layouts[hu as usize].as_ref().map_or(2, |l| l.pages.len())
    }

    /// Turns to the spread holding `page`: one page over, or leafing quickly through a few
    /// on the way to one far away (each page shows what is really there).
    fn go_to(&mut self, page: usize, hu: bool) {
        let to = (page.min(self.pages(hu).saturating_sub(1)) & !1) as isize;
        let from = self.spread as isize;
        self.queue.clear();
        if to == from {
            return;
        }
        let n = (((to - from).abs() / 2) as usize).clamp(1, RIFFLE_TURNS) as isize;
        for i in 1..=n {
            let s = ((from + (to - from) * i / n) as usize) & !1;
            if self.queue.back() != Some(&s) && s != self.spread {
                self.queue.push_back(s);
            }
        }
        if self.turn.is_none() {
            self.next_turn();
        }
    }

    /// Starts turning to the next spread waiting.
    fn next_turn(&mut self) {
        let Some(to) = self.queue.pop_front() else {
            return;
        };
        let from = self.spread;
        self.back = to < from;
        self.turns = self.turns.wrapping_add(1);
        self.spread = to;
        let leafing = !self.queue.is_empty() || (to as isize - from as isize).abs() > 2;
        let time = if leafing { RIFFLE_TIME } else { TURN_TIME };
        self.turn = Some(Turn { from, to, t: 0.0, time });
    }

    /// The spread the book ends up at once the turns waiting are done.
    fn target(&self) -> usize {
        self.queue.back().copied().unwrap_or(self.spread)
    }

    /// One page on (after the ones already waiting to turn, so quick clicks all count).
    fn next(&mut self, hu: bool) {
        let t = self.target();
        if t + 2 < self.pages(hu) {
            self.queue.push_back(t + 2);
            if self.turn.is_none() {
                self.next_turn();
            }
        }
    }

    fn prev(&mut self) {
        let t = self.target();
        if t >= 2 {
            self.queue.push_back(t - 2);
            if self.turn.is_none() {
                self.next_turn();
            }
        }
    }

    /// Every sheet is drawn again (the block textures were made anew).
    pub(super) fn textures_remade(&mut self) {
        for s in &mut self.sheets {
            s.page = None;
        }
        self.tabs = None;
    }
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
pub(in crate::game) fn sheet_levels(px: &[[u8; 4]], w: usize, cols: usize, n: usize) -> Vec<Vec<u8>> {
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
    fn holding_book(&self) -> bool {
        self.held() == GUIDE_BOOK && self.player.spawned && self.world_meta.is_some()
    }

    fn book_hu(&self) -> bool {
        crate::lang::is_hungarian()
    }

    /// Lays the book out in a language, if it is not yet.
    fn book_layout(&mut self, hu: bool) {
        if self.book.layouts[hu as usize].is_none() {
            use crate::keys::display;
            let keys = (
                display(self.settings.keys.get(Bind::Inventory)),
                display(self.settings.keys.get(Bind::Reload)),
            );
            self.book.layouts[hu as usize] = Some(layout(&self.ui.font, hu, &keys));
        }
    }

    /// Every frame: the book comes up to read while looking down (and goes down again), and
    /// a page turning goes on.
    pub(super) fn update_book(&mut self, dt: f32) {
        let holding = self.holding_book();
        let target = if holding && self.screen == Screen::Playing {
            ((-self.pitch.to_degrees() - 12.0) / 33.0).clamp(0.0, 1.0)
        } else if holding {
            self.book.read
        } else {
            0.0
        };
        self.book.read += (target - self.book.read) * (1.0 - (-9.0 * dt).exp());
        if !holding {
            self.book.showing = false;
            self.book.right_held = None;
        }
        let show = if self.book.showing { 1.0 } else { 0.0 };
        self.book.show += (show - self.book.show) * (1.0 - (-8.0 * dt).exp());
        if let Some(t) = &mut self.book.turn {
            t.t += dt;
            if t.t >= t.time {
                self.book.turn = None;
                self.book.next_turn();
            }
        }
        if holding {
            let hu = self.book_hu();
            self.book_layout(hu);
        }
    }

    /// Holding the book up to read (the number keys pick chapters then).
    pub(super) fn book_reading(&self) -> bool {
        self.holding_book() && self.book.read > 0.55
    }

    /// Holding the book: the mouse buttons are for its pages (never hitting blocks).
    pub(super) fn book_in_hand(&self) -> bool {
        self.holding_book()
    }

    /// The chapter the open spread is in.
    fn book_chapter(&self, hu: bool) -> Option<usize> {
        let lay = self.book.layouts[hu as usize].as_ref()?;
        lay.chapters.iter().rposition(|c| c.1 <= self.book.spread + 1)
    }

    /// Opens chapter `i` (0 the first).
    fn book_open_chapter(&mut self, i: usize) {
        let hu = self.book_hu();
        let page = self.book.layouts[hu as usize]
            .as_ref()
            .and_then(|l| l.chapters.get(i))
            .map(|c| c.1);
        if let Some(page) = page {
            self.book.go_to(page, hu);
        }
    }

    /// A number key while reading: opens that chapter instead of picking a hotbar slot.
    /// Returns true when it did.
    pub(super) fn book_digit(&mut self, i: usize) -> bool {
        if !self.book_reading() {
            return false;
        }
        self.book_open_chapter(i);
        true
    }

    /// The mouse buttons while holding the book: a left click turns back, a right click
    /// forward (the book moves with the view, so there is no aiming at its pages); holding
    /// the right button turns the book around to show it to someone, until it is let go.
    pub(super) fn book_buttons(&mut self, dt: f32, left_pressed: bool, right_down: bool) {
        let hu = self.book_hu();
        if left_pressed && !self.book.showing {
            self.book.prev();
        }
        match (self.book.right_held, right_down) {
            (None, true) => self.book.right_held = Some(0.0),
            (Some(t), true) => {
                self.book.right_held = Some(t + dt);
                if t + dt >= SHOW_HOLD {
                    self.book.showing = true;
                }
            }
            (Some(_), false) => {
                if !self.book.showing {
                    self.book.next(hu);
                }
                self.book.showing = false;
                self.book.right_held = None;
            }
            (None, false) => {}
        }
    }

    /// Showing the book to someone (holding the right button).
    pub(super) fn book_showing(&self) -> bool {
        self.holding_book() && self.book.showing
    }

    /// Keys while holding the book: the arrows and Page Up/Down turn pages, Home opens the
    /// contents, End the last page. Returns true when the key did something.
    pub(super) fn book_key(&mut self, code: KeyCode) -> bool {
        if !self.holding_book() {
            return false;
        }
        let hu = self.book_hu();
        match code {
            KeyCode::ArrowRight | KeyCode::PageDown => self.book.next(hu),
            KeyCode::ArrowLeft | KeyCode::PageUp => self.book.prev(),
            KeyCode::Home => self.book.go_to(0, hu),
            KeyCode::End => {
                let last = self.book.pages(hu).saturating_sub(1);
                self.book.go_to(last, hu);
            }
            _ => return false,
        }
        true
    }

    /// The book as the others see it: `Pose::book` and `Pose::book_page`.
    pub(super) fn book_pose(&self) -> (u8, u8) {
        use crate::net::book::{BACK, HUNGARIAN, OPEN, TURNS};
        if !self.holding_book() {
            return (0, 0);
        }
        let open = OPEN | if self.book.back { BACK } else { 0 } | (self.book.turns & TURNS);
        let page = (self.book.spread / 2).min(0x7f) as u8 | if self.book_hu() { HUNGARIAN } else { 0 };
        (open, page)
    }

    /// Reading the book, for the bubble above this player's head.
    pub(super) fn book_status(&self) -> bool {
        self.book_reading()
    }

    /// This player's book as it is shown now (None: not holding it).
    pub(super) fn book_view(&self) -> Option<BookView> {
        self.book.view.filter(|_| self.holding_book())
    }

    /// How far this player's book is lifted to read.
    pub(super) fn book_read(&self) -> f32 {
        self.book.read
    }

    /// Where the view's middle fell on this player's book (from the first-person hand).
    pub(super) fn set_book_hit(&mut self, hit: Option<BookHit>) {
        self.book.hit = hit;
    }

    /// Each frame before the models are built: which pages the books in hands show (this
    /// player's and the others'), drawn onto texture layers where they are not yet.
    pub(super) fn update_book_views(&mut self, dt: f32) {
        use crate::net::book::HUNGARIAN;
        self.book.frame += 1;
        let frame = self.book.frame;
        let dark = self.settings.dark_ui;
        // What is wanted: a page, and the animation frame it should show (None: any).
        let mut wants: Vec<(PageId, Option<u32>)> = Vec::new();
        let anim = (self.time * ANIM_FPS) as u32;
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
        let me = self.player.pos;
        let mut remote = Vec::new();
        for i in 0..self.remotes.len() {
            let r = &mut self.remotes[i];
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
            self.remotes[i].book_view = Some(BookView {
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
            let cv = draw_tabs(&self.ui.font, &self.texture_base, lay, open, hover, theme);
            sheet_levels(&cv.px, cv.w, TAB_LAYERS as usize, TAB_LAYERS as usize)
        };
        self.renderer.queue_layers(tex::BOOK_TABS, TAB_LAYERS, levels);
        self.book.tabs = Some((hu, dark, open, hover));
    }

    /// Draws a page onto sheet `i`'s texture layers.
    fn draw_sheet(&mut self, i: usize, id: PageId, frame: u32) {
        self.book_layout(id.hu);
        let Some(lay) = self.book.layouts[id.hu as usize].as_ref() else {
            return;
        };
        let look = Look {
            hu: id.hu,
            time: frame as f32 / ANIM_FPS,
            theme: if id.dark { &DARK } else { &LIGHT },
            hover: id.hover,
        };
        let levels = {
            let cv = draw_page(&self.ui.font, &self.texture_base, lay, id.page, &look);
            sheet_levels(&cv.px, cv.w, 2, SHEET_LAYERS as usize)
        };
        let base = tex::BOOK_SHEETS + i as u32 * SHEET_LAYERS;
        self.renderer.queue_layers(base, SHEET_LAYERS, levels);
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
        let w = content::SHEET_W;
        let px = vec![[10u8, 20, 30, 255]; w * content::SHEET_H];
        let levels = sheet_levels(&px, w, 2, SHEET_LAYERS as usize);
        assert_eq!(levels.len(), TILE.trailing_zeros() as usize + 1);
        assert_eq!(levels[0].len(), TILE * TILE * 4 * SHEET_LAYERS as usize);
        assert_eq!(levels.last().unwrap().len(), 4 * SHEET_LAYERS as usize);
        assert_eq!(&levels[3][..4], &[10, 20, 30, 255]);
    }

    #[test]
    fn a_book_page_turn_is_counted_for_the_others() {
        let mut b = Book::default();
        b.next(false);
        assert_eq!(b.spread, 0, "cover and contents only before the layout");
        b.layouts[0] = Some(Layout {
            pages: vec![Vec::new(); 7],
            chapters: Vec::new(),
            shorts: Vec::new(),
        });
        b.next(false);
        b.next(false);
        b.next(false);
        // Quick clicks wait their turn; each turns a page when the one before is done.
        assert_eq!(b.target(), 6);
        while b.turn.take().is_some() {
            b.next_turn();
        }
        assert_eq!(b.spread, 6);
        assert_eq!(b.turns, 3);
        b.prev();
        assert!(b.back);
        // Far away: leafing quickly through a few pages on the way.
        b.turn = None;
        b.queue.clear();
        b.spread = 0;
        b.layouts[0].as_mut().unwrap().pages = vec![Vec::new(); 40];
        b.go_to(30, false);
        assert!(b.turn.as_ref().is_some_and(|t| t.time == RIFFLE_TIME));
        assert_eq!(b.queue.back(), Some(&30));
        assert!(b.queue.len() < RIFFLE_TURNS);
    }
}
