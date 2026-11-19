//! Physics time budget tests for `src/ui/physics_budget.rs`.

use super::*;

/// A 60 Hz display's refresh interval.
const AT_60_HZ: f32 = 1.0 / 60.0;
/// Frames the budget is given to measure the cost.
const MEASURED_FRAMES: usize = 60;

/// Records `MEASURED_FRAMES` frames of a 60 Hz frame's model time, each
/// taking `seconds`.
fn measure(budget: &mut PhysicsBudget, seconds: f64) {
    for _ in 0..MEASURED_FRAMES {
        budget.record(AT_60_HZ, seconds);
    }
}

#[test]
fn nothing_measured_limits_nothing() {
    let mut budget = PhysicsBudget::new();
    assert_eq!(budget.allowance(AT_60_HZ), f32::INFINITY);
    // Paused frames and nonsense say nothing.
    budget.record(0.0, 0.002);
    budget.record(f32::NAN, 0.002);
    budget.record(AT_60_HZ, f64::NAN);
    assert_eq!(budget.allowance(AT_60_HZ), f32::INFINITY);
}

#[test]
fn a_fast_computer_runs_every_frame_in_full() {
    let mut budget = PhysicsBudget::new();
    // 2 ms of physics a frame.
    measure(&mut budget, 0.002);
    assert!(budget.allowance(AT_60_HZ) > 4.0 * AT_60_HZ);
}

#[test]
fn a_slow_computer_runs_what_fits_in_its_share() {
    let mut budget = PhysicsBudget::new();
    // 12 ms of physics a frame, far over half a 60 Hz frame.
    let seconds = 0.012;
    measure(&mut budget, seconds);
    let allowed = budget.allowance(AT_60_HZ);
    assert!(allowed < AT_60_HZ);
    let expected_seconds = PHYSICS_SHARE * f64::from(AT_60_HZ);
    let predicted = f64::from(allowed) / f64::from(AT_60_HZ) * seconds;
    assert!((predicted - expected_seconds).abs() < 1e-6, "{predicted}");
}

#[test]
fn one_slow_frame_barely_moves_the_cost() {
    let mut budget = PhysicsBudget::new();
    measure(&mut budget, 0.002);
    let before = budget.allowance(AT_60_HZ);
    budget.record(AT_60_HZ, 0.050);
    let after = budget.allowance(AT_60_HZ);
    // Still more than a frame: the next frames run in full.
    assert!(
        after > 0.25 * before && after > AT_60_HZ,
        "{before} {after}"
    );
}
