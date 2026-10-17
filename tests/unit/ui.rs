//! Button layout and stats formatting regression tests.
use super::*;

#[test]
fn button_rows_do_not_overlap_sliders() {
    let button_bottom =
        CONTROL_TOP + (BUTTON_ROWS - 1) as f32 * (BUTTON_HEIGHT + BUTTON_GAP) + BUTTON_HEIGHT;
    let slider_top = crate::slider::HEATER_Y - crate::slider::PANEL_PADDING_TOP;
    assert!(button_bottom + BUTTON_GAP < slider_top);
}

#[test]
fn two_column_buttons_do_not_overlap() {
    for index in 0..BUTTON_ACTIONS.len() {
        let (x, y, width) = button_rect(index, 800.0);
        assert!(width > 0.0);
        for other in index + 1..BUTTON_ACTIONS.len() {
            let (ox, oy, owidth) = button_rect(other, 800.0);
            assert!(
                x + width <= ox
                    || ox + owidth <= x
                    || y + BUTTON_HEIGHT <= oy
                    || oy + BUTTON_HEIGHT <= y
            );
        }
    }
}

#[test]
fn buttons_have_one_action_per_slot() {
    assert_eq!(BUTTON_ACTIONS.len(), BUTTON_ROWS * 2);
    assert_ne!(BUTTON_ACTIONS[2], BUTTON_ACTIONS[3]);
}
