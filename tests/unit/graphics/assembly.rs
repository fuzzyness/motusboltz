//! Tests for the pistons, rods, crankshaft, flywheel and stands,
//! `src/graphics/assembly.rs`.

use super::*;
use crate::graphics::outline::outside_block;
use crate::sim::engine::PARTICLE_RADIUS;
use crate::sim::geometry::DEFAULT_GEOMETRY;
use macroquad::prelude::Vec2;

const ANGLES: usize = 24;
const TOLERANCE: f32 = 1e-3;

fn assembly() -> (BlockShape, Assembly) {
    let pistons = PowerPistons::new(DEFAULT_GEOMETRY, PARTICLE_RADIUS);
    let assembly = Assembly::build(
        DEFAULT_GEOMETRY,
        pistons.stroke() / 2.0,
        pistons.shaft_height(),
    );
    (BlockShape::new(DEFAULT_GEOMETRY), assembly)
}

/// The piston face for a crank at `angle`, by the simulation's crank
/// geometry: the rod's length follows from where the shaft sits.
fn face(layout: Layout, angle: f32) -> f32 {
    let rod = layout.shaft_y - layout.block_top + layout.crank_radius;
    let (sin, cos) = angle.sin_cos();
    layout.shaft_y + layout.crank_radius * cos
        - (rod * rod - (layout.crank_radius * sin).powi(2)).sqrt()
}

/// Poses the assembly at each test angle and hands each pose to `check`.
fn each_pose(assembly: &mut Assembly, mut check: impl FnMut(&Assembly, f32, [f32; 2])) {
    let layout = assembly.layout;
    for step in 0..ANGLES {
        let angle = TAU * step as f32 / ANGLES as f32;
        let faces = [
            face(layout, angle + HOT_PHASE),
            face(layout, angle + COLD_PHASE),
        ];
        assembly.pose(angle, faces[0], faces[1]);
        check(assembly, angle, faces);
    }
}

/// The posed points of every part that moves as `wanted` says.
fn points(assembly: &Assembly, wanted: impl Fn(Motion) -> bool) -> Vec<Vec3> {
    assembly
        .parts
        .iter()
        .filter(|part| wanted(part.motion))
        .flat_map(|part| assembly.mesh.vertices[part.vertices.clone()].iter())
        .map(|vertex| vertex.position)
        .collect()
}

#[test]
fn everything_fits_one_draw_call() {
    let (_, assembly) = assembly();
    assert!(assembly.mesh.vertices.len() <= DRAW_CALL_VERTEX_CAPACITY);
    assert!(assembly.mesh.indices.len() <= DRAW_CALL_INDEX_CAPACITY);
    let count = assembly.mesh.vertices.len();
    assert!(
        assembly
            .mesh
            .indices
            .iter()
            .all(|&index| (index as usize) < count)
    );
}

#[test]
fn pistons_ride_on_the_gas_inside_their_bores() {
    let (shape, mut assembly) = assembly();
    each_pose(&mut assembly, |assembly, _, faces| {
        for (hot, face) in [(true, faces[0]), (false, faces[1])] {
            let x = side_x(assembly.layout, hot);
            let piston = points(
                assembly,
                |motion| matches!(motion, Motion::Piston { hot: side } if side == hot),
            );
            let lowest = piston.iter().map(|point| point.y).fold(f32::MAX, f32::min);
            assert!(
                (lowest - face).abs() < TOLERANCE,
                "crown at {lowest}, face at {face}"
            );
            for point in piston {
                let out = Vec2::new(point.x - x, point.z).length();
                assert!(out <= shape.bore - PISTON_CLEARANCE * shape.bore + TOLERANCE);
            }
        }
    });
}

