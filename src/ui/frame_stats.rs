//! Frame rate and physics time, averaged over short windows so they can be
//! read on a machine whose frame rate swings, and when the "physics time
//! limited" warning shows. Both are drawn in the corner of the scene, so
//! they show in every panel tab and with the panel hidden.

use std::collections::VecDeque;

/// Seconds of frames averaged into each reading.
const WINDOW_SECONDS: f64 = 0.5;
/// Seconds the warning stays up after the last frame that dropped time.
pub const LIMITED_HOLD_SECONDS: f64 = 3.0;
/// The warning shows only after this many frames dropped time within
/// `LIMITED_WINDOW_SECONDS`. One slow frame (a browser hiccup, a tab
/// switch) drops time once and then the physics keeps up; a computer that
/// cannot keep up drops time on frame after frame.
const LIMITED_MIN_FRAMES: usize = 3;
const LIMITED_WINDOW_SECONDS: f64 = 1.0;
const MILLISECONDS_PER_SECOND: f64 = 1_000.0;

pub const LIMITED_TEXT: &str = "Physics time limited: computer too slow";

/// Averages over one finished window.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Reading {
    fps: f64,
    /// Mean physics time per frame, in milliseconds.
    physics_ms: f64,
    /// Longest physics time of any frame in the window, in milliseconds.
    worst_ms: f64,
}

pub struct FrameStats {
    window_start: Option<f64>,
    frames: u32,
    physics_seconds: f64,
    worst_seconds: f64,
    reading: Option<Reading>,
    /// Times of the recent frames that dropped time, within the window.
    recent_drops: VecDeque<f64>,
    limited_until: f64,
}

impl FrameStats {
    pub fn new() -> Self {
        Self {
            window_start: None,
            frames: 0,
            physics_seconds: 0.0,
            worst_seconds: 0.0,
            reading: None,
            recent_drops: VecDeque::with_capacity(LIMITED_MIN_FRAMES),
            limited_until: f64::NEG_INFINITY,
        }
    }

    /// Records one frame that ended at wall time `now` (seconds), spent
    /// `physics_seconds` stepping the physics, and dropped time if `limited`.
    pub fn record(&mut self, now: f64, physics_seconds: f64, limited: bool) {
        if limited {
            self.record_drop(now);
        }
        let Some(start) = self.window_start else {
            // The first frame only starts the clock.
            self.window_start = Some(now);
            return;
        };
        self.frames += 1;
        self.physics_seconds += physics_seconds;
        self.worst_seconds = self.worst_seconds.max(physics_seconds);
        let elapsed = now - start;
        if elapsed >= WINDOW_SECONDS {
            let frames = f64::from(self.frames);
            self.reading = Some(Reading {
                fps: frames / elapsed,
                physics_ms: self.physics_seconds / frames * MILLISECONDS_PER_SECOND,
                worst_ms: self.worst_seconds * MILLISECONDS_PER_SECOND,
            });
            self.window_start = Some(now);
            self.frames = 0;
            self.physics_seconds = 0.0;
            self.worst_seconds = 0.0;
        }
    }

    /// The frame rate and physics time of the last finished window, once
    /// there is one.
    pub fn line(&self) -> Option<String> {
        self.reading.map(|reading| {
            format!(
                "{:.0} fps | physics {:.1} ms per frame, worst {:.1} ms",
                reading.fps, reading.physics_ms, reading.worst_ms
            )
        })
    }

    /// Notes a frame that dropped time. The warning starts once enough
    /// drops fall within the window; while it is up, every drop restarts
    /// the hold, so a swinging frame rate keeps it up steadily instead of
    /// flashing it.
    fn record_drop(&mut self, now: f64) {
        while self
            .recent_drops
            .front()
            .is_some_and(|&time| now - time > LIMITED_WINDOW_SECONDS)
        {
            self.recent_drops.pop_front();
        }
        if self.recent_drops.len() == LIMITED_MIN_FRAMES {
            self.recent_drops.pop_front();
        }
        self.recent_drops.push_back(now);
        if self.recent_drops.len() >= LIMITED_MIN_FRAMES || self.limited(now) {
            self.limited_until = now + LIMITED_HOLD_SECONDS;
        }
    }

    /// Whether the warning is up at wall time `now`.
    pub fn limited(&self, now: f64) -> bool {
        now < self.limited_until
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/frame_stats.rs"]
mod tests;
