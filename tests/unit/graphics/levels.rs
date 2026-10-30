//! Tests for easing the reservoir effects, `src/graphics/levels.rs`.

use super::*;

const FRAME: f64 = 1.0 / 60.0;

#[test]
fn starts_at_the_settings_then_eases_toward_new_ones() {
    let mut levels = ReservoirLevels::default();
    levels.follow(1_200.0, 100.0, 5.0);
    assert_eq!((levels.heat, levels.cold), (1.0, 1.0), "no fade at launch");

    // Both sliders back to 300 K: the effects shrink smoothly.
    let mut seconds = 5.0;
    let mut previous = levels.heat;
    let mut frames = 0;
    while levels.heat > 0.01 {
        seconds += FRAME;
        levels.follow(300.0, 300.0, seconds);
        let step = previous - levels.heat;
        assert!(step > 0.0 && step < 0.05, "a jump of {step}");
        assert_eq!(levels.heat, levels.cold);
        previous = levels.heat;
        frames += 1;
    }
    // Most of the way in under a second, all of it within about two.
    assert!((60..150).contains(&frames), "{frames} frames");
}

#[test]
fn ease_never_overshoots_and_a_stalled_frame_does_not_jump() {
    for seconds in [0.0, FRAME as f32, 0.5, 30.0] {
        let eased = ease(0.2, 0.8, seconds);
        assert!((0.2..=0.8).contains(&eased));
    }
    assert_eq!(ease(0.2, 0.8, 0.0), 0.2);
    assert!(
        ease(0.0, 1.0, 30.0) < 0.3,
        "a long stall moves at most one short step"
    );
}
