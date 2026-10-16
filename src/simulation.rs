//! Simulation state, fixed timestep, particle presets, and pressure sampling.

use macroquad::prelude::vec3;
use macroquad::rand::gen_range;

use crate::crankshaft::Crankshaft;
use crate::particle::{
    Particle,
    WallBounds,
    WallImpulses, //
};
use crate::physics;
use crate::piston::Piston;
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

/// Gas state, piston mechanism, collision grid, and measurement accumulators.
pub struct Simulation {
    pub particles: Vec<Particle>,
    collision_flashes: Vec<f32>,
    grid: SpatialGrid,
    piston: Piston,
    crankshaft: Option<Crankshaft>,
    particle_radius: f32,
    accumulator: f32,
    accumulated_wall_impulses: WallImpulses,
    pressure_sample_steps: u32,
    sampled_height_sum: f32,
    piston_work_by_gas: f32,
    pressures: Option<physics::WallPressures>,
}

impl Simulation {
    /// Creates the default particle-count preset.
    pub fn new() -> Self {
        Self::with_particle_count(DEFAULT_PARTICLE_COUNT)
    }

    /// Creates a non-overlapping gas lattice inside the full cylinder.
    ///
    /// # Panics
    ///
    /// Panics if `particle_count` is zero.
    pub fn with_particle_count(particle_count: usize) -> Self {
        Self::with_particle_count_below(particle_count, CHAMBER_HEIGHT / 2.0)
    }

