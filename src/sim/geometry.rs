//! Shape of the engine: two matching vertical cylinders joined below their
//! floors by a U passage, and the tests for whether a particle fits inside.

use macroquad::prelude::{
    Vec3,
    vec3, //
};

use crate::sim::units::METERS_PER_UNIT;

/// Particle centers are pushed back this fraction of the grid size inside
/// a wall, so floating-point rounding cannot leave them just outside.
pub const WALL_PROJECTION_MARGIN: f32 = 16.0 * f32::EPSILON;

/// Linear scale of the default chamber geometry.
pub const DEFAULT_CHAMBER_SCALE: f32 = 1.8;
/// Depth of the passage centerline below the cylinder floors, in passage
/// radii; must be at least one so the whole bottom run sits below the floors.
pub const CONNECTOR_DROP_RADII: f32 = 1.5;
/// Passage radius per unit of chamber scale; about half the bore radius, so
/// particles can move between cylinders within one engine cycle.
const CONNECTOR_RADIUS_PER_SCALE: f32 = 0.714;

/// Dimensions of two matching cylinders and their lower connecting passage.
#[derive(Clone, Copy, Debug)]
pub struct ChamberGeometry {
    pub cylinder_offset: f32,
    pub cylinder_radius: f32,
    pub cylinder_height: f32,
    pub connector_center_y: f32,
    pub connector_radius: f32,
}

impl ChamberGeometry {
    /// The default proportions at a uniform `scale`. The fields are public
    /// so a test can change one dimension at a time.
    pub const fn scaled(scale: f32) -> Self {
        Self {
            cylinder_offset: 2.1 * scale,
            cylinder_radius: 1.4 * scale,
            cylinder_height: 3.6 * scale,
            connector_center_y: -(1.8 + CONNECTOR_DROP_RADII * CONNECTOR_RADIUS_PER_SCALE) * scale,
            connector_radius: CONNECTOR_RADIUS_PER_SCALE * scale,
        }
    }

    /// Checks that the dimensions describe a usable two-cylinder chamber.
    ///
    /// # Panics
    ///
    /// Panics if any dimension is non-finite or too small for the particle
    /// radius, if the cylinders overlap, if the passage is not narrower than
    /// the bore, or if its horizontal run is not entirely below the floors.
    pub fn validate(self, particle_radius: f32) {
        let Self {
            cylinder_offset: offset,
            cylinder_radius: radius,
            cylinder_height: height,
            connector_center_y: center_y,
            connector_radius: tube_radius,
        } = self;
        assert!(particle_radius.is_finite() && particle_radius > 0.0);
        assert!(offset.is_finite() && radius.is_finite() && height.is_finite());
        assert!(center_y.is_finite() && tube_radius.is_finite());
        assert!(radius > particle_radius && height > 2.0 * particle_radius);
        assert!(offset > radius);
        assert!(tube_radius > particle_radius && tube_radius < radius);
        assert!(center_y + tube_radius <= -height / 2.0);
        assert!(tube_radius + 2.0 * particle_radius < height);
    }

    pub fn bottom(self) -> f32 {
        self.connector_center_y - self.connector_radius
    }

    /// Side of the cubic collision grid, centered on the origin; large enough
    /// to contain both cylinders and the entire lower passage.
    pub fn grid_side(self) -> f32 {
        (2.0 * (self.cylinder_offset + self.cylinder_radius))
            .max(self.cylinder_height)
            .max(-2.0 * self.bottom())
    }

    /// Centerlines of the three capsules forming the U: a leg down from each
    /// cylinder floor and the bottom run between them. Capsules overlap at
    /// the corners, which gives rounded elbows.
    pub fn connector_segments(self) -> [(Vec3, Vec3); 3] {
        let floor = -self.cylinder_height / 2.0;
        let left = vec3(-self.cylinder_offset, self.connector_center_y, 0.0);
        let right = vec3(self.cylinder_offset, self.connector_center_y, 0.0);
        [
            (vec3(left.x, floor, 0.0), left),
            (left, right),
            (right, vec3(right.x, floor, 0.0)),
        ]
    }

    /// Closest point on any passage centerline; shared by containment and
    /// wall projection.
    fn connector_axis_point(self, position: Vec3) -> Vec3 {
        let mut closest = Vec3::ZERO;
        let mut distance = f32::INFINITY;
        for (start, end) in self.connector_segments() {
            let axis = end - start;
            let point = start
                + axis * ((position - start).dot(axis) / axis.length_squared()).clamp(0.0, 1.0);
            let squared = position.distance_squared(point);
            if squared < distance {
                closest = point;
                distance = squared;
            }
        }
        closest
    }

