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

/// Every setting on, with everything shown.
const ALL_ON_SHOWN: ButtonStates = ButtonStates {
    hidden: false,
    ..ButtonStates::ALL_ON
};

/// The area for a window, with the panel and the sliders open or closed.
fn area_for(width: f32, height: f32, panel: bool, sliders: bool) -> SceneArea {
    let (left, span) = scene_span(width, panel, sliders);
    SceneArea {
        edge: scene_left(panel),
        left,
        width: span,
        window_width: width,
        height,
    }
}

/// Every combination of the states that change which buttons show.
fn all_states() -> Vec<ButtonStates> {
    let mut states = Vec::new();
    for bits in 0..8 {
        let on = |bit: u32| bits & (1 << bit) != 0;
        for base in [ALL_ON_SHOWN, ButtonStates::default()] {
            states.push(ButtonStates {
                start: on(0),
                panel: on(1),
                sliders: on(2),
                ..base
            });
        }
    }
    states
}

#[test]
fn buttons_stay_in_the_window_without_overlapping() {
    let pen = Pen::offline();
    for (width, height) in WINDOWS {
        for states in all_states() {
            let area = area_for(width, height, states.panel, states.sliders);
            let placed = place_buttons(&pen, area, states);
            for (index, button) in placed.iter().enumerate() {
                let rect = button.rect;
                assert!(rect.x >= -1e-3, "{:?} off the left", button.action);
                assert!(
                    rect.right() <= width + 1e-3,
                    "{:?} off the right",
                    button.action
                );
                assert!(rect.y >= 0.0 && rect.bottom() <= height);
                for other in &placed[index + 1..] {
                    assert!(
                        !rect.overlaps(&other.rect),
                        "{:?} overlaps {:?} at {width}x{height} in {states:?}",
                        button.action,
                        other.action
                    );
                }
            }
            // The pills in the middle stay clear of the open cards.
            for button in placed.iter().filter(|b| b.shape == Shape::Pill) {
                if button.action == Action::ToggleAll {
                    continue;
                }
                if states.panel {
                    assert!(!button.rect.overlaps(&panel_card(height)));
                }
                if states.sliders {
                    assert!(!button.rect.overlaps(&slider_column(width)));
                }
            }
        }
    }
}

/// The toggles and the bottom row are centered on the window, so opening
/// or closing either drawer does not move them.
#[test]
fn centered_buttons_stay_put_when_drawers_close() {
    let pen = Pen::offline();
    let centered = |action: Action| TOGGLES.contains(&action) || ROW_ACTIONS.contains(&action);
    for (width, height) in WINDOWS {
        let place = |panel: bool, sliders: bool| {
            let states = ButtonStates {
                panel,
                sliders,
                start: true,
                ..Default::default()
            };
            place_buttons(&pen, area_for(width, height, panel, sliders), states)
                .into_iter()
                .filter(|button| centered(button.action))
                .map(|button| (button.action, button.rect.center()))
                .collect::<Vec<_>>()
        };
        let open = place(true, true);
        assert_eq!(open.len(), TOGGLES.len() + ROW_ACTIONS.len() - 1);
        // Each group stays centered on the window.
        for group in [&TOGGLES[..], &ROW_ACTIONS[..]] {
            let xs: Vec<f32> = open
                .iter()
                .filter(|(action, _)| group.contains(action))
                .map(|(_, center)| center.x)
                .collect();
            let middle = (xs.iter().cloned().fold(f32::MAX, f32::min)
                + xs.iter().cloned().fold(f32::MIN, f32::max))
                / 2.0;
            assert!((middle - width / 2.0).abs() < 60.0, "off center");
        }
        // The toggles do not move at all, and nor does the row while it
        // keeps its size.
        let toggles = |placed: &[(Action, Vec2)]| {
            placed
                .iter()
                .filter(|(action, _)| TOGGLES.contains(action))
                .copied()
                .collect::<Vec<_>>()
        };
        for (panel, sliders) in [(false, true), (true, false), (false, false)] {
            assert_eq!(toggles(&open), toggles(&place(panel, sliders)));
        }
    }
}

/// Every action that can sit in the bottom row.
const ROW_ACTIONS: [Action; 5] = [
    Action::ResetGas,
    Action::Start,
    Action::TogglePause,
    Action::ToggleSlow,
    Action::ToggleTracers,
];

