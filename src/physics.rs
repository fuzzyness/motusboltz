//! Hard-sphere collision resolution and reduced-unit gas measurements.

use std::f32::consts::TAU;

use macroquad::prelude::Vec3;

use crate::particle::{
    PARTICLE_MASS,
    Particle, //
};

const COLLISION_DISTANCE_EPSILON: f32 = 1e-6;
const BOLTZMANN_CONSTANT: f32 = 1.0;
const TRANSLATIONAL_DEGREES_OF_FREEDOM: f32 = 3.0;

/// Resolves an elastic collision between two equal-mass hard spheres.
///
/// Corrects overlap, then exchanges the normal velocity component of
/// approaching particles. Returns `true` only when velocities change due
/// to an actual collision; overlap correction alone is not a collision event.
pub fn resolve_pair(a: &mut Particle, b: &mut Particle, radius: f32) -> bool {
    let minimum_distance = 2.0 * radius;
    let displacement = b.position - a.position;
    let distance_squared = displacement.length_squared();

    if distance_squared >= minimum_distance * minimum_distance {
        return false;
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
        return false;
    }

    a.velocity += normal * normal_speed;
    b.velocity -= normal * normal_speed;
    true
}

/// Returns translational kinetic energy in reduced simulation units.
pub(crate) fn particle_kinetic_energy(velocity: Vec3) -> f32 {
    0.5 * PARTICLE_MASS * velocity.length_squared()
}

/// Returns the mean velocity of the gas, excluding fluctuations.
pub(crate) fn mean_velocity(particles: &[Particle]) -> Vec3 {
    if particles.is_empty() {
        return Vec3::ZERO;
    }

    particles
        .iter()
        .map(|particle| particle.velocity)
        .sum::<Vec3>()
        / particles.len() as f32
}

/// Returns the sum of particle translational kinetic energies.
pub fn kinetic_energy(particles: &[Particle]) -> f32 {
    particles
        .iter()
        .map(|particle| particle_kinetic_energy(particle.velocity))
        .sum()
}

// The mean is supplied so velocity rescaling can reuse its existing value.
fn temperature_about_mean(particles: &[Particle], mean_velocity: Vec3) -> f32 {
    let thermal_energy: f32 = particles
        .iter()
        .map(|particle| particle_kinetic_energy(particle.velocity - mean_velocity))
        .sum();

    2.0 * thermal_energy
        / (TRANSLATIONAL_DEGREES_OF_FREEDOM * (particles.len() - 1) as f32 * BOLTZMANN_CONSTANT)
}

/// Returns kinetic temperature with bulk translational motion removed.
///
/// Uses `3(N - 1)` thermal degrees of freedom and `k_B = 1`, accounting
/// for the three degrees removed with the center-of-mass velocity.
pub fn temperature(particles: &[Particle]) -> f32 {
    if particles.len() < 2 {
        return 0.0;
    }

    temperature_about_mean(particles, mean_velocity(particles))
}

/// Changes thermal kinetic energy while preserving mean particle velocity.
///
/// The reduced-unit temperature excludes the gas's center-of-mass motion.
/// Scaling velocity relative to that mean changes thermal energy without
/// imposing a net momentum impulse. This is an instantaneous temperature
/// adjustment, not a microscopic model of heat transfer through the walls.
pub fn rescale_thermal_velocities(particles: &mut [Particle], target_temperature: f32) {
    assert!(target_temperature.is_finite() && target_temperature >= 0.0);

    let count = particles.len();
    if count < 2 {
        return;
    }

    let mean_velocity = mean_velocity(particles);

    let initial_temperature = temperature_about_mean(particles, mean_velocity);
    if initial_temperature > 0.0 {
        let scale = (target_temperature / initial_temperature).sqrt();

        for particle in particles {
            particle.velocity = mean_velocity + (particle.velocity - mean_velocity) * scale;
        }
    } else if target_temperature > 0.0 {
        // A gas with no thermal motion cannot be heated by multiplication.
        // Seed a balanced pair to generate the requested thermal energy.
        let thermal_speed = (TRANSLATIONAL_DEGREES_OF_FREEDOM
            * (count - 1) as f32
            * BOLTZMANN_CONSTANT
            * target_temperature
            / (2.0 * PARTICLE_MASS))
            .sqrt();

        particles[0].velocity = mean_velocity + Vec3::X * thermal_speed;
        particles[1].velocity = mean_velocity - Vec3::X * thermal_speed;
    }
}

