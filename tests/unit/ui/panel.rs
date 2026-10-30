//! Panel layout, view-switch and scrolling tests for `src/ui/panel.rs`,
//! without a graphics context.

use super::*;
use crate::sim::cycle::TurnSummary;
use crate::sim::geometry::DEFAULT_GEOMETRY;
use crate::sim::units::DEFAULT_CONNECTED_MARKERS;
use crate::ui::fonts::drawable;
use plot::LOOP_KEY;
use readout::{
    WAITING_GAP,
    WAITING_START,
    WAITING_TURN, //
};
use simple::{
    LAW_FIRST,
    LAW_SECOND,
    LAW_THIRD, //
};

const SEED: u64 = 42;
/// Canvas heights to check: a 1440x900 window, a 1366x768 screen used
/// full-screen, and the canvas a browser leaves on a 1366x768 and a
/// 1280x720 laptop after its toolbars.
const DEFAULT_HEIGHT: f32 = 900.0;
const LAPTOP_HEIGHT: f32 = 768.0;
const BROWSER_HEIGHTS: [f32; 2] = [620.0, 580.0];

fn engine() -> ConnectedChambers {
    ConnectedChambers::with_geometry_seed(DEFAULT_CONNECTED_MARKERS, DEFAULT_GEOMETRY, SEED)
}

/// A settled heated turn, in reduced units, that balances exactly (the same
/// as in the readout tests).
fn heated_turn() -> TurnSummary {
    TurnSummary {
        turns: 3,
        hot_heat: 3.0 * 9_000.0,
        cold_heat: 3.0 * -8_600.0,
        hot_entropy: 3.0 * 9_000.0 / 4.0,
        cold_entropy: 3.0 * -8_600.0,
        gas_work: 3.0 * 300.0,
        gas_energy_change: 3.0 * 100.0,
        duration: 3.0 * 80.0,
        hot_kelvin: 1_200.0,
        cold_kelvin: 300.0,
    }
}

/// A panel holding readings and a P-V range, as after a few turns, in
/// `mode`.
fn filled_panel(engine: &ConnectedChambers, mode: PanelMode) -> EnginePanel {
    let mut panel = EnginePanel::new();
    panel.select(mode);
    panel.refresh(engine);
    panel.readout.turn = Some(readout::turn(engine, heated_turn()));
    panel.range = Some(PlotRange {
        volume: engine.cylinder_volume_range_cm3(),
        pressure: (20.0, 70.0),
    });
    panel
}

/// Lays the panel out without a window and returns the P-V plot side and
/// every string it would have drawn.
fn layout(panel: &mut EnginePanel, engine: &ConnectedChambers, height: f32) -> (f32, Vec<String>) {
    let mut pen = Pen::offline();
    let side = panel.draw_with_height(&mut pen, engine, None, height);
    (side, pen.texts)
}

/// A press of the left button at `position`, with nothing else happening.
fn press(position: Vec2) -> PanelPointer {
    PanelPointer {
        position,
        wheel_pixels: 0.0,
        pressed: true,
        down: true,
    }
}

#[test]
fn view_switch_fits_the_header_without_overlapping() {
    for index in 0..MODES.len() {
        let tab = tab_rect(index);
        assert!(tab.x >= MARGIN && tab.right() <= PANEL_WIDTH - MARGIN + 1e-3);
        assert!(tab.bottom() <= HEADER_HEIGHT);
        if index + 1 < MODES.len() {
            assert!(tab.right() < tab_rect(index + 1).x);
        }
    }
}

#[test]
fn clicking_a_view_selects_it_at_the_top_and_the_key_toggles() {
    let engine = engine();
    let mut panel = filled_panel(&engine, PanelMode::Advanced);
    layout(&mut panel, &engine, BROWSER_HEIGHTS[0]);
    panel.scroll_to(f32::MAX);
    assert!(panel.scroll > 0.0);
    for (index, mode) in MODES.into_iter().enumerate() {
        assert!(panel.handle_pointer(press(tab_rect(index).center())));
        assert_eq!(panel.mode, mode);
        assert_eq!(panel.scroll, 0.0);
    }
    assert!(panel.advanced());
    panel.toggle_mode();
    assert!(!panel.advanced());
}

