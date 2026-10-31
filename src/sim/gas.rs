//! Gas particles: hard-sphere collisions, kinetic energy and temperature,
//! and Maxwell-distributed velocities.
//!
//! Everything here is in reduced units, where one unit of temperature is
//! 300 K and Boltzmann's constant is 1.

use macroquad::prelude::Vec3;
use macroquad::rand::RandGenerator;

/// Slows all motion by this factor so particles are easy to follow.
///
/// Particle mass and flywheel inertia scale with its square and drag scales
/// to match (see `pistons`), so temperatures, energies and the ratio of gas
/// speed to shaft speed are unchanged; only the pace changes. A heated
/// engine at the default fill takes about 6.5 to 7.5 s per turn per unit of
/// scale. Faster particles need more substeps, so the physics costs more as
/// the scale drops: at the largest fill, 2.5 costs 1.23 times as much as 3,
/// and 2 costs 1.66 times. Slow mode (see `app`) slows the pace further
/// without touching the physics.
pub const DYNAMICS_TIME_SCALE: f32 = 2.5;
/// Particle mass before the time scale.
const REFERENCE_PARTICLE_MASS: f32 = 0.001;
/// Mass of every particle.
pub const PARTICLE_MASS: f32 = REFERENCE_PARTICLE_MASS * DYNAMICS_TIME_SCALE * DYNAMICS_TIME_SCALE;
/// Temperature of the gas at every fill (1 = 300 K).
pub const INITIAL_TEMPERATURE: f32 = 1.0;
/// Translational degrees of freedom of a monatomic gas such as helium.
pub const DEGREES_OF_FREEDOM: f32 = 3.0;
/// Center distance below which two particles count as coincident.
const COINCIDENT_DISTANCE: f32 = 1e-6;

/// One simulated hard sphere, standing for a fixed amount of helium.
#[derive(Clone, Copy)]
pub struct Particle {
    /// Center, in model units.
    pub position: Vec3,
    /// Velocity, in model units per second.
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

/// Resolves an elastic collision between two equal-mass hard spheres of
/// `radius`.
///
/// Overlapping spheres are pushed apart, and approaching ones exchange the
/// velocity components along the line between their centers.
pub fn resolve_pair(a: &mut Particle, b: &mut Particle, radius: f32) -> Contact {
    let minimum_distance = 2.0 * radius;
    let displacement = b.position - a.position;
    let distance_squared = displacement.length_squared();
    if distance_squared >= minimum_distance * minimum_distance {
        return Contact::Apart;
    }

    let distance = distance_squared.sqrt();
    // A fixed axis avoids dividing by zero for coincident centers.
    let normal = if distance > COINCIDENT_DISTANCE {
        displacement / distance
    } else {
        Vec3::X
    };
    let half_overlap = 0.5 * (minimum_distance - distance);
    a.position -= normal * half_overlap;
    b.position += normal * half_overlap;

    let normal_speed = (b.velocity - a.velocity).dot(normal);
    if normal_speed >= 0.0 {
        return Contact::Overlap;
    }
    a.velocity += normal * normal_speed;
    b.velocity -= normal * normal_speed;
    Contact::Impact
}

/// Translational kinetic energy of one particle moving at `velocity`.
pub fn particle_kinetic_energy(velocity: Vec3) -> f32 {
    0.5 * PARTICLE_MASS * velocity.length_squared()
}

/// Mean velocity of `particles`, or zero for none.
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

/// Brings `particles` to rest on average and to exactly `temperature`.
///
/// Used only for a new fill. While the engine runs, heat enters only
/// through wall impacts (see `ThermalReservoirs::reflect`).
///
/// # Panics
///
/// Panics if `temperature` is negative or not finite.
pub fn thermalize(particles: &mut [Particle], temperature: f32) {
    assert!(temperature.is_finite() && temperature >= 0.0);
    if particles.len() < 2 {
        return;
    }
    let mean = mean_velocity(particles);
    for particle in particles.iter_mut() {
        particle.velocity -= mean;
    }
    // Rounding leaves a trace of the mean; scale the motion about it.
    let rest = mean_velocity(particles);
    let thermal_energy: f32 = particles
        .iter()
        .map(|particle| particle_kinetic_energy(particle.velocity - rest))
        .sum();
    let current = 2.0 * thermal_energy / (DEGREES_OF_FREEDOM * (particles.len() - 1) as f32);
    if current > 0.0 {
        let scale = (temperature / current).sqrt();
        for particle in particles {
            particle.velocity = rest + (particle.velocity - rest) * scale;
        }
    }
}

/// Speed away from a wall at `temperature`, weighted by flux: its density is
/// proportional to v exp(-m v^2 / 2kT).
pub fn sample_normal_speed(temperature: f32, random: &RandGenerator) -> f32 {
    let uniform = random.gen_range(0.0_f32, 1.0).max(f32::MIN_POSITIVE);
    (-2.0 * temperature * uniform.ln() / PARTICLE_MASS).sqrt()
}

/// Two independent Gaussian velocity components with variance kT/m, in the
/// Box-Muller form.
pub fn sample_gaussian_pair(temperature: f32, random: &RandGenerator) -> (f32, f32) {
    let radius = sample_normal_speed(temperature, random);
    let (sin, cos) = random.gen_range(0.0_f32, std::f32::consts::TAU).sin_cos();
    (radius * cos, radius * sin)
}

/// Random velocity from the Maxwell distribution at `temperature`.
pub fn sample_thermal_velocity(temperature: f32, random: &RandGenerator) -> Vec3 {
    let (x, y) = sample_gaussian_pair(temperature, random);
    let (z, _) = sample_gaussian_pair(temperature, random);
    Vec3::new(x, y, z)
}

#[cfg(test)]
#[path = "../../tests/unit/sim/gas.rs"]
mod tests;