#[test]
fn rods_run_from_wrist_pin_to_crank_pin() {
    let (shape, mut assembly) = assembly();
    each_pose(&mut assembly, |assembly, angle, faces| {
        let layout = assembly.layout;
        for (hot, phase, face) in [(true, HOT_PHASE, faces[0]), (false, COLD_PHASE, faces[1])] {
            let x = side_x(layout, hot);
            let (small, big) = (
                layout.wrist_pin(x, face),
                layout.crank_pin(x, angle + phase),
            );
            let axis = (big - small).normalize();
            let rod = points(
                assembly,
                |motion| matches!(motion, Motion::Rod { hot: side, .. } if side == hot),
            );
            let (mut low, mut high) = (f32::MAX, f32::MIN);
            for point in rod {
                let along = (point - small).dot(axis);
                let off = (point - small - axis * along).length();
                // The eye's corners: its radius out, and half the rod's
                // thickness along the pin.
                let corner = BIG_END_RADIUS.hypot(ROD_THICKNESS / 2.0) * layout.unit;
                assert!(off <= corner + TOLERANCE);
                (low, high) = (low.min(along), high.max(along));
                if point.y < shape.top {
                    // Below the block's top, a rod is inside its bore.
                    assert!(Vec2::new(point.x - x, point.z).length() < shape.bore);
                }
            }
            assert!((low + SMALL_END_RADIUS * layout.unit).abs() < TOLERANCE);
            let reach = big.distance(small) + BIG_END_RADIUS * layout.unit;
            assert!((high - reach).abs() < TOLERANCE);
        }
    });
}

#[test]
fn shaft_flywheel_and_stands_stay_above_the_block_and_clear_the_pistons() {
    let (shape, mut assembly) = assembly();
    each_pose(&mut assembly, |assembly, _, _| {
        let above = points(assembly, |motion| {
            matches!(motion, Motion::Shaft | Motion::Fixed)
        });
        assert!(above.iter().all(|point| point.y >= shape.top - TOLERANCE));
        let pistons = points(assembly, |motion| matches!(motion, Motion::Piston { .. }));
        let piston_inner = pistons
            .iter()
            .map(|point| point.x.abs())
            .fold(f32::MAX, f32::min);
        // The rim, the part that comes lowest, is narrower than the gap
        // between the pistons.
        let rim_half = RIM_WIDTH / 2.0 * shape.bore;
        assert!(rim_half < piston_inner, "the rim reaches the pistons");
        // The flywheel's lowest point is above every piston's top.
        let wheel_low = assembly.layout.shaft_y - FLYWHEEL_RADIUS * shape.bore;
        let piston_top = pistons.iter().map(|point| point.y).fold(f32::MIN, f32::max);
        assert!(wheel_low > piston_top);
    });
}

#[test]
fn stands_rest_on_the_block_between_the_bores() {
    let (shape, assembly) = assembly();
    let feet: Vec<Vec3> = points(&assembly, |motion| matches!(motion, Motion::Fixed))
        .into_iter()
        .filter(|point| point.y < shape.top + TOLERANCE)
        .collect();
    assert!(!feet.is_empty());
    for point in feet {
        let flat = Vec2::new(point.x, point.z);
        assert!(
            outside_block(shape.offset, shape.outer, flat) < 0.0,
            "{point} is off the block"
        );
        for axis in [-shape.offset, shape.offset] {
            assert!(
                Vec2::new(point.x - axis, point.z).length() > shape.bore,
                "{point} is over a bore"
            );
        }
    }
}

#[test]
fn legs_meet_their_pads_without_a_gap() {
    let (shape, assembly) = assembly();
    let unit = assembly.layout.unit;
    let (pad, half_width) = (FOOT_HEIGHT * unit, LEG_HALF_WIDTH * unit);
    let foot = vec3(STAND_X * unit, shape.top, FOOT_Z * unit);
    let top = vec3(
        STAND_X * unit,
        assembly.layout.shaft_y,
        0.5 * BEARING_RADIUS * unit,
    );
    let start = leg_start(foot, top, pad, half_width);
    let lean = (top - start).normalize();
    // The end's edges, across the lean: one meets the pad's top, the other
    // stays inside the pad.
    let across = Vec3::X.cross(lean).normalize();
    let heights = [start + across * half_width, start - across * half_width].map(|edge| edge.y);
    let (low, high) = (heights[0].min(heights[1]), heights[0].max(heights[1]));
    assert!(
        (high - (shape.top + pad)).abs() < 0.05 * pad,
        "gap under the leg"
    );
    assert!(low > shape.top, "the leg pokes through the pad");
}
