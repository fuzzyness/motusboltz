//! Tests for what stands in the figure's way, `src/ui/walking.rs`.

use super::*;
use crate::sim::geometry::DEFAULT_GEOMETRY;
use crate::sim::port::Port;
use crate::ui::first_person::{
    BODY_RADIUS,
    Walker, //
};
use crate::ui::input::FirstPersonKeys;

/// The table under the default engine, with the cartridge on it or not.
fn map(seated: bool) -> (BlockShape, WalkMap) {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    (
        shape,
        WalkMap::new(shape, Port::new(DEFAULT_GEOMETRY), seated),
    )
}

/// A round pushes out from its middle, and a slab from its line.
#[test]
fn shapes_measure_from_their_edges() {
    let round = Footprint::Round {
        center: vec2(1.0, 1.0),
        radius: 0.5,
    };
    let (distance, way) = round.distance(vec2(3.0, 1.0));
    assert!((distance - 1.5).abs() < 1e-6 && way == Vec2::X);
    let slab = Footprint::Slab {
        from: vec2(-1.0, 0.0),
        to: vec2(1.0, 0.0),
        radius: 0.5,
    };
    let (distance, way) = slab.distance(vec2(0.3, -0.25));
    assert!((distance + 0.25).abs() < 1e-6, "inside by a quarter");
    assert!(way.distance(-Vec2::Y) < 1e-6, "{way}");
    let (distance, _) = slab.distance(vec2(3.0, 0.0));
    assert!((distance - 1.5).abs() < 1e-6, "past its end");
}

/// The figure starts clear of everything, on the table.
#[test]
fn the_start_is_clear() {
    let (shape, map) = map(false);
    let feet = Walker::standing(shape).feet();
    let at = vec2(feet.x, feet.z);
    assert!(map.on_table(at));
    assert!(map.clearance(at, BODY_RADIUS * shape.bore, shape.base) > 0.0);
}

/// Everything on the table but the cartridge blocks the way at the
/// table's height: the block, the valves, the boxes, the tank and the
/// clock; the cartridge, there only while it lies there, is stepped onto.
#[test]
fn the_parts_on_the_table_block_the_way() {
    let (shape, map) = map(false);
    let level = |point: Vec3| vec2(point.x, point.z);
    let radius = BODY_RADIUS * shape.bore;
    let blocked = |point: Vec2| map.clearance(point, radius, shape.base) < 0.0;
    assert!(blocked(Vec2::ZERO));
    assert!(blocked(vec2(shape.offset, shape.outer)));
    for wheel in valve_wheels(shape) {
        assert!(blocked(level(wheel.center)));
    }
    for button in button_boxes(shape) {
        assert!(blocked(level(button.base)));
    }
    let tank = tank_place(shape, Port::new(DEFAULT_GEOMETRY));
    assert!(blocked(level(tank.foot)));
    assert!(blocked(level(clock_place(shape).base)));
    // The cartridge is low enough to step up onto.
    let rest = level(cartridge::rest(shape));
    assert!(!blocked(rest));
    let on = map.ground(rest, radius, shape.base).unwrap();
    assert!(on > shape.base && on <= shape.base + STEP_UP * shape.bore);
    let (_, seated) = self::map(true);
    assert!(seated.obstacles.len() < map.obstacles.len());
    // Open table is open.
    assert!(!blocked(vec2(0.0, 4.0 * shape.bore)));
    assert!(!blocked(vec2(0.0, -4.0 * shape.bore)));
}

/// Walking every which way from the start, round the engine and into
/// every nook, the figure never ends up inside anything.
#[test]
fn walking_never_goes_inside_anything() {
    let (shape, map) = map(false);
    let radius = BODY_RADIUS * shape.bore;
    for heading in 0..24 {
        let mut walker = Walker::standing(shape);
        walker.yaw = heading as f32 * std::f32::consts::TAU / 24.0;
        for frame in 0..600 {
            // Turning a little as it goes, to feel along what it meets.
            walker.yaw += 0.004;
            let keys = if frame % 200 < 150 {
                vec2(0.0, 1.0)
            } else {
                vec2(1.0, 0.0)
            };
            walker.walk(
                FirstPersonKeys {
                    way: keys,
                    ..Default::default()
                },
                1.0 / 60.0,
                &map,
            );
            if walker.airborne() {
                break;
            }
            let feet = walker.feet();
            let at = vec2(feet.x, feet.z);
            assert!(
                map.clearance(at, radius, shape.base) > -0.02 * shape.bore,
                "heading {heading} frame {frame} at {at}"
            );
        }
    }
}

/// The table's edge is where the footing ends.
#[test]
fn the_table_ends_at_its_edge() {
    let (shape, map) = map(true);
    let edge = TOP_HALF_LENGTH * shape.bore;
    assert!(map.on_table(vec2(edge - 0.01, 0.0)));
    assert!(!map.on_table(vec2(edge + 0.01, 0.0)));
    assert!(!map.on_table(vec2(0.0, -(TOP_HALF_DEPTH * shape.bore + 0.01))));
}

/// From on top of a button box, the box is what it stands on, and the
/// table beside it is too far down to step to.
#[test]
fn a_box_is_stood_on_from_above() {
    let (shape, map) = map(true);
    let radius = BODY_RADIUS * shape.bore;
    let button = button_boxes(shape)[0];
    let middle = vec2(button.base.x, button.base.z);
    let top = button.dome_top().y;
    assert_eq!(map.ground(middle, radius, top), Some(top));
    assert!(
        map.clearance(middle, radius, top) > 0.0,
        "nothing in the way up there"
    );
    // Its middle well past the box's edge, the box is not under it.
    let beside = middle + Vec2::X * 3.0 * radius;
    assert_eq!(map.ground(beside, radius, top), Some(shape.base));
}
