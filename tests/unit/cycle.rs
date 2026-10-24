//! Unit tests for `src/cycle.rs`, driven by synthetic substeps.

use super::*;

const FACE_AREA: f64 = 2.0;
const STEPS_PER_TURN: usize = 400;
const KELVIN: (f32, f32) = (1_200.0, 300.0);
const TOLERANCE: f64 = 1e-9;
/// A boundary can fall on either side of the substep that reaches it, so a
/// synthetic turn may be one substep short or long.
const STEP_FRACTION: f64 = 1.5 / STEPS_PER_TURN as f64;

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= STEP_FRACTION * expected.abs()
}

/// Feeds substeps that rotate the crank by `turns` (negative is the
/// heat-engine direction). Cumulative totals grow by `per_turn` for each
/// turn of rotation in either direction, as time only runs forward.
/// Pressure is constant within each half of the turn, so the loop is a
/// rectangle.
struct Driver {
    monitor: CycleMonitor,
    angle: f64,
    totals: CycleTotals,
}

impl Driver {
    fn new() -> Self {
        Self {
            monitor: CycleMonitor::new(FACE_AREA, CycleTotals::default()),
            angle: 0.0,
            totals: CycleTotals::default(),
        }
    }

    fn rotate(&mut self, turns: f64, per_turn: CycleTotals) {
        let steps = (turns.abs() * STEPS_PER_TURN as f64).round() as usize;
        let step = turns.signum() * TAU / STEPS_PER_TURN as f64;
        let fraction = 1.0 / STEPS_PER_TURN as f64;
        for _ in 0..steps {
            self.angle += step;
            let total = |a: f64, b: f64| a + b * fraction;
            self.totals = CycleTotals {
                hot_heat: total(self.totals.hot_heat, per_turn.hot_heat),
                cold_heat: total(self.totals.cold_heat, per_turn.cold_heat),
                hot_entropy: total(self.totals.hot_entropy, per_turn.hot_entropy),
                cold_entropy: total(self.totals.cold_entropy, per_turn.cold_entropy),
                gas_work: total(self.totals.gas_work, per_turn.gas_work),
                gas_energy: total(self.totals.gas_energy, per_turn.gas_energy),
            };
            let crank_angle = self.angle.rem_euclid(TAU);
            // High pressure in the first half of the crank angle.
            let pressure = if crank_angle < std::f64::consts::PI {
                3.0
            } else {
                1.0
            };
            let volume = 10.0 + 5.0 * crank_angle.cos();
            let dt = fraction;
            let totals = self.totals;
            self.monitor.record(
                Substep {
                    total_angle: self.angle,
                    crank_angle: crank_angle as f32,
                    dt,
                    gas_impulse: [pressure * FACE_AREA * dt, 0.0],
                    volume: [volume, volume],
                },
                KELVIN,
                || totals,
            );
        }
    }
}

const PER_TURN: CycleTotals = CycleTotals {
    hot_heat: 100.0,
    cold_heat: -80.0,
    hot_entropy: 25.0,
    cold_entropy: -80.0,
    gas_work: 15.0,
    gas_energy: 5.0,
};

#[test]
fn leaving_the_start_angle_is_not_a_turn() {
    let mut driver = Driver::new();
    driver.rotate(-0.99, PER_TURN);
    assert_eq!(driver.monitor.completed_turns(), 0);
    assert!(driver.monitor.recent().is_none());
    assert!(driver.monitor.last_loop(HOT).is_empty());
    assert!(driver.monitor.current_trace(HOT).count() > 0);
}

#[test]
fn each_full_turn_is_summarized_in_either_direction() {
    for direction in [-1.0, 1.0] {
        let mut driver = Driver::new();
        driver.rotate(direction * 3.5, PER_TURN);
        assert_eq!(driver.monitor.completed_turns(), 3);
        let turn = driver.monitor.last_turn().unwrap();
        assert!(close(turn.hot_heat, PER_TURN.hot_heat), "{turn:?}");
        assert!(close(turn.gas_work, PER_TURN.gas_work), "{turn:?}");
        assert!(close(turn.duration, 1.0), "{turn:?}");
        assert_eq!((turn.hot_kelvin, turn.cold_kelvin), KELVIN);
    }
}

