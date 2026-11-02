//! Tests for `src/graphics/regenerator.rs`: the screens' mesh and their
//! tint.

use super::*;
use crate::sim::geometry::DEFAULT_GEOMETRY;

/// Hot wall in the tint tests.
const HOT_KELVIN: f32 = 1_200.0;
/// Cold wall in the tint tests.
const COLD_KELVIN: f32 = 300.0;

/// Largest color channel difference treated as equal.
const TOLERANCE: f32 = 1e-5;

/// Whether `a` and `b` match in every channel.
fn same(a: Color, b: Color) -> bool {
    [a.r - b.r, a.g - b.g, a.b - b.b, a.a - b.a]
        .iter()
        .all(|difference| difference.abs() < TOLERANCE)
}

/// The default stack's screens.
fn screens() -> RegeneratorScreens {
    let regenerator = Regenerator::new(DEFAULT_GEOMETRY, 1, 1.0, 0);
    RegeneratorScreens::new(DEFAULT_GEOMETRY, regenerator.screens())
}

#[test]
fn every_screen_has_its_own_run_of_vertices_in_the_opening() {
    let screens = screens();
    assert_eq!(screens.starts.len(), DRAWN_SCREENS + 1);
    assert!(screens.starts.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(screens.starts[DRAWN_SCREENS], screens.mesh.vertices.len());
    let half_wall = DEFAULT_GEOMETRY.shared_wall_thickness() / 2.0;
    let center_y = DEFAULT_GEOMETRY.passage_center_y();
    for vertex in &screens.mesh.vertices {
        let position = vertex.position;
        assert!(position.x.abs() < half_wall + WIRE_RADIUS);
        let from_axis = vec3(0.0, position.y - center_y, position.z).length();
        assert!(from_axis <= DEFAULT_GEOMETRY.passage_radius);
    }
}

#[test]
fn the_tint_runs_from_cobalt_through_violet_to_copper() {
    let tint = |kelvin| tint(kelvin, HOT_KELVIN, COLD_KELVIN);
    assert!(same(tint(COLD_KELVIN), COLD_WIRE));
    assert!(same(tint((HOT_KELVIN + COLD_KELVIN) / 2.0), MIDDLE_WIRE));
    assert!(same(tint(HOT_KELVIN), HOT_WIRE));
    assert!(
        same(tint(0.0), COLD_WIRE),
        "below the cold wall stays cobalt"
    );
    assert!(
        same(tint(2.0 * HOT_KELVIN), HOT_WIRE),
        "above the hot wall stays copper"
    );
    let equal = super::tint(COLD_KELVIN, COLD_KELVIN, COLD_KELVIN);
    assert!(equal.r.is_finite());
}

#[test]
fn every_four_screens_are_drawn_as_one_at_their_middle_and_mean() {
    const {
        assert!(SCREEN_COUNT.is_multiple_of(DRAWN_SCREENS));
    }
    let screens = (0..SCREEN_COUNT).map(|index| Screen {
        x: index as f32,
        kelvin: 100.0 * index as f32,
    });
    let drawn = drawn(screens);
    assert_eq!(drawn.len(), DRAWN_SCREENS);
    let middle = (SCREENS_PER_DRAWN - 1) as f32 / 2.0;
    assert_eq!(drawn[0].x, middle);
    assert_eq!(drawn[0].kelvin, 100.0 * middle);
}
