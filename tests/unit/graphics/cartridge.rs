//! Tests for the regenerator's cartridge, `src/graphics/cartridge.rs`.

use super::*;
use crate::graphics::outline::tests::outside_block;
use crate::graphics::valves::valve_wheels;
use crate::sim::geometry::DEFAULT_GEOMETRY;

/// It lies flat on the table beyond the hot end, clear of the block, and
/// ends its trip seated in the opening, along the shaft.
#[test]
fn it_lies_beyond_the_hot_end_and_ends_in_the_opening() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let (start, flat) = pose(shape, 0.0);
    assert_eq!(start, rest(shape));
    assert!(flat.abs_diff_eq(Quat::IDENTITY, 1e-6));
    let radius = FIT * shape.opening;
    let seen_from_above = macroquad::prelude::Vec2::new(start.x, start.z);
    assert!(start.x < 0.0);
    assert!(outside_block(shape.offset, shape.outer, seen_from_above) > radius + 0.5 * shape.bore);
    let (end, turned) = pose(shape, 1.0);
    assert!(end.distance(seat(shape)) < 1e-5);
    assert!((turned * Vec3::Y).abs_diff_eq(Vec3::X, 1e-5));
}

/// On its way it passes over the gas valve, not through it.
#[test]
fn it_flies_over_the_gas_valve() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let wheel = valve_wheels(shape)[0];
    let clearance = FIT * shape.opening + wheel.radius;
    for step in 0..=200 {
        let (center, _) = pose(shape, step as f32 / 200.0);
        let level = (center.x - wheel.center.x).hypot(center.z - wheel.center.z);
        if level < clearance {
            assert!(center.y > wheel.center.y + clearance / 2.0, "{center}");
        }
    }
}

/// It moves along its trip at a steady pace, turns back when asked, and
/// counts as seated only all the way in.
#[test]
fn it_follows_the_regenerator_over_time() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let mut cartridge = Cartridge::build(shape, false);
    cartridge.follow(true, 10.0);
    assert_eq!(cartridge.progress, 0.0, "no clock on the first frame");
    cartridge.follow(true, 10.0 + TRIP_SECONDS as f64 / 2.0);
    assert!((cartridge.progress - 0.5).abs() < 1e-4);
    assert!(!cartridge.seated());
    cartridge.follow(false, 10.0 + TRIP_SECONDS as f64 * 0.75);
    assert!((cartridge.progress - 0.25).abs() < 1e-4);
    cartridge.follow(true, 20.0);
    assert!(cartridge.seated());
    assert!(Cartridge::build(shape, true).seated());
}

/// Seated, it fills the opening's length through the shared wall, inside
/// the opening's radius: the same cartridge as on the table.
#[test]
fn seated_it_fills_the_opening() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let wall = DEFAULT_GEOMETRY.shared_wall_thickness();
    assert!((THICKNESS * shape.bore - wall).abs() < 1e-4);
    let (center, turn) = pose(shape, 1.0);
    let built = build(shape);
    for vertex in &built.vertices {
        let placed = center + turn * vertex.position;
        assert!(placed.x.abs() <= wall / 2.0 + 1e-4, "{placed}");
        let from_axis = (placed - center).with_x(0.0).length();
        assert!(from_axis < shape.opening, "{placed}");
    }
}
