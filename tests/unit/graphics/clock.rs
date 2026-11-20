//! Tests for the desk clock, `src/graphics/clock.rs`.

use super::*;
use crate::graphics::cartridge;
use crate::graphics::outline::tests::outside_block;
use crate::graphics::table::over_top;
use crate::sim::geometry::DEFAULT_GEOMETRY;

/// The clock stands on the table beyond the hot end, clear of the block
/// seen from above, and behind the cartridge.
#[test]
fn the_clock_stands_on_the_table_beyond_the_hot_end() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let clock = Clock::build(shape);
    for vertex in &clock.body.mesh().vertices {
        let point = vertex.position;
        assert!(
            point.x < -(shape.offset + shape.outer),
            "{point} by the block"
        );
        assert!(point.y >= shape.base - 1e-4, "{point} under the table");
        let seen_from_above = Vec2::new(point.x, point.z);
        assert!(over_top(shape, seen_from_above), "{point} off the table");
        assert!(outside_block(shape.offset, shape.outer, seen_from_above) > 0.5 * shape.bore);
    }
    let place = clock.place();
    assert!(
        place.base.z < cartridge::rest(shape).z,
        "in front of the cartridge"
    );
}

/// The clock is about as tall as the engine's block, and far shorter
/// than the engine, with its handle at the top.
#[test]
fn the_clock_is_desk_sized() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let place = clock_place(shape);
    let height = place.height();
    let block = shape.top - shape.base;
    assert!(
        height > 0.6 * block && height < 1.6 * block,
        "{height} against {block}"
    );
    let top = Clock::build(shape)
        .body
        .mesh()
        .vertices
        .iter()
        .map(|vertex| vertex.position.y)
        .fold(f32::MIN, f32::max);
    let handle_top = shape.base + height + HANDLE_RADIUS * place.unit;
    assert!(
        (top - handle_top).abs() < 1e-3 * height,
        "{top} against {handle_top}"
    );
}

/// The dial faces the usual view, from the front and the right, and the
/// point to press is on it.
#[test]
fn the_dial_faces_the_usual_view() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let place = clock_place(shape);
    let facing = place.facing();
    assert!(facing.z > 0.5 && facing.x > 0.2, "{facing}");
    assert!(place.face().distance(place.middle()) < place.radius());
    assert!((place.face() - place.middle()).dot(facing) > 0.0);
}

/// The hands turn clockwise about the dial's middle: a hand at a quarter
/// turn points to 3, to the dial's right as seen from the front.
#[test]
fn a_quarter_turn_points_a_hand_at_three() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let place = clock_place(shape);
    let tip = Vec3::Y;
    let at = dial_center(place.unit);
    let turned = place
        .placing(at, std::f32::consts::FRAC_PI_2)
        .transform_point3(tip);
    let three = on_dial(
        place,
        std::f32::consts::FRAC_PI_2,
        1.0 / dial_radius(place.unit),
    );
    assert!(turned.distance(three) < 1e-4, "{turned} against {three}");
}

/// The handle loops up between the bells, its ends on the case.
#[test]
fn the_handle_arches_over_the_case() {
    let unit = 1.0;
    let path = handle_path(unit);
    let center = case_center(unit);
    let (first, last) = (path[0], path[path.len() - 1]);
    let flat = |point: Vec3| Vec2::new(point.x - center.x, point.y - center.y).length();
    assert!(flat(first) < CASE_RADIUS * unit && flat(last) < CASE_RADIUS * unit);
    let top = path.iter().map(|point| point.y).fold(f32::MIN, f32::max);
    assert!((top - (center.y + HANDLE_TOP * unit)).abs() < 1e-4);
}

/// The hammer stands still until rung, then rattles and settles.
#[test]
fn the_hammer_rattles_when_rung_and_settles() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let mut clock = Clock::build(shape);
    assert_eq!(clock.swing(), 0.0);
    clock.ring();
    clock.ringing -= 1.0 / (4.0 * RING_RATE);
    assert!(clock.swing().abs() > 0.5 * RING_SWING);
    clock.ringing = 0.0;
    assert_eq!(clock.swing(), 0.0);
}

/// Every numeral's digits have strokes.
#[test]
fn every_digit_is_drawn() {
    for glyph in '0'..='9' {
        assert!(!digit(glyph).is_empty(), "{glyph}");
    }
}
