//! Tests for `src/ui/shortcuts.rs`: the keys list fits where it drops down.

use super::*;

#[test]
fn list_fits_the_smallest_scene_and_draws_every_binding() {
    // The free part of the scene on a 1280x580 browser canvas with the
    // panel shown is about 630 points wide.
    const SMALL_SCENE: (f32, f32) = (630.0, 580.0);
    let mut pen = Pen::offline();
    let (width, height) = draw(
        &mut pen,
        0.0,
        46.0,
        SMALL_SCENE.0 - 32.0,
        SMALL_SCENE.1 - 140.0,
    );
    assert!(width <= SMALL_SCENE.0 - 32.0 + 1e-3);
    assert!(height <= SMALL_SCENE.1 - 140.0 + 1e-3);
    assert_eq!(extent(&pen, 1.0).0, width, "no shrinking needed");
    for (keys, action) in SHORTCUTS {
        assert!(pen.texts.iter().any(|text| text == keys));
        assert!(pen.texts.iter().any(|text| text == action));
    }
}
