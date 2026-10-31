//! Particle-collision and temperature tests for `src/sim/gas.rs`.

use macroquad::prelude::vec3;

use super::*;

/// Any radius works; these tests do not depend on the engine's.
const RADIUS: f32 = 0.18;
/// Center offset from the origin: the pair overlaps by 0.04.
const OFFSET: f32 = 0.16;
/// Speed of each test particle.
const SPEED: f32 = 1.0;
/// Largest difference treated as equal.
const TOLERANCE: f32 = 1e-6;

/// Two overlapping particles on the x axis, moving along it at `speeds`.
fn pair(speeds: [f32; 2]) -> [Particle; 2] {
    let particle = |x: f32, speed: f32| Particle {
        position: vec3(x, 0.0, 0.0),
        velocity: vec3(speed, 0.0, 0.0),
    };
    [particle(-OFFSET, speeds[0]), particle(OFFSET, speeds[1])]
}

#[test]
fn impacts_conserve_energy_and_momentum() {
    let mut particles = pair([SPEED, -SPEED]);
    let energy = kinetic_energy(&particles);
    let momentum = particles[0].velocity + particles[1].velocity;
    let [a, b] = &mut particles;
    assert_eq!(resolve_pair(a, b, RADIUS), Contact::Impact);
    assert!((kinetic_energy(&particles) - energy).abs() < TOLERANCE);
    assert!((particles[0].velocity + particles[1].velocity - momentum).length() < TOLERANCE);
    assert!(particles[0].velocity.x < 0.0 && particles[1].velocity.x > 0.0);
}

#[test]
fn receding_or_apart_particles_keep_their_velocities() {
    let mut particles = pair([-SPEED, SPEED]);
    let [a, b] = &mut particles;
    assert_eq!(resolve_pair(a, b, RADIUS), Contact::Overlap);
    assert_eq!((a.velocity.x, b.velocity.x), (-SPEED, SPEED));
    assert!(a.position.distance(b.position) >= 2.0 * RADIUS - TOLERANCE);
    assert_eq!(resolve_pair(a, b, RADIUS), Contact::Apart);
}

#[test]
fn thermalize_stops_bulk_flow_and_sets_the_temperature() {
    const DRIFT: f32 = 0.6;
    const TEMPERATURE: f32 = 2.0;
    let mut particles = pair([DRIFT + SPEED, DRIFT - SPEED]);
    thermalize(&mut particles, TEMPERATURE);
    assert!(mean_velocity(&particles).length() < TOLERANCE);
    // Two particles: N - 1 = 1 over three axes.
    let temperature = 2.0 * kinetic_energy(&particles) / DEGREES_OF_FREEDOM;
    assert!((temperature - TEMPERATURE).abs() < TOLERANCE);
}
