//! Panel formatting and layout tests, without a graphics context.

use super::*;
use crate::engine::{DEFAULT_GEOMETRY, KickDirection};

const SEED: u64 = 42;
const LAPTOP_HEIGHT: f32 = 768.0;
const DEFAULT_HEIGHT: f32 = 900.0;
/// A P-V plot this large is comfortably readable.
const COMFORTABLE_PLOT_SIDE: f32 = 200.0;

fn engine() -> ConnectedChambers {
    ConnectedChambers::with_geometry_seed(
        crate::gas::DEFAULT_CONNECTED_MARKERS,
        DEFAULT_GEOMETRY,
        SEED,
    )
}

/// A settled heated turn, in reduced units, that balances exactly.
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

/// A panel holding readings, with a P-V range, as after a few turns.
fn filled_panel(engine: &ConnectedChambers, details: bool) -> EnginePanel {
    let mut panel = EnginePanel::new();
    panel.show_details = details;
    panel.refresh(engine, 60);
    panel.turn = Some(turn_readout(engine, heated_turn()));
    panel.range = Some(PlotRange {
        volume: engine.cylinder_volume_range_cm3(),
        pressure: (20.0, 70.0),
    });
    panel
}

#[test]
fn signed_sums_show_every_sign() {
    assert_eq!(signed_sum(&[2347.12, -2284.81]), "2347.1 - 2284.8");
    assert_eq!(signed_sum(&[-3.0, 4.0]), "-3.0 + 4.0");
}

#[test]
fn ledger_puts_each_term_on_the_side_it_flows() {
    let engine = engine();
    let readout = turn_readout(&engine, heated_turn());
    assert_eq!(
        readout.inflow.iter().map(|&(_, c)| c).collect::<Vec<_>>(),
        [HOT_COLOR]
    );
    assert_eq!(
        readout.outflow.iter().map(|&(_, c)| c).collect::<Vec<_>>(),
        [COLD_COLOR, WORK_COLOR, STORED_COLOR]
    );
    assert!((sum(&readout.inflow) - sum(&readout.outflow)).abs() < 1e-9);
    assert!(readout.ledger_warning.is_none());
    assert!(readout.heading.contains("3 TURNS"));

    // Work done on the gas and energy released by it are inflows.
    let reversed = TurnSummary {
        gas_work: -300.0,
        gas_energy_change: -100.0,
        hot_heat: 9_000.0,
        cold_heat: -9_400.0,
        ..heated_turn()
    };
    let readout = turn_readout(&engine, reversed);
    assert_eq!(readout.inflow.len(), 3);
    assert_eq!(
        readout.outflow.iter().map(|&(_, c)| c).collect::<Vec<_>>(),
        [COLD_COLOR]
    );
}

#[test]
fn efficiency_and_entropy_readouts_cover_each_case() {
    let engine = engine();
    let heated = turn_readout(&engine, heated_turn());
    assert!(
        heated.efficiency_text.0.contains("3.3%"),
        "{:?}",
        heated.efficiency_text
    );
    assert!(heated.carnot_text.ends_with("75.0%"));
    assert!((heated.carnot - 0.75).abs() < 1e-9);
    assert_eq!(heated.entropy_value.1, GOOD);
    assert!(heated.entropy_note.0.contains("dS universe"));

    let equilibrium = turn_readout(
        &engine,
        TurnSummary {
            hot_kelvin: 300.0,
            gas_work: -30.0,
            ..heated_turn()
        },
    );
    assert_eq!(equilibrium.carnot, 0.0);
    assert_eq!(equilibrium.efficiency, 0.0);
    assert!(
        equilibrium
            .efficiency_text
            .0
            .contains("without a temperature difference")
    );

    let warming = turn_readout(
        &engine,
        TurnSummary {
            gas_energy_change: 3.0 * 2_000.0,
            cold_heat: 3.0 * -6_700.0,
            ..heated_turn()
        },
    );
    assert!(warming.entropy_note.0.starts_with("Not settled"));

    let unbalanced = turn_readout(
        &engine,
        TurnSummary {
            gas_work: 3.0 * 400.0,
            ..heated_turn()
        },
    );
    assert!(unbalanced.ledger_warning.is_some());
}

/// Lays the panel out without a window and returns the P-V plot side and
/// every string it would have drawn.
fn layout(panel: &EnginePanel, engine: &ConnectedChambers, height: f32) -> (f32, Vec<String>) {
    let mut pen = Pen::offline();
    let side = panel.draw_with_height(&mut pen, engine, false, height);
    (side, pen.texts)
}

#[test]
fn panel_fits_a_laptop_screen_with_a_readable_plot() {
    let engine = engine();
    let panel = filled_panel(&engine, false);
    assert!(layout(&panel, &engine, DEFAULT_HEIGHT).0 >= COMFORTABLE_PLOT_SIDE);
    assert!(layout(&panel, &engine, LAPTOP_HEIGHT).0 >= MIN_PLOT_SIDE);
    // With details shown on a short screen the plot may give way to a note.
    let (side, texts) = layout(&filled_panel(&engine, true), &engine, LAPTOP_HEIGHT);
    assert!(side >= MIN_PLOT_SIDE || texts.iter().any(|text| text == PLOT_TOO_SMALL));
}

/// Macroquad's built-in font is only relied on for ASCII, so every string
/// the panel draws, in every state, must be ASCII.
#[test]
fn every_panel_state_draws_ascii_text() {
    let mut engine = engine();
    let mut panel = EnginePanel::new();
    panel.refresh(&engine, 60);
    assert_eq!(panel.waiting, WAITING_START);
    let mut texts = layout(&panel, &engine, DEFAULT_HEIGHT).1;
    assert!(texts.iter().any(|text| text == WAITING_START));

    engine.kick_flywheel(KickDirection::Negative);
    // Long enough for the crank to complete a few loop bins.
    const FRAMES: usize = 900;
    for _ in 0..FRAMES {
        engine.update(1.0 / 60.0);
    }
    panel.show_details = true;
    panel.limited_since_refresh = true;
    panel.refresh(&engine, 60);
    assert_eq!(panel.waiting, WAITING_TURN);
    assert!(panel.range.is_some(), "the turn in progress sets the range");
    assert!(panel.now_lines.iter().any(|(text, _)| text == LIMITED_TEXT));
    texts.extend(layout(&panel, &engine, DEFAULT_HEIGHT).1);
    let filled = layout(&filled_panel(&engine, true), &engine, DEFAULT_HEIGHT).1;
    for expected in [LAW_FIRST, LAW_CARNOT, LAW_ENTROPY, LAW_WORK, THIRD_LAW_NOTE] {
        assert!(
            filled.iter().any(|text| text == expected),
            "{expected} not drawn"
        );
    }
    texts.extend(filled);
    for text in &texts {
        assert!(text.is_ascii(), "non-ASCII panel text: {text:?}");
    }
}
