//! Entry point and on-screen controls for the MotusBoltz simulation.

mod camera;
mod particle;
mod performance;
mod physics;
mod render;
mod simulation;
mod spatial_grid;
mod temperature_slider;

use std::fmt::Write;

use macroquad::prelude::{
    Color,
    KeyCode,
    SKYBLUE,
    WHITE,
    clear_background,
    draw_text,
    get_frame_time,
    get_time,
    is_key_pressed,
    next_frame,
    screen_height,
    set_camera,
    set_default_camera, //
};

use camera::OrbitCamera;
use performance::PerformanceMonitor;
use render::{
    CylinderDimensions,
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY,
    ParticleRenderer,
    draw_cylinder_chamber, //
};
use simulation::{
    CHAMBER_HEIGHT,
    CHAMBER_RADIUS,
    PARTICLE_COUNT_PRESETS,
    PRESSURE_WINDOW_SECONDS,
    Simulation, //
};
use temperature_slider::TemperatureSlider;

const BACKGROUND_COLOR: Color = Color {
    r: 12.0 / 255.0,
    g: 18.0 / 255.0,
    b: 30.0 / 255.0,
    a: 1.0,
};

const CHAMBER_COLOR: Color = SKYBLUE;
const TEXT_COLOR: Color = WHITE;

const PARTICLE_PRESET_KEYS: [KeyCode; PARTICLE_COUNT_PRESETS.len()] = [
    KeyCode::Key1,
    KeyCode::Key2,
    KeyCode::Key3,
    KeyCode::Key4,
    KeyCode::Key5,
];

const PRESET_HELP_LABEL: &str = "Particle presets:";
const ENERGY_COLOR_HELP: &str =
    "Particle energy: blue = low, orange = medium, red = high | Flash = collision";
const ENERGY_COLOR_HELP_LINE_SPACING: f32 = 28.0;

const TEXT_MARGIN_X: f32 = 20.0;
const TITLE_POSITION_Y: f32 = 35.0;
const METRICS_POSITION_Y: f32 = 70.0;
const METRICS_LINE_SPACING: f32 = 30.0;
const PERFORMANCE_SECTION_SPACING: f32 = 15.0;
const HELP_BOTTOM_MARGIN: f32 = 20.0;

const TITLE_FONT_SIZE: f32 = 30.0;
const METRICS_FONT_SIZE: f32 = 24.0;
const HELP_FONT_SIZE: f32 = 20.0;

const ENERGY_PRECISION: usize = 2;
const TEMPERATURE_PRECISION: usize = 3;
const PRESSURE_PRECISION: usize = 3;
const RADIUS_PRECISION: usize = 3;
const FPS_PRECISION: usize = 1;
const PHYSICS_MS_PRECISION: usize = 3;
const CHECKS_PRECISION: usize = 1;

fn draw_metric_lines(lines: &[String], start_y: f32) {
    for (line, value) in lines.iter().enumerate() {
        draw_text(
            value,
            TEXT_MARGIN_X,
            start_y + line as f32 * METRICS_LINE_SPACING,
            METRICS_FONT_SIZE,
            TEXT_COLOR,
        );
    }
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
    let mut temperature_slider = TemperatureSlider::new(simulation.temperature());

    // Generate the shortcut legend from the same presets used for input.
    let mut preset_help = String::from(PRESET_HELP_LABEL);
    for (index, count) in PARTICLE_COUNT_PRESETS.iter().enumerate() {
        write!(&mut preset_help, " [{}] {}", index + 1, count)
            .expect("writing to a String cannot fail");
    }

    loop {
        for (&key, &count) in PARTICLE_PRESET_KEYS
            .iter()
            .zip(PARTICLE_COUNT_PRESETS.iter())
        {
            if is_key_pressed(key) {
                if simulation.particles.len() != count {
                    simulation.set_particle_count(count);
                    // Keep the selected temperature across particle presets.
                    simulation.set_temperature(temperature_slider.value());
                    performance = PerformanceMonitor::new();
                }
                break;
            }
        }

        // Slider gestures change kinetic energy without orbiting the camera.
        if let Some(temperature) = temperature_slider.update() {
            simulation.set_temperature(temperature);
        }

        let frame_time = get_frame_time();
        // Time only the simulation update, excluding rendering and UI work.
        let physics_start = get_time();
        let update_stats = simulation.update(frame_time);
        let physics_duration = get_time() - physics_start;
        performance.record_frame(frame_time, physics_duration, update_stats);

        camera.update(!temperature_slider.blocks_camera());
        let view_camera = camera.camera();
        clear_background(BACKGROUND_COLOR);
        set_camera(&view_camera);

        draw_cylinder_chamber(CHAMBER_RADIUS, CHAMBER_HEIGHT, CHAMBER_COLOR);
        particle_renderer.draw(
            &simulation.particles,
            simulation.collision_flashes(),
            simulation.particle_radius(),
            &view_camera,
            CylinderDimensions {
                radius: CHAMBER_RADIUS,
                height: CHAMBER_HEIGHT,
            },
            BACKGROUND_COLOR,
        );

        // Return to screen-space coordinates for diagnostic text.
        set_default_camera();
        draw_text(
            "MotusBoltz",
            TEXT_MARGIN_X,
            TITLE_POSITION_Y,
            TITLE_FONT_SIZE,
            TEXT_COLOR,
        );

        let pressure_label = match simulation.pressure() {
            Some(pressure) => format!(
                "Pressure ({}s avg.): {:.*}",
                PRESSURE_WINDOW_SECONDS, PRESSURE_PRECISION, pressure,
            ),
            None => format!("Pressure ({}s avg.): measuring...", PRESSURE_WINDOW_SECONDS,),
        };

        let metrics = [
            format!("Particles: {}", simulation.particles.len()),
            format!(
                "Particle radius: {:.*}",
                RADIUS_PRECISION,
                simulation.particle_radius(),
            ),
            format!(
                "Kinetic energy: {:.*}",
                ENERGY_PRECISION,
                simulation.kinetic_energy(),
            ),
            format!(
                "Temperature: {:.*}",
                TEMPERATURE_PRECISION,
                simulation.temperature(),
            ),
            pressure_label,
        ];

        draw_metric_lines(&metrics, METRICS_POSITION_Y);

        if let Some(sample) = performance.sample() {
            let performance_y = METRICS_POSITION_Y
                + metrics.len() as f32 * METRICS_LINE_SPACING
                + PERFORMANCE_SECTION_SPACING;
            let performance_metrics = [
                format!("FPS: {:.*}", FPS_PRECISION, sample.fps),
                format!(
                    "Physics: {:.*} ms/frame",
                    PHYSICS_MS_PRECISION, sample.physics_ms_per_frame,
                ),
                format!("Physics steps/s: {:.0}", sample.physics_steps_per_second),
                format!(
                    "Collision checks/step: {:.*}",
                    CHECKS_PRECISION, sample.candidate_checks_per_step,
                ),
            ];

            draw_metric_lines(&performance_metrics, performance_y);
        }

        temperature_slider.draw();

        draw_text(
            ENERGY_COLOR_HELP,
            TEXT_MARGIN_X,
            screen_height() - HELP_BOTTOM_MARGIN - ENERGY_COLOR_HELP_LINE_SPACING,
            HELP_FONT_SIZE,
            TEXT_COLOR,
        );
        draw_text(
            &preset_help,
            TEXT_MARGIN_X,
            screen_height() - HELP_BOTTOM_MARGIN,
            HELP_FONT_SIZE,
            TEXT_COLOR,
        );

        next_frame().await;
    }
}
