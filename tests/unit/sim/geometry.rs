//! Shape tests for `src/sim/geometry.rs`: the opening between the bores,
//! containment and projection back inside the walls.

use super::*;
use crate::sim::engine::PARTICLE_RADIUS;

const POSITION_TOLERANCE: f32 = 1e-5;

#[test]
fn opening_joins_both_bores_through_the_shared_wall() {
    let geometry = DEFAULT_GEOMETRY;
    let top = geometry.cylinder_height / 2.0;
    let center_y = geometry.passage_center_y();
    // The middle of the wall, on the opening's axis: passage, not cylinder.
    let middle = vec3(0.0, center_y, 0.0);
    assert!(geometry.within_passage(middle, PARTICLE_RADIUS));
    for x in [geometry.hot_center_x(), geometry.cold_center_x()] {
        assert!(!geometry.within_cylinder(middle, x, PARTICLE_RADIUS, top));
    }
    // Each end of the opening reaches into its bore.
    for side in [-1.0, 1.0] {
        let mouth = vec3(side * geometry.cylinder_offset / 2.0, center_y, 0.0);
        assert!(geometry.within_passage(mouth, PARTICLE_RADIUS));
        assert!(geometry.contains(mouth, PARTICLE_RADIUS, top, top));
    }
    // The wall above the opening and the floor below it are solid.
    let above = vec3(0.0, geometry.passage_top() + PARTICLE_RADIUS, 0.0);
    assert!(!geometry.contains(above, PARTICLE_RADIUS, top, top));
    let below = vec3(0.0, geometry.floor(), 0.0);
    assert!(!geometry.contains(below, PARTICLE_RADIUS, top, top));
    // The whole axis stays inside the collision grid.
    let (start, end) = geometry.passage_axis();
    for point in [start, end] {
        assert!(
            point.abs().max_element() + geometry.passage_radius
                <= geometry.grid_side() / 2.0 + POSITION_TOLERANCE
        );
    }
}

#[test]
fn opening_is_flush_with_the_floors_and_below_the_tops() {
    let geometry = DEFAULT_GEOMETRY;
    assert!(
        (geometry.passage_center_y() - geometry.passage_radius - geometry.floor()).abs() < 1e-6
    );
    assert!(geometry.passage_top() < geometry.cylinder_height / 2.0);
    assert!(geometry.shared_wall_thickness() > 2.0 * PARTICLE_RADIUS);
    // The opening's lowest particles rest on the floor's level.
    let lowest = vec3(0.0, geometry.floor() + PARTICLE_RADIUS, 0.0);
    assert!(geometry.within_passage(lowest, PARTICLE_RADIUS));
}

#[test]
fn passage_projection_lands_inside_from_every_side() {
    let geometry = DEFAULT_GEOMETRY;
    let (start, end) = geometry.passage_axis();
    for center in [start, (start + end) / 2.0, end] {
        for normal in [Vec3::X, Vec3::Y, Vec3::Z, -Vec3::X, -Vec3::Y, -Vec3::Z] {
            let outside = center + normal * geometry.passage_radius * 2.0;
            let projected = geometry.project_to_passage(outside, PARTICLE_RADIUS);
            assert!(geometry.within_passage(projected, PARTICLE_RADIUS));
        }
    }
}

/// A radial clamp at the exact wall can round back *outside* after the
/// cylinder offset is added in f32, so test a full set of wall directions.
#[test]
fn projected_spheres_remain_inside_cylinders_at_all_angles() {
    let geometry = ChamberGeometry::default();
    let center_x = -geometry.cylinder_offset;
    let face_y = geometry.cylinder_height / 2.0;
    for angle in 0..360 {
        let radians = (angle as f32).to_radians();
        let position = vec3(
            center_x + geometry.cylinder_radius * 1.5 * radians.cos(),
            0.0,
            geometry.cylinder_radius * 1.5 * radians.sin(),
        );
        let projected = geometry.project_to_cylinder(position, center_x, PARTICLE_RADIUS, face_y);
        assert!(
            geometry.within_cylinder(projected, center_x, PARTICLE_RADIUS, face_y,),
            "projection at angle {angle} escaped: {projected:?}",
        );
    }
}

#[test]
fn round_opening_has_no_square_corners() {
    let geometry = DEFAULT_GEOMETRY;
    let usable_radius = geometry.passage_radius - PARTICLE_RADIUS;
    let center = vec3(0.0, geometry.passage_center_y(), 0.0);
    assert!(geometry.within_passage(center, PARTICLE_RADIUS));
    let corner = center + vec3(0.0, usable_radius * 0.8, usable_radius * 0.8);
    assert!(!geometry.within_passage(corner, PARTICLE_RADIUS));
}

#[test]
fn passage_projection_stays_inside_for_all_wall_angles() {
    let geometry = DEFAULT_GEOMETRY;
    let usable_radius = geometry.passage_radius - PARTICLE_RADIUS;
    for angle in 0..360 {
        let theta = (angle as f32).to_radians();
        let outside = vec3(
            0.0,
            geometry.passage_center_y() + (usable_radius + 0.3) * theta.cos(),
            (usable_radius + 0.3) * theta.sin(),
        );
        let inside = geometry.project_to_passage(outside, PARTICLE_RADIUS);
        assert!(
            geometry.within_passage(inside, PARTICLE_RADIUS),
            "round opening projection failed at {angle} degrees: {inside:?}"
        );
    }
}

/// An independent midpoint sum over slices across the opening: each slice at
/// depth z is 2 sqrt(r^2 - z^2) tall and runs between the two bore walls,
/// 2 (offset - sqrt(R^2 - z^2)) long.
#[test]
fn passage_volume_counts_only_the_opening_between_the_bores() {
    const SLICES: usize = 200_000;
    let geometry = DEFAULT_GEOMETRY;
    let r = geometry.passage_radius as f64;
    let bore = geometry.cylinder_radius as f64;
    let offset = geometry.cylinder_offset as f64;
    let dz = 2.0 * r / SLICES as f64;
    let volume: f64 = (0..SLICES)
        .map(|slice| {
            let z = -r + (slice as f64 + 0.5) * dz;
            2.0 * (r * r - z * z).sqrt() * 2.0 * (offset - (bore * bore - z * z).sqrt()) * dz
        })
        .sum();
    let expected = volume * METERS_PER_UNIT.powi(3);
    assert!((geometry.passage_volume_m3() / expected - 1.0).abs() < 1e-5);
    // Far smaller than a cylinder: the opening is short.
    let cylinder = std::f64::consts::PI * bore * bore * geometry.cylinder_height as f64;
    assert!(volume < 0.05 * cylinder);
}

#[test]
#[should_panic(expected = "the bores need a wall between them")]
fn touching_bores_are_rejected() {
    let mut geometry = DEFAULT_GEOMETRY;
    geometry.cylinder_offset = geometry.cylinder_radius;
    geometry.validate(PARTICLE_RADIUS);
}
