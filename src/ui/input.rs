//! Keyboard and mouse-wheel input: every key binding in one place, the help
//! text that lists them, and the wheel reading in pixels.

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
/// Refills with fresh gas.
const RESET_GAS_KEY: KeyCode = KeyCode::T;
/// Switches the panel between simple and advanced readings.
const DETAILS_KEY: KeyCode = KeyCode::I;
/// Shows or hides the panel.
const PANEL_KEY: KeyCode = KeyCode::G;
/// Puts the camera back.
const CAMERA_RESET_KEY: KeyCode = KeyCode::R;
/// Turns slow motion on or off.
const SLOW_KEY: KeyCode = KeyCode::L;
/// Shows or hides the keys list.
const KEYS_KEY: KeyCode = KeyCode::H;
/// Turns the tracers on or off.
const TRACERS_KEY: KeyCode = KeyCode::F;
/// Switches between the sky and the grid scenery.
const SCENERY_KEY: KeyCode = KeyCode::V;
/// Shows or hides the sliders.
const SLIDERS_KEY: KeyCode = KeyCode::C;
/// Hides or shows everything around the scene.
const HIDE_ALL_KEY: KeyCode = KeyCode::U;
/// Opens or closes the Demos menu.
const DEMOS_KEY: KeyCode = KeyCode::M;
/// Shows or hides the labels on the scene.
const LABELS_KEY: KeyCode = KeyCode::N;
/// Puts the regenerator in the opening or takes it out.
const REGENERATOR_KEY: KeyCode = KeyCode::E;
/// Gives the flywheel an additive kick, which can overdrive the engine.
///
/// It is left out of the help text on purpose; it exists for testing.
const KICK_KEY: KeyCode = KeyCode::K;
/// Keys 1 to 5 choose the fill presets, in order.
const FILL_KEYS: [KeyCode; 5] = [
    KeyCode::Key1,
    KeyCode::Key2,
    KeyCode::Key3,
    KeyCode::Key4,
    KeyCode::Key5,
];
/// Either Shift key turns the camera keys from orbiting into moving.
const SHIFT_KEYS: [KeyCode; 2] = [KeyCode::LeftShift, KeyCode::RightShift];
/// Camera direction up.
const UP: Vec2 = Vec2::new(0.0, 1.0);
/// Camera direction down.
const DOWN: Vec2 = Vec2::new(0.0, -1.0);
/// Camera direction left.
const LEFT: Vec2 = Vec2::new(-1.0, 0.0);
/// Camera direction right.
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
/// Keys that zoom in.
const ZOOM_IN_KEYS: [KeyCode; 2] = [KeyCode::Equal, KeyCode::KpAdd];
/// Keys that zoom out.
const ZOOM_OUT_KEYS: [KeyCode; 2] = [KeyCode::Minus, KeyCode::KpSubtract];

/// Every key binding the Keys list shows, as (keys, what they do).
///
/// The hidden kick is left out.
pub const SHORTCUTS: [(&str, &str); 19] = [
    ("Space", "Start, pause or resume"),
    ("L", "Slow motion"),
    ("F", "Follow particles (tracers)"),
    ("T", "Reset with fresh gas"),
    ("1 to 5", "Less or more helium"),
    ("E", "Regenerator in or out"),
    ("M", "Demos: ready-made settings"),
    ("N", "Show or hide the labels"),
    ("I", "Simple or advanced readings"),
    ("G", "Show or hide the readings"),
    ("C", "Show or hide the settings"),
    ("U", "Hide or show everything"),
    ("H", "Show or hide these keys"),
    ("V", "Sky or grid scenery"),
    ("Drag, WASD", "Turn the view"),
    ("Shift + either", "Move the view"),
    ("Right drag", "Move the view"),
    ("Wheel, - =", "Zoom"),
    ("R", "Reset the view"),
];

