/// Upper bound on the `dt` [`FrameClock::tick`] returns, in seconds. After a stall (a
/// breakpoint, a flash write, a slow SD read) the game slows down for one frame instead of
/// jumping ahead.
pub const MAX_DT: f32 = 0.1;

/// Turns a monotonic microsecond counter into the per-frame `dt` for [`HandheldGame::update`],
/// for runtimes that have a timer but no frame clock.
///
/// [`HandheldGame::update`]: crate::HandheldGame::update
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameClock {
    last_us: Option<u64>,
}

impl FrameClock {
    /// A clock that has not ticked yet.
    pub const fn new() -> Self {
        Self { last_us: None }
    }

    /// Seconds since the previous tick, capped at [`MAX_DT`]. The first tick returns `0.0`.
    pub fn tick(&mut self, now_us: u64) -> f32 {
        let dt = self
            .last_us
            .map_or(0.0, |last| now_us.saturating_sub(last) as f32 / 1_000_000.0);
        self.last_us = Some(now_us);
        dt.min(MAX_DT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_tick_is_zero_then_elapsed_then_capped() {
        let mut clock = FrameClock::new();
        assert_eq!(clock.tick(5_000_000), 0.0);
        assert_eq!(clock.tick(5_016_000), 0.016);
        assert_eq!(clock.tick(9_000_000), MAX_DT);
        // A counter that goes backwards (wrap, reset) yields 0, not a negative step.
        assert_eq!(clock.tick(1), 0.0);
    }
}
