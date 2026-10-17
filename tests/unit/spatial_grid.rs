//! Spatial-grid unit tests for `src/gas.rs`.

use super::*;
use macroquad::prelude::vec3;

const CHAMBER_DIAMETER: f32 = 4.0;
const PARTICLE_RADIUS: f32 = 0.18;

const PARTICLE_SPEED: f32 = 1.0;
const BOUNDARY_CELL_INDEX: usize = 5;

fn particle(x: f32, velocity_x: f32) -> Particle {
    Particle {
        position: vec3(x, 0.0, 0.0),
        velocity: vec3(velocity_x, 0.0, 0.0),
    }
}

#[test]
fn resolves_and_reports_a_collision_across_a_cell_boundary() {
    let mut grid = SpatialGrid::new(CHAMBER_DIAMETER, PARTICLE_RADIUS);
    let boundary = -grid.chamber_half_size + BOUNDARY_CELL_INDEX as f32 * grid.cell_size;
    let separation_from_boundary = PARTICLE_RADIUS / 2.0;
    let mut particles = [
        particle(boundary - separation_from_boundary, PARTICLE_SPEED),
        particle(boundary + separation_from_boundary, -PARTICLE_SPEED),
    ];

    // Confirm the particles occupy distinct cells before testing.
    assert_ne!(
        grid.cell_index(particles[0].position),
        grid.cell_index(particles[1].position),
    );

    let mut impacts = Vec::new();
    let checks = grid.resolve_collisions(&mut particles, |a, b| impacts.push((a, b)));

    assert_eq!(checks, 1);
    assert_eq!(impacts, vec![(0, 1)]);
    assert!(particles[0].velocity.x < 0.0);
    assert!(particles[1].velocity.x > 0.0);

    // The particles have already bounced. They should not flash again
    // merely because they still overlap at the next grid update.
    impacts.clear();
    grid.resolve_collisions(&mut particles, |a, b| impacts.push((a, b)));
    assert!(impacts.is_empty());
}

#[test]
fn skips_particles_in_distant_cells() {
    let mut grid = SpatialGrid::new(CHAMBER_DIAMETER, PARTICLE_RADIUS);
    let mut particles = [
        particle(-1.0, PARTICLE_SPEED),
        particle(1.0, -PARTICLE_SPEED),
    ];

    assert_eq!(grid.resolve_collisions(&mut particles, |_, _| {}), 0);
}

#[test]
fn clearing_grid_removes_stale_particle_locations() {
    let mut grid = SpatialGrid::new(CHAMBER_DIAMETER, PARTICLE_RADIUS);
    let mut particles = [
        particle(-1.5, PARTICLE_SPEED),
        particle(-1.4, -PARTICLE_SPEED),
    ];

    assert_eq!(grid.resolve_collisions(&mut particles, |_, _| {}), 1);
    particles[1].position = vec3(1.5, 0.0, 0.0);
    assert_eq!(grid.resolve_collisions(&mut particles, |_, _| {}), 0);
}

#[test]
fn visits_each_pair_in_the_same_cell_once() {
    let mut grid = SpatialGrid::new(CHAMBER_DIAMETER, PARTICLE_RADIUS);
    let mut particles = [
        particle(-1.5, 0.0),
        particle(-1.5, 0.0),
        particle(-1.5, 0.0),
    ];

    assert_eq!(grid.resolve_collisions(&mut particles, |_, _| {}), 3);
}
