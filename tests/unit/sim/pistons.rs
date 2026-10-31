//! Crankshaft, flywheel, friction and Start tests for `src/sim/pistons.rs`.

use std::f32::consts::TAU;

use super::*;
use crate::sim::engine::PARTICLE_RADIUS;

/// Largest difference treated as equal.
const EPSILON: f32 = 1e-4;
/// One fixed step.
const DT: f32 = 1.0 / 120.0;

impl PowerPistons {
    /// Holds the shaft until Start or the kick, whatever the gas does, for
    /// tests that need the pistons still.
    pub(crate) fn pin(&mut self) {
        self.hold();
        self.held_time = f64::NEG_INFINITY;
    }

    /// Whether static friction holds the shaft.
    pub(crate) fn held(&self) -> bool {
        self.held
    }
}

/// Pistons in the default chamber.
fn pistons() -> PowerPistons {
    PowerPistons::new(ChamberGeometry::default(), PARTICLE_RADIUS)
}

/// Pistons in the default chamber, free of static friction.
fn free_pistons() -> PowerPistons {
    let mut pistons = pistons();
    pistons.let_go();
    pistons
}

#[test]
fn pistons_start_a_quarter_turn_apart_above_the_opening() {
    let geometry = ChamberGeometry::default();
    let pistons = pistons();
    assert!((pistons.hot().face_y - geometry.cylinder_height / 2.0).abs() < EPSILON);
    assert!((pistons.cold().face_y - pistons.position_at(FRAC_PI_2)).abs() < EPSILON);
    // The stroke the geometry allows, which keeps the opening uncovered.
    const SAFE_STROKE: f32 = 4.8;
    assert!((pistons.stroke() - SAFE_STROKE).abs() < EPSILON);
    assert!(
        geometry.passage_top() + PARTICLE_RADIUS
            < geometry.cylinder_height / 2.0 - pistons.stroke()
    );
}

#[test]
fn a_full_revolution_restores_the_pistons() {
    let mut pistons = pistons();
    let before = (pistons.hot().face_y, pistons.cold().face_y);
    pistons.angle = TAU;
    pistons.update_faces(DT);
    assert!((pistons.hot().face_y - before.0).abs() < EPSILON);
    assert!((pistons.cold().face_y - before.1).abs() < EPSILON);
}

#[test]
fn rods_reach_the_pistons_at_every_angle() {
    let mut pistons = pistons();
    pistons.kick();
    for _ in 0..360 {
        pistons.advance(DT);
        for side in [PistonSide::Hot, PistonSide::Cold] {
            let theta = pistons.angle + side.phase();
            let crank = pistons.stroke / 2.0;
            let rise = pistons.shaft_y + crank * theta.cos() - pistons.position_at(theta);
            let length = rise.hypot(crank * theta.sin());
            assert!((length - pistons.rod_length).abs() < EPSILON);
        }
    }
}

#[test]
fn holding_stops_the_pistons_where_they_are() {
    let mut pistons = pistons();
    pistons.kick();
    pistons.advance(DT);
    let before = (pistons.hot().face_y, pistons.cold().face_y);
    pistons.hold();
    pistons.advance(DT);
    assert_eq!((pistons.hot().face_y, pistons.cold().face_y), before);
    assert_eq!(
        (pistons.hot().velocity, pistons.cold().velocity),
        (0.0, 0.0)
    );
    assert_eq!(pistons.start_speed_fraction(), 0.0);
}

#[test]
fn a_resting_shaft_has_no_energy_and_stays_put_without_forces() {
    let mut pistons = pistons();
    assert_eq!(pistons.kinetic_energy(), 0.0);
    assert_eq!(pistons.starter_work(), 0.0);
    for _ in 0..120 {
        pistons.advance(DT);
    }
    assert_eq!(pistons.angle(), 0.0);
    assert_eq!(pistons.kinetic_energy(), 0.0);
}

#[test]
fn gas_impacts_turn_a_free_shaft_and_conserve_energy() {
    // At a quarter turn the hot crank has the longest lever; at angle zero
    // only the cold one does.
    for (angle, side) in [(FRAC_PI_2, PistonSide::Hot), (0.0, PistonSide::Cold)] {
        let mut pistons = free_pistons();
        pistons.angle = angle;
        let speed = 1.5_f32;
        let reflected = pistons.collide(side, speed, 1.0);
        let gas_change = 0.5 * (reflected.powi(2) - speed.powi(2));
        assert!(pistons.kinetic_energy() > 0.0);
        assert!(pistons.angular_velocity() < 0.0, "heat-engine direction");
        assert!((gas_change as f64 + pistons.kinetic_energy()).abs() < EPSILON as f64);
    }
}

#[test]
fn kicks_and_start_are_booked_as_starter_work() {
    let mut kicked = pistons();
    kicked.kick();
    assert!(kicked.angular_velocity() < 0.0);
    assert!((kicked.starter_work() - kicked.kinetic_energy()).abs() < EPSILON as f64);

    let mut started = pistons();
    assert!(started.start());
    assert!((started.start_speed_fraction() - 1.0).abs() < 1e-6);
    assert!((started.kinetic_energy() - STARTER_ENERGY).abs() < EPSILON as f64);
    assert!((started.starter_work() - started.kinetic_energy()).abs() < EPSILON as f64);

    let mut doubled = pistons();
    doubled.set_starting_energy(2.0 * STARTER_ENERGY);
    doubled.start();
    assert!((doubled.kinetic_energy() - 2.0 * STARTER_ENERGY).abs() < EPSILON as f64);
}

