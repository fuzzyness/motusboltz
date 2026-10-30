//! Crankshaft, flywheel and Start tests for `src/sim/pistons.rs`.

use super::*;
use std::f32::consts::TAU;

// The full-volume reference exists only to validate moving piston volume.
impl ChamberGeometry {
    fn cylinder_volume_m3(self) -> f64 {
        std::f64::consts::PI
            * (self.cylinder_radius as f64 * METERS_PER_UNIT).powi(2)
            * (self.cylinder_height as f64 * METERS_PER_UNIT)
    }

    fn total_volume_m3(self) -> f64 {
        2.0 * self.cylinder_volume_m3() + self.passage_volume_m3()
    }
}

const EPSILON: f32 = 1e-4;
const PARTICLE_RADIUS: f32 = 0.13;
const TEST_DT: f32 = 1.0 / 120.0;

#[test]
fn initial_pistons_are_a_quarter_cycle_out_of_phase() {
    let geometry = ChamberGeometry::default();
    let pistons = PowerPistons::new(geometry, PARTICLE_RADIUS);
    assert!((pistons.hot().face_y - geometry.cylinder_height / 2.0).abs() < EPSILON);
    assert!((pistons.cold().face_y - pistons.position_at(FRAC_PI_2)).abs() < EPSILON);
    assert!(pistons.stroke() <= (STROKE_METERS / METERS_PER_UNIT) as f32);
    assert!(
        geometry.passage_top() + PARTICLE_RADIUS
            < geometry.cylinder_height / 2.0 - pistons.stroke()
    );
}

#[test]
fn a_full_revolution_restores_piston_positions() {
    let geometry = ChamberGeometry::default();
    let mut pistons = PowerPistons::new(geometry, PARTICLE_RADIUS);
    pistons.kick(KickDirection::Positive);
    let before_hot = pistons.hot().face_y;
    let before_cold = pistons.cold().face_y;
    pistons.angle = TAU;
    pistons.update_faces(TEST_DT);
    assert!((pistons.hot().face_y - before_hot).abs() < EPSILON);
    assert!((pistons.cold().face_y - before_cold).abs() < EPSILON);
}

#[test]
fn pausing_stops_motion_without_teleporting_pistons() {
    let geometry = ChamberGeometry::default();
    let mut pistons = PowerPistons::new(geometry, PARTICLE_RADIUS);
    pistons.kick(KickDirection::Positive);
    pistons.advance(TEST_DT);
    let before = (pistons.hot().face_y, pistons.cold().face_y);
    pistons.set_running(false);
    pistons.advance(TEST_DT);
    assert_eq!((pistons.hot().face_y, pistons.cold().face_y), before);
    assert_eq!(pistons.hot().velocity, 0.0);
    assert_eq!(pistons.cold().velocity, 0.0);
}

#[test]
fn kicked_motion_changes_total_working_volume() {
    let geometry = ChamberGeometry::default();
    let mut pistons = PowerPistons::new(geometry, PARTICLE_RADIUS);
    pistons.kick(KickDirection::Positive);
    let start = pistons.volume_m3(geometry);
    for _ in 0..400 {
        pistons.advance(TEST_DT);
    }
    assert!((pistons.volume_m3(geometry) - start).abs() > 1e-9);
    assert!(pistons.volume_m3(geometry) < geometry.total_volume_m3());
}

#[test]
fn free_flywheel_starts_at_rest_without_a_starter() {
    let pistons = PowerPistons::new(ChamberGeometry::default(), PARTICLE_RADIUS);
    assert_eq!(pistons.angular_velocity(), 0.0);
    assert_eq!(pistons.starter_work(), 0.0);
    assert_eq!(pistons.kinetic_energy(), 0.0);
}

#[test]
fn free_collision_conserves_particle_plus_rotor_energy() {
    let mut pistons = PowerPistons::new(ChamberGeometry::default(), PARTICLE_RADIUS);
    pistons.angle = FRAC_PI_2;
    let speed = 1.5_f32;
    let gas_before = 0.5 * speed.powi(2);
    let reflected = pistons.collide(PistonSide::Hot, speed, 1.0);
    let gas_after = 0.5 * reflected.powi(2);
    let rotor_work = pistons.kinetic_energy();
    assert!(rotor_work > 0.0);
    assert!(pistons.angular_velocity() < 0.0);
    assert!(((gas_after - gas_before) as f64 + rotor_work).abs() < EPSILON as f64);
}

