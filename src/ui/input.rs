//! Keyboard and mouse-wheel input: every key binding in one place, the help
//! text that lists them, and the wheel reading converted to pixels.

use macroquad::prelude::{
    KeyCode,
    Vec2,
    is_key_down,
    is_key_pressed,
    mouse_wheel, //
};

/// Starts the engine while Start is offered, and otherwise pauses or
/// resumes.
const GO_KEY: KeyCode = KeyCode::Space;
const RESET_GAS_KEY: KeyCode = KeyCode::T;
const DETAILS_KEY: KeyCode = KeyCode::I;
const PANEL_KEY: KeyCode = KeyCode::G;
const CAMERA_RESET_KEY: KeyCode = KeyCode::R;
const SLOW_KEY: KeyCode = KeyCode::L;
const KEYS_KEY: KeyCode = KeyCode::H;
const TRACERS_KEY: KeyCode = KeyCode::F;
const SCENERY_KEY: KeyCode = KeyCode::V;
const SLIDERS_KEY: KeyCode = KeyCode::C;
const HIDE_ALL_KEY: KeyCode = KeyCode::U;
/// The additive kick, which can overdrive the engine. Left out of the help
/// text on purpose; it exists for testing.
const KICK_KEY: KeyCode = KeyCode::K;
/// Keys 1 to 5 choose the fill presets, in order.
const FILL_KEYS: [KeyCode; 5] = [
    KeyCode::Key1,
    KeyCode::Key2,
    KeyCode::Key3,
    KeyCode::Key4,
    KeyCode::Key5,
];
const SHIFT_KEYS: [KeyCode; 2] = [KeyCode::LeftShift, KeyCode::RightShift];
const UP: Vec2 = Vec2::new(0.0, 1.0);
const DOWN: Vec2 = Vec2::new(0.0, -1.0);
const LEFT: Vec2 = Vec2::new(-1.0, 0.0);
const RIGHT: Vec2 = Vec2::new(1.0, 0.0);
/// Camera keys and the direction each one turns or moves the camera.
const CAMERA_DIRECTION_KEYS: [(KeyCode, Vec2); 8] = [
    (KeyCode::W, UP),
    (KeyCode::Up, UP),
    (KeyCode::S, DOWN),
    (KeyCode::Down, DOWN),
    (KeyCode::A, LEFT),
    (KeyCode::Left, LEFT),
    (KeyCode::D, RIGHT),
    (KeyCode::Right, RIGHT),
];
const ZOOM_IN_KEYS: [KeyCode; 2] = [KeyCode::Equal, KeyCode::KpAdd];
const ZOOM_OUT_KEYS: [KeyCode; 2] = [KeyCode::Minus, KeyCode::KpSubtract];

/// Every key binding the Keys list shows, as (keys, what they do). The
/// hidden kick is left out.
pub const SHORTCUTS: [(&str, &str); 15] = [
    ("Space", "Start, pause or resume"),
    ("L", "Slow motion"),
    ("F", "Follow particles (tracers)"),
    ("T", "Reset with fresh gas"),
    ("1 to 5", "Less or more helium"),
    ("I", "Simple or advanced readings"),
    ("G", "Show or hide the panel"),
    ("C", "Show or hide the sliders"),
    ("U", "Hide or show everything"),
    ("H", "Show or hide these keys"),
    ("V", "Sky or grid scenery"),
    ("Drag, WASD", "Turn the view"),
    ("Shift + either", "Move the view"),
    ("Wheel, - =", "Zoom"),
    ("R", "Reset the view"),
];

/// Scroll per wheel reading, in pixels. The browser and macOS report
/// pixels already; Windows reports 120 per notch and X11 reports 1.
#[cfg(any(target_arch = "wasm32", target_os = "macos"))]
const WHEEL_UNIT_PIXELS: f32 = 1.0;
#[cfg(target_os = "windows")]
const WHEEL_UNIT_PIXELS: f32 = 40.0 / 120.0;
#[cfg(not(any(target_arch = "wasm32", target_os = "macos", target_os = "windows")))]
const WHEEL_UNIT_PIXELS: f32 = 40.0;
/// Largest scroll accepted in one frame, so a platform that reports large
/// steps (such as Wayland) cannot jump the panel or the zoom.
const MAX_WHEEL_PIXELS: f32 = 200.0;

