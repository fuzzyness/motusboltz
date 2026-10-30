//! Floor-grid tests for `src/graphics/floor.rs`.

use super::*;

#[test]
fn grid_is_flat_under_the_block_and_fades_to_nothing_at_its_edge() {
    const FLOOR: f32 = -4.5;
    let grid = FloorGrid::new(FLOOR);
    assert!(
        grid.vertices()
            .all(|p| (p.y - (FLOOR - BELOW_BLOCK)).abs() < 1e-6)
    );
    assert!(
        grid.vertices()
            .all(|p| p.x.abs() <= GRID_HALF_WIDTH + LINE_WIDTH
                && p.z.abs() <= GRID_HALF_WIDTH + LINE_WIDTH)
    );
    let mut middle = 0_u8;
    for vertex in &grid.mesh.vertices {
        let reach = (vertex.position.x.powi(2) + vertex.position.z.powi(2)).sqrt();
        if reach >= GRID_HALF_WIDTH {
            assert_eq!(vertex.color[3], 0, "edges fade out");
        }
        if reach < 1.0 {
            middle = middle.max(vertex.color[3]);
        }
    }
    assert!(middle as f32 > 0.9 * GRID_OPACITY * 255.0);
    assert!(grid.mesh.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
}