    /// Exact volume of the union of the three capsules below the cylinder
    /// floors, with the elbow overlaps counted once. The capsule ends that
    /// poke above the floors lie inside the cylinders and are excluded.
    /// Assumes each leg is at least one passage radius long, which
    /// `validate` enforces.
    pub fn connector_volume_m3(self) -> f64 {
        let radius = self.connector_radius as f64;
        let leg = (-self.cylinder_height / 2.0 - self.connector_center_y) as f64;
        let pi = std::f64::consts::PI;
        (pi * radius.powi(2) * (2.0 * self.cylinder_offset as f64 + 2.0 * leg)
            + (2.0 * pi - 8.0) / 3.0 * radius.powi(3))
            * METERS_PER_UNIT.powi(3)
    }

    /// Horizontal position of the hot cylinder's axis (the left one).
    pub fn hot_center_x(self) -> f32 {
        -self.cylinder_offset
    }

    /// Horizontal position of the cold cylinder's axis (the right one).
    pub fn cold_center_x(self) -> f32 {
        self.cylinder_offset
    }

    /// Whether a sphere of `radius` centered at `position` fits inside the
    /// cylinder whose axis is at `center_x`, between its floor and the
    /// piston face at `top`.
    pub fn within_cylinder(self, position: Vec3, center_x: f32, radius: f32, top: f32) -> bool {
        let dx = position.x - center_x;
        let radial_limit = self.cylinder_radius - radius;
        dx * dx + position.z * position.z <= radial_limit * radial_limit
            && (-self.cylinder_height / 2.0 + radius..=top - radius).contains(&position.y)
    }

    /// Whether a sphere of `radius` centered at `position` fits inside the
    /// passage.
    pub fn within_connector(self, position: Vec3, radius: f32) -> bool {
        let limit = self.connector_radius - radius;
        position.distance_squared(self.connector_axis_point(position)) <= limit * limit
    }

    /// Whether a sphere fits anywhere in the connected gas volume, with the
    /// piston faces at `hot_top` and `cold_top`.
    pub fn contains(self, position: Vec3, radius: f32, hot_top: f32, cold_top: f32) -> bool {
        self.within_cylinder(position, self.hot_center_x(), radius, hot_top)
            || self.within_cylinder(position, self.cold_center_x(), radius, cold_top)
            || self.within_connector(position, radius)
    }

    /// Closest position where a sphere fits inside one cylinder, kept a
    /// rounding margin inside the walls.
    pub fn project_to_cylinder(self, position: Vec3, center_x: f32, radius: f32, top: f32) -> Vec3 {
        let dx = position.x - center_x;
        let dz = position.z;
        let radial_distance = (dx * dx + dz * dz).sqrt();
        // Rounding when adding `center_x` can otherwise put the returned
        // center a few ULPs beyond the exact radial containment boundary.
        let inset =
            (self.grid_side() * WALL_PROJECTION_MARGIN).min((self.cylinder_radius - radius) * 0.5);
        let radial_limit = self.cylinder_radius - radius - inset;
        let scale = if radial_distance > radial_limit {
            radial_limit / radial_distance
        } else {
            1.0
        };

        vec3(
            center_x + dx * scale,
            position.y.clamp(
                -self.cylinder_height / 2.0 + radius + inset,
                top - radius - inset,
            ),
            dz * scale,
        )
    }

    /// Closest position where a sphere fits inside the passage, measured to
    /// the nearest centerline and kept a rounding margin inside the wall.
    pub fn project_to_connector(self, position: Vec3, radius: f32) -> Vec3 {
        let center = self.connector_axis_point(position);
        let delta = position - center;
        let inset =
            (self.grid_side() * WALL_PROJECTION_MARGIN).min((self.connector_radius - radius) * 0.5);
        let limit = self.connector_radius - radius - inset;
        center + delta * (limit / delta.length().max(limit)).min(1.0)
    }
}

/// Default engine dimensions.
pub const DEFAULT_GEOMETRY: ChamberGeometry = ChamberGeometry::scaled(DEFAULT_CHAMBER_SCALE);

impl Default for ChamberGeometry {
    fn default() -> Self {
        DEFAULT_GEOMETRY
    }
}

#[cfg(test)]
#[path = "../../tests/unit/sim/geometry.rs"]
mod tests;
