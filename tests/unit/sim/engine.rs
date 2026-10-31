//! Simulation tests for `src/sim/engine.rs`: the fill, transport through
//! the opening between the cylinders, wall heat, piston work, timestep
//! limits, and the long started-engine regressions.

use super::*;
use crate::sim::cycle::{
    COLD,
    HOT,
    TurnSummary,
    loop_work, //
};
use crate::sim::gas::DYNAMICS_TIME_SCALE;
use crate::sim::geometry::DEFAULT_GEOMETRY;
use crate::sim::reservoir::{
    MAX_COLD_KELVIN,
    MAX_HOT_KELVIN,
    MIN_COLD_KELVIN,
    MIN_HOT_KELVIN, //
};
use crate::sim::units::{
    DEFAULT_CONCENTRATION,
    DEFAULT_PARTICLES,
    GAS_CONSTANT, //
};
/// A hot-wall setting between the extremes.
const TEST_WARM_KELVIN: f32 = 975.0;

/// Particles in each region.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RegionCounts {
    /// In the hot cylinder.
    hot: usize,
    /// In the opening between the bores.
    passage: usize,
    /// In the cold cylinder.
    cold: usize,
}

impl ConnectedChambers {
    /// Gas volume under the pistons where they are now.
    fn volume_m3(&self) -> f64 {
        self.pistons.volume_m3(self.geometry)
    }

    /// Initial concentration in mol/L, against the volume at the fill
    /// rather than under the pistons where they are now.
    fn initial_concentration(&self) -> f64 {
        let fill_volume =
            PowerPistons::new(self.geometry, PARTICLE_RADIUS).volume_m3(self.geometry);
        self.total_moles() / (fill_volume * 1_000.0)
    }

    /// Holds the pistons still whatever the gas does.
    fn pin_pistons(&mut self) {
        self.pistons.pin();
    }

    /// Refills with `count` particles if that changes the count.
    fn set_particle_count(&mut self, count: usize) {
        if count != self.particles.len() {
            self.refill(count);
        }
    }

    /// Whether a particle centered at `position` fits in the gas volume.
    fn contains(&self, position: Vec3) -> bool {
        self.geometry.contains(
            position,
            PARTICLE_RADIUS,
            self.pistons.hot().face_y,
            self.pistons.cold().face_y,
        )
    }

    /// The default chamber filled with `TEST_SEED`.
    fn seeded(count: usize) -> Self {
        Self::with_geometry_seed(count, DEFAULT_GEOMETRY, TEST_SEED)
    }

    /// `seeded` at an initial concentration in mol/L.
    fn seeded_at_concentration(concentration: f32) -> Self {
        Self::seeded(Self::particles_for_concentration(concentration))
    }

    /// Particles in each region.
    fn counts(&self) -> RegionCounts {
        let mut counts = RegionCounts::default();
        for particle in &self.particles {
            match self.region(particle.position) {
                Region::Hot => counts.hot += 1,
                Region::Passage => counts.passage += 1,
                Region::Cold => counts.cold += 1,
            }
        }
        counts
    }
}

/// Distance from the middle to each cylinder's axis.
const CYLINDER_OFFSET: f32 = DEFAULT_GEOMETRY.cylinder_offset;
/// Radius of each bore.
const CYLINDER_RADIUS: f32 = DEFAULT_GEOMETRY.cylinder_radius;
/// Height of each bore.
const CYLINDER_HEIGHT: f32 = DEFAULT_GEOMETRY.cylinder_height;
/// Radius of the opening between the bores.
const PASSAGE_RADIUS: f32 = DEFAULT_GEOMETRY.passage_radius;

/// Largest position difference treated as equal.
const POSITION_TOLERANCE: f32 = 1e-5;
/// Largest energy difference treated as equal.
const ENERGY_TOLERANCE: f32 = 1e-3;
/// Steps the transport tests run.
const TRANSFER_TEST_STEPS: usize = 1_500;
/// Particles in the transport tests.
const TEST_COUNT: usize = 64;
/// Seed for every test gas, so a failing test can be rerun exactly. Only
/// draws that are the point of a test (the new gas a reset makes, the
/// random-seed rotation sweep) are left to chance.
const TEST_SEED: u64 = 42;
/// Speed of a particle sent through the opening.
const TRANSIT_SPEED: f32 = 2.0;
/// The opening's axis: one opening radius above the floors.
const TRANSIT_HEIGHT: f32 = -CYLINDER_HEIGHT / 2.0 + PASSAGE_RADIUS;

/// A gas of `count` particles with insulated walls and pinned pistons, for
/// transport tests.
fn insulated(count: usize) -> ConnectedChambers {
    let mut simulation = ConnectedChambers::seeded(count);
    simulation.thermal.set_accommodation(0.0);
    simulation.pin_pistons();
    simulation
}

#[test]
fn initial_gas_is_distributed_without_overlaps() {
    for &count in &[TEST_COUNT, 256, 512, 1_024, 1_536, 2_048] {
        let simulation = insulated(count);
        let regions = simulation.counts();
        assert!(regions.hot > 0);
        assert!(regions.cold > 0);
        assert_eq!(regions.hot + regions.passage + regions.cold, count);
        assert!(
            simulation
                .particles
                .iter()
                .all(|p| simulation.contains(p.position))
        );
        for (index, particle) in simulation.particles.iter().enumerate() {
            for other in &simulation.particles[index + 1..] {
                assert!(
                    particle.position.distance(other.position) + POSITION_TOLERANCE
                        >= 2.0 * PARTICLE_RADIUS
                );
            }
        }
    }
}

#[test]
fn a_particle_can_cross_from_hot_to_cold_through_the_opening() {
    let mut simulation = insulated(1);
    simulation.particles[0].position = vec3(-CYLINDER_OFFSET, TRANSIT_HEIGHT, 0.0);
    simulation.particles[0].velocity = vec3(TRANSIT_SPEED, 0.0, 0.0);

    let original_energy = simulation.kinetic_energy();
    let mut saw_passage = false;
    let mut reached_cold = false;
    for _ in 0..TRANSFER_TEST_STEPS {
        simulation.update(FIXED_DT);
        let position = simulation.particles[0].position;
        assert!(simulation.contains(position));
        saw_passage |= simulation.region(position) == Region::Passage;
        if simulation.region(position) == Region::Cold {
            reached_cold = true;
            break;
        }
    }

    assert!(saw_passage);
    assert!(
        reached_cold,
        "particle did not travel into the cold chamber"
    );
    assert!((simulation.kinetic_energy() - original_energy).abs() < ENERGY_TOLERANCE);
}