/// The Simple view, the one students see first, fits a laptop screen
/// without scrolling and shows every law with the engine's numbers.
#[test]
fn simple_view_fits_a_laptop_and_covers_every_law() {
    let engine = engine();
    let mut panel = filled_panel(&engine, PanelMode::Simple);
    let (side, texts) = layout(&mut panel, &engine, LAPTOP_HEIGHT);
    assert_eq!(side, 0.0, "no P-V plot in the Simple view");
    assert_eq!(
        panel.max_scroll(),
        0.0,
        "Simple view scrolls at {LAPTOP_HEIGHT}"
    );
    for expected in [
        LAW_FIRST,
        LAW_SECOND,
        LAW_THIRD,
        "the system",
        "surroundings",
    ] {
        assert!(
            texts.iter().any(|text| text == expected),
            "{expected} not drawn"
        );
    }
    // The ledger's numbers, in joules, appear as drawn pieces.
    let turn = panel.readout.turn.unwrap();
    let heat_in = readout::amount(turn.hot_heat);
    assert!(texts.contains(&heat_in));
    assert!(texts.iter().any(|text| text.starts_with("ΔS universe = +")));
}

#[test]
fn advanced_view_keeps_a_comfortable_plot_and_scrolls_on_short_windows() {
    let engine = engine();
    let mut panel = filled_panel(&engine, PanelMode::Advanced);
    let (side, texts) = layout(&mut panel, &engine, DEFAULT_HEIGHT);
    assert!(side >= COMFORTABLE_PLOT_SIDE);
    for (key, _) in LOOP_KEY {
        assert!(texts.iter().any(|text| text == key));
    }
    for height in BROWSER_HEIGHTS {
        let mut panel = filled_panel(&engine, PanelMode::Advanced);
        let (side, _) = layout(&mut panel, &engine, height);
        assert!(side >= COMFORTABLE_PLOT_SIDE, "plot shrank at {height}");
        assert!(panel.max_scroll() > 0.0, "nothing to scroll at {height}");
        // Scrolling moves the content but never resizes the plot.
        panel.scroll_to(f32::MAX);
        assert_eq!(layout(&mut panel, &engine, height).0, side);
        assert_eq!(panel.scroll, panel.max_scroll());
    }
}

#[test]
fn wheel_scrolls_only_over_the_panel_and_stays_in_range() {
    let engine = engine();
    let mut panel = filled_panel(&engine, PanelMode::Advanced);
    layout(&mut panel, &engine, BROWSER_HEIGHTS[1]);
    let wheel = |x: f32, pixels: f32| PanelPointer {
        position: Vec2::new(x, HEADER_HEIGHT + 10.0),
        wheel_pixels: pixels,
        pressed: false,
        down: false,
    };
    const STEP: f32 = 40.0;
    assert!(!panel.handle_pointer(wheel(PANEL_WIDTH + 1.0, -STEP)));
    assert_eq!(panel.scroll, 0.0);
    assert!(panel.handle_pointer(wheel(MARGIN, -STEP)));
    assert_eq!(panel.scroll, STEP);
    panel.handle_pointer(wheel(MARGIN, STEP * 10.0));
    assert_eq!(panel.scroll, 0.0);
    panel.handle_pointer(wheel(MARGIN, -f32::MAX));
    assert_eq!(panel.scroll, panel.max_scroll());
}

