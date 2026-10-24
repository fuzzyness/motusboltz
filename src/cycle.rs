//! Per-turn bookkeeping of the engine cycle: heat, work, entropy flow and a
//! pressure-volume loop, measured between successive full turns of the
//! crank.
//!
//! Comparing whole turns matters. The pistons return to the same positions
//! every turn, so compression and back-pressure work cancel over a turn and
//! the first and second laws can be checked honestly. A partial turn mixes
//! different compression phases and is never reported.
//!
//! Everything here is in reduced simulation units; the engine converts to SI.

use std::f64::consts::TAU;

/// Pressure-volume samples per turn, binned by crank angle. Each bin
/// averages the pressure over 10 degrees of crank rotation; narrower bins
/// show mostly the noise of individual particle impacts.
pub const LOOP_BINS: usize = 36;
/// Number of most recent turns combined in `CycleMonitor::recent`.
pub const TURNS_AVERAGED: usize = 3;
/// Turns count as settled when the gas's energy changes by at most this
/// fraction of the heat flowing through it.
const SETTLED_FRACTION: f64 = 0.10;
/// Bound on the visit list when the shaft rocks back and forth in one turn.
const MAX_VISITS: usize = 4 * LOOP_BINS;

/// Cumulative totals read from the simulation at a turn boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CycleTotals {
    /// Heat into the gas from each reservoir (`HeatTransfer`).
    pub hot_heat: f64,
    pub cold_heat: f64,
    /// Heat over reservoir temperature (`EntropyFlow`).
    pub hot_entropy: f64,
    pub cold_entropy: f64,
    /// Work done by the gas on the pistons.
    pub gas_work: f64,
    /// Kinetic energy of the gas, which is its internal energy.
    pub gas_energy: f64,
}

/// Changes over one or more complete turns at fixed reservoir temperatures.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TurnSummary {
    /// Number of turns combined.
    pub turns: usize,
    pub hot_heat: f64,
    pub cold_heat: f64,
    pub hot_entropy: f64,
    pub cold_entropy: f64,
    pub gas_work: f64,
    /// Change in gas internal energy, dU.
    pub gas_energy_change: f64,
    /// Model time taken.
    pub duration: f64,
    pub hot_kelvin: f32,
    pub cold_kelvin: f32,
}

impl TurnSummary {
    fn between(start: CycleTotals, end: CycleTotals, duration: f64, kelvin: (f32, f32)) -> Self {
        Self {
            turns: 1,
            hot_heat: end.hot_heat - start.hot_heat,
            cold_heat: end.cold_heat - start.cold_heat,
            hot_entropy: end.hot_entropy - start.hot_entropy,
            cold_entropy: end.cold_entropy - start.cold_entropy,
            gas_work: end.gas_work - start.gas_work,
            gas_energy_change: end.gas_energy - start.gas_energy,
            duration,
            hot_kelvin: kelvin.0,
            cold_kelvin: kelvin.1,
        }
    }

    /// Sum of two summaries taken at the same reservoir temperatures.
    fn combined(self, other: Self) -> Self {
        Self {
            turns: self.turns + other.turns,
            hot_heat: self.hot_heat + other.hot_heat,
            cold_heat: self.cold_heat + other.cold_heat,
            hot_entropy: self.hot_entropy + other.hot_entropy,
            cold_entropy: self.cold_entropy + other.cold_entropy,
            gas_work: self.gas_work + other.gas_work,
            gas_energy_change: self.gas_energy_change + other.gas_energy_change,
            duration: self.duration + other.duration,
            ..self
        }
    }

    /// The same quantities averaged per turn.
    pub fn per_turn(self) -> Self {
        let turns = self.turns.max(1) as f64;
        Self {
            turns: 1,
            hot_heat: self.hot_heat / turns,
            cold_heat: self.cold_heat / turns,
            hot_entropy: self.hot_entropy / turns,
            cold_entropy: self.cold_entropy / turns,
            gas_work: self.gas_work / turns,
            gas_energy_change: self.gas_energy_change / turns,
            duration: self.duration / turns,
            ..self
        }
    }

    /// First law check, ideally zero: Q hot + Q cold - W - dU.
    pub fn first_law_residual(self) -> f64 {
        self.hot_heat + self.cold_heat - self.gas_work - self.gas_energy_change
    }

    /// Work out divided by heat in from the hot wall; `None` when the hot
    /// wall delivered no net heat, so efficiency is undefined.
    pub fn efficiency(self) -> Option<f64> {
        (self.hot_heat > 0.0).then(|| self.gas_work / self.hot_heat)
    }

