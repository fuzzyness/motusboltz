//! User interface and real-time simulation loop.

use macroquad::prelude::{
    Color,
    KeyCode,
    MouseButton,
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
    screen_width,
    set_camera,
    set_default_camera, //
};

use crate::camera::OrbitCamera;
use crate::engine::{
    ChamberGeometry,
    ConnectedChambers,
    KickDirection,
    MotionMode, //
};
use crate::gas::{
    DEFAULT_INITIAL_CONCENTRATION_MOL_PER_L,
    GasMeasurements,
    MAX_INITIAL_CONCENTRATION_MOL_PER_L,
    MIN_INITIAL_CONCENTRATION_MOL_PER_L, //
};
use crate::graphics::{
    ConnectedChamberVisual,
    CylinderDimensions,
    ParticleRenderer, //
    ShaftVisual,
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
const BUTTON_ROWS: usize = 3;
const HUD_X: f32 = 20.0;
const TITLE_Y: f32 = 36.0;
const STATS_START_Y: f32 = 75.0;
const HUD_LINE_HEIGHT: f32 = 25.0;
const HUD_FONT_SIZE: f32 = 19.0;
const TITLE_FONT_SIZE: f32 = 28.0;
const HELP_FONT_SIZE: f32 = 19.0;
const HELP_BOTTOM_Y: f32 = 24.0;
/// Quick presets represent 256, 512, 1,024, 1,536 and 2,048 markers.
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DemoAction {
    ToggleStats,
    ResetGas,
    TogglePistons,
    ToggleMode,
    KickPositive,
    KickNegative,
}

/// Button positions are shared by pointer hit testing and drawing.
const BUTTON_ACTIONS: [DemoAction; 6] = [
    DemoAction::ToggleStats,
    DemoAction::ResetGas,
    DemoAction::TogglePistons,
    DemoAction::ToggleMode,
    DemoAction::KickNegative,
    DemoAction::KickPositive,
];

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
fn demo_buttons() -> (bool, Option<DemoAction>) {
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

fn draw_buttons(show_stats: bool, pistons_running: bool, mode: MotionMode) {
    let labels = [
        if show_stats {
            "Hide stats"
        } else {
            "Show stats"
        },
        "Reset gas",
        if pistons_running { "Pause" } else { "Resume" },
        if mode == MotionMode::Free {
            "Mode: Free"
        } else {
            "Mode: Fixed"
        },
        "Kick - (A)",
        "Kick + (S)",
    ];
    for (index, label) in labels.iter().enumerate() {
        let (x, y, width) = button_rect(index, screen_width());
        draw_button(x, y, width, label);
    }
}

/// Formats marker count, temperature, molarity, and estimated ideal-gas pressure.
fn region_stats(name: &str, sample: GasMeasurements) -> String {
    let temperature = sample
        .temperature_kelvin
        .map_or_else(|| "--".to_owned(), |value| format!("{value:.0}"));
    let pressure = sample.pressure_pa.map_or_else(
        || "--".to_owned(),
        |value| format!("{:.1}", value / 100_000.0),
    );
    format!(
        "{name}: {} | {temperature} K | {:.2} M | ~{pressure} bar",
        sample.markers, sample.concentration_mol_per_l,
    )
}

/// Starts with an evenly spaced helium fill at 300 K in the connected volume.
pub async fn run() {
    let mut simulation =
        ConnectedChambers::with_initial_concentration(DEFAULT_INITIAL_CONCENTRATION_MOL_PER_L);
    let geometry = simulation.geometry();
    let mut camera = OrbitCamera::with_scale(
        geometry.cylinder_radius / ChamberGeometry::scaled(1.0).cylinder_radius,
    );
    let mut renderer = ParticleRenderer::new();
    let mut chamber = ConnectedChamberVisual::new(geometry);
    let shaft = ShaftVisual::new(geometry);
    let draw_dimensions = CylinderDimensions {
        radius: geometry.cylinder_offset + geometry.cylinder_radius,
        height: geometry.cylinder_height,
    };
    let mut concentration_slider =
        Slider::demo_concentration(DEFAULT_INITIAL_CONCENTRATION_MOL_PER_L);
    let mut heater_slider = Slider::demo_heater(simulation.heater_percent());
    let mut show_stats = true;
    let mut button_gesture = false;

    loop {
        let (over_buttons, action) = demo_buttons();
        if !is_mouse_button_down(MouseButton::Left) {
            button_gesture = false;
        }
        button_gesture |= action.is_some();
        if is_key_pressed(KeyCode::T) || action == Some(DemoAction::ResetGas) {
            simulation.reset();
        }
        if action == Some(DemoAction::ToggleStats) {
            show_stats = !show_stats;
        }
        if is_key_pressed(KeyCode::P) || action == Some(DemoAction::TogglePistons) {
            simulation.set_pistons_running(!simulation.power_pistons().running());
        }
        if is_key_pressed(KeyCode::M) || action == Some(DemoAction::ToggleMode) {
            let next = if simulation.power_pistons().mode() == MotionMode::Free {
                MotionMode::Controlled
            } else {
                MotionMode::Free
            };
            simulation.set_piston_mode(next);
        }
        if is_key_pressed(KeyCode::S) || action == Some(DemoAction::KickPositive) {
            simulation.kick_flywheel(KickDirection::Positive);
        }
        if is_key_pressed(KeyCode::A) || action == Some(DemoAction::KickNegative) {
            simulation.kick_flywheel(KickDirection::Negative);
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
        if heater_slider.update().changed {
            simulation.set_heater_percent(heater_slider.value());
        }

        simulation.update(get_frame_time());
        camera.update(
            !(concentration_slider.blocks_camera()
                || heater_slider.blocks_camera()
                || over_buttons
                || button_gesture),
        );
        let view_camera = camera.camera();
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

        draw_text("MotusBoltz", HUD_X, TITLE_Y, TITLE_FONT_SIZE, WHITE);
        if show_stats {
            let regions = simulation.measurements();
            let heat = simulation.heat();
            let pistons = simulation.power_pistons();
            let crank_mode = match pistons.mode() {
                MotionMode::Free => "Free",
                MotionMode::Controlled => "Controlled",
            };
            let crank_status = if pistons.running() { "" } else { " (paused)" };
            let stats = [
                format!(
                    "Helium: {} markers | {:.4} mol | {} in tube",
                    simulation.particles.len(),
                    simulation.total_moles(),
                    regions.passage_markers,
                ),
                region_stats("Hot", regions.left),
                region_stats("Cold", regions.right),
                format!(
                    "Crank: {crank_mode}{crank_status} | {:+.2} rpm | {:+.2} turns",
                    pistons.angular_velocity() * 60.0 / std::f32::consts::TAU,
                    pistons.revolutions(),
                ),
                format!(
                    "Heat: hot {:+.2} J | cold {:+.2} J",
                    simulation.energy_joules(heat.hot),
                    simulation.energy_joules(heat.cold),
                ),
                format!(
                    "Gas work: {:+.2} J | flywheel {:.2} J",
                    simulation.energy_joules(simulation.piston_work_by_gas()),
                    simulation.energy_joules(pistons.kinetic_energy()),
                ),
                if pistons.mode() == MotionMode::Free {
                    format!(
                        "Energy error: gas {:+.4} J | rotor {:+.4} J",
                        simulation.energy_joules(simulation.energy_balance_error()),
                        simulation.energy_joules(simulation.mechanical_balance_error()),
                    )
                } else {
                    format!(
                        "Energy error: gas {:+.4} J",
                        simulation.energy_joules(simulation.energy_balance_error()),
                    )
                },
            ];
            for (index, line) in stats.iter().enumerate() {
                draw_text(
                    line,
                    HUD_X,
                    STATS_START_Y + index as f32 * HUD_LINE_HEIGHT,
                    HUD_FONT_SIZE,
                    WHITE,
                );
            }
        }

        draw_text(
            "1–5: Fill  |  P: Pause  |  M: Mode  |  A/S: Kick  |  T: Reset  |  R: Camera",
            HUD_X,
            macroquad::prelude::screen_height() - HELP_BOTTOM_Y,
            HELP_FONT_SIZE,
            WHITE,
        );
        draw_buttons(
            show_stats,
            simulation.power_pistons().running(),
            simulation.power_pistons().mode(),
        );
        concentration_slider.draw();
        heater_slider.draw();
        next_frame().await;
    }
}

#[cfg(test)]
#[path = "../tests/unit/ui.rs"]
mod tests;
