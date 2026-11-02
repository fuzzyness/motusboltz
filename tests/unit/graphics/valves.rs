//! Tests for the gas and nitrogen valves, `src/graphics/valves.rs`.

use macroquad::prelude::Vec2;

use super::*;
use crate::sim::geometry::DEFAULT_GEOMETRY;

/// The valves of the default block, without a material.
fn valves() -> (BlockShape, Valves) {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    (shape, Valves::build(shape))
}

#[test]
fn each_valve_stands_in_front_of_its_own_half() {
    let (shape, valves) = valves();
    let [hot, cold] = valves.wheels;
    assert_eq!(
        (hot.side, cold.side),
        (ReservoirSide::Hot, ReservoirSide::Cold)
    );
    assert!(hot.center.x < 0.0 && cold.center.x > 0.0);
    for wheel in [hot, cold] {
        assert!(
            wheel.center.z - wheel.radius > shape.outer,
            "clear of the block"
        );
        assert!(wheel.center.y > shape.base);
    }
}

/// Turning a wheel moves only the wheel, about its own stem.
#[test]
fn a_turn_moves_only_the_wheel_about_its_stem() {
    let (_, mut valves) = valves();
    valves.pose((0.0, 0.0));
    let shut = valves.mesh.vertices.clone();
    valves.pose((1.0, 0.0));
    let wheel_start = valves.starts[0] + valves.built[0].wheel_start;
    let center = valves.wheels[0].center;
    for (index, (before, after)) in shut.iter().zip(&valves.mesh.vertices).enumerate() {
        let (before, after) = (before.position, after.position);
        let on_hot_wheel = (wheel_start..valves.starts[1]).contains(&index);
        if on_hot_wheel {
            let radius = |point: Vec3| Vec2::new(point.x - center.x, point.z - center.z).length();
            assert!((radius(after) - radius(before)).abs() < 1e-4);
            assert!((after.y - before.y).abs() < 1e-5);
        } else {
            assert_eq!(after, before, "vertex {index} moved");
        }
    }
    let first = valves.mesh.vertices[wheel_start].position;
    assert!(
        first.distance(shut[wheel_start].position) > 1e-3,
        "the wheel turned"
    );
}
