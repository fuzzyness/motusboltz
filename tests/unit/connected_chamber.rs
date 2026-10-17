//! Connected-chamber unit tests for `src/engine.rs`.

use super::*;

/// Counts in each region for initialization and transfer tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RegionCounts {
    left: usize,
    connector: usize,
    right: usize,
}

impl ConnectedChambers {
    /// Initial molarity, evaluated against the fill volume rather than the
    /// instantaneous volume under moving pistons.
    fn initial_concentration(&self) -> f64 {
        self.gas
            .initial_concentration(self.particles.len(), self.initial_volume_m3)
    }

    fn hot_temperature(&self) -> f32 {
        self.thermal.hot_temperature()
    }

    fn contains(&self, position: Vec3) -> bool {
        Self::contains_at(
            self.geometry,
            position,
            PARTICLE_RADIUS,
            self.pistons.left().face_y,
            self.pistons.right().face_y,
        )
    }

    fn counts(&self) -> RegionCounts {
        let mut counts = RegionCounts::default();
        for particle in &self.particles {
            match self.region(particle.position) {
                Region::Left => counts.left += 1,
                Region::Connector => counts.connector += 1,
                Region::Right => counts.right += 1,
            }
        }
        counts
    }
}

fn to_kelvin(reduced_temperature: f32) -> f64 {
    reduced_temperature as f64 * crate::gas::KELVIN_PER_UNIT
}

// Geometry references belong to the test suite, not the simulator.
const CYLINDER_OFFSET: f32 = DEFAULT_GEOMETRY.cylinder_offset;
const CYLINDER_RADIUS: f32 = DEFAULT_GEOMETRY.cylinder_radius;
const CYLINDER_HEIGHT: f32 = DEFAULT_GEOMETRY.cylinder_height;
const CONNECTOR_CENTER_Y: f32 = DEFAULT_GEOMETRY.connector_center_y;
const CONNECTOR_RADIUS: f32 = DEFAULT_GEOMETRY.connector_radius;
const MAX_PARTICLE_RADIUS: f32 = PARTICLE_RADIUS;

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
        assert!(regions.left > 0);
        assert!(regions.right > 0);
        assert_eq!(regions.left + regions.connector + regions.right, count);
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
                        >= 2.0 * simulation.particle_radius()
                );
            }
        }
    }
}

#[test]
fn connector_stops_just_inside_the_facing_cylinder_walls() {
    let geometry = ChamberGeometry::default();
    let inner_wall_x = geometry.cylinder_offset - geometry.cylinder_radius;
    let tube_end_x = geometry.connector_half_length();
    assert!(tube_end_x > inner_wall_x);
    assert!(tube_end_x - inner_wall_x < geometry.connector_radius);
    assert!(tube_end_x < geometry.cylinder_offset);
    assert!(2.0 * tube_end_x < 2.0 * geometry.cylinder_offset);
}

#[test]
fn shortened_connector_opens_into_both_cylinders() {
    let geometry = ChamberGeometry::default();
    let tube_end_x = geometry.connector_half_length();
    let z = geometry.connector_radius - PARTICLE_RADIUS;
    for x in [-tube_end_x, tube_end_x] {
        let position = vec3(x, geometry.connector_center_y, z);
        let bore_center = if x < 0.0 {
            -geometry.cylinder_offset
        } else {
            geometry.cylinder_offset
        };
        assert!(ConnectedChambers::within_connector(
            geometry,
            position,
            PARTICLE_RADIUS
        ));
        assert!(ConnectedChambers::within_cylinder(
            geometry,
            position,
            bore_center,
            PARTICLE_RADIUS,
            geometry.cylinder_height / 2.0,
        ));
    }
}

