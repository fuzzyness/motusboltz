//! Tests for the mixing box's gas, `src/sim/mixing.rs`.

use super::*;

/// Seconds a frame lasts in these tests.
const FRAME: f32 = 1.0 / 60.0;

/// Runs `gas` on for `seconds` in frames.
fn run(gas: &mut MixingGas, seconds: f32) {
    for _ in 0..(seconds / FRAME).round() as usize {
        gas.advance(FRAME);
    }
}

/// How many of the balls that started on the hot side are on it now, out
/// of the hot side's count.
fn hot_on_the_left(gas: &MixingGas) -> usize {
    gas.balls()[..BALLS_PER_SIDE]
        .iter()
        .filter(|ball| ball.position.x < 0.0)
        .count()
}

/// The spread of the balls' temperatures about their mean, as a share of
/// it.
fn spread(gas: &MixingGas) -> f32 {
    let warmths: Vec<f32> = gas.balls().iter().map(|ball| ball.warmth).collect();
    let mean = warmths.iter().sum::<f32>() / warmths.len() as f32;
    let variance =
        warmths.iter().map(|w| (w - mean) * (w - mean)).sum::<f32>() / warmths.len() as f32;
    variance.sqrt() / mean
}

/// Sorted, the hot balls are all on the left and the cold on the right,
/// at their first temperatures, with no entropy gained.
#[test]
fn it_starts_sorted() {
    let gas = MixingGas::new();
    assert_eq!(gas.balls().len(), 2 * BALLS_PER_SIDE);
    assert_eq!(hot_on_the_left(&gas), BALLS_PER_SIDE);
    assert!(
        gas.balls()[BALLS_PER_SIDE..]
            .iter()
            .all(|ball| ball.position.x > 0.0)
    );
    let (hot, cold) = MixingGas::first_warmths();
    assert!((hot / cold - HOT_OVER_COLD).abs() < 1e-4);
    assert_eq!(gas.entropy_share(), 0.0);
    assert!(!gas.raised());
}

/// With the divider down, nothing crosses and each side keeps its
/// temperature, and no energy is made or lost.
#[test]
fn the_divider_keeps_the_sides_apart() {
    let mut gas = MixingGas::new();
    let energy = gas.energy();
    run(&mut gas, 5.0);
    assert_eq!(hot_on_the_left(&gas), BALLS_PER_SIDE);
    assert!(
        gas.balls()[BALLS_PER_SIDE..]
            .iter()
            .all(|ball| ball.position.x > 0.0)
    );
    assert!((gas.energy() - energy).abs() < 1e-3 * energy);
    assert!(gas.entropy_share() < 0.1);
}

/// With the divider up, the gases mix and come to one temperature, the
/// entropy rising nearly as far as it can, and no energy is made or lost;
/// dropped again, the divider sorts nothing back out.
#[test]
fn lifted_they_mix_and_never_unmix() {
    let mut gas = MixingGas::new();
    let energy = gas.energy();
    let first_spread = spread(&gas);
    gas.toggle();
    run(&mut gas, 25.0);
    assert!((gas.lift() - 1.0).abs() < 1e-6);
    let mixed = hot_on_the_left(&gas);
    assert!(mixed < BALLS_PER_SIDE * 3 / 4, "{mixed} still on the left");
    assert!((gas.energy() - energy).abs() < 1e-3 * energy);
    assert!(gas.entropy_share() > 0.9, "{}", gas.entropy_share());
    assert!(
        spread(&gas) < first_spread / 2.0,
        "one temperature: {} from {first_spread}",
        spread(&gas)
    );
    gas.toggle();
    run(&mut gas, 10.0);
    assert_eq!(gas.lift(), 0.0);
    assert!(gas.entropy_share() > 0.85, "never unmixed");
    assert!(hot_on_the_left(&gas) < BALLS_PER_SIDE * 3 / 4);
}

/// Sorting puts everything back as it began.
#[test]
fn sorting_starts_it_over() {
    let mut gas = MixingGas::new();
    let first = gas.balls().to_vec();
    gas.toggle();
    run(&mut gas, 2.0);
    gas.sort();
    assert_eq!(gas.balls(), &first[..]);
    assert!(!gas.raised() && gas.lift() == 0.0);
}

/// A long stall runs only a few steps, rather than catching up at once.
#[test]
fn a_stall_is_not_caught_up() {
    let mut gas = MixingGas::new();
    let before = gas.balls().to_vec();
    gas.advance(60.0);
    let moved = gas
        .balls()
        .iter()
        .zip(&before)
        .map(|(now, then)| now.position.distance(then.position))
        .fold(0.0, f32::max);
    assert!(moved < 0.2, "moved {moved}");
}
