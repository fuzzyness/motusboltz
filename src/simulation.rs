//! Simulation state, fixed timestep, particle presets, and pressure sampling.

use macroquad::prelude::vec3;
use macroquad::rand::gen_range;

use crate::particle::Particle;
use crate::physics;
use crate::spatial_grid::SpatialGrid;

pub const CHAMBER_SIZE: f32 = 4.0;
pub const PARTICLE_RADIUS: f32 = 0.18;
pub const PRESSURE_WINDOW_SECONDS: u32 = 1;
pub const PARTICLE_COUNT_PRESETS: [usize; 5] = [64, 256, 512, 1_024, 2_048];

const DEFAULT_PARTICLE_COUNT: usize = PARTICLE_COUNT_PRESETS[0];
// Limit diameter to 80% of lattice spacing to avoid initial overlaps.
const PARTICLE_RADIUS_TO_LATTICE_SPACING: f32 = 0.4;
const INITIAL_SPEED_LIMIT: f32 = 1.5;

const PHYSICS_STEPS_PER_SECOND: u32 = 120;
const FIXED_DT: f32 = 1.0 / PHYSICS_STEPS_PER_SECOND as f32;
const PRESSURE_SAMPLE_STEPS: u32 = PHYSICS_STEPS_PER_SECOND * PRESSURE_WINDOW_SECONDS;
const MAX_FRAME_TIME: f32 = 0.05;

/// Physics work completed during one rendered frame.
#[derive(Clone, Copy, Default)]
pub struct PhysicsUpdateStats {
    pub steps: u32,
    pub candidate_checks: usize,
}

/// Gas state and its associated collision grid and sampling accumulators.
pub struct Simulation {
    pub particles: Vec<Particle>,
    grid: SpatialGrid,
    particle_radius: f32,
    accumulator: f32,
    accumulated_wall_impulse: f32,
    pressure_sample_steps: u32,
    pressure: Option<f32>,
}

impl Simulation {
    /// Creates the default particle-count preset.
    pub fn new() -> Self {
        Self::with_particle_count(DEFAULT_PARTICLE_COUNT)
    }

    /// Creates a gas with a non-overlapping initial cubic lattice.
    ///
    /// # Panics
    ///
    /// Panics if `particle_count` is zero.
    pub fn with_particle_count(particle_count: usize) -> Self {
        assert!(particle_count > 0, "particle count must be positive");

        // A cubic lattice provides a collision-free initial distribution.
        let mut sites_per_axis: usize = 1;
        while sites_per_axis.pow(3) < particle_count {
            sites_per_axis += 1;
        }

        let lattice_capacity = sites_per_axis.pow(3);
        let lattice_spacing = CHAMBER_SIZE / sites_per_axis as f32;
        let particle_radius =
            PARTICLE_RADIUS.min(lattice_spacing * PARTICLE_RADIUS_TO_LATTICE_SPACING);

        let mut particles = Vec::with_capacity(particle_count);

        for index in 0..particle_count {
            // Distribute partially filled lattices across the chamber.
            let site = ((2 * index + 1) * lattice_capacity) / (2 * particle_count);
            let x = site % sites_per_axis;
            let y = (site / sites_per_axis) % sites_per_axis;
            let z = site / (sites_per_axis * sites_per_axis);

            let centered = |coordinate: usize| {
                (coordinate as f32 + 0.5) * lattice_spacing - CHAMBER_SIZE / 2.0
            };

            particles.push(Particle {
                position: vec3(centered(x), centered(y), centered(z)),
                velocity: vec3(
                    gen_range(-INITIAL_SPEED_LIMIT, INITIAL_SPEED_LIMIT),
                    gen_range(-INITIAL_SPEED_LIMIT, INITIAL_SPEED_LIMIT),
                    gen_range(-INITIAL_SPEED_LIMIT, INITIAL_SPEED_LIMIT),
                ),
            });
        }

        Self {
            particles,
            grid: SpatialGrid::new(CHAMBER_SIZE, particle_radius),
            particle_radius,
            accumulator: 0.0,
            accumulated_wall_impulse: 0.0,
            pressure_sample_steps: 0,
            pressure: None,
        }
    }

