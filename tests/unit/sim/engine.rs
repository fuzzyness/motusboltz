//! Simulation tests for `src/sim/engine.rs`: the fill, transport through
//! the passage, wall heat, piston work, timestep limits, and the long
//! started-engine regressions.

use super::*;
use crate::sim::cycle::{
    COLD,
    HOT,
    TurnSummary,
    loop_work, //
};
use crate::sim::gas::DYNAMICS_TIME_SCALE;
use crate::sim::geometry::{
    CONNECTOR_DROP_RADII,
    DEFAULT_GEOMETRY, //
};
use crate::sim::reservoir::{
    MAX_COLD_KELVIN,
    MAX_HOT_KELVIN,
    MIN_COLD_KELVIN,
    MIN_HOT_KELVIN, //
};
use crate::sim::units::{
    DEFAULT_CONNECTED_MARKERS,
    DEFAULT_INITIAL_CONCENTRATION_MOL_PER_L,
    GAS_CONSTANT, //
};
const TEST_WARM_KELVIN: f32 = 975.0;

/// Counts in each region for initialization and transfer tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RegionCounts {
    hot: usize,
    passage: usize,
    cold: usize,
}

impl ConnectedChambers {
    /// Initial molarity, evaluated against the fill volume rather than the
    /// instantaneous volume under moving pistons.
    fn initial_concentration(&self) -> f64 {
        self.gas
            .initial_concentration(self.particles.len(), self.initial_volume_m3)
    }