    /// Initializes all sphere centers below the current piston face.
    ///
    /// This also supports resetting a gas while the crank is mid-stroke.
    fn with_particle_count_below(particle_count: usize, piston_top: f32) -> Self {
        assert!(particle_count > 0, "particle count must be positive");
        assert!(piston_top.is_finite());
        assert!((0.0..=CHAMBER_HEIGHT / 2.0).contains(&piston_top));

        // Retain lattice sites where each sphere fits within the curved wall,
        // the fixed bottom cap, and the current piston face.
        // Increase the resolution until enough valid sites are available.
        let mut sites_per_axis: usize = 0;
        let (sites, particle_radius) = loop {
            sites_per_axis += 1;
            let spacing = CHAMBER_DIAMETER / sites_per_axis as f32;
            let radius = PARTICLE_RADIUS.min(spacing * PARTICLE_RADIUS_TO_LATTICE_SPACING);
            let radial_limit = CHAMBER_RADIUS - radius;
            let bottom_limit = -CHAMBER_HEIGHT / 2.0 + radius;
            let top_limit = piston_top - radius;
            let mut sites = Vec::new();

            let centered = |coordinate: usize| (coordinate as f32 + 0.5) * spacing - CHAMBER_RADIUS;

            for y in 0..sites_per_axis {
                for z in 0..sites_per_axis {
                    for x in 0..sites_per_axis {
                        let position = vec3(centered(x), centered(y), centered(z));
                        let radial_distance_squared =
                            position.x * position.x + position.z * position.z;

                        if radial_distance_squared <= radial_limit * radial_limit
                            && (bottom_limit..=top_limit).contains(&position.y)
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
            piston: Piston::new(),
            crankshaft: None,
            particle_radius,
            accumulator: 0.0,
            accumulated_wall_impulses: WallImpulses::default(),
            pressure_sample_steps: 0,
            sampled_height_sum: 0.0,
            piston_work_by_gas: 0.0,
            pressures: None,
        }
    }

    /// Rebuilds the gas without changing the piston mode or crank state.
    ///
    /// New particles are laid out below the current piston, so a running
    /// crankshaft need not be stopped or displaced when the count changes.
    /// Gas measurements and collision flashes begin a fresh sampling window.
    pub fn set_particle_count(&mut self, particle_count: usize) {
        if self.particles.len() == particle_count {
            return;
        }

        let piston_top = self.piston_position().clamp(0.0, CHAMBER_HEIGHT / 2.0);
        let piston = std::mem::replace(&mut self.piston, Piston::new());
        let crankshaft = self.crankshaft.take();

        *self = Self::with_particle_count_below(particle_count, piston_top);
        self.piston = piston;
        self.crankshaft = crankshaft;
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
        let bottom_limit = -CHAMBER_HEIGHT / 2.0 + self.particle_radius;

        while self.accumulator >= FIXED_DT {
            // Both modes move the wall before testing crossings. A flywheel
            // advances under external load; a manual piston follows its target.
            if let Some(crank) = &mut self.crankshaft {
                crank.advance(FIXED_DT);
            } else {
                self.piston.advance(FIXED_DT);
            }

            let mut bounds = WallBounds {
                radial: radial_limit,
                bottom: bottom_limit,
                top: self.piston_position() - self.particle_radius,
                top_velocity: self.piston_velocity(),
            };

            for particle in &mut self.particles {
                // A collision changes crank speed immediately. Later molecules
                // in the same physics step must see that updated wall speed.
                let impulses = if let Some(crank) = &mut self.crankshaft {
                    bounds.top_velocity = crank.piston_velocity();
                    let impulses = particle.update_with_top_inverse_mass(
                        FIXED_DT,
                        bounds,
                        crank.top_inverse_mass(),
                    );
                    self.piston_work_by_gas += crank.apply_top_impulse(impulses.top);
                    impulses
                } else {
                    let impulses = particle.update(FIXED_DT, bounds);
                    self.piston_work_by_gas += impulses.top * bounds.top_velocity;
                    impulses
                };
                self.accumulated_wall_impulses += impulses;
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
                particle.constrain_position(bounds);
            }

            stats.steps += 1;
            self.pressure_sample_steps += 1;
            self.sampled_height_sum += self.piston_height();

            if self.pressure_sample_steps >= PRESSURE_SAMPLE_STEPS {
                let duration = self.pressure_sample_steps as f32 * FIXED_DT;
                // Average height gives the correct side-area-time integral.
                let mean_height = self.sampled_height_sum / self.pressure_sample_steps as f32;
                self.pressures = Some(physics::pressures_from_wall_impulses(
                    self.accumulated_wall_impulses,
                    CHAMBER_RADIUS,
                    mean_height,
                    duration,
                ));
                self.accumulated_wall_impulses = WallImpulses::default();
                self.pressure_sample_steps = 0;
                self.sampled_height_sum = 0.0;
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

        self.reset_pressure_sampling();
    }

    /// Changes the target volume without teleporting the moving piston.
    pub fn set_volume_fraction(&mut self, fraction: f32) {
        self.disengage_crankshaft();
        self.piston.set_volume_fraction(fraction);
        self.reset_pressure_sampling();
    }

    /// Selected manual volume for synchronizing the UI after gas resets.
    pub fn requested_volume_fraction(&self) -> f32 {
        self.piston.target_volume_fraction()
    }

    /// Current world-space Y coordinate of the gas-facing piston surface.
    pub fn piston_position(&self) -> f32 {
        self.crankshaft
            .as_ref()
            .map_or_else(|| self.piston.position(), Crankshaft::piston_position)
    }

    /// Measured velocity of the piston during the most recent physics step.
    pub fn piston_velocity(&self) -> f32 {
        self.crankshaft
            .as_ref()
            .map_or_else(|| self.piston.velocity(), Crankshaft::piston_velocity)
    }

    /// Actual, not requested, chamber volume.
    pub fn volume(&self) -> f32 {
        std::f32::consts::PI * CHAMBER_RADIUS.powi(2) * self.piston_height()
    }

    fn piston_height(&self) -> f32 {
        self.piston_position() + CHAMBER_HEIGHT / 2.0
    }

    /// Switches between prescribed volume control and a gas-driven crankshaft.
    pub fn toggle_crankshaft(&mut self) {
        if self.crankshaft.is_some() {
            self.disengage_crankshaft();
        } else {
            self.crankshaft = Some(Crankshaft::at_height(self.piston_position()));
        }
        self.reset_pressure_sampling();
    }

    fn disengage_crankshaft(&mut self) {
        if let Some(crank) = self.crankshaft.take() {
            self.piston.park_at(crank.piston_position());
        }
    }

    /// Spins the crank from rest using energy supplied by an external starter.
    pub fn start_crankshaft(&mut self) {
        if let Some(crank) = &mut self.crankshaft {
            crank.start();
        }
    }

    pub fn crankshaft(&self) -> Option<&Crankshaft> {
        self.crankshaft.as_ref()
    }

    fn reset_pressure_sampling(&mut self) {
        self.accumulated_wall_impulses = WallImpulses::default();
        self.pressure_sample_steps = 0;
        self.sampled_height_sum = 0.0;
        self.pressures = None;
    }

    /// Cumulative mechanical work done by the gas on the driven piston.
    pub fn piston_work_by_gas(&self) -> f32 {
        self.piston_work_by_gas
    }

    /// Returns sampled pressures; changing volume uses mean wall area in the window.
    pub fn pressures(&self) -> Option<physics::WallPressures> {
        self.pressures
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
    const COMPRESSED_VOLUME_FRACTION: f32 = 0.5;
    const BOUNDARY_TEST_VOLUME_FRACTION: f32 = 0.98;
    const FULL_VOLUME_FRACTION: f32 = 1.0;
    // Supports piston speeds down to about 0.02 units/s at 120 Hz.
    const MAX_PISTON_TEST_STEPS: usize = 12_000;
    const PISTON_TEST_PARTICLE_COUNT: usize = 2;
    const ENERGY_BALANCE_ABSOLUTE_TOLERANCE: f32 = 1e-4;
    const ENERGY_BALANCE_RELATIVE_TOLERANCE: f32 = 1e-3;

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
        assert_eq!(
            simulation.accumulated_wall_impulses,
            WallImpulses::default()
        );
        assert_eq!(simulation.pressure_sample_steps, 0);
        assert_eq!(simulation.sampled_height_sum, 0.0);
        assert!(simulation.pressures().is_none());
    }

    #[test]
    fn changing_count_keeps_running_crankshaft_and_particles_below_piston() {
        const TEST_PISTON_TOP: f32 = 1.0;
        const TEST_PARTICLE_COUNT: usize = 512;

        let mut simulation = Simulation::new();
        simulation.piston.park_at(TEST_PISTON_TOP);
        simulation.toggle_crankshaft();
        simulation.start_crankshaft();
        simulation.update(FIXED_DT);

        let top_before = simulation.piston_position();
        let speed_before = simulation.crankshaft().unwrap().angular_velocity();
        simulation.set_particle_count(TEST_PARTICLE_COUNT);

        assert_eq!(simulation.particles.len(), TEST_PARTICLE_COUNT);
        assert!(simulation.crankshaft().is_some());
        assert!((simulation.piston_position() - top_before).abs() < POSITION_TOLERANCE);
        assert_eq!(
            simulation.crankshaft().unwrap().angular_velocity(),
            speed_before
        );
        assert!(simulation.particles.iter().all(|particle| {
            particle.position.y + simulation.particle_radius()
                <= simulation.piston_position() + POSITION_TOLERANCE
        }));
    }

    #[test]
    fn changing_count_at_bottom_dead_center_keeps_gas_below_piston() {
        const BOTTOM_DEAD_CENTER: f32 = 0.0;
        let count = *PARTICLE_COUNT_PRESETS.last().unwrap();
        let mut simulation = Simulation::new();
        simulation.piston.park_at(BOTTOM_DEAD_CENTER);
        simulation.toggle_crankshaft();

        simulation.set_particle_count(count);

        assert!(simulation.crankshaft().is_some());
        assert_eq!(simulation.particles.len(), count);
        assert!(simulation.particles.iter().all(|particle| {
            particle.position.y + simulation.particle_radius()
                <= simulation.piston_position() + POSITION_TOLERANCE
        }));
    }

    #[test]
    fn changing_count_preserves_manual_piston_position_and_target() {
        const TARGET_VOLUME: f32 = 0.75;
        const TEST_PARTICLE_COUNT: usize = 256;

        let mut simulation = Simulation::new();
        simulation.set_volume_fraction(TARGET_VOLUME);
        simulation.update(MAX_FRAME_TIME);
        let top_before = simulation.piston_position();

        simulation.set_particle_count(TEST_PARTICLE_COUNT);

        assert!(simulation.crankshaft().is_none());
        assert!((simulation.piston_position() - top_before).abs() < POSITION_TOLERANCE);
        assert!(
            (simulation.requested_volume_fraction() - TARGET_VOLUME).abs() < POSITION_TOLERANCE
        );
        assert!(simulation.particles.iter().all(|particle| {
            particle.position.y + simulation.particle_radius()
                <= simulation.piston_position() + POSITION_TOLERANCE
        }));
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
    fn pressure_window_records_side_impulse_separately() {
        let mut simulation = Simulation::with_particle_count(1);
        let radial_limit = CHAMBER_RADIUS - simulation.particle_radius();
        let distance_from_wall = INITIAL_SPEED_LIMIT * FIXED_DT / 2.0;
        simulation.particles[0].position = vec3(radial_limit - distance_from_wall, 0.0, 0.0);
        simulation.particles[0].velocity = vec3(INITIAL_SPEED_LIMIT, 0.0, 0.0);

        for _ in 0..PRESSURE_SAMPLE_STEPS {
            simulation.update(FIXED_DT);
        }

        let pressures = simulation
            .pressures()
            .expect("pressure sample should be ready");
        assert!(pressures.side > 0.0);
        assert!(pressures.total > 0.0);
        assert_eq!(pressures.top, 0.0);
        assert_eq!(pressures.bottom, 0.0);
    }

    #[test]
    fn pressure_window_records_cap_impulse_separately() {
        let mut simulation = Simulation::with_particle_count(1);
        let cap_limit = CHAMBER_HEIGHT / 2.0 - simulation.particle_radius();
        let distance_from_cap = INITIAL_SPEED_LIMIT * FIXED_DT / 2.0;
        simulation.particles[0].position = vec3(0.0, cap_limit - distance_from_cap, 0.0);
        simulation.particles[0].velocity = vec3(0.0, INITIAL_SPEED_LIMIT, 0.0);

        for _ in 0..PRESSURE_SAMPLE_STEPS {
            simulation.update(FIXED_DT);
        }

        let pressures = simulation
            .pressures()
            .expect("pressure sample should be ready");
        assert!(pressures.top > 0.0);
        assert!(pressures.total > 0.0);
        assert_eq!(pressures.side, 0.0);
        assert_eq!(pressures.bottom, 0.0);
    }

    #[test]
    fn changing_temperature_resets_pressure_and_preserves_positions() {
        let mut simulation = Simulation::new();
        let original_positions: Vec<_> = simulation
            .particles
            .iter()
            .map(|particle| particle.position)
            .collect();
        simulation.accumulated_wall_impulses = WallImpulses {
            side: TEST_WALL_IMPULSE,
            top: TEST_WALL_IMPULSE,
            bottom: TEST_WALL_IMPULSE,
        };
        simulation.pressure_sample_steps = PRESSURE_SAMPLE_STEPS;
        simulation.pressures = Some(physics::pressures_from_wall_impulses(
            simulation.accumulated_wall_impulses,
            CHAMBER_RADIUS,
            CHAMBER_HEIGHT,
            FIXED_DT,
        ));

        simulation.set_temperature(TARGET_TEST_TEMPERATURE);

        assert!((simulation.temperature() - TARGET_TEST_TEMPERATURE).abs() < TEMPERATURE_TOLERANCE);
        assert_eq!(
            simulation.accumulated_wall_impulses,
            WallImpulses::default()
        );
        assert_eq!(simulation.pressure_sample_steps, 0);
        assert_eq!(simulation.sampled_height_sum, 0.0);
        assert!(simulation.pressures().is_none());
        assert!(
            simulation
                .particles
                .iter()
                .zip(original_positions)
                .all(|(particle, original)| particle.position == original)
        );
    }

    /// Advances one physics step at a time until the piston reaches its target.
    fn move_piston_to(simulation: &mut Simulation, volume_fraction: f32) {
        let full_volume = std::f32::consts::PI * CHAMBER_RADIUS.powi(2) * CHAMBER_HEIGHT;
        simulation.set_volume_fraction(volume_fraction);

        for _ in 0..MAX_PISTON_TEST_STEPS {
            if (simulation.volume() / full_volume - volume_fraction).abs() < POSITION_TOLERANCE {
                return;
            }

            simulation.update(FIXED_DT);
        }

        panic!("piston did not reach {volume_fraction:.2} of full volume");
    }

    /// For an adiabatic gas, the change in kinetic energy equals minus the
    /// work done by the gas. Compare differences because work is cumulative.
    fn assert_energy_balance(simulation: &Simulation, initial_energy: f32, initial_work: f32) {
        let energy_change = simulation.kinetic_energy() - initial_energy;
        let work_by_gas = simulation.piston_work_by_gas() - initial_work;
        let error = (energy_change + work_by_gas).abs();
        let scale = energy_change.abs().max(work_by_gas.abs());
        let tolerance =
            ENERGY_BALANCE_ABSOLUTE_TOLERANCE + ENERGY_BALANCE_RELATIVE_TOLERANCE * scale;

        assert!(
            error <= tolerance,
            "energy balance failed: delta E = {energy_change}, W = {work_by_gas}, error = {error}, tolerance = {tolerance}",
        );
    }

    #[test]
    fn piston_compression_changes_volume_and_keeps_particles_inside() {
        let mut simulation = Simulation::with_particle_count(PARTICLE_COUNT_PRESETS[0]);
        move_piston_to(&mut simulation, BOUNDARY_TEST_VOLUME_FRACTION);

        let top_limit = simulation.piston_position() - simulation.particle_radius();
        assert!(
            simulation
                .particles
                .iter()
                .all(|particle| particle.position.y <= top_limit + POSITION_TOLERANCE)
        );
    }

    #[test]
    fn adiabatic_compression_and_expansion_conserve_energy() {
        let mut simulation = Simulation::with_particle_count(PISTON_TEST_PARTICLE_COUNT);
        let cap_limit = CHAMBER_HEIGHT / 2.0 - simulation.particle_radius();

        // Start at rest with a particle touching the piston. The descending
        // piston must do work, so a trivial zero-work cycle cannot pass.
        simulation.particles[0].position = vec3(0.0, cap_limit, 0.0);
        simulation.particles[1].position = vec3(0.0, -cap_limit, 0.0);
        for particle in &mut simulation.particles {
            particle.velocity = Vec3::ZERO;
        }

        let initial_energy = simulation.kinetic_energy();
        let initial_work = simulation.piston_work_by_gas();

        move_piston_to(&mut simulation, COMPRESSED_VOLUME_FRACTION);
        assert!(simulation.kinetic_energy() > initial_energy);
        assert!(simulation.piston_work_by_gas() < initial_work);
        assert_energy_balance(&simulation, initial_energy, initial_work);

        move_piston_to(&mut simulation, FULL_VOLUME_FRACTION);
        assert_energy_balance(&simulation, initial_energy, initial_work);
    }

    #[test]
    fn switching_particle_count_preserves_requested_piston_volume() {
        const TARGET_VOLUME_FRACTION: f32 = 0.75;
        let mut simulation = Simulation::new();
        simulation.set_volume_fraction(TARGET_VOLUME_FRACTION);
        simulation.set_particle_count(PARTICLE_COUNT_PRESETS[SECOND_PRESET_INDEX]);

        assert!(
            (simulation.piston.target_volume_fraction() - TARGET_VOLUME_FRACTION).abs()
                < POSITION_TOLERANCE
        );
    }

    #[test]
    fn crankshaft_impact_conserves_gas_flywheel_and_load_energy() {
        const TEST_PISTON_HEIGHT: f32 = 1.0;
        const TEST_GAS_SPEED: f32 = 0.6;
        const BALANCE_TOLERANCE: f32 = 1e-3;

        let mut simulation = Simulation::with_particle_count(1);
        simulation.piston.park_at(TEST_PISTON_HEIGHT);
        simulation.toggle_crankshaft();
        simulation.start_crankshaft();

        simulation.particles[0].position =
            vec3(0.0, TEST_PISTON_HEIGHT - simulation.particle_radius(), 0.0);
        simulation.particles[0].velocity = vec3(0.0, TEST_GAS_SPEED, 0.0);

        let gas_before = simulation.kinetic_energy();
        let shaft_before = simulation.crankshaft().unwrap().kinetic_energy();
        let starter_before = simulation.crankshaft().unwrap().starter_work();
        simulation.update(FIXED_DT);
        let crank = simulation.crankshaft().unwrap();
        let energy_balance =
            simulation.kinetic_energy() + crank.kinetic_energy() + crank.load_energy()
                - (crank.starter_work() - starter_before)
                - gas_before
                - shaft_before;

        assert!(simulation.accumulated_wall_impulses.top > 0.0);
        assert!(energy_balance.abs() < BALANCE_TOLERANCE);
    }

    #[test]
    fn starter_compresses_a_heated_gas_with_many_particles() {
        const TEST_PARTICLE_COUNT: usize = 256;
        const TEST_TEMPERATURE: f32 = 4.0;
        const MIN_EXPECTED_COMPRESSION_FRACTION: f32 = 0.7;
        const MAX_STARTER_STEPS: usize = 1_000;

        let mut simulation = Simulation::with_particle_count(TEST_PARTICLE_COUNT);
        simulation.set_temperature(TEST_TEMPERATURE);
        simulation.toggle_crankshaft();
        simulation.start_crankshaft();

        let full_volume = simulation.volume();
        let mut minimum_volume = full_volume;
        for _ in 0..MAX_STARTER_STEPS {
            simulation.update(FIXED_DT);
            minimum_volume = minimum_volume.min(simulation.volume());
            if !simulation.crankshaft().unwrap().starter_active() {
                break;
            }
        }

        assert!(minimum_volume / full_volume < MIN_EXPECTED_COMPRESSION_FRACTION);
        assert!(!simulation.crankshaft().unwrap().starter_active());
        assert!(simulation.crankshaft().unwrap().starter_work() > 0.0);
    }

    #[test]
    fn disengaging_crank_preserves_piston_height() {
        let mut simulation = Simulation::new();
        simulation.toggle_crankshaft();
        simulation.start_crankshaft();
        simulation.update(FIXED_DT);
        let height_before = simulation.piston_position();
        simulation.toggle_crankshaft();

        assert!(simulation.crankshaft().is_none());
        assert!((simulation.piston_position() - height_before).abs() < POSITION_TOLERANCE);
        assert_eq!(simulation.piston_velocity(), 0.0);
    }
}
