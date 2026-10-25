//! Shape tests for `src/sim/geometry.rs`: the U passage, containment and
//! projection back inside the walls.

use super::*;
use crate::sim::engine::PARTICLE_RADIUS;

const POSITION_TOLERANCE: f32 = 1e-5;
const CYLINDER_HEIGHT: f32 = DEFAULT_GEOMETRY.cylinder_height;
const CONNECTOR_CENTER_Y: f32 = DEFAULT_GEOMETRY.connector_center_y;
const CONNECTOR_RADIUS: f32 = DEFAULT_GEOMETRY.connector_radius;

#[test]
fn u_passage_connects_floor_openings_and_stays_within_grid() {
    let geometry = DEFAULT_GEOMETRY;
    let floor = -geometry.cylinder_height / 2.0;
    for x in [-geometry.cylinder_offset, geometry.cylinder_offset] {
        let junction = vec3(x, floor + PARTICLE_RADIUS, 0.0);
        assert!(geometry.within_connector(junction, PARTICLE_RADIUS));
        assert!(geometry.within_cylinder(
            junction,
            x,
            PARTICLE_RADIUS,
            geometry.cylinder_height / 2.0
        ));
    }
    const PATH_STEPS: usize = 100;
    for (a, b) in geometry.connector_segments() {
        for step in 0..=PATH_STEPS {
            let point = a.lerp(b, step as f32 / PATH_STEPS as f32);
            assert!(geometry.within_connector(point, PARTICLE_RADIUS));
            assert!(
                point.abs().max_element() + geometry.connector_radius
                    <= geometry.grid_side() / 2.0 + POSITION_TOLERANCE
            );
        }
    }
    assert!(!geometry.within_connector(Vec3::ZERO, PARTICLE_RADIUS));
}

#[test]
fn u_passage_projection_covers_legs_elbows_and_bottom() {
    let geometry = DEFAULT_GEOMETRY;
    for (a, b) in geometry.connector_segments() {
        for center in [a, (a + b) / 2.0, b] {
            for normal in [Vec3::X, Vec3::Y, Vec3::Z, -Vec3::X, -Vec3::Y, -Vec3::Z] {
                let outside = center + normal * geometry.connector_radius * 2.0;
                let projected = geometry.project_to_connector(outside, PARTICLE_RADIUS);
                assert!(geometry.within_connector(projected, PARTICLE_RADIUS));
            }
        }
    }
}

#[test]
fn connector_sits_below_center_with_clearance_for_rounded_bottoms() {
    let bottom = -CYLINDER_HEIGHT / 2.0;
    let top = CYLINDER_HEIGHT / 2.0;
    assert!(CONNECTOR_CENTER_Y + CONNECTOR_RADIUS < bottom);
    const {
        assert!(CONNECTOR_CENTER_Y + CONNECTOR_RADIUS < 0.0);
    }
    assert!(CONNECTOR_CENTER_Y + CONNECTOR_RADIUS < top);
    assert!(bottom - CONNECTOR_CENTER_Y > PARTICLE_RADIUS);
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
fn round_tube_has_no_rectangular_corner_voids() {
    let geometry = DEFAULT_GEOMETRY;
    let usable_radius = geometry.connector_radius - PARTICLE_RADIUS;
    let center = vec3(0.0, geometry.connector_center_y, 0.0);
    assert!(geometry.within_connector(center, PARTICLE_RADIUS));
    let corner = center + vec3(0.0, usable_radius * 0.8, usable_radius * 0.8);
    assert!(!geometry.within_connector(corner, PARTICLE_RADIUS));
}

#[test]
fn circular_passage_projection_stays_inside_for_all_wall_angles() {
    let geometry = DEFAULT_GEOMETRY;
    let usable_radius = geometry.connector_radius - PARTICLE_RADIUS;
    for angle in 0..360 {
        let theta = (angle as f32).to_radians();
        let outside = vec3(
            0.0,
            geometry.connector_center_y + (usable_radius + 0.3) * theta.cos(),
            (usable_radius + 0.3) * theta.sin(),
        );
        let inside = geometry.project_to_connector(outside, PARTICLE_RADIUS);
        assert!(
            geometry.within_connector(inside, PARTICLE_RADIUS),
            "round tube projection failed at {angle} degrees: {inside:?}"
        );
    }
}

#[test]
fn tube_volume_uses_circular_cross_section() {
    let geometry = DEFAULT_GEOMETRY;
    // Independent midpoint integration of the exposed union cross sections.
    const SLICES: usize = 20_000;
    let radius = geometry.connector_radius as f64;
    let offset = geometry.cylinder_offset as f64;
    let lower = (geometry.connector_center_y - geometry.connector_radius) as f64;
    let upper = -geometry.cylinder_height as f64 / 2.0;
    let dy = (upper - lower) / SLICES as f64;
    let pi = std::f64::consts::PI;
    let mut volume = 0.0;
    for slice in 0..SLICES {
        let y = lower + (slice as f64 + 0.5) * dy - geometry.connector_center_y as f64;
        let h = (radius * radius - y * y).max(0.0).sqrt();
        let area = if y < 0.0 {
            4.0 * offset * h + pi * h * h
        } else {
            let overlap = h * (radius * radius - h * h).max(0.0).sqrt()
                + radius * radius * (h / radius).asin();
            4.0 * offset * h + 2.0 * pi * radius * radius - 2.0 * overlap
        };
        volume += area * dy;
    }
    let expected = volume * METERS_PER_UNIT.powi(3);
    assert!((geometry.connector_volume_m3() / expected - 1.0).abs() < 1e-5);
}
