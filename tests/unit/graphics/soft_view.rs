//! Tests for the softened view's size, `src/graphics/soft_view.rs`.

use super::*;

#[test]
fn view_is_softer_than_the_window_but_keeps_its_shape() {
    // A Retina Mac's window: 85% of its height in points.
    assert_eq!(pixels(1440.0, 900.0, 2.0), (1224, 765));
    // The X230's browser canvas: no fewer than 600 lines.
    assert_eq!(pixels(1366.0, 620.0, 1.0), (1322, 600));
    // A small screen with fewer lines than that is drawn at its own size.
    assert_eq!(pixels(800.0, 400.0, 1.0), (800, 400));
    for (width, height, dpi) in [
        (1440.0, 900.0, 2.0),
        (1366.0, 620.0, 1.0),
        (844.0, 390.0, 3.0),
    ] {
        let (x, y) = pixels(width, height, dpi);
        assert!(((x as f32 / y as f32) - width / height).abs() < 0.01);
        assert!(y as f32 <= height * dpi);
    }
}