#[test]
fn cold_piston_impulse_affects_the_same_rotor() {
    let mut pistons = PowerPistons::new(ChamberGeometry::default(), PARTICLE_RADIUS);
    // The hot-side crank is at dead center, but the cold-side crank is not.
    pistons.collide(PistonSide::Cold, 1.0, 1.0);
    assert!(pistons.kinetic_energy() > 0.0);
    assert!(pistons.angular_velocity() < 0.0);
}

#[test]
fn full_stroke_keeps_the_opening_uncovered() {
    let geometry = ChamberGeometry::default();
    let pistons = PowerPistons::new(geometry, PARTICLE_RADIUS);
    const EXPECTED_SAFE_STROKE: f32 = 4.8;
    assert!((pistons.stroke() - EXPECTED_SAFE_STROKE).abs() < EPSILON);
    assert!(
        geometry.cylinder_height / 2.0 - pistons.stroke() - PARTICLE_RADIUS
            > geometry.passage_top()
    );
}

#[test]
fn optional_kick_is_booked_as_external_energy() {
    let mut pistons = PowerPistons::new(ChamberGeometry::default(), PARTICLE_RADIUS);
    pistons.kick(KickDirection::Positive);
    assert!(pistons.angular_velocity() > 0.0);
    assert!((pistons.starter_work() - pistons.kinetic_energy()).abs() < EPSILON as f64);
}

#[test]
fn stationary_crank_stays_still_without_forces() {
    let mut pistons = PowerPistons::new(ChamberGeometry::default(), PARTICLE_RADIUS);
    pistons.set_reference_pressure(0.0);
    for _ in 0..120 {
        pistons.advance(TEST_DT);
    }
    assert_eq!(pistons.angle(), 0.0);
    assert_eq!(pistons.kinetic_energy(), 0.0);
}

#[test]
fn finite_connecting_rod_reaches_the_piston_at_every_angle() {
    let mut pistons = PowerPistons::new(ChamberGeometry::default(), PARTICLE_RADIUS);
    pistons.kick(KickDirection::Positive);
    for _ in 0..360 {
        pistons.advance(TEST_DT);
        for side in [PistonSide::Hot, PistonSide::Cold] {
            let phase = PowerPistons::phase(side);
            let theta = pistons.angle + phase;
            let crank = pistons.stroke / 2.0;
            let difference_y = pistons.shaft_y + crank * theta.cos() - pistons.position_at(theta);
            let difference_z = crank * theta.sin();
            let length = (difference_y.powi(2) + difference_z.powi(2)).sqrt();
            assert!((length - pistons.rod_length).abs() < EPSILON);
        }
    }
}

#[test]
fn diagnostic_kick_supports_both_directions() {
    for (direction, sign) in [
        (KickDirection::Positive, 1.0),
        (KickDirection::Negative, -1.0),
    ] {
        let mut pistons = PowerPistons::new(ChamberGeometry::default(), PARTICLE_RADIUS);
        pistons.kick(direction);
        assert_eq!(pistons.angular_velocity().signum(), sign);
        assert!((pistons.starter_work() - pistons.kinetic_energy()).abs() < EPSILON as f64);
    }
}

#[test]
fn gas_impulse_sets_the_expected_rotation_direction() {
    let mut pistons = PowerPistons::new(ChamberGeometry::default(), PARTICLE_RADIUS);
    pistons.angle = FRAC_PI_2;
    pistons.collide(PistonSide::Hot, 1.5, 1.0);
    assert!(pistons.angular_velocity() < 0.0);
}

#[test]
fn back_pressure_work_matches_displaced_volume_independently() {
    const TEST_PRESSURE: f32 = 2.0;
    const TEST_STEPS: usize = 2_000;
    const WORK_TOLERANCE: f64 = 1e-7;
    let mut rotor = PowerPistons::new(ChamberGeometry::default(), PARTICLE_RADIUS);
    rotor.set_reference_pressure(TEST_PRESSURE);
    rotor.kick(KickDirection::Negative);
    let volume = |rotor: &PowerPistons| {
        (rotor.area * (rotor.position_at(rotor.angle) + rotor.position_at(rotor.angle + FRAC_PI_2)))
            as f64
    };
    let initial_volume = volume(&rotor);
    for _ in 0..TEST_STEPS {
        rotor.advance(TEST_DT);
    }
    let expected = -TEST_PRESSURE as f64 * (volume(&rotor) - initial_volume);
    assert!((rotor.back_pressure_work() - expected).abs() < WORK_TOLERANCE);
}

