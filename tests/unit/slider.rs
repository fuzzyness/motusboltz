//! Slider bounds and value-quantization tests.
use super::*;

#[test]
fn heater_slider_is_clamped_and_quantized() {
    let slider = Slider::demo_heater(0.0);
    assert_eq!(slider.value_at_x(100.0 - 40.0, 100.0), 0.0);
    assert_eq!(slider.value_at_x(100.0 + SLIDER_WIDTH + 40.0, 100.0), 100.0);
    assert_eq!(slider.value_at_x(100.0 + SLIDER_WIDTH / 2.0, 100.0), 50.0);
}

#[test]
fn concentration_slider_matches_gas_range() {
    let mut slider = Slider::demo_concentration(0.5);
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
