//! Tests for `src/ui/layer.rs`. Drawing needs a window, so these check the
//! per-frame choice and the copy shader's source, which do not.

use super::*;

const PANEL: (u32, u32) = (340, 620);

#[test]
fn the_first_frame_makes_the_image() {
    assert_eq!(step(None, PANEL, false, true), Step::Make);
}

#[test]
fn an_unchanged_area_is_only_copied() {
    assert_eq!(step(Some(PANEL), PANEL, false, true), Step::Copy);
}

#[test]
fn changed_content_is_repainted_into_the_same_image() {
    assert_eq!(step(Some(PANEL), PANEL, true, true), Step::Repaint);
}

#[test]
fn a_resized_window_gets_a_new_image() {
    assert_eq!(step(Some(PANEL), (340, 580), false, true), Step::Make);
    // A Retina screen needs twice the pixels for the same area.
    assert_eq!(step(Some(PANEL), (680, 1240), false, true), Step::Make);
}

#[test]
fn without_the_copy_shader_or_room_it_draws_directly() {
    assert_eq!(step(None, PANEL, false, false), Step::Direct);
    assert_eq!(step(Some(PANEL), PANEL, false, false), Step::Direct);
    let tall = (340, MAX_IMAGE_SIDE + 1);
    assert_eq!(step(Some(PANEL), tall, false, true), Step::Direct);
    assert_eq!(step(None, (340, MAX_IMAGE_SIDE), false, true), Step::Make);
}

#[test]
fn copy_shaders_are_glsl_100() {
    for source in [COPY_VERTEX, COPY_FRAGMENT] {
        assert!(source.starts_with("#version 100\n"));
        assert!(source.is_ascii());
    }
    // Blending is off and alpha is forced to one, so the copy is opaque.
    assert!(COPY_FRAGMENT.contains("vec4(texture2D(Texture, uv).rgb, 1.0)"));
}
