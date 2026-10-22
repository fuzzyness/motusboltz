//! Bar charts of the latest sampled readings: cylinder pressures and particle
//! populations. Only one sample is stored, so memory use stays constant.
use crate::engine::ConnectedChambers;
use macroquad::prelude::*;

/// Minimum model time between samples; paused time produces no new samples.
const SAMPLE_INTERVAL: f64 = 0.1;
const PASCALS_PER_BAR: f64 = 100_000.0;
const PANEL_HEIGHT: f32 = 230.0;
const BOTTOM_MARGIN: f32 = 40.0;
const MARGIN: f32 = 12.0;
const PANEL_GAP: f32 = 10.0;
const PLOT_TOP: f32 = 65.0;
const PLOT_HEIGHT: f32 = 108.0;
const BAR_WIDTH_RATIO: f32 = 0.42;
const TITLE_BASELINE: f32 = 23.0;
const TITLE_FONT_SIZE: f32 = 19.0;
const VALUE_FONT_SIZE: f32 = 16.0;
const LABEL_FONT_SIZE: f32 = 15.0;
const CAPTION_FONT_SIZE: f32 = 12.0;
const COLUMN_GAP: f32 = 6.0;
const VALUE_GAP: f32 = 9.0;
const LABEL_BASELINE: f32 = 18.0;
const CAPTION_BOTTOM: f32 = 22.0;
const FOOTNOTE_BOTTOM: f32 = 6.0;
const SCIENTIFIC_LOWER: f64 = 0.01;
const SCIENTIFIC_UPPER: f64 = 10_000.0;
const RANGE_PADDING: f64 = 1.1;
const HOT: Color = Color::from_rgba(255, 160, 90, 255);
const COLD: Color = Color::from_rgba(95, 190, 255, 255);
const PASSAGE: Color = Color::from_rgba(180, 224, 209, 255);
const PANEL: Color = Color::from_rgba(19, 29, 43, 250);
const GRID: Color = Color::from_rgba(52, 65, 81, 255);
const TEXT: Color = Color::from_rgba(215, 226, 238, 255);

const PRESSURE_PANEL: usize = 0;
const PARTICLE_PANEL: usize = 1;
const PANEL_TITLES: [&str; 2] = [
    "Cylinder pressures",
    "Particles in each chamber and passage",
];
const PRESSURE_LABELS: [&str; 2] = ["Hot pressure", "Cold pressure"];
const PARTICLE_LABELS: [&str; 3] = ["Hot cylinder", "Cold cylinder", "U passage"];
const PRESSURE_FOOTNOTE: &str = "Ideal-gas estimate nRT/V from each cylinder's particles";
const PARTICLE_FOOTNOTE: &str = "Counts include every simulated particle";

#[derive(Clone, Copy, Debug)]
struct Sample {
    time: f64,
    pressure: [Option<f64>; 2],
    particles: [usize; 3],
}
impl Sample {
    fn capture(engine: &ConnectedChambers) -> Self {
        let regions = engine.measurements();
        Self {
            time: engine.elapsed_time(),
            pressure: [regions.hot, regions.cold]
                .map(|r| r.pressure_pa.map(|p| p / PASCALS_PER_BAR)),
            particles: [
                regions.hot.markers,
                regions.cold.markers,
                regions.passage_markers,
            ],
        }
    }
}

/// Rounds a reading up to one significant digit after 10% headroom, e.g.
/// 12.3 becomes 20. The scale is recomputed at every sample, so it follows
/// the current reading rather than holding a fixed range. Hot and cold
/// pressure always share one scale.
fn ceiling(value: f64) -> f64 {
    if !value.is_finite() || value <= 0.0 {
        return 1.0;
    }
    let magnitude = 10.0_f64.powf(value.log10().floor());
    (value * RANGE_PADDING / magnitude).ceil() * magnitude
}
fn value_text(value: f64) -> String {
    if value != 0.0 && !(SCIENTIFIC_LOWER..SCIENTIFIC_UPPER).contains(&value.abs()) {
        format!("{value:.1e}")
    } else {
        format!("{value:.2}")
    }
}
/// Draws text left-aligned at `x`, shrinking it only if it would exceed `width`.
fn fitted_text(text: &str, x: f32, y: f32, width: f32, size: f32, color: Color) {
    let measured = measure_text(text, None, size as u16, 1.0).width;
    draw_text(
        text,
        x,
        y,
        size * (width / measured.max(1.0)).min(1.0),
        color,
    );
}

