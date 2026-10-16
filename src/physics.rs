//! Hard-sphere collision resolution and reduced-unit gas measurements.

use macroquad::prelude::Vec3;

use crate::particle::{
    PARTICLE_MASS, //
    Particle,
};

const COLLISION_DISTANCE_EPSILON: f32 = 1e-6;
const BOLTZMANN_CONSTANT: f32 = 1.0;
const TRANSLATIONAL_DEGREES_OF_FREEDOM: f32 = 3.0;
const CUBE_FACE_COUNT: f32 = 6.0;

/// Resolves an elastic collision between two equal-mass hard spheres.
///
/// The spatial grid selects candidates; this function rejects separated
/// pairs, corrects overlap, and exchanges only the normal velocity component.
pub fn resolve_pair(a: &mut Particle, b: &mut Particle, radius: f32) {
    let minimum_distance = 2.0 * radius;
    let displacement = b.position - a.position;
    let distance_squared = displacement.length_squared();

    if distance_squared >= minimum_distance * minimum_distance {
        return;
    }

    let distance = distance_squared.sqrt();
    // A fixed fallback axis avoids division by zero for coincident centers.
    let normal = if distance > COLLISION_DISTANCE_EPSILON {
        displacement / distance
    } else {
        Vec3::X
    };

    let overlap = minimum_distance - distance;
    a.position -= normal * overlap * 0.5;
    b.position += normal * overlap * 0.5;

    let normal_speed = (b.velocity - a.velocity).dot(normal);
    if normal_speed >= 0.0 {
        return;
    }

    a.velocity += normal * normal_speed;
    b.velocity -= normal * normal_speed;
}

/// Returns translational kinetic energy in reduced simulation units.
fn particle_kinetic_energy(velocity: Vec3) -> f32 {
    0.5 * PARTICLE_MASS * velocity.length_squared()
}

/// Returns the sum of particle translational kinetic energies.
pub fn kinetic_energy(particles: &[Particle]) -> f32 {
    particles
        .iter()
        .map(|particle| particle_kinetic_energy(particle.velocity))
        .sum()
}

/// Returns kinetic temperature with bulk translational motion removed.
///
/// Uses three degrees of freedom per particle and `k_B = 1`.
pub fn temperature(particles: &[Particle]) -> f32 {
    let n = particles.len();
    if n < 2 {
        return 0.0;
    }

    let mean_velocity = particles
        .iter()
        .map(|particle| particle.velocity)
        .sum::<Vec3>()
        / n as f32;

    let thermal_energy: f32 = particles
        .iter()
        .map(|particle| particle_kinetic_energy(particle.velocity - mean_velocity))
        .sum();

    2.0 * thermal_energy / (TRANSLATIONAL_DEGREES_OF_FREEDOM * (n - 1) as f32 * BOLTZMANN_CONSTANT)
}

/// Returns average wall pressure from impulse over total area and time.
///
/// Assumes a cubic chamber and a positive sample duration.
pub fn pressure_from_wall_impulse(
    total_impulse: f32,
    chamber_size: f32,
    sample_duration: f32,
) -> f32 {
    let total_wall_area = CUBE_FACE_COUNT * chamber_size * chamber_size;
    total_impulse / (total_wall_area * sample_duration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::{
        CHAMBER_SIZE,
        PARTICLE_RADIUS, //
    };
    use macroquad::prelude::vec3;

    const TEST_PARTICLE_OFFSET: f32 = 0.16;
    const TEST_PARTICLE_SPEED: f32 = 1.0;
    const TEST_WALL_IMPULSE: f32 = 96.0;
    const TEST_SAMPLE_DURATION: f32 = 2.0;
    const EXPECTED_PRESSURE: f32 = 0.5;
    const TOLERANCE: f32 = 1e-6;

    #[test]
    fn elastic_collision_conserves_energy_and_momentum() {
        let mut particles = [
            Particle {
                position: vec3(-TEST_PARTICLE_OFFSET, 0.0, 0.0),
                velocity: vec3(TEST_PARTICLE_SPEED, 0.0, 0.0),
            },
            Particle {
                position: vec3(TEST_PARTICLE_OFFSET, 0.0, 0.0),
                velocity: vec3(-TEST_PARTICLE_SPEED, 0.0, 0.0),
            },
        ];

        let energy_before = kinetic_energy(&particles);
        let momentum_before = particles[0].velocity + particles[1].velocity;

        let (left, right) = particles.split_at_mut(1);
        resolve_pair(&mut left[0], &mut right[0], PARTICLE_RADIUS);

        let energy_after = kinetic_energy(&particles);
        let momentum_after = particles[0].velocity + particles[1].velocity;

        assert!((energy_after - energy_before).abs() < TOLERANCE);
        assert!((momentum_after - momentum_before).length() < TOLERANCE);
        assert!(particles[0].velocity.x < 0.0);
        assert!(particles[1].velocity.x > 0.0);
    }

    #[test]
    fn pressure_uses_total_wall_area_and_elapsed_time() {
        let pressure =
            pressure_from_wall_impulse(TEST_WALL_IMPULSE, CHAMBER_SIZE, TEST_SAMPLE_DURATION);

        assert!((pressure - EXPECTED_PRESSURE).abs() < TOLERANCE);
    }
}
