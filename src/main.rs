mod camera;
mod particle;
mod physics;
mod simulation;

use macroquad::prelude::*;

use camera::OrbitCamera;
use simulation::{CHAMBER_SIZE, PARTICLE_RADIUS, PRESSURE_WINDOW_SECONDS, Simulation};

const BACKGROUND_COLOR: Color = Color {
    r: 12.0 / 255.0,
    g: 18.0 / 255.0,
    b: 30.0 / 255.0,
    a: 1.0,
};

const CHAMBER_COLOR: Color = SKYBLUE;
const PARTICLE_COLOR: Color = ORANGE;
const TEXT_COLOR: Color = WHITE;

const TEXT_MARGIN_X: f32 = 20.0;
const TITLE_POSITION_Y: f32 = 35.0;
const METRICS_POSITION_Y: f32 = 70.0;
const METRICS_LINE_SPACING: f32 = 30.0;

const TITLE_FONT_SIZE: f32 = 30.0;
const METRICS_FONT_SIZE: f32 = 24.0;

const ENERGY_PRECISION: usize = 2;
const TEMPERATURE_PRECISION: usize = 3;
const PRESSURE_PRECISION: usize = 3;

#[macroquad::main("motusboltz")]
async fn main() {
    let mut simulation = Simulation::new();
    let mut camera = OrbitCamera::new();

    loop {
        simulation.update(get_frame_time());
        camera.update();

        clear_background(BACKGROUND_COLOR);

        set_camera(&camera.camera());

        draw_cube_wires(Vec3::ZERO, Vec3::splat(CHAMBER_SIZE), CHAMBER_COLOR);

        for particle in &simulation.particles {
            draw_sphere(particle.position, PARTICLE_RADIUS, None, PARTICLE_COLOR);
        }

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
                "Pressure ({} s avg.): {:.*}",
                PRESSURE_WINDOW_SECONDS, PRESSURE_PRECISION, pressure,
            ),
            None => format!(
                "Pressure ({} s avg.): measuring...",
                PRESSURE_WINDOW_SECONDS,
            ),
        };

        let metrics = [
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

        for (line, metric) in metrics.iter().enumerate() {
            draw_text(
                metric,
                TEXT_MARGIN_X,
                METRICS_POSITION_Y + line as f32 * METRICS_LINE_SPACING,
                METRICS_FONT_SIZE,
                TEXT_COLOR,
            );
        }

        next_frame().await;
    }
}
