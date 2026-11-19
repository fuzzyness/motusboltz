//! Frame rate, the work of each frame and its physics, and when the
//! "physics time limited" warning shows.
//!
//! The readings are averaged over short windows so they can be read on a
//! machine whose frame rate swings. Both are drawn in the corner of the
//! scene, so they show in every panel tab and with the panel hidden.
//!
//! The work is the time the program spends making a frame, before it
//! hands the frame over and waits for the next. A frame rate below the
//! display's with little work a frame means the graphics card is what
//! holds it back; with the work near the whole frame, the processor.

use std::collections::VecDeque;

/// Seconds of frames averaged into each reading.
const WINDOW_SECONDS: f64 = 0.5;
/// Seconds the warning stays up after the last frame that dropped time.
const LIMITED_HOLD_SECONDS: f64 = 3.0;
/// Frames that must drop time within `LIMITED_WINDOW_SECONDS` before the
/// warning shows.
///
/// One slow frame (a browser hiccup, a tab switch) drops time once and then
/// the physics keeps up; a computer that cannot keep up drops time on frame
/// after frame.
const LIMITED_MIN_FRAMES: usize = 3;
/// Seconds within which `LIMITED_MIN_FRAMES` drops raise the warning.
const LIMITED_WINDOW_SECONDS: f64 = 1.0;
/// Milliseconds in a second.
const MILLISECONDS_PER_SECOND: f64 = 1_000.0;

/// Warning shown while the physics cannot keep up.
pub const LIMITED_TEXT: &str = "Physics time limited: computer too slow";

/// Averages over one finished window, shown in the diagnostics build.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(not(any(test, feature = "diagnostics")), allow(dead_code))]
struct Reading {
    /// Frames per second.
    fps: f64,
    /// Mean time spent making a frame, in milliseconds.
    work_ms: f64,
    /// Mean physics time per frame, in milliseconds.
    physics_ms: f64,
    /// Longest physics time of any frame in the window, in milliseconds.
    worst_ms: f64,
}

/// The frame-rate readout and the warning.
pub struct FrameStats {
    /// Wall time the current window started, once a frame has been seen.
    window_start: Option<f64>,
    /// Frames in the current window.
    frames: u32,
    /// Physics time in the current window, in seconds.
    physics_seconds: f64,
    /// Longest physics time of a frame in the current window, in seconds.
    worst_seconds: f64,
    /// Time spent making the frames in the current window, in seconds.
    work_seconds: f64,
    /// Frames whose work is counted in `work_seconds`.
    work_frames: u32,
    /// Averages over the last finished window.
    reading: Option<Reading>,
    /// Times of the last `LIMITED_MIN_FRAMES` frames that dropped time.
    recent_drops: VecDeque<f64>,
    /// Wall time until which the warning stays up.
    limited_until: f64,
}

impl FrameStats {
    /// A readout with nothing recorded.
    pub fn new() -> Self {
        Self {
            window_start: None,
            frames: 0,
            physics_seconds: 0.0,
            worst_seconds: 0.0,
            work_seconds: 0.0,
            work_frames: 0,
            reading: None,
            recent_drops: VecDeque::with_capacity(LIMITED_MIN_FRAMES),
            limited_until: f64::NEG_INFINITY,
        }
    }

    /// Records one frame that ended at wall time `now`, spent
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
            let work_frames = f64::from(self.work_frames.max(1));
            self.reading = Some(Reading {
                fps: frames / elapsed,
                work_ms: self.work_seconds / work_frames * MILLISECONDS_PER_SECOND,
                physics_ms: self.physics_seconds / frames * MILLISECONDS_PER_SECOND,
                worst_ms: self.worst_seconds * MILLISECONDS_PER_SECOND,
            });
            self.window_start = Some(now);
            self.frames = 0;
            self.physics_seconds = 0.0;
            self.worst_seconds = 0.0;
            self.work_seconds = 0.0;
            self.work_frames = 0;
        }
    }

    /// Records that making a frame took `seconds`, from reading the input
    /// to handing the frame over.
    #[cfg(any(test, feature = "diagnostics"))]
    pub fn record_work(&mut self, seconds: f64) {
        self.work_seconds += seconds;
        self.work_frames += 1;
    }

    /// The frame rate, work and physics time of the last finished window,
    /// as labeled rows, once there is one.
    #[cfg(any(test, feature = "diagnostics"))]
    pub fn rows(&self) -> Option<[(&'static str, String); 3]> {
        self.reading.map(|reading| {
            [
                ("Frame rate", format!("{:.0} fps", reading.fps)),
                ("Work a frame", format!("{:.1} ms", reading.work_ms)),
                (
                    "Physics a frame",
                    format!(
                        "{:.1} ms (worst {:.1})",
                        reading.physics_ms, reading.worst_ms
                    ),
                ),
            ]
        })
    }

    /// Notes a frame that dropped time at wall time `now`.
    ///
    /// The warning starts once enough drops fall within the window. While
    /// it is up, every drop restarts the hold, so a swinging frame rate
    /// keeps it up steadily instead of flashing it.
    fn record_drop(&mut self, now: f64) {
        if self.recent_drops.len() == LIMITED_MIN_FRAMES {
            self.recent_drops.pop_front();
        }
        self.recent_drops.push_back(now);
        let repeated = self.recent_drops.len() == LIMITED_MIN_FRAMES
            && now - self.recent_drops[0] <= LIMITED_WINDOW_SECONDS;
        if repeated || self.limited(now) {
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
