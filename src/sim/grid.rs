//! Uniform grid that finds nearby particle pairs, so collision checks cost
//! about one pass over the particles instead of one per pair.

use macroquad::prelude::Vec3;

use crate::sim::gas::{
    Contact,
    Particle,
    resolve_pair, //
};

/// End of a cell's particle list. Indices are 32-bit to halve the memory
/// the grid walks; the particle count is far below `u32::MAX`.
const EMPTY_CELL: u32 = u32::MAX;
/// Empty cells kept around the stored box, so every neighbor index is valid
/// without coordinate or boundary checks.
const HALO_WIDTH: usize = 1;
const BITS_PER_WORD: usize = u64::BITS as usize;

/// A cell holding at least one particle, with where to find its occupancy
/// bit: its row of cells along x, and its position in that row.
#[derive(Clone, Copy)]
struct OccupiedCell {
    index: u32,
    row: u32,
    x: u32,
}

/// Uniform grid that finds nearby particle pairs for collision checks.
///
/// Cells lie on a cubic lattice of side `chamber_size` centered on the
/// origin, each at least one particle diameter wide, so overlapping
/// particles lie in the same cell or in neighboring cells. Only the cells
/// over the box the gas can reach are stored, which keeps the grid small
/// enough to stay in cache; positions outside that box are clamped into its
/// edge cells, which still pairs every overlapping particle.
pub struct SpatialGrid {
    /// First particle in each cell, or `EMPTY_CELL`.
    heads: Vec<u32>,
    /// Next particle in the same cell, by particle index.
    next: Vec<u32>,
    occupied_cells: Vec<OccupiedCell>,
    /// One bit per cell, set when the cell is occupied, packed by row along
    /// x with a spare word at the end of each row. Most neighbors are empty,
    /// and reading their bits a row at a time avoids testing them one by one.
    row_bits: Vec<u64>,
    words_per_row: usize,
    cell_size: f32,
    chamber_half_size: f32,
    /// Lattice indices of the first and last stored cells along each axis.
    first_cell: [u32; 3],
    last_cell: [u32; 3],
    /// Stored cells plus the halo on both sides, along x and y.
    padded_x: usize,
    padded_y: usize,
    neighbor_offsets: [isize; 13],
    particle_radius: f32,
}

impl SpatialGrid {
    /// Builds a grid over the whole cube of side `chamber_size`, for tests.
    #[cfg(test)]
    pub fn new(chamber_size: f32, particle_radius: f32) -> Self {
        let half = chamber_size / 2.0;
        Self::covering(
            chamber_size,
            particle_radius,
            Vec3::new(-half, -half, -half),
            Vec3::new(half, half, half),
        )
    }

    /// Builds a grid on the lattice filling the cube of side `chamber_size`,
    /// storing only the cells that overlap the box from `lower` to `upper`,
    /// plus one cell of margin. Cell boundaries are those of the whole cube,
    /// so particles inside the box are paired in exactly the same order as
    /// on a grid storing every cell.
    ///
    /// # Panics
    ///
    /// Panics if the chamber size or particle radius is nonpositive or
    /// non-finite, or if the box is inverted.
    pub fn covering(chamber_size: f32, particle_radius: f32, lower: Vec3, upper: Vec3) -> Self {
        assert!(chamber_size.is_finite() && chamber_size > 0.0);
        assert!(particle_radius.is_finite() && particle_radius > 0.0);
        assert!((0..3).all(|axis| lower[axis] <= upper[axis]));

        let diameter = 2.0 * particle_radius;
        let lattice_cells = ((chamber_size / diameter).floor() as usize).max(1);
        let cell_size = chamber_size / lattice_cells as f32;
        let chamber_half_size = chamber_size / 2.0;
        let lattice_index = |coordinate: f32| {
            ((coordinate + chamber_half_size) / cell_size)
                .floor()
                .clamp(0.0, (lattice_cells - 1) as f32) as usize
        };
        let mut first_cell = [0; 3];
        let mut last_cell = [0; 3];
        let mut cells = [0; 3];
        for axis in 0..3 {
            let first = lattice_index(lower[axis]).saturating_sub(1);
            let last = (lattice_index(upper[axis]) + 1).min(lattice_cells - 1);
            first_cell[axis] = u32::try_from(first).expect("grid too large");
            last_cell[axis] = u32::try_from(last).expect("grid too large");
            cells[axis] = last - first + 1;
        }

        let padded_x = cells[0] + 2 * HALO_WIDTH;
        let padded_y = cells[1] + 2 * HALO_WIDTH;
        let padded_z = cells[2] + 2 * HALO_WIDTH;
        let row = padded_x as isize;
        let plane = row * padded_y as isize;
        // Half of the 26 neighbors, so each pair of cells is visited once:
        // +x, then the row above, then the 9 cells of the next plane.
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
        let words_per_row = padded_x.div_ceil(BITS_PER_WORD) + 1;

        Self {
            heads: vec![EMPTY_CELL; padded_x * padded_y * padded_z],
            next: Vec::new(),
            occupied_cells: Vec::new(),
            row_bits: vec![0; padded_y * padded_z * words_per_row],
            words_per_row,
            cell_size,
            chamber_half_size,
            first_cell,
            last_cell,
            padded_x,
            padded_y,
            neighbor_offsets,
            particle_radius,
        }
    }

    /// Stored cell along `axis` for a coordinate, clamped into the box.
    /// Truncation equals rounding down here: the conversion turns negative
    /// values (and NaN) into zero, which the clamp would give anyway.
    fn axis_cell(&self, coordinate: f32, axis: usize) -> usize {
        let index = ((coordinate + self.chamber_half_size) / self.cell_size) as u32;
        (index.max(self.first_cell[axis]).min(self.last_cell[axis]) - self.first_cell[axis])
            as usize
    }

