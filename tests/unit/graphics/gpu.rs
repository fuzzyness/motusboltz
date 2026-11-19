//! Tests for `src/graphics/gpu.rs` that need no window.

use super::*;

#[test]
fn edited_meshes_leave_room_to_grow() {
    assert_eq!(room(0), 1);
    assert_eq!(room(100), 150);
    assert!(room(7) >= 7);
}

#[test]
fn the_vertex_layout_matches_macroquads_vertex() {
    let floats: usize = vertex_attributes()
        .iter()
        .map(|attribute| match attribute.format {
            VertexFormat::Float2 => 2,
            VertexFormat::Float3 => 3,
            VertexFormat::Float4 => 4,
            VertexFormat::Byte4 => 1,
            other => panic!("unexpected {other:?}"),
        })
        .sum();
    assert_eq!(floats * FLOAT_BYTES, std::mem::size_of::<Vertex>());
}

#[test]
fn meshes_keep_their_copy_and_know_an_edit() {
    let mut mesh = GpuMesh::empty();
    assert!(!mesh.edited.get());
    mesh.edit().indices.extend([0, 1, 2]);
    assert!(mesh.edited.get());
    assert_eq!(mesh.mesh().indices, [0, 1, 2]);
}
