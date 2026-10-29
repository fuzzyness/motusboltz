//! Panel layout, tab and scrolling tests for `src/ui/panel.rs`, without a
//! graphics context.

use super::*;
use crate::sim::cycle::TurnSummary;
use crate::sim::geometry::DEFAULT_GEOMETRY;
use crate::sim::units::DEFAULT_CONNECTED_MARKERS;
use laws::{
    LAW_CARNOT,
    LAW_ENTROPY,
    LAW_FIRST,
    THIRD_LAW_NOTE, //
};
use plot::LOOP_KEY;
use readout::{
    WAITING_START,
    WAITING_TURN, //
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

/// A panel holding readings and a P-V range, as after a few turns.
fn filled_panel(engine: &ConnectedChambers, details: bool) -> EnginePanel {
    let mut panel = EnginePanel::new();
    panel.show_details = details;
    panel.refresh(engine);
    panel.turn = Some(turn_readout(engine, heated_turn()));
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
    let side = panel.draw_with_height(&mut pen, engine, false, height);
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
fn tabs_fit_the_header_without_overlapping() {
    for index in 0..VIEWS.len() {
        let tab = tab_rect(index);
        assert!(tab.x >= MARGIN && tab.right() <= PANEL_WIDTH - MARGIN + 1e-3);
        assert!(tab.bottom() <= HEADER_HEIGHT);
        if index + 1 < VIEWS.len() {
            assert!(tab.right() < tab_rect(index + 1).x);
        }
    }
}

#[test]
fn clicking_a_tab_selects_it_at_the_top() {
    let engine = engine();
    let mut panel = filled_panel(&engine, true);
    layout(&mut panel, &engine, BROWSER_HEIGHTS[0]);
    panel.scroll_to(f32::MAX);
    assert!(panel.scroll > 0.0);
    for (index, view) in VIEWS.into_iter().enumerate() {
        assert!(panel.handle_pointer(press(tab_rect(index).center())));
        assert_eq!(panel.view, view);
        assert_eq!(panel.scroll, 0.0);
    }
}

#[test]
fn loops_tab_gives_a_large_plot_at_every_window_height() {
    let engine = engine();
    for height in [DEFAULT_HEIGHT, LAPTOP_HEIGHT]
        .into_iter()
        .chain(BROWSER_HEIGHTS)
    {
        let mut panel = filled_panel(&engine, true);
        panel.select(PanelView::Loops);
        let (side, texts) = layout(&mut panel, &engine, height);
        assert_eq!(side, MAX_PLOT_SIDE, "plot at {height}");
        assert_eq!(panel.max_scroll(), 0.0, "scrolls at {height}");
        for (key, _) in LOOP_KEY {
            assert!(texts.iter().any(|text| text == key));
        }
    }
}

#[test]
fn all_view_fits_a_tall_window_and_scrolls_on_a_short_one() {
    let engine = engine();
    let mut panel = filled_panel(&engine, false);
    let (side, _) = layout(&mut panel, &engine, DEFAULT_HEIGHT);
    assert!(side >= COMFORTABLE_PLOT_SIDE);
    assert_eq!(panel.max_scroll(), 0.0);

    for height in BROWSER_HEIGHTS {
        let mut panel = filled_panel(&engine, true);
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
    let mut panel = filled_panel(&engine, true);
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
    let mut panel = filled_panel(&engine, true);
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

/// Macroquad's built-in font is only relied on for ASCII, so every string
/// the panel draws, in every state and tab, must be ASCII.
#[test]
fn every_panel_state_draws_ascii_text() {
    let mut engine = engine();
    let mut panel = EnginePanel::new();
    panel.refresh(&engine);
    assert_eq!(panel.waiting, WAITING_START);
    let mut texts = layout(&mut panel, &engine, DEFAULT_HEIGHT).1;
    assert!(texts.iter().any(|text| text == WAITING_START));

    engine.start_engine();
    // Long enough for the crank to complete a few loop bins.
    const FRAMES: usize = 900;
    for _ in 0..FRAMES {
        engine.update(1.0 / 60.0);
    }
    panel.show_details = true;
    panel.refresh(&engine);
    assert_eq!(panel.waiting, WAITING_TURN);
    assert!(panel.range.is_some(), "the turn in progress sets the range");
    texts.extend(layout(&mut panel, &engine, DEFAULT_HEIGHT).1);

    let mut filled = filled_panel(&engine, true);
    for view in VIEWS {
        filled.select(view);
        let drawn = layout(&mut filled, &engine, BROWSER_HEIGHTS[0]).1;
        assert!(drawn.iter().any(|text| text == view.label()));
        texts.extend(drawn);
    }
    filled.select(PanelView::Laws);
    let laws = layout(&mut filled, &engine, DEFAULT_HEIGHT).1;
    for expected in [LAW_FIRST, LAW_CARNOT, LAW_ENTROPY, THIRD_LAW_NOTE] {
        assert!(
            laws.iter().any(|text| text == expected),
            "{expected} not drawn"
        );
    }
    filled.select(PanelView::Loops);
    let loops = layout(&mut filled, &engine, DEFAULT_HEIGHT).1;
    assert!(loops.iter().any(|text| text == LAW_WORK));
    for text in texts.iter().chain(&laws).chain(&loops) {
        assert!(text.is_ascii(), "non-ASCII panel text: {text:?}");
    }
}

#[test]
fn repaints_only_when_the_panel_changes() {
    let engine = engine();
    let mut panel = EnginePanel::new();
    assert!(panel.needs_paint(false, DEFAULT_HEIGHT), "never painted");
    assert!(!panel.needs_paint(false, DEFAULT_HEIGHT), "nothing changed");

    panel.update(&engine, 1.0);
    assert!(panel.needs_paint(false, DEFAULT_HEIGHT), "text rebuilt");
    panel.update(&engine, 1.0 + REFRESH_SECONDS / 2.0);
    assert!(
        !panel.needs_paint(false, DEFAULT_HEIGHT),
        "too soon to rebuild"
    );

    assert!(panel.needs_paint(true, DEFAULT_HEIGHT), "paused");
    assert!(panel.needs_paint(true, LAPTOP_HEIGHT), "window resized");
    panel.select(PanelView::Loops);
    assert!(panel.needs_paint(true, LAPTOP_HEIGHT), "other tab");
    panel.toggle_details();
    assert!(panel.needs_paint(true, LAPTOP_HEIGHT), "details shown");
    panel.content_height = 2.0 * LAPTOP_HEIGHT;
    panel.viewport_height = LAPTOP_HEIGHT - HEADER_HEIGHT;
    panel.scroll_to(40.0);
    assert!(panel.needs_paint(true, LAPTOP_HEIGHT), "scrolled");
    assert!(!panel.needs_paint(true, LAPTOP_HEIGHT));
}
