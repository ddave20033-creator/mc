//! What the guide book says, chapter by chapter, in English and Hungarian, before it is laid
//! out on pages (`layout`): the text, and the recipes, furnace times, fuels and gun numbers
//! read from the game itself (so it always tells how things work now). Each chapter is its
//! own file, in the order of the book.

mod ammo;
mod furnaces;
mod grill;
mod guns;
mod recipes;
mod start;
mod station;
mod tools;

use super::pictures::Pic;
use crate::entity::Furnace;
use crate::entity::survival::Consumable;
use crate::item::*;
use crate::world::{Block, FURNACE, FURNACES};

/// What the book says, before it is laid out on pages.
pub(super) enum El {
    /// Starts a new page with a big title, and goes in the contents (and on a tab, by its
    /// short name).
    Chapter(String, String),
    Head(String),
    Text(String),
    /// A paragraph after a dash.
    Bullet(String),
    /// The recipe making this item.
    Recipe(ItemId),
    /// What this item smelts into, and in which furnace how fast.
    Smelt(ItemId, String),
    /// An item with a few words beside it.
    Row(ItemId, String),
    Picture(Pic),
    /// A gun's number: its name, how big it is next to the other guns' (0..1), the value.
    Stat(String, f32, String),
    /// The rest goes on the next page.
    Break,
}

/// The words on the cover and the contents page.
#[derive(Clone)]
pub(super) struct FrontMatter {
    pub(super) title: String,
    pub(super) subtitle: String,
    /// How to read it, at the foot of the cover.
    pub(super) hint: String,
    pub(super) contents: String,
    /// Under the contents: how to open a chapter.
    pub(super) tip: String,
}

/// Text in the book's language.
pub(super) fn tr(hu: bool, en: &str, hun: &str) -> String {
    (if hu { hun } else { en }).to_string()
}

/// A number as the language writes it (a decimal comma in Hungarian).
pub(super) fn num(hu: bool, v: f32) -> String {
    let s = if (v - v.round()).abs() < 0.01 {
        format!("{}", v.round())
    } else if (v * 10.0 - (v * 10.0).round()).abs() < 0.01 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    };
    if hu {
        s.replace('.', ",")
    } else {
        s
    }
}

/// Seconds a furnace of this tier takes to smelt one item.
pub(super) fn smelt_time(tier: u8) -> f32 {
    Furnace { tier, ..Default::default() }.smelt_time()
}

/// The furnace of a tier (`FURNACES`; the plain one for any other).
fn furnace_of_tier(tier: u8) -> Block {
    FURNACES.get((tier as usize).wrapping_sub(1)).copied().unwrap_or(FURNACE)
}

/// The cover's and the contents page's words.
pub(super) fn front_matter(hu: bool) -> FrontMatter {
    FrontMatter {
        title: tr(hu, "Guide Book", "Kézikönyv"),
        subtitle: tr(hu, "Crafting \u{b7} Furnaces \u{b7} Guns", "Barkácsolás \u{b7} Kemencék \u{b7} Fegyverek"),
        hint: tr(
            hu,
            "Look down to read. Right click: next page, left click: back.",
            "Nézz le az olvasáshoz. Jobb klikk: lapozás, bal klikk: vissza.",
        ),
        contents: tr(hu, "Contents", "Tartalom"),
        tip: tr(
            hu,
            "While reading, press a chapter's number (1-8, also on the tabs on top) to open it. Home comes back here.",
            "Olvasás közben a fejezet számával (1-8, a fenti füleken is) odalapozol. A Home ide hoz vissza.",
        ),
    }
}

/// The book being written: what it says so far in its language, the keys its text names, and
/// the recipes shown already (the last chapter has all the others).
pub(super) struct Writer<'k> {
    hu: bool,
    /// The names of the inventory and the reload keys.
    keys: &'k (String, String),
    out: Vec<El>,
    shown: Vec<ItemId>,
}

impl Writer<'_> {
    /// Text in the book's language.
    fn tr(&self, en: &str, hun: &str) -> String {
        tr(self.hu, en, hun)
    }

    /// A number as the book's language writes it.
    fn num(&self, v: f32) -> String {
        num(self.hu, v)
    }

    fn push(&mut self, el: El) {
        self.out.push(el);
    }

    fn chapter(&mut self, title: (&str, &str), short: (&str, &str)) {
        let el = El::Chapter(self.tr(title.0, title.1), self.tr(short.0, short.1));
        self.push(el);
    }

    fn head(&mut self, en: &str, hun: &str) {
        let el = El::Head(self.tr(en, hun));
        self.push(el);
    }

    fn text(&mut self, en: &str, hun: &str) {
        let el = El::Text(self.tr(en, hun));
        self.push(el);
    }

    fn bullet(&mut self, en: &str, hun: &str) {
        let el = El::Bullet(self.tr(en, hun));
        self.push(el);
    }

    /// The recipe making `id` (and it is not shown again with the rest).
    fn recipe(&mut self, id: ItemId) {
        self.shown.push(id);
        self.push(El::Recipe(id));
    }

    /// What `id` smelts into: in the first furnace that can, and how long it takes there.
    fn smelt(&mut self, id: ItemId) {
        let tier = smelt_tier(id);
        let at = format!("{}, {} {}", name(furnace_of_tier(tier) as ItemId), self.num(smelt_time(tier)), self.tr("s", "mp"));
        self.push(El::Smelt(id, at));
    }

    /// The text for something that may make you sick: how likely, or that it is safe.
    fn sickness(&self, c: &Consumable) -> String {
        match c.sick {
            Some(k) => format!("{}% {}", (k.chance * 100.0).round(), self.tr("chance of getting sick", "eséllyel rosszul leszel")),
            None => self.tr("safe", "biztonságos"),
        }
    }
}

/// Everything the book says, in one language (`keys`: the names of the inventory and the
/// reload keys).
pub(super) fn content(hu: bool, keys: &(String, String)) -> Vec<El> {
    let mut w = Writer { hu, keys, out: Vec::new(), shown: Vec::new() };
    start::write(&mut w);
    tools::write(&mut w);
    furnaces::write(&mut w);
    grill::write(&mut w);
    station::write(&mut w);
    guns::write(&mut w);
    ammo::write(&mut w);
    recipes::write(&mut w);
    w.out
}
