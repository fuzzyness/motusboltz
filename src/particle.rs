//! Free-flight motion and elastic collisions against cylindrical boundaries.

use std::ops::AddAssign;

use macroquad::prelude::Vec3;

/// Common particle mass in reduced simulation units.
pub const PARTICLE_MASS: f32 = 1.0;

/// Magnitudes of momentum transferred to the cylinder's boundaries.
///
/// The side wall and fixed bottom cap are stationary; the top is a moving
/// piston. All three impulse magnitudes are recorded separately. The components
/// are positive scalars, not signed vectors, and use reduced simulation units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WallImpulses {
    pub side: f32,
    pub top: f32,
    pub bottom: f32,
}

impl WallImpulses {
    /// Returns the total scalar impulse delivered to all three surfaces.
    pub fn total(self) -> f32 {
        self.side + self.top + self.bottom
    }
}

impl AddAssign for WallImpulses {
    fn add_assign(&mut self, other: Self) {
        self.side += other.side;
        self.top += other.top;
        self.bottom += other.bottom;
    }
}

/// A hard sphere represented by its center position and velocity.
pub struct Particle {
    pub position: Vec3,
    pub velocity: Vec3,
}

/// Collision limits for particle centers, including a moving top cap.
#[derive(Clone, Copy)]
pub struct WallBounds {
    pub radial: f32,
    pub bottom: f32,
    pub top: f32,
    pub top_velocity: f32,
}

impl Particle {
    /// Advances a particle, reflecting against the walls and moving piston.
    ///
    /// Wall positions already account for the hard-sphere radius. An impact
    /// with the moving piston changes the particle's laboratory-frame energy.
    pub fn update(&mut self, dt: f32, bounds: WallBounds) -> WallImpulses {
        self.update_with_top_inverse_mass(dt, bounds, 0.0)
    }

    /// Accounts for the flywheel's effective mass during piston collisions.
    /// An inverse mass of zero recovers the prescribed-motion piston.
    pub fn update_with_top_inverse_mass(
        &mut self,
        dt: f32,
        bounds: WallBounds,
        top_inverse_mass: f32,
    ) -> WallImpulses {
        self.position += self.velocity * dt;

        let mut impulses = bounce_caps(
            &mut self.position.y,
            &mut self.velocity.y,
            bounds.bottom,
            bounds.top,
            bounds.top_velocity,
            top_inverse_mass,
        );
        impulses.side = self.bounce_side(bounds.radial);
        impulses
    }

    /// Clamps overlap corrections without treating them as new wall impacts.
    pub fn constrain_position(&mut self, bounds: WallBounds) {
        self.position.y = self.position.y.clamp(bounds.bottom, bounds.top);

        let radial_distance_squared = self.radial_distance_squared();
        if radial_distance_squared > bounds.radial * bounds.radial {
            let scale = bounds.radial / radial_distance_squared.sqrt();
            self.position.x *= scale;
            self.position.z *= scale;
        }
    }

    fn radial_distance_squared(&self) -> f32 {
        self.position.x * self.position.x + self.position.z * self.position.z
    }

    /// Reflects the outward component of velocity against the curved wall.
    fn bounce_side(&mut self, radial_limit: f32) -> f32 {
        let radial_distance_squared = self.radial_distance_squared();
        if radial_distance_squared <= radial_limit * radial_limit {
            return 0.0;
        }

        let radial_distance = radial_distance_squared.sqrt();
        let normal_x = self.position.x / radial_distance;
        let normal_z = self.position.z / radial_distance;

        self.position.x = normal_x * radial_limit;
        self.position.z = normal_z * radial_limit;

        let outward_speed = self.velocity.x * normal_x + self.velocity.z * normal_z;
        if outward_speed <= 0.0 {
            return 0.0;
        }

        // A frictionless wall leaves tangential and vertical motion unchanged.
        self.velocity.x -= 2.0 * outward_speed * normal_x;
        self.velocity.z -= 2.0 * outward_speed * normal_z;
        2.0 * PARTICLE_MASS * outward_speed
    }
}

