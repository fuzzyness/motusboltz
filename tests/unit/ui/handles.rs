//! Tests for `src/ui/handles.rs`: valve settings, the walls' ramp, and
//! following the pointer onto a part.

use std::f32::consts::FRAC_PI_2;

use macroquad::prelude::{
    Mat3,
    vec3, //
};

use super::*;

/// Size of the test screen.
const SIZE: Vec2 = Vec2::new(1_000.0, 800.0);
/// Largest difference treated as equal.
const TOLERANCE: f32 = 1e-3;

/// A camera at `eye` looking at the origin, as the app's sees the scene.
fn matrix(eye: Vec3) -> Mat4 {
    let projection = Mat4::perspective_rh_gl(1.0, SIZE.x / SIZE.y, 0.1, 100.0);
    projection * Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y)
}

#[test]
fn shut_valves_leave_room_temperature_and_open_ones_the_sliders_far_ends() {
    for side in [ReservoirSide::Hot, ReservoirSide::Cold] {
        let (shut, open) = valve_range(side);
        assert_eq!(shut, 300.0);
        assert_eq!(valve_kelvin(side, 0.0), shut);
        assert_eq!(valve_kelvin(side, 1.0), open);
        assert_eq!(valve_turn(side, shut), 0.0);
        assert!((valve_turn(side, open) - VALVE_TURNS * TAU).abs() < TOLERANCE);
        let middle = valve_kelvin(side, 0.5);
        assert!((valve_opening(side, middle) - 0.5).abs() < TOLERANCE);
        assert_eq!(valve_kelvin(side, 2.0), open, "no further than open");
    }
    assert_eq!(valve_range(ReservoirSide::Hot).1, MAX_HOT_KELVIN);
    assert_eq!(valve_range(ReservoirSide::Cold).1, MIN_COLD_KELVIN);
}

#[test]
fn walls_move_toward_their_settings_at_a_steady_pace_without_passing_them() {
    let second = 1.0;
    assert_eq!(
        ramped(300.0, 1_200.0, second),
        300.0 + RAMP_KELVIN_PER_SECOND
    );
    assert_eq!(ramped(300.0, 77.0, second), 300.0 - RAMP_KELVIN_PER_SECOND);
    assert_eq!(ramped(1_190.0, 1_200.0, second), 1_200.0);
    assert_eq!(ramped(500.0, 1_200.0, -1.0), 500.0, "time never runs back");
}

#[test]
fn turns_across_the_seam_read_the_short_way_round() {
    assert!((wrapped(0.1) - 0.1).abs() < TOLERANCE);
    assert!((wrapped(TAU - 0.1) + 0.1).abs() < TOLERANCE);
    assert!((wrapped(-TAU + 0.2) - 0.2).abs() < TOLERANCE);
    let mut grab = Grab::new(Handle::Crank);
    assert_eq!(
        grab.turned(Some(PI - 0.05)),
        0.0,
        "the first reading only starts"
    );
    assert!((grab.turned(Some(-PI + 0.05)) - 0.1).abs() < TOLERANCE);
    assert_eq!(grab.turned(None), 0.0, "a lost reading turns nothing");
}

/// The pointer over a point on screen follows back to that point on the
/// plane through it.
#[test]
fn the_pointer_is_followed_back_onto_a_parts_plane() {
    let seen = matrix(vec3(6.0, 4.0, 8.0));
    for target in [
        vec3(0.0, 0.0, 0.0),
        vec3(1.0, -2.0, 0.5),
        vec3(-2.0, 1.0, 1.5),
    ] {
        let pointer = project(seen, SIZE, target).unwrap();
        let ray = Ray::through(seen, SIZE, pointer).unwrap();
        for normal in [Vec3::X, Vec3::Y] {
            let point = ray.meet(target, normal).unwrap();
            assert!(point.distance(target) < 1e-2, "{point} for {target}");
        }
    }
    let ray = Ray::through(matrix(vec3(0.0, 0.0, 8.0)), SIZE, SIZE / 2.0).unwrap();
    assert_eq!(ray.meet(Vec3::ZERO, Vec3::X), None, "a plane seen edge on");
}

#[test]
fn a_press_grabs_the_nearest_handle_within_reach() {
    let seen = matrix(vec3(6.0, 4.0, 8.0));
    let (near, far) = (vec3(0.0, 0.0, 0.0), vec3(0.0, 3.0, 0.0));
    let handles = [
        (Handle::Crank, far),
        (Handle::Valve(ReservoirSide::Hot), near),
    ];
    let spot = project(seen, SIZE, near).unwrap();
    assert_eq!(
        pick(&handles, seen, SIZE, spot + Vec2::X * 5.0),
        Some(Handle::Valve(ReservoirSide::Hot))
    );
    assert_eq!(pick(&handles, seen, SIZE, spot + Vec2::X * 200.0), None);
}

/// A wheel's angle runs counterclockwise seen from above, the way its turn
/// is counted and drawn (a positive turn about +y).
#[test]
fn wheel_angles_run_counterclockwise_seen_from_above() {
    let center = vec3(1.0, 2.0, 3.0);
    assert!(wheel_angle(center, center + Vec3::X).abs() < TOLERANCE);
    let quarter = Mat3::from_rotation_y(FRAC_PI_2) * Vec3::X;
    assert!((wheel_angle(center, center + quarter) - FRAC_PI_2).abs() < TOLERANCE);
}
