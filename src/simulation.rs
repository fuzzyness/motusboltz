//! Simulation state, fixed timestep, particle presets, and pressure sampling.

use macroquad::prelude::vec3;
use macroquad::rand::gen_range;

use crate::particle::Particle;
use crate::physics;
use crate::spatial_grid::SpatialGrid;

/// Radius of the vertical cylinder, measured to its physical side wall.
pub const CHAMBER_RADIUS: f32 = 2.0;
/// Distance between the two flat circular end caps.
pub const CHAMBER_HEIGHT: f32 = 4.0;
/// Side length of the current broad-phase grid bounds (height equals diameter).
pub const CHAMBER_DIAMETER: f32 = 2.0 * CHAMBER_RADIUS;
pub const PARTICLE_RADIUS: f32 = 0.18;
pub const PRESSURE_WINDOW_SECONDS: u32 = 1;
pub const PARTICLE_COUNT_PRESETS: [usize; 5] = [64, 256, 512, 1_024, 2_048];

/// Duration of a particle-particle collision highlight (simulation seconds).
pub const COLLISION_FLASH_DURATION: f32 = 0.01;

const DEFAULT_PARTICLE_COUNT: usize = PARTICLE_COUNT_PRESETS[0];
// Limit diameter to 80% of lattice spacing to avoid initial overlaps.
const PARTICLE_RADIUS_TO_LATTICE_SPACING: f32 = 0.4;
pub(crate) const INITIAL_SPEED_LIMIT: f32 = 1.5;

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
    collision_flashes: Vec<f32>,
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

    /// Creates a gas on a non-overlapping lattice clipped to the cylinder.
    ///
    /// # Panics
    ///
    /// Panics if `particle_count` is zero.
    pub fn with_particle_count(particle_count: usize) -> Self {
        assert!(particle_count > 0, "particle count must be positive");

        // Start with a regular lattice and retain only sites where an entire
        // sphere fits within both the curved wall and the flat end caps.
        // Increase the resolution until enough valid sites are available.
        let mut sites_per_axis: usize = 0;
        let (sites, particle_radius) = loop {
            sites_per_axis += 1;
            let spacing = CHAMBER_DIAMETER / sites_per_axis as f32;
            let radius = PARTICLE_RADIUS.min(spacing * PARTICLE_RADIUS_TO_LATTICE_SPACING);
            let radial_limit = CHAMBER_RADIUS - radius;
            let cap_limit = CHAMBER_HEIGHT / 2.0 - radius;
            let mut sites = Vec::new();

            let centered = |coordinate: usize| (coordinate as f32 + 0.5) * spacing - CHAMBER_RADIUS;

            for y in 0..sites_per_axis {
                for z in 0..sites_per_axis {
                    for x in 0..sites_per_axis {
                        let position = vec3(centered(x), centered(y), centered(z));
                        let radial_distance_squared =
                            position.x * position.x + position.z * position.z;

                        if radial_distance_squared <= radial_limit * radial_limit
                            && position.y.abs() <= cap_limit
                        {
                            sites.push(position);
                        }
                    }
                }
            }

            if sites.len() >= particle_count {
                break (sites, radius);
            }
        };

        let mut particles = Vec::with_capacity(particle_count);
        let site_count = sites.len();

        for index in 0..particle_count {
            // Spread partially filled lattices over the available volume.
            let site = ((2 * index + 1) * site_count) / (2 * particle_count);

            particles.push(Particle {
                position: sites[site],
                velocity: vec3(
                    gen_range(-INITIAL_SPEED_LIMIT, INITIAL_SPEED_LIMIT),
                    gen_range(-INITIAL_SPEED_LIMIT, INITIAL_SPEED_LIMIT),
                    gen_range(-INITIAL_SPEED_LIMIT, INITIAL_SPEED_LIMIT),
                ),
            });
        }

        Self {
            particles,
            collision_flashes: vec![0.0; particle_count],
            // The bounding cube still contains the cylinder; its empty corner
            // cells require no special handling in the broad-phase grid.
            grid: SpatialGrid::new(CHAMBER_DIAMETER, particle_radius),
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
        let radial_limit = CHAMBER_RADIUS - self.particle_radius;
        let cap_limit = CHAMBER_HEIGHT / 2.0 - self.particle_radius;

        while self.accumulator >= FIXED_DT {
            for particle in &mut self.particles {
                self.accumulated_wall_impulse += particle.update(FIXED_DT, radial_limit, cap_limit);
            }

            // Fade previous impacts in simulation time rather than wall time.
            for remaining in &mut self.collision_flashes {
                *remaining = (*remaining - FIXED_DT).max(0.0);
            }

            let flashes = &mut self.collision_flashes;
            stats.candidate_checks +=
                self.grid
                    .resolve_collisions(&mut self.particles, |first, second| {
                        flashes[first] = COLLISION_FLASH_DURATION;
                        flashes[second] = COLLISION_FLASH_DURATION;
                    });

            // Collision overlap correction can move particle centers past
            // the walls. Constrain them again before rendering or advancing.
            // This positional correction must not count as a wall impact.
            for particle in &mut self.particles {
                particle.constrain_position(radial_limit, cap_limit);
            }

            stats.steps += 1;
            self.pressure_sample_steps += 1;

            if self.pressure_sample_steps >= PRESSURE_SAMPLE_STEPS {
                let duration = self.pressure_sample_steps as f32 * FIXED_DT;
                self.pressure = Some(physics::pressure_from_wall_impulse(
                    self.accumulated_wall_impulse,
                    CHAMBER_RADIUS,
                    CHAMBER_HEIGHT,
                    duration,
                ));
                self.accumulated_wall_impulse = 0.0;
                self.pressure_sample_steps = 0;
            }

            self.accumulator -= FIXED_DT;
        }

        stats
    }

    /// Remaining collision highlight durations, one per particle.
    pub fn collision_flashes(&self) -> &[f32] {
        &self.collision_flashes
    }

    /// Returns the system's total translational kinetic energy.
    pub fn kinetic_energy(&self) -> f32 {
        physics::kinetic_energy(&self.particles)
    }

    /// Returns the gas temperature in reduced simulation units.
    pub fn temperature(&self) -> f32 {
        physics::temperature(&self.particles)
    }

    /// Sets the kinetic temperature by rescaling velocities around their mean.
    ///
    /// Clears the previous pressure measurement because its sampling window
    /// would otherwise mix wall impulses from different temperatures.
    pub fn set_temperature(&mut self, target_temperature: f32) {
        physics::rescale_thermal_velocities(&mut self.particles, target_temperature);

        self.accumulated_wall_impulse = 0.0;
        self.pressure_sample_steps = 0;
        self.pressure = None;
    }

    /// Returns the most recent full-window wall-pressure estimate.
    pub fn pressure(&self) -> Option<f32> {
        self.pressure
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::prelude::Vec3;

    const POSITION_TOLERANCE: f32 = 1e-5;
    const SECOND_PRESET_INDEX: usize = 1;
    const TARGET_TEST_TEMPERATURE: f32 = 2.0;
    const TEMPERATURE_TOLERANCE: f32 = 1e-4;
    const TEST_WALL_IMPULSE: f32 = 10.0;

    #[test]
    fn presets_fit_inside_chamber_without_overlapping() {
        for &count in &PARTICLE_COUNT_PRESETS {
            let simulation = Simulation::with_particle_count(count);
            let radial_limit = CHAMBER_RADIUS - simulation.particle_radius();
            let cap_limit = CHAMBER_HEIGHT / 2.0 - simulation.particle_radius();
            let minimum_distance = 2.0 * simulation.particle_radius();

            assert_eq!(simulation.particles.len(), count);
            for (index, particle) in simulation.particles.iter().enumerate() {
                let radial_distance = (particle.position.x * particle.position.x
                    + particle.position.z * particle.position.z)
                    .sqrt();
                assert!(radial_distance <= radial_limit + POSITION_TOLERANCE);
                assert!(particle.position.y.abs() <= cap_limit + POSITION_TOLERANCE);

                for other in &simulation.particles[index + 1..] {
                    let distance = particle.position.distance(other.position);
                    assert!(distance + POSITION_TOLERANCE >= minimum_distance);
                }
            }
        }
    }

    #[test]
    fn collision_correction_cannot_push_particles_through_the_wall() {
        let mut simulation = Simulation::with_particle_count(2);
        let radial_limit = CHAMBER_RADIUS - simulation.particle_radius();
        let separation = simulation.particle_radius();

        // Both centers are inside at the start, but separating the
        // overlapping pair would otherwise push the outer one past the wall.
        simulation.particles[0].position = vec3(radial_limit, 0.0, 0.0);
        simulation.particles[1].position = vec3(radial_limit - separation, 0.0, 0.0);
        simulation.particles[0].velocity = Vec3::ZERO;
        simulation.particles[1].velocity = vec3(INITIAL_SPEED_LIMIT, 0.0, 0.0);

        simulation.update(FIXED_DT);

        for particle in &simulation.particles {
            let radial_distance = (particle.position.x * particle.position.x
                + particle.position.z * particle.position.z)
                .sqrt();
            assert!(radial_distance <= radial_limit + POSITION_TOLERANCE);
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

    #[test]
    fn collision_flashes_start_at_impact_and_expire() {
        let mut simulation = Simulation::with_particle_count(2);
        let radius = simulation.particle_radius();

        simulation.particles[0].position = vec3(-radius / 2.0, 0.0, 0.0);
        simulation.particles[1].position = vec3(radius / 2.0, 0.0, 0.0);
        simulation.particles[0].velocity = vec3(INITIAL_SPEED_LIMIT, 0.0, 0.0);
        simulation.particles[1].velocity = vec3(-INITIAL_SPEED_LIMIT, 0.0, 0.0);

        simulation.update(FIXED_DT);
        assert_eq!(
            simulation.collision_flashes(),
            &[COLLISION_FLASH_DURATION; 2]
        );

        // They are now receding; the highlight must decay rather than restart.
        let decay_step = COLLISION_FLASH_DURATION.min(MAX_FRAME_TIME);
        simulation.update(decay_step);
        assert!(simulation.collision_flashes()[0] < COLLISION_FLASH_DURATION);
        assert!(simulation.collision_flashes()[1] < COLLISION_FLASH_DURATION);

        for particle in &mut simulation.particles {
            particle.velocity = Vec3::ZERO;
        }
        for _ in 0..(PHYSICS_STEPS_PER_SECOND / 2) {
            simulation.update(FIXED_DT);
        }
        assert!(
            simulation
                .collision_flashes()
                .iter()
                .all(|&remaining| remaining == 0.0)
        );

        simulation.set_particle_count(PARTICLE_COUNT_PRESETS[SECOND_PRESET_INDEX]);
        assert_eq!(
            simulation.collision_flashes().len(),
            simulation.particles.len()
        );
        assert!(
            simulation
                .collision_flashes()
                .iter()
                .all(|&remaining| remaining == 0.0)
        );
    }

    #[test]
    fn changing_temperature_resets_pressure_and_preserves_positions() {
        let mut simulation = Simulation::new();
        let original_positions: Vec<_> = simulation
            .particles
            .iter()
            .map(|particle| particle.position)
            .collect();
        simulation.accumulated_wall_impulse = TEST_WALL_IMPULSE;
        simulation.pressure_sample_steps = PRESSURE_SAMPLE_STEPS;
        simulation.pressure = Some(TEST_WALL_IMPULSE);

        simulation.set_temperature(TARGET_TEST_TEMPERATURE);

        assert!((simulation.temperature() - TARGET_TEST_TEMPERATURE).abs() < TEMPERATURE_TOLERANCE);
        assert_eq!(simulation.accumulated_wall_impulse, 0.0);
        assert_eq!(simulation.pressure_sample_steps, 0);
        assert!(simulation.pressure().is_none());
        assert!(
            simulation
                .particles
                .iter()
                .zip(original_positions)
                .all(|(particle, original)| particle.position == original)
        );
    }
}
