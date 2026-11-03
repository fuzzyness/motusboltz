//! Orbit, pan and zoom for the 3D scene, by mouse and by keyboard.

use std::f32::consts::FRAC_PI_2;

use macroquad::prelude::{
    Camera3D,
    MouseButton,
    Vec2,
    Vec3,
    is_mouse_button_down,
    is_mouse_button_pressed,
    mouse_position,
    vec2,
    vec3, //
};

use crate::ui::input::CameraKeys;

/// Where the camera starts, from the point it turns around, before the
/// view's scale.
const INITIAL_OFFSET: Vec3 = Vec3::new(8.4, 4.0, 8.4);
/// The point the camera turns around.
///
/// It is above the block's middle, halfway up the whole engine from the
/// floor to the top of the flywheel, so all of it fits the first view.
const ORBIT_CENTER: Vec3 = Vec3::new(0.0, 2.6, 0.0);

/// The nearest distance drawn.
///
/// The softened view's depth buffer has 16 bits, so the range is kept as
/// tight as the scene allows (the camera never comes closer than
/// `MIN_DISTANCE` times the view's scale, at least 4, to the orbit center);
/// a far range of 10,000 with a near of 0.01 left surfaces flickering
/// through each other.
const NEAR: f32 = 1.0;
/// The farthest distance drawn (see `NEAR`).
const FAR: f32 = 400.0;

/// Orbit per pixel of mouse drag, in radians.
const ORBIT_SENSITIVITY: f32 = 0.005;
/// How far short of straight up or down the camera stops, in radians.
const POLE_MARGIN: f32 = 0.01;
/// Steepest pitch, looking down.
const MAX_PITCH: f32 = FRAC_PI_2 - POLE_MARGIN;
/// Lowest pitch, a little under level: the camera never goes down under
/// the table or the cloud, out of the world.
const MIN_PITCH: f32 = -0.1;
/// Farthest pan across the view either way, before the view's scale, so
/// the engine never leaves the world's middle.
const MAX_PAN_ACROSS: f32 = 6.0;
/// Farthest pan up, before the view's scale.
const MAX_PAN_UP: f32 = 5.0;
/// Farthest pan down, before the view's scale: not far, or the camera
/// would sink under the table.
const MAX_PAN_DOWN: f32 = 2.0;

/// Zoom per pixel of wheel scroll, as a fraction of the distance.
const ZOOM_SENSITIVITY: f32 = 0.005;
/// Nearest distance to the orbit center, before the view's scale.
const MIN_DISTANCE: f32 = 4.0;
/// Farthest distance to the orbit center, before the view's scale.
const MAX_DISTANCE: f32 = 31.25;

/// Keyboard orbit speed, in radians per second.
const KEY_ORBIT_SPEED: f32 = 1.2;
/// Keyboard pan speed, as a fraction of the visible height per second.
const KEY_PAN_SPEED: f32 = 0.5;
/// Keyboard zoom rate, in e-folds of the distance per second.
const KEY_ZOOM_RATE: f32 = 0.9;

/// Pointer state for one frame, read by the caller.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pointer {
    /// False while the pointer is over the panel or a control, or a gesture
    /// that started there is still held.
    pub enabled: bool,
    /// Whether Shift is held, which turns a left drag into a pan.
    pub shift_down: bool,
    /// Wheel scroll this frame, in pixels; positive zooms in.
    pub wheel_pixels: f32,
}

/// What a mouse drag does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Drag {
    /// Turns the camera around the engine.
    Orbit,
    /// Moves the camera in its own plane.
    Pan,
}

/// Camera that orbits the chamber while keeping its pan offset in view space.
///
/// The orbit center stays fixed at the chamber. Panning moves the chamber in
/// the viewport without changing the point around which the camera rotates.
pub struct OrbitCamera {
    /// Distance to the orbit center.
    distance: f32,
    /// Scale of the view, which scales the distances.
    view_scale: f32,
    /// Turn around the vertical axis, in radians.
    yaw: f32,
    /// Angle above the horizontal, in radians.
    pitch: f32,
    /// Pan in the camera's own plane, in world units.
    pan_offset: Vec2,
    /// The drag in progress and where the pointer was last frame.
    drag: Option<(Drag, Vec2)>,
    /// A button was pressed over the scene and is still held: only then
    /// does the camera move with the mouse, so a button that only reads as
    /// held, its release lost (as when the window switches to full screen
    /// mid-click), does not turn it.
    gesture: bool,
}

