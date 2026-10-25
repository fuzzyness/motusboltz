//! Particle-collision and temperature tests for `src/sim/gas.rs`.

use super::*;

/// Temperature of a whole sample (the simulation measures regions with
/// `KineticMoments` instead).
fn temperature(particles: &[Particle]) -> f32 {
    if particles.len() < 2 {
        return 0.0;
    }
    temperature_about_mean(particles, mean_velocity(particles))
}
/// Any radius works; these tests do not depend on the engine's.
const PARTICLE_RADIUS: f32 = 0.18;
use macroquad::prelude::vec3;

const TEST_PARTICLE_OFFSET: f32 = 0.16;
const TEST_PARTICLE_SPEED: f32 = 1.0;
const TOLERANCE: f32 = 1e-6;
const TARGET_TEST_TEMPERATURE: f32 = 2.0;
const TEST_DRIFT_SPEED: f32 = 0.6;

#[test]
fn elastic_collision_conserves_energy_and_momentum() {
    let mut particles = [
        Particle {
            position: vec3(-TEST_PARTICLE_OFFSET, 0.0, 0.0),
            velocity: vec3(TEST_PARTICLE_SPEED, 0.0, 0.0),
        },
        Particle {
            position: vec3(TEST_PARTICLE_OFFSET, 0.0, 0.0),
            velocity: vec3(-TEST_PARTICLE_SPEED, 0.0, 0.0),
        },
    ];

    let energy_before = kinetic_energy(&particles);
    let momentum_before = particles[0].velocity + particles[1].velocity;

    let (left, right) = particles.split_at_mut(1);
    assert!(resolve_pair(&mut left[0], &mut right[0], PARTICLE_RADIUS));

    let energy_after = kinetic_energy(&particles);
    let momentum_after = particles[0].velocity + particles[1].velocity;

    assert!((energy_after - energy_before).abs() < TOLERANCE);
    assert!((momentum_after - momentum_before).length() < TOLERANCE);
    assert!(particles[0].velocity.x < 0.0);
    assert!(particles[1].velocity.x > 0.0);
}

#[test]
fn separated_or_receding_particles_do_not_trigger_collision_events() {
    let mut first = Particle {
        position: vec3(-TEST_PARTICLE_OFFSET, 0.0, 0.0),
        velocity: vec3(-TEST_PARTICLE_SPEED, 0.0, 0.0),
    };
    let mut second = Particle {
        position: vec3(TEST_PARTICLE_OFFSET, 0.0, 0.0),
        velocity: vec3(TEST_PARTICLE_SPEED, 0.0, 0.0),
    };

    // Overlapping but separating: correct geometry without reporting
    // a kinetic collision or changing either velocity.
    assert!(!resolve_pair(&mut first, &mut second, PARTICLE_RADIUS));
    assert_eq!(first.velocity.x, -TEST_PARTICLE_SPEED);
    assert_eq!(second.velocity.x, TEST_PARTICLE_SPEED);

    first.position = vec3(-PARTICLE_RADIUS * 2.0, 0.0, 0.0);
    second.position = vec3(PARTICLE_RADIUS * 2.0, 0.0, 0.0);
    assert!(!resolve_pair(&mut first, &mut second, PARTICLE_RADIUS));
}

#[test]
fn temperature_adjustment_preserves_bulk_motion() {
    let mut particles = [
        Particle {
            position: Vec3::ZERO,
            velocity: vec3(TEST_DRIFT_SPEED + TEST_PARTICLE_SPEED, 0.0, 0.0),
        },
        Particle {
            position: Vec3::ZERO,
            velocity: vec3(TEST_DRIFT_SPEED - TEST_PARTICLE_SPEED, 0.0, 0.0),
        },
    ];
    let momentum_before = particles
        .iter()
        .map(|particle| particle.velocity * PARTICLE_MASS)
        .sum::<Vec3>();

    rescale_thermal_velocities(&mut particles, TARGET_TEST_TEMPERATURE);

    let momentum_after = particles
        .iter()
        .map(|particle| particle.velocity * PARTICLE_MASS)
        .sum::<Vec3>();
    assert!((momentum_before - momentum_after).length() < TOLERANCE);
    assert!((temperature(&particles) - TARGET_TEST_TEMPERATURE).abs() < TOLERANCE);
}

#[test]
fn temperature_adjustment_can_heat_a_motionless_gas() {
    let mut particles = [
        Particle {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
        },
        Particle {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
        },
    ];

    rescale_thermal_velocities(&mut particles, TARGET_TEST_TEMPERATURE);

    let net_velocity = particles[0].velocity + particles[1].velocity;
    assert!(net_velocity.length() < TOLERANCE);
    assert!((temperature(&particles) - TARGET_TEST_TEMPERATURE).abs() < TOLERANCE);
}