pub struct LiveCharts {
    sample: Option<Sample>,
    panels: [Rect; 2],
    // Text is formatted only when a new sample is taken, not every frame.
    pressure_values: [String; 2],
    particle_values: [String; 3],
    pressure_scale: f64,
    particle_scale: f64,
    captions: [String; 2],
}
impl LiveCharts {
    pub fn reserved_height() -> f32 {
        PANEL_HEIGHT + BOTTOM_MARGIN
    }
    pub fn new() -> Self {
        Self {
            sample: None,
            panels: [Rect::default(); 2],
            pressure_values: std::array::from_fn(|_| "--".into()),
            particle_values: std::array::from_fn(|_| "--".into()),
            pressure_scale: 1.0,
            particle_scale: 1.0,
            captions: std::array::from_fn(|_| String::new()),
        }
    }
    pub fn clear(&mut self) {
        *self = Self::new();
    }
    /// Takes a new sample once at least `SAMPLE_INTERVAL` of model time has
    /// passed, or immediately after the simulation's clock moves backward.
    pub fn observe(&mut self, engine: &ConnectedChambers) {
        let time = engine.elapsed_time();
        if self
            .sample
            .is_some_and(|last| time >= last.time && time - last.time < SAMPLE_INTERVAL)
        {
            return;
        }
        let sample = Sample::capture(engine);
        let total_particles = sample.particles.iter().sum::<usize>();
        self.pressure_scale = ceiling(
            sample
                .pressure
                .into_iter()
                .flatten()
                .fold(0.0_f64, f64::max),
        );
        self.particle_scale = total_particles.max(1) as f64;
        self.pressure_values = sample
            .pressure
            .map(|p| p.map_or_else(|| "--".into(), |p| format!("{} bar", value_text(p))));
        self.particle_values = sample.particles.map(|count| count.to_string());
        self.captions = [
            format!(
                "Pressure scale: 0 to {} bar (both cylinders)",
                value_text(self.pressure_scale)
            ),
            format!(
                "Particle scale: 0 to {} | total {total_particles}",
                self.particle_scale
            ),
        ];
        self.sample = Some(sample);
    }
    fn layout(&mut self, size: Vec2) {
        let width = (size.x - 2.0 * MARGIN - PANEL_GAP) / 2.0;
        for (index, panel) in self.panels.iter_mut().enumerate() {
            *panel = Rect::new(
                MARGIN + index as f32 * (width + PANEL_GAP),
                size.y - Self::reserved_height(),
                width,
                PANEL_HEIGHT,
            );
        }
    }
    pub fn blocks_camera(&self, pointer: Vec2) -> bool {
        self.panels.iter().any(|panel| panel.contains(pointer))
    }
    pub fn draw(&mut self) {
        self.layout(vec2(screen_width(), screen_height()));
        let Some(sample) = self.sample else {
            return;
        };
        let pressure_panel = self.panels[PRESSURE_PANEL];
        let particle_panel = self.panels[PARTICLE_PANEL];
        draw_panel_frame(pressure_panel, PANEL_TITLES[PRESSURE_PANEL]);
        for (bar, label) in PRESSURE_LABELS.iter().enumerate() {
            draw_bar(
                pressure_panel,
                bar,
                PRESSURE_LABELS.len(),
                sample.pressure[bar].unwrap_or(0.0) / self.pressure_scale,
                [HOT, COLD][bar],
                &self.pressure_values[bar],
                label,
            );
        }
        draw_panel_notes(pressure_panel, &self.captions[0], PRESSURE_FOOTNOTE);

        draw_panel_frame(particle_panel, PANEL_TITLES[PARTICLE_PANEL]);
        for (bar, label) in PARTICLE_LABELS.iter().enumerate() {
            draw_bar(
                particle_panel,
                bar,
                PARTICLE_LABELS.len(),
                sample.particles[bar] as f64 / self.particle_scale,
                [HOT, COLD, PASSAGE][bar],
                &self.particle_values[bar],
                label,
            );
        }
        draw_panel_notes(particle_panel, &self.captions[1], PARTICLE_FOOTNOTE);
    }
}

fn draw_panel_frame(panel: Rect, title: &str) {
    draw_rectangle(panel.x, panel.y, panel.w, panel.h, PANEL);
    fitted_text(
        title,
        panel.x + MARGIN,
        panel.y + TITLE_BASELINE,
        panel.w - 2.0 * MARGIN,
        TITLE_FONT_SIZE,
        TEXT,
    );
}

/// Draws one bar in an evenly divided slot; `fraction` is clamped to 0..=1.
fn draw_bar(
    panel: Rect,
    bar: usize,
    bar_count: usize,
    fraction: f64,
    color: Color,
    value: &str,
    label: &str,
) {
    let slot = (panel.w - 2.0 * MARGIN) / bar_count as f32;
    let x = panel.x + MARGIN + bar as f32 * slot;
    let base = panel.y + PLOT_TOP + PLOT_HEIGHT;
    let width = slot * BAR_WIDTH_RATIO;
    let height = PLOT_HEIGHT * fraction.clamp(0.0, 1.0) as f32;
    let bar_x = x + (slot - width) / 2.0;
    draw_rectangle(bar_x, panel.y + PLOT_TOP, width, PLOT_HEIGHT, GRID);
    draw_rectangle(bar_x, base - height, width, height, color);
    fitted_text(
        value,
        x,
        panel.y + PLOT_TOP - VALUE_GAP,
        slot - COLUMN_GAP,
        VALUE_FONT_SIZE,
        TEXT,
    );
    fitted_text(
        label,
        x,
        base + LABEL_BASELINE,
        slot - COLUMN_GAP,
        LABEL_FONT_SIZE,
        TEXT,
    );
}

fn draw_panel_notes(panel: Rect, caption: &str, footnote: &str) {
    for (text, offset) in [(caption, CAPTION_BOTTOM), (footnote, FOOTNOTE_BOTTOM)] {
        fitted_text(
            text,
            panel.x + MARGIN,
            panel.y + panel.h - offset,
            panel.w - 2.0 * MARGIN,
            CAPTION_FONT_SIZE,
            TEXT,
        );
    }
}

#[cfg(test)]
#[path = "../tests/unit/charts.rs"]
mod tests;
