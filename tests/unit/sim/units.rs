//! Helium-calibration and measurement tests for `src/sim/units.rs`.

use macroquad::prelude::vec3;

use super::*;

/// Largest difference treated as equal.
const TOLERANCE: f64 = 1e-10;
/// Initial volume the tests calibrate for: one liter.
const VOLUME_M3: f64 = 1e-3;

#[test]
fn largest_fill_holds_the_largest_concentration() {
    let gas = HeliumCalibration::new(VOLUME_M3);
    let liters = VOLUME_M3 * LITERS_PER_CUBIC_METER;
    let concentration = gas.total_moles(MAX_PARTICLES) / liters;
    assert!((concentration - MAX_CONCENTRATION as f64).abs() < TOLERANCE);
    assert!(
        (gas.total_moles(2 * MIN_PARTICLES) - 2.0 * gas.total_moles(MIN_PARTICLES)).abs()
            < TOLERANCE
    );
}

#[test]
fn temperature_leaves_out_bulk_flow() {
    const DRIFT: f32 = 5.0;
    let mut sample = KineticMoments::default();
    sample.record(vec3(DRIFT + 1.0, 0.0, 0.0));
    sample.record(vec3(DRIFT - 1.0, 0.0, 0.0));
    // Thermal energy is one particle mass, over N - 1 = 1 and three axes.
    let expected = 2.0 * PARTICLE_MASS as f64 * KELVIN_PER_UNIT / 3.0;
    assert!((sample.temperature_kelvin().unwrap() - expected).abs() < TOLERANCE);
}

/// The concentration is the helium amount over the volume, and a measured
/// pressure passes through in pascals.
#[test]
fn concentration_and_measured_pressure_come_out_in_si() {
    const REDUCED_PRESSURE: f64 = 0.5;
    let gas = HeliumCalibration::new(VOLUME_M3);
    let mut sample = KineticMoments::default();
    sample.record(vec3(-1.0, 0.0, 0.0));
    sample.record(vec3(1.0, 0.0, 0.0));
    let unread = gas.measure(sample, VOLUME_M3, None);
    assert_eq!(unread.pressure_pa, None);
    let readings = gas.measure(sample, VOLUME_M3, Some(REDUCED_PRESSURE));
    let moles = gas.total_moles(readings.count);
    let expected = gas.pressure_pa(REDUCED_PRESSURE);
    assert!((readings.pressure_pa.unwrap() - expected).abs() < TOLERANCE);
    let liters = VOLUME_M3 * LITERS_PER_CUBIC_METER;
    assert!((readings.concentration_mol_per_l - moles / liters).abs() < TOLERANCE);
}

#[test]
fn energy_matches_a_monatomic_gas() {
    const COUNT: usize = 200;
    let gas = HeliumCalibration::new(VOLUME_M3);
    // Internal energy 3/2 N kT, with kT = 1 in reduced units.
    let reduced = 1.5 * COUNT as f32;
    let expected = 1.5 * gas.total_moles(COUNT) * GAS_CONSTANT * KELVIN_PER_UNIT;
    assert!((gas.energy_joules(reduced) - expected).abs() < TOLERANCE);
}
