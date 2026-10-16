//! Free-flight motion and elastic collisions with stationary chamber walls.

use macroquad::prelude::Vec3;

/// Common particle mass in reduced simulation units.
pub const PARTICLE_MASS: f32 = 1.0;

/// A hard sphere represented by its center position and velocity.
pub struct Particle {
    pub position: Vec3,
    pub velocity: Vec3,
}

impl Particle {
    /// Advances the particle and reflects it from the chamber walls.
    ///
    /// Returns the sum of impulse magnitudes transferred to the walls.
    /// `wall_limit` is the maximum center coordinate on each axis.
    pub fn update(&mut self, dt: f32, wall_limit: f32) -> f32 {
        self.position += self.velocity * dt;

        let x_impulse = bounce(&mut self.position.x, &mut self.velocity.x, wall_limit);
        let y_impulse = bounce(&mut self.position.y, &mut self.velocity.y, wall_limit);
        let z_impulse = bounce(&mut self.position.z, &mut self.velocity.z, wall_limit);

        x_impulse + y_impulse + z_impulse
    }
}

/// Reflects one velocity component and returns the wall impulse magnitude.
fn bounce(position: &mut f32, velocity: &mut f32, limit: f32) -> f32 {
    if *position > limit {
        *position = limit;
        let speed = (*velocity).abs();
        *velocity = -speed;
        2.0 * PARTICLE_MASS * speed
    } else if *position < -limit {
        *position = -limit;
        let speed = (*velocity).abs();
        *velocity = speed;
        2.0 * PARTICLE_MASS * speed
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::prelude::vec3;

    const WALL_LIMIT: f32 = 1.82;
    const DISTANCE_FROM_WALL: f32 = 0.12;
    const INITIAL_SPEED: f32 = 1.5;
    const TIME_STEP: f32 = 0.2;
    const TOLERANCE: f32 = 1e-6;

    #[test]
    fn wall_collision_preserves_speed_and_reports_impulse() {
        let mut particle = Particle {
            position: vec3(WALL_LIMIT - DISTANCE_FROM_WALL, 0.0, 0.0),
            velocity: vec3(INITIAL_SPEED, 0.0, 0.0),
        };

        let impulse = particle.update(TIME_STEP, WALL_LIMIT);
        let expected_impulse = 2.0 * PARTICLE_MASS * INITIAL_SPEED;

        assert!((particle.position.x - WALL_LIMIT).abs() < TOLERANCE);
        assert!((particle.velocity.x + INITIAL_SPEED).abs() < TOLERANCE);
        assert!((impulse - expected_impulse).abs() < TOLERANCE);
    }

    #[test]
    fn no_wall_collision_produces_no_impulse() {
        let mut particle = Particle {
            position: Vec3::ZERO,
            velocity: vec3(INITIAL_SPEED, 0.0, 0.0),
        };

        let impulse = particle.update(TIME_STEP, WALL_LIMIT);

        assert_eq!(impulse, 0.0);
    }
}
