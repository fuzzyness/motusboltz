//! Tests for `src/ui/scene_labels.rs`: the tags sit on their parts, follow
//! the camera and stay in the scene.

use super::*;
use crate::app::initial_camera;
use crate::sim::geometry::DEFAULT_GEOMETRY;
use crate::sim::units::DEFAULT_PARTICLES;
use crate::ui::controls::row_top;
use crate::ui::fonts::tests::drawable;
use crate::ui::layout::scene_span;

/// A filled engine of the default size.
fn engine() -> ConnectedChambers {
    const SEED: u64 = 42;
    ConnectedChambers::with_geometry_seed(DEFAULT_PARTICLES, DEFAULT_GEOMETRY, SEED)
}

/// The scene of a window `width` by `height` with the panel shown.
fn area(width: f32, height: f32) -> SceneArea {
    let (left, span) = scene_span(width, true, true);
    SceneArea {
        edge: left,
        left,
        width: span,
        window_width: width,
        height,
        sliders: true,
    }
}

/// The starting camera's matrix for a window `width` by `height`, made
/// without a window as `Camera3D::matrix` makes it.
fn matrix(width: f32, height: f32) -> Mat4 {
    let camera = initial_camera(DEFAULT_GEOMETRY).camera();
    Mat4::perspective_rh_gl(camera.fovy, width / height, camera.z_near, camera.z_far)
        * Mat4::look_at_rh(camera.position, camera.target, camera.up)
}

#[test]
fn the_hot_tag_is_left_of_the_cold_one() {
    let tags = tags(&engine());
    let (width, height) = (1440.0, 900.0);
    let size = vec2(width, height);
    let hot = project(matrix(width, height), size, tags[1].anchor).unwrap();
    let cold = project(matrix(width, height), size, tags[2].anchor).unwrap();
    assert!(hot.x < width / 2.0 && cold.x > width / 2.0);
    assert!(
        tags.iter()
            .all(|tag| drawable(tag.name) && drawable(tag.role))
    );
}

#[test]
fn a_point_behind_the_camera_has_no_place() {
    let matrix = matrix(1440.0, 900.0);
    let size = vec2(1440.0, 900.0);
    let ahead = project(matrix, size, Vec3::ZERO).unwrap();
    assert!((ahead - size / 2.0).length() < size.y / 4.0);
    let eye = initial_camera(DEFAULT_GEOMETRY).camera().position;
    assert!(project(matrix, size, eye + (eye - Vec3::ZERO)).is_none());
}

#[test]
fn the_tags_stay_in_the_scene_apart() {
    let engine = engine();
    let tags = tags(&engine);
    for (width, height) in [(1440.0, 900.0), (1280.0, 580.0)] {
        let area = area(width, height);
        // A card as wide as the sliders' in the scene's bottom-left
        // corner, where the energy strip goes in a short window.
        let corner = Rect::new(area.edge, row_top(height) - 130.0, 300.0, 90.0);
        for cards in [vec![], vec![corner]] {
            let pen = Pen::offline();
            let placed = place(&pen, &tags, matrix(width, height), area, &cards);
            let placed: Vec<_> = placed
                .into_iter()
                .map(|placed| placed.map(|(dot, rect, _)| (dot, rect)))
                .collect();
            assert!(placed[0].is_some(), "the system always has its tag");
            let boxes: Vec<Rect> = placed.into_iter().flatten().map(|(_, rect)| rect).collect();
            for (index, rect) in boxes.iter().enumerate() {
                assert!(rect.x >= area.edge && rect.right() <= width);
                assert!(!slider_column(width).overlaps(rect), "clear of the sliders");
                assert!(rect.y > TOGGLES_BOTTOM && rect.bottom() < row_top(height));
                let earlier = cards.iter().chain(&boxes[..index]);
                assert!(earlier.into_iter().all(|other| !other.overlaps(rect)));
            }
        }
    }
}

#[test]
fn a_tag_grows_out_of_its_dot() {
    let dot = vec2(10.0, 10.0);
    let rect = Rect::new(100.0, 50.0, 80.0, 30.0);
    let gone = grown_from(dot, rect, 0.0);
    assert_eq!((gone.x, gone.y, gone.w, gone.h), (10.0, 10.0, 0.0, 0.0));
    assert_eq!(grown_from(dot, rect, 1.0), rect);
    let half = grown_from(dot, rect, 0.5);
    assert_eq!(half.center(), dot.lerp(rect.center(), 0.5));
    assert_eq!(half.w, 40.0);
}

/// The buttons always have tags; the cartridge has one only while it lies
/// on the table, and the opening's names the regenerator once it is in.
#[test]
fn the_parts_to_press_have_tags() {
    let mut engine = engine();
    let names = |engine: &ConnectedChambers| -> Vec<&str> {
        tags(engine).iter().map(|tag| tag.name).collect()
    };
    let out = names(&engine);
    assert!(out.contains(&"Reset") && out.contains(&"Tracers"));
    assert_eq!(out.iter().filter(|name| **name == "Regenerator").count(), 1);
    engine.set_regenerator(true);
    let inside = names(&engine);
    assert_eq!(inside.len(), out.len() - 1);
    assert_eq!(
        inside.iter().filter(|name| **name == "Regenerator").count(),
        1
    );
}