/// An open drawer shows its hump, beside its card; a closed one, its tab,
/// at the window's edge.
#[test]
fn drawers_show_a_hump_when_open_and_an_edge_tab_when_closed() {
    let pen = Pen::offline();
    let (width, height) = (1440.0, 900.0);
    for open in [true, false] {
        let states = ButtonStates {
            panel: open,
            sliders: open,
            ..Default::default()
        };
        let placed = place_buttons(&pen, area_for(width, height, open, open), states);
        let find = |action| {
            *placed
                .iter()
                .find(|button| button.action == action)
                .expect("drawer button")
        };
        let panel = find(Action::TogglePanel);
        let sliders = find(Action::ToggleSliders);
        if open {
            assert_eq!(panel.shape, Shape::Hump(Edge::Left));
            assert_eq!(sliders.shape, Shape::Hump(Edge::Right));
            assert!((panel.rect.x - panel_card(height).right()).abs() < 1e-3);
            assert!((sliders.rect.right() - slider_column(width).x).abs() < 1e-3);
        } else {
            assert_eq!(panel.shape, Shape::Tab(Edge::Left));
            assert_eq!(sliders.shape, Shape::Tab(Edge::Right));
            assert_eq!(panel.rect.x, 0.0);
            assert!((sliders.rect.right() - width).abs() < 1e-3);
        }
    }
}

/// With everything hidden, only Show all remains, in the same corner as
/// Hide all.
#[test]
fn hide_all_leaves_only_itself() {
    let pen = Pen::offline();
    for states in all_states() {
        let area = area_for(1440.0, 900.0, states.panel, states.sliders);
        let hidden = ButtonStates {
            hidden: true,
            ..states
        };
        let placed = place_buttons(&pen, area, hidden);
        assert_eq!(placed.len(), 1);
        assert_eq!(placed[0].action, Action::ToggleAll);
        assert_eq!(placed[0].label.text, "Show all");
        let shown = place_buttons(&pen, area, states);
        let corner = shown
            .iter()
            .find(|button| button.action == Action::ToggleAll)
            .expect("Hide all");
        assert_eq!(corner.label.text, "Hide all");
        assert_eq!(corner.rect, placed[0].rect);
    }
}

/// Start and Pause take turns in one place: exactly one of them shows, in
/// the same spot and at the same width.
#[test]
fn start_and_pause_share_one_place() {
    let pen = Pen::offline();
    let area = area_for(1440.0, 900.0, true, true);
    let shared = |states: ButtonStates| {
        let shown: Vec<Placed> = place_buttons(&pen, area, states)
            .into_iter()
            .filter(|button| SHARED.contains(&button.action))
            .collect();
        assert_eq!(shown.len(), 1, "{states:?}");
        (shown[0].action, shown[0].rect)
    };
    for base in [ButtonStates::default(), ALL_ON_SHOWN] {
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
fn labels_name_their_keys_and_use_font_characters() {
    let off = ButtonStates::default();
    let on = ButtonStates::ALL_ON;
    assert_eq!(Action::Start.label(off).key, "Space");
    assert_eq!(Action::TogglePause.label(off).key, "Space");
    assert_eq!(Action::TogglePanel.label(off).key, "G");
    assert_eq!(Action::ToggleSliders.label(off).key, "C");
    assert_eq!(Action::ToggleAll.label(off).text, "Hide all");
    assert_eq!(Action::ToggleAll.label(on).text, "Show all");
    assert_eq!(Action::ToggleAll.label(on).key, "U");
    assert_eq!(Action::TogglePause.label(on).text, "Resume");
    assert_eq!(Action::ToggleKeys.label(off).key, "H");
    for action in [
        Action::TogglePanel,
        Action::ToggleSliders,
        Action::ToggleKeys,
        Action::ToggleScenery,
        Action::ToggleAll,
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

/// A button keeps its width when its label changes (Pause to Resume, Hide
/// all to Show all), so nothing jumps.
#[test]
fn buttons_keep_their_width_when_labels_change() {
    let pen = Pen::offline();
    let area = area_for(1440.0, 900.0, true, true);
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
    let corner = |hidden| {
        place_buttons(
            &pen,
            area,
            ButtonStates {
                hidden,
                ..Default::default()
            },
        )
        .into_iter()
        .find(|button| button.action == Action::ToggleAll)
        .map(|button| button.rect.w)
    };
    assert_eq!(corner(false), corner(true));
}