#[test]
fn start_never_exceeds_the_starting_speed() {
    let mut pistons = pistons();
    pistons.start();
    let (speed, work) = (pistons.angular_velocity(), pistons.starter_work());
    assert!(!pistons.start());
    assert_eq!(
        (pistons.angular_velocity(), pistons.starter_work()),
        (speed, work)
    );
    // A shaft kicked faster than the starting speed is left alone too.
    pistons.kick();
    let (speed, work) = (pistons.angular_velocity(), pistons.starter_work());
    assert!(!pistons.start());
    assert_eq!(
        (pistons.angular_velocity(), pistons.starter_work()),
        (speed, work)
    );
}

#[test]
fn start_tops_up_a_slow_or_backward_shaft() {
    for initial in [-0.5, 0.0, 0.7] {
        let mut pistons = free_pistons();
        pistons.angular_velocity = -initial * pistons.start_speed;
        assert!((pistons.start_speed_fraction() - initial).abs() < 1e-6);
        let before = pistons.kinetic_energy();
        assert!(pistons.start());
        assert!((pistons.start_speed_fraction() - 1.0).abs() < 1e-12);
        // Reversing a backward shaft can cost energy as well as add it.
        let booked = pistons.starter_work();
        assert!((booked - (pistons.kinetic_energy() - before)).abs() < EPSILON as f64);
    }
}

#[test]
fn back_pressure_work_matches_the_displaced_volume() {
    const PRESSURE: f32 = 2.0;
    let mut pistons = pistons();
    pistons.set_reference_pressure(PRESSURE);
    pistons.kick();
    let before = pistons.swept_volume();
    for _ in 0..2_000 {
        pistons.advance(DT);
    }
    let expected = -PRESSURE as f64 * (pistons.swept_volume() - before);
    assert!((pistons.back_pressure_work() - expected).abs() < 1e-7);
}

/// A coasting shaft loses its energy to the bearing and the load only, and
/// the load takes most of it, because its torque grows with the square of
/// the speed.
#[test]
fn a_coasting_shaft_splits_its_loss_between_bearing_and_load() {
    let mut pistons = pistons();
    pistons.start();
    let start = pistons.kinetic_energy();
    for _ in 0..2_400 {
        pistons.advance(DT);
    }
    let lost = start - pistons.kinetic_energy();
    assert!((pistons.drag_loss() - lost).abs() < 1e-9 * start + 1e-6);
    assert!(pistons.load_work() > 0.5 * pistons.drag_loss());
    assert!(pistons.load_work() < pistons.drag_loss());
}

#[test]
fn moving_pistons_change_the_gas_volume() {
    let geometry = ChamberGeometry::default();
    let mut pistons = pistons();
    pistons.kick();
    let before = pistons.volume_m3(geometry);
    for _ in 0..400 {
        pistons.advance(DT);
    }
    assert!((pistons.volume_m3(geometry) - before).abs() > 1e-9);
}

/// A held shaft gathers the gas's push without moving, stays put while
/// static friction outweighs it, and lets go once the push is stronger.
#[test]
fn static_friction_holds_until_the_push_wins() {
    // Pushes over one window just short of, then just past, the friction.
    for (scale, released) in [(0.9, false), (1.1, true)] {
        let mut pistons = pistons();
        pistons.set_reference_pressure(1.0);
        pistons.angle = FRAC_PI_2;
        pistons.update_faces(DT);
        let limit = STATIC_FRICTION_SHARE * pistons.torque_scale();
        let lever = pistons.lever(PistonSide::Hot) as f64;
        // The impulse that, with the back pressure, pushes `scale` of the
        // friction the way the lever turns the shaft.
        let impulse =
            (lever.signum() * limit * scale - pistons.back_pressure_torque()) * HOLD_WINDOW / lever;
        assert!(impulse > 0.0);
        pistons.collide(PistonSide::Hot, (impulse / 2.0) as f32, 1.0);
        // One weighing, whether the window fills on the last step or the
        // one before.
        let steps = (HOLD_WINDOW / DT as f64).round() as usize + 1;
        for _ in 0..steps {
            pistons.advance(DT);
        }
        assert_eq!(pistons.held(), !released, "push {scale} of the friction");
    }
}

/// A turning shaft that reverses is caught and held at once, and the
/// little energy it had left is booked as friction.
#[test]
fn a_turnaround_is_held_by_static_friction() {
    let mut pistons = free_pistons();
    pistons.angular_velocity = -0.01;
    pistons.advance(DT);
    assert!(!pistons.held());
    pistons.angular_velocity = 0.002;
    let energy = pistons.kinetic_energy();
    pistons.advance(DT);
    assert!(pistons.held());
    assert_eq!(pistons.angular_velocity(), 0.0);
    assert!(pistons.drag_loss() >= energy);
}
