//! The panel's wording and numbers: live readings, the per-turn energy
//! ledger, efficiency against the Carnot limit, and the entropy change,
//! formatted from the simulation for drawing.

use macroquad::prelude::Color;

use super::{
    COLD_COLOR,
    GOOD,
    HOT_COLOR,
    QUIET,
    STORED_COLOR,
    TEXT,
    WARNING,
    WORK_COLOR, //
};
use crate::sim::cycle::{
    COLD,
    HOT,
    TurnSummary,
    loop_work, //
};
use crate::sim::engine::ConnectedChambers;
use crate::sim::units::GasMeasurements;

const PASCALS_PER_BAR: f64 = 100_000.0;
const SECONDS_PER_MINUTE: f32 = 60.0;
/// The ledger is shown as balanced to one decimal; a larger mismatch is
/// flagged. Measured mismatches are about 0.001 J per turn.
const LEDGER_TOLERANCE_JOULES: f64 = 0.05;

pub const WAITING_START: &str = "Press Start (Enter) to turn the crank.";
pub const WAITING_TURN: &str = "Waiting for the crank to finish a full turn.";
pub const LIMITED_TEXT: &str = "Physics time limited: computer too slow";
const LOOPS_NOT_YET: &str = "Dots: each cylinder's state now";

/// A line of text with its color.
pub type Line = (String, Color);

/// Live readings, refreshed a few times a second.
#[derive(Default)]
pub struct NowReadout {
    pub lines: Vec<Line>,
    /// Extra lines shown with Details on.
    pub details: Vec<String>,
}

/// Inputs to the live readings that come from the app rather than the
/// simulation.
pub struct FrameStats {
    /// Whether physics time was dropped since the last refresh.
    pub limited: bool,
    pub fps: i32,
    /// Smoothed time spent stepping the physics each frame.
    pub physics_ms: f64,
}

/// Formatted readings for the recent turns.
pub struct TurnReadout {
    pub heading: String,
    /// Energy flowing in and out, as (amount, color) bar segments.
    pub inflow: Vec<(f64, Color)>,
    pub outflow: Vec<(f64, Color)>,
    pub ledger_values: String,
    /// Shown only if the ledger fails to balance, which would be a bug.
    pub ledger_warning: Option<String>,
    pub efficiency: f64,
    pub carnot: f64,
    pub efficiency_text: Line,
    pub carnot_text: String,
    pub entropy_value: Line,
    pub entropy_note: Line,
}

/// The "Engine now" lines and, with `details`, the extra lines.
pub fn now_readout(engine: &ConnectedChambers, details: bool, stats: FrameStats) -> NowReadout {
    let regions = engine.measurements();
    let pistons = engine.power_pistons();
    let mut lines = vec![
        (gas_line("Hot gas", regions.hot), HOT_COLOR),
        (gas_line("Cold gas", regions.cold), COLD_COLOR),
        (
            format!(
                "Crank: {:+.2} model rpm | {:.1} turns",
                pistons.angular_velocity() * SECONDS_PER_MINUTE / std::f32::consts::TAU,
                pistons.revolutions().abs(),
            ),
            TEXT,
        ),
        (
            format!(
                "Flywheel energy: {:.1} J",
                engine.energy_joules(pistons.kinetic_energy())
            ),
            TEXT,
        ),
    ];
    if stats.limited {
        lines.push((LIMITED_TEXT.to_owned(), WARNING));
    }
    if !details {
        return NowReadout {
            lines,
            details: Vec::new(),
        };
    }
    let heat = engine.heat();
    let details = vec![
        format!(
            "Helium: {} particles | {:.4} mol",
            engine.particles.len(),
            engine.total_moles()
        ),
        format!(
            "Particles: hot {} | cold {} | passage {}",
            regions.hot.markers, regions.cold.markers, regions.passage_markers
        ),
        format!(
            "Concentration: hot {:.2} mol/L | cold {:.2} mol/L",
            regions.hot.concentration_mol_per_l, regions.cold.concentration_mol_per_l
        ),
        format!(
            "Heat into gas: hot {:+.1} J | cold {:+.1} J",
            engine.energy_joules(heat.hot),
            engine.energy_joules(heat.cold)
        ),
        format!(
            "Gas work: {:+.1} J | starter: {:+.1} J",
            engine.energy_joules(engine.piston_work_by_gas()),
            engine.energy_joules(pistons.starter_work())
        ),
        format!(
            "Balance error: gas {:+.4} J | shaft {:+.4} J",
            engine.energy_joules(engine.energy_balance_error()),
            engine.energy_joules(engine.mechanical_balance_error())
        ),
        format!(
            "Display: {} fps | physics {:.1} ms per frame",
            stats.fps, stats.physics_ms
        ),
    ];
    NowReadout { lines, details }
}

/// One "Engine now" line: temperature and ideal-gas pressure estimate.
fn gas_line(name: &str, sample: GasMeasurements) -> String {
    let temperature = sample
        .temperature_kelvin
        .map_or_else(|| "--".to_owned(), |kelvin| format!("{kelvin:.0} K"));
    let pressure = sample.pressure_pa.map_or_else(
        || "--".to_owned(),
        |pa| format!("~{:.1} bar", pa / PASCALS_PER_BAR),
    );
    format!("{name}: {temperature} | {pressure}")
}

