//! Orbit, pan, and zoom controls for the 3D chamber.

use std::f32::consts::FRAC_PI_2;

use macroquad::prelude::{
    Camera3D,
    KeyCode,
    MouseButton,
    Vec2,
    Vec3,
    is_key_down,
    is_key_pressed,
    is_mouse_button_down,
    mouse_position,
    mouse_wheel,
    screen_height,
    vec2,
    vec3, //
};

const INITIAL_POSITION: Vec3 = Vec3::new(6.0, 4.0, 6.0);
const ORBIT_CENTER: Vec3 = Vec3::ZERO;

const ORBIT_SENSITIVITY: f32 = 0.005;
const POLE_MARGIN: f32 = 0.01;
const MAX_PITCH: f32 = FRAC_PI_2 - POLE_MARGIN;

const ZOOM_SENSITIVITY: f32 = 0.005;
const MIN_DISTANCE: f32 = 4.0;
const MAX_DISTANCE: f32 = 25.0;

/// Camera that orbits the chamber while keeping its pan offset in view space.
///
/// The orbit center stays fixed at the chamber. Panning moves the chamber in
/// the viewport without changing the point around which the camera rotates.
pub struct OrbitCamera {
    distance: f32,
    yaw: f32,
    pitch: f32,
    pan_offset: Vec2,
    previous_orbit_mouse: Option<Vec2>,
    previous_pan_mouse: Option<Vec2>,
}

impl OrbitCamera {
    /// Creates a camera with the initial chamber-centered view.
    pub fn new() -> Self {
        let offset = INITIAL_POSITION - ORBIT_CENTER;
        let distance = offset.length();

        Self {
            distance,
            yaw: offset.x.atan2(offset.z),
            pitch: (offset.y / distance).asin(),
            pan_offset: Vec2::ZERO,
            previous_orbit_mouse: None,
            previous_pan_mouse: None,
        }
    }

    /// Restores the original orbit, framing, and zoom level.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Applies keyboard and, when enabled, pointer input for one frame.
    pub fn update(&mut self, pointer_enabled: bool) {
        if is_key_pressed(KeyCode::R) {
            self.reset();
            return;
        }

        // Do not let a slider gesture become a camera drag.
        if !pointer_enabled {
            self.previous_orbit_mouse = None;
            self.previous_pan_mouse = None;
            return;
        }

        let left_down = is_mouse_button_down(MouseButton::Left);
        let right_down = is_mouse_button_down(MouseButton::Right);
        let shift_down = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);

        let pan_active = right_down || (left_down && shift_down);
        let orbit_active = left_down && !pan_active;

        if pan_active {
            self.previous_orbit_mouse = None;
            self.update_pan();
        } else if orbit_active {
            self.previous_pan_mouse = None;
            self.update_orbit();
        } else {
            self.previous_orbit_mouse = None;
            self.previous_pan_mouse = None;
        }

        // Some trackpads report scroll events during drag gestures.
        if !left_down && !right_down {
            self.update_zoom();
        }
    }

    fn update_orbit(&mut self) {
        let (x, y) = mouse_position();
        let current_mouse = vec2(x, y);

        if let Some(previous_mouse) = self.previous_orbit_mouse {
            let delta = current_mouse - previous_mouse;

            self.yaw -= delta.x * ORBIT_SENSITIVITY;
            self.pitch = (self.pitch - delta.y * ORBIT_SENSITIVITY).clamp(-MAX_PITCH, MAX_PITCH);
        }

        self.previous_orbit_mouse = Some(current_mouse);
    }

    fn update_pan(&mut self) {
        let (x, y) = mouse_position();
        let current_mouse = vec2(x, y);

        if let Some(previous_mouse) = self.previous_pan_mouse {
            let delta = current_mouse - previous_mouse;
            let camera = self.camera();

            // Store the translation in camera-plane coordinates rather than
            // changing the orbit pivot. The basis will rotate with the camera.
            let world_units_per_pixel =
                2.0 * self.distance * (camera.fovy / 2.0).tan() / screen_height();

            self.pan_offset += vec2(-delta.x, delta.y) * world_units_per_pixel;
        }

        self.previous_pan_mouse = Some(current_mouse);
    }

    fn update_zoom(&mut self) {
        let (_, scroll_y) = mouse_wheel();

        if scroll_y != 0.0 {
            let zoom_factor = (-scroll_y * ZOOM_SENSITIVITY).exp();

            self.distance = (self.distance * zoom_factor).clamp(MIN_DISTANCE, MAX_DISTANCE);
        }
    }

    /// Returns the chamber-centered offset before applying the pan.
    fn orbit_offset(&self) -> Vec3 {
        let cos_pitch = self.pitch.cos();

        vec3(
            self.distance * cos_pitch * self.yaw.sin(),
            self.distance * self.pitch.sin(),
            self.distance * cos_pitch * self.yaw.cos(),
        )
    }

    /// Builds the Macroquad camera without moving the orbit center.
    pub fn camera(&self) -> Camera3D {
        let orbit_offset = self.orbit_offset();
        let forward = -orbit_offset.normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward).normalize();

        // Express the pan in the current viewing plane. Both camera
        // endpoints shift together, preserving orientation and screen-space
        // framing even when yaw or pitch changes.
        let pan = right * self.pan_offset.x + up * self.pan_offset.y;

        Camera3D {
            position: ORBIT_CENTER + orbit_offset + pan,
            target: ORBIT_CENTER + pan,
            up: Vec3::Y,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
