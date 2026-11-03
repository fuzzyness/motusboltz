//! Tests for the cloud under the engine, `src/graphics/cloud.rs`.

use super::*;
use crate::graphics::table::feet;
use crate::sim::geometry::DEFAULT_GEOMETRY;

/// The cloud stays under the table's feet, but for bumps along its edge,
/// out beyond the legs.
#[test]
fn cloud_stays_under_the_table_except_its_edge_bumps_beyond_the_legs() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let cloud = CloudFloor::new(shape);
    let floor = ground(shape);
    let mut above = 0;
    for vertex in &cloud.mesh.vertices {
        let point = vertex.position;
        if point.y > floor + 1e-3 {
            above += 1;
            let seen_from_above = Vec2::new(point.x, point.z);
            for foot in feet(shape) {
                assert!(
                    seen_from_above.distance(foot) > shape.bore,
                    "{point} rises among the legs"
                );
            }
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

/// Every foot of the table, and a little round it, has a puff's top under
/// it.
#[test]
fn top_carries_every_foot() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let floor = ground(shape);
    let tops: Vec<Puff> = puffs(shape)
        .into_iter()
        .filter(|puff| puff.center.y + FLATTEN * puff.radius > floor - 0.1)
        .collect();
    for foot in feet(shape) {
        for step in 0..16 {
            let angle = std::f32::consts::TAU * step as f32 / 16.0;
            let spot = foot + Vec2::from_angle(angle) * shape.bore;
            assert!(
                tops.iter().any(
                    |puff| Vec2::new(puff.center.x - spot.x, puff.center.z - spot.y).length()
                        < puff.radius
                ),
                "nothing under {spot}"
            );
        }
    }
}
