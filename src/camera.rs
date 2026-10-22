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

const INITIAL_POSITION: Vec3 = Vec3::new(8.4, 5.6, 8.4);
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
    view_scale: f32,
    yaw: f32,
    pitch: f32,
    pan_offset: Vec2,
    previous_orbit_mouse: Option<Vec2>,
    previous_pan_mouse: Option<Vec2>,
}

impl OrbitCamera {
    /// Frames a larger or smaller chamber and preserves that view on reset.
    pub fn with_scale(view_scale: f32) -> Self {
        assert!(view_scale.is_finite() && view_scale > 0.0);
        let offset = INITIAL_POSITION - ORBIT_CENTER;
        let distance = offset.length();

        Self {
            distance: distance * view_scale,
            view_scale,
            yaw: offset.x.atan2(offset.z),
            pitch: (offset.y / distance).asin(),
            pan_offset: Vec2::ZERO,
            previous_orbit_mouse: None,
            previous_pan_mouse: None,
        }
    }

    /// Restores the original orbit, framing, and zoom level.
    pub fn reset(&mut self) {
        *self = Self::with_scale(self.view_scale);
    }

    /// Applies keyboard and, when enabled, pointer input for one frame.
    pub fn update(&mut self, pointer_enabled: bool) {
        if is_key_pressed(KeyCode::R) {
            self.reset();
            return;
        }

        // The mouse is over a control or chart, or a control drag is in
        // progress; drop any drag so the camera does not jump afterward.
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

        // Zoom only when no button is held: some trackpads report scroll
        // events during drag gestures.
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

            self.distance = (self.distance * zoom_factor).clamp(
                MIN_DISTANCE * self.view_scale,
                MAX_DISTANCE * self.view_scale,
            );
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
#[path = "../tests/unit/camera.rs"]
mod tests;
