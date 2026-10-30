//! Shape of the engine: two matching vertical cylinders side by side in one
//! block, joined by a round opening through the wall between them, and the
//! tests for whether a particle fits inside.

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
/// Bore radius per unit of chamber scale.
const CYLINDER_RADIUS_PER_SCALE: f32 = 1.4;
/// Thickness of the wall between the two bores, per unit of chamber scale.
const SHARED_WALL_PER_SCALE: f32 = 0.28;
/// Cylinder height per unit of chamber scale: room for the full stroke
/// above the opening (see `pistons.rs`).
const CYLINDER_HEIGHT_PER_SCALE: f32 = 4.62;
/// Opening radius per unit of chamber scale. Gas sloshing between the
/// cylinders limits how fast the engine turns, so the opening is wide: with
/// 1,024 particles at 1200/100 K and almost no load, a radius of 0.9
/// settled at 16.5 s per turn against 23.7 s for 0.5 (and 26.5 s for the
/// U passage below the floors used before v0.6).
const PASSAGE_RADIUS_PER_SCALE: f32 = 0.9;
/// Number of intervals in the Simpson's rule sum for the passage volume.
const VOLUME_INTERVALS: usize = 512;

/// Dimensions of two matching cylinders and the opening between them.
#[derive(Clone, Copy, Debug)]
pub struct ChamberGeometry {
    /// Distance from the middle of the shared wall to each cylinder's axis.
    pub cylinder_offset: f32,
    pub cylinder_radius: f32,
    pub cylinder_height: f32,
    /// Radius of the straight, round opening through the shared wall. Its
    /// axis runs between the cylinder axes, one radius above the floors, so
    /// its lowest line is flush with them.
    pub passage_radius: f32,
}

impl ChamberGeometry {
    /// The default proportions at a uniform `scale`. The fields are public
    /// so a test can change one dimension at a time.
    pub const fn scaled(scale: f32) -> Self {
        Self {
            cylinder_offset: (CYLINDER_RADIUS_PER_SCALE + SHARED_WALL_PER_SCALE / 2.0) * scale,
            cylinder_radius: CYLINDER_RADIUS_PER_SCALE * scale,
            cylinder_height: CYLINDER_HEIGHT_PER_SCALE * scale,
            passage_radius: PASSAGE_RADIUS_PER_SCALE * scale,
        }
    }

    /// Checks that the dimensions describe a usable two-cylinder chamber.
    ///
    /// # Panics
    ///
    /// Panics if any dimension is non-finite or too small for the particle
    /// radius, if the bores touch or overlap, if the opening is not
    /// narrower than the bore, or if the opening leaves no room above it.
    pub fn validate(self, particle_radius: f32) {
        let Self {
            cylinder_offset: offset,
            cylinder_radius: radius,
            cylinder_height: height,
            passage_radius,
        } = self;
        assert!(particle_radius.is_finite() && particle_radius > 0.0);
        assert!(offset.is_finite() && radius.is_finite() && height.is_finite());
        assert!(passage_radius.is_finite());
        assert!(radius > particle_radius && height > 2.0 * particle_radius);
        assert!(
            self.shared_wall_thickness() > 0.0,
            "the bores need a wall between them"
        );
        assert!(passage_radius > particle_radius && passage_radius < radius);
        assert!(2.0 * (passage_radius + particle_radius) < height);
    }

    /// Height of both cylinder floors, the lowest place gas can be.
    pub fn floor(self) -> f32 {
        -self.cylinder_height / 2.0
    }

    /// Thickness of the wall between the bores.
    pub fn shared_wall_thickness(self) -> f32 {
        2.0 * (self.cylinder_offset - self.cylinder_radius)
    }

    /// Height of the opening's axis: one radius above the floors.
    pub fn passage_center_y(self) -> f32 {
        self.floor() + self.passage_radius
    }

    /// Height of the opening's highest line; the pistons stay above it.
    pub fn passage_top(self) -> f32 {
        self.floor() + 2.0 * self.passage_radius
    }

