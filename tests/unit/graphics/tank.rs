//! Tests for the helium tank, `src/graphics/tank.rs`.

use super::*;
use crate::graphics::buttons::button_boxes;
use crate::sim::geometry::DEFAULT_GEOMETRY;

/// The block and port of the default engine.
fn parts() -> (BlockShape, Port) {
    (
        BlockShape::new(DEFAULT_GEOMETRY),
        Port::new(DEFAULT_GEOMETRY),
    )
}

/// The bottle stands on the table beyond the cold end, clear of the block
/// and of both buttons, and fits in a draw call.
#[test]
fn the_tank_stands_clear_beyond_the_cold_end() {
    let (shape, port) = parts();
    let place = tank_place(shape, port);
    assert!(place.foot.x > shape.offset + shape.outer);
    assert_eq!(place.foot.y, shape.base);
    for button in button_boxes(shape) {
        let apart = vec3(
            button.base.x - place.foot.x,
            0.0,
            button.base.z - place.foot.z,
        );
        assert!(apart.length() > place.radius + shape.bore, "{apart}");
    }
    assert!(place.gauge.y > shape.base + place.height / 2.0);
    let tank = Tank::build(shape, port);
    assert!(!tank.body.mesh().vertices.is_empty());
}

/// The gauge reads lower the more gas the engine holds, and the needle
/// turns with it within its sweep.
#[test]
fn the_needle_falls_as_the_engine_fills() {
    let empty_engine = tank_share(0);
    let full_engine = tank_share(MAX_PARTICLES);
    assert_eq!(empty_engine, 1.0);
    assert!(full_engine < empty_engine && full_engine > 0.0);
    assert!(needle_turn(full_engine) < needle_turn(empty_engine));
    assert!(needle_turn(1.0).abs() <= NEEDLE_SWEEP / 2.0 + 1e-6);
}

/// Across the slider's range, from the least fill to the most, the needle
/// crosses most of the dial, so each pump shows; and the knob stands on
/// top of the bottle, within its height.
#[test]
fn the_needle_swings_across_the_dial() {
    let swing = needle_turn(tank_share(MIN_PARTICLES)) - needle_turn(tank_share(MAX_PARTICLES));
    assert!(swing >= 0.8 * NEEDLE_SWEEP, "{swing}");
    let (shape, port) = parts();
    let place = tank_place(shape, port);
    assert!((place.knob.x - place.foot.x).abs() < 1e-5);
    assert!(place.knob.y > place.foot.y + 0.8 * place.height);
    assert!(place.knob.y < place.foot.y + place.height);
}
