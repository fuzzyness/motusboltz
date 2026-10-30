//! Tests for placing the background's glows, `src/graphics/backdrop.rs`.

use super::*;
use crate::sim::geometry::DEFAULT_GEOMETRY;

const ASPECT: f32 = 1.6;

/// The matrix Macroquad's `Camera3D` makes (its own method needs a window
/// to read the size from, even when given the aspect).
fn matrix(position: Vec3, target: Vec3) -> Mat4 {
    let camera = Camera3D::default();
    Mat4::perspective_rh_gl(camera.fovy, ASPECT, camera.z_near, camera.z_far)
        * Mat4::look_at_rh(position, target, Vec3::Y)
}

#[test]
fn a_centered_cylinder_glows_from_the_middle_of_the_window() {
    let spot = spot(matrix(vec3(0.0, 0.0, 10.0), Vec3::ZERO), 0.0, -1.0, 1.0).unwrap();
    assert!((spot.center - vec2(0.5, 0.5)).length() < 1e-5);
    // 2 units tall at 10 units away, with a 45 degree view: 0.2 / tan(22.5).
    let height = 0.2 / (22.5_f32.to_radians()).tan() / 2.0;
    assert!((spot.radius - GLOW_REACH * height).abs() < 1e-4);
}

#[test]
fn glows_follow_each_cylinder_as_the_camera_turns() {
    let geometry = DEFAULT_GEOMETRY;
    let (floor, top) = (geometry.floor(), geometry.cylinder_height / 2.0);
    let eye = vec3(19.0, 12.6, 19.0);
    for (position, hot_on_left) in [(eye, true), (eye * vec3(-1.0, 1.0, -1.0), false)] {
        let matrix = matrix(position, Vec3::ZERO);
        let hot = spot(matrix, geometry.hot_center_x(), floor, top).unwrap();
        let cold = spot(matrix, geometry.cold_center_x(), floor, top).unwrap();
        assert_eq!(hot.center.x < cold.center.x, hot_on_left);
        for glow in [hot, cold] {
            assert!((0.0..1.0).contains(&glow.center.x) && (0.0..1.0).contains(&glow.center.y));
            assert!(glow.radius > 0.1 && glow.radius < 1.0);
        }
    }
}

#[test]
fn no_glow_for_a_cylinder_behind_the_camera() {
    let looking_away = matrix(vec3(10.0, 0.0, 0.0), vec3(20.0, 0.0, 0.0));
    assert_eq!(spot(looking_away, 0.0, -1.0, 1.0), None);
}

#[test]
fn shaders_are_glsl_100_and_declare_their_uniforms() {
    for shader in [BACKDROP_VERTEX, BACKDROP_FRAGMENT] {
        assert!(shader.starts_with("#version 100\n"));
    }
    for uniform in [
        "uniform vec4 Hot;",
        "uniform vec4 Cold;",
        "uniform float Aspect;",
    ] {
        assert!(BACKDROP_FRAGMENT.contains(uniform), "{uniform} missing");
    }
    // Sines lose accuracy for large arguments on some GPUs (seen as a black
    // wedge on Mesa), so the noise must not use one.
    assert!(!BACKDROP_FRAGMENT.contains("sin("));
}
