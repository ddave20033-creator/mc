//! The game's simulation: it runs in fixed steps (`clock`), 20 ticks a second, whatever the
//! frame rate.

pub mod clock;
pub mod felling;
pub mod grenade;
pub mod rules;
pub mod server;

/// A day and a night, in seconds (Minecraft's 20 minutes).
pub const DAY_LENGTH: f32 = 1200.0;

/// Night, when beds can be slept in (Minecraft: ticks 12542..23459 of 24000).
pub fn is_night(time_of_day: f32) -> bool {
    (12542.0 / 24000.0..23459.0 / 24000.0).contains(&time_of_day)
}

/// The time of day `dt` seconds on (0..1, a day long).
pub fn advance_time(time_of_day: f32, dt: f32) -> f32 {
    (time_of_day + dt / DAY_LENGTH).fract()
}

/// A time of day as a command gives it: a name or Minecraft ticks.
pub fn parse_time(v: &str) -> Option<f32> {
    Some(match v {
        "day" => 1000.0,
        "noon" => 6000.0,
        "sunset" => 12000.0,
        "night" => 13000.0,
        "midnight" => 18000.0,
        "sunrise" => 23000.0,
        _ => v.parse::<f32>().ok()?,
    })
}
