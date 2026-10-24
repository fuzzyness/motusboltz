//! Button layout, scene layout and camera framing tests.
use super::*;

#[test]
fn button_rows_do_not_overlap_sliders() {
    let button_bottom =
        CONTROL_TOP + (BUTTON_ROWS - 1) as f32 * (BUTTON_HEIGHT + BUTTON_GAP) + BUTTON_HEIGHT;
    let slider_top = crate::slider::HOT_Y - crate::slider::PANEL_PADDING_TOP;
    assert!(button_bottom + BUTTON_GAP < slider_top);
}

#[test]
fn two_column_buttons_do_not_overlap() {
    for index in 0..BUTTON_ACTIONS.len() {
        let (x, y, width) = button_rect(index, 800.0);
        assert!(width > 0.0);
        for other in index + 1..BUTTON_ACTIONS.len() {
            let (ox, oy, owidth) = button_rect(other, 800.0);
            assert!(
                x + width <= ox
                    || ox + owidth <= x
                    || y + BUTTON_HEIGHT <= oy
                    || oy + BUTTON_HEIGHT <= y
            );
        }
    }
}

#[test]
fn buttons_have_one_action_per_slot() {
    assert_eq!(BUTTON_ACTIONS.len(), BUTTON_ROWS * 2);
    assert_eq!(BUTTON_ACTIONS[2], Action::KickNegative);
    assert_eq!(BUTTON_ACTIONS[3], Action::ToggleDetails);
}

/// Window sizes the scene must fit: the default, a common school laptop,
/// and a small browser window.
const WINDOWS: [(f32, f32); 3] = [(1440.0, 900.0), (1366.0, 768.0), (1280.0, 720.0)];

#[test]
fn scene_sits_between_the_panel_and_the_controls() {
    for (width, _) in WINDOWS {
        let (left, scene) = scene_span(width, true);
        assert_eq!(left, COLUMN_WIDTH);
        let (x, _, _) = button_rect(0, width);
        assert!(left + scene <= x - crate::slider::PANEL_PADDING_X + 1e-3);
        let (hidden_left, wider) = scene_span(width, false);
        assert_eq!(hidden_left, 0.0);
        assert!(wider > scene);
    }
}

#[test]
fn initial_camera_frames_lower_passage_and_longer_linkage() {
    use crate::engine::{DEFAULT_GEOMETRY, PowerPistons};
    use macroquad::prelude::{Mat4, Vec3, vec3};
    const PARTICLE_RADIUS: f32 = 0.13;
    const WHEEL_RADIUS: f32 = 2.10;
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
            vec3(-geometry.cylinder_offset, geometry.bottom(), 0.0),
            vec3(geometry.cylinder_offset, geometry.bottom(), 0.0),
            vec3(
                -geometry.cylinder_offset - geometry.cylinder_radius,
                0.0,
                0.0,
            ),
            vec3(
                geometry.cylinder_offset + geometry.cylinder_radius,
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

#[test]
fn scene_viewport_counts_physical_pixels() {
    const RETINA_SCALE: f32 = 2.0;
    for (width, height) in WINDOWS {
        let (left, scene) = scene_span(width, true);
        let standard = scene_viewport(width, height, true, 1.0);
        assert_eq!(standard, (left as i32, 0, scene as i32, height as i32));
        let (x, y, w, h) = scene_viewport(width, height, true, RETINA_SCALE);
        assert_eq!((x, y), (2 * standard.0, 0));
        assert!((w - 2 * standard.2).abs() <= 1 && (h - 2 * standard.3).abs() <= 1);
    }
}

#[test]
fn build_label_names_this_version() {
    assert!(BUILD_LABEL.starts_with(concat!("v", env!("CARGO_PKG_VERSION"), " (")));
    assert!(BUILD_LABEL.ends_with(')'));
}

/// Macroquad's built-in font is only relied on for ASCII.
#[test]
fn fixed_canvas_text_is_ascii() {
    for text in [
        HELP_TEXT,
        BUILD_LABEL,
        TITLE,
        "Reset (T)",
        "Pause (Space)",
        "Resume (Space)",
        "Kick (A)",
        "Details (D)",
        "Less (D)",
    ] {
        assert!(text.is_ascii(), "non-ASCII canvas text: {text:?}");
    }
}
