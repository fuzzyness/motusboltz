//! Tests for `src/graphics/heat_sparks.rs`: sparks show the net heat flow.

use super::*;
use crate::sim::geometry::DEFAULT_GEOMETRY;
use macroquad::prelude::Vec3;

/// A camera on the front side, looking at the engine.
fn camera() -> Camera3D {
    Camera3D {
        position: Vec3::new(8.0, 6.0, 20.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        ..Default::default()
    }
}

/// Sparks shown on the hot and the cold side of a run at `hot` and `cold`
/// kelvin, after the balance has settled, as (heat in, heat out) counts.
fn shown(hot: f32, cold: f32) -> [(usize, usize); 2] {
    const SEED: u64 = 4242;
    const FRAMES: usize = 600;
    const FRAME: f64 = 1.0 / 60.0;
    let mut simulation = ConnectedChambers::with_geometry_seed(1_024, DEFAULT_GEOMETRY, SEED);
    simulation.set_hot_kelvin(hot);
    simulation.set_cold_kelvin(cold);
    simulation.start_engine();
    let mut sparks = HeatSparks::new(DEFAULT_GEOMETRY);
    for frame in 0..FRAMES {
        simulation.update(FRAME as f32);
        sparks.build(&simulation, &camera(), frame as f64 * FRAME);
    }
    let mut counts = [(0, 0); 2];
    for (center, color) in sparks.sprites.centers() {
        let side = &mut counts[usize::from(center.x > 0.0)];
        if color.r > color.b {
            side.0 += 1;
        } else {
            side.1 += 1;
        }
    }
    counts
}

#[test]
fn heat_flows_in_at_the_burner_and_out_at_the_ice() {
    let [hot, cold] = shown(1_200.0, 100.0);
    assert!(hot.0 > 10 && hot.1 == 0, "hot side {hot:?}");
    assert!(cold.1 > 10 && cold.0 == 0, "cold side {cold:?}");
}

#[test]
fn equal_temperatures_show_few_sparks() {
    let [hot, cold] = shown(300.0, 300.0);
    let busy = shown(1_200.0, 100.0);
    let total =
        |sides: [(usize, usize); 2]| sides.iter().map(|side| side.0 + side.1).sum::<usize>();
    assert!(
        total([hot, cold]) * 4 < total(busy),
        "{hot:?} {cold:?} vs {busy:?}"
    );
}
