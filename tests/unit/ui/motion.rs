//! Tests for `src/ui/motion.rs`: the curves start, end and move as meant.

use super::*;

/// One frame at 60 frames per second.
const FRAME: f32 = 1.0 / 60.0;

#[test]
fn smooth_runs_from_rest_to_rest() {
    assert_eq!(smooth(-1.0), 0.0);
    assert_eq!(smooth(0.0), 0.0);
    assert_eq!(smooth(0.5), 0.5);
    assert_eq!(smooth(1.0), 1.0);
    assert_eq!(smooth(2.0), 1.0);
    assert!(smooth(0.1) < 0.1 && smooth(0.9) > 0.9);
}

#[test]
fn bounce_ends_at_rest_after_passing_it_slightly() {
    assert_eq!(bounce(0.0, 1.0), 0.0);
    assert_eq!(bounce(1.0, 1.0), 1.0);
    let peak = (0..=100)
        .map(|step| bounce(step as f32 / 100.0, 1.0))
        .fold(0.0, f32::max);
    assert!(peak > 1.02 && peak < 1.05, "{peak}");
}

#[test]
fn advance_crosses_in_its_time_and_stops_at_the_ends() {
    const SECONDS: f32 = 0.3;
    let mut progress = 0.0;
    let mut frames = 0;
    while progress < 1.0 {
        progress = advance(progress, true, FRAME, SECONDS);
        frames += 1;
    }
    assert_eq!(frames, (SECONDS / FRAME).ceil() as usize);
    assert_eq!(advance(1.0, true, FRAME, SECONDS), 1.0);
    assert_eq!(advance(0.0, false, FRAME, SECONDS), 0.0);
}

#[test]
fn approach_closes_the_gap_the_same_at_any_frame_rate() {
    const SECONDS: f32 = 0.1;
    let mut fine = 0.0;
    for _ in 0..4 {
        fine = approach(fine, 1.0, FRAME / 4.0, SECONDS);
    }
    let coarse = approach(0.0, 1.0, FRAME, SECONDS);
    assert!((fine - coarse).abs() < 1e-5);
    assert!((approach(0.0, 1.0, SECONDS, SECONDS) - (1.0 - (-1.0f32).exp())).abs() < 1e-6);
}

#[test]
fn faded_keeps_the_color_and_scales_its_opacity() {
    let color = Color::new(0.2, 0.4, 0.6, 0.8);
    let half = faded(color, 0.5);
    assert_eq!((half.r, half.g, half.b), (0.2, 0.4, 0.6));
    assert!((half.a - 0.4).abs() < 1e-6);
    assert_eq!(faded(color, 2.0).a, 0.8);
}