#[test]
fn connector_projection_respects_the_shortened_ends() {
    let geometry = ChamberGeometry::default();
    let tube_end_x = geometry.connector_half_length();
    for x in [-geometry.cylinder_offset, geometry.cylinder_offset] {
        let point = vec3(x, geometry.connector_center_y, 0.0);
        let corrected = ConnectedChambers::project_to_connector(geometry, point, PARTICLE_RADIUS);
        assert!((corrected.x.abs() - tube_end_x).abs() < 1e-5);
        assert!(ConnectedChambers::within_connector(
            geometry,
            corrected,
            PARTICLE_RADIUS,
        ));
        assert!(!ConnectedChambers::within_connector(
            geometry,
            point,
            PARTICLE_RADIUS,
        ));
    }
}

#[test]
fn a_particle_can_cross_from_left_to_right_through_the_connector() {
    let mut simulation = insulated(1);
    simulation.particles[0].position = vec3(-CYLINDER_OFFSET, TRANSIT_HEIGHT, 0.0);
    simulation.particles[0].velocity = vec3(TRANSIT_SPEED, 0.0, 0.0);

    let original_energy = simulation.kinetic_energy();
    let mut saw_connector = false;
    let mut reached_right = false;
    for _ in 0..TRANSFER_TEST_STEPS {
        simulation.update(FIXED_DT);
        let position = simulation.particles[0].position;
        assert!(simulation.contains(position));
        saw_connector |= simulation.region(position) == Region::Connector;
        if simulation.region(position) == Region::Right {
            reached_right = true;
            break;
        }
    }

    assert!(saw_connector);
    assert!(
        reached_right,
        "particle did not travel into the right chamber"
    );
    assert!((simulation.kinetic_energy() - original_energy).abs() < ENERGY_TOLERANCE);
}

#[test]
fn connector_sits_below_center_with_clearance_for_rounded_bottoms() {
    let bottom = -CYLINDER_HEIGHT / 2.0;
    let top = CYLINDER_HEIGHT / 2.0;
    assert!(bottom < CONNECTOR_CENTER_Y - CONNECTOR_RADIUS);
    const {
        assert!(CONNECTOR_CENTER_Y + CONNECTOR_RADIUS < 0.0);
    }
    assert!(CONNECTOR_CENTER_Y + CONNECTOR_RADIUS < top);
    assert!(CONNECTOR_CENTER_Y - CONNECTOR_RADIUS - bottom > MAX_PARTICLE_RADIUS);
}

#[test]
fn travel_through_the_connector_works_in_both_directions() {
    let mut simulation = insulated(1);
    simulation.particles[0].position = vec3(CYLINDER_OFFSET, TRANSIT_HEIGHT, 0.0);
    simulation.particles[0].velocity = vec3(-TRANSIT_SPEED, 0.0, 0.0);

    let reached_left = (0..TRANSFER_TEST_STEPS).any(|_| {
        simulation.update(FIXED_DT);
        simulation.region(simulation.particles[0].position) == Region::Left
    });
    assert!(reached_left, "reverse passage must be open too");
}

#[test]
fn cylinder_wall_is_closed_above_the_connector() {
    const ABOVE_CONNECTOR_Y: f32 = 1.0;
    let mut simulation = insulated(1);
    simulation.particles[0].position = vec3(-CYLINDER_OFFSET, ABOVE_CONNECTOR_Y, 0.0);
    simulation.particles[0].velocity = vec3(TRANSIT_SPEED, 0.0, 0.0);
    let original_energy = simulation.kinetic_energy();

    for _ in 0..TRANSFER_TEST_STEPS {
        simulation.update(FIXED_DT);
        let position = simulation.particles[0].position;
        assert!(simulation.contains(position));
        assert_eq!(simulation.region(position), Region::Left);
    }
    assert!((simulation.kinetic_energy() - original_energy).abs() < ENERGY_TOLERANCE);
}

#[test]
fn connector_walls_reflect_without_changing_energy() {
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
    assert_eq!(counts.left + counts.connector + counts.right, TEST_COUNT);
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
    simulation.set_heater_percent(100.0);
    simulation.thermal.set_accommodation(1.0);
    simulation.set_pistons_running(false);
    let x = CYLINDER_OFFSET + CYLINDER_RADIUS - simulation.particle_radius();
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
    assert!((simulation.kinetic_energy() - before - heat.total()).abs() < ENERGY_BALANCE_TOLERANCE);
}

