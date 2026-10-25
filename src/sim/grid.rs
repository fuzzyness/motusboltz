//! Uniform grid that finds nearby particle pairs, so collision checks cost
//! about one pass over the particles instead of one per pair.

use macroquad::prelude::Vec3;

use crate::sim::gas::{
    Particle,
    resolve_pair, //
};

const EMPTY_CELL: usize = usize::MAX;

/// Uniform grid that finds nearby particle pairs for collision checks.
///
/// Every cell is at least one particle diameter wide, so overlapping
/// particles lie in the same cell or in neighboring cells. Positions outside
/// the grid are clamped into its edge cells.
pub struct SpatialGrid {
    heads: Vec<usize>,
    next: Vec<usize>,
    occupied_cells: Vec<usize>,
    cells_per_axis: usize,
    cell_size: f32,
    padded_side: usize,
    neighbor_offsets: [isize; 13],
    chamber_half_size: f32,
    particle_radius: f32,
}

impl SpatialGrid {
    /// Builds a cubic grid of side `chamber_size`, centered on the origin.
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
        // Empty halo cells eliminate 13 sets of coordinate/boundary checks
        // for every occupied cell, without wrapping across opposite faces.
        const HALO_WIDTH: usize = 1;
        let padded_side = cells_per_axis + 2 * HALO_WIDTH;
        let row = padded_side as isize;
        let plane = row * row;
        let neighbor_offsets = [
            1,
            row - 1,
            row,
            row + 1,
            plane - row - 1,
            plane - row,
            plane - row + 1,
            plane - 1,
            plane,
            plane + 1,
            plane + row - 1,
            plane + row,
            plane + row + 1,
        ];
        let cell_count = padded_side.pow(3);

        Self {
            heads: vec![EMPTY_CELL; cell_count],
            next: Vec::new(),
            occupied_cells: Vec::new(),
            cells_per_axis,
            padded_side,
            neighbor_offsets,
            cell_size: chamber_size / cells_per_axis as f32,
            chamber_half_size: chamber_size / 2.0,
            particle_radius,
        }
    }

    fn flatten(&self, x: usize, y: usize, z: usize) -> usize {
        ((z + 1) * self.padded_side + y + 1) * self.padded_side + x + 1
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
        // Flat linked lists avoid per-cell allocations; clear only occupied cells.
        self.next.resize(particles.len(), EMPTY_CELL);
        for &index in &self.occupied_cells {
            self.heads[index] = EMPTY_CELL;
        }
        self.occupied_cells.clear();

        for (particle_index, particle) in particles.iter().enumerate() {
            let cell_index = self.cell_index(particle.position);
            if self.heads[cell_index] == EMPTY_CELL {
                self.occupied_cells.push(cell_index);
            }
            self.next[particle_index] = self.heads[cell_index];
            self.heads[cell_index] = particle_index;
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
        let mut check_pair = |first, second| {
            if Self::resolve_index_pair(particles, first, second, self.particle_radius) {
                on_collision(first, second);
            }
            candidate_checks += 1;
        };

        // Offsets preserve the original forward-neighbor collision order.
        for &cell_index in &self.occupied_cells {
            let mut first = self.heads[cell_index];
            while first != EMPTY_CELL {
                let mut second = self.next[first];
                while second != EMPTY_CELL {
                    check_pair(first, second);
                    second = self.next[second];
                }
                first = self.next[first];
            }
            for &offset in &self.neighbor_offsets {
                let neighbor = (cell_index as isize + offset) as usize;
                if self.heads[neighbor] == EMPTY_CELL {
                    continue;
                }
                let mut first = self.heads[cell_index];
                while first != EMPTY_CELL {
                    let mut second = self.heads[neighbor];
                    while second != EMPTY_CELL {
                        check_pair(first, second);
                        second = self.next[second];
                    }
                    first = self.next[first];
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

        resolve_pair(a, b, radius)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/sim/grid.rs"]
mod tests;
