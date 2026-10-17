//! Unit tests for `src/camera.rs`.

use super::*;

impl OrbitCamera {
    fn new() -> Self {
        Self::with_scale(1.0)
    }
}

const TEST_PAN: Vec2 = Vec2::new(1.25, -0.75);
const TEST_YAW_CHANGE: f32 = 0.6;
const TEST_PITCH_CHANGE: f32 = 0.3;
const POSITION_TOLERANCE: f32 = 1e-4;

/// Coordinates of the chamber center in the camera's viewing frame.
fn chamber_view_position(camera: &Camera3D) -> Vec3 {
    let forward = (camera.target - camera.position).normalize();
    let right = forward.cross(camera.up).normalize();
    let up = right.cross(forward).normalize();
    let to_chamber = ORBIT_CENTER - camera.position;

    vec3(
        to_chamber.dot(right),
        to_chamber.dot(up),
        to_chamber.dot(forward),
    )
}

#[test]
fn orbit_preserves_panned_chamber_framing() {
    let mut camera = OrbitCamera::new();
    camera.pan_offset = TEST_PAN;

    let before = chamber_view_position(&camera.camera());

    camera.yaw += TEST_YAW_CHANGE;
    camera.pitch += TEST_PITCH_CHANGE;

    let after = chamber_view_position(&camera.camera());

    assert!((after - before).length() < POSITION_TOLERANCE);
    assert!((after.x + TEST_PAN.x).abs() < POSITION_TOLERANCE);
    assert!((after.y + TEST_PAN.y).abs() < POSITION_TOLERANCE);
    assert!((after.z - camera.distance).abs() < POSITION_TOLERANCE);
}

#[test]
fn reset_clears_pan_and_restores_initial_view() {
    let mut camera = OrbitCamera::new();
    camera.pan_offset = TEST_PAN;
    camera.yaw += TEST_YAW_CHANGE;

    camera.reset();

    let original = OrbitCamera::new().camera();
    let actual = camera.camera();

    assert!((actual.position - original.position).length() < POSITION_TOLERANCE);
    assert!((actual.target - original.target).length() < POSITION_TOLERANCE);
    assert_eq!(camera.pan_offset, Vec2::ZERO);
}

#[test]
fn scaled_camera_reset_keeps_larger_chamber_framed() {
    const TEST_SCALE: f32 = 1.8;
    let mut camera = OrbitCamera::with_scale(TEST_SCALE);
    let initial_distance = camera.distance;
    camera.distance /= 2.0;
    camera.reset();
    assert!((camera.distance - initial_distance).abs() < POSITION_TOLERANCE);
}

#[test]
fn camera_starts_farther_from_the_chambers() {
    let camera = OrbitCamera::new();
    let previous_distance = Vec3::new(6.0, 4.0, 6.0).length();
    assert!(camera.distance > previous_distance * 1.3);
}
