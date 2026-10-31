//! Tests for the line around each half of the block, `src/graphics/outline.rs`.

use super::*;

/// How far `point`, seen from above, is outside the block whose bores are
/// `offset` from its middle and whose rounded ends have radius `outer`;
/// negative inside.
///
/// It is for the tests of everything placed around the block.
pub(crate) fn outside_block(offset: f32, outer: f32, point: Vec2) -> f32 {
    let nearest = vec2(point.x.clamp(-offset, offset), 0.0);
    point.distance(nearest) - outer
}
use crate::graphics::block::BlockShape;
use crate::sim::geometry::DEFAULT_GEOMETRY;

/// Points sampled along each outline.
const SAMPLES: usize = 400;
/// How far the straight runs reach toward the middle.
const REACH: f32 = 0.85;
/// Distance of the outlines out from the block.
const GAP: f32 = 0.3;

/// The default block and an outline around each half.
fn outlines() -> (BlockShape, [HalfOutline; 2]) {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let outline = |half| HalfOutline::new(half, shape.offset, shape.outer + GAP, REACH);
    (shape, [outline(Half::Hot), outline(Half::Cold)])
}

#[test]
fn line_keeps_its_distance_from_the_block_on_its_own_half() {
    let (shape, outlines) = outlines();
    for outline in outlines {
        let side = outline.axis().x.signum();
        assert_eq!(outline.axis().x, side * shape.offset);
        for step in 0..=SAMPLES {
            let (point, out) = outline.at(step as f32 / SAMPLES as f32);
            let distance = outside_block(shape.offset, shape.outer, point);
            assert!((distance - GAP).abs() < 1e-4, "{point} is {distance} out");
            assert!((out.length() - 1.0).abs() < 1e-5);
            // The middle tenth of the block is left to the other half.
            assert!(point.x * side >= (1.0 - REACH) * shape.offset - 1e-4);
        }
    }
}

#[test]
fn line_is_continuous_and_points_out_square_to_its_direction() {
    let (_, outlines) = outlines();
    for outline in outlines {
        let step_length = outline.length() / SAMPLES as f32;
        let mut previous = outline.at(0.0).0;
        for step in 1..=SAMPLES {
            let (point, out) = outline.at(step as f32 / SAMPLES as f32);
            let moved = point - previous;
            assert!((moved.length() - step_length).abs() < 0.01 * step_length);
            assert!(moved.normalize().dot(out).abs() < 0.05);
            previous = point;
        }
    }
}

#[test]
fn cold_line_mirrors_the_hot_line() {
    let (_, [hot, cold]) = outlines();
    for step in 0..=SAMPLES {
        let fraction = step as f32 / SAMPLES as f32;
        let ((hot_point, hot_out), (cold_point, cold_out)) = (hot.at(fraction), cold.at(fraction));
        assert!((hot_point * vec2(-1.0, 1.0) - cold_point).length() < 1e-5);
        assert!((hot_out * vec2(-1.0, 1.0) - cold_out).length() < 1e-5);
    }
    // Both start on the front (+z) side, which faces the first view.
    assert!(hot.at(0.0).0.y > 0.0 && cold.at(0.0).0.y > 0.0);
}
