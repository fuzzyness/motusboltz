//! Spatial-grid unit tests for `src/sim/grid.rs`.

use super::*;
use crate::sim::gas::Particle;
use macroquad::prelude::vec3;

const CHAMBER_DIAMETER: f32 = 4.0;
const PARTICLE_RADIUS: f32 = 0.18;

const PARTICLE_SPEED: f32 = 1.0;
const BOUNDARY_CELL_INDEX: usize = 5;

/// A contact callback that records the pairs that bounced.
fn record_impacts(impacts: &mut Vec<(usize, usize)>) -> impl FnMut(usize, usize, Contact) + '_ {
    |a, b, contact| {
        if contact == Contact::Impact {
            impacts.push((a, b));
        }
    }
}

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
    let checks = grid.resolve_collisions(&mut particles, record_impacts(&mut impacts));

    assert_eq!(checks, 1);
    assert_eq!(impacts, vec![(0, 1)]);
    assert!(particles[0].velocity.x < 0.0);
    assert!(particles[1].velocity.x > 0.0);

    // The particles have already bounced. They should not flash again
    // merely because they still overlap at the next grid update.
    impacts.clear();
    grid.resolve_collisions(&mut particles, record_impacts(&mut impacts));
    assert!(impacts.is_empty());
}

#[test]
fn skips_particles_in_distant_cells() {
    let mut grid = SpatialGrid::new(CHAMBER_DIAMETER, PARTICLE_RADIUS);
    let mut particles = [
        particle(-1.0, PARTICLE_SPEED),
        particle(1.0, -PARTICLE_SPEED),
    ];

    assert_eq!(grid.resolve_collisions(&mut particles, |_, _, _| {}), 0);
}

#[test]
fn clearing_grid_removes_stale_particle_locations() {
    let mut grid = SpatialGrid::new(CHAMBER_DIAMETER, PARTICLE_RADIUS);
    let mut particles = [
        particle(-1.5, PARTICLE_SPEED),
        particle(-1.4, -PARTICLE_SPEED),
    ];

    assert_eq!(grid.resolve_collisions(&mut particles, |_, _, _| {}), 1);
    particles[1].position = vec3(1.5, 0.0, 0.0);
    assert_eq!(grid.resolve_collisions(&mut particles, |_, _, _| {}), 0);
}

#[test]
fn visits_each_pair_in_the_same_cell_once() {
    let mut grid = SpatialGrid::new(CHAMBER_DIAMETER, PARTICLE_RADIUS);
    let mut particles = [
        particle(-1.5, 0.0),
        particle(-1.5, 0.0),
        particle(-1.5, 0.0),
    ];

    assert_eq!(grid.resolve_collisions(&mut particles, |_, _, _| {}), 3);
}

/// Puts a touching, approaching pair across each face, edge and corner of
/// the lattice cell at indices `cell` and checks that the grid finds each
/// pair exactly once.
fn assert_every_direction_found(grid: &mut SpatialGrid, cell: [u32; 3]) {
    const BORDER_INSET: f32 = 0.01;
    let low = cell.map(|index| -grid.chamber_half_size + index as f32 * grid.cell_size);
    let high = low.map(|low| low + grid.cell_size);
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                if dx == 0 && dy == 0 && dz == 0 {
                    continue;
                }
                let mut a = Vec3::ZERO;
                let mut b = Vec3::ZERO;
                for (axis, direction) in [dx, dy, dz].into_iter().enumerate() {
                    let (low, high) = (low[axis], high[axis]);
                    (a[axis], b[axis]) = match direction {
                        -1 => (low + BORDER_INSET, low - BORDER_INSET),
                        1 => (high - BORDER_INSET, high + BORDER_INSET),
                        _ => ((low + high) / 2.0, (low + high) / 2.0),
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
                    grid.resolve_collisions(&mut pair, |_, _, contact| collisions +=
                        usize::from(contact == Contact::Impact)),
                    1
                );
                assert_eq!(
                    collisions, 1,
                    "missed direction {dx},{dy},{dz} at cell {cell:?}"
                );
            }
        }
    }
}

#[test]
fn every_neighbor_direction_is_checked_once() {
    let mut grid = SpatialGrid::new(CHAMBER_DIAMETER, PARTICLE_RADIUS);
    let cell = grid.last_cell[0] / 2;
    assert_every_direction_found(&mut grid, [cell; 3]);
}

