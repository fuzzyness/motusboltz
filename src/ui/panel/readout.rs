//! The panel's numbers, gathered from the simulation a few times a second.
//!
//! They are what the gas and the flywheel are doing now, and where the
//! energy went over the recent turns. Formatting helpers shared by both
//! views are here too.

use crate::sim::cycle::{
    COLD,
    HOT,
    TurnSummary,
    loop_work, //
};
use crate::sim::engine::ConnectedChambers;
use crate::sim::units::GasMeasurements;

/// Pascals in a bar.
pub const PASCALS_PER_BAR: f64 = 100_000.0;
/// Seconds in a minute.
const SECONDS_PER_MINUTE: f64 = 60.0;
/// Below this speed, in turns per minute, the flywheel counts as at rest.
///
/// Without Start, gas pushes alone drift a free shaft at about 0.1; the
/// slowest engine that keeps turning (450/300 K) makes about 0.4.
const AT_REST_TURNS_PER_MINUTE: f64 = 0.2;
/// Largest ledger mismatch shown as balanced, in joules per turn.
///
/// The ledger is shown to one decimal; a larger mismatch is flagged.
/// Measured mismatches are about 0.001 J per turn.
pub const LEDGER_TOLERANCE_JOULES: f64 = 0.05;

/// Shown while the flywheel is at rest.
pub const WAITING_START: &str = "Press Start (Space) to turn the flywheel.";
/// Shown while both sides are at one temperature.
pub const WAITING_GAP: &str = "No temperature difference: drag Hot side up.";
/// Shown during the first turn.
pub const WAITING_TURN: &str = "Waiting for the flywheel to finish a turn.";
/// Shown during the first turn after new settings.
pub const WAITING_SETTINGS: &str = "New settings: waiting for a full turn.";
/// Shown in place of the loop work before the first loop.
const LOOPS_NOT_YET: &str = "Dots: each cylinder's state now";

/// One cylinder's gas right now.
///
/// Temperature and pressure are `None` while the cylinder holds too few
/// particles to measure.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gas {
    /// Temperature.
    pub kelvin: Option<f64>,
    /// Pressure on the cylinder's piston, measured from the gas's hits, in
    /// bar.
    pub bar: Option<f64>,
    /// Particles in the cylinder.
    pub particles: usize,
    /// Concentration, in mol/L.
    pub mol_per_l: f64,
}

impl From<GasMeasurements> for Gas {
    fn from(sample: GasMeasurements) -> Self {
        Self {
            kelvin: sample.temperature_kelvin,
            bar: sample.pressure_pa.map(|pa| pa / PASCALS_PER_BAR),
            particles: sample.count,
            mol_per_l: sample.concentration_mol_per_l,
        }
    }
}

/// What the gas and the flywheel are doing now, and running totals since
/// the gas was filled.
///
/// Energies are in joules.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Now {
    /// Hot-wall setting.
    pub hot_wall_kelvin: f32,
    /// Cold-wall setting.
    pub cold_wall_kelvin: f32,
    /// Gas in the hot cylinder.
    pub hot: Gas,
    /// Gas in the cold cylinder.
    pub cold: Gas,
    /// Particles in the opening between the cylinders.
    pub passage_particles: usize,
    /// All particles.
    pub particles: usize,
    /// All the helium, in moles.
    pub moles: f64,
    /// Flywheel speed, positive in the heat-engine direction.
    pub turns_per_minute: f64,
    /// Revolutions so far, either way.
    pub turns: f64,
    /// Flywheel energy.
    pub flywheel: f64,
    /// Energy Start and the kick gave.
    pub starter: f64,
    /// Work the load took.
    pub to_load: f64,
    /// Energy lost to friction.
    pub friction: f64,
    /// Heat from the hot wall.
    pub heat_from_hot: f64,
    /// Heat from the cold wall.
    pub heat_from_cold: f64,
    /// Work the gas did on the pistons.
    pub gas_work: f64,
    /// Gas first-law residual.
    pub gas_error: f64,
    /// Flywheel energy-balance residual.
    pub shaft_error: f64,
}

