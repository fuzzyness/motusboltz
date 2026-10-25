//! Spatial-grid unit tests for `src/sim/grid.rs`.

use super::*;
use crate::sim::gas::Particle;
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

#[test]
fn every_neighbor_direction_is_checked_once() {
    const BORDER_INSET: f32 = 0.01;
    let mut grid = SpatialGrid::new(CHAMBER_DIAMETER, PARTICLE_RADIUS);
    let cell = grid.cells_per_axis / 2;
    let low = -grid.chamber_half_size + cell as f32 * grid.cell_size;
    let high = low + grid.cell_size;
    let center = (low + high) / 2.0;
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                if dx == 0 && dy == 0 && dz == 0 {
                    continue;
                }
                let mut a = Vec3::ZERO;
                let mut b = Vec3::ZERO;
                for (axis, direction) in [dx, dy, dz].into_iter().enumerate() {
                    (a[axis], b[axis]) = match direction {
                        -1 => (low + BORDER_INSET, low - BORDER_INSET),
                        1 => (high - BORDER_INSET, high + BORDER_INSET),
                        _ => (center, center),
                    };
                }
                let velocity = (b - a).normalize() * PARTICLE_SPEED;
                let mut pair = [
                    Particle {
                        position: a,
                        velocity,
                    },
                    Particle {
                        position: b,
                        velocity: -velocity,
                    },
                ];
                let mut collisions = 0;
                assert_eq!(
                    grid.resolve_collisions(&mut pair, |_, _| collisions += 1),
                    1
                );
                assert_eq!(collisions, 1, "missed direction {dx},{dy},{dz}");
            }
        }
    }
}

#[test]
fn halo_handles_all_corners_and_does_not_wrap_opposite_faces() {
    const CORNER_INSET_RADII: f32 = 0.5;
    let mut grid = SpatialGrid::new(CHAMBER_DIAMETER, PARTICLE_RADIUS);
    let corner = grid.chamber_half_size - PARTICLE_RADIUS * CORNER_INSET_RADII;
    for x in [-1.0, 1.0] {
        for y in [-1.0, 1.0] {
            for z in [-1.0, 1.0] {
                let position = vec3(x, y, z) * corner;
                let inward = -position.normalize();
                let mut pair = [
                    Particle {
                        position,
                        velocity: inward,
                    },
                    Particle {
                        position: position + inward * PARTICLE_RADIUS,
                        velocity: -inward,
                    },
                ];
                let mut impacts = 0;
                assert_eq!(grid.resolve_collisions(&mut pair, |_, _| impacts += 1), 1);
                assert_eq!(impacts, 1);
            }
        }
    }
    let mut opposite = [particle(-corner, 0.0), particle(corner, 0.0)];
    assert_eq!(grid.resolve_collisions(&mut opposite, |_, _| {}), 0);
}
