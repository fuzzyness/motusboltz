//! Tests for the liquid-nitrogen coil and its mist,
//! `src/graphics/nitrogen.rs`.

use macroquad::prelude::Vec2;

use super::*;
use crate::graphics::outline::tests::outside_block;
use crate::sim::geometry::DEFAULT_GEOMETRY;

/// A camera at the front right, looking down at the engine.
const EYE: Vec3 = Vec3::new(30.0, 20.0, 30.0);

/// Facing a camera that looks down at the engine from its front right.
fn facing() -> Facing {
    Facing::along(Vec3::new(-1.0, -0.66, -1.0).normalize())
}

/// The coil around the default block.
fn nitrogen() -> (BlockShape, Nitrogen) {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    (shape, Nitrogen::new(shape))
}

#[test]
fn cold_level_runs_from_none_at_room_temperature_to_full_at_the_bottom() {
    assert_eq!(cold_level(MAX_COLD_KELVIN), 0.0);
    assert_eq!(cold_level(MIN_COLD_KELVIN), 1.0);
    let middle = (MAX_COLD_KELVIN + MIN_COLD_KELVIN) / 2.0;
    assert!((cold_level(middle) - 0.5).abs() < 1e-6);
    assert_eq!(cold_level(400.0), 0.0);
    assert_eq!(cold_level(MIN_COLD_KELVIN - 20.0), 1.0);
}

#[test]
fn the_coil_is_empty_and_still_at_300_k() {
    let (_, mut nitrogen) = nitrogen();
    nitrogen.build(cold_level(MAX_COLD_KELVIN), 3.0, facing(), EYE);
    assert!(nitrogen.liquid.indices.is_empty());
    assert_eq!(nitrogen.mist.len(), 0);
    assert_eq!(nitrogen.bubbles.len(), 0);
    assert!(
        !nitrogen.glass.indices.is_empty(),
        "the glass is always there"
    );
}

/// Below 300 K the liquid fills the whole coil, though not the outlet's
/// two bends, at every setting.
#[test]
fn the_liquid_always_fills_the_whole_coil() {
    let (_, mut nitrogen) = nitrogen();
    let outlet_rings = nitrogen.glass.vertices.len() / (nitrogen.stations.len() + 2);
    for kelvin in [295.0, 230.0, 130.0, MIN_COLD_KELVIN] {
        nitrogen.build(cold_level(kelvin), 0.0, facing(), EYE);
        assert_eq!(
            nitrogen.liquid.vertices.len() + 2 * outlet_rings,
            nitrogen.glass.vertices.len(),
            "{kelvin} K"
        );
        assert!(nitrogen.mist.len() > 0 && nitrogen.bubbles.len() > 0);
    }
}

/// The liquid flows faster and the smoke lingers longer the colder it is.
#[test]
fn colder_flows_faster_and_its_smoke_lingers() {
    assert!(bubble_pace(1.0) > bubble_pace(0.1));
    assert!(bubble_pace(0.0) > 0.0, "it always circulates");
    assert!(puff_rate(1.0) < puff_rate(0.1));
}

/// More smoke shows the colder it is.
#[test]
fn colder_makes_more_smoke() {
    let (_, mut nitrogen) = nitrogen();
    let mut previous = 0;
    for kelvin in [280.0, 200.0, 120.0, MIN_COLD_KELVIN] {
        nitrogen.build(cold_level(kelvin), 1.0, facing(), EYE);
        assert!(nitrogen.mist.len() > previous, "{kelvin} K");
        previous = nitrogen.mist.len();
    }
}

/// A change of setting keeps the bubbles where they are; only their pace
/// changes from then on.
#[test]
fn a_change_of_setting_does_not_move_the_bubbles() {
    let (_, mut nitrogen) = nitrogen();
    nitrogen.build(0.2, 5.0, facing(), EYE);
    let flowed = nitrogen.flowed;
    nitrogen.build(0.9, 5.0, facing(), EYE);
    assert_eq!(nitrogen.flowed, flowed);
    nitrogen.build(0.9, 5.1, facing(), EYE);
    assert!(nitrogen.flowed > flowed);
}

