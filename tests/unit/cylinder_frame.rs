//! Cylinder-frame unit tests for `src/graphics.rs`.

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
