//! Tests for `src/ui/button.rs`: a press sinks a button and springs it back.

use super::*;

/// A press starts at its rest before, sinks all the way, and ends at its
/// rest after: a toggle switched on clicks in and stays partway down.
#[test]
fn a_press_sinks_and_springs_back_to_its_new_rest() {
    assert_eq!(press_depth(0.0, 0.0, LATCH_DEPTH), 0.0);
    let deepest = (0..=100)
        .map(|step| press_depth(step as f32 * PRESS_SECONDS / 100.0, 0.0, LATCH_DEPTH))
        .fold(0.0, f32::max);
    assert!((deepest - 1.0).abs() < 1e-3);
    assert!((press_depth(PRESS_SECONDS, 0.0, LATCH_DEPTH) - LATCH_DEPTH).abs() < 1e-6);
    assert_eq!(press_depth(PRESS_SECONDS, LATCH_DEPTH, 0.0), 0.0);
}

#[test]
fn a_sunk_button_drops_and_narrows_inside_its_place() {
    let rect = Rect::new(10.0, 20.0, 100.0, 30.0);
    assert_eq!(sunk(rect, 0.0), rect);
    let pressed = sunk(rect, 1.0);
    assert!(pressed.y > rect.y && pressed.w < rect.w && pressed.h < rect.h);
    assert!(pressed.x > rect.x && pressed.right() < rect.right());
    assert!(pressed.bottom() <= rect.bottom() + 1.0);
}
