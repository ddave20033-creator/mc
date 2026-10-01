//! The guide book in the hands: held open in both hands like a map, and lifted up in front of
//! the eyes to read when looking down. The right mouse button turns a page, the left one
//! turns back (so do the arrow keys); while reading, the number keys open the chapters
//! marked on the tabs along its top.
//! The pages are drawn onto block texture layers (`sheets`): those of this player's book and
//! of the books in the other players' hands, who send the page they are on, so everyone sees
//! what is on it.
//!
//! - `pages`: what the book says, chapter by chapter, in English and Hungarian.
//! - `layout`: that laid out on pages; `draw` and `pictures`: a page drawn on a `canvas`.

mod canvas;
mod draw;
mod layout;
mod pages;
mod pictures;
mod sheets;

pub(super) use sheets::sheet_levels;

use crate::client::{Game, Screen};
use crate::item::GUIDE_BOOK;
use crate::app::keys::Bind;
use crate::model::items::book::{BookHit, BookView, RIFFLE_TIME, TURN_TIME};
use crate::textures::tex;
use winit::keyboard::KeyCode;

use layout::{Layout, layout};
use sheets::Sheet;

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
    /// The book laid out in English and in Hungarian (made when first needed), and the keys
    /// its text names (bound to others, it is laid out again).
    layouts: [Option<Layout>; 2],
    layout_keys: [Option<(String, String)>; 2],
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
            layout_keys: [None, None],
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


impl Game {
    fn holding_book(&self) -> bool {
        self.held() == GUIDE_BOOK && self.me.body.spawned && self.level.meta.is_some()
    }

    fn book_hu(&self) -> bool {
        crate::app::lang::is_hungarian()
    }

    /// Lays the book out in a language, if it is not yet (or the keys it names were bound
    /// to others since: its pages are drawn again).
    fn book_layout(&mut self, hu: bool) {
        use crate::app::keys::display;
        let keys = (
            display(self.settings.keys.get(Bind::Inventory)),
            display(self.settings.keys.get(Bind::Reload)),
        );
        let i = hu as usize;
        if self.book.layouts[i].is_none() || self.book.layout_keys[i].as_ref() != Some(&keys) {
            if self.book.layouts[i].is_some() {
                self.book.textures_remade();
            }
            self.book.layouts[i] = Some(layout(&self.ui.pixel_font, hu, &keys));
            self.book.layout_keys[i] = Some(keys);
        }
    }

    /// Every frame: the book comes up to read while looking down (and goes down again), and
    /// a page turning goes on.
    pub(super) fn update_book(&mut self, dt: f32) {
        let holding = self.holding_book();
        let target = if holding && self.screen == Screen::Playing {
            ((-self.me.look.pitch.to_degrees() - 12.0) / 33.0).clamp(0.0, 1.0)
        } else if holding {
            self.book.read
        } else {
            0.0
        };
        self.book.read += (target - self.book.read) * (crate::util::damp(9.0, dt));
        if !holding {
            self.book.showing = false;
            self.book.right_held = None;
        }
        let show = if self.book.showing { 1.0 } else { 0.0 };
        self.book.show += (show - self.book.show) * (crate::util::damp(8.0, dt));
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
    fn book_reading(&self) -> bool {
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

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_book_page_turn_is_counted_for_the_others() {
        let mut b = Book::default();
        b.next(false);
        assert_eq!(b.spread, 0, "cover and contents only before the layout");
        b.layouts[0] = Some(Layout {
            pages: vec![Vec::new(); 7],
            chapters: Vec::new(),
            shorts: Vec::new(),
            front: pages::front_matter(false),
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