#[test]
fn travel_through_the_opening_works_in_both_directions() {
    let mut simulation = insulated(1);
    simulation.particles[0].position = vec3(CYLINDER_OFFSET, TRANSIT_HEIGHT, 0.0);
    simulation.particles[0].velocity = vec3(-TRANSIT_SPEED, 0.0, 0.0);

    let reached_hot = (0..TRANSFER_TEST_STEPS).any(|_| {
        simulation.update(FIXED_DT);
        simulation.region(simulation.particles[0].position) == Region::Hot
    });
    assert!(reached_hot, "the opening must be open both ways");
}

#[test]
fn shared_wall_is_closed_above_the_opening() {
    const ABOVE_OPENING_Y: f32 = 1.0;
    const {
        assert!(ABOVE_OPENING_Y > -CYLINDER_HEIGHT / 2.0 + 2.0 * PASSAGE_RADIUS + PARTICLE_RADIUS);
    }
    let mut simulation = insulated(1);
    simulation.particles[0].position = vec3(-CYLINDER_OFFSET, ABOVE_OPENING_Y, 0.0);
    simulation.particles[0].velocity = vec3(TRANSIT_SPEED, 0.0, 0.0);
    let original_energy = simulation.kinetic_energy();

    for _ in 0..TRANSFER_TEST_STEPS {
        simulation.update(FIXED_DT);
        let position = simulation.particles[0].position;
        assert!(simulation.contains(position));
        assert_eq!(simulation.region(position), Region::Hot);
    }
    assert!((simulation.kinetic_energy() - original_energy).abs() < ENERGY_TOLERANCE);
}

#[test]
fn opening_walls_reflect_without_changing_energy() {
    let mut simulation = insulated(1);
    simulation.particles[0].position = vec3(0.0, TRANSIT_HEIGHT, 0.0);
    simulation.particles[0].velocity = vec3(0.0, 0.0, TRANSIT_SPEED);
    let energy = simulation.kinetic_energy();
    for _ in 0..TRANSFER_TEST_STEPS {
        simulation.update(FIXED_DT);
        assert!(simulation.contains(simulation.particles[0].position));
    }
    assert!((simulation.kinetic_energy() - energy).abs() < ENERGY_TOLERANCE);
}

#[test]
fn two_chamber_gas_conserves_particles_and_energy() {
    let mut simulation = insulated(TEST_COUNT);
    let initial_energy = simulation.kinetic_energy();
    for _ in 0..TRANSFER_TEST_STEPS {
        simulation.update(FIXED_DT);
    }
    let counts = simulation.counts();
    assert_eq!(counts.hot + counts.passage + counts.cold, TEST_COUNT);
    assert!(
        simulation
            .particles
            .iter()
            .all(|p| simulation.contains(p.position))
    );
    assert!(
        (simulation.kinetic_energy() - initial_energy).abs() < ENERGY_TOLERANCE * initial_energy
    );
}
#[test]
fn thermal_reservoirs_exchange_energy_at_both_cylinder_walls() {
    const WALL_SPEED: f32 = 2.0;
    const ENERGY_BALANCE_TOLERANCE: f32 = 1e-4;
    let mut simulation = ConnectedChambers::seeded(2);
    simulation.set_hot_kelvin(MAX_HOT_KELVIN);
    simulation.thermal.set_accommodation(1.0);
    simulation.pin_pistons();
    let x = CYLINDER_OFFSET + CYLINDER_RADIUS - PARTICLE_RADIUS;
    let step_into_wall = WALL_SPEED * FIXED_DT / 2.0;
    simulation.particles[0].position = vec3(-x + step_into_wall, 0.0, 0.0);
    simulation.particles[0].velocity = vec3(-WALL_SPEED, 0.0, 0.0);
    simulation.particles[1].position = vec3(x - step_into_wall, 0.0, 0.0);
    simulation.particles[1].velocity = vec3(WALL_SPEED, 0.0, 0.0);

    let before = simulation.kinetic_energy();
    simulation.update(FIXED_DT);
    let heat = simulation.heat();
    assert_ne!(heat.hot, 0.0);
    assert_ne!(heat.cold, 0.0);
    assert!(
        ((simulation.kinetic_energy() - before) as f64 - heat.total()).abs()
            < ENERGY_BALANCE_TOLERANCE as f64
    );
}

/// With the hot wall at its lowest (300 K) and the cold wall at 100 K, heat
/// still flows from hot to cold: the cold wall cools the gas, and gas that
/// drifts back into the hot cylinder takes heat from its 300 K wall. The
/// pistons are held, so only heat carried through the gas counts. Over 40
/// seeds, 20 s gave the hot wall 139 units on average, with a spread of 23
/// and never below 84, so the signs do not depend on this seed.
#[test]
fn cold_sink_alone_drives_heat_from_hot_to_cold() {
    const COUNT: usize = 256;
    const SECONDS: f32 = 20.0;
    let mut simulation = ConnectedChambers::seeded(COUNT);
    simulation.set_hot_kelvin(MIN_HOT_KELVIN);
    simulation.set_cold_kelvin(MIN_COLD_KELVIN);
    simulation.pin_pistons();
    for _ in 0..(SECONDS / FIXED_DT) as usize {
        simulation.step(FIXED_DT);
    }
    let heat = simulation.heat();
    assert!(heat.hot > 0.0, "hot wall gave the gas {} units", heat.hot);
    assert!(
        heat.cold < 0.0,
        "cold wall gave the gas {} units",
        heat.cold
    );
}

#[test]
fn fixed_chamber_first_law_holds_during_heating_and_cooling() {
    const THERMAL_TEST_STEPS: usize = 1_200;
    const TEST_HOT_KELVIN: f32 = MAX_HOT_KELVIN;
    const RELATIVE_TOLERANCE: f32 = 2e-3;
    let mut simulation = ConnectedChambers::seeded(TEST_COUNT);
    simulation.set_hot_kelvin(TEST_HOT_KELVIN);
    simulation.thermal.set_accommodation(1.0);
    simulation.pin_pistons();
    let initial_energy = simulation.kinetic_energy();
    for _ in 0..THERMAL_TEST_STEPS {
        simulation.update(FIXED_DT);
    }
    assert!(simulation.heat().hot != 0.0);
    assert!(
        simulation.energy_balance_error().abs()
            < (RELATIVE_TOLERANCE * initial_energy.max(simulation.kinetic_energy())) as f64
    );
}

