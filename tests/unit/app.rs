//! Tests for `src/app.rs`: the build label, the first camera view and the
//! fill presets.

use super::*;
use crate::graphics::assembly::flywheel_radius;
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
    assert!(crate::ui::fonts::drawable(BUILD_LABEL));
    assert!(crate::ui::fonts::drawable(LIMITED_TEXT));
}

/// The scene fills the window and the camera looks at its middle. The
/// floors, the crank and the flywheel all land in the centered span that
/// the panel and the sliders leave clear (see `layout::scene_span`), in
/// every window, with the panel shown or hidden.
#[test]
fn initial_camera_frames_the_engine_in_the_clear_span() {
    let geometry = DEFAULT_GEOMETRY;
    let pistons = PowerPistons::new(geometry, PARTICLE_RADIUS);
    let camera = initial_camera(geometry).camera();
    for (width, height) in WINDOWS {
        // Recreate the perspective transform without a window.
        let matrix =
            Mat4::perspective_rh_gl(camera.fovy, width / height, camera.z_near, camera.z_far)
                * Mat4::look_at_rh(camera.position, camera.target, camera.up);
        for show_panel in [true, false] {
            let (left, span) = scene_span(width, show_panel);
            // The span's edges in clip space.
            let clip_x = |x: f32| 2.0 * x / width - 1.0;
            let (low, high) = (clip_x(left), clip_x(left + span));
            let wheel = flywheel_radius(geometry);
            for point in [
                vec3(0.0, pistons.shaft_height() + wheel, 0.0),
                vec3(0.0, pistons.shaft_height(), wheel),
                vec3(0.0, pistons.shaft_height(), -wheel),
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
                let (x, y) = (clip.x / clip.w, clip.y / clip.w);
                assert!(
                    x > low && x < high && y.abs() < 1.0,
                    "{point:?} outside the clear span in a {width}x{height} window"
                );
            }
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