/// What the turn readouts show before the first full turn.
pub fn waiting_text(engine: &ConnectedChambers) -> &'static str {
    if engine.power_pistons().angular_velocity() == 0.0 && engine.cycle().completed_turns() == 0 {
        WAITING_START
    } else {
        WAITING_TURN
    }
}

/// The work enclosed by each cylinder's last loop and their sum.
pub fn loop_text(engine: &ConnectedChambers) -> String {
    let cycle = engine.cycle();
    if cycle.last_loop(HOT).is_empty() {
        return LOOPS_NOT_YET.to_owned();
    }
    let [hot_work, cold_work] =
        [HOT, COLD].map(|cylinder| engine.energy_joules(loop_work(cycle.last_loop(cylinder))));
    format!(
        "Hot {hot_work:+.1} J + cold {cold_work:+.1} J = W {:+.1} J",
        hot_work + cold_work
    )
}

/// Formats the recent turns, averaged per turn.
pub fn turn_readout(engine: &ConnectedChambers, recent: TurnSummary) -> TurnReadout {
    let turns = recent.turns;
    let turn = recent.per_turn();
    let joules = |reduced: f64| engine.energy_joules(reduced);
    let (hot, cold, work, stored) = (
        joules(turn.hot_heat),
        joules(turn.cold_heat),
        joules(turn.gas_work),
        joules(turn.gas_energy_change),
    );
    let mut inflow = Vec::new();
    let mut outflow = Vec::new();
    // Each term goes on the side it flows: a positive heat is into the gas,
    // positive work and stored energy are out of the heat supplied.
    for (amount, color, into_gas) in [
        (hot, HOT_COLOR, true),
        (cold, COLD_COLOR, true),
        (work, WORK_COLOR, false),
        (stored, STORED_COLOR, false),
    ] {
        let side = if (amount > 0.0) == into_gas {
            &mut inflow
        } else {
            &mut outflow
        };
        if amount != 0.0 {
            side.push((amount.abs(), color));
        }
    }

    let heading = if turns == 1 {
        format!("LAST FULL TURN ({:.0} s)", turn.duration)
    } else {
        format!(
            "AVERAGE OF LAST {turns} TURNS ({:.0} s each)",
            turn.duration
        )
    };
    let carnot = turn.carnot_limit();
    let efficiency = turn.efficiency().filter(|_| turn.gas_work > 0.0);
    let efficiency_text = if carnot == 0.0 {
        (
            "Efficiency: none without a temperature difference".to_owned(),
            QUIET,
        )
    } else if let Some(value) = efficiency {
        (
            format!("Efficiency W / Q hot = {:.1}%", 100.0 * value),
            TEXT,
        )
    } else {
        ("Efficiency 0%: no net work out per turn".to_owned(), QUIET)
    };
    let carnot_text = format!(
        "Carnot limit 1 - {:.0}/{:.0} = {:.1}%",
        turn.cold_kelvin,
        turn.hot_kelvin,
        100.0 * carnot
    );

    let entropy = engine.entropy_joules_per_kelvin(turn.surroundings_entropy_change());
    let entropy_value = (
        format!("= {entropy:+.3} J/K per turn"),
        if entropy > 0.0 { GOOD } else { WARNING },
    );
    let entropy_note = if !turn.is_settled() {
        (
            format!("Not settled (dU {stored:+.0} J): dS gas not counted"),
            QUIET,
        )
    } else if entropy > 0.0 {
        (
            "Gas back to start, so dS universe = dS surr".to_owned(),
            QUIET,
        )
    } else {
        ("Unexpected: dS surr is not positive".to_owned(), WARNING)
    };

    TurnReadout {
        heading,
        inflow,
        outflow,
        ledger_values: format!(
            "{} = {} J",
            signed_sum(&[hot, cold]),
            signed_sum(&[work, stored])
        ),
        ledger_warning: {
            let residual = joules(turn.first_law_residual());
            (residual.abs() > LEDGER_TOLERANCE_JOULES)
                .then(|| format!("Unexpected: off by {residual:+.2} J"))
        },
        efficiency: efficiency.unwrap_or(0.0).clamp(0.0, 1.0),
        carnot,
        efficiency_text,
        carnot_text,
        entropy_value,
        entropy_note,
    }
}

/// Writes terms as a sum with explicit signs, e.g. "2347.1 - 2284.8".
fn signed_sum(terms: &[f64]) -> String {
    let mut text = String::new();
    for (index, &term) in terms.iter().enumerate() {
        match (index, term < 0.0) {
            (0, false) => text += &format!("{term:.1}"),
            (0, true) => text += &format!("-{:.1}", -term),
            (_, false) => text += &format!(" + {term:.1}"),
            (_, true) => text += &format!(" - {:.1}", -term),
        }
    }
    text
}

/// Total of a bar's segments.
pub fn sum(segments: &[(f64, Color)]) -> f64 {
    segments.iter().map(|&(amount, _)| amount).sum()
}

#[cfg(test)]
#[path = "../../../tests/unit/ui/panel/readout.rs"]
mod tests;
