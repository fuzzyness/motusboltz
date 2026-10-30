//! Button layout and label tests for `src/ui/controls.rs`.

use super::*;
use crate::ui::layout::CONTROL_PADDING_X;
use crate::ui::slider::{
    HOT_Y,
    PANEL_PADDING_TOP, //
};

const SCREEN_WIDTHS: [f32; 3] = [1280.0, 1366.0, 1440.0];

#[test]
fn buttons_do_not_overlap_each_other_or_the_sliders() {
    let slider_top = HOT_Y - PANEL_PADDING_TOP;
    for width in SCREEN_WIDTHS {
        for index in 0..BUTTON_ACTIONS.len() {
            let button = button_rect(index, width);
            assert!(button.bottom() + BUTTON_GAP <= slider_top);
            assert!(button.x >= control_left(width) - CONTROL_PADDING_X);
            assert!(button.right() <= width);
            for other in index + 1..BUTTON_ACTIONS.len() {
                assert!(!button.overlaps(&button_rect(other, width)));
            }
        }
    }
}

#[test]
fn start_and_details_buttons_name_their_new_keys() {
    assert_eq!(BUTTON_ACTIONS[2], Action::Start);
    let off = ButtonStates::default();
    assert_eq!(Action::Start.label(off), "Start (Enter)");
    assert_eq!(BUTTON_ACTIONS[3], Action::ToggleDetails);
    assert_eq!(Action::ToggleDetails.label(off), "Details (I)");
    assert_eq!(
        Action::ToggleDetails.label(ButtonStates::ALL_ON),
        "Less (I)"
    );
    assert_eq!(Action::ToggleSlow.label(off), "Slow (L)");
    assert_eq!(Action::ToggleSlow.label(ButtonStates::ALL_ON), "Normal (L)");
    assert_eq!(Action::ToggleTracers.label(off), "Tracers (F)");
}

/// Every label fits its button at the shared size and is ASCII, since
/// Macroquad's built-in font is only relied on for ASCII.
#[test]
fn button_labels_share_a_size_that_fits_and_are_ascii() {
    let pen = Pen::offline();
    let size = button_font_size(&pen);
    assert!(size <= BUTTON_FONT_SIZE);
    for action in BUTTON_ACTIONS {
        for states in [ButtonStates::default(), ButtonStates::ALL_ON] {
            let label = action.label(states);
            assert!(label.is_ascii(), "{label}");
            let width = BUTTON_WIDTH - 2.0 * BUTTON_TEXT_PADDING;
            assert!(pen.measure(label, size) <= width + 1e-3, "{label}");
        }
    }
}
