//! The time each frame moves everything on by, steadied to the display's
//! refresh.
//!
//! The frames reach the screen a whole number of refresh intervals apart,
//! but the time measured between them wobbles around that: the browser's
//! clock is coarse, and the system wakes the program a little early or
//! late. Moving the scene by the measured time would make it jitter even
//! at a perfect frame rate; moving it by the intervals the frames are
//! shown for keeps it smooth. So the clock learns the display's interval
//! from the frames and gives each frame a whole number of them: one, or
//! more after a frame was missed.
//!
//! Frames that fit no whole number pass through as measured, and a run of
//! them, as when the window moves to a display of another rate, makes the
//! clock learn the interval anew. What the measured and the given times
//! differ by is paid back a little each frame, so the clock keeps to the
//! wall clock in the long run.

/// Shortest refresh interval taken for a display's, in seconds (500 Hz).
const MIN_INTERVAL: f32 = 1.0 / 500.0;
/// Longest refresh interval taken for a display's, in seconds (20 Hz);
/// slower frames are the program's own.
const MAX_INTERVAL: f32 = 1.0 / 20.0;
/// How far a frame may be from a whole number of intervals, as a share of
/// one interval, and still count as that number.
const SNAP_TOLERANCE: f32 = 0.25;
/// Most whole intervals a frame is counted as; a longer frame is a stall,
/// and passes through as measured.
const MAX_SNAPPED_INTERVALS: f32 = 4.0;
/// Share of the gap between a fitting frame's time per interval and the
/// learned interval that the frame closes.
const INTERVAL_SMOOTHING: f32 = 0.02;
/// Frames in a row that fit no whole number of intervals before the clock
/// learns the interval anew.
const RELEARN_FRAMES: u32 = 12;
/// Share of the time owed that each frame pays back.
const REPAY_SHARE: f32 = 0.02;
/// Most a frame is lengthened or shortened by paying back, as a share of
/// its time, too little to see.
const MAX_REPAY_SHARE: f32 = 0.05;
/// Most time owed, in intervals; the rest of a longer debt is forgotten,
/// so a stall is not caught up.
const MAX_OWED_INTERVALS: f32 = 2.0;

/// Steadies the measured frame times (see the module documentation).
pub struct FrameClock {
    /// The display's refresh interval as learned, in seconds, once a
    /// frame has given a likely one.
    interval: Option<f32>,
    /// Frames in a row that fit no whole number of intervals.
    misses: u32,
    /// Measured time not yet given out, in seconds; negative when more
    /// was given out than measured.
    owed: f32,
    /// All the time given out so far, in seconds.
    seconds: f64,
}

impl FrameClock {
    /// A clock that has seen no frame.
    pub fn new() -> Self {
        Self {
            interval: None,
            misses: 0,
            owed: 0.0,
            seconds: 0.0,
        }
    }

    /// Takes a frame measured at `measured` seconds since the last, and
    /// returns the time it moves everything on by.
    ///
    /// A measurement that is not a finite, positive time counts as none.
    pub fn tick(&mut self, measured: f32) -> f32 {
        let measured = if measured.is_finite() {
            measured.max(0.0)
        } else {
            0.0
        };
        let steady = self.steady(measured);
        let owed_limit = self
            .interval
            .map_or(0.0, |interval| MAX_OWED_INTERVALS * interval);
        self.owed = (self.owed + measured - steady).clamp(-owed_limit, owed_limit);
        let repay_limit = MAX_REPAY_SHARE * steady;
        let repaid = (self.owed * REPAY_SHARE).clamp(-repay_limit, repay_limit);
        self.owed -= repaid;
        let given = steady + repaid;
        self.seconds += f64::from(given);
        given
    }

    /// All the time given out so far, in seconds: a clock that runs as
    /// smoothly as the frames are shown.
    pub fn seconds(&self) -> f64 {
        self.seconds
    }

    /// The display's refresh interval as learned, in seconds, once the
    /// frames have shown a likely one.
    pub fn interval(&self) -> Option<f32> {
        self.interval
    }

    /// The whole number of intervals a frame measured at `measured`
    /// seconds was shown for, or the measured time where it fits none,
    /// learning the interval as it goes.
    fn steady(&mut self, measured: f32) -> f32 {
        let likely = (MIN_INTERVAL..=MAX_INTERVAL).contains(&measured);
        let Some(interval) = self.interval else {
            self.interval = likely.then_some(measured);
            return measured;
        };
        let intervals = (measured / interval).round();
        let fits = (1.0..=MAX_SNAPPED_INTERVALS).contains(&intervals)
            && (measured - intervals * interval).abs() <= SNAP_TOLERANCE * interval;
        if fits {
            self.misses = 0;
            let learned = interval + (measured / intervals - interval) * INTERVAL_SMOOTHING;
            self.interval = Some(learned);
            return intervals * learned;
        }
        self.misses += 1;
        if self.misses >= RELEARN_FRAMES {
            self.misses = 0;
            self.interval = likely.then_some(measured);
        }
        measured
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/frame_clock.rs"]
mod tests;