#[test]
fn drag_loss_accounts_for_lost_flywheel_energy() {
    let mut pistons = PowerPistons::new(ChamberGeometry::default(), PARTICLE_RADIUS);
    pistons.set_reference_pressure(0.0);
    pistons.kick(KickDirection::Negative);
    for _ in 0..120 {
        pistons.advance(TEST_DT);
    }
    assert!(pistons.drag_loss() > 0.0);
    assert!(pistons.kinetic_energy() < pistons.starter_work());
    assert!(
        (pistons.kinetic_energy() + pistons.drag_loss() - pistons.starter_work()).abs()
            < EPSILON as f64
    );
}

#[test]
fn start_brings_a_resting_shaft_to_the_starting_speed() {
    let mut pistons = PowerPistons::new(ChamberGeometry::default(), PARTICLE_RADIUS);
    assert!(pistons.start());
    assert!(pistons.angular_velocity() < 0.0, "heat-engine direction");
    assert!((pistons.kinetic_energy() - STARTER_ENERGY).abs() < EPSILON as f64);
    assert!((pistons.starter_work() - pistons.kinetic_energy()).abs() < EPSILON as f64);
}

#[test]
fn repeated_start_never_exceeds_the_starting_speed() {
    let mut pistons = PowerPistons::new(ChamberGeometry::default(), PARTICLE_RADIUS);
    pistons.start();
    let speed = pistons.angular_velocity();
    let work = pistons.starter_work();
    for _ in 0..10 {
        assert!(!pistons.start());
    }
    assert_eq!(pistons.angular_velocity(), speed);
    assert_eq!(pistons.starter_work(), work);
    // A shaft kicked faster than the starting speed is left alone too.
    pistons.kick(KickDirection::Negative);
    let faster = pistons.angular_velocity();
    let kicked_work = pistons.starter_work();
    assert!(!pistons.start());
    assert_eq!(pistons.angular_velocity(), faster);
    assert_eq!(pistons.starter_work(), kicked_work);
}

#[test]
fn start_tops_up_a_slow_or_backward_shaft() {
    for initial in [-0.5, 0.0, 0.7] {
        let mut pistons = PowerPistons::new(ChamberGeometry::default(), PARTICLE_RADIUS);
        pistons.angular_velocity = initial * pistons.start_speed;
        let before = pistons.kinetic_energy();
        assert!(pistons.start());
        assert!((pistons.angular_velocity + pistons.start_speed).abs() < 1e-12);
        // Reversing a backward shaft can cost energy as well as add it.
        let booked = pistons.starter_work();
        assert!((booked - (pistons.kinetic_energy() - before)).abs() < EPSILON as f64);
    }
}

#[test]
fn starting_energy_sets_the_starting_speed() {
    let mut pistons = PowerPistons::new(ChamberGeometry::default(), PARTICLE_RADIUS);
    pistons.set_starting_energy(2.0 * STARTER_ENERGY);
    pistons.start();
    assert!((pistons.kinetic_energy() - 2.0 * STARTER_ENERGY).abs() < EPSILON as f64);
}

/// A coasting shaft loses its energy to the bearing and the load only, and
/// the load's share is booked separately: it is most of the loss, because
/// the load's torque grows with the square of the speed.
#[test]
fn coasting_shaft_splits_its_loss_between_bearing_and_load() {
    const STEPS: usize = 2_400;
    const RELATIVE_TOLERANCE: f64 = 1e-9;
    let mut pistons = PowerPistons::new(ChamberGeometry::default(), PARTICLE_RADIUS);
    assert!(pistons.start());
    let start = pistons.kinetic_energy();
    for _ in 0..STEPS {
        pistons.advance(TEST_DT);
    }
    let lost = start - pistons.kinetic_energy() + pistons.back_pressure_work();
    assert!(pistons.load_work() > 0.0);
    assert!(pistons.load_work() < pistons.drag_loss());
    assert!(pistons.load_work() > 0.5 * pistons.drag_loss());
    assert!((pistons.drag_loss() - lost).abs() < RELATIVE_TOLERANCE * start + 1e-6);
}