#[test]
fn reservoir_settings_survive_refill_and_heat_resets() {
    const TARGET_HOT_KELVIN: f32 = TEST_WARM_KELVIN;
    let mut simulation = ConnectedChambers::seeded(64);
    simulation.set_hot_kelvin(TARGET_HOT_KELVIN);
    simulation.set_particle_count(256);
    assert_eq!(simulation.hot_kelvin(), TARGET_HOT_KELVIN);
    simulation.reset();
    assert_eq!(simulation.hot_kelvin(), TARGET_HOT_KELVIN);
    assert_eq!(simulation.heat().total(), 0.0);
}

#[test]
fn opening_and_upper_caps_are_adiabatic() {
    const WALL_SPEED: f32 = 2.0;
    let mut simulation = ConnectedChambers::seeded(2);
    simulation.thermal.set_accommodation(1.0);
    simulation.set_hot_kelvin(MAX_HOT_KELVIN);
    simulation.pin_pistons();
    let radius = PARTICLE_RADIUS;
    simulation.particles[0].position = vec3(0.0, TRANSIT_HEIGHT, PASSAGE_RADIUS - radius);
    simulation.particles[0].velocity = vec3(0.0, 0.0, WALL_SPEED);
    simulation.particles[1].position = vec3(-CYLINDER_OFFSET, CYLINDER_HEIGHT / 2.0 - radius, 0.0);
    simulation.particles[1].velocity = vec3(0.0, WALL_SPEED, 0.0);
    let energy = simulation.kinetic_energy();
    simulation.update(FIXED_DT);
    assert_eq!(simulation.heat().total(), 0.0);
    assert!((simulation.kinetic_energy() - energy).abs() < ENERGY_TOLERANCE);
}

#[test]
fn real_unit_measurements_conserve_helium_amount_on_transfer() {
    const PARTICLES: usize = 256;
    let mut simulation = insulated(PARTICLES);
    let total_moles = simulation.total_moles();
    let first = simulation.measurements();
    assert!(first.hot.count > 0);
    assert!(first.cold.count > 0);
    assert_eq!(
        first.hot.count + first.passage_count + first.cold.count,
        PARTICLES
    );
    assert!(first.hot.temperature_kelvin.unwrap() > 0.0);
    assert!(first.cold.temperature_kelvin.unwrap() > 0.0);
    for _ in 0..TRANSFER_TEST_STEPS {
        simulation.update(FIXED_DT);
    }
    let readings = simulation.measurements();
    assert_eq!(
        readings.hot.count + readings.passage_count + readings.cold.count,
        PARTICLES
    );
    assert_eq!(simulation.total_moles(), total_moles);
}

#[test]
fn initial_gas_and_wall_temperatures_are_in_kelvin() {
    const ABSOLUTE_TOLERANCE_K: f64 = 0.02;
    let mut simulation = ConnectedChambers::seeded(TEST_COUNT);
    let readings = simulation.measurements();
    assert!((readings.hot.temperature_kelvin.unwrap() - 300.0).abs() < ABSOLUTE_TOLERANCE_K);
    assert!((readings.cold.temperature_kelvin.unwrap() - 300.0).abs() < ABSOLUTE_TOLERANCE_K);
    assert_eq!(simulation.hot_kelvin(), 300.0);
    simulation.set_hot_kelvin(MAX_HOT_KELVIN);
    assert_eq!(simulation.hot_kelvin(), 1_200.0);
}

#[test]
fn alternate_geometry_survives_reset_and_particle_count_changes() {
    const TEST_SCALE: f32 = 2.2;
    let geometry = ChamberGeometry::scaled(TEST_SCALE);
    let mut simulation = ConnectedChambers::with_geometry_seed(TEST_COUNT, geometry, TEST_SEED);
    simulation.set_hot_kelvin(TEST_WARM_KELVIN);
    assert_eq!(
        simulation.geometry().cylinder_radius,
        geometry.cylinder_radius
    );
    simulation.reset();
    simulation.set_particle_count(256);
    assert_eq!(
        simulation.geometry().cylinder_height,
        geometry.cylinder_height
    );
    assert_eq!(simulation.hot_kelvin(), TEST_WARM_KELVIN);
}

#[test]
#[should_panic(expected = "chamber is too small")]
fn undersized_chamber_rejects_particles_instead_of_shrinking_them() {
    let _ = ConnectedChambers::with_geometry_seed(2_048, ChamberGeometry::scaled(1.0), TEST_SEED);
}

#[test]
fn helium_fill_and_volume_scale_together() {
    const DOUBLE_SCALE: f32 = 2.0;
    const RELATIVE_TOLERANCE: f64 = 1e-5;
    let base =
        ConnectedChambers::with_geometry_seed(TEST_COUNT, ChamberGeometry::scaled(1.0), TEST_SEED);
    let doubled = ConnectedChambers::with_geometry_seed(
        TEST_COUNT,
        ChamberGeometry::scaled(DOUBLE_SCALE),
        TEST_SEED,
    );
    let volume_ratio = doubled.volume_m3() / base.volume_m3();
    let moles_ratio = doubled.total_moles() / base.total_moles();
    assert!(volume_ratio > 1.0);
    assert!((moles_ratio - volume_ratio).abs() < RELATIVE_TOLERANCE);
}

#[test]
fn taller_chambers_expand_collision_grid_bounds() {
    let mut geometry = ChamberGeometry::default();
    geometry.cylinder_height = geometry.grid_side() * 1.1;
    let simulation = ConnectedChambers::with_geometry_seed(TEST_COUNT, geometry, TEST_SEED);
    assert!(simulation.geometry().grid_side() >= geometry.cylinder_height);
    assert!(simulation.geometry().grid_side() / 2.0 >= -geometry.floor());
    assert!(
        simulation
            .particles
            .iter()
            .all(|p| simulation.contains(p.position))
    );
}

/// The collision grid stores only the cells over `gas_bounds`, so every
/// particle center must stay inside them, pistons moving and walls hot.
#[test]
fn particle_centers_stay_within_the_gas_bounds() {
    const STEPS: usize = 240;
    let mut simulation = ConnectedChambers::seeded(TEST_COUNT);
    simulation.set_hot_kelvin(MAX_HOT_KELVIN);
    assert!(simulation.start_engine());
    let (lower, upper) = simulation.geometry().gas_bounds();
    for _ in 0..STEPS {
        simulation.step(FIXED_DT);
        for particle in &simulation.particles {
            for axis in 0..3 {
                assert!((lower[axis]..=upper[axis]).contains(&particle.position[axis]));
            }
        }
    }
}

