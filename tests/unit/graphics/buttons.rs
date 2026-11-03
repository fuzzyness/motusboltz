//! Tests for the Pause and Tracers buttons, `src/graphics/buttons.rs`.

use super::*;
use crate::graphics::outline::tests::outside_block;
use crate::graphics::valves::valve_wheels;
use crate::sim::geometry::DEFAULT_GEOMETRY;

/// Each box with its dome stands a little higher than a valve, big enough
/// to see and press.
#[test]
fn a_box_is_a_little_higher_than_a_valve() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let valve = valve_wheels(shape)[0].center.y - shape.base;
    for box_ in button_boxes(shape) {
        let height = box_.dome_top().y - shape.base;
        assert!(
            height > valve && height < 1.6 * valve,
            "{height} against {valve}"
        );
    }
}

/// The boxes lie on the table beyond the cold end, apart from each other,
/// out of the way of the block seen head on.
#[test]
fn the_boxes_lie_beyond_the_cold_end_apart() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let boxes = button_boxes(shape);
    let buttons = Buttons::build(shape);
    for vertex in &buttons.bodies.vertices {
        let point = vertex.position;
        assert!(point.x > shape.offset + shape.outer, "{point} by the block");
        assert!(point.y >= shape.base - 1e-5);
        let seen_from_above = macroquad::prelude::Vec2::new(point.x, point.z);
        assert!(outside_block(shape.offset, shape.outer, seen_from_above) > 0.5 * shape.bore);
    }
    let reach = BOX_HALF_WIDTH * std::f32::consts::SQRT_2 * shape.bore;
    assert!(boxes[0].base.distance(boxes[1].base) > 2.0 * reach);
    assert_ne!(boxes[0].yaw, boxes[1].yaw);
}

/// A dome sinks while held, and for a moment when its switch flips by key
/// or on screen, but does not stay down; it glows while its switch is on.
#[test]
fn a_dome_sinks_while_held_and_glows_while_on() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let mut buttons = Buttons::build(shape);
    let top = |buttons: &Buttons| {
        buttons.domes[0]
            .vertices
            .iter()
            .map(|vertex| vertex.position.y)
            .fold(f32::MIN, f32::max)
    };
    let unpressed = top(&buttons);
    let depth = PRESS_DEPTH * shape.bore * BOX_SCALE;
    let off = DomeStates::default();
    let held = DomeStates {
        on: [true, false],
        held: [true, false],
    };
    let on = DomeStates {
        on: [true, false],
        held: [false, false],
    };
    buttons.pose(off, 0.0);
    buttons.pose(held, 1.0);
    assert!(
        (unpressed - top(&buttons) - depth).abs() < 1e-5,
        "down while held"
    );
    assert_eq!(buttons.glow, [1.0, 0.0]);
    buttons.pose(on, 2.0);
    assert!((top(&buttons) - unpressed).abs() < 1e-5, "up once let go");
    assert_eq!(buttons.glow, [1.0, 0.0], "still lit");
    // Switched off by key: a quick tap, then up again, unlit.
    buttons.pose(off, 2.0 + PRESS_SECONDS as f64);
    assert!(top(&buttons) < unpressed - depth / 2.0, "tapped");
    buttons.pose(off, 3.0);
    assert!((top(&buttons) - unpressed).abs() < 1e-5);
    assert_eq!(buttons.glow, [0.0, 0.0]);
}

/// Every name is engraved, letter by letter.
#[test]
fn every_letter_of_both_names_has_strokes() {
    for switch in [Switch::Pause, Switch::Tracers] {
        for glyph in name(switch).chars() {
            assert!(!letter(glyph).is_empty(), "{glyph}");
        }
    }
    // Each name fits across its box's side.
    let advance = LETTER_COLUMNS + LETTER_GAP;
    let longest = name(Switch::Tracers).len() as f32 * advance - LETTER_GAP;
    let width = longest * LETTER_HEIGHT / LETTER_ROWS;
    assert!(width < 2.0 * (BOX_HALF_WIDTH - CORNER_RADIUS));
    const { assert!(LETTER_HEIGHT < LID_HEIGHT) };
}
