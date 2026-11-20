//! Tests for the desk clock's time, `src/ui/desk_clock.rs`.

use std::f32::consts::{
    FRAC_PI_2,
    PI, //
};

use super::*;

/// Close enough for angles.
const CLOSE: f32 = 1e-5;

/// At 3:00 the hour hand points at 3 and the minute hand at 12; at 6:30
/// the hour hand is halfway between 6 and 7.
#[test]
fn the_hands_tell_the_time() {
    let three = hand_angles(3.0 * SECONDS_PER_HOUR);
    assert!((three.hour - FRAC_PI_2).abs() < CLOSE);
    assert!(three.minute.abs() < CLOSE && three.second.abs() < CLOSE);
    let half_six = hand_angles(6.5 * SECONDS_PER_HOUR);
    assert!((half_six.hour - (PI + PI / 12.0)).abs() < CLOSE);
    assert!((half_six.minute - PI).abs() < CLOSE);
    // 15:00 shows as 3:00, and a day later the same.
    let afternoon = hand_angles(15.0 * SECONDS_PER_HOUR + SECONDS_PER_DAY);
    assert!((afternoon.hour - three.hour).abs() < CLOSE);
}

/// The second hand ticks once a second rather than sweeping.
#[test]
fn the_second_hand_ticks() {
    let tick = std::f32::consts::TAU / 60.0;
    assert!((hand_angles(15.2).second - 15.0 * tick).abs() < CLOSE);
    assert!((hand_angles(15.9).second - 15.0 * tick).abs() < CLOSE);
    assert!((hand_angles(16.0).second - 16.0 * tick).abs() < CLOSE);
}

/// In slow motion the clock falls behind, paused it stops, and at full
/// speed it catches up and then keeps time.
#[test]
fn the_clock_falls_behind_and_catches_up() {
    let mut clock = DeskClock::new();
    clock.advance(10.0, 1.0);
    assert_eq!(clock.lag(), 0.0);
    clock.advance(10.0, 0.25);
    assert!((clock.lag() - 7.5).abs() < 1e-9);
    clock.advance(2.0, 0.0);
    assert!((clock.lag() - 9.5).abs() < 1e-9);
    // Back at full speed it makes up the lag, faster the more there is.
    let before = clock.lag();
    clock.advance(0.1, 1.0);
    let quick = before - clock.lag();
    assert!(quick > 0.1 * MIN_CATCH_UP_RATE);
    for _ in 0..600 {
        clock.advance(1.0 / 60.0, 1.0);
    }
    assert_eq!(clock.lag(), 0.0);
}

/// A long pause is made up in a few seconds, not as long again.
#[test]
fn a_long_pause_is_made_up_quickly() {
    let mut clock = DeskClock::new();
    clock.advance(600.0, 0.0);
    let mut seconds = 0.0;
    while clock.lag() > 0.0 {
        clock.advance(1.0 / 60.0, 1.0);
        seconds += 1.0 / 60.0;
    }
    assert!(seconds < 15.0, "{seconds}");
}

/// The hands show the computer's time less the lag.
#[test]
fn the_hands_show_the_time_less_the_lag() {
    let mut clock = DeskClock::new();
    let utc = 1_700_000_000.0;
    let on_time = clock.hands(utc);
    clock.advance(SECONDS_PER_MINUTE, 0.0);
    let behind = clock.hands(utc);
    let minute = std::f32::consts::TAU / 60.0;
    let difference = (on_time.minute - behind.minute).rem_euclid(std::f32::consts::TAU);
    assert!((difference - minute).abs() < 1e-4, "{difference}");
}

/// A quick press is a click; one held past `LONG_PRESS_SECONDS` is long
/// once, and letting go after it does nothing more.
#[test]
fn a_press_is_a_click_or_a_long_press() {
    let mut clock = DeskClock::new();
    assert!(!clock.hold(1.0));
    assert_eq!(clock.release(), None);
    clock.press();
    assert!(clock.pressed());
    assert!(!clock.hold(LONG_PRESS_SECONDS / 2.0));
    assert_eq!(clock.release(), Some(PressEnd::Click));
    assert!(!clock.pressed());
    clock.press();
    assert!(!clock.hold(LONG_PRESS_SECONDS * 0.9));
    assert!(clock.hold(LONG_PRESS_SECONDS * 0.2));
    assert!(!clock.hold(1.0));
    assert_eq!(clock.release(), Some(PressEnd::AfterLong));
}

/// The computer's time zone is a whole number of quarter hours from UTC,
/// within a day.
#[test]
fn the_time_zone_is_sensible() {
    let offset = zone_offset(1_700_000_000.0);
    assert!(offset.abs() <= 14.0 * SECONDS_PER_HOUR, "{offset}");
    assert_eq!(offset.rem_euclid(15.0 * SECONDS_PER_MINUTE), 0.0);
}