    /// Side of the cubic collision grid, centered on the origin; large enough
    /// to contain both cylinders.
    pub fn grid_side(self) -> f32 {
        (2.0 * (self.cylinder_offset + self.cylinder_radius)).max(self.cylinder_height)
    }

    /// Corners of the smallest box holding every place a particle center
    /// can be: both cylinders from their floors up to their tops.
    pub fn gas_bounds(self) -> (Vec3, Vec3) {
        let half_width = self.cylinder_offset + self.cylinder_radius;
        (
            vec3(-half_width, self.floor(), -self.cylinder_radius),
            vec3(half_width, self.cylinder_height / 2.0, self.cylinder_radius),
        )
    }

    /// The opening's axis, from the hot cylinder's axis to the cold one's.
    /// The parts of the round tube around it that lie inside the bores add
    /// nothing; the part between them is the passage.
    pub fn passage_axis(self) -> (Vec3, Vec3) {
        let y = self.passage_center_y();
        (
            vec3(-self.cylinder_offset, y, 0.0),
            vec3(self.cylinder_offset, y, 0.0),
        )
    }

    /// Volume of the opening outside both bores: for each point of its
    /// round cross section, the length between the two bore walls, summed
    /// with Simpson's rule. With z = r sin(t), the cross section's width
    /// 2 sqrt(r^2 - z^2) and the bore walls' distance from the axes,
    /// sqrt(R^2 - z^2), give a smooth integrand in t.
    pub fn passage_volume_m3(self) -> f64 {
        let r = self.passage_radius as f64;
        let bore = self.cylinder_radius as f64;
        let offset = self.cylinder_offset as f64;
        let half_pi = std::f64::consts::FRAC_PI_2;
        let step = std::f64::consts::PI / VOLUME_INTERVALS as f64;
        // Integral of sqrt(r^2 - z^2) sqrt(R^2 - z^2) dz over [-r, r].
        let mut overlap = 0.0;
        for index in 0..=VOLUME_INTERVALS {
            let t = -half_pi + index as f64 * step;
            let weight = if index == 0 || index == VOLUME_INTERVALS {
                1.0
            } else if index % 2 == 1 {
                4.0
            } else {
                2.0
            };
            let (sin, cos) = t.sin_cos();
            overlap += weight * r * r * cos * cos * (bore * bore - r * r * sin * sin).sqrt();
        }
        overlap *= step / 3.0;
        let volume = 2.0 * std::f64::consts::PI * r * r * offset - 4.0 * overlap;
        volume * METERS_PER_UNIT.powi(3)
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
    /// round opening between the cylinders (or the parts of the same tube
    /// inside the bores).
    pub fn within_passage(self, position: Vec3, radius: f32) -> bool {
        let limit = self.passage_radius - radius;
        position.distance_squared(segment_point(self.passage_axis(), position)) <= limit * limit
    }

    /// Whether a sphere fits anywhere in the connected gas volume, with the
    /// piston faces at `hot_top` and `cold_top`.
    pub fn contains(self, position: Vec3, radius: f32, hot_top: f32, cold_top: f32) -> bool {
        self.within_cylinder(position, self.hot_center_x(), radius, hot_top)
            || self.within_cylinder(position, self.cold_center_x(), radius, cold_top)
            || self.within_passage(position, radius)
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

    /// Closest position where a sphere fits inside the opening's tube,
    /// measured to its axis and kept a rounding margin inside the wall.
    pub fn project_to_passage(self, position: Vec3, radius: f32) -> Vec3 {
        let center = segment_point(self.passage_axis(), position);
        let delta = position - center;
        let inset =
            (self.grid_side() * WALL_PROJECTION_MARGIN).min((self.passage_radius - radius) * 0.5);
        let limit = self.passage_radius - radius - inset;
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

/// Closest point to `position` on the segment from `start` to `end`.
fn segment_point((start, end): (Vec3, Vec3), position: Vec3) -> Vec3 {
    let axis = end - start;
    start + axis * ((position - start).dot(axis) / axis.length_squared()).clamp(0.0, 1.0)
}

#[cfg(test)]
#[path = "../../tests/unit/sim/geometry.rs"]
mod tests;
