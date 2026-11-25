//! Tests for the glass windows, `src/graphics/windows.rs`.

use super::*;
use crate::graphics::hatch::hatch_reach;
use crate::sim::geometry::DEFAULT_GEOMETRY;

/// How far a pane at middle angle `middle` is from angle `angle`, both
/// round the same cylinder, in radians.
fn off_pane(middle: f32, angle: f32) -> f32 {
    let turn = std::f32::consts::TAU;
    ((angle - middle).rem_euclid(turn) - PI).abs() - PI
}

/// The front and back panes leave wall between each other at the far end
/// and keep clear of the opening through the shared wall.
#[test]
fn the_panes_leave_wall_between_them_and_clear_the_opening() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let middles: Vec<f32> = pane_middles().collect();
    assert_eq!(middles.len(), 2);
    let [front, back] = [middles[0], middles[1]];
    assert!(
        off_pane(front, back).abs() > 2.0 * PANE_HALF_ANGLE,
        "a pillar"
    );
    let opening = (shape.opening / shape.bore).asin();
    for middle in middles {
        assert!(
            middle.abs() - PANE_HALF_ANGLE > opening,
            "clear of the opening"
        );
    }
}

/// The panes stand within the wall, with metal and frame above and below
/// them, and clear of the regenerator's door and rails on the front face.
#[test]
fn the_panes_stand_clear_of_the_ends_and_the_door() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let layout = PaneLayout::new(shape);
    assert!(layout.bottom - layout.frame > shape.floor);
    assert!(layout.top + layout.frame < shape.top);
    let reach = hatch_reach(shape);
    for middle in pane_middles() {
        for edge in [middle - layout.half_angle, middle + layout.half_angle] {
            let (point, _) = outer_point(shape, edge);
            // A frame's width further round, at the bore's radius at most.
            let frame = layout.frame * shape.outer / shape.bore;
            assert!(-point.x > reach + frame, "clear of the door at {edge}");
        }
    }
}

/// Across every pane, the outside of the block is where its rounded ends
/// and flat faces are, the way out of it square to them.
#[test]
fn outer_points_lie_on_the_blocks_outside() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let axis = Vec3::new(-shape.offset, 0.0, 0.0);
    let steps = 12;
    let across = pane_middles().flat_map(|middle| {
        (0..=steps)
            .map(move |step| middle + PANE_HALF_ANGLE * (2.0 * step as f32 / steps as f32 - 1.0))
    });
    for angle in across {
        let (point, out) = outer_point(shape, angle);
        assert!((out.length() - 1.0).abs() < 1e-5);
        assert!(point.x <= 1e-5, "never past the middle at {angle}");
        if point.x < axis.x - 1e-5 {
            assert!((point.distance(axis) - shape.outer).abs() < 1e-4);
        } else {
            assert!(
                (point.z.abs() - shape.outer).abs() < 1e-4,
                "on a face at {angle}"
            );
        }
    }
}

/// The glass lies just outside the wall, never inside a bore, and carries
/// its place on its pane as shares of the pane's size.
#[test]
fn the_glass_lies_just_outside_the_wall() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let mesh = glass_mesh(shape);
    let panes = 2 * pane_middles().count();
    assert_eq!(mesh.vertices.len(), panes * 2 * (GLASS_COLUMNS + 1));
    for vertex in &mesh.vertices {
        let point = vertex.position;
        let axis = Vec3::new(point.x.signum() * shape.offset, 0.0, 0.0);
        let from_axis = Vec3::new(point.x - axis.x, 0.0, point.z).length();
        assert!(from_axis > shape.outer - 1e-4);
        assert!(vertex.uv.x.abs() <= 1.0 + 1e-5 && vertex.uv.y.abs() <= 1.0 + 1e-5);
    }
}
