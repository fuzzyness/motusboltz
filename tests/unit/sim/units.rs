//! Helium-calibration and measurement tests for `src/sim/units.rs`.

use super::*;

impl HeliumCalibration {
    pub(crate) fn initial_concentration(self, markers: usize, total_volume_m3: f64) -> f64 {
        self.total_moles(markers) / (total_volume_m3 * 1_000.0)
    }
}
use macroquad::prelude::vec3;

use crate::sim::gas::PARTICLE_MASS;

const TEST_TOLERANCE: f64 = 1e-10;
const TEST_VOLUME_M3: f64 = 1e-3;
const TEST_MAX_MARKERS: usize = 2_048;
const TEST_MAX_CONCENTRATION: f64 = 1.0;
const TEST_MARKERS: usize = 200;

#[test]
fn full_marker_capacity_represents_one_molar_in_the_total_volume() {
    let gas = HeliumCalibration::new(TEST_VOLUME_M3, TEST_MAX_MARKERS, TEST_MAX_CONCENTRATION);
    assert!(
        (gas.initial_concentration(TEST_MAX_MARKERS, TEST_VOLUME_M3) - 1.0).abs() < TEST_TOLERANCE
    );
    assert!((gas.total_moles(TEST_MAX_MARKERS) - 1.0).abs() < TEST_TOLERANCE);
}

#[test]
fn local_temperature_excludes_uniform_drift() {
    const DRIFT: f32 = 5.0;
    let mut sample = KineticMoments::default();
    sample.record(vec3(DRIFT + 1.0, 0.0, 0.0));
    sample.record(vec3(DRIFT - 1.0, 0.0, 0.0));
    // E_thermal = particle mass, N-1 = 1, k_B = 1.
    let expected_kelvin = 2.0 * PARTICLE_MASS as f64 * KELVIN_PER_UNIT / 3.0;
    assert!((sample.temperature_kelvin().unwrap() - expected_kelvin).abs() < TEST_TOLERANCE);
}

#[test]
fn pressures_follow_temperature_and_molar_concentration() {
    let gas = HeliumCalibration::new(TEST_VOLUME_M3, TEST_MAX_MARKERS, TEST_MAX_CONCENTRATION);
    let mut sample = KineticMoments::default();
    sample.record(vec3(-1.0, 0.0, 0.0));
    sample.record(vec3(1.0, 0.0, 0.0));
    let measurements = gas.measure(sample, TEST_VOLUME_M3);
    let pressure = measurements.pressure_pa.unwrap();
    let moles = gas.total_moles(measurements.markers);
    let expected = moles * GAS_CONSTANT * measurements.temperature_kelvin.unwrap() / TEST_VOLUME_M3;
    assert!((pressure - expected).abs() < TEST_TOLERANCE);
    assert!(
        (measurements.concentration_mol_per_l - moles * 1e-3 / TEST_VOLUME_M3).abs()
            < TEST_TOLERANCE
    );
}

#[test]
fn increasing_marker_count_increases_helium_amount() {
    let gas = HeliumCalibration::new(TEST_VOLUME_M3, TEST_MAX_MARKERS, TEST_MAX_CONCENTRATION);
    assert!(
        (gas.total_moles(TEST_MARKERS * 2) - 2.0 * gas.total_moles(TEST_MARKERS)).abs()
            < TEST_TOLERANCE
    );
}

#[test]
fn energy_units_agree_with_monatomic_gas_internal_energy() {
    let gas = HeliumCalibration::new(TEST_VOLUME_M3, TEST_MAX_MARKERS, TEST_MAX_CONCENTRATION);
    let e_reduced = 1.5 * TEST_MARKERS as f32;
    let expected = 1.5 * gas.total_moles(TEST_MARKERS) * GAS_CONSTANT * KELVIN_PER_UNIT;
    assert!((gas.energy_joules(e_reduced) - expected).abs() < TEST_TOLERANCE);
}
