//! Slider bounds and value-quantization tests for `src/ui/slider.rs`.

use super::*;

#[test]
fn hot_kelvin_slider_is_clamped_and_quantized() {
    const TRACK_X: f32 = 100.0;
    let slider = Slider::hot_temperature(MIN_HOT_KELVIN);
    assert_eq!(
        (slider.setting.label, slider.setting.unit),
        ("Hot side", "K")
    );
    assert_eq!(slider.value_text(1_200.0), "1200 K");
    assert_eq!(slider.value_at_x(0.0, TRACK_X), MIN_HOT_KELVIN);
    assert_eq!(
        slider.value_at_x(TRACK_X + SLIDER_WIDTH, TRACK_X),
        MAX_HOT_KELVIN
    );
    assert_eq!(
        slider.value_at_x(TRACK_X + SLIDER_WIDTH / 2.0, TRACK_X),
        (MIN_HOT_KELVIN + MAX_HOT_KELVIN) / 2.0
    );
    const FRACTIONAL_KELVIN: f32 = 501.4;
    let fraction = (FRACTIONAL_KELVIN - MIN_HOT_KELVIN) / (MAX_HOT_KELVIN - MIN_HOT_KELVIN);
    assert_eq!(
        slider.value_at_x(TRACK_X + fraction * SLIDER_WIDTH, TRACK_X),
        501.0
    );
}

#[test]
fn concentration_slider_matches_gas_range() {
    let mut slider = Slider::concentration(0.5);
    assert!((slider.value() - 0.5).abs() < 1e-6);
    assert_eq!(slider.value_at_x(0.0, 100.0), MIN_CONCENTRATION);
    assert_eq!(
        slider.value_at_x(100.0 + SLIDER_WIDTH * 2.0, 100.0),
        MAX_CONCENTRATION
    );
    slider.set_value(20.0);
    assert_eq!(slider.value(), MAX_CONCENTRATION);
}

#[test]
fn cold_slider_reaches_both_temperature_limits() {
    const TRACK_X: f32 = 100.0;
    let slider = Slider::cold_temperature(MAX_COLD_KELVIN);
    assert_eq!(slider.value_at_x(TRACK_X, TRACK_X), MIN_COLD_KELVIN);
    assert_eq!(
        slider.value_at_x(TRACK_X + SLIDER_WIDTH, TRACK_X),
        MAX_COLD_KELVIN
    );
    assert_eq!(slider.value(), crate::sim::units::KELVIN_PER_UNIT as f32);
}

/// The switch starts with its knob where it is, and set from elsewhere it
/// changes at once while the knob is left to slide over.
#[test]
fn the_switch_turns_at_once_and_its_knob_follows() {
    let switch = Switch::regenerator(false);
    assert!(!switch.is_on());
    assert_eq!(switch.slide, 0.0);
    let mut switch = Switch::regenerator(true);
    assert_eq!(switch.slide, 1.0);
    switch.set_on(false);
    assert!(!switch.is_on());
    assert_eq!(switch.slide, 1.0, "the knob slides over when animated");
}

/// The switch's row sits under the helium slider and ends the column.
#[test]
fn the_switch_row_ends_the_column_under_the_helium_slider() {
    let (top, bottom) = column_span();
    assert_eq!(top, HOT_Y - AREA_ABOVE);
    const { assert!(SWITCH_Y - SWITCH_HIT_HALF_HEIGHT >= CONCENTRATION_Y + AREA_BELOW) };
    assert_eq!(bottom, SWITCH_Y + SWITCH_HIT_HALF_HEIGHT);
}

#[test]
fn mixing_runs_from_one_color_to_the_other() {
    const TOLERANCE: f32 = 1e-6;
    let same = |a: Color, b: Color| {
        [a.r - b.r, a.g - b.g, a.b - b.b, a.a - b.a]
            .iter()
            .all(|difference| difference.abs() < TOLERANCE)
    };
    assert!(same(mixed(TRACK_COLOR, SWITCH_COLOR, 0.0), TRACK_COLOR));
    assert!(same(mixed(TRACK_COLOR, SWITCH_COLOR, 1.0), SWITCH_COLOR));
}