/// A row more than 64 cells long keeps its occupancy bits in several words;
/// neighbors on either side of a word boundary must still be found.
#[test]
fn neighbors_across_occupancy_words_are_found() {
    // A long, thin box: 138 cells along x, a few along y and z.
    const LONG_CHAMBER: f32 = 50.0;
    let mut grid = SpatialGrid::covering(
        LONG_CHAMBER,
        PARTICLE_RADIUS,
        vec3(-LONG_CHAMBER / 2.0, -0.5, -0.5),
        vec3(LONG_CHAMBER / 2.0, 0.5, 0.5),
    );
    assert!(
        grid.padded_x > 2 * BITS_PER_WORD,
        "box too short for the test"
    );
    let middle = |axis: usize| (grid.first_cell[axis] + grid.last_cell[axis]) / 2;
    let (y, z) = (middle(1), middle(2));
    // Lattice cell c sits at padded position c + 1, so cells 62 to 64 put
    // x - 1, x and x + 1 on both sides of the boundary at bit 64, and cells
    // 126 and 127 on both sides of the one at bit 128.
    for x in [61, 62, 63, 64, 126, 127] {
        assert_every_direction_found(&mut grid, [x, y, z]);
    }
}

/// The engine stores only the cells over the gas. For particles inside the
/// stored box that must change nothing: the same pairs, in the same order,
/// with the same results as on the grid over the whole cube.
#[test]
fn a_cropped_grid_matches_the_full_grid() {
    use macroquad::rand::RandGenerator;
    const COUNT: usize = 150;
    const FULL_CHAMBER: f32 = 8.0;
    const SUBSTEPS: usize = 20;
    const DT: f32 = 0.02;
    let lower = vec3(-1.2, -0.6, -0.4);
    let upper = vec3(0.9, 1.1, 0.5);
    let random = RandGenerator::new();
    random.srand(7);
    let mut cropped_particles: Vec<Particle> = (0..COUNT)
        .map(|_| Particle {
            position: vec3(
                random.gen_range(lower.x, upper.x),
                random.gen_range(lower.y, upper.y),
                random.gen_range(lower.z, upper.z),
            ),
            velocity: vec3(
                random.gen_range(-1.0, 1.0),
                random.gen_range(-1.0, 1.0),
                random.gen_range(-1.0, 1.0),
            ),
        })
        .collect();
    let mut full_particles = cropped_particles.clone();
    let mut full = SpatialGrid::new(FULL_CHAMBER, PARTICLE_RADIUS);
    let mut cropped = SpatialGrid::covering(FULL_CHAMBER, PARTICLE_RADIUS, lower, upper);
    assert!(cropped.heads.len() < full.heads.len() / 4);
    let mut contacts = 0;
    for _ in 0..SUBSTEPS {
        for particles in [&mut full_particles, &mut cropped_particles] {
            for particle in particles.iter_mut() {
                particle.position += particle.velocity * DT;
                // Keep everyone inside the stored box.
                for axis in 0..3 {
                    particle.position[axis] =
                        particle.position[axis].clamp(lower[axis], upper[axis]);
                }
            }
        }
        let mut full_order = Vec::new();
        let mut cropped_order = Vec::new();
        full.resolve_collisions(&mut full_particles, |a, b, contact| {
            full_order.push((a, b, contact))
        });
        cropped.resolve_collisions(&mut cropped_particles, |a, b, contact| {
            cropped_order.push((a, b, contact))
        });
        assert_eq!(full_order, cropped_order);
        contacts += full_order.len();
    }
    assert!(contacts > 100, "too few contacts to compare: {contacts}");
    for (full, cropped) in full_particles.iter().zip(&cropped_particles) {
        assert_eq!(full.position, cropped.position);
        assert_eq!(full.velocity, cropped.velocity);
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
                assert_eq!(
                    grid.resolve_collisions(&mut pair, |_, _, contact| impacts +=
                        usize::from(contact == Contact::Impact)),
                    1
                );
                assert_eq!(impacts, 1);
            }
        }
    }
    let mut opposite = [particle(-corner, 0.0), particle(corner, 0.0)];
    assert_eq!(grid.resolve_collisions(&mut opposite, |_, _, _| {}), 0);
}
