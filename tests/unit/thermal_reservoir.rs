//! Thermal-reservoir unit tests for `src/gas.rs`.

use super::*;

impl ThermalReservoirs {
    pub(crate) fn set_accommodation(&mut self, value: f32) {
        assert!((0.0..=1.0).contains(&value));
        self.accommodation = value;
    }
}
use macroquad::prelude::{
    Vec3,
    vec3, //
};

const TOLERANCE: f32 = 1e-5;
const INCIDENT_SPEED: f32 = 1.0;
const FAST_INCIDENT_SPEED: f32 = 3.0;
const SAMPLES: usize = 128;

fn particle(normal_speed: f32) -> Particle {
    Particle {
        position: Vec3::ZERO,
        velocity: vec3(normal_speed, 0.7, -0.3) / PARTICLE_MASS.sqrt(),
    }
}

#[test]
fn hot_setting_changes_reservoir_not_gas() {
    let mut walls = ThermalReservoirs::new();
    assert_eq!(walls.hot_kelvin(), MIN_HOT_KELVIN);
    walls.set_hot_kelvin(MAX_HOT_KELVIN);
    assert_eq!(walls.hot_kelvin(), MAX_HOT_KELVIN);
    assert_eq!(walls.heat().total(), 0.0);
}

#[test]
fn insulated_reflection_is_elastic_and_preserves_tangential_velocity() {
    let mut walls = ThermalReservoirs::new();
    walls.set_accommodation(1.0);
    let mut particle = particle(INCIDENT_SPEED);
    walls.reflect(&mut particle, Vec3::X, None);
    assert_eq!(
        particle.velocity,
        vec3(-INCIDENT_SPEED, 0.7, -0.3) / PARTICLE_MASS.sqrt()
    );
    assert_eq!(walls.heat().total(), 0.0);
}

#[test]
fn hot_wall_can_heat_and_cold_wall_can_cool() {
    let mut walls = ThermalReservoirs::new();
    walls.set_accommodation(1.0);
    walls.set_hot_kelvin(MAX_HOT_KELVIN);
    for _ in 0..SAMPLES {
        let mut hot = particle(INCIDENT_SPEED);
        walls.reflect(&mut hot, Vec3::X, Some(ReservoirSide::Hot));
        assert!(hot.velocity.x < 0.0);
        let mut cold = particle(FAST_INCIDENT_SPEED);
        walls.reflect(&mut cold, Vec3::X, Some(ReservoirSide::Cold));
        assert!(cold.velocity.x < 0.0);
    }
    assert!(walls.heat().hot > 0.0);
    assert!(walls.heat().cold < 0.0);
}

#[test]
fn heat_tracks_particle_energy_exactly() {
    let mut walls = ThermalReservoirs::new();
    walls.set_accommodation(1.0);
    let mut particle = particle(INCIDENT_SPEED);
    let before = 0.5 * PARTICLE_MASS * particle.velocity.length_squared();
    walls.reflect(&mut particle, Vec3::X, Some(ReservoirSide::Hot));
    let after = 0.5 * PARTICLE_MASS * particle.velocity.length_squared();
    assert!(((after - before) as f64 - walls.heat().hot).abs() < TOLERANCE as f64);
}

#[test]
fn cold_control_has_shared_equilibrium_endpoint_and_does_not_inject_heat() {
    let mut walls = ThermalReservoirs::with_seed(42);
    assert_eq!(walls.cold_kelvin(), walls.hot_kelvin());
    walls.set_cold_kelvin(MIN_COLD_KELVIN);
    assert_eq!(walls.cold_kelvin(), MIN_COLD_KELVIN);
    assert_eq!(walls.heat().total(), 0.0);
    assert_eq!(walls.hot_kelvin(), MIN_HOT_KELVIN);
}

#[test]
fn cold_wall_emission_matches_flux_weighted_temperature() {
    const SAMPLE_COUNT: usize = 20_000;
    const RELATIVE_TOLERANCE: f64 = 0.04;
    const SEED: u64 = 75;
    let mut walls = ThermalReservoirs::with_seed(SEED);
    walls.set_accommodation(1.0);
    walls.set_cold_kelvin(MIN_COLD_KELVIN);
    let mut normal_squared = 0.0;
    let mut tangent_squared = 0.0;
    for _ in 0..SAMPLE_COUNT {
        let mut p = particle(INCIDENT_SPEED);
        walls.reflect(&mut p, Vec3::X, Some(ReservoirSide::Cold));
        normal_squared += (p.velocity.x * p.velocity.x) as f64;
        tangent_squared += (p.velocity.y * p.velocity.y + p.velocity.z * p.velocity.z) as f64;
    }
    let mean = normal_squared / SAMPLE_COUNT as f64;
    let expected = 2.0 * MIN_COLD_KELVIN as f64 / KELVIN_PER_UNIT / PARTICLE_MASS as f64;
    assert!((mean - expected).abs() < RELATIVE_TOLERANCE * expected);
    assert!(
        (tangent_squared / SAMPLE_COUNT as f64 - expected).abs() < RELATIVE_TOLERANCE * expected
    );
}
