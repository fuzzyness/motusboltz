//! Wording and number tests for `src/ui/panel/readout.rs`.

use super::*;
use crate::sim::engine::ConnectedChambers;
use crate::sim::geometry::DEFAULT_GEOMETRY;
use crate::sim::units::DEFAULT_CONNECTED_MARKERS;

const SEED: u64 = 42;

fn engine() -> ConnectedChambers {
    ConnectedChambers::with_geometry_seed(DEFAULT_CONNECTED_MARKERS, DEFAULT_GEOMETRY, SEED)
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

#[test]
fn numbers_use_true_minus_signs_and_thousands_separators() {
    assert_eq!(signed_sum(&[2347.12, -2284.81]), "2347.1 − 2284.8");
    assert_eq!(signed_sum(&[-3.0, 4.0]), "−3.0 + 4.0");
    assert_eq!(amount(5_247.4), "5,247");
    assert_eq!(amount(-12_345.0), "−12,345");
    assert_eq!(amount(3.24), "3.2");
    assert_eq!(amount(-0.01), "0.0", "no minus on a rounded zero");
    assert_eq!(signed_number(43.45, 1), "+43.5");
    assert_eq!(signed_number(-0.0001, 2), "+0.00");
    assert_eq!(signed_number(-2.5, 1), "−2.5");
    assert_eq!(kelvin_text(Some(1057.4)), "1057 K");
    assert_eq!(kelvin_text(None), "—");
}

#[test]
fn speed_reads_at_rest_forward_or_backward() {
    assert_eq!(speed_text(0.0), "at rest");
    assert_eq!(speed_text(0.1), "at rest", "gas pushes alone");
    assert_eq!(speed_text(3.44), "3.4 rpm");
    assert_eq!(speed_text(-0.8), "−0.8 rpm");
}

#[test]
fn a_heated_turn_balances_and_counts_as_settled() {
    let engine = engine();
    let turn = turn(&engine, heated_turn());
    assert_eq!(turn.turns, 3);
    assert!((turn.seconds - 80.0).abs() < 1e-9);
    let efficiency = turn.efficiency.expect("net work out");
    assert!((efficiency - 300.0 / 9_000.0).abs() < 1e-9);
    assert!((turn.carnot - 0.75).abs() < 1e-9);
    assert!(turn.entropy > 0.0);
    assert!(turn.settled);
    assert!(turn.residual.abs() < LEDGER_TOLERANCE_JOULES);
    let ledger = turn.hot_heat + turn.cold_heat - turn.work - turn.stored;
    assert!(ledger.abs() < 1e-9);
}

#[test]
fn turns_without_net_work_or_still_warming_say_so() {
    let engine = engine();
    let no_work = turn(
        &engine,
        TurnSummary {
            gas_work: -30.0,
            cold_heat: 3.0 * -8_900.0 + 30.0,
            ..heated_turn()
        },
    );
    assert!(no_work.efficiency.is_none());

    let warming = turn(
        &engine,
        TurnSummary {
            gas_energy_change: 3.0 * 2_000.0,
            cold_heat: 3.0 * -6_700.0,
            ..heated_turn()
        },
    );
    assert!(!warming.settled);
}

#[test]
fn waiting_text_asks_for_a_temperature_difference_then_for_start() {
    let mut engine = engine();
    assert_eq!(waiting_text(&engine), WAITING_GAP, "both sides at 300 K");
    engine.set_hot_kelvin(1_200.0);
    assert_eq!(waiting_text(&engine), WAITING_START);
    engine.start_engine();
    engine.update(1.0 / 60.0);
    assert_eq!(waiting_text(&engine), WAITING_TURN);
}
