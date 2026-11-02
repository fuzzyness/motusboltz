//! Per-turn bookkeeping of the engine cycle: heat, work, entropy flow and a
//! pressure-volume loop, measured between successive full turns of the
//! crank.
//!
//! Comparing whole turns matters. The pistons return to the same positions
//! every turn, so compression and back-pressure work cancel over a turn and
//! the first and second laws can be checked honestly. A partial turn mixes
//! different compression phases and is never reported.
//!
//! Everything here is in reduced simulation units; the engine converts to
//! SI.

use std::f64::consts::TAU;

/// Pressure-volume samples per turn, binned by crank angle.
///
/// Each bin averages the pressure over 10 degrees of crank rotation;
/// narrower bins show mostly the noise of single particle impacts.
pub const LOOP_BINS: usize = 36;
/// Most recent turns combined in `CycleMonitor::recent`.
pub const TURNS_AVERAGED: usize = 3;
/// Largest change in the gas's energy, as a share of the heat flowing
/// through it, for turns to count as settled.
const SETTLED_FRACTION: f64 = 0.10;
/// Bound on the visit list when the shaft rocks back and forth in one turn.
const MAX_VISITS: usize = 4 * LOOP_BINS;
/// Index of the hot cylinder in per-cylinder arrays.
pub const HOT: usize = 0;
/// Index of the cold cylinder in per-cylinder arrays.
pub const COLD: usize = 1;

/// Running totals read from the simulation at a turn boundary, or their
/// changes over some turns.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CycleTotals {
    /// Heat into the gas from the hot reservoir.
    pub hot_heat: f64,
    /// Heat into the gas from the cold reservoir.
    pub cold_heat: f64,
    /// Heat into the gas from the regenerator's screens; negative when they
    /// stored heat.
    pub regenerator_heat: f64,
    /// Heat from the hot reservoir over its temperature.
    pub hot_entropy: f64,
    /// Heat from the cold reservoir over its temperature.
    pub cold_entropy: f64,
    /// Work done by the gas on the pistons.
    pub gas_work: f64,
    /// Kinetic energy of the gas, which is its internal energy.
    pub gas_energy: f64,
}

impl CycleTotals {
    /// Combines each quantity of `self` with the same one of `other`.
    fn zip(self, other: Self, combine: impl Fn(f64, f64) -> f64) -> Self {
        Self {
            hot_heat: combine(self.hot_heat, other.hot_heat),
            cold_heat: combine(self.cold_heat, other.cold_heat),
            regenerator_heat: combine(self.regenerator_heat, other.regenerator_heat),
            hot_entropy: combine(self.hot_entropy, other.hot_entropy),
            cold_entropy: combine(self.cold_entropy, other.cold_entropy),
            gas_work: combine(self.gas_work, other.gas_work),
            gas_energy: combine(self.gas_energy, other.gas_energy),
        }
    }
}

/// Changes over one or more complete turns at fixed reservoir temperatures.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TurnSummary {
    /// Turns combined.
    pub turns: usize,
    /// Changes over those turns; `gas_energy` is the change in the gas's
    /// internal energy, dU.
    pub change: CycleTotals,
    /// Model time taken.
    pub duration: f64,
    /// Hot reservoir temperature throughout.
    pub hot_kelvin: f32,
    /// Cold reservoir temperature throughout.
    pub cold_kelvin: f32,
}

impl TurnSummary {
    /// The same quantities averaged per turn.
    pub fn per_turn(self) -> Self {
        let turns = self.turns.max(1) as f64;
        Self {
            turns: 1,
            change: self.change.zip(self.change, |value, _| value / turns),
            duration: self.duration / turns,
            ..self
        }
    }

    /// First-law residual, ideally zero: Q hot + Q cold - W - dU, where dU
    /// is the engine's (see `engine_energy_change`).
    pub fn first_law_residual(self) -> f64 {
        let change = self.change;
        change.hot_heat + change.cold_heat - change.gas_work - self.engine_energy_change()
    }

    /// Work out over heat in from the hot wall, or `None` when the hot wall
    /// gave no net heat.
    pub fn efficiency(self) -> Option<f64> {
        (self.change.hot_heat > 0.0).then(|| self.change.gas_work / self.change.hot_heat)
    }

    /// The Carnot limit 1 - Tc/Th for these reservoir temperatures.
    pub fn carnot_limit(self) -> f64 {
        (1.0 - self.cold_kelvin as f64 / self.hot_kelvin as f64).max(0.0)
    }

    /// Entropy change of both reservoirs, -(Qh/Th + Qc/Tc).
    ///
    /// The regenerator's screens are part of the engine, like the gas, not
    /// of its surroundings.
    pub fn surroundings_entropy_change(self) -> f64 {
        -(self.change.hot_entropy + self.change.cold_entropy)
    }

    /// The change in the engine's own energy: the gas's, and the
    /// regenerator's screens', which lose what they give the gas.
    pub fn engine_energy_change(self) -> f64 {
        self.change.gas_energy - self.change.regenerator_heat
    }

    /// Whether the gas and the regenerator's screens each ended the turns
    /// with nearly the energy they started with.
    ///
    /// Only then is the engine's own entropy change negligible, so the
    /// surroundings' change is the change of the universe.
    pub fn is_settled(self) -> bool {
        let change = self.change;
        let throughput = (change.hot_heat.abs() + change.cold_heat.abs()) / 2.0;
        let limit = SETTLED_FRACTION * throughput;
        throughput > 0.0
            && change.gas_energy.abs() <= limit
            && change.regenerator_heat.abs() <= limit
    }
}