#[test]
fn fixed_chamber_first_law_holds_during_heating_and_cooling() {
    const THERMAL_TEST_STEPS: usize = 1_200;
    const TEST_HEATER: f32 = 100.0;
    const RELATIVE_TOLERANCE: f32 = 2e-3;
    let mut simulation = ConnectedChambers::new(TEST_COUNT);
    simulation.set_heater_percent(TEST_HEATER);
    simulation.thermal.set_accommodation(1.0);
    simulation.set_pistons_running(false);
    let initial_energy = simulation.kinetic_energy();
    for _ in 0..THERMAL_TEST_STEPS {
        simulation.update(FIXED_DT);
    }
    assert!(simulation.heat().hot != 0.0);
    assert!(
        simulation.energy_balance_error().abs()
            < RELATIVE_TOLERANCE * initial_energy.max(simulation.kinetic_energy())
    );
}

#[test]
fn heater_and_cumulative_heat_reset_with_new_gas() {
    const TARGET_HEATER: f32 = 75.0;
    let mut simulation = ConnectedChambers::new(64);
    simulation.set_heater_percent(TARGET_HEATER);
    simulation.set_particle_count(256);
    assert_eq!(simulation.heater_percent(), TARGET_HEATER);
    simulation.reset();
    assert_eq!(simulation.heater_percent(), TARGET_HEATER);
    assert_eq!(simulation.heat().total(), 0.0);
}

#[test]
fn passage_and_upper_caps_are_adiabatic() {
    const WALL_SPEED: f32 = 2.0;
    let mut simulation = ConnectedChambers::new(2);
    simulation.thermal.set_accommodation(1.0);
    simulation.set_heater_percent(100.0);
    simulation.set_pistons_running(false);
    let radius = simulation.particle_radius();
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
    assert!(first.left.markers > 0);
    assert!(first.right.markers > 0);
    assert_eq!(
        first.left.markers + first.passage_markers + first.right.markers,
        MARKERS
    );
    assert!(first.left.temperature_kelvin.unwrap() > 0.0);
    assert!(first.right.temperature_kelvin.unwrap() > 0.0);
    for _ in 0..TRANSFER_TEST_STEPS {
        simulation.update(FIXED_DT);
    }
    let readings = simulation.measurements();
    assert_eq!(
        readings.left.markers + readings.passage_markers + readings.right.markers,
        MARKERS
    );
    assert_eq!(simulation.total_moles(), total_moles);
}

#[test]
fn initial_gas_and_wall_temperatures_are_in_kelvin() {
    const ABSOLUTE_TOLERANCE_K: f64 = 0.02;
    let mut simulation = ConnectedChambers::new(TEST_COUNT);
    let readings = simulation.measurements();
    assert!((readings.left.temperature_kelvin.unwrap() - 300.0).abs() < ABSOLUTE_TOLERANCE_K);
    assert!((readings.right.temperature_kelvin.unwrap() - 300.0).abs() < ABSOLUTE_TOLERANCE_K);
    assert_eq!(to_kelvin(simulation.hot_temperature()), 300.0);
    simulation.set_heater_percent(100.0);
    assert_eq!(to_kelvin(simulation.hot_temperature()), 1_200.0);
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
        assert_eq!(simulation.particle_radius(), PARTICLE_RADIUS);
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
    simulation.set_heater_percent(75.0);
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
    assert_eq!(simulation.heater_percent(), 75.0);
    assert_eq!(simulation.particle_radius(), PARTICLE_RADIUS);
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
    let simulation = ConnectedChambers::with_geometry(TEST_COUNT, geometry);
    assert_eq!(simulation.geometry().grid_side(), geometry.cylinder_height);
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
    assert_eq!(full.particles.len(), crate::gas::MAX_SIMULATED_MARKERS);
    assert!((full.initial_concentration() - 1.0).abs() < 1e-12);
    assert!((initial.left.concentration_mol_per_l - 1.0).abs() < LOCAL_CONCENTRATION_TOLERANCE);
    assert!((initial.right.concentration_mol_per_l - 1.0).abs() < LOCAL_CONCENTRATION_TOLERANCE);
    assert!((full.total_moles() - full.volume_m3() * 1_000.0).abs() < 1e-10);
}