#[test]
fn initial_concentration_is_bounded_by_one_molar_across_the_system() {
    const LOCAL_CONCENTRATION_TOLERANCE: f64 = 0.10;
    let full = ConnectedChambers::seeded_at_concentration(1.0);
    let initial = full.measurements();
    assert_eq!(full.particles.len(), MAX_PARTICLES);
    assert!((full.initial_concentration() - 1.0).abs() < 1e-12);
    assert!((initial.hot.concentration_mol_per_l - 1.0).abs() < LOCAL_CONCENTRATION_TOLERANCE);
    assert!((initial.cold.concentration_mol_per_l - 1.0).abs() < LOCAL_CONCENTRATION_TOLERANCE);
    assert!((full.total_moles() - full.volume_m3() * 1_000.0).abs() < 1e-10);
}

#[test]
fn concentration_controls_gas_amount_and_reset_preserves_it() {
    const INITIAL_CONCENTRATION: f32 = 0.25;
    let mut simulation = ConnectedChambers::seeded_at_concentration(INITIAL_CONCENTRATION);
    assert!(
        (simulation.initial_concentration() - INITIAL_CONCENTRATION as f64).abs()
            <= 0.5 / MAX_PARTICLES as f64
    );
    let initial_moles = simulation.total_moles();
    simulation.set_hot_kelvin(TEST_WARM_KELVIN);
    simulation.reset();
    assert_eq!(simulation.total_moles(), initial_moles);
    simulation.set_concentration(0.50);
    assert_eq!(simulation.total_moles(), 2.0 * initial_moles);
    assert_eq!(simulation.hot_kelvin(), TEST_WARM_KELVIN);
}

#[test]
fn concentration_scaling_is_independent_of_geometry_size() {
    const INITIAL_CONCENTRATION: f32 = 0.50;
    let mut large_geometry = ChamberGeometry::default();
    large_geometry.cylinder_height *= 1.25;
    let particle_count = ConnectedChambers::particles_for_concentration(INITIAL_CONCENTRATION);
    let original = ConnectedChambers::with_geometry_seed(
        particle_count,
        ChamberGeometry::default(),
        TEST_SEED,
    );
    let larger = ConnectedChambers::with_geometry_seed(particle_count, large_geometry, TEST_SEED);
    assert!((original.initial_concentration() - larger.initial_concentration()).abs() < 1e-12);
    assert!(larger.total_moles() > original.total_moles());
}

#[test]
fn maximum_initial_concentration_gives_near_equal_ideal_pressures() {
    const EXPECTED_PRESSURE_PA: f64 = 1_000.0 * GAS_CONSTANT * 300.0;
    const RELATIVE_TOLERANCE: f64 = 0.10;
    let simulation = ConnectedChambers::seeded_at_concentration(1.0);
    let readings = simulation.measurements();
    for pressure in [
        readings.hot.pressure_pa.unwrap(),
        readings.cold.pressure_pa.unwrap(),
    ] {
        assert!(
            ((pressure - EXPECTED_PRESSURE_PA) / EXPECTED_PRESSURE_PA).abs() < RELATIVE_TOLERANCE
        );
    }
}

#[test]
fn default_fill_holds_1024_particles_at_half_molar() {
    const CONCENTRATION_TOLERANCE: f64 = 1e-12;
    let simulation = ConnectedChambers::seeded_at_concentration(DEFAULT_CONCENTRATION);
    let regions = simulation.measurements();
    assert_eq!(simulation.particles.len(), DEFAULT_PARTICLES);
    assert!((simulation.initial_concentration() - 0.5).abs() < CONCENTRATION_TOLERANCE);
    assert!(regions.hot.count > 0);
    assert!(regions.cold.count > 0);
    assert!(regions.passage_count > 0);
}

#[test]
fn minimum_fill_represents_256_particles() {
    let minimum = ConnectedChambers::seeded_at_concentration(MIN_CONCENTRATION);
    assert_eq!(minimum.particles.len(), MIN_PARTICLES);
    assert!((minimum.initial_concentration() - 0.125).abs() < 1e-12);
}

#[test]
fn resetting_refills_both_cylinders_at_the_same_temperature() {
    const TEMPERATURE_TOLERANCE_K: f64 = 0.02;
    let mut simulation = ConnectedChambers::seeded_at_concentration(0.5);
    simulation.set_hot_kelvin(MAX_HOT_KELVIN);
    for _ in 0..100 {
        simulation.update(FIXED_DT);
    }
    simulation.reset();
    let readings = simulation.measurements();
    assert_eq!(simulation.hot_kelvin(), MAX_HOT_KELVIN);
    assert!((readings.hot.temperature_kelvin.unwrap() - 300.0).abs() < TEMPERATURE_TOLERANCE_K);
    assert!((readings.cold.temperature_kelvin.unwrap() - 300.0).abs() < TEMPERATURE_TOLERANCE_K);
    assert!(readings.hot.count > 0 && readings.cold.count > 0);
    assert_eq!(simulation.heat().total(), 0.0);
}

#[test]
fn moving_pistons_keep_every_sphere_below_their_faces() {
    const TEST_STEPS: usize = 1_000;
    let mut simulation = insulated(256);
    simulation.kick_flywheel();
    simulation.thermal.set_accommodation(0.0);
    for step in 0..TEST_STEPS {
        simulation.update(FIXED_DT);
        for (index, particle) in simulation.particles.iter().enumerate() {
            assert!(
                simulation.contains(particle.position),
                "sphere {index} escaped at step {step}: {:?}",
                particle.position,
            );
        }
    }
    assert!(simulation.piston_work_by_gas().abs() > 0.0);
}

#[test]
fn piston_collision_exchanges_equal_and_opposite_energy() {
    const TEST_PARTICLE_SPEED: f32 = 1.0;
    let geometry = ChamberGeometry::default();
    let mut pistons = PowerPistons::new(geometry, PARTICLE_RADIUS);
    pistons.kick();
    // Turn until the hot piston moves down onto the particle.
    for _ in 0..1_000 {
        pistons.advance(FIXED_DT);
        if pistons.hot().velocity < 0.0 {
            break;
        }
    }
    let state = pistons.hot();
    assert!(state.velocity < 0.0);
    let mut particle = Particle {
        position: vec3(-geometry.cylinder_offset, state.face_y, 0.0),
        velocity: vec3(0.0, TEST_PARTICLE_SPEED, 0.0),
    };
    let before = gas::particle_kinetic_energy(particle.velocity);
    let walls = Walls {
        geometry,
        hot_top: state.face_y,
        cold_top: pistons.cold().face_y,
    };
    let work = walls.bounce_piston(
        &mut particle,
        -geometry.cylinder_offset,
        state,
        PistonSide::Hot,
        &mut pistons,
    );
    let after = gas::particle_kinetic_energy(particle.velocity);
    assert!((after - before + work).abs() < ENERGY_TOLERANCE);
    assert!(work < 0.0);
    assert!(particle.position.y < state.face_y - PARTICLE_RADIUS);
}

