//! Tests for the burner and its flames, `src/graphics/fire.rs`.

use super::*;
use crate::graphics::outline::outside_block;
use crate::sim::geometry::DEFAULT_GEOMETRY;

/// Moments to sample the animation at, including a long-running clock.
const TIMES: [f64; 6] = [0.0, 0.17, 0.5, 1.3, 7.9, 86_400.25];

fn fire() -> (BlockShape, Fire) {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    (shape, Fire::new(shape))
}

/// The first view's direction.
fn facing() -> Facing {
    Facing::along(Vec3::new(-1.0, -0.66, -1.0).normalize())
}

/// Every sprite's middle and color at `heat`, over all the sample times.
fn sprites_at(fire: &mut Fire, heat: f32) -> Vec<(Vec3, Color)> {
    TIMES
        .iter()
        .flat_map(|&seconds| {
            fire.build(heat, seconds, facing());
            fire.centers()
        })
        .collect()
}

#[test]
fn heat_level_runs_from_off_at_room_temperature_to_full_at_the_top() {
    assert_eq!(heat_level(MIN_HOT_KELVIN), 0.0);
    assert_eq!(heat_level(MAX_HOT_KELVIN), 1.0);
    assert!((heat_level(750.0) - 0.5).abs() < 1e-6);
    assert_eq!(heat_level(250.0), 0.0);
    assert_eq!(heat_level(5_000.0), 1.0);
}

#[test]
fn burner_is_out_at_300_k_and_lit_above_it() {
    let (_, mut fire) = fire();
    assert!(sprites_at(&mut fire, heat_level(MIN_HOT_KELVIN)).is_empty());
    let lit = sprites_at(&mut fire, heat_level(310.0));
    assert!(!lit.is_empty(), "a little heat shows a little flame");
    assert!(
        !fire.burner.indices.is_empty(),
        "the burner shows even when out"
    );
}

#[test]
fn flames_grow_taller_and_whiter_as_the_hot_side_heats() {
    let (shape, mut fire) = fire();
    let tallest = |fire: &mut Fire, heat| {
        sprites_at(fire, heat)
            .iter()
            .map(|(center, _)| center.y - shape.base)
            .fold(0.0, f32::max)
    };
    let low = tallest(&mut fire, heat_level(400.0));
    let middle = tallest(&mut fire, heat_level(750.0));
    let high = tallest(&mut fire, heat_level(1_200.0));
    assert!(low < middle && middle < high, "{low} {middle} {high}");
    assert!(high < shape.top - shape.base, "flames stay below the top");
    // The roots' blue component rises as they turn toward white.
    assert!(mix_color(ROOT_LOW, ROOT_HIGH, 1.0).b > mix_color(ROOT_LOW, ROOT_HIGH, 0.1).b);
}

#[test]
fn flames_stay_beside_the_hot_half_and_outside_the_block() {
    let (shape, mut fire) = fire();
    let sprites = sprites_at(&mut fire, 1.0);
    assert!(sprites.len() <= TIMES.len() * crate::graphics::sprites::MAX_SPRITES);
    let mut upright = 0;
    for (center, _) in sprites {
        if center.y < shape.base + 2.0 * ABOVE_FLOOR {
            continue; // The glow on the floor, which spreads under the block.
        }
        upright += 1;
        let point = Vec2::new(center.x, center.z);
        assert!(point.x < 0.0, "{center} is on the cold half");
        assert!(
            outside_block(shape.offset, shape.outer, point) > 0.0,
            "{center} is inside the block"
        );
    }
    assert!(upright > 0);
}

#[test]
fn burner_sits_on_the_floor_around_the_hot_half() {
    let (shape, fire) = fire();
    let gap = BURNER_GAP * shape.bore;
    let half_width = BURNER_WIDTH * shape.bore / 2.0;
    for vertex in &fire.burner.vertices {
        let position = vertex.position;
        assert!(position.y >= shape.base - 1e-5);
        assert!(position.y <= shape.base + BURNER_HEIGHT * shape.bore + 1e-5);
        assert!(position.x < 0.0);
        let out = outside_block(shape.offset, shape.outer, Vec2::new(position.x, position.z));
        assert!(
            (out - gap).abs() <= half_width + 1e-3,
            "{position} is {out} out"
        );
    }
    assert!(fire.burner.vertices.len() < crate::graphics::DRAW_CALL_VERTEX_CAPACITY);
}
