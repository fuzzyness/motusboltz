//! Main loop: input, simulation stepping, and screen layout. The 3D scene
//! sits between the readings panel on the left and the controls on the
//! right.

use macroquad::prelude::{
    Color,
    KeyCode,
    MouseButton,
    Vec2,
    WHITE,
    clear_background,
    draw_rectangle,
    draw_text,
    get_frame_time,
    is_key_pressed,
    is_mouse_button_down,
    is_mouse_button_pressed,
    measure_text,
    mouse_position,
    next_frame,
    screen_height,
    screen_width,
    set_camera,
    set_default_camera, //
};

use crate::camera::OrbitCamera;
use crate::engine::{
    ChamberGeometry,
    ConnectedChambers,
    KickDirection,
    //
};
use crate::gas::{
    DEFAULT_INITIAL_CONCENTRATION_MOL_PER_L,
    MAX_INITIAL_CONCENTRATION_MOL_PER_L,
    MIN_INITIAL_CONCENTRATION_MOL_PER_L, //
};
use crate::graphics::{
    ConnectedChamberVisual,
    CylinderDimensions,
    ParticleRenderer,
    ShaftVisual, //
};
use crate::panel::{
    COLUMN_WIDTH,
    EnginePanel,
    TITLE,
    fitted_text, //
};
use crate::slider::Slider;

const BACKGROUND_COLOR: Color = Color {
    r: 12.0 / 255.0,
    g: 18.0 / 255.0,
    b: 30.0 / 255.0,
    a: 1.0,
};
const PANEL_COLOR: Color = Color::from_rgba(38, 52, 68, 255);
const RIGHT_MARGIN: f32 = 32.0;
const CONTROL_WIDTH: f32 = 260.0;
const CONTROL_TOP: f32 = 24.0;
const BUTTON_HEIGHT: f32 = 40.0;
const BUTTON_GAP: f32 = 12.0;
const BUTTON_GAP_X: f32 = 10.0;
const BUTTON_FONT_SIZE: f32 = 18.0;
const BUTTON_WIDTH: f32 = (CONTROL_WIDTH - BUTTON_GAP_X) / 2.0;
#[cfg(test)]
const BUTTON_ROWS: usize = 2;
/// Width kept free for the controls at the right edge.
const CONTROL_COLUMN_WIDTH: f32 = CONTROL_WIDTH + RIGHT_MARGIN + crate::slider::PANEL_PADDING_X;
const SCENE_TEXT_MARGIN: f32 = 20.0;
const TITLE_Y: f32 = 36.0;
const TITLE_FONT_SIZE: f32 = 28.0;
const HELP_FONT_SIZE: f32 = 16.0;
const HELP_BOTTOM_Y: f32 = 24.0;
const HELP_TEXT: &str =
    "1-5: Fill | Space: Pause | A: Kick | T: Reset | R: Camera | D: Details | G: Panel";
/// Clock resolution used for the startup seed, in ticks per second.
const SEED_TICKS_PER_SECOND: f64 = 1.0e6;
/// Keys 1 to 5: 256, 512, 1,024, 1,536 and 2,048 particles.
const CONCENTRATION_PRESETS: [f32; 5] = [
    MIN_INITIAL_CONCENTRATION_MOL_PER_L,
    0.25,
    DEFAULT_INITIAL_CONCENTRATION_MOL_PER_L,
    0.75,
    MAX_INITIAL_CONCENTRATION_MOL_PER_L,
];
const PRESET_KEYS: [KeyCode; CONCENTRATION_PRESETS.len()] = [
    KeyCode::Key1,
    KeyCode::Key2,
    KeyCode::Key3,
    KeyCode::Key4,
    KeyCode::Key5,
];

/// On-screen button actions, in button order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    ResetGas,
    TogglePause,
    KickNegative,
    ToggleDetails,
}

const BUTTON_ACTIONS: [Action; 4] = [
    Action::ResetGas,
    Action::TogglePause,
    Action::KickNegative,
    Action::ToggleDetails,
];

