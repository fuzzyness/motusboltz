//! Trail mesh tests for `src/graphics/trails.rs`.

use super::*;
use macroquad::prelude::vec3;

const STEP: f64 = 0.01;

fn particle(position: Vec3, velocity: Vec3) -> Particle {
    Particle { position, velocity }
}

/// A particle that bounced twice gives three segments: two between bounce
/// points and one to where it is now. Each is a quad facing the eye, and
/// the newest end is the most opaque.
#[test]
fn trails_are_camera_facing_ribbons_that_fade_with_age() {
    let eye = vec3(0.0, 0.0, 10.0);
    let mut particles = [particle(Vec3::ZERO, Vec3::X)];
    let mut tracers = Tracers::new(&[0], &particles);
    for velocity in [Vec3::Y, -Vec3::X] {
        particles[0].position += particles[0].velocity * STEP as f32;
        particles[0].velocity = velocity;
        tracers.record(STEP, &particles);
    }
    particles[0].position += particles[0].velocity * STEP as f32;
    tracers.record(STEP, &particles);

    let mut renderer = TrailRenderer::new();
    renderer.build(&tracers, &particles, eye);
    assert_eq!(renderer.mesh.vertices.len(), 3 * VERTICES_PER_SEGMENT);
    assert_eq!(renderer.mesh.indices.len(), 3 * INDICES_PER_SEGMENT);
    assert!(
        renderer
            .mesh
            .indices
            .iter()
            .all(|&index| (index as usize) < renderer.mesh.vertices.len())
    );
    // Each quad is TRAIL_WIDTH across, and its width points neither along
    // the segment nor toward the eye, so the ribbon faces the camera.
    for quad in renderer.mesh.vertices.chunks(VERTICES_PER_SEGMENT) {
        let across = quad[0].position - quad[1].position;
        let along = quad[2].position - quad[0].position;
        let middle = (quad[0].position + quad[3].position) / 2.0;
        assert!((across.length() - TRAIL_WIDTH).abs() < 1e-5);
        assert!(across.dot(along).abs() < 1e-6);
        assert!(across.dot(eye - middle).abs() < 1e-5);
    }
    let first = renderer.mesh.vertices.first().unwrap().color[3];
    let last = renderer.mesh.vertices.last().unwrap().color[3];
    assert!(last > first, "the newest end is the most opaque");
}

#[test]
fn a_still_particle_draws_nothing() {
    let particles = [particle(Vec3::ONE, Vec3::ZERO)];
    let tracers = Tracers::new(&[0], &particles);
    let mut renderer = TrailRenderer::new();
    renderer.build(&tracers, &particles, Vec3::Z * 10.0);
    assert!(renderer.mesh.indices.is_empty());
}