#[test]
fn moving_insulated_pistons_obey_first_law() {
    const TEST_STEPS: usize = 1_000;
    const RELATIVE_TOLERANCE: f32 = 5e-3;
    let mut simulation = insulated(256);
    simulation.kick_flywheel();
    simulation.thermal.set_accommodation(0.0);
    let reference = simulation.kinetic_energy();
    for _ in 0..TEST_STEPS {
        simulation.update(FIXED_DT);
    }
    assert_eq!(simulation.heat().total(), 0.0);
    assert!(simulation.piston_work_by_gas().abs() > 0.0);
    assert!(simulation.energy_balance_error().abs() < (RELATIVE_TOLERANCE * reference) as f64);
}

#[test]
fn heating_and_piston_motion_track_heat_and_work_separately() {
    const TEST_STEPS: usize = 700;
    const RELATIVE_TOLERANCE: f32 = 5e-3;
    let mut simulation = ConnectedChambers::seeded(256);
    simulation.thermal.set_accommodation(1.0);
    simulation.set_hot_kelvin(MAX_HOT_KELVIN);
    simulation.kick_flywheel();
    let reference = simulation.kinetic_energy();
    for _ in 0..TEST_STEPS {
        simulation.update(FIXED_DT);
    }
    assert!(simulation.heat().hot != 0.0);
    assert!(simulation.piston_work_by_gas().abs() > 0.0);
    assert!(simulation.energy_balance_error().abs() < (RELATIVE_TOLERANCE * reference) as f64);
}

#[test]
fn refills_start_the_shaft_at_rest() {
    let mut simulation = ConnectedChambers::seeded(256);
    simulation.start_engine();
    simulation.set_particle_count(512);
    assert!(simulation.power_pistons().held());
    simulation.start_engine();
    simulation.reset();
    assert!(simulation.power_pistons().held());
    assert_eq!(simulation.power_pistons().angular_velocity(), 0.0);
}

#[test]
fn free_flywheel_tracks_gas_and_mechanical_energy_separately() {
    const TEST_STEPS: usize = 400;
    const RELATIVE_TOLERANCE: f32 = 0.015;
    let mut simulation = insulated(256);
    simulation.kick_flywheel();
    let gas_reference = simulation.kinetic_energy();
    for _ in 0..TEST_STEPS {
        simulation.update(FIXED_DT);
    }
    assert!(simulation.energy_balance_error().abs() < (RELATIVE_TOLERANCE * gas_reference) as f64);
    assert!(
        simulation.mechanical_balance_error().abs() < (RELATIVE_TOLERANCE * gas_reference) as f64
    );
}

#[test]
fn seeded_simulations_remain_identical_when_interleaved() {
    const STEPS: usize = 200;
    let mut first = ConnectedChambers::seeded(TEST_COUNT);
    let mut second = ConnectedChambers::seeded(TEST_COUNT);
    first.set_hot_kelvin(MAX_HOT_KELVIN);
    second.set_hot_kelvin(MAX_HOT_KELVIN);
    for _ in 0..STEPS {
        first.update(FIXED_DT);
        second.update(FIXED_DT);
    }
    for (a, b) in first.particles.iter().zip(&second.particles) {
        assert_eq!(a.position, b.position);
        assert_eq!(a.velocity, b.velocity);
    }
    assert_eq!(first.heat().hot, second.heat().hot);
}

#[test]
fn reservoir_controls_change_no_particles_until_an_impact() {
    let mut simulation = ConnectedChambers::seeded(TEST_COUNT);
    let before: Vec<_> = simulation.particles.iter().map(|p| p.velocity).collect();
    simulation.set_cold_kelvin(MIN_COLD_KELVIN);
    simulation.set_hot_kelvin(MAX_HOT_KELVIN);
    assert_eq!(simulation.heat().total(), 0.0);
    for (particle, velocity) in simulation.particles.iter().zip(before) {
        assert_eq!(particle.velocity, velocity);
    }
    simulation.reset();
    assert_eq!(simulation.cold_kelvin(), MIN_COLD_KELVIN);
    assert_eq!(simulation.hot_kelvin(), MAX_HOT_KELVIN);
    assert_eq!(simulation.power_pistons().starter_work(), 0.0);
}

#[test]
fn fast_head_on_particles_do_not_tunnel_past_each_other() {
    const SPEED: f32 = 120.0;
    const HALF_SEPARATION: f32 = 0.5;
    let mut simulation = insulated(2);
    let center = -simulation.geometry.cylinder_offset;
    simulation.particles[0].position = vec3(center - HALF_SEPARATION, 0.0, 0.0);
    simulation.particles[1].position = vec3(center + HALF_SEPARATION, 0.0, 0.0);
    simulation.particles[0].velocity = vec3(SPEED, 0.0, 0.0);
    simulation.particles[1].velocity = vec3(-SPEED, 0.0, 0.0);
    let before = simulation.kinetic_energy();
    simulation.step(FIXED_DT);
    assert!(simulation.particles[0].velocity.x < 0.0);
    assert!(simulation.particles[1].velocity.x > 0.0);
    assert!((simulation.kinetic_energy() - before).abs() < ENERGY_TOLERANCE);
}

/// One recorded turn and the work enclosed by each cylinder's P-V loop.
struct RecordedTurn {
    /// The turn as the cycle monitor summed it.
    summary: TurnSummary,
    /// Work enclosed by the hot and the cold loop.
    loop_work: [f64; 2],
}

/// Result of one started run, sampled at the last completed revolution.
struct CycleOutcome {
    /// Every turn the cycle monitor recorded.
    turns: Vec<RecordedTurn>,
    /// Revolutions completed in the heat-engine direction.
    completed_turns: usize,
    /// Flywheel energy Start gave.
    starter_energy: f64,
    /// Flywheel energy at the last completed revolution.
    phase_energy: f64,
    /// Gas work up to the last completed revolution.
    phase_gas_work: f64,
    /// Work the load took up to the last completed revolution.
    phase_load_work: f64,
    /// Starter work at the end, which must equal `starter_energy`.
    final_starter_work: f64,
    /// Heat from each reservoir at the end.
    heat: PerReservoir,
    /// Gas first-law residual at the end.
    gas_residual: f64,
    /// Flywheel energy-balance residual at the end.
    shaft_residual: f64,
}

