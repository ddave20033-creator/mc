//! LAN multiplayer networking: the message protocol, TCP connections, the host's server
//! and LAN discovery.
//!
//! The host runs the world (blocks, fluids, mobs, dropped items, furnaces) and sends what
//! changes; players generate the untouched terrain themselves from the seed. Every message
//! is a little-endian binary frame: a u32 length, then a tag byte and the fields.
//!
//! Discovery works like Minecraft's "Open to LAN": the host announces its game on a UDP
//! broadcast every 1.5 seconds and the multiplayer screen lists what it hears.

mod codec;
mod delta;
mod conn;
mod msg;

pub use conn::{local_ip, Conn, Finder, Frame, Server, DEFAULT_PORT};
pub use delta::EntitySync;
pub use msg::{ItemNet, MobNet, Msg, PlayerState, Pose, NO_BLOCK};

/// Bumped whenever the messages change; host and players must match.
pub const PROTOCOL: u16 = 37;

/// `Pose::book`: the book is held open; the last page turn went back; the number of page
/// turns so far (low 6 bits, wrapping), so the others turn a page when it changes.
/// `Pose::book_page`: the book is read in Hungarian (the rest is the spread).
pub mod book {
    pub const OPEN: u8 = 0x80;
    pub const BACK: u8 = 0x40;
    pub const TURNS: u8 = 0x3f;
    pub const HUNGARIAN: u8 = 0x80;
}

/// `Pose::status`: typing in the chat, in the pause menu, away (the game window is not in
/// front) or looking at an item screen.
pub mod status {
    pub const NONE: u8 = 0;
    pub const TYPING: u8 = 1;
    pub const MENU: u8 = 2;
    pub const AFK: u8 = 3;
    pub const INVENTORY: u8 = 4;
    pub const READING: u8 = 5;
}

pub mod pose_flags {
    pub const BURNING: u8 = 1;
    pub const BLOCKING: u8 = 2;
    pub const HURT: u8 = 4;
    pub const DEAD: u8 = 8;
    pub const CREATIVE: u8 = 16;
    /// Lying in a bed (the pose's position is on top of the bed's head half).
    pub const SLEEPING: u8 = 32;
    /// Aiming a gun down its sights.
    pub const AIMING: u8 = 64;
    /// Holding the guide book turned around to show it.
    pub const SHOWING: u8 = 128;
}

/// `PlayerState::mode` (a saved state's old creative flag reads as 0 or 1).
pub mod mode {
    pub const SURVIVAL: u8 = 0;
    pub const CREATIVE: u8 = 1;
    pub const SPECTATOR: u8 = 2;
}

/// What hurt a player (`Msg::Hurt`, `Msg::AttackPlayer`): armor takes some kinds better.
pub mod hurt {
    pub const MELEE: u8 = 0;
    pub const BULLET: u8 = 1;
    pub const BLAST: u8 = 2;
    /// A wolf's bite (the host's wolves bite LAN players).
    pub const WOLF: u8 = 3;
}

/// What a `Msg::Fx` shows (and sounds) at its place: what the server does that everyone
/// should see, but what is only a sight.
pub mod fx {
    /// A wolf took a bone: crumbs of it, and it barks.
    pub const WOLF_TAKES: u8 = 0;
    /// A wolf would not have the bone: crumbs, and a puff of smoke.
    pub const WOLF_REFUSES: u8 = 1;
    /// A puff of smoke (a mob gone, a dummy taken down).
    pub const POOF: u8 = 2;
}

/// Block entity kinds in `Msg::Container`.
pub mod container {
    pub const CHEST: u8 = 0;
    pub const TABLE: u8 = 1;
}