/// The top cap uses relative velocity; the bottom cap is stationary.
fn bounce_caps(
    position: &mut f32,
    velocity: &mut f32,
    bottom: f32,
    top: f32,
    top_velocity: f32,
    top_inverse_mass: f32,
) -> WallImpulses {
    if *position > top {
        *position = top;
        let relative_speed = *velocity - top_velocity;
        if relative_speed > 0.0 {
            // Elastic impact of a particle on a constrained rotating flywheel.
            // The wall's effective inverse mass is (dy/dtheta)^2 / inertia.
            let impulse = 2.0 * relative_speed / (1.0 / PARTICLE_MASS + top_inverse_mass);
            *velocity -= impulse / PARTICLE_MASS;
            return WallImpulses {
                top: impulse,
                ..Default::default()
            };
        }
    } else if *position < bottom {
        *position = bottom;
        if *velocity < 0.0 {
            let impulse = -2.0 * PARTICLE_MASS * *velocity;
            *velocity = -*velocity;
            return WallImpulses {
                bottom: impulse,
                ..Default::default()
            };
        }
    }

    WallImpulses::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::prelude::vec3;

    const RADIAL_LIMIT: f32 = 1.82;
    const CAP_LIMIT: f32 = 1.82;
    const STATIONARY_BOUNDS: WallBounds = WallBounds {
        radial: RADIAL_LIMIT,
        bottom: -CAP_LIMIT,
        top: CAP_LIMIT,
        top_velocity: 0.0,
    };
    const DISTANCE_FROM_WALL: f32 = 0.12;
    const INITIAL_SPEED: f32 = 1.5;
    const TIME_STEP: f32 = 0.2;
    const TOLERANCE: f32 = 1e-6;
    const DIAGONAL_COMPONENT: f32 = std::f32::consts::FRAC_1_SQRT_2;

    #[test]
    fn side_collision_reflects_radial_velocity_and_reports_impulse() {
        let mut particle = Particle {
            position: vec3(RADIAL_LIMIT - DISTANCE_FROM_WALL, 0.0, 0.0),
            velocity: vec3(INITIAL_SPEED, 0.0, 0.0),
        };

        let impulse = particle.update(TIME_STEP, STATIONARY_BOUNDS);
        let expected_impulse = 2.0 * PARTICLE_MASS * INITIAL_SPEED;

        assert!((particle.position.x - RADIAL_LIMIT).abs() < TOLERANCE);
        assert!((particle.velocity.x + INITIAL_SPEED).abs() < TOLERANCE);
        assert!((impulse.side - expected_impulse).abs() < TOLERANCE);
        assert_eq!(impulse.top, 0.0);
        assert_eq!(impulse.bottom, 0.0);
    }

    #[test]
    fn angled_side_collision_preserves_tangential_and_vertical_motion() {
        // At 45 degrees the radial direction is (x + z) / sqrt(2).
        let radial = vec3(DIAGONAL_COMPONENT, 0.0, DIAGONAL_COMPONENT);
        let tangent = vec3(-DIAGONAL_COMPONENT, 0.0, DIAGONAL_COMPONENT);
        let vertical_speed = INITIAL_SPEED / 2.0;
        let mut particle = Particle {
            position: radial * (RADIAL_LIMIT + DISTANCE_FROM_WALL),
            velocity: radial * INITIAL_SPEED + tangent + Vec3::Y * vertical_speed,
        };
        let energy_before = particle.velocity.length_squared();

        // Start outside the wall to isolate the radial reflection from
        // tangential displacement during free flight.
        let impulse = particle.update(0.0, STATIONARY_BOUNDS);

        assert!(
            (particle.position.x * particle.position.x + particle.position.z * particle.position.z
                - RADIAL_LIMIT * RADIAL_LIMIT)
                .abs()
                < TOLERANCE
        );
        assert!((particle.velocity.dot(radial) + INITIAL_SPEED).abs() < TOLERANCE);
        assert!((particle.velocity.dot(tangent) - 1.0).abs() < TOLERANCE);
        assert!((particle.velocity.y - vertical_speed).abs() < TOLERANCE);
        assert!((particle.velocity.length_squared() - energy_before).abs() < TOLERANCE);
        assert!((impulse.side - 2.0 * PARTICLE_MASS * INITIAL_SPEED).abs() < TOLERANCE);
        assert_eq!(impulse.top, 0.0);
        assert_eq!(impulse.bottom, 0.0);
    }

    #[test]
    fn cap_collision_preserves_speed_and_reports_impulse() {
        let mut particle = Particle {
            position: vec3(0.0, CAP_LIMIT - DISTANCE_FROM_WALL, 0.0),
            velocity: vec3(0.0, INITIAL_SPEED, 0.0),
        };

        let impulse = particle.update(TIME_STEP, STATIONARY_BOUNDS);

        assert!((particle.position.y - CAP_LIMIT).abs() < TOLERANCE);
        assert!((particle.velocity.y + INITIAL_SPEED).abs() < TOLERANCE);
        assert!((impulse.top - 2.0 * PARTICLE_MASS * INITIAL_SPEED).abs() < TOLERANCE);
        assert_eq!(impulse.side, 0.0);
        assert_eq!(impulse.bottom, 0.0);
    }

    #[test]
    fn bottom_cap_collision_reports_separate_impulse() {
        let mut particle = Particle {
            position: vec3(0.0, -CAP_LIMIT + DISTANCE_FROM_WALL, 0.0),
            velocity: vec3(0.0, -INITIAL_SPEED, 0.0),
        };

        let impulse = particle.update(TIME_STEP, STATIONARY_BOUNDS);

        assert!((particle.position.y + CAP_LIMIT).abs() < TOLERANCE);
        assert!((particle.velocity.y - INITIAL_SPEED).abs() < TOLERANCE);
        assert!((impulse.bottom - 2.0 * PARTICLE_MASS * INITIAL_SPEED).abs() < TOLERANCE);
        assert_eq!(impulse.side, 0.0);
        assert_eq!(impulse.top, 0.0);
    }

    #[test]
    fn simultaneous_side_and_top_impacts_are_recorded() {
        let mut particle = Particle {
            position: vec3(
                RADIAL_LIMIT - DISTANCE_FROM_WALL,
                CAP_LIMIT - DISTANCE_FROM_WALL,
                0.0,
            ),
            velocity: vec3(INITIAL_SPEED, INITIAL_SPEED, 0.0),
        };

        let impulse = particle.update(TIME_STEP, STATIONARY_BOUNDS);
        let expected = 2.0 * PARTICLE_MASS * INITIAL_SPEED;

        assert!((impulse.side - expected).abs() < TOLERANCE);
        assert!((impulse.top - expected).abs() < TOLERANCE);
        assert_eq!(impulse.bottom, 0.0);
        assert!((impulse.total() - 2.0 * expected).abs() < TOLERANCE);
    }

    #[test]
    fn no_wall_collision_produces_no_impulse() {
        let mut particle = Particle {
            position: Vec3::ZERO,
            velocity: vec3(INITIAL_SPEED, 0.0, 0.0),
        };

        assert_eq!(
            particle.update(TIME_STEP, STATIONARY_BOUNDS),
            WallImpulses::default()
        );
    }

    #[test]
    fn post_collision_constraint_preserves_velocity() {
        let original_velocity = vec3(INITIAL_SPEED, -INITIAL_SPEED, INITIAL_SPEED);
        let mut particle = Particle {
            position: vec3(
                RADIAL_LIMIT + DISTANCE_FROM_WALL,
                CAP_LIMIT + DISTANCE_FROM_WALL,
                RADIAL_LIMIT + DISTANCE_FROM_WALL,
            ),
            velocity: original_velocity,
        };

        particle.constrain_position(STATIONARY_BOUNDS);

        let radial_distance = (particle.position.x * particle.position.x
            + particle.position.z * particle.position.z)
            .sqrt();

        assert!(radial_distance <= RADIAL_LIMIT + TOLERANCE);
        assert!((particle.position.y - CAP_LIMIT).abs() < TOLERANCE);
        assert_eq!(particle.velocity, original_velocity);
    }

    #[test]
    fn inward_motion_outside_boundary_does_not_create_impulse() {
        let mut particle = Particle {
            position: vec3(RADIAL_LIMIT + DISTANCE_FROM_WALL, 0.0, 0.0),
            velocity: vec3(-INITIAL_SPEED, 0.0, 0.0),
        };

        assert_eq!(
            particle.update(0.0, STATIONARY_BOUNDS),
            WallImpulses::default()
        );
        assert_eq!(particle.position.x, RADIAL_LIMIT);
        assert_eq!(particle.velocity.x, -INITIAL_SPEED);
    }

    #[test]
    fn descending_piston_heats_a_particle_and_records_impulse() {
        const PISTON_VELOCITY: f32 = -0.5;
        let mut particle = Particle {
            position: vec3(0.0, CAP_LIMIT + DISTANCE_FROM_WALL, 0.0),
            velocity: Vec3::ZERO,
        };
        let bounds = WallBounds {
            top_velocity: PISTON_VELOCITY,
            ..STATIONARY_BOUNDS
        };
        let impulse = particle.update(0.0, bounds);

        assert!((particle.velocity.y - 2.0 * PISTON_VELOCITY).abs() < TOLERANCE);
        assert!((impulse.top - -2.0 * PARTICLE_MASS * PISTON_VELOCITY).abs() < TOLERANCE);
        assert_eq!(particle.position.y, CAP_LIMIT);
        let kinetic_gain = PARTICLE_MASS * particle.velocity.length_squared() / 2.0;
        let work_by_gas = impulse.top * PISTON_VELOCITY;
        assert!((kinetic_gain + work_by_gas).abs() < TOLERANCE);
    }

    #[test]
    fn rising_piston_cools_an_overtaking_particle() {
        const PISTON_VELOCITY: f32 = 0.5;
        let mut particle = Particle {
            position: vec3(0.0, CAP_LIMIT + DISTANCE_FROM_WALL, 0.0),
            velocity: vec3(0.0, INITIAL_SPEED, 0.0),
        };
        let bounds = WallBounds {
            top_velocity: PISTON_VELOCITY,
            ..STATIONARY_BOUNDS
        };
        let original_energy = particle.velocity.length_squared();
        let impulse = particle.update(0.0, bounds);

        assert!(particle.velocity.length_squared() < original_energy);
        assert!(impulse.top > 0.0);
    }
}
