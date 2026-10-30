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
    assert!(KeyCommands::from_pressed(only(&[KeyCode::I])).details);
    assert!(KeyCommands::from_pressed(only(&[KeyCode::Space])).go);
    // Space now starts the engine; Enter does nothing.
    assert_eq!(
        KeyCommands::from_pressed(only(&[KeyCode::Enter])),
        KeyCommands::default()
    );
    assert!(KeyCommands::from_pressed(only(&[KeyCode::T])).reset_gas);
    assert!(KeyCommands::from_pressed(only(&[KeyCode::G])).panel);
    assert!(KeyCommands::from_pressed(only(&[KeyCode::R])).camera_reset);
    assert!(KeyCommands::from_pressed(only(&[KeyCode::K])).kick);
    assert!(KeyCommands::from_pressed(only(&[KeyCode::L])).slow);
    assert!(KeyCommands::from_pressed(only(&[KeyCode::F])).tracers);
    assert!(KeyCommands::from_pressed(only(&[KeyCode::H])).keys);
    assert!(KeyCommands::from_pressed(only(&[KeyCode::C])).sliders);
    assert!(KeyCommands::from_pressed(only(&[KeyCode::U])).hide_all);
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

/// The Keys list names every binding, in ASCII, without the hidden kick.
#[test]
fn shortcuts_list_the_bindings_in_ascii_without_the_hidden_kick() {
    for (keys, action) in SHORTCUTS {
        assert!(keys.is_ascii() && action.is_ascii(), "{keys}: {action}");
        assert_ne!(keys, "K");
    }
    let listed: Vec<&str> = SHORTCUTS.iter().map(|&(keys, _)| keys).collect();
    for keys in [
        "Space", "L", "F", "T", "1 to 5", "I", "G", "C", "U", "H", "R",
    ] {
        assert!(listed.contains(&keys), "{keys} missing from the list");
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