/// Default fill, one Start press, cold reservoir at 300 K. Energy is sampled at
/// completed revolutions because compression and back-pressure work make
/// arbitrary crank phases incomparable.
fn started_run(seed: u64, hot_kelvin: f32, dt: f32, duration: f32) -> CycleOutcome {
    let mut simulation =
        ConnectedChambers::with_geometry_seed(DEFAULT_PARTICLES, DEFAULT_GEOMETRY, seed);
    simulation.set_hot_kelvin(hot_kelvin);
    simulation.set_cold_kelvin(MAX_COLD_KELVIN);
    assert!(simulation.start_engine());
    let starter_energy = simulation.pistons.starter_work();
    let mut completed_turns = 0;
    let mut phase_energy = starter_energy;
    let mut phase_gas_work = 0.0;
    let mut phase_load_work = 0.0;
    let mut recorded = Vec::new();
    for _ in 0..(duration / dt) as usize {
        simulation.step(dt);
        let cycle = simulation.cycle();
        if cycle.completed_turns() as usize > recorded.len() {
            recorded.push(RecordedTurn {
                summary: cycle.last_turn().unwrap(),
                loop_work: [HOT, COLD].map(|cylinder| loop_work(cycle.last_loop(cylinder))),
            });
        }
        let turns = (-simulation.pistons.revolutions()).max(0.0) as usize;
        if turns > completed_turns {
            completed_turns = turns;
            phase_energy = simulation.pistons.kinetic_energy();
            phase_gas_work = simulation.piston_work_by_gas();
            phase_load_work = simulation.pistons.load_work();
        }
    }
    CycleOutcome {
        turns: recorded,
        completed_turns,
        starter_energy,
        phase_energy,
        phase_gas_work,
        phase_load_work,
        final_starter_work: simulation.pistons.starter_work(),
        heat: simulation.heat(),
        gas_residual: simulation.energy_balance_error(),
        shaft_residual: simulation.mechanical_balance_error(),
    }
}

/// Model time of each started-engine regression run.
const ROTATION_DURATION: f32 = 120.0 * DYNAMICS_TIME_SCALE;
/// Fewest revolutions a run must complete.
const MIN_TURNS: usize = 4;
/// Largest energy-balance residual accepted.
const BALANCE_TOLERANCE: f64 = 0.05;
/// A heated engine powering its load keeps at least this fraction of the
/// Start energy in the flywheel (measured: 1.08 to 1.25 at 1200/300 K).
const HOT_KEEP_RATIO: f64 = 0.8;
/// ...while its gas does net work worth at least this fraction of the Start
/// energy (measured: 1.06 to 1.29).
const HOT_WORK_RATIO: f64 = 0.5;
/// An engine with no temperature difference keeps less than this fraction
/// (measured: 0.14 to 0.16 at its last full turn, after about 8 turns).
const EQUILIBRIUM_REMAINING_RATIO: f64 = 0.4;
/// The two P-V loop areas must add up to the measured gas work within this
/// fraction of the gross work on both pistons. The remaining error comes
/// from averaging pressure over 10-degree bins; measured: below 0.7%.
const LOOP_WORK_TOLERANCE: f64 = 0.02;

/// Panics with the seed in the message if a run misses the criteria: a
/// heated engine must keep turning while its gas does net work for the
/// load, an equilibrium engine must run down, and both energy balances
/// must close.
fn check_started_run(seed: u64, hot_kelvin: f32, dt: f32, outcome: &CycleOutcome) {
    let o = outcome;
    println!(
        "seed={seed}, hot={hot_kelvin} K, dt={dt}: turns={}, phase energy={:.3} of {:.3}, gas work={:.3}, load work={:.3}, gas residual={:.6}, shaft residual={:.6}",
        o.completed_turns,
        o.phase_energy,
        o.starter_energy,
        o.phase_gas_work,
        o.phase_load_work,
        o.gas_residual,
        o.shaft_residual
    );
    let context = format!("seed {seed}, hot {hot_kelvin} K, dt {dt}");
    assert!(o.completed_turns >= MIN_TURNS, "too few turns: {context}");
    check_recorded_turns(o, &context);
    assert_eq!(o.final_starter_work, o.starter_energy, "{context}");
    assert!(o.gas_residual.abs() < BALANCE_TOLERANCE, "{context}");
    assert!(o.shaft_residual.abs() < BALANCE_TOLERANCE, "{context}");
    if hot_kelvin > MAX_COLD_KELVIN {
        assert!(
            o.phase_energy > HOT_KEEP_RATIO * o.starter_energy,
            "heated engine slowed down under its load: {context}"
        );
        assert!(
            o.phase_gas_work > HOT_WORK_RATIO * o.starter_energy,
            "heated gas did too little work: {context}"
        );
        assert!(o.phase_load_work > 0.0, "{context}");
        assert!(o.heat.hot > 0.0 && o.heat.cold < 0.0, "{context}");
    } else {
        assert!(
            o.phase_energy < EQUILIBRIUM_REMAINING_RATIO * o.starter_energy,
            "equilibrium engine did not lose energy: {context}"
        );
        assert!(o.phase_gas_work < 0.0, "{context}");
    }
}

/// Checks every recorded turn against the laws the panel displays: the
/// first-law ledger balances, the loop areas equal the work, efficiency
/// stays below the Carnot limit, and settled turns raise the entropy of the
/// surroundings.
fn check_recorded_turns(outcome: &CycleOutcome, context: &str) {
    // The first boundary only restarts the count, so one turn fewer.
    assert!(
        outcome.turns.len() + 1 >= outcome.completed_turns,
        "turns not recorded: {context}"
    );
    let mut worst_loop_mismatch: f64 = 0.0;
    let mut least_settled_entropy = f64::INFINITY;
    for (index, turn) in outcome.turns.iter().enumerate() {
        let summary = turn.summary;
        let at = format!("turn {}: {context}", index + 1);
        assert!(
            summary.first_law_residual().abs() < BALANCE_TOLERANCE,
            "ledger off by {}: {at}",
            summary.first_law_residual()
        );
        let [hot, cold] = turn.loop_work;
        let mismatch = (hot + cold - summary.change.gas_work).abs() / (hot.abs() + cold.abs());
        worst_loop_mismatch = worst_loop_mismatch.max(mismatch);
        assert!(
            mismatch < LOOP_WORK_TOLERANCE,
            "loop areas {hot} + {cold} vs work {}: {at}",
            summary.change.gas_work
        );
        if let Some(efficiency) = summary
            .efficiency()
            .filter(|_| summary.change.gas_work > 0.0)
        {
            assert!(efficiency < summary.carnot_limit(), "beat Carnot: {at}");
        }
        if summary.is_settled() {
            let entropy = summary.surroundings_entropy_change();
            least_settled_entropy = least_settled_entropy.min(entropy);
            assert!(entropy > 0.0, "settled turn lowered entropy: {at}");
        }
    }
    println!(
        "  {} turns recorded; worst loop mismatch {:.3}%; least settled dS surr {:.3} (reduced)",
        outcome.turns.len(),
        100.0 * worst_loop_mismatch,
        least_settled_entropy
    );
}

