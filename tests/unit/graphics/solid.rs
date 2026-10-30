//! Tests for building solid parts, `src/graphics/solid.rs`.

use super::*;
use macroquad::prelude::vec3;

const SEGMENTS: usize = 24;
const TOLERANCE: f32 = 1e-4;

fn normal(vertex: &Vertex) -> Vec3 {
    vertex.normal.truncate()
}

fn indices_are_valid(solid: &Solid) {
    assert_eq!(solid.indices.len() % 3, 0);
    assert!(
        solid
            .indices
            .iter()
            .all(|&index| (index as usize) < solid.vertices.len())
    );
}

#[test]
fn cylinder_is_closed_with_outward_unit_normals() {
    let (start, end, radius) = (vec3(1.0, 2.0, 3.0), vec3(1.0, 2.0, 6.0), 0.5);
    let mut solid = Solid::new();
    solid.cylinder(start, end, radius, SEGMENTS, finish(200, 200, 200, 0.5));
    indices_are_valid(&solid);
    let middle = (start + end) / 2.0;
    for vertex in &solid.vertices {
        let point = vertex.position;
        assert!((normal(vertex).length() - 1.0).abs() < TOLERANCE);
        let along = (point - start).dot(Vec3::Z);
        let out = (point - start - Vec3::Z * along).length();
        assert!(out <= radius + TOLERANCE && (-TOLERANCE..=3.0 + TOLERANCE).contains(&along));
        // Each face's normal points away from the middle of the bar.
        assert!(normal(vertex).dot(point - middle) > -TOLERANCE);
    }
}

#[test]
fn bar_tapers_between_its_ends() {
    let mut solid = Solid::new();
    solid.bar(
        Vec3::ZERO,
        Vec3::Y * 4.0,
        Vec3::X,
        (0.5, 0.25),
        0.1,
        finish(100, 100, 100, 0.3),
    );
    indices_are_valid(&solid);
    assert_eq!(solid.vertices.len(), 6 * 4);
    for vertex in &solid.vertices {
        let point = vertex.position;
        let half_width = if point.y < 2.0 { 0.5 } else { 0.25 };
        assert!((point.x.abs() - half_width).abs() < TOLERANCE);
        assert!((point.z.abs() - 0.1).abs() < TOLERANCE);
        assert!((normal(vertex).length() - 1.0).abs() < TOLERANCE);
    }
}

#[test]
fn web_outline_wraps_both_circles_and_is_convex() {
    let (radii, reach) = ((0.5, 0.3), 2.0);
    let outline = web_outline(radii, reach, 12);
    for point in &outline {
        let near = point.length();
        let far = point.distance(Vec2::new(reach, 0.0));
        assert!(
            (near - radii.0).abs() < TOLERANCE || (far - radii.1).abs() < TOLERANCE,
            "{point} is on neither circle"
        );
        assert!(near >= radii.0 - TOLERANCE && far >= radii.1 - TOLERANCE);
    }
    for index in 0..outline.len() {
        let [a, b, c] = [0, 1, 2].map(|step| outline[(index + step) % outline.len()]);
        assert!(
            (b - a).perp_dot(c - b) >= -TOLERANCE,
            "not convex at {index}"
        );
    }
    let mut solid = Solid::new();
    solid.plate(
        Frame::along(Vec3::ZERO, Vec3::X),
        &outline,
        0.2,
        finish(120, 120, 120, 0.5),
    );
    indices_are_valid(&solid);
    assert!(
        solid
            .vertices
            .iter()
            .all(|vertex| (-TOLERANCE..=0.2 + TOLERANCE).contains(&vertex.position.x))
    );
}

#[test]
fn shaders_are_glsl_100_and_declare_their_uniforms() {
    for shader in [METAL_VERTEX, METAL_FRAGMENT] {
        assert!(shader.starts_with("#version 100\n"));
    }
    for uniform in ["uniform vec3 Eye;", "uniform vec3 Light;"] {
        assert!(METAL_FRAGMENT.contains(uniform));
    }
}
