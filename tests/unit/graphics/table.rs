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

/// The grain runs along the shaft on the top and up the end panel.
#[test]
fn the_grain_follows_each_part() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let solid = build(shape);
    let along_shaft = |vertex: &&macroquad::models::Vertex| vertex.color[3] == u8::MAX;
    // Left of the knee space, under the top, there is only the end panel;
    // the rail and the pencil drawer end at its inside face.
    let panel = end_panel(shape);
    let in_panel: Vec<_> = solid
        .vertices
        .iter()
        .filter(|v| v.position.x < panel.high.x - 1e-3 && v.position.y < panel.high.y - 0.01)
        .collect();
    assert!(!in_panel.is_empty() && !in_panel.iter().any(along_shaft));
    // The top's surface runs along the shaft.
    let surface = shape.base - 0.01;
    let top: Vec<_> = solid
        .vertices
        .iter()
        .filter(|v| v.position.y > surface)
        .collect();
    assert!(!top.is_empty() && top.iter().all(along_shaft));
}

/// The end panel and the pedestal stand at the desk's two ends, under the
/// top's overhang, and between them the knee space leaves room for the
/// figure under the pencil drawer, with the rail across its back.
#[test]
fn the_knee_space_lies_between_the_end_panel_and_the_pedestal() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let unit = shape.bore;
    let panel = end_panel(shape);
    let pedestal = cabinet_place(shape);
    let (length, depth) = (TOP_HALF_LENGTH * unit, TOP_HALF_DEPTH * unit);
    assert!(panel.low.x > -length && pedestal.right < length);
    assert!(pedestal.front < depth && panel.high.z < depth);
    let (left, right) = knee_space(shape);
    assert!(right - left > 8.0 * unit, "a wide knee space");
    let pencil = pencil_drawer(shape);
    assert!(pencil.left >= left && pencil.right <= right);
    assert!(pencil.top < underside(shape));
    // The figure, 3.4 bore radii tall, stands under the pencil drawer.
    assert!(pencil.bottom - ground(shape) > shape.top - shape.base);
    let rail = back_rail(shape);
    assert!(rail.low.z > -depth && rail.high.z < pencil.back);
    assert!(rail.low.y > ground(shape) && rail.high.y < pencil.bottom);
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
