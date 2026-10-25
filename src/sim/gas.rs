//! Gas particles: hard-sphere collisions, kinetic energy and temperature,
//! and Maxwell-distributed velocities. Everything here is in reduced units,
//! where one unit of temperature is 300 K and Boltzmann's constant is 1.

use macroquad::prelude::Vec3;
use macroquad::rand::RandGenerator;

/// Slows all motion by this factor so particles are easy to follow. Particle
/// mass and flywheel inertia scale with its square and drag scales to match
/// (`pistons.rs`), so temperatures, energies and the ratio of gas speed to
/// shaft speed are unchanged; only the pace of the animation changes. A
/// heated engine at the default fill takes about 15 s per turn per unit of
/// scale (about 45 s at 3). The physics work per real second grows as the
/// scale drops, because faster particles need more substeps.
pub const DYNAMICS_TIME_SCALE: f32 = 3.0;
const REFERENCE_PARTICLE_MASS: f32 = 0.001;
pub const PARTICLE_MASS: f32 = REFERENCE_PARTICLE_MASS * DYNAMICS_TIME_SCALE * DYNAMICS_TIME_SCALE;
/// Initial gas temperature in reduced units (1 = 300 K).
pub const INITIAL_TEMPERATURE: f32 = 1.0;
/// Translational degrees of freedom of a monatomic gas such as helium.
pub const DEGREES_OF_FREEDOM: f32 = 3.0;
const BOLTZMANN_CONSTANT: f32 = 1.0;
const COLLISION_DISTANCE_EPSILON: f32 = 1e-6;

/// One simulated hard sphere, standing for a fixed amount of helium.
#[derive(Clone, Copy)]
pub struct Particle {
    pub position: Vec3,
    pub velocity: Vec3,
}

/// What `resolve_pair` did to two particles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Contact {
    /// Not touching; nothing changed.
    Apart,
    /// Overlapping but already separating: pushed apart, velocities kept.
    Overlap,
    /// Overlapping and approaching: pushed apart and bounced.
    Impact,
}

/// Resolves an elastic collision between two equal-mass hard spheres.
///
/// Corrects overlap, then exchanges the normal velocity component of
/// approaching particles. Only `Contact::Impact` changes velocities;
/// overlap correction alone is not a collision event, but it does move
/// both particles.
pub fn resolve_pair(a: &mut Particle, b: &mut Particle, radius: f32) -> Contact {
    let minimum_distance = 2.0 * radius;
    let displacement = b.position - a.position;
    let distance_squared = displacement.length_squared();

    if distance_squared >= minimum_distance * minimum_distance {
        return Contact::Apart;
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
        return Contact::Overlap;
    }

    a.velocity += normal * normal_speed;
    b.velocity -= normal * normal_speed;
    Contact::Impact
}

/// Translational kinetic energy of one particle.
pub fn particle_kinetic_energy(velocity: Vec3) -> f32 {
    0.5 * PARTICLE_MASS * velocity.length_squared()
}

/// Arithmetic mean velocity of the particles; zero for none.
pub fn mean_velocity(particles: &[Particle]) -> Vec3 {
    if particles.is_empty() {
        return Vec3::ZERO;
    }

    particles
        .iter()
        .map(|particle| particle.velocity)
        .sum::<Vec3>()
        / particles.len() as f32
}

/// Sum of the particles' translational kinetic energies.
pub fn kinetic_energy(particles: &[Particle]) -> f32 {
    particles
        .iter()
        .map(|particle| particle_kinetic_energy(particle.velocity))
        .sum()
}

/// Temperature of motion relative to `mean_velocity`, which the caller
/// supplies so a rescale can reuse it.
fn temperature_about_mean(particles: &[Particle], mean_velocity: Vec3) -> f32 {
    let thermal_energy: f32 = particles
        .iter()
        .map(|particle| particle_kinetic_energy(particle.velocity - mean_velocity))
        .sum();

    2.0 * thermal_energy / (DEGREES_OF_FREEDOM * (particles.len() - 1) as f32 * BOLTZMANN_CONSTANT)
}

/// Scales velocities about their mean so the sample has exactly
/// `target_temperature`, keeping total momentum unchanged.
///
/// Used only to set the initial 300 K fill. Heating while the engine runs
/// happens solely through wall impacts (`ThermalReservoirs::reflect`).
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
        let thermal_speed =
            (DEGREES_OF_FREEDOM * (count - 1) as f32 * BOLTZMANN_CONSTANT * target_temperature
                / (2.0 * PARTICLE_MASS))
                .sqrt();

        particles[0].velocity = mean_velocity + Vec3::X * thermal_speed;
        particles[1].velocity = mean_velocity - Vec3::X * thermal_speed;
    }
}

/// Speed away from a wall at `temperature`, flux weighted: probability
/// density proportional to v * exp(-m * v^2 / (2 * k_B * T)).
pub fn sample_normal_speed(temperature: f32, random: &RandGenerator) -> f32 {
    let uniform = random.gen_range(0.0_f32, 1.0_f32).max(f32::MIN_POSITIVE);
    (-2.0 * temperature * uniform.ln() / PARTICLE_MASS).sqrt()
}

/// Two independent Gaussian velocity components with variance kT/m
/// (Box-Muller form).
pub fn sample_gaussian_pair(temperature: f32, random: &RandGenerator) -> (f32, f32) {
    let radius = sample_normal_speed(temperature, random);
    let angle = random.gen_range(0.0_f32, std::f32::consts::TAU);
    let (sin, cos) = angle.sin_cos();
    (radius * cos, radius * sin)
}

/// Random 3D velocity from the Maxwell distribution at `temperature`.
pub fn sample_thermal_velocity(temperature: f32, random: &RandGenerator) -> Vec3 {
    let (x, y) = sample_gaussian_pair(temperature, random);
    let (z, _) = sample_gaussian_pair(temperature, random);
    Vec3::new(x, y, z)
}

#[cfg(test)]
#[path = "../../tests/unit/sim/gas.rs"]
mod tests;
