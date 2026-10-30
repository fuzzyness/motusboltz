//! Engine-block mesh and shader tests for `src/graphics/block.rs`.

use std::collections::HashMap;

use super::*;
use crate::sim::geometry::DEFAULT_GEOMETRY;

/// A vertex position rounded to a fine grid, so equal points compare equal.
fn key(point: Vec3) -> [i64; 3] {
    const GRID: f32 = 1e5;
    [point.x, point.y, point.z].map(|c| (c * GRID).round() as i64)
}

fn mesh() -> (BlockShape, Mesh) {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    (shape, block_mesh(shape))
}

/// The block is one closed solid: every edge, by position, belongs to
/// exactly two triangles. That is what lets the shader paint any surface
/// seen from inside the metal as the face of the cut.
#[test]
fn block_is_a_closed_solid() {
    let (_, mesh) = mesh();
    let mut edges: HashMap<([i64; 3], [i64; 3]), usize> = HashMap::new();
    for triangle in mesh.indices.as_chunks::<3>().0 {
        let points = [0, 1, 2].map(|i| key(mesh.vertices[triangle[i] as usize].position));
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            let (p, q) = (points[a], points[b]);
            assert_ne!(p, q, "degenerate edge");
            *edges
                .entry(if p < q { (p, q) } else { (q, p) })
                .or_default() += 1;
        }
    }
    let open: Vec<_> = edges.iter().filter(|&(_, &count)| count != 2).collect();
    assert!(
        open.is_empty(),
        "{} edges not shared by two triangles: {:?}",
        open.len(),
        &open[..open.len().min(6)]
    );
}

#[test]
fn block_mesh_fits_one_draw_call_with_unit_normals() {
    let (_, mesh) = mesh();
    assert!(mesh.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
    assert!(mesh.indices.len() < DRAW_CALL_INDEX_CAPACITY);
    assert!(
        mesh.indices
            .iter()
            .all(|&i| (i as usize) < mesh.vertices.len())
    );
    for vertex in &mesh.vertices {
        let n = vertex.normal;
        let length = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt();
        assert!((length - 1.0).abs() < 1e-4, "{n:?}");
        assert!(vertex.position.is_finite());
    }
}

/// The block encloses all the gas: every vertex lies on or outside the
/// gas space, the walls are at least WALL_THICKNESS thick, and it reaches
/// from below the floors to the cylinder tops.
#[test]
fn block_wraps_the_bores_and_the_opening() {
    let (shape, mesh) = mesh();
    let geometry = DEFAULT_GEOMETRY;
    let (mut lowest, mut highest, mut widest) = (f32::INFINITY, f32::NEG_INFINITY, 0.0_f32);
    for vertex in &mesh.vertices {
        let p = vertex.position;
        // No vertex sits strictly inside the gas.
        assert!(!geometry.contains(p, 1e-3, shape.top, shape.top), "{p:?}");
        lowest = lowest.min(p.y);
        highest = highest.max(p.y);
        widest = widest.max(p.x.abs());
        // Within the block's racetrack outline.
        let from_axis = vec3(p.x.abs() - shape.offset, 0.0, p.z);
        let inside = if p.x.abs() >= shape.offset {
            from_axis.length() <= shape.outer + 1e-4
        } else {
            p.z.abs() <= shape.outer + 1e-4
        };
        assert!(inside, "{p:?}");
    }
    assert!((lowest - shape.base).abs() < 1e-5);
    assert!((highest - shape.top).abs() < 1e-5);
    assert!((widest - (shape.offset + shape.outer)).abs() < 1e-4);
    assert!(shape.outer - shape.bore >= WALL_THICKNESS - 1e-6);
}

/// The window sits inside the block's outline, leaving metal all round.
#[test]
fn window_leaves_a_frame_of_metal() {
    let (shape, _) = mesh();
    let [half_width, middle, half_height, corner] = shape.window();
    assert!(half_width < shape.offset + shape.outer);
    assert!(
        half_width > shape.offset,
        "the window shows each bore's axis"
    );
    assert!(middle - half_height > shape.base);
    assert!(middle + half_height < shape.top);
    assert!(corner < half_height && corner < half_width);
}

#[test]
fn the_cut_faces_the_camera_from_any_side() {
    assert_eq!(cut_normal(vec3(0.0, 5.0, 10.0)), Vec3::Z);
    let side = cut_normal(vec3(8.0, 3.0, -8.0));
    assert!((side.length() - 1.0).abs() < 1e-6 && side.y == 0.0);
    assert!(side.x > 0.0 && side.z < 0.0);
    // Straight above, it falls back to the front.
    assert_eq!(cut_normal(vec3(0.0, 10.0, 0.0)), Vec3::Z);
}

#[test]
fn shaders_are_glsl_100_and_declare_their_uniforms() {
    for source in [BLOCK_VERTEX, BLOCK_FRAGMENT] {
        assert!(source.starts_with("#version 100"));
        assert!(source.is_ascii());
    }
    assert!(BLOCK_FRAGMENT.contains("precision mediump float;"));
    for uniform in ["Eye", "CutNormal", "Window", "Offset", "Light", "Texture"] {
        assert!(
            BLOCK_FRAGMENT.contains(&format!(" {uniform};")),
            "{uniform}"
        );
    }
}