/// The coil runs from the floor up, outside the block on its cold half,
/// starting at the inlet where the valve's pipe joins.
#[test]
fn the_coil_winds_up_the_cold_half_outside_the_block() {
    let (shape, nitrogen) = nitrogen();
    let first = nitrogen.stations[0];
    assert!((first.point.x - coil_inlet_x(shape)).abs() < 1e-4);
    assert_eq!(first.along, 0.0);
    assert_eq!(nitrogen.stations.last().unwrap().along, 1.0);
    for pair in nitrogen.stations.windows(2) {
        assert!(pair[1].along >= pair[0].along);
    }
    for vertex in &nitrogen.glass.vertices {
        let position = vertex.position;
        let seen_from_above = Vec2::new(position.x, position.z);
        assert!(outside_block(shape.offset, shape.outer, seen_from_above) > 0.0);
        assert!(position.x > 0.0, "{position} is on the hot half");
        assert!(position.y > shape.base);
    }
}

/// Bubbles ride the liquid, but not where the coil lines up with the
/// block's window: seen from the front right, some go, whether in front of
/// the window or behind it.
#[test]
fn bubbles_leave_the_window_clear() {
    let (shape, mut nitrogen) = nitrogen();
    nitrogen.build(0.5, 7.3, facing(), EYE);
    let seen = nitrogen.bubbles.len();
    assert!(seen > 0 && seen < BUBBLES as usize, "{seen}");
    let [_, middle, _, _] = shape.window();
    let toward_front = Vec3::new(0.0, 10.0, 40.0);
    for z in [shape.outer, -shape.outer] {
        let point = Vec3::new(shape.offset, middle, z);
        assert!(in_window(shape, toward_front, point, 0.0), "{point}");
        assert!(!in_window(
            shape,
            toward_front,
            point + Vec3::Y * 100.0,
            0.0
        ));
    }
    let past_the_end = Vec3::new(shape.offset + shape.outer + shape.bore, middle, 0.0);
    assert!(!in_window(shape, toward_front, past_the_end, 0.0));
}

/// The coil ends on its top pass, with no vent rising above it, and the
/// smoke stays close to the pipes.
#[test]
fn the_coil_ends_on_its_top_pass_and_its_smoke_stays_near_it() {
    let (shape, mut nitrogen) = nitrogen();
    let (first, last) = (nitrogen.stations[0], *nitrogen.stations.last().unwrap());
    let top = first.point.y + (PASSES - 1) as f32 * PASS_SPACING * shape.bore;
    assert!((last.point.y - top).abs() < 1e-4);
    nitrogen.build(1.0, 2.5, facing(), EYE);
    assert!(nitrogen.mist.len() > 0);
    const { assert!(PUFF_SIZE_OLD + PUFF_SPREAD + DRIFT < 0.5, "a small wisp") };
}

/// The outlet leaves the coil's end and turns down to a mouth near the
/// floor, and smoke from it stays low and close.
#[test]
fn the_outlet_turns_down_and_its_smoke_stays_low() {
    let (shape, mut nitrogen) = nitrogen();
    let [end, elbow, mouth] = nitrogen.outlet.map(|station| station.point);
    assert_eq!(end, nitrogen.stations.last().unwrap().point);
    assert!((elbow.y - end.y).abs() < 1e-5, "it steps out level");
    assert!(
        mouth.y < end.y && mouth.y > shape.base,
        "then down to the floor"
    );
    const { assert!(OUTLET_SPREAD < 0.2 && OUTLET_SINK < 0.5, "a slow pool") };
    nitrogen.build(1.0, 4.0, facing(), EYE);
    assert!(nitrogen.mist.len() > 0);
}
