//! Key-binding and wheel tests for `src/ui/input.rs`.

use super::*;

/// A predicate reporting exactly `keys` as held or pressed.
fn only(keys: &[KeyCode]) -> impl Fn(KeyCode) -> bool + '_ {
    move |key| keys.contains(&key)
}

#[test]
fn letters_and_arrows_orbit_and_shift_turns_them_into_moves() {
    for (keys, direction) in [
        ([KeyCode::W, KeyCode::Up], UP),
        ([KeyCode::S, KeyCode::Down], DOWN),
        ([KeyCode::A, KeyCode::Left], LEFT),
        ([KeyCode::D, KeyCode::Right], RIGHT),
    ] {
        for key in keys {
            let orbit = CameraKeys::from_held(only(&[key]));
            assert_eq!(orbit.orbit, direction, "{key:?}");
            assert_eq!(orbit.pan, Vec2::ZERO);
            for shift in SHIFT_KEYS {
                let pan = CameraKeys::from_held(only(&[key, shift]));
                assert_eq!(pan.pan, direction, "{shift:?}+{key:?}");
                assert_eq!(pan.orbit, Vec2::ZERO);
            }
        }
        // The letter and the arrow together count once.
        assert_eq!(CameraKeys::from_held(only(&keys)).orbit, direction);
    }
}

#[test]
fn opposite_keys_cancel_and_diagonals_combine() {
    assert_eq!(
        CameraKeys::from_held(only(&[KeyCode::A, KeyCode::D])).orbit,
        Vec2::ZERO
    );
    assert_eq!(
        CameraKeys::from_held(only(&[KeyCode::W, KeyCode::Right])).orbit,
        Vec2::new(1.0, 1.0)
    );
    assert_eq!(CameraKeys::from_held(only(&[])), CameraKeys::default());
}

#[test]
fn minus_and_equal_zoom_out_and_in() {
    for key in ZOOM_IN_KEYS {
        assert_eq!(CameraKeys::from_held(only(&[key])).zoom, 1.0);
    }
    for key in ZOOM_OUT_KEYS {
        assert_eq!(CameraKeys::from_held(only(&[key])).zoom, -1.0);
    }
    let both = CameraKeys::from_held(only(&[KeyCode::Minus, KeyCode::Equal]));
    assert_eq!(both.zoom, 0.0);
}

#[test]
fn command_keys_map_to_their_commands() {
    assert!(KeyCommands::from_pressed(only(&[KeyCode::Enter])).start);
    assert!(KeyCommands::from_pressed(only(&[KeyCode::KpEnter])).start);
    assert!(KeyCommands::from_pressed(only(&[KeyCode::I])).details);
    assert!(KeyCommands::from_pressed(only(&[KeyCode::Space])).pause);
    assert!(KeyCommands::from_pressed(only(&[KeyCode::T])).reset_gas);
    assert!(KeyCommands::from_pressed(only(&[KeyCode::G])).panel);
    assert!(KeyCommands::from_pressed(only(&[KeyCode::R])).camera_reset);
    assert!(KeyCommands::from_pressed(only(&[KeyCode::K])).kick);
    assert!(KeyCommands::from_pressed(only(&[KeyCode::L])).slow);
    assert!(KeyCommands::from_pressed(only(&[KeyCode::F])).tracers);
    for (index, key) in FILL_KEYS.into_iter().enumerate() {
        assert_eq!(
            KeyCommands::from_pressed(only(&[key])).fill_preset,
            Some(index)
        );
    }
    // The camera keys are not commands: A and D no longer start the engine
    // or toggle details.
    for (key, _) in CAMERA_DIRECTION_KEYS {
        assert_eq!(
            KeyCommands::from_pressed(only(&[key])),
            KeyCommands::default()
        );
    }
}

#[test]
fn help_lists_the_bindings_in_ascii_without_the_hidden_kick() {
    for line in HELP_LINES {
        assert!(line.is_ascii(), "{line}");
        assert!(!line.contains("K:"));
    }
    let help = HELP_LINES.join(" ");
    for name in ["Enter", "Space", "I:", "WASD", "Shift", "-/=", "R:"] {
        assert!(help.contains(name), "{name} missing from help");
    }
}

#[test]
fn wheel_readings_become_bounded_pixels() {
    assert_eq!(normalized_wheel(0.0), 0.0);
    assert!(normalized_wheel(1.0) > 0.0);
    assert!(normalized_wheel(-1.0) < 0.0);
    assert_eq!(normalized_wheel(f32::MAX), MAX_WHEEL_PIXELS);
    assert_eq!(normalized_wheel(-f32::MAX), -MAX_WHEEL_PIXELS);
}
