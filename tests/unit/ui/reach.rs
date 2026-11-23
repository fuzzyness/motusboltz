//! Tests for what the figure can use, `src/ui/reach.rs`.

use macroquad::prelude::vec3;

use super::*;
use crate::ui::fonts::tests::drawable;

/// Two things ahead, one a little off the line of sight.
fn ahead() -> [Usable; 2] {
    [
        Usable {
            handle: Handle::ResetButton,
            point: vec3(0.0, 0.0, -2.0),
            radius: 0.5,
        },
        Usable {
            handle: Handle::Clock,
            point: vec3(0.3, 0.0, -2.0),
            radius: 0.5,
        },
    ]
}

/// The line of sight finds what it passes nearest the middle of, within
/// reach and in front; off to the side, too far or behind, nothing.
#[test]
fn the_line_of_sight_finds_the_nearest_within_reach() {
    let eye = Vec3::ZERO;
    let forward = -Vec3::Z;
    assert_eq!(
        aimed(eye, forward, &ahead(), 3.0),
        Some(Handle::ResetButton)
    );
    let leaning = vec3(0.15, 0.0, -1.0).normalize();
    assert_eq!(aimed(eye, leaning, &ahead(), 3.0), Some(Handle::Clock));
    assert_eq!(aimed(eye, Vec3::X, &ahead(), 3.0), None, "to the side");
    assert_eq!(aimed(eye, forward, &ahead(), 1.0), None, "out of reach");
    assert_eq!(aimed(eye, Vec3::Z, &ahead(), 3.0), None, "behind");
}

/// Every caption says what E does, and can be drawn.
#[test]
fn every_caption_says_what_e_does_and_can_be_drawn() {
    use crate::sim::reservoir::ReservoirSide;
    let handles = [
        Handle::Crank,
        Handle::Valve(ReservoirSide::Hot),
        Handle::Valve(ReservoirSide::Cold),
        Handle::TankKnob,
        Handle::Clock,
        Handle::ResetButton,
        Handle::TracersButton,
        Handle::Cartridge,
    ];
    for handle in handles {
        for state in [CartridgeState::Lying, CartridgeState::Seated] {
            let text = caption(handle, state);
            assert!(text.contains(" E"), "{text}");
            assert!(drawable(text), "{text}");
        }
        assert!(drawable(using_caption(handle)));
    }
    assert_ne!(
        caption(Handle::Cartridge, CartridgeState::Lying),
        caption(Handle::Cartridge, CartridgeState::Seated)
    );
}