/// Energy per turn, averaged over the recent turns, with the numbers each
/// law needs.
///
/// Energies are in joules.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Turn {
    /// Turns averaged.
    pub turns: usize,
    /// Model seconds per turn.
    pub seconds: f64,
    /// Heat into the gas from the hot wall.
    pub hot_heat: f64,
    /// Heat into the gas from the cold wall, negative when heat leaves the
    /// gas there.
    pub cold_heat: f64,
    /// Work the gas did.
    pub work: f64,
    /// Change in the gas's own energy, dU.
    pub stored: f64,
    /// Hot-wall setting over the turns.
    pub hot_kelvin: f32,
    /// Cold-wall setting over the turns.
    pub cold_kelvin: f32,
    /// Work out over heat in, when the engine did net work.
    pub efficiency: Option<f64>,
    /// Carnot limit for the settings.
    pub carnot: f64,
    /// Entropy change of the surroundings (both walls), in J/K.
    pub entropy: f64,
    /// The gas ended the turns with nearly the energy it started with, so
    /// its own entropy change is negligible and `entropy` is also the
    /// change of the universe.
    pub settled: bool,
    /// Q hot + Q cold - W - dU, ideally zero.
    pub residual: f64,
}

/// Everything the panel shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Readout {
    /// What is happening now.
    pub now: Now,
    /// The recent turns, once there is one.
    pub turn: Option<Turn>,
    /// Shown in place of the per-turn numbers until there are some.
    pub waiting: &'static str,
    /// The work enclosed by each P-V loop and their sum.
    pub loop_text: String,
}

/// Reads everything the panel shows from the simulation.
pub fn readout(engine: &ConnectedChambers) -> Readout {
    Readout {
        now: now(engine),
        turn: engine.cycle().recent().map(|recent| turn(engine, recent)),
        waiting: waiting_text(engine),
        loop_text: loop_text(engine),
    }
}

/// What the gas and the flywheel are doing now.
fn now(engine: &ConnectedChambers) -> Now {
    let regions = engine.measurements();
    let pistons = engine.power_pistons();
    let joules = |reduced: f64| engine.units().energy_joules(reduced);
    let heat = engine.heat();
    Now {
        hot_wall_kelvin: engine.hot_kelvin(),
        cold_wall_kelvin: engine.cold_kelvin(),
        hot: regions.hot.into(),
        cold: regions.cold.into(),
        passage_particles: regions.passage_count,
        particles: engine.particles.len(),
        moles: engine.total_moles(),
        turns_per_minute: turns_per_minute(engine),
        turns: pistons.revolutions().abs(),
        flywheel: joules(pistons.kinetic_energy()),
        starter: joules(pistons.starter_work()),
        to_load: joules(pistons.load_work()),
        friction: joules(pistons.drag_loss() - pistons.load_work()),
        heat_from_hot: joules(heat.hot),
        heat_from_cold: joules(heat.cold),
        gas_work: joules(engine.piston_work_by_gas()),
        gas_error: joules(engine.energy_balance_error()),
        shaft_error: joules(engine.mechanical_balance_error()),
    }
}

/// The recent turns, averaged per turn, in joules.
pub fn turn(engine: &ConnectedChambers, recent: TurnSummary) -> Turn {
    let per_turn = recent.per_turn();
    let joules = |reduced: f64| engine.units().energy_joules(reduced);
    Turn {
        turns: recent.turns,
        seconds: per_turn.duration,
        hot_heat: joules(per_turn.change.hot_heat),
        cold_heat: joules(per_turn.change.cold_heat),
        work: joules(per_turn.change.gas_work),
        stored: joules(per_turn.change.gas_energy),
        hot_kelvin: per_turn.hot_kelvin,
        cold_kelvin: per_turn.cold_kelvin,
        efficiency: per_turn
            .efficiency()
            .filter(|_| per_turn.change.gas_work > 0.0)
            .map(|value| value.clamp(0.0, 1.0)),
        carnot: per_turn.carnot_limit(),
        entropy: engine
            .units()
            .entropy_joules_per_kelvin(per_turn.surroundings_entropy_change()),
        settled: per_turn.is_settled(),
        residual: joules(per_turn.first_law_residual()),
    }
}