#[test]
fn rocking_back_through_the_same_angle_restarts_the_turn() {
    let mut driver = Driver::new();
    driver.rotate(-1.5, PER_TURN);
    assert_eq!(driver.monitor.completed_turns(), 1);
    // Back across the angle just crossed, then forward again past it.
    driver.rotate(0.75, PER_TURN);
    driver.rotate(-0.75, PER_TURN);
    assert_eq!(driver.monitor.completed_turns(), 1);
    // One full turn from that restart completes the next turn.
    driver.rotate(-1.0, PER_TURN);
    assert_eq!(driver.monitor.completed_turns(), 2);
}

#[test]
fn a_settings_change_discards_the_turn_and_history() {
    let mut driver = Driver::new();
    driver.rotate(-2.25, PER_TURN);
    assert_eq!(driver.monitor.recent().unwrap().turns, 2);
    driver.monitor.mark_settings_changed();
    assert!(driver.monitor.recent().is_none());
    assert!(driver.monitor.last_loop(HOT).is_empty());
    // Finish the turn that straddled the change: it must be dropped.
    driver.rotate(-1.0, PER_TURN);
    assert!(driver.monitor.recent().is_none(), "straddling turn kept");
    driver.rotate(-1.0, PER_TURN);
    assert_eq!(driver.monitor.recent().unwrap().turns, 1);
}

#[test]
fn recent_averages_the_last_turns_only() {
    let mut driver = Driver::new();
    driver.rotate(-(TURNS_AVERAGED as f64 + 2.5), PER_TURN);
    let recent = driver.monitor.recent().unwrap();
    assert_eq!(recent.turns, TURNS_AVERAGED);
    let turn = recent.per_turn();
    assert!(close(turn.hot_heat, PER_TURN.hot_heat), "{turn:?}");
    assert!(close(turn.duration, 1.0), "{turn:?}");
}

#[test]
fn loop_points_are_mean_pressure_and_volume_per_bin() {
    let mut driver = Driver::new();
    driver.rotate(-2.5, PER_TURN);
    let points = driver.monitor.last_loop(HOT);
    assert_eq!(points.len(), LOOP_BINS);
    for point in points {
        assert!(
            (1.0 - TOLERANCE..=3.0 + TOLERANCE).contains(&point.pressure),
            "{point:?}"
        );
        assert!((5.0..=15.0).contains(&point.volume), "{point:?}");
    }
    let mean = points.iter().map(|p| p.pressure).sum::<f64>() / LOOP_BINS as f64;
    assert!((mean - 2.0).abs() < 0.05);
    // The cold piston saw no impulse.
    assert!(
        driver
            .monitor
            .last_loop(COLD)
            .iter()
            .all(|p| p.pressure == 0.0)
    );
}

#[test]
fn loop_work_is_the_enclosed_area() {
    let rectangle = [
        LoopPoint {
            volume: 1.0,
            pressure: 4.0,
        },
        LoopPoint {
            volume: 3.0,
            pressure: 4.0,
        },
        LoopPoint {
            volume: 3.0,
            pressure: 1.0,
        },
        LoopPoint {
            volume: 1.0,
            pressure: 1.0,
        },
    ];
    // Expanding at 4, compressing at 1, over a volume change of 2.
    assert!((loop_work(&rectangle) - 6.0).abs() < TOLERANCE);
    let mut reversed = rectangle;
    reversed.reverse();
    assert!((loop_work(&reversed) + 6.0).abs() < TOLERANCE);
}

#[test]
fn summary_laws_and_limits() {
    let turn = TurnSummary {
        turns: 1,
        hot_heat: 100.0,
        cold_heat: -80.0,
        hot_entropy: 100.0 / 4.0,
        cold_entropy: -80.0,
        gas_work: 15.0,
        gas_energy_change: 5.0,
        duration: 1.0,
        hot_kelvin: 1_200.0,
        cold_kelvin: 300.0,
    };
    assert!(turn.first_law_residual().abs() < TOLERANCE);
    assert!((turn.efficiency().unwrap() - 0.15).abs() < TOLERANCE);
    assert!((turn.carnot_limit() - 0.75).abs() < TOLERANCE);
    assert!((turn.surroundings_entropy_change() - 55.0).abs() < TOLERANCE);
    // dU is 5 out of a throughput of 90: settled.
    assert!(turn.is_settled());
    let warming = TurnSummary {
        gas_energy_change: 20.0,
        ..turn
    };
    assert!(!warming.is_settled());
    let equal = TurnSummary {
        cold_kelvin: 1_200.0,
        ..turn
    };
    assert_eq!(equal.carnot_limit(), 0.0);
    let no_heat = TurnSummary {
        hot_heat: -1.0,
        ..turn
    };
    assert!(no_heat.efficiency().is_none());
}
