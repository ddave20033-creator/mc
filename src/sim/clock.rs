//! The game's clock: its logic (moving, falling, later mobs and machines) runs in fixed steps,
//! ticks, 20 a second whatever the frame rate, so nothing goes faster or jumps higher on a
//! faster computer. Drawing happens as often as it can, showing things between two ticks.

use std::time::{Duration, Instant};

pub const TICKS_PER_SECOND: u32 = 20;
pub const TICK: Duration = Duration::from_millis(1000 / TICKS_PER_SECOND as u64);
/// A tick in seconds (for speeds given a second).
pub const TICK_SECS: f32 = 1.0 / TICKS_PER_SECOND as f32;

/// Ticks run at once at most after a stall (dragging the window, a slow frame): past that the
/// game goes on from where it is instead of racing to catch up.
const MOST_AT_ONCE: u32 = 10;

pub struct Clock {
    /// When the next tick is due.
    next: Instant,
    /// Ticks run so far.
    pub tick: u64,
}

impl Clock {
    pub fn new(now: Instant) -> Clock {
        Clock { next: now + TICK, tick: 0 }
    }

    /// How many ticks are due by `now` (they are counted as run).
    pub fn due(&mut self, now: Instant) -> u32 {
        let mut n = 0;
        while now >= self.next && n < MOST_AT_ONCE {
            self.next += TICK;
            self.tick += 1;
            n += 1;
        }
        if now >= self.next {
            self.next = now + TICK;
        }
        n
    }

    /// Where `now` is between the last tick (0) and the next (1): what is drawn is that far
    /// from the state after the last tick toward the one after the next.
    pub fn between(&self, now: Instant) -> f32 {
        let left = self.next.saturating_duration_since(now).as_secs_f32();
        (1.0 - left / TICK_SECS).clamp(0.0, 1.0)
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twenty_ticks_a_second_at_any_frame_rate() {
        for fps in [7u32, 20, 60, 144, 1000] {
            let start = Instant::now();
            let mut clock = Clock::new(start);
            let mut ticks = 0;
            for frame in 1..=fps {
                ticks += clock.due(start + Duration::from_secs(1) * frame / fps);
            }
            assert_eq!(ticks, 20, "at {fps} frames a second");
        }
    }

    #[test]
    fn a_long_stall_is_not_raced_through() {
        let start = Instant::now();
        let mut clock = Clock::new(start);
        assert_eq!(clock.due(start + Duration::from_secs(5)), MOST_AT_ONCE);
        // Then on as usual from there.
        assert_eq!(clock.due(start + Duration::from_secs(5) + TICK), 1);
    }

    #[test]
    fn drawing_goes_between_ticks() {
        let start = Instant::now();
        let clock = Clock::new(start);
        assert_eq!(clock.between(start), 0.0);
        assert!((clock.between(start + TICK / 2) - 0.5).abs() < 0.01);
        assert_eq!(clock.between(start + TICK * 2), 1.0);
    }
}