    /// Carnot limit 1 - Tc/Th for these reservoir temperatures.
    pub fn carnot_limit(self) -> f64 {
        (1.0 - self.cold_kelvin as f64 / self.hot_kelvin as f64).max(0.0)
    }

    /// Entropy change of both reservoirs, -(Qh/Th + Qc/Tc).
    pub fn surroundings_entropy_change(self) -> f64 {
        -(self.hot_entropy + self.cold_entropy)
    }

    /// Whether the gas ended the turns with nearly the energy it started
    /// with. Only then is the gas's own entropy change negligible, so the
    /// surroundings' change is the change of the universe.
    pub fn is_settled(self) -> bool {
        let throughput = (self.hot_heat.abs() + self.cold_heat.abs()) / 2.0;
        throughput > 0.0 && self.gas_energy_change.abs() <= SETTLED_FRACTION * throughput
    }
}

/// One point of the pressure-volume loop: volume in cubic model units and
/// pressure in reduced units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LoopPoint {
    pub volume: f64,
    pub pressure: f64,
}

/// Work enclosed by a closed loop, the trapezoid sum of P dV around it.
/// Positive when the gas expands at higher pressure than it is compressed.
pub fn loop_work(points: &[LoopPoint]) -> f64 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| 0.5 * (a.pressure + b.pressure) * (b.volume - a.volume))
        .sum()
}

/// Index of the hot cylinder in per-cylinder arrays; the cold one is 1.
pub const HOT: usize = 0;
pub const COLD: usize = 1;

/// Per cylinder: gas impulse on its piston, and volume, accumulated over one
/// crank-angle bin along with the time spent in it.
#[derive(Clone, Copy, Debug, Default)]
struct LoopBin {
    impulse: [f64; 2],
    volume_time: [f64; 2],
    time: f64,
}

impl LoopBin {
    /// Mean volume of one cylinder and mean gas pressure on its piston.
    fn point(self, cylinder: usize, face_area: f64) -> Option<LoopPoint> {
        (self.time > 0.0).then(|| LoopPoint {
            volume: self.volume_time[cylinder] / self.time,
            pressure: self.impulse[cylinder] / (self.time * face_area),
        })
    }
}

/// What the simulation reports after each integration substep. Arrays are
/// indexed by `HOT` and `COLD`.
#[derive(Clone, Copy, Debug)]
pub struct Substep {
    /// Total signed crank rotation since the fill, in radians.
    pub total_angle: f64,
    /// Crank angle reduced to 0..2 pi.
    pub crank_angle: f32,
    pub dt: f64,
    /// Upward impulse the gas gave each piston face during the substep.
    pub gas_impulse: [f64; 2],
    /// Gas volume of each cylinder, below its piston, in cubic model units.
    pub volume: [f64; 2],
}

/// Detects full turns of the crank and keeps the most recent summaries and
/// the last turn's pressure-volume loop for each cylinder.
///
/// There is one loop per cylinder because the two cylinders run at
/// different pressures (gas cannot cross the passage instantly). Each loop
/// encloses the work done by the gas on that cylinder's piston, so the two
/// areas add up to the total gas work; a single loop of average pressure
/// against total volume would not.
///
/// A turn is complete when the crank crosses a whole-turn angle different
/// from the last one crossed; it then has the same piston positions as at
/// the start. Crossing the same angle again (the shaft rocked backward)
/// restarts the count instead. A turn during which a reservoir temperature
/// changed is discarded, along with the history, so averages always use
/// one setting.
pub struct CycleMonitor {
    /// Area of one piston face, in square model units.
    face_area: f64,
    turn_index: i64,
    last_boundary: i64,
    start: CycleTotals,
    elapsed: f64,
    settings_changed: bool,
    history: [TurnSummary; TURNS_AVERAGED],
    history_len: usize,
    history_next: usize,
    completed_turns: u64,
    bins: [LoopBin; LOOP_BINS],
    visits: Vec<usize>,
    last_loops: [Vec<LoopPoint>; 2],
}

impl CycleMonitor {
    /// Starts at crank angle zero with the given cumulative totals.
    pub fn new(face_area: f64, start: CycleTotals) -> Self {
        assert!(face_area.is_finite() && face_area > 0.0);
        Self {
            face_area,
            turn_index: 0,
            last_boundary: 0,
            start,
            elapsed: 0.0,
            settings_changed: false,
            history: [TurnSummary::default(); TURNS_AVERAGED],
            history_len: 0,
            history_next: 0,
            completed_turns: 0,
            bins: [LoopBin::default(); LOOP_BINS],
            visits: Vec::with_capacity(MAX_VISITS),
            last_loops: std::array::from_fn(|_| Vec::with_capacity(LOOP_BINS)),
        }
    }

