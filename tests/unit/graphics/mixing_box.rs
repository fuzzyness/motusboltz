//! Tests for the mixing box, `src/graphics/mixing_box.rs`.

use super::*;
use crate::sim::geometry::DEFAULT_GEOMETRY;
use crate::sim::port::Port;
use crate::ui::walking::{
    Footprint,
    WalkMap, //
};

/// Whether `a` and `b` are the same color, but for rounding.
fn same(a: Color, b: Color) -> bool {
    let close = |x: f32, y: f32| (x - y).abs() < 1e-5;
    close(a.r, b.r) && close(a.g, b.g) && close(a.b, b.b) && close(a.a, b.a)
}

/// The balls run from blue when cold through purple to red when hot.
#[test]
fn the_balls_go_from_blue_through_purple_to_red() {
    assert!(same(ball_color(0.0), COLD_BALL));
    assert!(same(ball_color(0.5), EVEN_BALL));
    assert!(same(ball_color(1.0), HOT_BALL));
    assert!(same(ball_color(-1.0), COLD_BALL));
    assert!(same(ball_color(2.0), HOT_BALL));
}

/// The box stands on the table, clear of everything else on it, and of
/// the cartridge where it first lies.
#[test]
fn the_box_stands_clear_on_the_table() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let place = mixing_box_place(shape);
    let (left, right, depth) = place.footprint();
    let mine = Footprint::Slab {
        from: vec2(left.x, left.z),
        to: vec2(right.x, right.z),
        radius: depth,
    };
    let rest = crate::graphics::cartridge::rest(shape);
    let map = WalkMap::new(shape, Port::new(DEFAULT_GEOMETRY), Some(rest));
    let samples = 16;
    for step in 0..=samples {
        let share = step as f32 / samples as f32;
        let point = left.lerp(right, share);
        let point = vec2(point.x, point.z);
        for way in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
            assert!(map.on_table(point + way * depth), "on the table");
        }
        // What stands on the table, not the table itself, its legs or the
        // cabinet under it.
        let on_table = |o: &&crate::ui::walking::Obstacle| o.bottom >= shape.base - 1e-4;
        for obstacle in map
            .obstacles
            .iter()
            .filter(on_table)
            .filter(|o| o.footprint != mine)
        {
            let (outside, _) = obstacle.footprint.distance(point);
            assert!(outside > depth, "clear of {obstacle:?}");
        }
    }
    assert!(
        map.obstacles.iter().any(|o| o.footprint == mine),
        "walked round"
    );
}

/// The divider's grip stands above the lid, and the box's middle inside
/// it.
#[test]
fn the_grip_is_above_the_lid() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let place = mixing_box_place(shape);
    assert!(place.grip().y > place.top());
    assert!(place.middle().y < place.top() && place.middle().y > place.floor.y);
    assert!(place.floor.y > shape.base);
}
