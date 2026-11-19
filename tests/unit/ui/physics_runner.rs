//! Tests for `src/ui/physics_runner.rs`.

use super::*;
use crate::sim::geometry::DEFAULT_GEOMETRY;

/// Particles in the engines below.
const COUNT: usize = 600;
/// Seed of the engines below.
const SEED: u64 = 7;
/// A 60 Hz frame.
const FRAME: f32 = 1.0 / 60.0;
/// Frames each test below runs.
const FRAMES: usize = 90;

/// An engine with the hot wall at its hottest, started.
fn engine() -> Box<ConnectedChambers> {
    let mut engine = ConnectedChambers::with_geometry_seed(COUNT, DEFAULT_GEOMETRY, SEED);
    engine.set_hot_kelvin(crate::sim::reservoir::MAX_HOT_KELVIN);
    assert!(engine.start_engine());
    Box::new(engine)
}

/// Asserts that two engines' gas and crank are the same, bit for bit.
fn assert_same(actual: &ConnectedChambers, expected: &ConnectedChambers) {
    assert_eq!(actual.particles.len(), expected.particles.len());
    for (actual, expected) in actual.particles.iter().zip(&expected.particles) {
        assert_eq!(actual.position, expected.position);
        assert_eq!(actual.velocity, expected.velocity);
    }
    assert_eq!(
        actual.power_pistons().angle(),
        expected.power_pistons().angle()
    );
}

/// Steps on the physics thread, with a setting changed between frames as
/// the app does, give the same engine as stepping in place.
#[test]
fn the_thread_steps_exactly_as_in_place() {
    let mut runner = PhysicsRunner::new();
    let mut threaded = engine();
    let mut inline = PhysicsRunner::inline();
    let mut reference = engine();
    for frame in 0..FRAMES {
        let cold = 300.0 - frame as f32;
        for (runner, simulation) in [(&mut runner, &mut threaded), (&mut inline, &mut reference)] {
            simulation.set_cold_kelvin(cold);
            runner.start(simulation, FRAME);
            let ran = runner.finish(simulation).expect("a step ran");
            assert_eq!(ran.model, FRAME);
            assert!(!ran.limited);
        }
    }
    assert_same(&threaded, &reference);
}

/// While the engine is away the app holds the stand-in, and nothing ran
/// is reported where nothing was started.
#[test]
fn the_engine_comes_back_and_nothing_is_reported_twice() {
    let mut runner = PhysicsRunner::new();
    let mut simulation = engine();
    let count = simulation.particles.len();
    runner.start(&mut simulation, FRAME);
    assert!(runner.finish(&mut simulation).is_some());
    assert_eq!(simulation.particles.len(), count);
    assert!(runner.finish(&mut simulation).is_none());
}
