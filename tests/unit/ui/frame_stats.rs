//! Frame-rate readout and warning-hold tests for `src/ui/frame_stats.rs`.

use super::*;

const FRAME: f64 = 1.0 / 60.0;

/// Records `count` frames from `start`, one every `FRAME`, each spending
/// `physics` seconds on the physics. Returns the time of the last frame.
fn frames(stats: &mut FrameStats, start: f64, count: usize, physics: f64) -> f64 {
    let mut now = start;
    for _ in 0..count {
        now += FRAME;
        stats.record(now, physics, false);
    }
    now
}

#[test]
fn readings_average_whole_windows() {
    let mut stats = FrameStats::new();
    stats.record(0.0, 0.004, false);
    let now = frames(&mut stats, 0.0, 20, 0.004);
    assert_eq!(stats.line(), None, "no window has finished yet");

    frames(&mut stats, now, 9, 0.004);
    // The thirtieth frame closes the window at exactly half a second.
    stats.record(0.5, 0.010, false);
    let reading = stats.reading.expect("half a second of frames");
    assert!((reading.fps - 60.0).abs() < 0.5, "{reading:?}");
    assert!((reading.physics_ms - 4.2).abs() < 0.01, "{reading:?}");
    assert!((reading.worst_ms - 10.0).abs() < 1e-9, "{reading:?}");
    assert_eq!(
        stats.line().as_deref(),
        Some("60 fps | physics 4.2 ms per frame, worst 10.0 ms")
    );
}

#[test]
fn a_slow_window_reports_its_own_rate() {
    let mut stats = FrameStats::new();
    stats.record(0.0, 0.0, false);
    for frame in 1..=10 {
        stats.record(f64::from(frame) * 0.05, 0.030, false);
    }
    let reading = stats.reading.unwrap();
    assert!((reading.fps - 20.0).abs() < 0.5, "{reading:?}");
    assert!((reading.physics_ms - 30.0).abs() < 1e-6, "{reading:?}");
}

#[test]
fn one_dropped_frame_does_not_raise_the_warning() {
    let mut stats = FrameStats::new();
    stats.record(1.0, 0.0, true);
    assert!(!stats.limited(1.0));
    // A second hiccup much later is still a one-off.
    stats.record(5.0, 0.0, true);
    stats.record(5.5, 0.0, true);
    assert!(!stats.limited(5.5));
}

#[test]
fn repeated_drops_raise_the_warning_and_it_holds() {
    let mut stats = FrameStats::new();
    assert!(!stats.limited(0.0));
    for frame in 0..LIMITED_MIN_FRAMES {
        assert!(!stats.limited(1.0 + frame as f64 * FRAME));
        stats.record(1.0 + frame as f64 * FRAME, 0.0, true);
    }
    let last = 1.0 + (LIMITED_MIN_FRAMES - 1) as f64 * FRAME;
    assert!(stats.limited(last));
    assert!(stats.limited(last + LIMITED_HOLD_SECONDS - 0.01));
    assert!(!stats.limited(last + LIMITED_HOLD_SECONDS));

    // While the warning is up, each dropped frame restarts the hold, so a
    // swinging frame rate keeps it up steadily instead of flashing it.
    let mut stats = FrameStats::new();
    for frame in 0..LIMITED_MIN_FRAMES {
        stats.record(1.0 + frame as f64 * FRAME, 0.0, true);
    }
    stats.record(3.0, 0.0, true);
    stats.record(3.5, 0.0, false);
    assert!(stats.limited(3.0 + LIMITED_HOLD_SECONDS - 0.01));
    assert!(!stats.limited(3.0 + LIMITED_HOLD_SECONDS));
}

/// Drops spread out more thinly than the window allows never add up.
#[test]
fn drops_must_fall_within_the_window() {
    let mut stats = FrameStats::new();
    let spacing = LIMITED_WINDOW_SECONDS / (LIMITED_MIN_FRAMES - 1) as f64 + 0.01;
    for drop in 0..10 {
        stats.record(drop as f64 * spacing, 0.0, true);
        assert!(!stats.limited(drop as f64 * spacing));
    }
}

#[test]
fn readout_text_is_ascii() {
    let mut stats = FrameStats::new();
    stats.record(0.0, 0.0, false);
    frames(&mut stats, 0.0, 40, 0.002);
    assert!(stats.line().unwrap().is_ascii());
    assert!(LIMITED_TEXT.is_ascii());
}
