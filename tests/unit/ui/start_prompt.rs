//! Tests for `src/ui/start_prompt.rs`: when the Start button shows.

use super::*;

#[test]
fn shown_at_rest_and_hidden_once_the_engine_runs() {
    let mut prompt = StartPrompt::new();
    assert!(prompt.shown());
    prompt.update(0.0, 0.0);
    assert!(prompt.shown());
    prompt.update(1.0, 0.1);
    assert!(!prompt.shown());
}

#[test]
fn returns_only_after_the_engine_stays_stopped() {
    let mut prompt = StartPrompt::new();
    prompt.update(1.0, 0.0);
    // Slowing through the middle band keeps it hidden.
    prompt.update(0.2, 1.0);
    assert!(!prompt.shown());
    prompt.update(0.1, 2.0);
    assert!(!prompt.shown(), "too soon");
    prompt.update(0.1, 2.0 + SHOW_AFTER_SECONDS / 2.0);
    assert!(!prompt.shown(), "still too soon");
    prompt.update(-0.1, 2.0 + SHOW_AFTER_SECONDS);
    assert!(prompt.shown(), "stopped long enough, even rocking backward");
    // Back in the middle band it stays shown until the engine runs again.
    prompt.update(0.2, 10.0);
    assert!(prompt.shown());
    prompt.update(RUNNING_FRACTION, 11.0);
    assert!(!prompt.shown());
}

#[test]
fn a_brief_dip_does_not_bring_it_back() {
    let mut prompt = StartPrompt::new();
    prompt.update(1.0, 0.0);
    prompt.update(0.1, 1.0);
    prompt.update(0.5, 1.5);
    prompt.update(0.1, 2.0);
    prompt.update(0.1, 2.0 + SHOW_AFTER_SECONDS / 2.0);
    assert!(!prompt.shown(), "the timer restarts after the dip");
}

#[test]
fn a_refill_shows_it_at_once() {
    let mut prompt = StartPrompt::new();
    prompt.update(1.0, 0.0);
    prompt.update(0.0, 0.1);
    assert!(prompt.shown());
}

/// Against the real engine: Start brings the shaft to its starting speed,
/// which hides the button, and a fresh fill shows it again.
#[test]
fn start_hides_the_button_and_a_refill_brings_it_back() {
    use crate::sim::engine::ConnectedChambers;
    use crate::sim::geometry::DEFAULT_GEOMETRY;
    use crate::sim::units::DEFAULT_CONNECTED_MARKERS;
    const SEED: u64 = 42;
    let mut engine =
        ConnectedChambers::with_geometry_seed(DEFAULT_CONNECTED_MARKERS, DEFAULT_GEOMETRY, SEED);
    let mut prompt = StartPrompt::new();
    assert_eq!(engine.power_pistons().start_speed_fraction(), 0.0);
    engine.start_engine();
    let fraction = engine.power_pistons().start_speed_fraction();
    assert!((fraction - 1.0).abs() < 1e-6, "{fraction}");
    prompt.update(fraction, 0.0);
    assert!(!prompt.shown());
    engine.reset();
    prompt.update(engine.power_pistons().start_speed_fraction(), 1.0);
    assert!(prompt.shown());
}