/// Left edge, top edge and width of button `index` in a two-column grid.
fn button_rect(index: usize, screen_width: f32) -> (f32, f32, f32) {
    let col = (index % 2) as f32;
    let row = (index / 2) as f32;
    let x = screen_width - CONTROL_WIDTH - RIGHT_MARGIN + col * (BUTTON_WIDTH + BUTTON_GAP_X);
    let y = CONTROL_TOP + row * (BUTTON_HEIGHT + BUTTON_GAP);
    (x, y, BUTTON_WIDTH)
}

fn button_at(x: f32, y: f32, bx: f32, by: f32, width: f32) -> bool {
    (bx..=bx + width).contains(&x) && (by..=by + BUTTON_HEIGHT).contains(&y)
}

/// Returns whether the pointer overlaps a button and its click action.
fn buttons() -> (bool, Option<Action>) {
    let (x, y) = mouse_position();
    let hovered = BUTTON_ACTIONS
        .iter()
        .enumerate()
        .find_map(|(index, &action)| {
            let (bx, by, width) = button_rect(index, screen_width());
            button_at(x, y, bx, by, width).then_some(action)
        });
    let action = if is_mouse_button_pressed(MouseButton::Left) {
        hovered
    } else {
        None
    };
    (hovered.is_some(), action)
}

fn draw_button(x: f32, y: f32, width: f32, label: &str) {
    draw_rectangle(x, y, width, BUTTON_HEIGHT, PANEL_COLOR);
    let metrics = measure_text(label, None, BUTTON_FONT_SIZE as u16, 1.0);
    draw_text(
        label,
        x + (width - metrics.width) / 2.0,
        y + (BUTTON_HEIGHT + metrics.height) / 2.0,
        BUTTON_FONT_SIZE,
        WHITE,
    );
}

fn draw_buttons(paused: bool, details: bool) {
    let labels = [
        "Reset (T)",
        if paused {
            "Resume (Space)"
        } else {
            "Pause (Space)"
        },
        "Kick (A)",
        if details { "Less (D)" } else { "Details (D)" },
    ];
    for (index, label) in labels.iter().enumerate() {
        let (x, y, width) = button_rect(index, screen_width());
        draw_button(x, y, width, label);
    }
}

/// Left edge and width of the 3D scene: between the panel (when shown)
/// and the control column.
fn scene_span(screen_width: f32, show_panel: bool) -> (f32, f32) {
    let left = if show_panel { COLUMN_WIDTH } else { 0.0 };
    (left, (screen_width - left - CONTROL_COLUMN_WIDTH).max(1.0))
}

/// Extra zoom-out so the passage, crank and flywheel all fit the first view.
const CAMERA_FRAMING_MARGIN: f32 = 1.25;
fn initial_camera(geometry: ChamberGeometry) -> OrbitCamera {
    OrbitCamera::with_scale(
        CAMERA_FRAMING_MARGIN * geometry.cylinder_radius
            / ChamberGeometry::scaled(1.0).cylinder_radius,
    )
}

/// Seed from the wall clock, so every launch starts from a different gas.
/// `miniquad::date::now` works natively and in the browser.
fn clock_seed() -> u64 {
    (macroquad::miniquad::date::now() * SEED_TICKS_PER_SECOND) as u64
}

