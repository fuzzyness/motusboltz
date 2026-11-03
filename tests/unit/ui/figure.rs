//! Tests for the first-person figure, `src/ui/figure.rs`.

use super::*;

/// Moving the figure sideways swings its feet back behind it, never past
/// the widest swing, and it hangs straight again once the pointer stops.
#[test]
fn the_figure_swings_behind_and_settles() {
    let dt = 1.0 / 60.0;
    let mut drag = Drag::new(vec2(100.0, 100.0));
    for step in 1..=20 {
        drag.follow(vec2(100.0 + 8.0 * step as f32, 100.0), dt);
        assert!(drag.swing.abs() <= MAX_SWING);
    }
    assert!(drag.swing > 0.1, "moving right swings the feet back, left");
    for _ in 0..300 {
        drag.follow(drag.pointer, dt);
    }
    assert!(drag.swing.abs() < 1e-3, "{}", drag.swing);
    assert_eq!(drag.pointer, vec2(260.0, 100.0));
}

/// The button sits left of Hide all, level with it, without touching it.
#[test]
fn the_button_sits_left_of_hide_all() {
    let pen = Pen::offline();
    let area = SceneArea {
        edge: 0.0,
        left: 0.0,
        width: 1440.0,
        window_width: 1440.0,
        height: 900.0,
        sliders: true,
    };
    let spot = button_rect(&pen, area);
    let corner = corner_button(&pen, area);
    assert!(spot.right() < corner.x);
    assert!((spot.center().y - corner.center().y).abs() < 1e-4);
    assert!(spot.bottom() < area.height);
}