/// Returns average pressure over the cylinder's side and two end caps.
///
/// The total area is `2 pi r h + 2 pi r^2`. Wall impulses are summed as
/// magnitudes, so this is one surface-averaged pressure measurement.
pub fn pressure_from_wall_impulse(
    total_impulse: f32,
    chamber_radius: f32,
    chamber_height: f32,
    sample_duration: f32,
) -> f32 {
    let total_wall_area = TAU * chamber_radius * (chamber_height + chamber_radius);
    total_impulse / (total_wall_area * sample_duration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::{
        CHAMBER_HEIGHT,
        CHAMBER_RADIUS,
        PARTICLE_RADIUS, //
    };
    use macroquad::prelude::vec3;

    const TEST_PARTICLE_OFFSET: f32 = 0.16;
    const TEST_PARTICLE_SPEED: f32 = 1.0;
    const TEST_WALL_IMPULSE: f32 = 96.0;
    const TEST_SAMPLE_DURATION: f32 = 2.0;
    const EXPECTED_PRESSURE: f32 = 2.0 / std::f32::consts::PI;
    const TOLERANCE: f32 = 1e-6;
    const TARGET_TEST_TEMPERATURE: f32 = 2.0;
    const TEST_DRIFT_SPEED: f32 = 0.6;

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
        assert!(resolve_pair(&mut left[0], &mut right[0], PARTICLE_RADIUS));

        let energy_after = kinetic_energy(&particles);
        let momentum_after = particles[0].velocity + particles[1].velocity;

        assert!((energy_after - energy_before).abs() < TOLERANCE);
        assert!((momentum_after - momentum_before).length() < TOLERANCE);
        assert!(particles[0].velocity.x < 0.0);
        assert!(particles[1].velocity.x > 0.0);
    }

    #[test]
    fn separated_or_receding_particles_do_not_trigger_collision_events() {
        let mut first = Particle {
            position: vec3(-TEST_PARTICLE_OFFSET, 0.0, 0.0),
            velocity: vec3(-TEST_PARTICLE_SPEED, 0.0, 0.0),
        };
        let mut second = Particle {
            position: vec3(TEST_PARTICLE_OFFSET, 0.0, 0.0),
            velocity: vec3(TEST_PARTICLE_SPEED, 0.0, 0.0),
        };

        // Overlapping but separating: correct geometry without reporting
        // a kinetic collision or changing either velocity.
        assert!(!resolve_pair(&mut first, &mut second, PARTICLE_RADIUS));
        assert_eq!(first.velocity.x, -TEST_PARTICLE_SPEED);
        assert_eq!(second.velocity.x, TEST_PARTICLE_SPEED);

        first.position = vec3(-PARTICLE_RADIUS * 2.0, 0.0, 0.0);
        second.position = vec3(PARTICLE_RADIUS * 2.0, 0.0, 0.0);
        assert!(!resolve_pair(&mut first, &mut second, PARTICLE_RADIUS));
    }

    #[test]
    fn pressure_uses_total_wall_area_and_elapsed_time() {
        let pressure = pressure_from_wall_impulse(
            TEST_WALL_IMPULSE,
            CHAMBER_RADIUS,
            CHAMBER_HEIGHT,
            TEST_SAMPLE_DURATION,
        );

        assert!((pressure - EXPECTED_PRESSURE).abs() < TOLERANCE);
    }

    #[test]
    fn temperature_adjustment_preserves_bulk_motion() {
        let mut particles = [
            Particle {
                position: Vec3::ZERO,
                velocity: vec3(TEST_DRIFT_SPEED + TEST_PARTICLE_SPEED, 0.0, 0.0),
            },
            Particle {
                position: Vec3::ZERO,
                velocity: vec3(TEST_DRIFT_SPEED - TEST_PARTICLE_SPEED, 0.0, 0.0),
            },
        ];
        let momentum_before = particles
            .iter()
            .map(|particle| particle.velocity)
            .sum::<Vec3>();

        rescale_thermal_velocities(&mut particles, TARGET_TEST_TEMPERATURE);

        let momentum_after = particles
            .iter()
            .map(|particle| particle.velocity)
            .sum::<Vec3>();
        assert!((momentum_before - momentum_after).length() < TOLERANCE);
        assert!((temperature(&particles) - TARGET_TEST_TEMPERATURE).abs() < TOLERANCE);
    }

    #[test]
    fn temperature_adjustment_can_heat_a_motionless_gas() {
        let mut particles = [
            Particle {
                position: Vec3::ZERO,
                velocity: Vec3::ZERO,
            },
            Particle {
                position: Vec3::ZERO,
                velocity: Vec3::ZERO,
            },
        ];

        rescale_thermal_velocities(&mut particles, TARGET_TEST_TEMPERATURE);

        let net_velocity = particles[0].velocity + particles[1].velocity;
        assert!(net_velocity.length() < TOLERANCE);
        assert!((temperature(&particles) - TARGET_TEST_TEMPERATURE).abs() < TOLERANCE);
    }
}
