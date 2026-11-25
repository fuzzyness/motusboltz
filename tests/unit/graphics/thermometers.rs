//! Tests for the thermometers, `src/graphics/thermometers.rs`.

use super::*;
use crate::sim::geometry::DEFAULT_GEOMETRY;
use crate::sim::reservoir::{
    MAX_COLD_KELVIN,
    MAX_HOT_KELVIN,
    MIN_HOT_KELVIN, //
};

/// Each scale reaches over its wall's whole range from absolute zero, its
/// long ticks on round numbers, and the cold one stops at 77 K.
#[test]
fn the_scales_cover_the_walls_and_the_cold_one_stops_at_77_kelvin() {
    let hot = Scale::of(ReservoirSide::Hot);
    let cold = Scale::of(ReservoirSide::Cold);
    assert!(hot.top >= MAX_HOT_KELVIN && cold.top >= MAX_COLD_KELVIN);
    for scale in [hot, cold] {
        assert_eq!(scale.steps % scale.major, 0, "a long tick at the far end");
        let long = scale.top * scale.major as f32 / scale.steps as f32;
        assert_eq!(long % 100.0, 0.0, "numbers in hundreds");
    }
    assert_eq!(hot.share(0.0), 0.0, "the hot one reads from absolute zero");
    assert!(hot.share(MIN_HOT_KELVIN) > 0.0);
    assert_eq!(cold.stop, Some(MIN_COLD_KELVIN));
    assert_eq!(
        cold.share(0.0),
        cold.share(MIN_COLD_KELVIN),
        "stopped at the pin"
    );
    assert!(cold.share(MIN_COLD_KELVIN) > 0.0, "short of absolute zero");
    assert_eq!(cold.share(2.0 * cold.top), 1.0);
}

/// The scale sweeps from bottom left through straight up to bottom right.
#[test]
fn the_needle_sweeps_round_the_top_of_the_face() {
    assert!((needle_turn(0.5)).abs() < 1e-6);
    assert!((needle_turn(0.0) + 0.75 * PI).abs() < 1e-6);
    assert!((needle_turn(1.0) - 0.75 * PI).abs() < 1e-6);
}

/// The stems enter the block's top in steel, between each bore and the
/// front face, and the dials stand above it, clear of each other and of
/// the flywheel's stands between the bores.
#[test]
fn the_dials_stand_on_the_steel_clear_of_the_stands() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let unit = shape.bore;
    let stem = STEM_RADIUS * unit;
    for side in [ReservoirSide::Hot, ReservoirSide::Cold] {
        let foot = stem_foot(shape, side);
        let axis = Vec3::new(foot.x.signum() * shape.offset, foot.y, 0.0);
        assert!(foot.distance(axis) - stem > shape.bore, "outside the bore");
        assert!(foot.z + stem < shape.outer, "inside the front face");
        let middle = dial_middle(shape, side);
        let rim = (FACE_RADIUS + RIM_WIDTH) * unit;
        assert!(middle.y - rim > shape.top, "above the top");
        // The stands are 0.22 bore radii from the middle, their legs 0.05
        // either side.
        assert!(middle.x.abs() - rim > 0.27 * unit, "clear of the stands");
    }
}

/// The needles swing to the walls' temperatures, easing there, and the
/// cold one rests on its pin however cold it is asked to read.
#[test]
fn the_needles_follow_the_walls() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let mut thermometers = Thermometers {
        dials: [ReservoirSide::Hot, ReservoirSide::Cold].map(|side| Dial {
            side,
            body: GpuMesh::empty(),
            needle: GpuMesh::empty(),
            pin: dial_middle(shape, side),
            share: Scale::of(side).share(0.0),
        }),
        last_seconds: None,
        material: None,
    };
    thermometers.follow(900.0, 150.0, 0.0);
    let hot = Scale::of(ReservoirSide::Hot);
    let cold = Scale::of(ReservoirSide::Cold);
    assert_eq!(
        thermometers.share(ReservoirSide::Hot),
        hot.share(900.0),
        "at once at first"
    );
    thermometers.follow(300.0, 0.0, 0.05);
    let partway = thermometers.share(ReservoirSide::Hot);
    assert!(
        hot.share(300.0) < partway && partway < hot.share(900.0),
        "easing"
    );
    for frame in 1..=120 {
        thermometers.follow(300.0, 0.0, 0.05 + f64::from(frame) / 60.0);
    }
    assert!((thermometers.share(ReservoirSide::Hot) - hot.share(300.0)).abs() < 1e-4);
    let resting = thermometers.share(ReservoirSide::Cold);
    assert!(
        (resting - cold.share(MIN_COLD_KELVIN)).abs() < 1e-4,
        "on its pin"
    );
}
