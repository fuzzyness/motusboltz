//! Button layout and stats formatting regression tests.
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
    assert_eq!(BUTTON_ACTIONS.len(), 3);
    assert_eq!(BUTTON_ACTIONS[2], Action::KickNegative);
}

#[test]
fn initial_camera_frames_lower_passage_and_longer_linkage() {
    use crate::engine::{DEFAULT_GEOMETRY, PowerPistons};
    use macroquad::prelude::{Mat4, Vec3, vec3};
    const PARTICLE_RADIUS: f32 = 0.13;
    const WHEEL_RADIUS: f32 = 2.10;
    const VIEW_ASPECT: f32 = 1440.0 / 630.0;
    let geometry = DEFAULT_GEOMETRY;
    let pistons = PowerPistons::new(geometry, PARTICLE_RADIUS);
    let mut camera = initial_camera(geometry).camera();
    camera.aspect = Some(VIEW_ASPECT);
    // Recreate the perspective transform without a window/context.
    let matrix = Mat4::perspective_rh_gl(camera.fovy, VIEW_ASPECT, camera.z_near, camera.z_far)
        * Mat4::look_at_rh(camera.position, camera.target, camera.up);
    for point in [
        vec3(0.0, pistons.shaft_height() + WHEEL_RADIUS, 0.0),
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
            "clipped scene point {point:?}"
        );
    }
}

/// Macroquad's built-in font is only relied on for ASCII.
#[test]
fn fixed_canvas_text_is_ascii() {
    for text in [
        HELP_TEXT,
        TITLE_RUNNING,
        TITLE_PAUSED,
        "Reset (T)",
        "Pause (Space)",
        "Resume (Space)",
        "Kick (A)",
    ] {
        assert!(text.is_ascii(), "non-ASCII canvas text: {text:?}");
    }
}
