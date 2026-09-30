//! The game's simulation: it runs in fixed steps (`clock`), 20 ticks a second, whatever the
//! frame rate.

pub mod clock;
pub mod rules;
pub mod server;

/// A day and a night, in seconds (Minecraft's 20 minutes).
pub const DAY_LENGTH: f32 = 1200.0;

/// Night, when beds can be slept in (Minecraft: ticks 12542..23459 of 24000).
pub fn is_night(time_of_day: f32) -> bool {
    (12542.0 / 24000.0..23459.0 / 24000.0).contains(&time_of_day)
}
