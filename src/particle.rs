//! Free-flight motion and elastic collisions with a stationary cylinder.

use macroquad::prelude::Vec3;

/// Common particle mass in reduced simulation units.
pub const PARTICLE_MASS: f32 = 1.0;

/// A hard sphere represented by its center position and velocity.
pub struct Particle {
    pub position: Vec3,
    pub velocity: Vec3,
}

impl Particle {
    /// Advances a particle inside a vertical cylinder and reports wall impulse.
    ///
    /// The limits are measured from the cylinder axis and center plane to
    /// the *particle center*. Both already account for the sphere radius.
    /// Returns the combined impulse magnitude against the side and end caps.
    pub fn update(&mut self, dt: f32, radial_limit: f32, cap_limit: f32) -> f32 {
        self.position += self.velocity * dt;

        let cap_impulse = bounce_cap(&mut self.position.y, &mut self.velocity.y, cap_limit);
        let side_impulse = self.bounce_side(radial_limit);

        cap_impulse + side_impulse
    }

    /// Keeps the entire sphere inside the chamber after overlap correction.
    ///
    /// Particle-particle collisions can displace a center beyond a wall after
    /// the normal wall update. This only clamps position; any subsequent wall
    /// impact is handled during the next physics step.
    pub fn constrain_position(&mut self, radial_limit: f32, cap_limit: f32) {
        self.position.y = self.position.y.clamp(-cap_limit, cap_limit);

        let radial_distance_squared = self.radial_distance_squared();

        if radial_distance_squared > radial_limit * radial_limit {
            let scale = radial_limit / radial_distance_squared.sqrt();
            self.position.x *= scale;
            self.position.z *= scale;
        }
    }

    fn radial_distance_squared(&self) -> f32 {
        self.position.x * self.position.x + self.position.z * self.position.z
    }

    /// Reflects the outward radial velocity against the curved cylinder wall.
    fn bounce_side(&mut self, radial_limit: f32) -> f32 {
        let radial_distance_squared = self.radial_distance_squared();

        if radial_distance_squared <= radial_limit * radial_limit {
            return 0.0;
        }

        let radial_distance = radial_distance_squared.sqrt();
        let normal_x = self.position.x / radial_distance;
        let normal_z = self.position.z / radial_distance;

        // Keep the entire sphere inside the cylinder, not just its center.
        self.position.x = normal_x * radial_limit;
        self.position.z = normal_z * radial_limit;

        let outward_speed = self.velocity.x * normal_x + self.velocity.z * normal_z;
        if outward_speed <= 0.0 {
            // An overlap correction may have displaced an inward-moving
            // particle beyond the boundary. Clamp it without an impact.
            return 0.0;
        }

        // Reflect only the wall-normal component; tangential motion and
        // vertical velocity are unchanged by a frictionless side wall.
        self.velocity.x -= 2.0 * outward_speed * normal_x;
        self.velocity.z -= 2.0 * outward_speed * normal_z;

        2.0 * PARTICLE_MASS * outward_speed
    }
}

/// Constrains the center between the flat caps at +/- `limit`.
fn bounce_cap(position: &mut f32, velocity: &mut f32, limit: f32) -> f32 {
    if *position > limit {
        *position = limit;
        if *velocity > 0.0 {
            let impulse = 2.0 * PARTICLE_MASS * *velocity;
            *velocity = -*velocity;
            return impulse;
        }
    } else if *position < -limit {
        *position = -limit;
        if *velocity < 0.0 {
            let impulse = -2.0 * PARTICLE_MASS * *velocity;
            *velocity = -*velocity;
            return impulse;
        }
    }

    0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::prelude::vec3;

    const RADIAL_LIMIT: f32 = 1.82;
    const CAP_LIMIT: f32 = 1.82;
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

        let impulse = particle.update(TIME_STEP, RADIAL_LIMIT, CAP_LIMIT);
        let expected_impulse = 2.0 * PARTICLE_MASS * INITIAL_SPEED;

        assert!((particle.position.x - RADIAL_LIMIT).abs() < TOLERANCE);
        assert!((particle.velocity.x + INITIAL_SPEED).abs() < TOLERANCE);
        assert!((impulse - expected_impulse).abs() < TOLERANCE);
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
        let impulse = particle.update(0.0, RADIAL_LIMIT, CAP_LIMIT);

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
        assert!((impulse - 2.0 * PARTICLE_MASS * INITIAL_SPEED).abs() < TOLERANCE);
    }

    #[test]
    fn cap_collision_preserves_speed_and_reports_impulse() {
        let mut particle = Particle {
            position: vec3(0.0, CAP_LIMIT - DISTANCE_FROM_WALL, 0.0),
            velocity: vec3(0.0, INITIAL_SPEED, 0.0),
        };

        let impulse = particle.update(TIME_STEP, RADIAL_LIMIT, CAP_LIMIT);

        assert!((particle.position.y - CAP_LIMIT).abs() < TOLERANCE);
        assert!((particle.velocity.y + INITIAL_SPEED).abs() < TOLERANCE);
        assert!((impulse - 2.0 * PARTICLE_MASS * INITIAL_SPEED).abs() < TOLERANCE);
    }

    #[test]
    fn no_wall_collision_produces_no_impulse() {
        let mut particle = Particle {
            position: Vec3::ZERO,
            velocity: vec3(INITIAL_SPEED, 0.0, 0.0),
        };

        assert_eq!(particle.update(TIME_STEP, RADIAL_LIMIT, CAP_LIMIT), 0.0);
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

        particle.constrain_position(RADIAL_LIMIT, CAP_LIMIT);

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

        assert_eq!(particle.update(0.0, RADIAL_LIMIT, CAP_LIMIT), 0.0);
        assert_eq!(particle.position.x, RADIAL_LIMIT);
        assert_eq!(particle.velocity.x, -INITIAL_SPEED);
    }
}
