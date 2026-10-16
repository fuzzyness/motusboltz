//! Entry point and on-screen controls for the MotusBoltz simulation.

mod camera;
mod crankshaft;
mod diagnostics;
mod mechanism_render;
mod particle;
mod performance;
mod physics;
mod piston;
mod render;
mod simulation;
mod slider;
mod spatial_grid;

use macroquad::prelude::{
    Color, KeyCode, SKYBLUE, clear_background, get_frame_time, get_time, is_key_pressed,
    next_frame, set_camera,
};

use camera::OrbitCamera;
use diagnostics::Diagnostics;
use mechanism_render::{CrankVisual, CylinderFrame, PistonVisual};
use performance::PerformanceMonitor;
use render::{
    CylinderDimensions, DRAW_CALL_INDEX_CAPACITY, DRAW_CALL_VERTEX_CAPACITY, ParticleRenderer,
};
use simulation::{
    CHAMBER_HEIGHT,
    CHAMBER_RADIUS,
    PARTICLE_COUNT_PRESETS,
    Simulation, //
};
use slider::{ControlAction, ControlButtons, Slider, VOLUME_PERCENT_SCALE};

const BACKGROUND_COLOR: Color = Color {
    r: 12.0 / 255.0,
    g: 18.0 / 255.0,
    b: 30.0 / 255.0,
    a: 1.0,
};

const CHAMBER_COLOR: Color = SKYBLUE;
const CHAMBER_DIMENSIONS: CylinderDimensions = CylinderDimensions {
    radius: CHAMBER_RADIUS,
    height: CHAMBER_HEIGHT,
};

const PARTICLE_PRESET_KEYS: [KeyCode; PARTICLE_COUNT_PRESETS.len()] = [
    KeyCode::Key1,
    KeyCode::Key2,
    KeyCode::Key3,
    KeyCode::Key4,
    KeyCode::Key5,
];

/// Restarts the gas at a new count while preserving its selected temperature.
fn change_particle_count(
    simulation: &mut Simulation,
    count: usize,
    temperature: f32,
    performance: &mut PerformanceMonitor,
) {
    if simulation.particles.len() == count {
        return;
    }

    simulation.set_particle_count(count);
    simulation.set_temperature(temperature);
    *performance = PerformanceMonitor::new();
}

fn window_conf() -> macroquad::conf::Conf {
    let mut config = macroquad::conf::Conf::default();
    config.miniquad_conf.window_title = "motusboltz".to_owned();
    config.draw_call_vertex_capacity = DRAW_CALL_VERTEX_CAPACITY;
    config.draw_call_index_capacity = DRAW_CALL_INDEX_CAPACITY;
    config
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut simulation = Simulation::new();
    let mut camera = OrbitCamera::new();
    let mut performance = PerformanceMonitor::new();
    let mut particle_renderer = ParticleRenderer::new();
    let cylinder_frame = CylinderFrame::new(CHAMBER_DIMENSIONS, CHAMBER_COLOR);
    let mut piston_visual = PistonVisual::new(CHAMBER_RADIUS);
    let mut crank_visual = CrankVisual::new();
    let mut temperature_slider = Slider::temperature(simulation.temperature());
    let mut particle_slider = Slider::particles(simulation.particles.len());
    let mut volume_slider = Slider::volume(VOLUME_PERCENT_SCALE);
    let mut control_buttons = ControlButtons::default();
    let mut diagnostics = Diagnostics::new();

    loop {
        // Buttons and their keyboard shortcuts perform the same actions.
        let button_action = control_buttons.update(simulation.crankshaft().is_some());
        if is_key_pressed(KeyCode::M) || button_action == Some(ControlAction::SwitchMode) {
            simulation.toggle_crankshaft();
            // A newly disengaged crank leaves the manual piston at its current
            // height. Reset the slider's pending input to that same height.
            volume_slider.set_value(simulation.requested_volume_fraction() * VOLUME_PERCENT_SCALE);
        }
        if is_key_pressed(KeyCode::Space) || button_action == Some(ControlAction::StartCrank) {
            simulation.start_crankshaft();
        }
        if button_action == Some(ControlAction::ToggleStats) {
            diagnostics.toggle();
        }

        for (&key, &count) in PARTICLE_PRESET_KEYS
            .iter()
            .zip(PARTICLE_COUNT_PRESETS.iter())
        {
            if is_key_pressed(key) {
                if simulation.particles.len() != count {
                    change_particle_count(
                        &mut simulation,
                        count,
                        temperature_slider.value(),
                        &mut performance,
                    );
                }
                particle_slider.set_value(count as f32);
                if simulation.crankshaft().is_none() {
                    volume_slider
                        .set_value(simulation.requested_volume_fraction() * VOLUME_PERCENT_SCALE);
                }
                break;
            }
        }

        // The temperature slider applies immediately, as before.
        if temperature_slider.update().changed {
            simulation.set_temperature(temperature_slider.value());
        }

        // Rebuild the gas only when a particle-count drag is released.
        // Rebuilding for every pixel would allocate meshes and reset pressure
        // samples repeatedly, particularly for large particle counts.
        if particle_slider.update().released {
            change_particle_count(
                &mut simulation,
                particle_slider.value() as usize,
                temperature_slider.value(),
                &mut performance,
            );
            if simulation.crankshaft().is_none() {
                volume_slider
                    .set_value(simulation.requested_volume_fraction() * VOLUME_PERCENT_SCALE);
            }
        }

        // The manual volume control is hidden and inactive in crank mode.
        if simulation.crankshaft().is_none() && volume_slider.update().released {
            simulation.set_volume_fraction(volume_slider.value() / VOLUME_PERCENT_SCALE);
        }

        let frame_time = get_frame_time();
        // Time only the simulation update, excluding rendering and UI work.
        let physics_start = get_time();
        let update_stats = simulation.update(frame_time);
        let physics_duration = get_time() - physics_start;
        performance.record_frame(frame_time, physics_duration, update_stats);

        camera.update(
            !(temperature_slider.blocks_camera()
                || particle_slider.blocks_camera()
                || (simulation.crankshaft().is_none() && volume_slider.blocks_camera())
                || control_buttons.blocks_camera()),
        );
        let view_camera = camera.camera();
        clear_background(BACKGROUND_COLOR);
        set_camera(&view_camera);

        cylinder_frame.draw();
        particle_renderer.draw(
            &simulation.particles,
            simulation.collision_flashes(),
            simulation.particle_radius(),
            &view_camera,
            CHAMBER_DIMENSIONS,
            BACKGROUND_COLOR,
        );
        piston_visual.draw(simulation.piston_position());
        if let Some(crank) = simulation.crankshaft() {
            crank_visual.draw(crank);
        }

        diagnostics.draw(&simulation, &performance);
        temperature_slider.draw();
        particle_slider.draw();
        if simulation.crankshaft().is_none() {
            volume_slider.draw();
        }
        control_buttons.draw(simulation.crankshaft().is_some(), diagnostics.visible());

        next_frame().await;
    }
}
