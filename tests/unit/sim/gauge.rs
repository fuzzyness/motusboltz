//! Tests for `src/sim/gauge.rs`: a gauge reads the mean push on its face.

use super::*;

/// One substep.
const DT: f64 = 1.0 / 240.0;
/// Area of the face in these tests.
const AREA: f64 = 2.0;

/// `gauge` after `seconds` of a steady `pressure` on the face.
fn steady(gauge: &mut PressureGauge, pressure: f64, seconds: f64) {
    for _ in 0..(seconds / DT).round() as usize {
        gauge.record(pressure * AREA * DT, DT);
    }
}

#[test]
fn a_steady_push_reads_as_its_pressure_once_watched_long_enough() {
    let mut gauge = PressureGauge::new();
    assert_eq!(gauge.pressure(AREA), None);
    steady(&mut gauge, 3.0, 0.25 * GAUGE_SECONDS);
    assert_eq!(gauge.pressure(AREA), None, "too soon");
    steady(&mut gauge, 3.0, GAUGE_SECONDS);
    assert!((gauge.pressure(AREA).unwrap() - 3.0).abs() < 1e-9);
}

/// A change in pressure shows mostly within one averaging time, and fully
/// within a few.
#[test]
fn old_hits_fade_from_the_reading() {
    let mut gauge = PressureGauge::new();
    steady(&mut gauge, 1.0, 5.0 * GAUGE_SECONDS);
    steady(&mut gauge, 2.0, GAUGE_SECONDS);
    let after_one = gauge.pressure(AREA).unwrap();
    assert!(after_one > 1.55 && after_one < 1.7, "{after_one}");
    steady(&mut gauge, 2.0, 5.0 * GAUGE_SECONDS);
    assert!((gauge.pressure(AREA).unwrap() - 2.0).abs() < 0.01);
}

/// Hits come in bursts, not every substep; the reading is the same.
#[test]
fn bursts_average_out() {
    let mut gauge = PressureGauge::new();
    for step in 0..(3.0 * GAUGE_SECONDS / DT) as usize {
        let impulse = if step % 4 == 0 { 4.0 * AREA * DT } else { 0.0 };
        gauge.record(impulse, DT);
    }
    assert!((gauge.pressure(AREA).unwrap() - 1.0).abs() < 0.01);
}
