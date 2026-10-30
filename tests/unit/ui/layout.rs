//! Screen-layout tests for `src/ui/layout.rs`.

use super::*;

/// Window sizes the scene must fit: the default window, a common school
/// laptop full-screen, a small window, and the canvas a browser leaves on
/// 1366x768 and 1280x720 screens after its toolbars.
const WINDOWS: [(f32, f32); 5] = [
    (1440.0, 900.0),
    (1366.0, 768.0),
    (1280.0, 720.0),
    (1366.0, 620.0),
    (1280.0, 580.0),
];

#[test]
fn span_is_centered_and_clear_of_the_panel_and_the_sliders() {
    for (width, _) in WINDOWS {
        for show_panel in [true, false] {
            for show_sliders in [true, false] {
                let (left, span) = scene_span(width, show_panel, show_sliders);
                assert!((left + span / 2.0 - width / 2.0).abs() < 1e-3, "off center");
                assert!(left >= scene_left(show_panel));
                assert!(left >= EDGE_TAB_WIDTH);
                if show_sliders {
                    assert!(left + span <= control_left(width) - CONTROL_PADDING_X + 1e-3);
                }
            }
        }
        // Closing either drawer never narrows the span.
        let open = scene_span(width, true, true).1;
        assert!(scene_span(width, false, true).1 >= open);
        assert!(scene_span(width, true, false).1 >= open);
        assert!(scene_span(width, false, false).1 >= open);
    }
}