/// One-shot commands from keys pressed this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct KeyCommands {
    /// Start, or pause and resume (see `GO_KEY`).
    pub go: bool,
    pub reset_gas: bool,
    pub details: bool,
    pub panel: bool,
    pub keys: bool,
    pub camera_reset: bool,
    pub slow: bool,
    pub tracers: bool,
    pub scenery: bool,
    pub sliders: bool,
    pub hide_all: bool,
    pub kick: bool,
    /// Index of the fill preset chosen with keys 1 to 5.
    pub fill_preset: Option<usize>,
}

impl KeyCommands {
    pub fn read() -> Self {
        Self::from_pressed(is_key_pressed)
    }

    /// The commands for the keys that `pressed` reports.
    pub fn from_pressed(pressed: impl Fn(KeyCode) -> bool) -> Self {
        Self {
            go: pressed(GO_KEY),
            reset_gas: pressed(RESET_GAS_KEY),
            details: pressed(DETAILS_KEY),
            panel: pressed(PANEL_KEY),
            keys: pressed(KEYS_KEY),
            camera_reset: pressed(CAMERA_RESET_KEY),
            slow: pressed(SLOW_KEY),
            tracers: pressed(TRACERS_KEY),
            scenery: pressed(SCENERY_KEY),
            sliders: pressed(SLIDERS_KEY),
            hide_all: pressed(HIDE_ALL_KEY),
            kick: pressed(KICK_KEY),
            fill_preset: FILL_KEYS.into_iter().position(&pressed),
        }
    }
}

/// Camera keys held this frame. Each axis is -1, 0 or 1; x is right and y
/// is up.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraKeys {
    /// Direction to orbit the camera around the engine.
    pub orbit: Vec2,
    /// Direction to move (pan) the camera, with Shift held.
    pub pan: Vec2,
    /// 1 to zoom in, -1 to zoom out.
    pub zoom: f32,
}

impl CameraKeys {
    pub fn read() -> Self {
        Self::from_held(is_key_down)
    }

    /// The camera motion for the keys that `held` reports. Opposite keys
    /// cancel, and two keys for the same direction count once.
    pub fn from_held(held: impl Fn(KeyCode) -> bool) -> Self {
        let direction = CAMERA_DIRECTION_KEYS
            .into_iter()
            .filter(|&(key, _)| held(key))
            .fold(Vec2::ZERO, |sum, (_, direction)| sum + direction);
        let direction = Vec2::new(direction.x.clamp(-1.0, 1.0), direction.y.clamp(-1.0, 1.0));
        let zoom_in = ZOOM_IN_KEYS.into_iter().any(&held);
        let zoom_out = ZOOM_OUT_KEYS.into_iter().any(&held);
        let shift = SHIFT_KEYS.into_iter().any(&held);
        Self {
            orbit: if shift { Vec2::ZERO } else { direction },
            pan: if shift { direction } else { Vec2::ZERO },
            zoom: zoom_in as i32 as f32 - zoom_out as i32 as f32,
        }
    }
}

/// Whether either Shift key is held.
pub fn shift_down() -> bool {
    SHIFT_KEYS.into_iter().any(is_key_down)
}

/// Vertical wheel or two-finger scroll this frame, in pixels; positive
/// scrolls up (toward the top of a page) or zooms in.
pub fn wheel_pixels() -> f32 {
    normalized_wheel(mouse_wheel().1)
}

fn normalized_wheel(reading: f32) -> f32 {
    (reading * WHEEL_UNIT_PIXELS).clamp(-MAX_WHEEL_PIXELS, MAX_WHEEL_PIXELS)
}

#[cfg(test)]
#[path = "../../tests/unit/ui/input.rs"]
mod tests;
