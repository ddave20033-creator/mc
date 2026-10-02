//! The minigames: small games of their own, apart from single player and multiplayer, chosen
//! on the minigames screen (`ui::screens::minigames`).
//!
//! Each minigame lives in its own folder here (`soccer/`, ...) with everything it needs: its
//! card on the minigames screen and, later, its game. The screen only knows the list below.
//!
//! Adding one:
//! 1. a folder `src/minigames/<name>/` with a `mod.rs` holding its `pub const INFO: Minigame`
//!    and its card's picture (`card.rs`);
//! 2. `pub mod <name>;` and `<name>::INFO` in `ALL` below;
//! 3. its name in `app/lang.rs` (`minigames.<name>`, English and Hungarian).

pub mod soccer;

use crate::ui::{Color, Ui};

/// What the minigames screen needs to know about a minigame.
pub struct Minigame {
    /// Its name's key in `app/lang.rs`.
    pub name: &'static str,
    /// The color of its card's border when hovered and of the line under its name.
    pub accent: Color,
    /// Paints the card's picture in `[x, y, w, h]` (the card's name and frame are drawn
    /// around it by the screen); `hover` goes 0..1 while the card is pointed at.
    pub card: fn(ui: &mut Ui, rect: [f32; 4], hover: f32),
}

/// Every minigame, in the order of their cards on the screen.
pub const ALL: &[Minigame] = &[soccer::INFO];