    /// The cell holding `position`: its index, its row along x and its
    /// place in that row, all counting the halo.
    fn locate(&self, position: Vec3) -> OccupiedCell {
        let x = self.axis_cell(position.x, 0) + HALO_WIDTH;
        let y = self.axis_cell(position.y, 1) + HALO_WIDTH;
        let z = self.axis_cell(position.z, 2) + HALO_WIDTH;
        let row = z * self.padded_y + y;
        OccupiedCell {
            index: (row * self.padded_x + x) as u32,
            row: row as u32,
            x: x as u32,
        }
    }

    #[cfg(test)]
    fn cell_index(&self, position: Vec3) -> usize {
        self.locate(position).index as usize
    }

    fn rebuild(&mut self, particles: &[Particle]) {
        // Flat linked lists avoid per-cell allocations; clear only occupied
        // cells. Every bit in a cleared word belongs to an occupied cell.
        self.next.resize(particles.len(), EMPTY_CELL);
        for &cell in &self.occupied_cells {
            self.heads[cell.index as usize] = EMPTY_CELL;
            let word = self.bit_word(cell);
            self.row_bits[word] = 0;
        }
        self.occupied_cells.clear();

        for (particle_index, particle) in particles.iter().enumerate() {
            let cell = self.locate(particle.position);
            let index = cell.index as usize;
            if self.heads[index] == EMPTY_CELL {
                self.occupied_cells.push(cell);
                let word = self.bit_word(cell);
                self.row_bits[word] |= 1 << (cell.x as usize % BITS_PER_WORD);
            }
            self.next[particle_index] = self.heads[index];
            self.heads[index] = particle_index as u32;
        }
    }

    /// Index of the word in `row_bits` holding a cell's occupancy bit.
    fn bit_word(&self, cell: OccupiedCell) -> usize {
        cell.row as usize * self.words_per_row + cell.x as usize / BITS_PER_WORD
    }

    /// Which of a cell's 13 forward neighbors are occupied, as bits in
    /// `neighbor_offsets` order.
    fn occupied_neighbors(&self, cell: OccupiedCell) -> u16 {
        // Bits 0, 1 and 2 of `three` are the cells at x - 1, x and x + 1 in
        // a row; the halo keeps x - 1 at zero or more.
        let start = cell.x as usize - 1;
        let (word, shift) = (start / BITS_PER_WORD, start % BITS_PER_WORD);
        let three = |row: usize| {
            let base = row * self.words_per_row + word;
            let mut bits = self.row_bits[base] >> shift;
            // Only rows wider than a word can have the three cells straddle
            // two words.
            if shift > BITS_PER_WORD - 3 {
                bits |= self.row_bits[base + 1] << (BITS_PER_WORD - shift);
            }
            (bits & 0b111) as u16
        };
        let row = cell.row as usize;
        let plane = self.padded_y;
        (three(row) >> 2)
            | three(row + 1) << 1
            | three(row + plane - 1) << 4
            | three(row + plane) << 7
            | three(row + plane + 1) << 10
    }

    /// Resolves nearby pairs, invoking `on_contact` for every pair that
    /// touched: `Contact::Overlap` when the pair was only pushed apart,
    /// `Contact::Impact` when it also bounced.
    ///
    /// Each pair is checked once. Returns the total number of candidate
    /// pairs checked.
    pub fn resolve_collisions<F>(&mut self, particles: &mut [Particle], mut on_contact: F) -> usize
    where
        F: FnMut(usize, usize, Contact),
    {
        self.rebuild(particles);

        let mut candidate_checks = 0;
        let mut check_pair = |first: u32, second: u32| {
            let (first, second) = (first as usize, second as usize);
            let contact = Self::resolve_index_pair(particles, first, second, self.particle_radius);
            if contact != Contact::Apart {
                on_contact(first, second, contact);
            }
            candidate_checks += 1;
        };

        // Offsets preserve the original forward-neighbor collision order.
        for &cell in &self.occupied_cells {
            let cell_index = cell.index as usize;
            let mut first = self.heads[cell_index];
            while first != EMPTY_CELL {
                let mut second = self.next[first as usize];
                while second != EMPTY_CELL {
                    check_pair(first, second);
                    second = self.next[second as usize];
                }
                first = self.next[first as usize];
            }
            // Visit only the occupied neighbors, lowest bit (first offset)
            // first.
            let mut neighbors = self.occupied_neighbors(cell);
            while neighbors != 0 {
                let bit = neighbors.trailing_zeros() as usize;
                neighbors &= neighbors - 1;
                let neighbor = (cell_index as isize + self.neighbor_offsets[bit]) as usize;
                let mut first = self.heads[cell_index];
                while first != EMPTY_CELL {
                    let mut second = self.heads[neighbor];
                    while second != EMPTY_CELL {
                        check_pair(first, second);
                        second = self.next[second as usize];
                    }
                    first = self.next[first as usize];
                }
            }
        }

        candidate_checks
    }

    /// Resolves one pair on copies, writing them back only if they
    /// touched, which is rare. Copying avoids borrowing two elements of the
    /// slice at once, and with it a branch on which index is lower.
    fn resolve_index_pair(
        particles: &mut [Particle],
        first: usize,
        second: usize,
        radius: f32,
    ) -> Contact {
        let (mut a, mut b) = (particles[first], particles[second]);
        let contact = resolve_pair(&mut a, &mut b, radius);
        if contact != Contact::Apart {
            particles[first] = a;
            particles[second] = b;
        }
        contact
    }
}

#[cfg(test)]
#[path = "../../tests/unit/sim/grid.rs"]
mod tests;