/// What the per-turn readouts show before there is a full turn to show.
///
/// That is that the sides need a temperature difference, how to start,
/// that new settings started the average over, or that a turn is under
/// way.
pub fn waiting_text(engine: &ConnectedChambers) -> &'static str {
    if engine.hot_kelvin() <= engine.cold_kelvin() {
        WAITING_GAP
    } else if turns_per_minute(engine).abs() < AT_REST_TURNS_PER_MINUTE {
        WAITING_START
    } else if engine.cycle().completed_turns() > 0 {
        WAITING_SETTINGS
    } else {
        WAITING_TURN
    }
}

/// The flywheel's speed in turns per minute of model time, positive in
/// the heat-engine direction.
fn turns_per_minute(engine: &ConnectedChambers) -> f64 {
    let angular_velocity = engine.power_pistons().angular_velocity() as f64;
    -angular_velocity * SECONDS_PER_MINUTE / std::f64::consts::TAU
}

/// The work enclosed by each cylinder's last loop and their sum.
pub fn loop_text(engine: &ConnectedChambers) -> String {
    let cycle = engine.cycle();
    if cycle.last_loop(HOT).is_empty() {
        return LOOPS_NOT_YET.to_owned();
    }
    let units = engine.units();
    let [hot_work, cold_work] =
        [HOT, COLD].map(|cylinder| units.energy_joules(loop_work(cycle.last_loop(cylinder))));
    format!(
        "Hot {} J + cold {} J = W {} J",
        signed_number(hot_work, 1),
        signed_number(cold_work, 1),
        signed_number(hot_work + cold_work, 1)
    )
}

/// A whole number of joules, or one decimal below 10, with thousands
/// separators and a true minus sign.
///
/// For example, "5,247", "3.2" or "−12".
pub fn amount(value: f64) -> String {
    if value.abs() < 10.0 {
        let text = format!("{:.1}", value.abs());
        return if value < 0.0 && text != "0.0" {
            format!("−{text}")
        } else {
            text
        };
    }
    let rounded = value.round() as i64;
    let digits = rounded.unsigned_abs().to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    if rounded < 0 {
        format!("−{grouped}")
    } else {
        grouped
    }
}

/// Writes terms as a sum with explicit signs, e.g. "2347.1 − 2284.8".
pub fn signed_sum(terms: &[f64]) -> String {
    let mut text = String::new();
    for (index, &term) in terms.iter().enumerate() {
        match (index, term < 0.0) {
            (0, false) => text += &format!("{term:.1}"),
            (0, true) => text += &format!("−{:.1}", -term),
            (_, false) => text += &format!(" + {term:.1}"),
            (_, true) => text += &format!(" − {:.1}", -term),
        }
    }
    text
}

/// The flywheel's speed for the Simple view, in revolutions per minute of
/// model time.
///
/// For example, "3.4 rpm", "−0.8 rpm" when it turns backward, or "at rest".
pub fn speed_text(turns_per_minute: f64) -> String {
    if turns_per_minute.abs() < AT_REST_TURNS_PER_MINUTE {
        "at rest".to_owned()
    } else if turns_per_minute < 0.0 {
        format!("−{:.1} rpm", -turns_per_minute)
    } else {
        format!("{turns_per_minute:.1} rpm")
    }
}

/// A number with an explicit sign and a true minus sign.
pub fn signed_number(value: f64, decimals: usize) -> String {
    let text = format!("{:.*}", decimals, value.abs());
    let zero = text.chars().all(|c| c == '0' || c == '.');
    if value < 0.0 && !zero {
        format!("−{text}")
    } else {
        format!("+{text}")
    }
}

/// A temperature, or a dash when there is none to show.
pub fn kelvin_text(kelvin: Option<f64>) -> String {
    kelvin.map_or_else(|| "—".to_owned(), |kelvin| format!("{kelvin:.0} K"))
}

#[cfg(test)]
#[path = "../../../tests/unit/ui/panel/readout.rs"]
mod tests;