#[test]
fn concentration_controls_gas_amount_and_reset_preserves_it() {
    const INITIAL_CONCENTRATION: f32 = 0.25;
    let mut simulation = ConnectedChambers::with_initial_concentration(INITIAL_CONCENTRATION);
    assert!(
        (simulation.initial_concentration() - INITIAL_CONCENTRATION as f64).abs()
            <= 0.5 / crate::gas::MAX_SIMULATED_MARKERS as f64
    );
    let initial_moles = simulation.total_moles();
    simulation.set_heater_percent(75.0);
    simulation.reset();
    assert_eq!(simulation.total_moles(), initial_moles);
    simulation.set_initial_concentration(0.50);
    assert_eq!(simulation.total_moles(), 2.0 * initial_moles);
    assert_eq!(simulation.heater_percent(), 75.0);
}

#[test]
fn concentration_scaling_is_independent_of_geometry_size() {
    const INITIAL_CONCENTRATION: f32 = 0.50;
    let mut large_geometry = ChamberGeometry::default();
    large_geometry.cylinder_height *= 1.25;
    let marker_count = ConnectedChambers::markers_for_concentration(INITIAL_CONCENTRATION);
    let original = ConnectedChambers::with_geometry(marker_count, ChamberGeometry::default());
    let larger = ConnectedChambers::with_geometry(marker_count, large_geometry);
    assert!((original.initial_concentration() - larger.initial_concentration()).abs() < 1e-12);
    assert!(larger.total_moles() > original.total_moles());
}

#[test]
fn maximum_initial_concentration_gives_near_equal_ideal_pressures() {
    const EXPECTED_PRESSURE_PA: f64 = 1_000.0 * crate::gas::GAS_CONSTANT * 300.0;
    const RELATIVE_TOLERANCE: f64 = 0.10;
    let simulation = ConnectedChambers::with_initial_concentration(1.0);
    let readings = simulation.measurements();
    for pressure in [
        readings.left.pressure_pa.unwrap(),
        readings.right.pressure_pa.unwrap(),
    ] {
        assert!(
            ((pressure - EXPECTED_PRESSURE_PA) / EXPECTED_PRESSURE_PA).abs() < RELATIVE_TOLERANCE
        );
    }
}

#[test]
fn default_fill_uses_1024_markers_at_half_molar() {
    const CONCENTRATION_TOLERANCE: f64 = 1e-12;
    let simulation = ConnectedChambers::with_initial_concentration(
        crate::gas::DEFAULT_INITIAL_CONCENTRATION_MOL_PER_L,
    );
    let regions = simulation.measurements();
    assert_eq!(
        simulation.particles.len(),
        crate::gas::DEFAULT_CONNECTED_MARKERS
    );
    assert!((simulation.initial_concentration() - 0.5).abs() < CONCENTRATION_TOLERANCE);
    assert!(regions.left.markers > 0);
    assert!(regions.right.markers > 0);
    assert!(regions.passage_markers > 0);
}

#[test]
fn minimum_fill_represents_256_markers() {
    let minimum = ConnectedChambers::with_initial_concentration(
        crate::gas::MIN_INITIAL_CONCENTRATION_MOL_PER_L,
    );
    assert_eq!(minimum.particles.len(), crate::gas::MIN_CONNECTED_MARKERS);
    assert!((minimum.initial_concentration() - 0.125).abs() < 1e-12);
}