    /// Records a substep. `totals` is called only at a turn boundary, so
    /// the full-gas energy sum is not computed every substep. `kelvin` is
    /// (hot, cold).
    pub fn record(
        &mut self,
        substep: Substep,
        kelvin: (f32, f32),
        totals: impl FnOnce() -> CycleTotals,
    ) {
        self.elapsed += substep.dt;
        let bin =
            ((substep.crank_angle as f64 / TAU * LOOP_BINS as f64) as usize).min(LOOP_BINS - 1);
        let slot = &mut self.bins[bin];
        for cylinder in [HOT, COLD] {
            slot.impulse[cylinder] += substep.gas_impulse[cylinder];
            slot.volume_time[cylinder] += substep.volume[cylinder] * substep.dt;
        }
        slot.time += substep.dt;
        if self.visits.last() != Some(&bin) && self.visits.len() < MAX_VISITS {
            self.visits.push(bin);
        }

        let index = (substep.total_angle / TAU).floor() as i64;
        if index == self.turn_index {
            return;
        }
        // Moving backward crosses the boundary at the old index; forward,
        // at the new one.
        let boundary = self.turn_index.max(index);
        self.turn_index = index;
        let end = totals();
        if boundary != self.last_boundary && !self.settings_changed {
            let summary = TurnSummary::between(self.start, end, self.elapsed, kelvin);
            self.history[self.history_next] = summary;
            self.history_next = (self.history_next + 1) % TURNS_AVERAGED;
            self.history_len = (self.history_len + 1).min(TURNS_AVERAGED);
            self.completed_turns += 1;
            self.rebuild_last_loops();
        }
        self.last_boundary = boundary;
        self.start = end;
        self.elapsed = 0.0;
        self.settings_changed = false;
        self.bins = [LoopBin::default(); LOOP_BINS];
        self.visits.clear();
    }

    /// The bins of the finished turn, in the order the crank swept them.
    fn rebuild_last_loops(&mut self) {
        let mut seen = [false; LOOP_BINS];
        for points in &mut self.last_loops {
            points.clear();
        }
        for &bin in &self.visits {
            if std::mem::replace(&mut seen[bin], true) {
                continue;
            }
            for cylinder in [HOT, COLD] {
                if let Some(point) = self.bins[bin].point(cylinder, self.face_area) {
                    self.last_loops[cylinder].push(point);
                }
            }
        }
    }

    /// Discards the turn in progress and the history; call when a
    /// reservoir temperature changes.
    pub fn mark_settings_changed(&mut self) {
        self.settings_changed = true;
        self.history_len = 0;
        self.history_next = 0;
        for points in &mut self.last_loops {
            points.clear();
        }
    }

    /// Turns completed and recorded since the fill or the last settings
    /// change.
    pub fn completed_turns(&self) -> u64 {
        self.completed_turns
    }

    /// The most recent turn, if any.
    #[cfg(test)]
    pub fn last_turn(&self) -> Option<TurnSummary> {
        (self.history_len > 0)
            .then(|| self.history[(self.history_next + TURNS_AVERAGED - 1) % TURNS_AVERAGED])
    }

    /// Up to `TURNS_AVERAGED` most recent turns combined (not yet divided;
    /// see `TurnSummary::per_turn`).
    pub fn recent(&self) -> Option<TurnSummary> {
        self.history[..self.history_len]
            .iter()
            .copied()
            .reduce(TurnSummary::combined)
    }

    /// One cylinder's pressure-volume loop for the most recent turn, in
    /// sweep order; empty before the first turn.
    pub fn last_loop(&self, cylinder: usize) -> &[LoopPoint] {
        &self.last_loops[cylinder]
    }

    /// One cylinder's points for the turn in progress, in sweep order,
    /// leaving out the bin still being filled; the last point is the most
    /// recent complete reading.
    pub fn current_trace(&self, cylinder: usize) -> impl Iterator<Item = LoopPoint> + '_ {
        let complete = self.visits.len().saturating_sub(1);
        self.visits[..complete]
            .iter()
            .filter_map(move |&bin| self.bins[bin].point(cylinder, self.face_area))
    }
}

#[cfg(test)]
#[path = "../tests/unit/cycle.rs"]
mod tests;
