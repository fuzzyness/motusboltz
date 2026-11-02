//! Tests for `src/sim/regenerator.rs`: where the screens sit, how they
//! start, and the heat a particle trades with them on its way through.

use macroquad::prelude::Vec3;

use super::*;
use crate::sim::gas::particle_kinetic_energy;
use crate::sim::geometry::DEFAULT_GEOMETRY;

/// Particles the test stacks are sized for.
const PARTICLES: usize = 1_024;
/// Seed for every test stack.
const SEED: u64 = 42;
/// Hot end of a stack put in, in reduced units (1,200 K).
const HOT: f64 = 4.0;
/// Cold end of a stack put in, and the temperature a new stack starts at,
/// in reduced units (300 K).
const COLD: f64 = 1.0;
/// Speed of a particle far hotter than any screen.
const FAST: f32 = 100.0;
/// Speed of a particle far colder than any screen.
const SLOW: f32 = 0.5;
/// Particles sent through in the warming and cooling tests.
const PASSES: usize = 200;
/// Largest energy difference treated as equal, in reduced units.
const TOLERANCE: f64 = 1e-3;

impl Regenerator {
    /// Heat the screens hold above absolute zero.
    fn stored_energy(&self) -> f64 {
        self.temperatures.iter().sum::<f64>() * self.heat_capacity
    }
}

/// Half the thickness of the wall between the bores.
fn half_wall() -> f32 {
    DEFAULT_GEOMETRY.shared_wall_thickness() / 2.0
}

/// A test stack, put in from `HOT` to `COLD` if `enabled`.
fn stack(enabled: bool) -> Regenerator {
    let mut regenerator = Regenerator::new(DEFAULT_GEOMETRY, PARTICLES, COLD, SEED);
    regenerator.set_enabled(enabled, HOT, COLD);
    regenerator
}

/// A particle on the opening's axis that has just moved to `x`, moving
/// along it at `velocity_x`.
fn moved_to(x: f32, velocity_x: f32) -> Particle {
    Particle {
        position: Vec3::new(x, DEFAULT_GEOMETRY.passage_center_y(), 0.0),
        velocity: Vec3::new(velocity_x, 0.0, 0.0),
    }
}

/// Total heat `regenerator` gives `PASSES` particles moving at `speed`
/// across the whole stack.
fn heat_through(regenerator: &mut Regenerator, speed: f32) -> f64 {
    let across = 2.0 * half_wall();
    (0..PASSES)
        .filter_map(|_| {
            let mut particle = moved_to(across, speed);
            regenerator.pass(&mut particle, -across)
        })
        .map(f64::from)
        .sum()
}

#[test]
fn screens_sit_in_order_inside_the_wall_between_the_bores() {
    let regenerator = stack(false);
    let positions: Vec<f32> = regenerator.screens().map(|screen| screen.x).collect();
    assert_eq!(positions.len(), SCREEN_COUNT);
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(positions[0] > -half_wall());
    assert!(positions[SCREEN_COUNT - 1] < half_wall());
}

#[test]
fn a_new_stack_is_out_of_the_opening_at_one_temperature() {
    let regenerator = Regenerator::new(DEFAULT_GEOMETRY, PARTICLES, COLD, SEED);
    assert!(!regenerator.is_enabled());
    let room = (COLD * KELVIN_PER_UNIT) as f32;
    assert!(regenerator.screens().all(|screen| screen.kelvin == room));
}

#[test]
fn a_stack_put_in_runs_from_the_hot_temperature_to_the_cold_one() {
    let regenerator = stack(true);
    assert!(regenerator.is_enabled());
    let kelvin: Vec<f32> = regenerator.screens().map(|screen| screen.kelvin).collect();
    assert_eq!(kelvin[0], (HOT * KELVIN_PER_UNIT) as f32);
    assert_eq!(kelvin[SCREEN_COUNT - 1], (COLD * KELVIN_PER_UNIT) as f32);
    assert!(kelvin.windows(2).all(|pair| pair[0] > pair[1]));
}

#[test]
fn nothing_passes_through_a_stack_out_of_the_opening() {
    let mut regenerator = stack(false);
    let mut particle = moved_to(half_wall(), FAST);
    let before = particle.velocity;
    assert_eq!(regenerator.pass(&mut particle, -half_wall()), None);
    assert_eq!(particle.velocity, before);
    assert_eq!(regenerator.heat(), 0.0);
}

#[test]
fn a_move_that_crosses_no_screen_trades_no_heat() {
    let mut regenerator = stack(true);
    let outside = 2.0 * half_wall();
    let mut particle = moved_to(-outside, FAST);
    assert_eq!(regenerator.pass(&mut particle, -2.0 * outside), None);
    assert_eq!(particle.velocity, Vec3::new(FAST, 0.0, 0.0));
}

/// Either way through, the particle keeps going, and every bit of energy it
/// gains comes out of the screens.
#[test]
fn a_crossing_particle_keeps_its_direction_and_its_heat_leaves_the_screens() {
    for direction in [1.0, -1.0] {
        let mut regenerator = stack(true);
        let stored = regenerator.stored_energy();
        let across = 2.0 * half_wall() * direction;
        let mut particle = moved_to(across, SLOW * direction);
        let before = particle_kinetic_energy(particle.velocity) as f64;
        let heat = regenerator.pass(&mut particle, -across).unwrap() as f64;
        assert_eq!(particle.velocity.x.signum(), direction);
        let after = particle_kinetic_energy(particle.velocity) as f64;
        assert!((after - before - heat).abs() < TOLERANCE);
        assert!((regenerator.heat() - heat).abs() < TOLERANCE);
        assert!((stored - regenerator.stored_energy() - heat).abs() < TOLERANCE);
        assert!(heat > 0.0, "a slow particle takes heat from the screens");
    }
}

#[test]
fn hot_gas_warms_the_screens_and_cold_gas_cools_them() {
    let mut warming = stack(true);
    let first_kelvin = |regenerator: &Regenerator| regenerator.screens().next().unwrap().kelvin;
    let start = first_kelvin(&warming);
    assert!(heat_through(&mut warming, FAST) < 0.0);
    assert!(first_kelvin(&warming) > start);

    let mut cooling = stack(true);
    assert!(heat_through(&mut cooling, SLOW) > 0.0);
    assert!(first_kelvin(&cooling) < start);
}