#[test]
fn resetting_refills_both_cylinders_at_the_same_temperature() {
    const TEMPERATURE_TOLERANCE_K: f64 = 0.02;
    let mut simulation = ConnectedChambers::with_initial_concentration(0.5);
    simulation.set_heater_percent(100.0);
    for _ in 0..100 {
        simulation.update(FIXED_DT);
    }
    simulation.reset();
    let readings = simulation.measurements();
    assert_eq!(simulation.heater_percent(), 100.0);
    assert!((readings.left.temperature_kelvin.unwrap() - 300.0).abs() < TEMPERATURE_TOLERANCE_K);
    assert!((readings.right.temperature_kelvin.unwrap() - 300.0).abs() < TEMPERATURE_TOLERANCE_K);
    assert!(readings.left.markers > 0 && readings.right.markers > 0);
    assert_eq!(simulation.heat().total(), 0.0);
}

#[test]
fn moving_pistons_keep_every_sphere_below_their_faces() {
    const TEST_STEPS: usize = 1_000;
    let mut simulation = insulated(256);
    simulation.set_piston_mode(MotionMode::Controlled);
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
    pistons.set_mode(MotionMode::Controlled);
    for _ in 0..200 {
        pistons.advance(FIXED_DT);
    }
    let state = pistons.left();
    assert!(state.velocity < 0.0);
    let mut particle = Particle {
        position: vec3(-geometry.cylinder_offset, state.face_y, 0.0),
        velocity: vec3(0.0, TEST_PARTICLE_SPEED, 0.0),
    };
    let before = physics::particle_kinetic_energy(particle.velocity);
    let work = ConnectedChambers::bounce_piston(
        &mut particle,
        geometry,
        -geometry.cylinder_offset,
        PARTICLE_RADIUS,
        state,
        PistonSide::Hot,
        &mut pistons,
    );
    let after = physics::particle_kinetic_energy(particle.velocity);
    assert!((after - before + work).abs() < ENERGY_TOLERANCE);
    assert!(work < 0.0);
    assert!(particle.position.y < state.face_y - PARTICLE_RADIUS);
}

#[test]
fn moving_insulated_pistons_obey_first_law() {
    const TEST_STEPS: usize = 1_000;
    const RELATIVE_TOLERANCE: f32 = 5e-3;
    let mut simulation = insulated(256);
    simulation.set_piston_mode(MotionMode::Controlled);
    simulation.thermal.set_accommodation(0.0);
    let reference = simulation.kinetic_energy();
    for _ in 0..TEST_STEPS {
        simulation.update(FIXED_DT);
    }
    assert_eq!(simulation.heat().total(), 0.0);
    assert!(simulation.piston_work_by_gas().abs() > 0.0);
    assert!(simulation.energy_balance_error().abs() < RELATIVE_TOLERANCE * reference);
}

