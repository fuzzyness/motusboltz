//! Orbit, pan and zoom tests for `src/ui/camera.rs`.

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

const FRAME: f32 = 1.0 / 60.0;

fn keys(orbit: Vec2, pan: Vec2, zoom: f32) -> CameraKeys {
    CameraKeys { orbit, pan, zoom }
}

#[test]
fn orbit_keys_turn_the_camera_at_a_steady_rate() {
    let mut camera = OrbitCamera::new();
    let (yaw, pitch) = (camera.yaw, camera.pitch);
    camera.apply_keys(keys(Vec2::new(1.0, 1.0), Vec2::ZERO, 0.0), FRAME);
    assert!((camera.yaw - yaw - KEY_ORBIT_SPEED * FRAME).abs() < 1e-6);
    assert!((camera.pitch - pitch - KEY_ORBIT_SPEED * FRAME).abs() < 1e-6);
    // Holding W for a long time stops short of looking straight down.
    camera.apply_keys(keys(Vec2::new(0.0, 1.0), Vec2::ZERO, 0.0), 100.0);
    assert_eq!(camera.pitch, MAX_PITCH);
}

/// Shift+D moves the camera to its right, so the engine appears to move
/// left; Shift+W moves it up.
#[test]
fn pan_keys_move_the_camera_in_its_own_plane() {
    let mut camera = OrbitCamera::new();
    let before = camera.camera();
    camera.apply_keys(keys(Vec2::ZERO, Vec2::new(1.0, 1.0), 0.0), FRAME);
    let after = camera.camera();
    let forward = (before.target - before.position).normalize();
    let right = forward.cross(Vec3::Y).normalize();
    let up = right.cross(forward);
    let moved = after.position - before.position;
    assert!(moved.dot(right) > 0.0 && moved.dot(up) > 0.0);
    assert!(moved.dot(forward).abs() < POSITION_TOLERANCE);
    assert!(((after.target - before.target) - moved).length() < POSITION_TOLERANCE);
}

#[test]
fn zoom_keys_change_distance_within_limits() {
    let mut camera = OrbitCamera::new();
    let distance = camera.distance;
    camera.apply_keys(keys(Vec2::ZERO, Vec2::ZERO, 1.0), FRAME);
    assert!(camera.distance < distance);
    camera.apply_keys(keys(Vec2::ZERO, Vec2::ZERO, 1.0), 100.0);
    assert_eq!(camera.distance, MIN_DISTANCE * camera.view_scale);
    camera.apply_keys(keys(Vec2::ZERO, Vec2::ZERO, -1.0), 100.0);
    assert_eq!(camera.distance, MAX_DISTANCE * camera.view_scale);
}

#[test]
fn right_drag_pans_natively_but_not_in_the_browser() {
    assert_eq!(RIGHT_DRAG_PANS, !cfg!(target_arch = "wasm32"));
}

#[test]
fn keys_work_while_the_pointer_is_over_the_panel() {
    let mut camera = OrbitCamera::new();
    let yaw = camera.yaw;
    camera.update(
        keys(Vec2::new(1.0, 0.0), Vec2::ZERO, 0.0),
        Pointer::default(),
        FRAME,
        900.0,
    );
    assert!(camera.yaw > yaw);
}