/// Runs the application: a default fill at 300 K with both reservoirs at
/// 300 K and the flywheel at rest until the first kick.
pub async fn run() {
    macroquad::rand::srand(clock_seed());
    let mut simulation =
        ConnectedChambers::with_initial_concentration(DEFAULT_INITIAL_CONCENTRATION_MOL_PER_L);
    let mut paused = false;
    let mut panel = EnginePanel::new();
    let mut show_panel = true;
    let geometry = simulation.geometry();
    let mut camera = initial_camera(geometry);
    let mut renderer = ParticleRenderer::new();
    let mut chamber = ConnectedChamberVisual::new(geometry);
    let shaft = ShaftVisual::new(geometry);
    // A cylinder enclosing the whole apparatus, for the particle depth fade.
    let draw_dimensions = CylinderDimensions {
        radius: geometry.cylinder_offset + geometry.cylinder_radius,
        height: geometry.grid_side(),
    };
    let mut concentration_slider = Slider::concentration(DEFAULT_INITIAL_CONCENTRATION_MOL_PER_L);
    let mut hot_slider = Slider::hot_temperature(simulation.hot_kelvin());
    let mut cold_slider = Slider::cold_temperature(simulation.cold_kelvin());
    // True while a press that started on a button is held, so dragging off
    // the button does not rotate the camera.
    let mut button_gesture = false;

    loop {
        let (over_buttons, action) = buttons();
        if !is_mouse_button_down(MouseButton::Left) {
            button_gesture = false;
        }
        button_gesture |= action.is_some();
        if is_key_pressed(KeyCode::T) || action == Some(Action::ResetGas) {
            simulation.reset();
        }
        if is_key_pressed(KeyCode::Space) || action == Some(Action::TogglePause) {
            paused = !paused;
        }
        if is_key_pressed(KeyCode::A) || action == Some(Action::KickNegative) {
            simulation.kick_flywheel(KickDirection::Negative);
        }
        if is_key_pressed(KeyCode::D) || action == Some(Action::ToggleDetails) {
            panel.toggle_details();
        }

        for (&key, &concentration) in PRESET_KEYS.iter().zip(&CONCENTRATION_PRESETS) {
            if is_key_pressed(key) {
                simulation.set_initial_concentration(concentration);
                concentration_slider.set_value(concentration);
                break;
            }
        }

        if concentration_slider.update().released {
            simulation.set_initial_concentration(concentration_slider.value());
        }
        if hot_slider.update().changed {
            simulation.set_hot_kelvin(hot_slider.value());
        }

        if cold_slider.update().changed {
            simulation.set_cold_kelvin(cold_slider.value());
        }
        let limited = !paused && simulation.update(get_frame_time());
        if show_panel {
            panel.update(&simulation, limited, macroquad::time::get_time());
        }
        if is_key_pressed(KeyCode::G) {
            show_panel = !show_panel;
        }
        camera.update(
            !(concentration_slider.blocks_camera()
                || hot_slider.blocks_camera()
                || cold_slider.blocks_camera()
                || over_buttons
                || button_gesture
                || (show_panel && EnginePanel::blocks_camera(Vec2::from(mouse_position())))),
        );
        let mut view_camera = camera.camera();
        let (scene_left, scene_width) = scene_span(screen_width(), show_panel);
        let scene_height = screen_height().max(1.0);
        view_camera.viewport = Some((
            scene_left as i32,
            0,
            scene_width as i32,
            scene_height as i32,
        ));
        view_camera.aspect = Some(scene_width / scene_height);
        clear_background(BACKGROUND_COLOR);
        set_camera(&view_camera);
        chamber.draw(
            simulation.power_pistons().left().face_y,
            simulation.power_pistons().right().face_y,
        );
        shaft.draw(simulation.power_pistons());
        renderer.draw(
            &simulation.particles,
            simulation.collision_flashes(),
            simulation.particle_radius(),
            &view_camera,
            draw_dimensions,
            BACKGROUND_COLOR,
        );
        set_default_camera();

        if show_panel {
            panel.draw(&simulation, paused);
        } else {
            draw_text(TITLE, SCENE_TEXT_MARGIN, TITLE_Y, TITLE_FONT_SIZE, WHITE);
        }
        fitted_text(
            HELP_TEXT,
            scene_left + SCENE_TEXT_MARGIN,
            screen_height() - HELP_BOTTOM_Y,
            scene_width - 2.0 * SCENE_TEXT_MARGIN,
            HELP_FONT_SIZE,
            WHITE,
        );
        draw_buttons(paused, panel.details_shown());
        concentration_slider.draw();
        hot_slider.draw();
        cold_slider.draw();
        next_frame().await;
    }
}

#[cfg(test)]
#[path = "../tests/unit/ui.rs"]
mod tests;