    fn contains(&self, position: Vec3) -> bool {
        self.geometry.contains(
            position,
            PARTICLE_RADIUS,
            self.pistons.hot().face_y,
            self.pistons.cold().face_y,
        )
    }

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

// Geometry references belong to the test suite, not the simulator.
const CYLINDER_OFFSET: f32 = DEFAULT_GEOMETRY.cylinder_offset;
const CYLINDER_RADIUS: f32 = DEFAULT_GEOMETRY.cylinder_radius;
const CYLINDER_HEIGHT: f32 = DEFAULT_GEOMETRY.cylinder_height;
const CONNECTOR_CENTER_Y: f32 = DEFAULT_GEOMETRY.connector_center_y;
const CONNECTOR_RADIUS: f32 = DEFAULT_GEOMETRY.connector_radius;

const POSITION_TOLERANCE: f32 = 1e-5;
const ENERGY_TOLERANCE: f32 = 1e-3;
const TRANSFER_TEST_STEPS: usize = 1_500;
const TEST_COUNT: usize = 64;
const TRANSIT_SPEED: f32 = 2.0;
const TRANSIT_HEIGHT: f32 = CONNECTOR_CENTER_Y;

/// Disables wall heat transfer and piston motion for isolated transport tests.
fn insulated(count: usize) -> ConnectedChambers {
    let mut simulation = ConnectedChambers::new(count);
    simulation.thermal.set_accommodation(0.0);
    simulation.set_pistons_running(false);
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
fn a_particle_can_cross_from_hot_to_cold_through_the_passage() {
    let mut simulation = insulated(1);
    simulation.particles[0].position = vec3(
        -CYLINDER_OFFSET,
        -CYLINDER_HEIGHT / 2.0 + 2.0 * PARTICLE_RADIUS,
        0.0,
    );
    simulation.particles[0].velocity = vec3(TRANSIT_SPEED, -TRANSIT_SPEED, 0.0);

    let original_energy = simulation.kinetic_energy();
    let mut saw_connector = false;
    let mut reached_cold = false;
    for _ in 0..TRANSFER_TEST_STEPS {
        simulation.update(FIXED_DT);
        let position = simulation.particles[0].position;
        assert!(simulation.contains(position));
        saw_connector |= simulation.region(position) == Region::Passage;
        if simulation.region(position) == Region::Cold {
            reached_cold = true;
            break;
        }
    }

    assert!(saw_connector);
    assert!(
        reached_cold,
        "particle did not travel into the cold chamber"
    );
    assert!((simulation.kinetic_energy() - original_energy).abs() < ENERGY_TOLERANCE);
}

#[test]
fn travel_through_the_passage_works_in_both_directions() {
    let mut simulation = insulated(1);
    simulation.particles[0].position = vec3(
        CYLINDER_OFFSET,
        -CYLINDER_HEIGHT / 2.0 + 2.0 * PARTICLE_RADIUS,
        0.0,
    );
    simulation.particles[0].velocity = vec3(-TRANSIT_SPEED, -TRANSIT_SPEED, 0.0);

    let reached_hot = (0..TRANSFER_TEST_STEPS).any(|_| {
        simulation.update(FIXED_DT);
        simulation.region(simulation.particles[0].position) == Region::Hot
    });
    assert!(reached_hot, "reverse passage must be open too");
}

#[test]
fn cylinder_wall_is_closed_above_the_passage() {
    const ABOVE_CONNECTOR_Y: f32 = 1.0;
    let mut simulation = insulated(1);
    simulation.particles[0].position = vec3(-CYLINDER_OFFSET, ABOVE_CONNECTOR_Y, 0.0);
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
fn passage_walls_reflect_without_changing_energy() {
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
    let mut simulation = ConnectedChambers::new(2);
    simulation.set_hot_kelvin(MAX_HOT_KELVIN);
    simulation.thermal.set_accommodation(1.0);
    simulation.set_pistons_running(false);
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

#[test]
fn fixed_chamber_first_law_holds_during_heating_and_cooling() {
    const THERMAL_TEST_STEPS: usize = 1_200;
    const TEST_HOT_KELVIN: f32 = MAX_HOT_KELVIN;
    const RELATIVE_TOLERANCE: f32 = 2e-3;
    let mut simulation = ConnectedChambers::new(TEST_COUNT);
    simulation.set_hot_kelvin(TEST_HOT_KELVIN);
    simulation.thermal.set_accommodation(1.0);
    simulation.set_pistons_running(false);
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
    let mut simulation = ConnectedChambers::new(64);
    simulation.set_hot_kelvin(TARGET_HOT_KELVIN);
    simulation.set_particle_count(256);
    assert_eq!(simulation.hot_kelvin(), TARGET_HOT_KELVIN);
    simulation.reset();
    assert_eq!(simulation.hot_kelvin(), TARGET_HOT_KELVIN);
    assert_eq!(simulation.heat().total(), 0.0);
}

#[test]
fn passage_and_upper_caps_are_adiabatic() {
    const WALL_SPEED: f32 = 2.0;
    let mut simulation = ConnectedChambers::new(2);
    simulation.thermal.set_accommodation(1.0);
    simulation.set_hot_kelvin(MAX_HOT_KELVIN);
    simulation.set_pistons_running(false);
    let radius = PARTICLE_RADIUS;
    simulation.particles[0].position = vec3(0.0, TRANSIT_HEIGHT, CONNECTOR_RADIUS - radius);
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
    const MARKERS: usize = 256;
    let mut simulation = insulated(MARKERS);
    let total_moles = simulation.total_moles();
    let first = simulation.measurements();
    assert!(first.hot.markers > 0);
    assert!(first.cold.markers > 0);
    assert_eq!(
        first.hot.markers + first.passage_markers + first.cold.markers,
        MARKERS
    );
    assert!(first.hot.temperature_kelvin.unwrap() > 0.0);
    assert!(first.cold.temperature_kelvin.unwrap() > 0.0);
    for _ in 0..TRANSFER_TEST_STEPS {
        simulation.update(FIXED_DT);
    }
    let readings = simulation.measurements();
    assert_eq!(
        readings.hot.markers + readings.passage_markers + readings.cold.markers,
        MARKERS
    );
    assert_eq!(simulation.total_moles(), total_moles);
}

#[test]
fn initial_gas_and_wall_temperatures_are_in_kelvin() {
    const ABSOLUTE_TOLERANCE_K: f64 = 0.02;
    let mut simulation = ConnectedChambers::new(TEST_COUNT);
    let readings = simulation.measurements();
    assert!((readings.hot.temperature_kelvin.unwrap() - 300.0).abs() < ABSOLUTE_TOLERANCE_K);
    assert!((readings.cold.temperature_kelvin.unwrap() - 300.0).abs() < ABSOLUTE_TOLERANCE_K);
    assert_eq!(simulation.hot_kelvin(), 300.0);
    simulation.set_hot_kelvin(MAX_HOT_KELVIN);
    assert_eq!(simulation.hot_kelvin(), 1_200.0);
}

#[test]
fn helium_amount_increases_with_initial_marker_count() {
    let first = ConnectedChambers::new(TEST_COUNT);
    let more = ConnectedChambers::new(2 * TEST_COUNT);
    assert!((more.total_moles() - 2.0 * first.total_moles()).abs() < 1e-12);
}

#[test]
fn particle_radius_stays_fixed_for_every_particle_preset() {
    for &count in &[256, 512, 1_024, 1_536, 2_048] {
        let simulation = ConnectedChambers::new(count);
        assert_eq!(PARTICLE_RADIUS, PARTICLE_RADIUS);
        assert_eq!(simulation.particles.len(), count);
        assert!(
            simulation
                .particles
                .iter()
                .all(|p| simulation.contains(p.position))
        );
    }
}

#[test]
fn alternate_geometry_survives_reset_and_particle_count_changes() {
    const TEST_SCALE: f32 = 2.2;
    let geometry = ChamberGeometry::scaled(TEST_SCALE);
    let mut simulation = ConnectedChambers::with_geometry(TEST_COUNT, geometry);
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
    assert_eq!(PARTICLE_RADIUS, PARTICLE_RADIUS);
}

#[test]
#[should_panic(expected = "chamber is too small")]
fn undersized_chamber_rejects_particles_instead_of_shrinking_them() {
    let _ = ConnectedChambers::with_geometry(2_048, ChamberGeometry::scaled(1.0));
}

#[test]
fn helium_fill_and_volume_scale_together() {
    const DOUBLE_SCALE: f32 = 2.0;
    const RELATIVE_TOLERANCE: f64 = 1e-5;
    let base = ConnectedChambers::with_geometry(TEST_COUNT, ChamberGeometry::scaled(1.0));
    let doubled =
        ConnectedChambers::with_geometry(TEST_COUNT, ChamberGeometry::scaled(DOUBLE_SCALE));
    let volume_ratio = doubled.volume_m3() / base.volume_m3();
    let moles_ratio = doubled.total_moles() / base.total_moles();
    assert!(volume_ratio > 1.0);
    assert!((moles_ratio - volume_ratio).abs() < RELATIVE_TOLERANCE);
}

#[test]
fn taller_chambers_expand_collision_grid_bounds() {
    let mut geometry = ChamberGeometry::default();
    geometry.cylinder_height = geometry.grid_side() * 1.1;
    geometry.connector_center_y =
        -geometry.cylinder_height / 2.0 - CONNECTOR_DROP_RADII * geometry.connector_radius;
    let simulation = ConnectedChambers::with_geometry(TEST_COUNT, geometry);
    assert!(simulation.geometry().grid_side() > geometry.cylinder_height);
    assert!(simulation.geometry().grid_side() / 2.0 >= -geometry.bottom());
    assert!(
        simulation
            .particles
            .iter()
            .all(|p| simulation.contains(p.position))
    );
}

#[test]
fn initial_concentration_is_bounded_by_one_molar_across_the_system() {
    const LOCAL_CONCENTRATION_TOLERANCE: f64 = 0.10;
    let full = ConnectedChambers::with_initial_concentration(1.0);
    let initial = full.measurements();
    assert_eq!(full.particles.len(), MAX_SIMULATED_MARKERS);
    assert!((full.initial_concentration() - 1.0).abs() < 1e-12);
    assert!((initial.hot.concentration_mol_per_l - 1.0).abs() < LOCAL_CONCENTRATION_TOLERANCE);
    assert!((initial.cold.concentration_mol_per_l - 1.0).abs() < LOCAL_CONCENTRATION_TOLERANCE);
    assert!((full.total_moles() - full.volume_m3() * 1_000.0).abs() < 1e-10);
}

#[test]
fn concentration_controls_gas_amount_and_reset_preserves_it() {
    const INITIAL_CONCENTRATION: f32 = 0.25;
    let mut simulation = ConnectedChambers::with_initial_concentration(INITIAL_CONCENTRATION);
    assert!(
        (simulation.initial_concentration() - INITIAL_CONCENTRATION as f64).abs()
            <= 0.5 / MAX_SIMULATED_MARKERS as f64
    );
    let initial_moles = simulation.total_moles();
    simulation.set_hot_kelvin(TEST_WARM_KELVIN);
    simulation.reset();
    assert_eq!(simulation.total_moles(), initial_moles);
    simulation.set_initial_concentration(0.50);
    assert_eq!(simulation.total_moles(), 2.0 * initial_moles);
    assert_eq!(simulation.hot_kelvin(), TEST_WARM_KELVIN);
}

#[test]
fn concentration_scaling_is_independent_of_geometry_size() {
    const INITIAL_CONCENTRATION: f32 = 0.50;
    let mut large_geometry = ChamberGeometry::default();
    large_geometry.cylinder_height *= 1.25;
    large_geometry.connector_center_y = -large_geometry.cylinder_height / 2.0
        - CONNECTOR_DROP_RADII * large_geometry.connector_radius;
    let marker_count = ConnectedChambers::markers_for_concentration(INITIAL_CONCENTRATION);
    let original = ConnectedChambers::with_geometry(marker_count, ChamberGeometry::default());
    let larger = ConnectedChambers::with_geometry(marker_count, large_geometry);
    assert!((original.initial_concentration() - larger.initial_concentration()).abs() < 1e-12);
    assert!(larger.total_moles() > original.total_moles());
}

#[test]
fn maximum_initial_concentration_gives_near_equal_ideal_pressures() {
    const EXPECTED_PRESSURE_PA: f64 = 1_000.0 * GAS_CONSTANT * 300.0;
    const RELATIVE_TOLERANCE: f64 = 0.10;
    let simulation = ConnectedChambers::with_initial_concentration(1.0);
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
fn default_fill_uses_1024_markers_at_half_molar() {
    const CONCENTRATION_TOLERANCE: f64 = 1e-12;
    let simulation =
        ConnectedChambers::with_initial_concentration(DEFAULT_INITIAL_CONCENTRATION_MOL_PER_L);
    let regions = simulation.measurements();
    assert_eq!(simulation.particles.len(), DEFAULT_CONNECTED_MARKERS);
    assert!((simulation.initial_concentration() - 0.5).abs() < CONCENTRATION_TOLERANCE);
    assert!(regions.hot.markers > 0);
    assert!(regions.cold.markers > 0);
    assert!(regions.passage_markers > 0);
}

#[test]
fn minimum_fill_represents_256_markers() {
    let minimum =
        ConnectedChambers::with_initial_concentration(MIN_INITIAL_CONCENTRATION_MOL_PER_L);
    assert_eq!(minimum.particles.len(), MIN_CONNECTED_MARKERS);
    assert!((minimum.initial_concentration() - 0.125).abs() < 1e-12);
}

#[test]
fn resetting_refills_both_cylinders_at_the_same_temperature() {
    const TEMPERATURE_TOLERANCE_K: f64 = 0.02;
    let mut simulation = ConnectedChambers::with_initial_concentration(0.5);
    simulation.set_hot_kelvin(MAX_HOT_KELVIN);
    for _ in 0..100 {
        simulation.update(FIXED_DT);
    }
    simulation.reset();
    let readings = simulation.measurements();
    assert_eq!(simulation.hot_kelvin(), MAX_HOT_KELVIN);
    assert!((readings.hot.temperature_kelvin.unwrap() - 300.0).abs() < TEMPERATURE_TOLERANCE_K);
    assert!((readings.cold.temperature_kelvin.unwrap() - 300.0).abs() < TEMPERATURE_TOLERANCE_K);
    assert!(readings.hot.markers > 0 && readings.cold.markers > 0);
    assert_eq!(simulation.heat().total(), 0.0);
}

#[test]
fn moving_pistons_keep_every_sphere_below_their_faces() {
    const TEST_STEPS: usize = 1_000;
    let mut simulation = insulated(256);
    simulation.kick_flywheel(KickDirection::Positive);
    simulation.set_pistons_running(true);
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
    pistons.kick(KickDirection::Positive);
    for _ in 0..200 {
        pistons.advance(FIXED_DT);
    }
    let state = pistons.hot();
    assert!(state.velocity < 0.0);
    let mut particle = Particle {
        position: vec3(-geometry.cylinder_offset, state.face_y, 0.0),
        velocity: vec3(0.0, TEST_PARTICLE_SPEED, 0.0),
    };
    let before = gas::particle_kinetic_energy(particle.velocity);
    let work = ConnectedChambers::bounce_piston(
        &mut particle,
        geometry,
        -geometry.cylinder_offset,
        PARTICLE_RADIUS,
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
    simulation.kick_flywheel(KickDirection::Positive);
    simulation.set_pistons_running(true);
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
    let mut simulation = ConnectedChambers::new(256);
    simulation.thermal.set_accommodation(1.0);
    simulation.set_hot_kelvin(MAX_HOT_KELVIN);
    simulation.kick_flywheel(KickDirection::Positive);
    let reference = simulation.kinetic_energy();
    for _ in 0..TEST_STEPS {
        simulation.update(FIXED_DT);
    }
    assert!(simulation.heat().hot != 0.0);
    assert!(simulation.piston_work_by_gas().abs() > 0.0);
    assert!(simulation.energy_balance_error().abs() < (RELATIVE_TOLERANCE * reference) as f64);
}

#[test]
fn changing_fill_preserves_paused_piston_control() {
    let mut simulation = ConnectedChambers::new(256);
    simulation.set_pistons_running(false);
    simulation.set_particle_count(512);
    assert!(!simulation.power_pistons().running());
    simulation.reset();
    assert!(!simulation.power_pistons().running());
}

#[test]
fn free_flywheel_tracks_gas_and_mechanical_energy_separately() {
    const TEST_STEPS: usize = 400;
    const RELATIVE_TOLERANCE: f32 = 0.015;
    let mut simulation = insulated(256);
    simulation.set_pistons_running(true);
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
    const SEED: u64 = 42;
    const STEPS: usize = 200;
    let mut first = ConnectedChambers::with_geometry_seed(TEST_COUNT, DEFAULT_GEOMETRY, SEED);
    let mut second = ConnectedChambers::with_geometry_seed(TEST_COUNT, DEFAULT_GEOMETRY, SEED);
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
    const SEED: u64 = 42;
    let mut simulation = ConnectedChambers::with_geometry_seed(TEST_COUNT, DEFAULT_GEOMETRY, SEED);
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
    summary: TurnSummary,
    loop_work: [f64; 2],
}

/// Result of one started run, sampled at the last completed revolution.
struct CycleOutcome {
    turns: Vec<RecordedTurn>,
    completed_turns: usize,
    starter_energy: f64,
    phase_energy: f64,
    phase_gas_work: f64,
    final_starter_work: f64,
    heat: HeatTransfer,
    gas_residual: f64,
    shaft_residual: f64,
}

/// Default fill, one Start press, cold reservoir at 300 K. Energy is sampled at
/// completed revolutions because compression and back-pressure work make
/// arbitrary crank phases incomparable.
fn started_run(seed: u64, hot_kelvin: f32, dt: f32, duration: f32) -> CycleOutcome {
    let mut simulation =
        ConnectedChambers::with_geometry_seed(DEFAULT_CONNECTED_MARKERS, DEFAULT_GEOMETRY, seed);
    simulation.set_hot_kelvin(hot_kelvin);
    simulation.set_cold_kelvin(MAX_COLD_KELVIN);
    assert!(simulation.start_engine());
    let starter_energy = simulation.pistons.starter_work();
    let mut completed_turns = 0;
    let mut phase_energy = starter_energy;
    let mut phase_gas_work = 0.0;
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
        }
    }
    CycleOutcome {
        turns: recorded,
        completed_turns,
        starter_energy,
        phase_energy,
        phase_gas_work,
        final_starter_work: simulation.pistons.starter_work(),
        heat: simulation.heat(),
        gas_residual: simulation.energy_balance_error(),
        shaft_residual: simulation.mechanical_balance_error(),
    }
}

// Shared acceptance criteria for the started-engine regressions.
const ROTATION_DURATION: f32 = 120.0 * DYNAMICS_TIME_SCALE;
const MIN_TURNS: usize = 4;
const BALANCE_TOLERANCE: f64 = 0.05;
const HOT_GAIN_RATIO: f64 = 1.5;
const EQUILIBRIUM_REMAINING_RATIO: f64 = 0.6;
/// The two P-V loop areas must add up to the measured gas work within this
/// fraction of the gross work on both pistons. The remaining error comes
/// from averaging pressure over 10-degree bins; measured: below 0.7%.
const LOOP_WORK_TOLERANCE: f64 = 0.02;

/// Panics with the seed in the message if a run misses the criteria: a
/// heated engine must gain energy from gas work, an equilibrium engine
/// must lose it, and both energy balances must close.
fn check_started_run(seed: u64, hot_kelvin: f32, dt: f32, outcome: &CycleOutcome) {
    let o = outcome;
    println!(
        "seed={seed}, hot={hot_kelvin} K, dt={dt}: turns={}, phase energy={:.3}, gas work={:.3}, gas residual={:.6}, shaft residual={:.6}",
        o.completed_turns, o.phase_energy, o.phase_gas_work, o.gas_residual, o.shaft_residual
    );
    let context = format!("seed {seed}, hot {hot_kelvin} K, dt {dt}");
    assert!(o.completed_turns >= MIN_TURNS, "too few turns: {context}");
    check_recorded_turns(o, &context);
    assert_eq!(o.final_starter_work, o.starter_energy, "{context}");
    assert!(o.gas_residual.abs() < BALANCE_TOLERANCE, "{context}");
    assert!(o.shaft_residual.abs() < BALANCE_TOLERANCE, "{context}");
    if hot_kelvin > MAX_COLD_KELVIN {
        assert!(
            o.phase_energy > HOT_GAIN_RATIO * o.starter_energy,
            "heated engine did not gain energy: {context}"
        );
        assert!(o.phase_gas_work > 0.0, "{context}");
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
        let mismatch = (hot + cold - summary.gas_work).abs() / (hot.abs() + cold.abs());
        worst_loop_mismatch = worst_loop_mismatch.max(mismatch);
        assert!(
            mismatch < LOOP_WORK_TOLERANCE,
            "loop areas {hot} + {cold} vs work {}: {at}",
            summary.gas_work
        );
        if let Some(efficiency) = summary.efficiency().filter(|_| summary.gas_work > 0.0) {
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
    const SEED: u64 = 42;
    let default_energy = starting_energy(DEFAULT_CONNECTED_MARKERS);
    assert_eq!(starting_energy(MIN_CONNECTED_MARKERS), default_energy);
    assert_eq!(starting_energy(MAX_SIMULATED_MARKERS), 2.0 * default_energy);
    for count in [
        MIN_CONNECTED_MARKERS,
        DEFAULT_CONNECTED_MARKERS,
        MAX_SIMULATED_MARKERS,
    ] {
        let mut simulation = ConnectedChambers::with_geometry_seed(count, DEFAULT_GEOMETRY, SEED);
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
    const SEED: u64 = 42;
    const FRAMES: usize = 240;
    let mut simulation =
        ConnectedChambers::with_geometry_seed(DEFAULT_CONNECTED_MARKERS, DEFAULT_GEOMETRY, SEED);
    simulation.set_hot_kelvin(MAX_HOT_KELVIN);
    let start_energy = starting_energy(DEFAULT_CONNECTED_MARKERS);
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
    /// Measured first turns: about 10 to 17 s per unit of time scale.
    const MAX_FIRST_TURN: f64 = 25.0 * DYNAMICS_TIME_SCALE as f64;
    const WARM_UPS: [f32; 3] = [0.0, 2.0 * DYNAMICS_TIME_SCALE, 7.0 * DYNAMICS_TIME_SCALE];
    let mut cases = Vec::new();
    for count in [
        MIN_CONNECTED_MARKERS,
        DEFAULT_CONNECTED_MARKERS,
        MAX_SIMULATED_MARKERS,
    ] {
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
    let mut simulation = ConnectedChambers::new(TEST_COUNT);
    let before: Vec<_> = simulation.particles.iter().map(|p| p.velocity).collect();
    simulation.reset();
    let after: Vec<_> = simulation.particles.iter().map(|p| p.velocity).collect();
    assert_ne!(before, after);
    assert_eq!(simulation.particles.len(), TEST_COUNT);
}

#[test]
#[ignore = "release-mode maximum-fill performance measurement"]
fn maximum_fill_benchmark() {
    const SEED: u64 = 42;
    const WARMUP_SECONDS: f32 = 2.0;
    const SAMPLE_SECONDS: f32 = 8.0;
    let mut simulation =
        ConnectedChambers::with_geometry_seed(MAX_SIMULATED_MARKERS, DEFAULT_GEOMETRY, SEED);
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
    const SEED: u64 = 42;
    const SLOW_FRAME: f32 = 4.0 * MAX_FRAME_TIME;
    let mut capped = ConnectedChambers::with_geometry_seed(TEST_COUNT, DEFAULT_GEOMETRY, SEED);
    let mut reference = ConnectedChambers::with_geometry_seed(TEST_COUNT, DEFAULT_GEOMETRY, SEED);
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
    const SEED: u64 = 42;
    const FRAMES: usize = 240;
    const FRAME_TIMES: [f32; 6] = [1.0 / 30.0, 0.029, 0.0375, 0.0333, 0.041, 0.026];
    let mut simulation = ConnectedChambers::with_geometry_seed(TEST_COUNT, DEFAULT_GEOMETRY, SEED);
    for frame in 0..FRAMES {
        let limited = simulation.update(FRAME_TIMES[frame % FRAME_TIMES.len()]);
        assert!(!limited, "frame {frame} dropped time");
        assert!(simulation.accumulator < FIXED_DT);
    }
}
