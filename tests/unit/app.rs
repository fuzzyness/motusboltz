//! Tests for `src/app.rs`: the build label, the first camera view, the help
//! text and the fill presets.

use super::*;
use crate::graphics::shaft::WHEEL_RADIUS;
use crate::sim::engine::PARTICLE_RADIUS;
use crate::sim::geometry::DEFAULT_GEOMETRY;
use crate::sim::pistons::PowerPistons;
use crate::sim::units::{
    MAX_SIMULATED_MARKERS,
    MIN_CONNECTED_MARKERS, //
};
use macroquad::prelude::{
    Mat4,
    Vec3,
    vec3, //
};

/// Window and browser-canvas sizes the scene must fit (see the layout
/// tests).
const WINDOWS: [(f32, f32); 5] = [
    (1440.0, 900.0),
    (1366.0, 768.0),
    (1280.0, 720.0),
    (1366.0, 620.0),
    (1280.0, 580.0),
];

#[test]
fn build_label_names_this_version() {
    assert!(BUILD_LABEL.starts_with(concat!("v", env!("CARGO_PKG_VERSION"), " (")));
    assert!(BUILD_LABEL.ends_with(')'));
    assert!(BUILD_LABEL.is_ascii());
    assert!(TITLE.is_ascii());
}

#[test]
fn initial_camera_frames_the_floors_and_the_flywheel() {
    let geometry = DEFAULT_GEOMETRY;
    let pistons = PowerPistons::new(geometry, PARTICLE_RADIUS);
    let camera = initial_camera(geometry).camera();
    for (width, height) in WINDOWS {
        let (_, scene_width) = scene_span(width, true);
        let aspect = scene_width / height;
        // Recreate the perspective transform without a window/context.
        let matrix = Mat4::perspective_rh_gl(camera.fovy, aspect, camera.z_near, camera.z_far)
            * Mat4::look_at_rh(camera.position, camera.target, camera.up);
        for point in [
            vec3(0.0, pistons.shaft_height() + WHEEL_RADIUS, 0.0),
            vec3(0.0, pistons.shaft_height(), WHEEL_RADIUS),
            vec3(0.0, pistons.shaft_height(), -WHEEL_RADIUS),
            vec3(geometry.hot_center_x(), geometry.floor(), 0.0),
            vec3(geometry.cold_center_x(), geometry.floor(), 0.0),
            vec3(geometry.hot_center_x() - geometry.cylinder_radius, 0.0, 0.0),
            vec3(
                geometry.cold_center_x() + geometry.cylinder_radius,
                0.0,
                0.0,
            ),
            Vec3::ZERO,
        ] {
            let clip = matrix * point.extend(1.0);
            assert!(clip.w > 0.0);
            assert!(
                clip.x.abs() < clip.w && clip.y.abs() < clip.w,
                "clipped scene point {point:?} in a {width}x{height} window"
            );
        }
    }
}

/// The two help lines fit the scene without shrinking much, so they stay
/// readable on the smallest browser canvas.
#[test]
fn help_lines_stay_readable_at_every_window_width() {
    const MIN_READABLE_SIZE: f32 = 12.0;
    let pen = Pen::offline();
    for (width, _) in WINDOWS {
        let (_, scene_width) = scene_span(width, true);
        for line in HELP_LINES {
            let size = pen.fitted_size(line, scene_width - 2.0 * SCENE_TEXT_MARGIN, HELP_FONT_SIZE);
            assert!(
                size >= MIN_READABLE_SIZE,
                "help shrinks to {size} at {width}: {line}"
            );
        }
    }
}

#[test]
fn fill_presets_span_the_particle_range() {
    let counts: Vec<usize> = CONCENTRATION_PRESETS
        .iter()
        .map(|&concentration| {
            ConnectedChambers::with_initial_concentration(concentration)
                .particles
                .len()
        })
        .collect();
    assert_eq!(counts.first(), Some(&MIN_CONNECTED_MARKERS));
    assert_eq!(counts.last(), Some(&MAX_SIMULATED_MARKERS));
    assert!(counts.windows(2).all(|pair| pair[0] < pair[1]));
}
