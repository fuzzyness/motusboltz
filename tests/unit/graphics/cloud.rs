//! Tests for the cloud under the engine, `src/graphics/cloud.rs`.

use super::*;
use crate::graphics::outline::outside_block;
use crate::sim::geometry::DEFAULT_GEOMETRY;

#[test]
fn cloud_stays_under_the_floor_except_its_edge_bumps_beyond_the_burner_and_ice() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let cloud = CloudFloor::new(shape);
    let mut above = 0;
    for vertex in &cloud.mesh.vertices {
        let point = vertex.position;
        if point.y > shape.base + 1e-3 {
            above += 1;
            let out = outside_block(shape.offset, shape.outer, Vec2::new(point.x, point.z));
            // The burner and the ice heap reach about 0.65 bore radii out.
            assert!(
                out > 0.7 * shape.bore,
                "{point} rises among the burner or ice"
            );
        }
    }
    assert!(above > 0, "the edge bumps show above the top");
    assert!(cloud.mesh.vertices.len() <= DRAW_CALL_VERTEX_CAPACITY);
    assert!(cloud.mesh.indices.len() <= DRAW_CALL_INDEX_CAPACITY);
    let count = cloud.mesh.vertices.len();
    assert!(
        cloud
            .mesh
            .indices
            .iter()
            .all(|&index| (index as usize) < count)
    );
}

#[test]
fn top_covers_the_ground_under_the_engine() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let tops: Vec<Puff> = puffs(shape)
        .into_iter()
        .filter(|puff| puff.center.y + FLATTEN * puff.radius > shape.base - 0.1)
        .collect();
    // Every point of the block's outline, and a bore radius beyond it, has
    // a puff's top under it.
    for step in 0..64 {
        let angle = std::f32::consts::TAU * step as f32 / 64.0;
        for reach in [0.0, 0.5, 1.0, 1.4] {
            let x = (shape.offset + shape.outer * reach) * angle.cos();
            let z = shape.outer * reach * angle.sin();
            assert!(
                tops.iter().any(
                    |puff| Vec2::new(puff.center.x - x, puff.center.z - z).length() < puff.radius
                ),
                "nothing under ({x}, {z})"
            );
        }
    }
}
