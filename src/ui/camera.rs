//! Orbit, pan and zoom for the 3D scene, by mouse and by keyboard.

use std::f32::consts::FRAC_PI_2;

use macroquad::prelude::{
    Camera3D,
    MouseButton,
    Vec2,
    Vec3,
    is_mouse_button_down,
    mouse_position,
    vec2,
    vec3, //
};

use crate::ui::input::CameraKeys;

const INITIAL_POSITION: Vec3 = Vec3::new(8.4, 5.6, 8.4);
const ORBIT_CENTER: Vec3 = Vec3::ZERO;

const ORBIT_SENSITIVITY: f32 = 0.005;
const POLE_MARGIN: f32 = 0.01;
const MAX_PITCH: f32 = FRAC_PI_2 - POLE_MARGIN;

/// Zoom per pixel of wheel scroll, as a fraction of the distance.
const ZOOM_SENSITIVITY: f32 = 0.005;
const MIN_DISTANCE: f32 = 4.0;
const MAX_DISTANCE: f32 = 25.0;

/// Keyboard orbit speed, in radians per second.
const KEY_ORBIT_SPEED: f32 = 1.2;
/// Keyboard pan speed, as a fraction of the visible height per second.
const KEY_PAN_SPEED: f32 = 0.5;
/// Keyboard zoom rate: the distance changes by a factor of e every this
/// many reciprocal seconds.
const KEY_ZOOM_RATE: f32 = 0.9;

/// Right-drag pans natively. In the browser a right click also opens the
/// context menu, which swallows the button release, so the camera stayed
/// stuck panning; there Shift+drag and Shift+WASD/arrows pan instead.
const RIGHT_DRAG_PANS: bool = !cfg!(target_arch = "wasm32");

/// Pointer state for one frame, read by the caller.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pointer {
    /// False while the pointer is over the panel or a control, or a gesture
    /// that started there is still held.
    pub enabled: bool,
    pub shift_down: bool,
    /// Wheel scroll this frame, in pixels; positive zooms in.
    pub wheel_pixels: f32,
}

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

    /// Applies one frame of input: held camera keys over `dt` seconds, and
    /// mouse drags and the wheel when the pointer is enabled.
    pub fn update(&mut self, keys: CameraKeys, pointer: Pointer, dt: f32, view_height: f32) {
        self.apply_keys(keys, dt);

        // The mouse is over a control or the panel, or a gesture that
        // started there is held; drop any drag so the camera does not jump.
        if !pointer.enabled {
            self.previous_orbit_mouse = None;
            self.previous_pan_mouse = None;
            return;
        }

        let left_down = is_mouse_button_down(MouseButton::Left);
        let right_down = RIGHT_DRAG_PANS && is_mouse_button_down(MouseButton::Right);
        let pan_active = right_down || (left_down && pointer.shift_down);
        let orbit_active = left_down && !pan_active;
        let mouse = Vec2::from(mouse_position());

        if pan_active {
            self.previous_orbit_mouse = None;
            self.drag_pan(mouse, view_height);
        } else if orbit_active {
            self.previous_pan_mouse = None;
            self.drag_orbit(mouse);
        } else {
            self.previous_orbit_mouse = None;
            self.previous_pan_mouse = None;
        }

        // Zoom only when no button is held: some trackpads report scroll
        // events during drag gestures.
        if !left_down && !right_down {
            self.zoom_by(-pointer.wheel_pixels * ZOOM_SENSITIVITY);
        }
    }

    /// Orbits with W/A/S/D or the arrows, pans with Shift held, and zooms
    /// with - and =, at rates independent of the frame rate.
    fn apply_keys(&mut self, keys: CameraKeys, dt: f32) {
        self.yaw += keys.orbit.x * KEY_ORBIT_SPEED * dt;
        self.pitch =
            (self.pitch + keys.orbit.y * KEY_ORBIT_SPEED * dt).clamp(-MAX_PITCH, MAX_PITCH);
        self.pan_offset += keys.pan * self.visible_height() * KEY_PAN_SPEED * dt;
        self.zoom_by(-keys.zoom * KEY_ZOOM_RATE * dt);
    }

    fn drag_orbit(&mut self, mouse: Vec2) {
        if let Some(previous) = self.previous_orbit_mouse {
            let delta = mouse - previous;
            self.yaw -= delta.x * ORBIT_SENSITIVITY;
            self.pitch = (self.pitch - delta.y * ORBIT_SENSITIVITY).clamp(-MAX_PITCH, MAX_PITCH);
        }
        self.previous_orbit_mouse = Some(mouse);
    }

    /// Moves the view with the pointer, so the chamber follows the drag.
    fn drag_pan(&mut self, mouse: Vec2, view_height: f32) {
        if let Some(previous) = self.previous_pan_mouse {
            let delta = mouse - previous;
            let world_units_per_pixel = self.visible_height() / view_height.max(1.0);
            // Stored in camera-plane coordinates rather than moving the
            // orbit pivot; the basis rotates with the camera.
            self.pan_offset += vec2(-delta.x, delta.y) * world_units_per_pixel;
        }
        self.previous_pan_mouse = Some(mouse);
    }

    /// Scales the distance by e to the power `amount`, within the limits.
    fn zoom_by(&mut self, amount: f32) {
        if amount != 0.0 {
            self.distance = (self.distance * amount.exp()).clamp(
                MIN_DISTANCE * self.view_scale,
                MAX_DISTANCE * self.view_scale,
            );
        }
    }

    /// Height of the view at the orbit center, in world units.
    fn visible_height(&self) -> f32 {
        2.0 * self.distance * (self.camera().fovy / 2.0).tan()
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
#[path = "../../tests/unit/ui/camera.rs"]
mod tests;
