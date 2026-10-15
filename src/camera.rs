use std::f32::consts::FRAC_PI_2;

use macroquad::prelude::*;

const INITIAL_POSITION: Vec3 = Vec3::new(6.0, 4.0, 6.0);
const INITIAL_TARGET: Vec3 = Vec3::ZERO;

const ORBIT_SENSITIVITY: f32 = 0.005;
const POLE_MARGIN: f32 = 0.01;
const MAX_PITCH: f32 = FRAC_PI_2 - POLE_MARGIN;

const ZOOM_SENSITIVITY: f32 = 0.005;
const MIN_DISTANCE: f32 = 4.0;
const MAX_DISTANCE: f32 = 25.0;

pub struct OrbitCamera {
    target: Vec3,
    distance: f32,
    yaw: f32,
    pitch: f32,
    previous_orbit_mouse: Option<Vec2>,
    previous_pan_mouse: Option<Vec2>,
}

impl OrbitCamera {
    pub fn new() -> Self {
        let offset = INITIAL_POSITION - INITIAL_TARGET;
        let distance = offset.length();

        Self {
            target: INITIAL_TARGET,
            distance,
            yaw: offset.x.atan2(offset.z),
            pitch: (offset.y / distance).asin(),
            previous_orbit_mouse: None,
            previous_pan_mouse: None,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn update(&mut self) {
        if is_key_pressed(KeyCode::R) {
            println!("Camera reset");
            self.reset();
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

        // Prevent scrolling gestures from triggering zoom
        // while the user is dragging the camera.
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

            // Calculate the camera's local axes.
            let forward = (camera.target - camera.position).normalize();
            let right = forward.cross(camera.up).normalize();
            let up = right.cross(forward).normalize();

            // Convert pixel movement into world-space movement.
            let world_units_per_pixel =
                2.0 * self.distance * (camera.fovy / 2.0).tan() / screen_height();

            let translation = (-right * delta.x + up * delta.y) * world_units_per_pixel;

            self.target += translation;
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

    pub fn camera(&self) -> Camera3D {
        let cos_pitch = self.pitch.cos();

        let position = self.target
            + vec3(
                self.distance * cos_pitch * self.yaw.sin(),
                self.distance * self.pitch.sin(),
                self.distance * cos_pitch * self.yaw.cos(),
            );

        Camera3D {
            position,
            target: self.target,
            up: Vec3::Y,
            ..Default::default()
        }
    }
}
