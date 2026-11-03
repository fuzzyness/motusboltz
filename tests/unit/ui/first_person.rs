//! Tests for first-person mode, `src/ui/first_person.rs`.

use super::*;
use crate::graphics::table::{TOP_HALF_DEPTH, TOP_HALF_LENGTH, top_height};
use crate::sim::geometry::DEFAULT_GEOMETRY;

/// The figure starts on the table, outside the block, its eyes about as
/// high as the engine, looking at the engine.
#[test]
fn the_figure_starts_on_the_table_facing_the_engine() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let walker = Walker::standing(shape);
    let eye = walker.eye;
    assert!(eye.x.abs() < TOP_HALF_LENGTH * shape.bore);
    assert!(eye.z.abs() < TOP_HALF_DEPTH * shape.bore);
    assert!(eye.z > shape.outer + shape.bore, "in front of the block");
    assert!(eye.y > top_height(shape) + shape.bore);
    let toward = (vec3(0.0, eye.y, 0.0) - eye).normalize();
    let level = vec3(walker.forward().x, 0.0, walker.forward().z).normalize();
    assert!(level.dot(toward) > 0.99, "it faces the engine");
    assert!(walker.forward().y < 0.0, "looking a little down");
    assert!((walker.forward().length() - 1.0).abs() < 1e-5);
}

/// The mouse turns the head from where it was first read, and the head
/// never tips past straight up or down.
#[test]
fn the_mouse_turns_the_head() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let mut walker = Walker::standing(shape);
    let start = walker;
    // Still at first, then the capture's jumps: none of it turns the head.
    walker.look(vec2(500.0, 500.0));
    walker.look(vec2(500.0, 500.0));
    walker.look(vec2(-3000.0, -2000.0));
    walker.look(vec2(720.0, 450.0));
    assert_eq!((walker.yaw, walker.pitch), (start.yaw, start.pitch));
    walker.look(vec2(820.0, 450.0));
    assert!(walker.yaw < start.yaw, "right turns right");
    let before = walker.yaw;
    walker.look(vec2(-100_000.0, 450.0));
    let most = MAX_LOOK_STEP * LOOK_SENSITIVITY;
    assert!(
        (walker.yaw - before - most).abs() < 1e-4,
        "a stray jump is held back"
    );
    for step in 1..=100 {
        walker.look(vec2(-100_000.0, -100.0 * step as f32));
    }
    assert!(walker.pitch < FRAC_PI_2 && walker.pitch > 1.0);
    walker.open_menu();
    walker.close_menu();
    let held = walker;
    walker.look(vec2(0.0, 0.0));
    walker.look(vec2(-5000.0, -5000.0));
    assert_eq!(walker.yaw, held.yaw, "a fresh capture's jump is ignored");
    let camera = walker.camera();
    assert_eq!(camera.position, walker.eye);
}

/// The menu's two buttons are centered, one above the other, and each
/// answers to a click inside it.
#[test]
fn the_menu_offers_resume_then_exit() {
    let (width, height) = (1440.0, 900.0);
    let [(first, resume), (second, exit)] = menu_buttons(width, height);
    assert_eq!((first, second), (Choice::Resume, Choice::Exit));
    assert!(resume.bottom() < exit.y);
    assert!((resume.center().x - width / 2.0).abs() < 1e-4);
    assert_eq!(
        menu_choice(width, height, resume.center()),
        Some(Choice::Resume)
    );
    assert_eq!(
        menu_choice(width, height, exit.center()),
        Some(Choice::Exit)
    );
    assert_eq!(menu_choice(width, height, vec2(5.0, 5.0)), None);
}
