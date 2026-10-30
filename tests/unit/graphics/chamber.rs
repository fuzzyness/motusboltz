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

/// The rims lie on the bore walls, the outline stays within the wall
/// between the bores, and its lowest line is flush with the floors.
#[test]
fn passage_outline_follows_both_bore_walls_down_to_the_floor() {
    const TOLERANCE: f32 = 1e-4;
    let geometry = crate::sim::geometry::DEFAULT_GEOMETRY;
    let lines = passage_wireframe(geometry);
    assert!(!lines.is_empty());
    let mut on_wall = [false; 2];
    let mut lowest = f32::INFINITY;
    for (a, b) in lines {
        for point in [a, b] {
            assert!(point.is_finite());
            let center = point.x.signum() * geometry.cylinder_offset;
            let from_axis = ((point.x - center).powi(2) + point.z.powi(2)).sqrt();
            assert!((from_axis - geometry.cylinder_radius).abs() < TOLERANCE);
            // From the middle of the wall (z = 0) out to the tube's sides.
            let widest = geometry.cylinder_offset
                - (geometry.cylinder_radius.powi(2) - geometry.passage_radius.powi(2)).sqrt();
            assert!(point.x.abs() >= geometry.shared_wall_thickness() / 2.0 - TOLERANCE);
            assert!(point.x.abs() <= widest + TOLERANCE);
            on_wall[usize::from(point.x > 0.0)] = true;
            lowest = lowest.min(point.y);
        }
    }
    assert_eq!(on_wall, [true; 2]);
    assert!((lowest - geometry.floor()).abs() < TOLERANCE);
}
