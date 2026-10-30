//! Path-recording tests for `src/sim/tracers.rs`.

use super::*;
use macroquad::prelude::vec3;

const STEP: f64 = 0.001;

fn particle(position: Vec3, velocity: Vec3) -> Particle {
    Particle { position, velocity }
}

#[test]
fn a_path_gains_a_point_only_when_the_velocity_changes() {
    let mut particles = [particle(Vec3::ZERO, Vec3::X)];
    let mut tracers = Tracers::new(&[0], &particles);
    for _ in 0..10 {
        particles[0].position += particles[0].velocity * STEP as f32;
        tracers.record(STEP, &particles);
    }
    let (index, points) = tracers.paths().next().unwrap();
    assert_eq!(index, 0);
    assert_eq!(points.len(), 1, "straight flight needs no new points");

    // A bounce: the velocity reverses, and the bounce point is kept.
    particles[0].velocity = -Vec3::X;
    tracers.record(STEP, &particles);
    let (_, points) = tracers.paths().next().unwrap();
    assert_eq!(points.len(), 2);
    assert_eq!(points[1].position, particles[0].position);
    assert!((points[1].time - 11.0 * STEP).abs() < 1e-12);
}

#[test]
fn old_points_drop_out_but_the_path_still_reaches_back() {
    let mut particles = [particle(Vec3::ZERO, Vec3::X)];
    let mut tracers = Tracers::new(&[0], &particles);
    let steps = (3.0 * TRAIL_SECONDS / STEP) as usize;
    for step in 0..steps {
        // Bounce every step, so every step adds a point.
        particles[0].velocity = if step % 2 == 0 { Vec3::Y } else { Vec3::X };
        tracers.record(STEP, &particles);
    }
    let (_, points) = tracers.paths().next().unwrap();
    let oldest = tracers.time() - TRAIL_SECONDS;
    assert!(points[0].time <= oldest, "the path covers the whole window");
    assert!(points[1].time > oldest, "only one point is older");
    assert!(points.len() <= MAX_PATH_POINTS);
}

#[test]
fn every_followed_particle_has_its_own_path() {
    let particles = [
        particle(vec3(1.0, 0.0, 0.0), Vec3::X),
        particle(vec3(2.0, 0.0, 0.0), Vec3::Y),
        particle(vec3(3.0, 0.0, 0.0), Vec3::Z),
    ];
    let tracers = Tracers::new(&[2, 0], &particles);
    assert_eq!(tracers.indices().collect::<Vec<_>>(), [2, 0]);
    let starts: Vec<_> = tracers
        .paths()
        .map(|(_, points)| points[0].position)
        .collect();
    assert_eq!(starts, [particles[2].position, particles[0].position]);
}
