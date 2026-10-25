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
fn scene_sits_between_the_panel_and_the_controls() {
    for (width, _) in WINDOWS {
        let (left, scene) = scene_span(width, true);
        assert_eq!(left, PANEL_WIDTH);
        assert!(left + scene <= control_left(width) - CONTROL_PADDING_X + 1e-3);
        let (hidden_left, wider) = scene_span(width, false);
        assert_eq!(hidden_left, 0.0);
        assert!(wider > scene);
    }
}

#[test]
fn scene_viewport_counts_physical_pixels() {
    const RETINA_SCALE: f32 = 2.0;
    for (width, height) in WINDOWS {
        let (left, scene) = scene_span(width, true);
        let standard = scene_viewport(width, height, true, 1.0);
        assert_eq!(standard, (left as i32, 0, scene as i32, height as i32));
        let (x, y, w, h) = scene_viewport(width, height, true, RETINA_SCALE);
        assert_eq!((x, y), (2 * standard.0, 0));
        assert!((w - 2 * standard.2).abs() <= 1 && (h - 2 * standard.3).abs() <= 1);
    }
}
