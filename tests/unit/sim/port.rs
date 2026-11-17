//! Tests for `src/sim/port.rs`: where the port is, and how the gas closes
//! up after particles leave.

use super::*;
use crate::sim::engine::PARTICLE_RADIUS;
use crate::sim::geometry::DEFAULT_GEOMETRY;

/// The port opens from the cold cylinder's outer side, low down, and a new
/// particle starts inside the bore, clear of the wall.
#[test]
fn the_port_is_low_on_the_cold_cylinders_outer_side() {
    let geometry = DEFAULT_GEOMETRY;
    let port = Port::new(geometry);
    assert!(port.outward.x > 0.0, "out the cold end");
    assert!((port.outward.length() - 1.0).abs() < 1e-5);
    assert_eq!(port.outward.y, 0.0);
    let floor = geometry.floor();
    assert!(port.height > floor + port.radius);
    assert!(port.height < floor + geometry.cylinder_height / 4.0);
    let mouth = port.mouth(PARTICLE_RADIUS);
    let top = geometry.cylinder_height / 2.0;
    assert!(geometry.within_cylinder(mouth, geometry.cold_center_x(), PARTICLE_RADIUS, top));
    let (side, up) = port.across();
    assert!(side.dot(port.outward).abs() < 1e-5 && up.dot(side).abs() < 1e-5);
}

/// Those that stay close up in order; those that left have no place.
#[test]
fn the_rest_close_up_in_order() {
    let moved = closed_up(5, &[1, 3]);
    assert_eq!(moved, [Some(0), None, Some(1), None, Some(2)]);
}
