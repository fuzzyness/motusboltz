//! Cylinder-frame and passage tests for `src/graphics/chamber.rs`.

use super::*;
use macroquad::prelude::ORANGE;

const CHAMBER_HEIGHT: f32 = 4.0;
const CHAMBER_RADIUS: f32 = 2.0;

#[test]
fn cylinder_frame_is_one_valid_mesh() {
    let frame = CylinderFrame::new(
        CylinderDimensions {
            radius: CHAMBER_RADIUS,
            height: CHAMBER_HEIGHT,
        },
        ORANGE,
    );
    let expected_vertices =
        (2 * CYLINDER_RING_SEGMENTS + 2 * CYLINDER_VERTICAL_GUIDES) * FRAME_TUBE_SIDES;
    let expected_indices =
        (2 * CYLINDER_RING_SEGMENTS + CYLINDER_VERTICAL_GUIDES) * FRAME_TUBE_SIDES * 6;

    assert_eq!(frame.mesh.vertices.len(), expected_vertices);
    assert_eq!(frame.mesh.indices.len(), expected_indices);
    assert!(frame.mesh.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
    assert!(frame.mesh.indices.len() < DRAW_CALL_INDEX_CAPACITY);
    assert!(
        frame
            .mesh
            .indices
            .iter()
            .all(|&index| (index as usize) < frame.mesh.vertices.len())
    );
}

#[test]
fn passage_wireframe_reaches_both_floors_and_covers_the_bottom_run() {
    const TOLERANCE: f32 = 1e-4;
    let geometry = crate::sim::geometry::DEFAULT_GEOMETRY;
    let lines = passage_wireframe(geometry);
    assert!(!lines.is_empty());
    let mut reaches_floor = [false; 2];
    let mut bottom = f32::INFINITY;
    for (a, b) in lines {
        for point in [a, b] {
            assert!(point.is_finite());
            assert!(point.y <= -geometry.cylinder_height / 2.0 + TOLERANCE);
            assert!(point.abs().max_element() <= geometry.grid_side() / 2.0 + TOLERANCE);
            if (point.y + geometry.cylinder_height / 2.0).abs() < TOLERANCE {
                reaches_floor[usize::from(point.x > 0.0)] = true;
            }
            bottom = bottom.min(point.y);
        }
    }
    assert_eq!(reaches_floor, [true; 2]);
    assert!((bottom - geometry.bottom()).abs() < TOLERANCE);
}