    /// Restarts the gas only if the requested particle count differs.
    pub fn set_particle_count(&mut self, particle_count: usize) {
        if self.particles.len() != particle_count {
            // Reset the collision grid and pressure sampling with the gas.
            *self = Self::with_particle_count(particle_count);
        }
    }

    /// Returns the hard-sphere radius for this particle count.
    pub fn particle_radius(&self) -> f32 {
        self.particle_radius
    }

    /// Advances the fixed-timestep simulation and returns physics counters.
    ///
    /// Elapsed frame time is capped to avoid unbounded catch-up work after
    /// a slow frame or when the application resumes from suspension.
    pub fn update(&mut self, frame_time: f32) -> PhysicsUpdateStats {
        let mut stats = PhysicsUpdateStats::default();
        self.accumulator += frame_time.min(MAX_FRAME_TIME);
        let wall_limit = CHAMBER_SIZE / 2.0 - self.particle_radius;

        while self.accumulator >= FIXED_DT {
            for particle in &mut self.particles {
                self.accumulated_wall_impulse += particle.update(FIXED_DT, wall_limit);
            }

            stats.candidate_checks += self.grid.resolve_collisions(&mut self.particles);
            stats.steps += 1;
            self.pressure_sample_steps += 1;

            if self.pressure_sample_steps >= PRESSURE_SAMPLE_STEPS {
                let duration = self.pressure_sample_steps as f32 * FIXED_DT;
                self.pressure = Some(physics::pressure_from_wall_impulse(
                    self.accumulated_wall_impulse,
                    CHAMBER_SIZE,
                    duration,
                ));
                self.accumulated_wall_impulse = 0.0;
                self.pressure_sample_steps = 0;
            }

            self.accumulator -= FIXED_DT;
        }

        stats
    }

    /// Returns the system's total translational kinetic energy.
    pub fn kinetic_energy(&self) -> f32 {
        physics::kinetic_energy(&self.particles)
    }

    /// Returns the gas temperature in reduced simulation units.
    pub fn temperature(&self) -> f32 {
        physics::temperature(&self.particles)
    }

    /// Returns the most recent full-window wall-pressure estimate.
    pub fn pressure(&self) -> Option<f32> {
        self.pressure
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POSITION_TOLERANCE: f32 = 1e-5;
    const SECOND_PRESET_INDEX: usize = 1;

    #[test]
    fn presets_fit_inside_chamber_without_overlapping() {
        for &count in &PARTICLE_COUNT_PRESETS {
            let simulation = Simulation::with_particle_count(count);
            let limit = CHAMBER_SIZE / 2.0 - simulation.particle_radius();
            let minimum_distance = 2.0 * simulation.particle_radius();

            assert_eq!(simulation.particles.len(), count);
            for (index, particle) in simulation.particles.iter().enumerate() {
                assert!(particle.position.x.abs() <= limit + POSITION_TOLERANCE);
                assert!(particle.position.y.abs() <= limit + POSITION_TOLERANCE);
                assert!(particle.position.z.abs() <= limit + POSITION_TOLERANCE);

                for other in &simulation.particles[index + 1..] {
                    let distance = particle.position.distance(other.position);
                    assert!(distance + POSITION_TOLERANCE >= minimum_distance);
                }
            }
        }
    }

    #[test]
    fn changing_count_reinitializes_the_simulation() {
        let mut simulation = Simulation::new();
        let next_count = PARTICLE_COUNT_PRESETS[SECOND_PRESET_INDEX];

        simulation.update(MAX_FRAME_TIME);
        simulation.set_particle_count(next_count);

        assert_eq!(simulation.particles.len(), next_count);
        assert_eq!(simulation.accumulator, 0.0);
        assert_eq!(simulation.accumulated_wall_impulse, 0.0);
        assert_eq!(simulation.pressure_sample_steps, 0);
        assert!(simulation.pressure().is_none());
    }
}
