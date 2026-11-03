//! Tests for the full-screen button, `src/ui/full_screen.rs`.

use super::*;

/// A button for the tests.
const RECT: Rect = Rect {
    x: 100.0,
    y: 50.0,
    w: BUTTON_SIZE,
    h: BUTTON_SIZE,
};

/// In a window, each corner's arms run back toward the middle from its
/// tip; full screen, away from it; either way the mark stays inside the
/// button.
#[test]
fn the_corners_point_out_then_in() {
    let middle = RECT.center();
    for (full, arms_nearer) in [(false, true), (true, false)] {
        for [first, tip, last] in corners(RECT, full) {
            for end in [first, last] {
                assert_eq!(end.distance(middle) < tip.distance(middle), arms_nearer);
                assert!(RECT.contains(end) && RECT.contains(tip));
            }
        }
    }
}