impl OrbitCamera {
    /// A camera framing a chamber scaled by `view_scale`, which keeps that
    /// framing on reset.
    ///
    /// # Panics
    ///
    /// Panics if `view_scale` is not positive and finite.
    pub fn with_scale(view_scale: f32) -> Self {
        assert!(view_scale.is_finite() && view_scale > 0.0);
        let offset = INITIAL_OFFSET;
        let distance = offset.length();

        Self {
            distance: distance * view_scale,
            view_scale,
            yaw: offset.x.atan2(offset.z),
            pitch: (offset.y / distance).asin(),
            pan_offset: Vec2::ZERO,
            drag: None,
            gesture: false,
        }
    }

    /// Restores the original orbit, framing, and zoom level.
    pub fn reset(&mut self) {
        *self = Self::with_scale(self.view_scale);
    }

    /// Applies one frame of input.
    ///
    /// Held camera keys act over `dt` seconds, and mouse drags and the
    /// wheel act when the pointer is enabled.
    pub fn update(&mut self, keys: CameraKeys, pointer: Pointer, dt: f32, view_height: f32) {
        self.apply_keys(keys, dt);

        // The mouse is over a control or the panel, or a gesture that
        // started there is held; drop any drag so the camera does not jump.
        if !pointer.enabled {
            self.drag = None;
            self.gesture = false;
            return;
        }

        let left_down = is_mouse_button_down(MouseButton::Left);
        let pressed = is_mouse_button_pressed(MouseButton::Left)
            || is_mouse_button_pressed(MouseButton::Right);
        // A right drag pans, as Shift with a left drag does. In the browser,
        // the page keeps the context menu from opening and hands on a
        // release outside the canvas, so the drag always ends (see
        // `web/page.js`).
        let right_down = is_mouse_button_down(MouseButton::Right);
        self.gesture = gesture_held(self.gesture, pressed, left_down || right_down);
        let drag = if !self.gesture {
            None
        } else if right_down || (left_down && pointer.shift_down) {
            Some(Drag::Pan)
        } else {
            left_down.then_some(Drag::Orbit)
        };
        let mouse = Vec2::from(mouse_position());
        // A drag moves the camera from the second frame of the same kind of
        // drag on.
        if let Some((kind, previous)) = self.drag.filter(|&(kind, _)| Some(kind) == drag) {
            let delta = mouse - previous;
            match kind {
                Drag::Orbit => self.orbit_by(delta),
                Drag::Pan => self.pan_by(delta, view_height),
            }
        }
        self.drag = drag.map(|kind| (kind, mouse));

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
        self.pitch = (self.pitch + keys.orbit.y * KEY_ORBIT_SPEED * dt).clamp(MIN_PITCH, MAX_PITCH);
        self.pan_offset += keys.pan * self.visible_height() * KEY_PAN_SPEED * dt;
        self.keep_pan_in_bounds();
        self.zoom_by(-keys.zoom * KEY_ZOOM_RATE * dt);
    }

    /// Turns the camera for a mouse drag of `delta` pixels.
    fn orbit_by(&mut self, delta: Vec2) {
        self.yaw -= delta.x * ORBIT_SENSITIVITY;
        self.pitch = (self.pitch - delta.y * ORBIT_SENSITIVITY).clamp(MIN_PITCH, MAX_PITCH);
    }

    /// Moves the view with a mouse drag of `delta` pixels in a view
    /// `view_height` pixels high, so the chamber follows the drag.
    fn pan_by(&mut self, delta: Vec2, view_height: f32) {
        let world_units_per_pixel = self.visible_height() / view_height.max(1.0);
        // Stored in camera-plane coordinates rather than moving the orbit
        // pivot; the basis rotates with the camera.
        self.pan_offset += vec2(-delta.x, delta.y) * world_units_per_pixel;
        self.keep_pan_in_bounds();
    }

    /// Holds the pan within the world's middle (see `MAX_PAN_ACROSS`).
    fn keep_pan_in_bounds(&mut self) {
        let scale = self.view_scale;
        self.pan_offset = vec2(
            self.pan_offset
                .x
                .clamp(-MAX_PAN_ACROSS * scale, MAX_PAN_ACROSS * scale),
            self.pan_offset
                .y
                .clamp(-MAX_PAN_DOWN * scale, MAX_PAN_UP * scale),
        );
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

    /// The camera's offset from the orbit center, before the pan.
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
            z_near: NEAR,
            z_far: FAR,
            ..Default::default()
        }
    }
}

/// Whether a drag gesture is under way this frame, for one that `was`
/// last frame, a button `pressed` over the scene this frame, and a button
/// `down`: it starts with a press and lasts while a button is held, so a
/// button that only reads as held, its press never seen, starts none.
fn gesture_held(was: bool, pressed: bool, down: bool) -> bool {
    (was || pressed) && down
}

#[cfg(test)]
#[path = "../../tests/unit/ui/camera.rs"]
mod tests;
