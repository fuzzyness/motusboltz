//! Button layout and label tests for `src/ui/controls.rs`.

use super::*;
use crate::ui::fonts::drawable;
use crate::ui::layout::{
    scene_left,
    scene_span, //
};

/// Window sizes the controls must fit: the default window, a school
/// laptop full-screen, and the canvas a browser leaves on 1366x768 and
/// 1280x720 screens.
const WINDOWS: [(f32, f32); 4] = [
    (1440.0, 900.0),
    (1366.0, 768.0),
    (1366.0, 620.0),
    (1280.0, 580.0),
];

/// Every combination of the states that change which buttons show.
fn all_states() -> Vec<ButtonStates> {
    let mut states = Vec::new();
    for bits in 0..4 {
        let on = |bit: u32| bits & (1 << bit) != 0;
        states.push(ButtonStates {
            start: on(0),
            panel: on(1),
            ..ButtonStates::ALL_ON
        });
        states.push(ButtonStates {
            start: on(0),
            panel: on(1),
            ..ButtonStates::default()
        });
    }
    states
}

#[test]
fn buttons_stay_in_the_free_scene_without_overlapping() {
    let pen = Pen::offline();
    for (width, height) in WINDOWS {
        for states in all_states() {
            let (left, free) = scene_span(width, states.panel);
            let edge = scene_left(states.panel);
            let area = SceneArea {
                edge,
                left,
                width: free,
                height,
            };
            let placed = place_buttons(&pen, area, states);
            for (index, button) in placed.iter().enumerate() {
                let rect = button.rect;
                assert!(rect.x >= edge - 1e-3, "{:?} off the left", button.action);
                assert!(
                    rect.right() <= left + free + 1e-3,
                    "{:?} off the right",
                    button.action
                );
                assert!(rect.y >= 0.0 && rect.bottom() <= height);
                for other in &placed[index + 1..] {
                    assert!(
                        !rect.overlaps(&other.rect),
                        "{:?} overlaps {:?} at {width}x{height}",
                        button.action,
                        other.action
                    );
                }
            }
        }
    }
}

/// The bottom row is centered on the window, so showing or hiding the
/// panel does not move it.
#[test]
fn bottom_row_stays_put_when_the_panel_is_hidden() {
    let pen = Pen::offline();
    for (width, height) in WINDOWS {
        let row = |panel: bool| {
            let (left, span) = scene_span(width, panel);
            let states = ButtonStates {
                panel,
                start: true,
                ..Default::default()
            };
            let area = SceneArea {
                edge: scene_left(panel),
                left,
                width: span,
                height,
            };
            place_buttons(&pen, area, states)
                .into_iter()
                .filter(|button| !TOGGLES.contains(&button.action))
                .map(|button| (button.action, button.rect.x, button.rect.y))
                .collect::<Vec<_>>()
        };
        assert_eq!(row(true), row(false), "at {width}x{height}");
    }
}

/// Start and Pause take turns in one place: exactly one of them shows, in
/// the same spot and at the same width.
#[test]
fn start_and_pause_share_one_place() {
    let pen = Pen::offline();
    let area = SceneArea {
        edge: 340.0,
        left: 340.0,
        width: 760.0,
        height: 900.0,
    };
    let shared = |states: ButtonStates| {
        let shown: Vec<Placed> = place_buttons(&pen, area, states)
            .into_iter()
            .filter(|button| SHARED.contains(&button.action))
            .collect();
        assert_eq!(shown.len(), 1, "{states:?}");
        (shown[0].action, shown[0].rect)
    };
    for base in [ButtonStates::default(), ButtonStates::ALL_ON] {
        let (start, start_rect) = shared(ButtonStates {
            start: true,
            ..base
        });
        let (pause, pause_rect) = shared(ButtonStates {
            start: false,
            ..base
        });
        assert_eq!((start, pause), (Action::Start, Action::TogglePause));
        assert_eq!(start_rect, pause_rect);
    }
}

#[test]
fn start_shows_only_when_offered() {
    let pen = Pen::offline();
    let area = SceneArea {
        edge: 340.0,
        left: 340.0,
        width: 760.0,
        height: 900.0,
    };
    let shown = |states: ButtonStates| {
        place_buttons(&pen, area, states)
            .iter()
            .any(|button| button.action == Action::Start)
    };
    assert!(shown(ButtonStates {
        start: true,
        ..Default::default()
    }));
    assert!(!shown(ButtonStates::default()));
}

#[test]
fn labels_name_their_keys_and_use_font_characters() {
    let off = ButtonStates::default();
    let on = ButtonStates::ALL_ON;
    assert_eq!(Action::Start.label(off).key, "Space");
    assert_eq!(Action::TogglePause.label(off).key, "Space");
    assert_eq!(Action::TogglePanel.label(off).text, "Show panel");
    assert_eq!(Action::TogglePanel.label(on).text, "Hide panel");
    assert_eq!(Action::TogglePause.label(on).text, "Resume");
    assert_eq!(Action::ToggleKeys.label(off).key, "H");
    for action in [
        Action::TogglePanel,
        Action::ToggleKeys,
        Action::ResetGas,
        Action::TogglePause,
        Action::ToggleSlow,
        Action::ToggleTracers,
        Action::Start,
    ] {
        for states in [off, on] {
            let label = action.label(states);
            assert!(drawable(label.text) && drawable(label.key), "{label:?}");
        }
    }
    assert!(drawable(NO_GAP_TIP));
}

/// A button keeps its width when its label changes (Pause to Resume), so
/// the row does not jump.
#[test]
fn buttons_keep_their_width_when_labels_change() {
    let pen = Pen::offline();
    let area = SceneArea {
        edge: 340.0,
        left: 340.0,
        width: 760.0,
        height: 900.0,
    };
    let widths = |states| {
        place_buttons(&pen, area, states)
            .iter()
            .map(|button| (button.action, button.rect.w))
            .collect::<Vec<_>>()
    };
    let paused = ButtonStates {
        paused: true,
        ..Default::default()
    };
    assert_eq!(widths(ButtonStates::default()), widths(paused));
}
