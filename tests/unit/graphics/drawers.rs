//! Tests for the table's drawers, `src/graphics/drawers.rs`.

use super::*;
use crate::graphics::table::{
    TABLE_HEIGHT,
    top_height, //
};
use crate::sim::geometry::DEFAULT_GEOMETRY;

/// Highest the figure jumps, in bore radii, from `first_person`.
const JUMP: f32 = 1.8;

/// The drawers fill the cabinet from its plinth to the table's top, each
/// the same height, every step from the ground to the first, from one to
/// the next and from the last to the table's top a plain jump.
#[test]
fn the_drawers_are_steps_a_jump_apart() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let unit = shape.bore;
    let cabinet = cabinet_place(shape);
    let places = drawer_places(shape);
    let mut below = cabinet.bottom;
    for place in places {
        assert!(place.bottom > below - 1e-4, "stacked");
        assert!(place.top - below < JUMP * unit, "a jump up from below");
        assert!(place.left > cabinet.left && place.right < cabinet.right);
        assert!(place.back > cabinet.back && place.front == cabinet.front);
        assert!(place.most > 0.0 && place.most < place.front - place.back);
        below = place.top;
    }
    assert!(below < cabinet.top, "under the table's top");
    assert!(
        top_height(shape) - below < JUMP * unit,
        "a jump up onto the top"
    );
    assert!((cabinet.top - cabinet.bottom) < TABLE_HEIGHT * unit);
}

/// A drawer pulls out only so far, and pushes in only to shut, each on
/// its own.
#[test]
fn a_drawer_pulls_out_and_pushes_in_within_its_runners() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let places = drawer_places(shape);
    let mut drawers = Drawers {
        places,
        pulls: [0.0; DRAWER_COUNT],
        boxes: Vec::new(),
        wood: None,
    };
    drawers.pull(2, shape.bore);
    assert_eq!(drawers.pulls()[2], shape.bore);
    assert_eq!(drawers.pulls()[1], 0.0);
    drawers.pull(2, 100.0 * shape.bore);
    assert_eq!(drawers.pulls()[2], places[2].most);
    drawers.pull(2, -1000.0 * shape.bore);
    assert_eq!(drawers.pulls()[2], 0.0);
    drawers.pull(DRAWER_COUNT, shape.bore);
    let grip = places[0].grip(shape.bore);
    assert!((grip.z - (places[0].front + shape.bore)).abs() < 1e-5);
}

/// The pedestal stands at the desk's right end, right of the engine's
/// middle, from its back to its front, under the top's overhang.
#[test]
fn the_pedestal_stands_at_the_right_end() {
    use crate::graphics::table::{
        OVERHANG,
        TOP_HALF_LENGTH, //
    };
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let cabinet = cabinet_place(shape);
    let unit = shape.bore;
    assert!(cabinet.left > 0.0);
    assert!((cabinet.right - (TOP_HALF_LENGTH - OVERHANG) * unit).abs() < 1e-5);
    assert!((cabinet.back + TOP_HALF_DEPTH * unit).abs() < 1e-5);
    assert!(cabinet.front < TOP_HALF_DEPTH * unit);
}

/// Each drawer's solid has its raised panel and knob standing out from
/// its front.
#[test]
fn a_drawer_has_a_raised_panel_and_a_knob() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let place = drawer_places(shape)[0];
    let solid = drawer_solid(place, shape.bore);
    let foremost = solid
        .vertices
        .iter()
        .map(|vertex| vertex.position.z)
        .fold(f32::MIN, f32::max);
    assert!(foremost > place.front + (PANEL_RISE + 0.5 * KNOB_REACH) * shape.bore);
}