/// Fixed-seed regression, including a halved timestep.
/// Run with `cargo test --release sustained_rotation -- --ignored --nocapture`.
#[test]
#[ignore = "long particle-driven cycle regression; run in release mode"]
fn sustained_rotation_requires_a_temperature_difference() {
    let cases = [
        (42, MAX_HOT_KELVIN, FIXED_DT),
        (7, MAX_HOT_KELVIN, FIXED_DT),
        (42, MAX_HOT_KELVIN, FIXED_DT / 2.0),
        (42, MIN_HOT_KELVIN, FIXED_DT),
        (7, MIN_HOT_KELVIN, FIXED_DT),
    ];
    for (seed, hot_kelvin, dt) in cases {
        check_started_run(
            seed,
            hot_kelvin,
            dt,
            &started_run(seed, hot_kelvin, dt, ROTATION_DURATION),
        );
    }
}

/// The same criteria on seeds drawn fresh from the clock each run, so the
/// result does not depend on a few hand-picked seeds. A failure prints its
/// seed, which can then be added to the fixed-seed test.
/// Run with `cargo test --release rotation_holds -- --ignored --nocapture`.
#[test]
#[ignore = "long random-seed cycle sweep; run in release mode"]
fn rotation_holds_for_random_seeds() {
    const HEATED_SEEDS: usize = 8;
    const EQUILIBRIUM_SEEDS: usize = 4;
    let clock = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    let random = RandGenerator::new();
    random.srand(clock);
    let cases: Vec<(u64, f32)> = (0..HEATED_SEEDS + EQUILIBRIUM_SEEDS)
        .map(|index| {
            let hot = if index < HEATED_SEEDS {
                MAX_HOT_KELVIN
            } else {
                MIN_HOT_KELVIN
            };
            (random.gen_range(0, u64::MAX), hot)
        })
        .collect();
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let results: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = cases
            .chunks(cases.len().div_ceil(threads))
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|&(seed, hot)| {
                            (
                                seed,
                                hot,
                                started_run(seed, hot, FIXED_DT, ROTATION_DURATION),
                            )
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect()
    });
    for (seed, hot, outcome) in &results {
        check_started_run(*seed, *hot, FIXED_DT, outcome);
    }
}

#[test]
fn starting_energy_grows_with_fill_above_the_default() {
    let default_energy = starting_energy(DEFAULT_PARTICLES);
    assert_eq!(starting_energy(MIN_PARTICLES), default_energy);
    assert_eq!(starting_energy(MAX_PARTICLES), 2.0 * default_energy);
    for count in [MIN_PARTICLES, DEFAULT_PARTICLES, MAX_PARTICLES] {
        let mut simulation = ConnectedChambers::seeded(count);
        assert!(simulation.start_engine());
        let energy = starting_energy(count);
        assert!((simulation.pistons.kinetic_energy() - energy).abs() < 1e-9 * energy);
        assert_eq!(
            simulation.pistons.starter_work(),
            simulation.pistons.kinetic_energy()
        );
        assert!(!simulation.start_engine(), "second press changed the speed");
    }
}

/// Pressing Start every frame of a heated run never leaves the flywheel
/// faster than it was before the press or than the starting speed, so
/// presses cannot overdrive the engine.
#[test]
fn repeated_start_presses_cannot_overdrive_the_engine() {
    const FRAMES: usize = 240;
    let mut simulation =
        ConnectedChambers::with_geometry_seed(DEFAULT_PARTICLES, DEFAULT_GEOMETRY, TEST_SEED);
    simulation.set_hot_kelvin(MAX_HOT_KELVIN);
    let start_energy = starting_energy(DEFAULT_PARTICLES);
    for frame in 0..FRAMES {
        let before = simulation.pistons.kinetic_energy();
        let work_before = simulation.pistons.starter_work();
        let changed = simulation.start_engine();
        let after = simulation.pistons.kinetic_energy();
        assert!(
            after <= before.max(start_energy) * (1.0 + 1e-9),
            "frame {frame}: press raised the energy from {before} to {after}"
        );
        assert!(simulation.pistons.angular_velocity() < 0.0);
        let booked = simulation.pistons.starter_work() - work_before;
        assert!((booked - (after - before)).abs() < 1e-9 * start_energy);
        assert_eq!(changed, booked != 0.0);
        simulation.update(1.0 / 60.0);
    }
    assert!(simulation.mechanical_balance_error().abs() < BALANCE_TOLERANCE);
}