/// A tag moved aside glides to its new place instead of jumping; a new tag
/// takes its place at once.
#[test]
fn a_crowded_tag_glides_aside() {
    let mut shifts = Shifts::default();
    let target = vec2(0.0, -60.0);
    assert_eq!(shifts.ease("Tracers", Vec2::ZERO), Vec2::ZERO);
    shifts.tick(1.0 / 60.0);
    let first = shifts.ease("Tracers", target);
    assert!(first.y < 0.0 && first.y > target.y / 2.0, "{first}");
    for _ in 0..60 {
        shifts.ease("Tracers", target);
    }
    assert!(shifts.current("Tracers").unwrap().distance(target) < 0.1);
    assert_eq!(shifts.ease("Pause", target), target);
}

/// A point that should count as over `spot`, seen from `eye`: a ball's
/// middle, a wall's outline away from the other cylinder, a bore's middle.
fn inside(spot: Spot, eye: Vec3) -> Vec3 {
    match spot {
        Spot::Ball { center, .. } => center,
        Spot::Rim {
            axis,
            inner,
            outer,
            low,
            high,
            side,
        } => {
            let looking = (vec2(axis.x, axis.y) - vec2(eye.x, eye.z)).normalize();
            let across = vec2(-looking.y, looking.x);
            let across = if across.x * side >= 0.0 {
                across
            } else {
                -across
            };
            let edge = vec2(axis.x, axis.y) + across * (inner + outer) / 2.0;
            vec3(edge.x, (low + high) / 2.0, edge.y)
        }
        Spot::Bore {
            axis, low, high, ..
        } => vec3(axis.x, (low + high) / 2.0, axis.y),
    }
}

/// Over each of a part's spots, its tag and no other is under the pointer,
/// and nothing is far from the engine; the hand crank and both valves
/// have tags, and the gas has a leader to each bore.
#[test]
fn one_tag_is_under_the_pointer_over_its_part() {
    let engine = engine();
    let tags = tags(&engine);
    for name in ["Hand crank", "Gas valve", "Nitrogen valve"] {
        assert!(tags.iter().any(|tag| tag.name == name), "{name}");
    }
    assert!(tags[0].also.is_some() && tags[0].spots.len() == 2);
    let (width, height) = (1440.0, 900.0);
    let (matrix, size) = (matrix(width, height), vec2(width, height));
    let eye = initial_camera(DEFAULT_GEOMETRY).camera().position;
    for (index, tag) in tags.iter().enumerate() {
        for spot in &tag.spots {
            let point = project(matrix, size, inside(*spot, eye)).unwrap();
            let over = hovered(&tags, matrix, size, point);
            let winner: Vec<_> = tags
                .iter()
                .zip(&over)
                .filter(|(_, o)| **o)
                .map(|(t, _)| t.name)
                .collect();
            assert!(over[index], "{} at {point}: {winner:?}", tag.name);
            assert_eq!(over.iter().filter(|&&over| over).count(), 1);
        }
    }
    let corner = vec2(2.0, 2.0);
    assert!(
        hovered(&tags, matrix, size, corner)
            .iter()
            .all(|&over| !over)
    );
}

/// The opening, between the bores, shows its own tag only; the walls'
/// tags wait for the pointer near their outlines.
#[test]
fn the_opening_and_the_walls_keep_apart() {
    let engine = engine();
    let tags = tags(&engine);
    let (width, height) = (1440.0, 900.0);
    let (matrix, size) = (matrix(width, height), vec2(width, height));
    let opening = tags.iter().position(|tag| tag.name == "Opening").unwrap();
    let over = hovered(
        &tags,
        matrix,
        size,
        project(matrix, size, tags[opening].anchor).unwrap(),
    );
    assert!(over[opening] && over.iter().filter(|&&over| over).count() == 1);
    // Halfway between a wall's outline and its axis is the gas, not the
    // wall.
    let geometry = engine.geometry();
    let halfway = vec3(
        geometry.hot_center_x() - geometry.cylinder_radius / 2.0,
        0.0,
        0.0,
    );
    let over = hovered(&tags, matrix, size, project(matrix, size, halfway).unwrap());
    assert!(over[0] && !over[1], "{over:?}");
}

/// Every tag has its own key, and the opening's keeps its key when the
/// regenerator goes in and the tag takes the name the cartridge's had, so
/// one never takes over the other's growth or place.
#[test]
fn each_tag_keeps_its_own_key() {
    let mut engine = engine();
    let keys = |engine: &ConnectedChambers| -> Vec<&str> {
        tags(engine).iter().map(|tag| tag.key).collect()
    };
    let out = keys(&engine);
    let mut unique = out.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), out.len());
    assert!(out.contains(&CARTRIDGE_KEY));
    engine.set_regenerator(true);
    let opening = tags(&engine)
        .into_iter()
        .find(|tag| tag.name == "Regenerator")
        .unwrap();
    assert_eq!(opening.key, "opening");
    assert!(!keys(&engine).contains(&CARTRIDGE_KEY));
}