/// Scroll per wheel reading in the browser and on macOS, which report
/// pixels already.
#[cfg(any(target_arch = "wasm32", target_os = "macos"))]
const WHEEL_UNIT_PIXELS: f32 = 1.0;
/// Scroll per wheel reading on Windows, which reports 120 per notch.
#[cfg(target_os = "windows")]
const WHEEL_UNIT_PIXELS: f32 = 40.0 / 120.0;
/// Scroll per wheel reading on X11, which reports 1 per notch.
#[cfg(not(any(target_arch = "wasm32", target_os = "macos", target_os = "windows")))]
const WHEEL_UNIT_PIXELS: f32 = 40.0;
/// Largest scroll accepted in one frame, in pixels.
///
/// The limit keeps a platform that reports large steps (such as Wayland)
/// from jumping the panel or the zoom.
const MAX_WHEEL_PIXELS: f32 = 200.0;

/// One-shot commands from keys pressed this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct KeyCommands {
    /// Start, or pause and resume (see `GO_KEY`).
    pub go: bool,
    /// Refill with fresh gas.
    pub reset_gas: bool,
    /// Switch between simple and advanced readings.
    pub details: bool,
    /// Show or hide the panel.
    pub panel: bool,
    /// Show or hide the keys list.
    pub keys: bool,
    /// Put the camera back.
    pub camera_reset: bool,
    /// Turn slow motion on or off.
    pub slow: bool,
    /// Turn the tracers on or off.
    pub tracers: bool,
    /// Switch the scenery.
    pub scenery: bool,
    /// Show or hide the sliders.
    pub sliders: bool,
    /// Hide or show everything.
    pub hide_all: bool,
    /// Open or close the Demos menu.
    pub demos: bool,
    /// Show or hide the labels.
    pub labels: bool,
    /// Put the regenerator in or take it out.
    pub regenerator: bool,
    /// Kick the flywheel.
    pub kick: bool,
    /// Index of the fill preset chosen with keys 1 to 5.
    pub fill_preset: Option<usize>,
}

impl KeyCommands {
    /// The commands for the keys pressed this frame.
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
            demos: pressed(DEMOS_KEY),
            labels: pressed(LABELS_KEY),
            regenerator: pressed(REGENERATOR_KEY),
            kick: pressed(KICK_KEY),
            fill_preset: FILL_KEYS.into_iter().position(&pressed),
        }
    }
}

/// Camera keys held this frame.
///
/// Each axis is -1, 0 or 1; x is right and y is up.
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
    /// The camera keys held this frame.
    pub fn read() -> Self {
        Self::from_held(is_key_down)
    }

    /// The camera motion for the keys that `held` reports.
    ///
    /// Opposite keys cancel, and two keys for the same direction count
    /// once.
    pub fn from_held(held: impl Fn(KeyCode) -> bool) -> Self {
        let direction = CAMERA_DIRECTION_KEYS
            .into_iter()
            .filter(|&(key, _)| held(key))
            .fold(Vec2::ZERO, |sum, (_, direction)| sum + direction);
        let direction = direction.clamp(Vec2::NEG_ONE, Vec2::ONE);
        let zoom_in = ZOOM_IN_KEYS.into_iter().any(&held);
        let zoom_out = ZOOM_OUT_KEYS.into_iter().any(&held);
        let shift = SHIFT_KEYS.into_iter().any(&held);
        Self {
            orbit: if shift { Vec2::ZERO } else { direction },
            pan: if shift { direction } else { Vec2::ZERO },
            zoom: f32::from(zoom_in) - f32::from(zoom_out),
        }
    }
}

/// Whether either Shift key is held.
pub fn shift_down() -> bool {
    SHIFT_KEYS.into_iter().any(is_key_down)
}

/// Vertical wheel or two-finger scroll this frame, in pixels.
///
/// Positive scrolls up (toward the top of a page) or zooms in.
pub fn wheel_pixels() -> f32 {
    normalized_wheel(mouse_wheel().1)
}

/// Scroll in pixels for a wheel `reading`, limited to one frame's worth.
fn normalized_wheel(reading: f32) -> f32 {
    (reading * WHEEL_UNIT_PIXELS).clamp(-MAX_WHEEL_PIXELS, MAX_WHEEL_PIXELS)
}

#[cfg(test)]
#[path = "../../tests/unit/ui/input.rs"]
mod tests;