#[test]
fn scrollbar_drag_and_track_clicks_scroll_the_content() {
    let engine = engine();
    let mut panel = filled_panel(&engine, PanelMode::Advanced);
    layout(&mut panel, &engine, BROWSER_HEIGHTS[1]);
    let track = panel.scroll_track().expect("short window must scroll");
    let thumb = panel.thumb_rect().unwrap();
    assert!(thumb.h < track.h && thumb.y == track.y);

    // Drag the thumb to the bottom of the track.
    assert!(panel.handle_pointer(press(thumb.center())));
    let mut drag = press(Vec2::new(thumb.center().x, track.bottom() + 50.0));
    drag.pressed = false;
    // The gesture keeps the camera blocked even off the panel.
    drag.position.x = PANEL_WIDTH + 100.0;
    assert!(panel.handle_pointer(drag));
    assert_eq!(panel.scroll, panel.max_scroll());
    let released = PanelPointer {
        down: false,
        pressed: false,
        ..drag
    };
    assert!(!panel.handle_pointer(released));

    // A click on the track above the thumb pages up.
    let top = Vec2::new(track.center().x, track.y + 1.0);
    panel.handle_pointer(press(top));
    assert_eq!(
        panel.scroll,
        (panel.max_scroll() - panel.viewport_height).max(0.0)
    );
}

/// Every string the panel draws, in every state and view, uses only
/// characters the canvas fonts have.
#[test]
fn every_panel_state_draws_only_font_characters() {
    let mut engine = engine();
    let mut panel = EnginePanel::new();
    panel.refresh(&engine);
    assert_eq!(panel.readout.waiting, WAITING_GAP);
    let mut texts = layout(&mut panel, &engine, DEFAULT_HEIGHT).1;
    assert!(texts.iter().any(|text| text == WAITING_GAP));

    engine.set_hot_kelvin(1_200.0);
    panel.refresh(&engine);
    assert_eq!(panel.readout.waiting, WAITING_START);
    texts.extend(layout(&mut panel, &engine, DEFAULT_HEIGHT).1);

    engine.start_engine();
    // Long enough for the crank to complete a few loop bins.
    const FRAMES: usize = 900;
    for _ in 0..FRAMES {
        engine.update(1.0 / 60.0);
    }
    for mode in MODES {
        panel.select(mode);
        panel.refresh(&engine);
        if panel.readout.turn.is_none() {
            assert_eq!(panel.readout.waiting, WAITING_TURN);
        }
        let drawn = layout(&mut panel, &engine, DEFAULT_HEIGHT).1;
        assert!(drawn.iter().any(|text| text == mode.label()));
        texts.extend(drawn);
    }
    assert!(panel.range.is_some(), "the turn in progress sets the range");

    for mode in MODES {
        let mut filled = filled_panel(&engine, mode);
        texts.extend(layout(&mut filled, &engine, BROWSER_HEIGHTS[0]).1);
    }
    for text in &texts {
        assert!(drawable(text), "panel text the fonts lack: {text:?}");
    }
}

#[test]
fn repaints_only_when_the_panel_changes() {
    let engine = engine();
    let mut panel = EnginePanel::new();
    assert!(panel.needs_paint(None, DEFAULT_HEIGHT), "never painted");
    assert!(!panel.needs_paint(None, DEFAULT_HEIGHT), "nothing changed");

    panel.update(&engine, 1.0);
    assert!(panel.needs_paint(None, DEFAULT_HEIGHT), "text rebuilt");
    panel.update(&engine, 1.0 + REFRESH_SECONDS / 2.0);
    assert!(
        !panel.needs_paint(None, DEFAULT_HEIGHT),
        "too soon to rebuild"
    );

    assert!(panel.needs_paint(Some("Paused"), DEFAULT_HEIGHT), "paused");
    assert!(
        panel.needs_paint(Some("Paused"), LAPTOP_HEIGHT),
        "window resized"
    );
    panel.toggle_mode();
    assert!(
        panel.needs_paint(Some("Paused"), LAPTOP_HEIGHT),
        "other view"
    );
    panel.content_height = 2.0 * LAPTOP_HEIGHT;
    panel.viewport_height = LAPTOP_HEIGHT - HEADER_HEIGHT;
    panel.scroll_to(40.0);
    assert!(panel.needs_paint(Some("Paused"), LAPTOP_HEIGHT), "scrolled");
    assert!(!panel.needs_paint(Some("Paused"), LAPTOP_HEIGHT));
}