/// One point of a pressure-volume loop.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LoopPoint {
    /// Gas volume, in cubic model units.
    pub volume: f64,
    /// Gas pressure, in reduced units.
    pub pressure: f64,
}

/// Work enclosed by a closed loop, the trapezoid sum of P dV around it.
///
/// It is positive when the gas expands at higher pressure than it is
/// compressed.
pub fn loop_work(points: &[LoopPoint]) -> f64 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| 0.5 * (a.pressure + b.pressure) * (b.volume - a.volume))
        .sum()
}

/// Gas impulse and volume for each cylinder, gathered over one crank-angle
/// bin along with the time spent in it.
#[derive(Clone, Copy, Debug, Default)]
struct LoopBin {
    /// Upward impulse on each piston face.
    impulse: [f64; 2],
    /// Each cylinder's volume integrated over time.
    volume_time: [f64; 2],
    /// Model time spent in the bin.
    time: f64,
}

impl LoopBin {
    /// Mean volume of `cylinder` and mean gas pressure on its piston.
    fn point(self, cylinder: usize, face_area: f64) -> Option<LoopPoint> {
        (self.time > 0.0).then(|| LoopPoint {
            volume: self.volume_time[cylinder] / self.time,
            pressure: self.impulse[cylinder] / (self.time * face_area),
        })
    }
}

/// What the simulation reports after each integration substep.
///
/// Arrays are indexed by `HOT` and `COLD`.
#[derive(Clone, Copy, Debug)]
pub struct Substep {
    /// Signed crank rotation since the fill, in radians.
    pub total_angle: f64,
    /// Crank angle reduced to 0..2 pi.
    pub crank_angle: f32,
    /// Model time the substep took.
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
/// changed is discarded, along with the history, so averages always use one
/// setting.
pub struct CycleMonitor {
    /// Area of one piston face, in square model units.
    face_area: f64,
    /// Whole turns of the crank since the fill, rounded down.
    turn_index: i64,
    /// The whole-turn angle last crossed, in turns.
    last_boundary: i64,
    /// Totals when the turn in progress began.
    start: CycleTotals,
    /// Model time since the turn in progress began.
    elapsed: f64,
    /// A reservoir temperature changed during the turn in progress.
    settings_changed: bool,
    /// The most recent turns, as a ring.
    history: [TurnSummary; TURNS_AVERAGED],
    /// Turns held in `history`.
    history_len: usize,
    /// Where the next turn goes in `history`.
    history_next: usize,
    /// Turns recorded since the fill.
    completed_turns: u64,
    /// The turn in progress, binned by crank angle.
    bins: [LoopBin; LOOP_BINS],
    /// Bins in the order the crank swept them this turn.
    visits: Vec<usize>,
    /// Each cylinder's loop for the last complete turn.
    last_loops: [Vec<LoopPoint>; 2],
}

impl CycleMonitor {
    /// Starts at crank angle zero with the totals `start`, for pistons of
    /// face area `face_area`.
    ///
    /// # Panics
    ///
    /// Panics if `face_area` is not positive and finite.
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

    /// Records a substep at reservoir temperatures `kelvin` (hot, cold).
    ///
    /// `totals` is called only at a turn boundary, so the full gas energy
    /// sum is not computed every substep.
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
        // Moving backward crosses the boundary at the old index; forward, at
        // the new one.
        let boundary = self.turn_index.max(index);
        self.turn_index = index;
        let end = totals();
        if boundary != self.last_boundary && !self.settings_changed {
            self.history[self.history_next] = TurnSummary {
                turns: 1,
                change: end.zip(self.start, |end, start| end - start),
                duration: self.elapsed,
                hot_kelvin: kelvin.0,
                cold_kelvin: kelvin.1,
            };
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

    /// Builds each cylinder's loop from the bins of the finished turn, in
    /// the order the crank swept them.
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

    /// Discards the turn in progress and the history; call when a reservoir
    /// temperature changes.
    pub fn mark_settings_changed(&mut self) {
        self.settings_changed = true;
        self.history_len = 0;
        self.history_next = 0;
        for points in &mut self.last_loops {
            points.clear();
        }
    }

    /// Turns recorded since the fill.
    pub fn completed_turns(&self) -> u64 {
        self.completed_turns
    }

    /// Up to `TURNS_AVERAGED` most recent turns combined, not yet divided
    /// (see `TurnSummary::per_turn`).
    pub fn recent(&self) -> Option<TurnSummary> {
        self.history[..self.history_len]
            .iter()
            .copied()
            .reduce(|a, b| TurnSummary {
                turns: a.turns + b.turns,
                change: a.change.zip(b.change, |a, b| a + b),
                duration: a.duration + b.duration,
                ..a
            })
    }

    /// The pressure-volume loop of `cylinder` for the most recent turn, in
    /// sweep order; empty before the first turn.
    pub fn last_loop(&self, cylinder: usize) -> &[LoopPoint] {
        &self.last_loops[cylinder]
    }

    /// The points of `cylinder` for the turn in progress, in sweep order.
    ///
    /// The bin still being filled is left out, so the last point is the most
    /// recent complete reading.
    pub fn current_trace(&self, cylinder: usize) -> impl Iterator<Item = LoopPoint> + '_ {
        let complete = self.visits.len().saturating_sub(1);
        self.visits[..complete]
            .iter()
            .filter_map(move |&bin| self.bins[bin].point(cylinder, self.face_area))
    }
}

#[cfg(test)]
#[path = "../../tests/unit/sim/cycle.rs"]
mod tests;