#[test]
fn heating_and_piston_motion_track_heat_and_work_separately() {
    const TEST_STEPS: usize = 700;
    const RELATIVE_TOLERANCE: f32 = 5e-3;
    let mut simulation = ConnectedChambers::new(256);
    simulation.thermal.set_accommodation(1.0);
    simulation.set_heater_percent(100.0);
    simulation.set_piston_mode(MotionMode::Controlled);
    simulation.thermal.set_accommodation(1.0);
    let reference = simulation.kinetic_energy();
    for _ in 0..TEST_STEPS {
        simulation.update(FIXED_DT);
    }
    assert!(simulation.heat().hot != 0.0);
    assert!(simulation.piston_work_by_gas().abs() > 0.0);
    assert!(simulation.energy_balance_error().abs() < RELATIVE_TOLERANCE * reference);
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

/// A radial clamp at the exact wall can round back *outside* after the
/// cylinder offset is added in f32, so test a full set of wall directions.
#[test]
fn projected_spheres_remain_inside_cylinders_at_all_angles() {
    let geometry = ChamberGeometry::default();
    let center_x = -geometry.cylinder_offset;
    let face_y = geometry.cylinder_height / 2.0;
    for angle in 0..360 {
        let radians = (angle as f32).to_radians();
        let position = vec3(
            center_x + geometry.cylinder_radius * 1.5 * radians.cos(),
            0.0,
            geometry.cylinder_radius * 1.5 * radians.sin(),
        );
        let projected = ConnectedChambers::project_to_cylinder(
            geometry,
            position,
            center_x,
            PARTICLE_RADIUS,
            face_y,
        );
        assert!(
            ConnectedChambers::within_cylinder(
                geometry,
                projected,
                center_x,
                PARTICLE_RADIUS,
                face_y,
            ),
            "projection at angle {angle} escaped: {projected:?}",
        );
    }
}

#[test]
fn free_flywheel_tracks_gas_and_mechanical_energy_separately() {
    const TEST_STEPS: usize = 400;
    const RELATIVE_TOLERANCE: f32 = 0.015;
    let mut simulation = insulated(256);
    assert_eq!(simulation.power_pistons().mode(), MotionMode::Free);
    simulation.set_pistons_running(true);
    let gas_reference = simulation.kinetic_energy();
    for _ in 0..TEST_STEPS {
        simulation.update(FIXED_DT);
    }
    assert!(simulation.energy_balance_error().abs() < RELATIVE_TOLERANCE * gas_reference);
    assert!(simulation.mechanical_balance_error().abs() < RELATIVE_TOLERANCE * gas_reference);
}

#[test]
fn reset_preserves_free_or_controlled_piston_mode() {
    let mut simulation = ConnectedChambers::new(256);
    simulation.set_piston_mode(MotionMode::Controlled);
    simulation.reset();
    assert_eq!(simulation.power_pistons().mode(), MotionMode::Controlled);
    simulation.set_piston_mode(MotionMode::Free);
    simulation.reset();
    assert_eq!(simulation.power_pistons().mode(), MotionMode::Free);
    assert_eq!(simulation.power_pistons().starter_work(), 0.0);
}

#[test]
fn round_tube_has_no_rectangular_corner_voids() {
    let geometry = DEFAULT_GEOMETRY;
    let usable_radius = geometry.connector_radius - PARTICLE_RADIUS;
    let center = vec3(0.0, geometry.connector_center_y, 0.0);
    assert!(ConnectedChambers::within_connector(
        geometry,
        center,
        PARTICLE_RADIUS
    ));
    let corner = center + vec3(0.0, usable_radius * 0.8, usable_radius * 0.8);
    assert!(!ConnectedChambers::within_connector(
        geometry,
        corner,
        PARTICLE_RADIUS
    ));
}

#[test]
fn circular_passage_projection_stays_inside_for_all_wall_angles() {
    let geometry = DEFAULT_GEOMETRY;
    let usable_radius = geometry.connector_radius - PARTICLE_RADIUS;
    for angle in 0..360 {
        let theta = (angle as f32).to_radians();
        let outside = vec3(
            0.0,
            geometry.connector_center_y + (usable_radius + 0.3) * theta.cos(),
            (usable_radius + 0.3) * theta.sin(),
        );
        let inside = ConnectedChambers::project_to_connector(geometry, outside, PARTICLE_RADIUS);
        assert!(
            ConnectedChambers::within_connector(geometry, inside, PARTICLE_RADIUS),
            "round tube projection failed at {angle} degrees: {inside:?}"
        );
    }
}

#[test]
fn tube_volume_uses_circular_cross_section() {
    let geometry = DEFAULT_GEOMETRY;
    let gap = 2.0 * (geometry.cylinder_offset - geometry.cylinder_radius) as f64;
    let expected = gap
        * std::f64::consts::PI
        * (geometry.connector_radius as f64).powi(2)
        * METERS_PER_UNIT.powi(3);
    assert!((geometry.connector_volume_m3() - expected).abs() < 1e-15);
}
