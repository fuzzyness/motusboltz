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

    let no_work = turn_readout(
        &engine,
        TurnSummary {
            gas_work: 0.0,
            cold_heat: 3.0 * -8_900.0,
            ..heated_turn()
        },
    );
    assert!(no_work.efficiency_text.0.ends_with("per turn"));

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

#[test]
fn details_add_lines() {
    let engine = engine();
    let plain = now_readout(&engine, false);
    assert!(plain.details.is_empty());
    let detailed = now_readout(&engine, true);
    assert_eq!(detailed.lines, plain.lines);
    assert!(
        detailed
            .details
            .iter()
            .any(|text| text.starts_with("Balance error"))
    );
    assert_eq!(waiting_text(&engine), WAITING_START);
}
