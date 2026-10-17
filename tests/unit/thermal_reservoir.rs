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
        velocity: vec3(normal_speed, 0.7, -0.3),
    }
}

#[test]
fn heater_changes_reservoir_not_gas() {
    let mut walls = ThermalReservoirs::new();
    assert_eq!(walls.hot_temperature(), COLD_TEMPERATURE);
    walls.set_heater_percent(MAX_HEATER_PERCENT);
    assert_eq!(walls.hot_temperature(), MAX_HOT_TEMPERATURE);
    assert_eq!(walls.heat().total(), 0.0);
}

#[test]
fn insulated_reflection_is_elastic_and_preserves_tangential_velocity() {
    let mut walls = ThermalReservoirs::new();
    walls.set_accommodation(1.0);
    let mut particle = particle(INCIDENT_SPEED);
    walls.reflect(&mut particle, Vec3::X, None);
    assert_eq!(particle.velocity, vec3(-INCIDENT_SPEED, 0.7, -0.3));
    assert_eq!(walls.heat().total(), 0.0);
}

#[test]
fn hot_wall_can_heat_and_cold_wall_can_cool() {
    let mut walls = ThermalReservoirs::new();
    walls.set_accommodation(1.0);
    walls.set_heater_percent(MAX_HEATER_PERCENT);
    for _ in 0..SAMPLES {
        let mut hot = particle(INCIDENT_SPEED);
        walls.reflect(&mut hot, Vec3::X, Some(ReservoirSide::Hot));
        assert!(hot.velocity.x < 0.0);
        assert_eq!(hot.velocity.y, 0.7);
        assert_eq!(hot.velocity.z, -0.3);
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
    assert!((after - before - walls.heat().hot).abs() < TOLERANCE);
}
