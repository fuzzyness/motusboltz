//! Frame-time steadying tests for `src/ui/frame_clock.rs`.

use super::*;

/// A 60 Hz display's refresh interval.
const AT_60_HZ: f32 = 1.0 / 60.0;
/// A 144 Hz display's refresh interval.
const AT_144_HZ: f32 = 1.0 / 144.0;
/// How far a measured frame wobbles each way, in seconds, as a browser's
/// coarse clock makes it.
const WOBBLE: f32 = 0.0009;
/// Frames the clock is given to learn a display's interval.
const LEARNING_FRAMES: usize = 60;
/// Frames each check below looks at.
const CHECKED_FRAMES: usize = 120;

/// The `index`th of a run of frames measured about `interval` apart,
/// wobbling up to `WOBBLE` either way in a fixed pattern with no period
/// of its own.
fn wobbly(interval: f32, index: usize) -> f32 {
    const PATTERN: [f32; 7] = [0.0, 1.0, -1.0, 0.6, -0.4, -0.9, 0.7];
    interval + WOBBLE * PATTERN[index % PATTERN.len()]
}

/// Ticks `count` frames of `measured` (by index), returning the times
/// given.
fn run(clock: &mut FrameClock, count: usize, measured: impl Fn(usize) -> f32) -> Vec<f32> {
    (0..count)
        .map(|index| clock.tick(measured(index)))
        .collect()
}

/// The difference between the longest and shortest of `times`.
fn spread(times: &[f32]) -> f32 {
    let longest = times.iter().copied().fold(f32::MIN, f32::max);
    let shortest = times.iter().copied().fold(f32::MAX, f32::min);
    longest - shortest
}

#[test]
fn wobbling_frames_come_out_even() {
    let mut clock = FrameClock::new();
    run(&mut clock, LEARNING_FRAMES, |index| wobbly(AT_60_HZ, index));
    let given = run(&mut clock, CHECKED_FRAMES, |index| wobbly(AT_60_HZ, index));
    // In: almost 2 ms between the shortest and longest. Out: a tenth of
    // that at most.
    assert!(spread(&given) < 0.1 * 2.0 * WOBBLE, "{given:?}");
    for time in given {
        assert!((time - AT_60_HZ).abs() < 0.02 * AT_60_HZ, "{time}");
    }
}

#[test]
fn a_missed_refresh_gives_two_intervals() {
    let mut clock = FrameClock::new();
    run(&mut clock, LEARNING_FRAMES, |_| AT_60_HZ);
    let missed = clock.tick(2.0 * AT_60_HZ + 0.1 * WOBBLE);
    assert!(
        (missed - 2.0 * AT_60_HZ).abs() < 0.01 * AT_60_HZ,
        "{missed}"
    );
    let next = clock.tick(AT_60_HZ);
    assert!((next - AT_60_HZ).abs() < 0.01 * AT_60_HZ, "{next}");
}

#[test]
fn the_clock_keeps_to_the_wall_clock() {
    const FRAMES: usize = 600;
    let mut clock = FrameClock::new();
    let measured: Vec<f32> = (0..FRAMES)
        .map(|index| {
            // Now and then a frame is missed.
            let missed = if index % 97 == 50 { AT_60_HZ } else { 0.0 };
            wobbly(AT_60_HZ, index) + missed
        })
        .collect();
    let given = run(&mut clock, FRAMES, |index| measured[index]);
    let wall: f64 = measured.iter().map(|&time| f64::from(time)).sum();
    let steady: f64 = given.iter().map(|&time| f64::from(time)).sum();
    assert!((steady - clock.seconds()).abs() < 1e-9);
    assert!(
        (wall - steady).abs() < f64::from(AT_60_HZ),
        "{wall} {steady}"
    );
}

#[test]
fn a_display_of_another_rate_is_learned_anew() {
    let mut clock = FrameClock::new();
    run(&mut clock, LEARNING_FRAMES, |index| wobbly(AT_60_HZ, index));
    run(&mut clock, LEARNING_FRAMES, |_| AT_144_HZ);
    let given = run(&mut clock, CHECKED_FRAMES, |_| AT_144_HZ);
    for time in given {
        assert!((time - AT_144_HZ).abs() < 0.01 * AT_144_HZ, "{time}");
    }
}

#[test]
fn a_stall_and_the_first_frame_pass_through() {
    const STALL: f32 = 0.5;
    let mut clock = FrameClock::new();
    assert_eq!(clock.tick(AT_60_HZ), AT_60_HZ);
    run(&mut clock, LEARNING_FRAMES, |_| AT_60_HZ);
    let stalled = clock.tick(STALL);
    // Nearly as measured: at most a sliver of a frame is paid back.
    assert!(
        (stalled - STALL).abs() < MAX_REPAY_SHARE * STALL,
        "{stalled}"
    );
}

#[test]
fn a_frame_measured_as_nothing_moves_nothing() {
    let mut clock = FrameClock::new();
    assert_eq!(clock.tick(0.0), 0.0);
    assert_eq!(clock.tick(f32::NAN), 0.0);
    assert_eq!(clock.tick(-AT_60_HZ), 0.0);
    assert_eq!(clock.seconds(), 0.0);
}
