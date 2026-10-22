//! Validate readings and sampling without a graphics context.
use super::*;
use crate::engine::{DEFAULT_GEOMETRY, KickDirection};
use crate::gas::MIN_CONNECTED_MARKERS;
const SEED: u64 = 42;
const FRAME_DT: f32 = 1.0 / 60.0;

#[test]
fn snapshot_counts_all_particles_and_reports_pressure() {
    let mut engine =
        ConnectedChambers::with_geometry_seed(MIN_CONNECTED_MARKERS, DEFAULT_GEOMETRY, SEED);
    engine.kick_flywheel(KickDirection::Negative);
    engine.update(FRAME_DT);
    let sample = Sample::capture(&engine);
    let measurements = engine.measurements();
    assert_eq!(
        sample.particles.iter().sum::<usize>(),
        MIN_CONNECTED_MARKERS
    );
    assert!(sample.particles.iter().all(|&count| count > 0));
    assert_eq!(
        sample.pressure,
        [measurements.hot, measurements.cold].map(|r| r.pressure_pa.map(|p| p / PASCALS_PER_BAR))
    );
}

#[test]
fn samples_pause_and_reset_with_simulation_and_use_independent_scales() {
    let mut engine =
        ConnectedChambers::with_geometry_seed(MIN_CONNECTED_MARKERS, DEFAULT_GEOMETRY, SEED);
    let mut charts = LiveCharts::new();
    charts.observe(&engine);
    engine.update(FRAME_DT);
    charts.observe(&engine);
    assert_eq!(charts.sample.unwrap().time, 0.0);
    const FRAMES: usize = 20;
    for _ in 0..FRAMES {
        engine.update(FRAME_DT);
    }
    charts.observe(&engine);
    let time = charts.sample.unwrap().time;
    assert!(time > 0.0);
    charts.observe(&engine);
    assert_eq!(charts.sample.unwrap().time, time);
    assert_eq!(charts.particle_scale, MIN_CONNECTED_MARKERS as f64);
    assert!(charts.pressure_scale.is_finite() && charts.pressure_scale > 0.0);
    engine.reset();
    charts.observe(&engine);
    assert_eq!(charts.sample.unwrap().time, 0.0);
    charts.clear();
    assert!(charts.sample.is_none());
}

#[test]
fn panels_fit_and_intercept_camera_input() {
    const WINDOW: Vec2 = Vec2::new(1440.0, 900.0);
    let mut charts = LiveCharts::new();
    charts.layout(WINDOW);
    assert!(charts.panels[0].right() < charts.panels[1].x);
    for panel in charts.panels {
        assert!(panel.x >= 0.0 && panel.y >= 0.0);
        assert!(panel.right() <= WINDOW.x && panel.bottom() <= WINDOW.y);
        assert!(charts.blocks_camera(panel.center()));
    }
    assert!(!charts.blocks_camera(Vec2::ZERO));
}

/// Macroquad's built-in font is only relied on for ASCII.
#[test]
fn chart_text_is_ascii() {
    let mut engine =
        ConnectedChambers::with_geometry_seed(MIN_CONNECTED_MARKERS, DEFAULT_GEOMETRY, SEED);
    engine.update(FRAME_DT);
    let mut charts = LiveCharts::new();
    charts.observe(&engine);
    let fixed = PANEL_TITLES
        .iter()
        .chain(&PRESSURE_LABELS)
        .chain(&PARTICLE_LABELS)
        .chain(&[PRESSURE_FOOTNOTE, PARTICLE_FOOTNOTE])
        .copied();
    let formatted = charts
        .captions
        .iter()
        .chain(&charts.pressure_values)
        .chain(&charts.particle_values)
        .map(String::as_str);
    for text in fixed.chain(formatted) {
        assert!(text.is_ascii(), "non-ASCII chart text: {text:?}");
    }
}
