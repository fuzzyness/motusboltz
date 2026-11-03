//! Tests for the table under the engine, `src/graphics/table.rs`.

use super::*;
use crate::sim::geometry::DEFAULT_GEOMETRY;

/// The table's top is just under the block, and its legs reach the ground.
#[test]
fn the_top_is_under_the_block_and_the_legs_reach_the_ground() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let solid = build(shape);
    let highest = solid
        .vertices
        .iter()
        .map(|vertex| vertex.position.y)
        .fold(f32::MIN, f32::max);
    let lowest = solid
        .vertices
        .iter()
        .map(|vertex| vertex.position.y)
        .fold(f32::MAX, f32::min);
    assert!(highest < shape.base && highest > shape.base - 0.01);
    assert!((lowest - ground(shape)).abs() < 1e-4);
    assert!(ground(shape) < shape.base - shape.bore);
}

/// The top reaches past the block, the valves and the parts on the floor
/// beside it, and every foot stands under the top.
#[test]
fn the_top_covers_the_engine_and_the_feet_stand_under_it() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let (length, depth) = (TOP_HALF_LENGTH * shape.bore, TOP_HALF_DEPTH * shape.bore);
    assert!(length > shape.offset + shape.outer + 2.0 * shape.bore);
    assert!(depth > shape.outer + 2.0 * shape.bore);
    for foot in feet(shape) {
        assert!(foot.x.abs() < length && foot.y.abs() < depth);
        assert!(foot.x.abs() > shape.offset + shape.outer);
    }
}

/// The grain runs along the shaft on the top and up the legs.
#[test]
fn the_grain_follows_each_part() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let solid = build(shape);
    let below_top = shape.base - (TOP_THICKNESS + APRON_DEPTH) * shape.bore - 0.01;
    let along_shaft = |vertex: &&macroquad::models::Vertex| vertex.color[3] == u8::MAX;
    // Below the apron there are only legs.
    for vertex in solid.vertices.iter().filter(|v| v.position.y < below_top) {
        assert!(!along_shaft(&vertex), "{}", vertex.position);
    }
    // The top's surface runs along the shaft.
    let surface = shape.base - 0.01;
    let top: Vec<_> = solid
        .vertices
        .iter()
        .filter(|v| v.position.y > surface)
        .collect();
    assert!(!top.is_empty() && top.iter().all(along_shaft));
}

/// The highlight covers the top just above it, and only the top counts as
/// a place to drop the figure.
#[test]
fn the_highlight_lies_on_the_top() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let mesh = highlight_mesh(shape);
    for vertex in &mesh.vertices {
        let point = vertex.position;
        assert!(point.y > top_height(shape) && point.y < top_height(shape) + 0.5);
        assert!(over_top(shape, Vec2::new(point.x, point.z)));
    }
    assert!(
        mesh.indices
            .iter()
            .all(|&index| (index as usize) < mesh.vertices.len())
    );
    assert!(over_top(shape, Vec2::ZERO));
    assert!(!over_top(
        shape,
        Vec2::new(TOP_HALF_LENGTH * shape.bore + 1.0, 0.0)
    ));
}
