//! Optional on-screen simulation diagnostics and shortcut help.

use std::fmt::Write;

use macroquad::prelude::{Color, WHITE, draw_text, screen_height, set_default_camera};

use crate::performance::PerformanceMonitor;
use crate::simulation::{PARTICLE_COUNT_PRESETS, PRESSURE_WINDOW_SECONDS, Simulation};

const TEXT_COLOR: Color = WHITE;

const PRESET_HELP_LABEL: &str = "Particle presets:";
const ENERGY_COLOR_HELP: &str =
    "Particle energy: blue = low, orange = medium, red = high | Flash = collision";
const ENERGY_COLOR_HELP_LINE_SPACING: f32 = 28.0;

const TEXT_MARGIN_X: f32 = 20.0;
const TITLE_POSITION_Y: f32 = 35.0;
const METRICS_POSITION_Y: f32 = 70.0;
const METRICS_LINE_SPACING: f32 = 24.0;
const PERFORMANCE_SECTION_SPACING: f32 = 15.0;
const HELP_BOTTOM_MARGIN: f32 = 20.0;

const TITLE_FONT_SIZE: f32 = 30.0;
const METRICS_FONT_SIZE: f32 = 20.0;
const HELP_FONT_SIZE: f32 = 20.0;

const ENERGY_PRECISION: usize = 2;
const TEMPERATURE_PRECISION: usize = 3;
const PRESSURE_PRECISION: usize = 3;
const RADIUS_PRECISION: usize = 3;
const VOLUME_PRECISION: usize = 2;
const PISTON_SPEED_PRECISION: usize = 2;
const FPS_PRECISION: usize = 1;
const PHYSICS_MS_PRECISION: usize = 3;
const CHECKS_PRECISION: usize = 1;

/// Formats a pressure reading while the next sampling window is incomplete.
fn format_pressure(label: &str, pressure: Option<f32>) -> String {
    match pressure {
        Some(value) => format!("{}: {:.*}", label, PRESSURE_PRECISION, value),
        None => format!("{label}: measuring..."),
    }
}

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

/// Diagnostic overlay, hidden until requested by the user.
pub struct Diagnostics {
    preset_help: String,
    visible: bool,
}

impl Diagnostics {
    /// Constructs the shortcut legend from the actual particle-count presets.
    pub fn new() -> Self {
        let mut preset_help = String::from(PRESET_HELP_LABEL);
        for (index, count) in PARTICLE_COUNT_PRESETS.iter().enumerate() {
            write!(&mut preset_help, " [{}] {}", index + 1, count)
                .expect("writing to a String cannot fail");
        }

        Self {
            preset_help,
            visible: false,
        }
    }

    /// Switches between the uncluttered view and detailed measurements.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    /// Draws the title and, if requested, the pressure and performance data.
    pub fn draw(&self, simulation: &Simulation, performance: &PerformanceMonitor) {
        // Return to screen-space coordinates for diagnostic text.
        set_default_camera();
        draw_text(
            "MotusBoltz",
            TEXT_MARGIN_X,
            TITLE_POSITION_Y,
            TITLE_FONT_SIZE,
            TEXT_COLOR,
        );

        // Detailed diagnostics and shortcut help remain hidden until requested.
        if self.visible {
            let pressures = simulation.pressures();
            let total_pressure_label = format!("Pressure ({}s avg.)", PRESSURE_WINDOW_SECONDS);

            let mechanism_mode = if simulation.crankshaft().is_some() {
                "Crankshaft (M toggle, Space starter)"
            } else {
                "Manual piston (M for crankshaft)"
            };
            let metrics = [
                format!("Mechanism: {mechanism_mode}"),
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
                format!("Volume: {:.*}", VOLUME_PRECISION, simulation.volume()),
                format!(
                    "Piston velocity: {:.*}",
                    PISTON_SPEED_PRECISION,
                    simulation.piston_velocity(),
                ),
                format!(
                    "Piston work by gas: {:.*}",
                    ENERGY_PRECISION,
                    simulation.piston_work_by_gas()
                ),
                format!(
                    "Flywheel energy: {:.*}",
                    ENERGY_PRECISION,
                    simulation
                        .crankshaft()
                        .map_or(0.0, |crank| crank.kinetic_energy()),
                ),
                format!(
                    "Crank speed: {:.*} rad/s",
                    PISTON_SPEED_PRECISION,
                    simulation
                        .crankshaft()
                        .map_or(0.0, |crank| crank.angular_velocity()),
                ),
                format!(
                    "Starter motor work: {:.*}",
                    ENERGY_PRECISION,
                    simulation
                        .crankshaft()
                        .map_or(0.0, |crank| crank.starter_work()),
                ),
                format!(
                    "Fan/bearing load energy: {:.*}",
                    ENERGY_PRECISION,
                    simulation
                        .crankshaft()
                        .map_or(0.0, |crank| crank.load_energy()),
                ),
                format_pressure(&total_pressure_label, pressures.map(|p| p.total)),
                format_pressure("Side-wall pressure", pressures.map(|p| p.side)),
                format_pressure("Top-cap pressure", pressures.map(|p| p.top)),
                format_pressure("Bottom-cap pressure", pressures.map(|p| p.bottom)),
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
            draw_text(
                ENERGY_COLOR_HELP,
                TEXT_MARGIN_X,
                screen_height() - HELP_BOTTOM_MARGIN - ENERGY_COLOR_HELP_LINE_SPACING,
                HELP_FONT_SIZE,
                TEXT_COLOR,
            );
            draw_text(
                &self.preset_help,
                TEXT_MARGIN_X,
                screen_height() - HELP_BOTTOM_MARGIN,
                HELP_FONT_SIZE,
                TEXT_COLOR,
            );
        }
    }
}
