//! Broad-phase collision detection using a reusable uniform 3D grid.

use macroquad::prelude::Vec3;

use crate::particle::Particle;
use crate::physics;

/// A uniform collision grid for equal-radius hard spheres.
///
/// Every cell is at least one particle diameter wide, so overlapping
/// particles lie in the same cell or in immediately adjacent cells.
pub struct SpatialGrid {
    cells: Vec<Vec<usize>>,
    occupied_cells: Vec<usize>,
    cells_per_axis: usize,
    cell_size: f32,
    chamber_half_size: f32,
    particle_radius: f32,
}

impl SpatialGrid {
    /// Builds a cubic broad-phase grid around the cylindrical chamber.
    ///
    /// # Panics
    ///
    /// Panics if the chamber size or particle radius is nonpositive or
    /// non-finite.
    pub fn new(chamber_size: f32, particle_radius: f32) -> Self {
        assert!(chamber_size.is_finite() && chamber_size > 0.0);
        assert!(particle_radius.is_finite() && particle_radius > 0.0);

        let diameter = 2.0 * particle_radius;
        let cells_per_axis = ((chamber_size / diameter).floor() as usize).max(1);
        let cell_count = cells_per_axis.pow(3);

        Self {
            cells: vec![Vec::new(); cell_count],
            occupied_cells: Vec::new(),
            cells_per_axis,
            cell_size: chamber_size / cells_per_axis as f32,
            chamber_half_size: chamber_size / 2.0,
            particle_radius,
        }
    }

    fn flatten(&self, x: usize, y: usize, z: usize) -> usize {
        (z * self.cells_per_axis + y) * self.cells_per_axis + x
    }

    fn unflatten(&self, index: usize) -> (usize, usize, usize) {
        let side = self.cells_per_axis;
        let x = index % side;
        let y = (index / side) % side;
        let z = index / (side * side);
        (x, y, z)
    }

    fn axis_cell(&self, coordinate: f32) -> usize {
        let index = ((coordinate + self.chamber_half_size) / self.cell_size).floor();
        index.clamp(0.0, (self.cells_per_axis - 1) as f32) as usize
    }

    fn cell_index(&self, position: Vec3) -> usize {
        self.flatten(
            self.axis_cell(position.x),
            self.axis_cell(position.y),
            self.axis_cell(position.z),
        )
    }

    fn rebuild(&mut self, particles: &[Particle]) {
        // Reuse buckets to avoid reallocating them every physics step.
        for &index in &self.occupied_cells {
            self.cells[index].clear();
        }
        self.occupied_cells.clear();

        for (particle_index, particle) in particles.iter().enumerate() {
            let cell_index = self.cell_index(particle.position);
            if self.cells[cell_index].is_empty() {
                self.occupied_cells.push(cell_index);
            }
            self.cells[cell_index].push(particle_index);
        }
    }

    /// Resolves nearby pairs, invoking `on_collision` for actual impacts.
    ///
    /// Each pair is checked once. The callback receives particle indices only
    /// if the collision changes their velocities, not for overlap correction.
    /// Returns the total number of candidate pairs checked.
    pub fn resolve_collisions<F>(
        &mut self,
        particles: &mut [Particle],
        mut on_collision: F,
    ) -> usize
    where
        F: FnMut(usize, usize),
    {
        self.rebuild(particles);

        let mut candidate_checks = 0;
        let side = self.cells_per_axis;

        for &cell_index in &self.occupied_cells {
            let occupants = &self.cells[cell_index];

            // Each pair of occupants in this cell is visited once.
            for (offset, &first) in occupants.iter().enumerate() {
                for &second in &occupants[offset + 1..] {
                    if Self::resolve_index_pair(particles, first, second, self.particle_radius) {
                        on_collision(first, second);
                    }
                    candidate_checks += 1;
                }
            }
            let (cx, cy, cz) = self.unflatten(cell_index);
            let min_x = cx.saturating_sub(1);
            let min_y = cy.saturating_sub(1);
            let min_z = cz.saturating_sub(1);
            let max_x = (cx + 1).min(side - 1);
            let max_y = (cy + 1).min(side - 1);
            let max_z = (cz + 1).min(side - 1);

            for z in min_z..=max_z {
                for y in min_y..=max_y {
                    for x in min_x..=max_x {
                        let neighbor_index = self.flatten(x, y, z);

                        // Avoid checking the same pair of cells in reverse order.
                        if neighbor_index <= cell_index {
                            continue;
                        }

                        for &first in occupants {
                            for &second in &self.cells[neighbor_index] {
                                if Self::resolve_index_pair(
                                    particles,
                                    first,
                                    second,
                                    self.particle_radius,
                                ) {
                                    on_collision(first, second);
                                }
                                candidate_checks += 1;
                            }
                        }
                    }
                }
            }
        }

        candidate_checks
    }

    fn resolve_index_pair(
        particles: &mut [Particle],
        first: usize,
        second: usize,
        radius: f32,
    ) -> bool {
        // `split_at_mut()` guarantees disjoint mutable particle references.
        let (a, b) = if first < second {
            let (left, right) = particles.split_at_mut(second);
            (&mut left[first], &mut right[0])
        } else {
            let (left, right) = particles.split_at_mut(first);
            (&mut right[0], &mut left[second])
        };

        physics::resolve_pair(a, b, radius)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::prelude::vec3;

    use crate::simulation::{
        CHAMBER_DIAMETER,
        PARTICLE_RADIUS, //
    };

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
}