/// One Start press must carry a resting crank through a full turn at the
/// smallest, default and largest fills, whatever the reservoirs and however
/// long the gas warmed or cooled first (the crank is free meanwhile, as in
/// the app). The hardest case measured is 2,048 particles at 1200/300 K,
/// where the old fixed kick rocked back after about half a turn.
/// Run with `cargo test --release one_start -- --ignored --nocapture`.
#[test]
#[ignore = "long one-press start regression; run in release mode"]
fn one_start_completes_a_turn_at_every_fill() {
    /// Measured first turns: about 5 to 8.5 s per unit of time scale (12.9
    /// to 21.0 s at 2.5; before v0.6's lighter flywheel, 31 to 51 s at 3).
    const MAX_FIRST_TURN: f64 = 25.0 * DYNAMICS_TIME_SCALE as f64;
    const WARM_UPS: [f32; 3] = [0.0, 2.0 * DYNAMICS_TIME_SCALE, 7.0 * DYNAMICS_TIME_SCALE];
    let mut cases = Vec::new();
    for count in [MIN_PARTICLES, DEFAULT_PARTICLES, MAX_PARTICLES] {
        for kelvin in [(1_200.0, 300.0), (1_200.0, 100.0), (300.0, 100.0)] {
            for warm_up in WARM_UPS {
                cases.push((cases.len() as u64 + 1, count, kelvin, warm_up));
            }
        }
    }
    let run = |&(seed, count, (hot, cold), warm_up): &(u64, usize, (f32, f32), f32)| {
        let mut simulation = ConnectedChambers::with_geometry_seed(count, DEFAULT_GEOMETRY, seed);
        simulation.set_hot_kelvin(hot);
        simulation.set_cold_kelvin(cold);
        for _ in 0..(warm_up / FIXED_DT) as usize {
            simulation.step(FIXED_DT);
        }
        let start_angle = simulation.pistons.total_angle();
        assert!(simulation.start_engine());
        let mut elapsed = 0.0_f64;
        let mut reversed = false;
        while elapsed < MAX_FIRST_TURN {
            simulation.step(FIXED_DT);
            elapsed += FIXED_DT as f64;
            reversed |= simulation.pistons.angular_velocity() >= 0.0;
            if simulation.pistons.total_angle() - start_angle <= -std::f64::consts::TAU {
                return (seed, count, hot, cold, warm_up, Some(elapsed), reversed);
            }
        }
        (seed, count, hot, cold, warm_up, None, reversed)
    };
    let results: Vec<_> = std::thread::scope(|scope| {
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
        let handles: Vec<_> = cases
            .chunks(cases.len().div_ceil(threads))
            .map(|chunk| scope.spawn(move || chunk.iter().map(run).collect::<Vec<_>>()))
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect()
    });
    for &(seed, count, hot, cold, warm_up, turn, reversed) in &results {
        println!(
            "seed {seed}, {count} particles, {hot}/{cold} K, warm-up {warm_up} s: first turn {turn:.1?} s{}",
            if reversed { ", reversed" } else { "" }
        );
    }
    for (seed, count, hot, cold, warm_up, turn, reversed) in results {
        let context =
            format!("seed {seed}, {count} particles, {hot}/{cold} K, warm-up {warm_up} s");
        assert!(
            turn.is_some(),
            "no full turn within {MAX_FIRST_TURN} s: {context}"
        );
        assert!(!reversed, "crank rocked back: {context}");
    }
}

#[test]
fn reset_draws_a_new_random_gas() {
    let mut simulation = ConnectedChambers::seeded(TEST_COUNT);
    let before: Vec<_> = simulation.particles.iter().map(|p| p.velocity).collect();
    simulation.reset();
    let after: Vec<_> = simulation.particles.iter().map(|p| p.velocity).collect();
    assert_ne!(before, after);
    assert_eq!(simulation.particles.len(), TEST_COUNT);
}

#[test]
#[ignore = "release-mode maximum-fill performance measurement"]
fn maximum_fill_benchmark() {
    const WARMUP_SECONDS: f32 = 2.0;
    const SAMPLE_SECONDS: f32 = 8.0;
    let mut simulation = ConnectedChambers::seeded(MAX_PARTICLES);
    simulation.set_hot_kelvin(MAX_HOT_KELVIN);
    simulation.start_engine();
    for _ in 0..(WARMUP_SECONDS / FIXED_DT) as usize {
        simulation.step(FIXED_DT);
    }
    let start = std::time::Instant::now();
    for _ in 0..(SAMPLE_SECONDS / FIXED_DT) as usize {
        simulation.step(FIXED_DT);
    }
    println!(
        "max-fill: {:.3} ms/model-second; gas residual={:.6}; shaft residual={:.6}",
        start.elapsed().as_secs_f64() * 1000.0 / SAMPLE_SECONDS as f64,
        simulation.energy_balance_error(),
        simulation.mechanical_balance_error()
    );
    std::hint::black_box(simulation);
}

#[test]
fn slow_frame_caps_catchup_without_larger_physics_steps() {
    const SLOW_FRAME: f32 = 4.0 * MAX_FRAME_TIME;
    let mut capped = ConnectedChambers::seeded(TEST_COUNT);
    let mut reference = ConnectedChambers::seeded(TEST_COUNT);
    assert!(capped.update(SLOW_FRAME));
    for _ in 0..MAX_FIXED_STEPS_PER_FRAME {
        reference.step(FIXED_DT);
    }
    for (actual, expected) in capped.particles.iter().zip(&reference.particles) {
        assert_eq!(actual.position, expected.position);
        assert_eq!(actual.velocity, expected.velocity);
    }
    assert!(capped.accumulator < FIXED_DT);
    assert!(!capped.update(0.0));
    assert!(!reference.update(FIXED_DT));
}

/// A display held at 30 FPS delivers frames of about 33 ms that arrive
/// unevenly. None of that time may be dropped: the budget must absorb the
/// late frames and leave no backlog. With the old budget of 4 steps, these
/// frames dropped time and flashed the warning.
#[test]
fn uneven_thirty_fps_frames_keep_real_time() {
    const FRAMES: usize = 240;
    const FRAME_TIMES: [f32; 6] = [1.0 / 30.0, 0.029, 0.0375, 0.0333, 0.041, 0.026];
    let mut simulation = ConnectedChambers::seeded(TEST_COUNT);
    for frame in 0..FRAMES {
        let limited = simulation.update(FRAME_TIMES[frame % FRAME_TIMES.len()]);
        assert!(!limited, "frame {frame} dropped time");
        assert!(simulation.accumulator < FIXED_DT);
    }
}

/// Tracers follow particles from both cylinders, record bounces as the gas
/// moves, leave the physics unchanged, and are cleared by a refill.
#[test]
fn tracers_follow_both_sides_without_changing_the_physics() {
    const FOLLOWED: usize = 5;
    const STEPS: usize = 120;
    let mut followed = ConnectedChambers::seeded(DEFAULT_PARTICLES);
    let mut plain = ConnectedChambers::seeded(DEFAULT_PARTICLES);
    for simulation in [&mut followed, &mut plain] {
        simulation.set_hot_kelvin(MAX_HOT_KELVIN);
        assert!(simulation.start_engine());
    }
    followed.follow_particles(FOLLOWED, TEST_SEED);
    let tracers = followed.tracers().unwrap();
    let indices: Vec<usize> = tracers.indices().collect();
    assert_eq!(indices.len(), FOLLOWED);
    let regions: Vec<Region> = indices
        .iter()
        .map(|&index| followed.region(followed.particles[index].position))
        .collect();
    assert!(regions.contains(&Region::Hot) && regions.contains(&Region::Cold));
    for _ in 0..STEPS {
        followed.step(FIXED_DT);
        plain.step(FIXED_DT);
    }
    let tracers = followed.tracers().unwrap();
    assert!((tracers.time() - STEPS as f64 * FIXED_DT as f64).abs() < 1e-6);
    assert!(tracers.paths().all(|(_, points)| points.len() > 1));
    for (a, b) in followed.particles.iter().zip(&plain.particles) {
        assert_eq!(a.position, b.position);
        assert_eq!(a.velocity, b.velocity);
    }
    followed.reset();
    assert!(followed.tracers().is_none());
}
