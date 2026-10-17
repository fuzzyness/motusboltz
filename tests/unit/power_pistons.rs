//! Controlled and free-crankshaft tests for `src/engine.rs`.

use super::*;

// The full-volume reference exists only to validate moving piston volume.
impl ChamberGeometry {
    fn cylinder_volume_m3(self) -> f64 {
        std::f64::consts::PI
            * (self.cylinder_radius as f64 * METERS_PER_UNIT).powi(2)
            * (self.cylinder_height as f64 * METERS_PER_UNIT)
    }

    fn total_volume_m3(self) -> f64 {
        2.0 * self.cylinder_volume_m3() + self.connector_volume_m3()
    }
}

const EPSILON: f32 = 1e-4;
const PARTICLE_RADIUS: f32 = 0.13;
const TEST_DT: f32 = 1.0 / 120.0;

#[test]
fn initial_pistons_are_a_quarter_cycle_out_of_phase() {
    let geometry = ChamberGeometry::default();
    let pistons = PowerPistons::new(geometry, PARTICLE_RADIUS);
    assert!((pistons.left().face_y - geometry.cylinder_height / 2.0).abs() < EPSILON);
    assert!((pistons.right().face_y - pistons.position_at(FRAC_PI_2)).abs() < EPSILON);
    assert!(pistons.stroke() <= (STROKE_METERS / METERS_PER_UNIT) as f32);
    assert!(
        geometry.connector_center_y + geometry.connector_radius + PARTICLE_RADIUS
            < geometry.cylinder_height / 2.0 - pistons.stroke()
    );
}

#[test]
fn a_full_revolution_restores_piston_positions() {
    let geometry = ChamberGeometry::default();
    let mut pistons = PowerPistons::new(geometry, PARTICLE_RADIUS);
    pistons.set_mode(MotionMode::Controlled);
    let before_left = pistons.left().face_y;
    let before_right = pistons.right().face_y;
    let steps = (TAU / (DRIVE_ANGULAR_SPEED * TEST_DT)).round() as usize;
    for _ in 0..steps {
        pistons.advance(TEST_DT);
    }
    let step_displacement_bound = pistons.stroke() * DRIVE_ANGULAR_SPEED * TEST_DT;
    assert!((pistons.left().face_y - before_left).abs() < step_displacement_bound);
    assert!((pistons.right().face_y - before_right).abs() < step_displacement_bound);
}

#[test]
fn pausing_stops_motion_without_teleporting_pistons() {
    let geometry = ChamberGeometry::default();
    let mut pistons = PowerPistons::new(geometry, PARTICLE_RADIUS);
    pistons.set_mode(MotionMode::Controlled);
    pistons.advance(TEST_DT);
    let before = (pistons.left().face_y, pistons.right().face_y);
    pistons.set_running(false);
    pistons.advance(TEST_DT);
    assert_eq!((pistons.left().face_y, pistons.right().face_y), before);
    assert_eq!(pistons.left().velocity, 0.0);
    assert_eq!(pistons.right().velocity, 0.0);
}

#[test]
fn prescribed_motion_changes_total_working_volume() {
    let geometry = ChamberGeometry::default();
    let mut pistons = PowerPistons::new(geometry, PARTICLE_RADIUS);
    pistons.set_mode(MotionMode::Controlled);
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
    assert_eq!(pistons.mode(), MotionMode::Free);
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
    assert!((gas_after - gas_before + rotor_work).abs() < EPSILON);
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
fn increased_stroke_preserves_open_passage() {
    let geometry = ChamberGeometry::default();
    let pistons = PowerPistons::new(geometry, PARTICLE_RADIUS);
    assert!((pistons.stroke() - 3.8).abs() < EPSILON);
    assert!(
        geometry.cylinder_height / 2.0 - pistons.stroke() - PARTICLE_RADIUS
            > geometry.connector_center_y + geometry.connector_radius
    );
}

#[test]
fn optional_kick_is_booked_as_external_energy() {
    let mut pistons = PowerPistons::new(ChamberGeometry::default(), PARTICLE_RADIUS);
    pistons.kick(KickDirection::Positive);
    assert!(pistons.angular_velocity() > 0.0);
    assert!((pistons.starter_work() - pistons.kinetic_energy()).abs() < EPSILON);
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
    pistons.set_mode(MotionMode::Controlled);
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
        assert!((pistons.starter_work() - pistons.kinetic_energy()).abs() < EPSILON);
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
fn mode_changes_reset_angular_speed() {
    let mut pistons = PowerPistons::new(ChamberGeometry::default(), PARTICLE_RADIUS);
    pistons.kick(KickDirection::Negative);
    pistons.set_mode(MotionMode::Controlled);
    pistons.set_mode(MotionMode::Free);
    assert_eq!(pistons.angular_velocity(), 0.0);
}
