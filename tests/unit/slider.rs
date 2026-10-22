//! Slider bounds and value-quantization tests.
use super::*;

#[test]
fn hot_kelvin_slider_is_clamped_and_quantized() {
    const TRACK_X: f32 = 100.0;
    let slider = Slider::hot_temperature(MIN_HOT_KELVIN);
    assert_eq!(slider.label, "Hot reservoir (K)");
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
    assert_eq!(
        slider.value_at_x(0.0, 100.0),
        MIN_INITIAL_CONCENTRATION_MOL_PER_L
    );
    assert_eq!(
        slider.value_at_x(100.0 + SLIDER_WIDTH * 2.0, 100.0),
        MAX_INITIAL_CONCENTRATION_MOL_PER_L
    );
    slider.set_value(20.0);
    assert_eq!(slider.value(), MAX_INITIAL_CONCENTRATION_MOL_PER_L);
}

#[test]
fn cold_slider_reaches_both_temperature_limits() {
    const TRACK_X: f32 = 100.0;
    let slider = Slider::cold_temperature(crate::gas::MAX_COLD_KELVIN);
    assert_eq!(
        slider.value_at_x(TRACK_X, TRACK_X),
        crate::gas::MIN_COLD_KELVIN
    );
    assert_eq!(
        slider.value_at_x(TRACK_X + SLIDER_WIDTH, TRACK_X),
        crate::gas::MAX_COLD_KELVIN
    );
    assert_eq!(slider.value(), crate::gas::KELVIN_PER_UNIT as f32);
}
