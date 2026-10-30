//! The hot and cold walls: their temperatures, the heat each exchanges with
//! the gas at wall impacts, and the entropy that heat carries.

use macroquad::prelude::Vec3;
use macroquad::rand::RandGenerator;

use crate::sim::gas::{
    Particle,
    particle_kinetic_energy,
    sample_gaussian_pair,
    sample_normal_speed, //
};
use crate::sim::units::KELVIN_PER_UNIT;

/// Lowest hot-reservoir temperature, 300 K, equal to the highest cold one so
/// both reservoirs can be set equal.
pub const MIN_HOT_KELVIN: f32 = KELVIN_PER_UNIT as f32;
/// Lowest cold-reservoir temperature.
pub const MIN_COLD_KELVIN: f32 = 100.0;
/// Highest cold-reservoir temperature, 300 K.
pub const MAX_COLD_KELVIN: f32 = KELVIN_PER_UNIT as f32;
/// Highest hot-reservoir temperature.
pub const MAX_HOT_KELVIN: f32 = 1_200.0;
/// Chance that a cylinder-wall impact exchanges heat with its reservoir.
const ACCOMMODATION_PROBABILITY: f32 = 0.5;

/// Which thermal reservoir supplies or absorbs energy at a wall impact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReservoirSide {
    Hot,
    Cold,
}

/// Cumulative signed heat delivered to the gas, in reduced energy units.
/// Negative values mean energy has left the gas.
#[derive(Clone, Copy, Debug, Default)]
pub struct HeatTransfer {
    pub hot: f64,
    pub cold: f64,
}

impl HeatTransfer {
    pub fn total(self) -> f64 {
        self.hot + self.cold
    }
}

/// Cumulative heat into the gas divided by the temperature of the reservoir
/// that supplied it, q / T summed over every heat-exchanging wall impact, in
/// reduced units (energy per reduced temperature).
///
/// Each reservoir's own entropy changes by minus its entry, so the entropy
/// change of the surroundings is `-(hot + cold)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct EntropyFlow {
    pub hot: f64,
    pub cold: f64,
}

/// Hot and cold wall temperatures, the random stream for wall impacts, and
/// the cumulative heat each reservoir has given the gas. Both start at 300 K.
pub struct ThermalReservoirs {
    hot_kelvin: f32,
    cold_kelvin: f32,
    accommodation: f32,
    heat: HeatTransfer,
    entropy: EntropyFlow,
    random: RandGenerator,
}

impl ThermalReservoirs {
    /// Reservoirs at 300 K whose wall impacts use their own seeded generator.
    pub fn with_seed(seed: u64) -> Self {
        let random = RandGenerator::new();
        random.srand(seed);
        Self {
            random,
            hot_kelvin: MIN_HOT_KELVIN,
            cold_kelvin: MAX_COLD_KELVIN,
            accommodation: ACCOMMODATION_PROBABILITY,
            heat: HeatTransfer::default(),
            entropy: EntropyFlow::default(),
        }
    }

    /// Sets the hot wall temperature; particle velocities are not touched.
    pub fn set_hot_kelvin(&mut self, kelvin: f32) {
        assert!(kelvin.is_finite() && (MIN_HOT_KELVIN..=MAX_HOT_KELVIN).contains(&kelvin));
        self.hot_kelvin = kelvin;
    }

    /// Sets the cold wall temperature; particle velocities are not touched.
    pub fn set_cold_kelvin(&mut self, kelvin: f32) {
        assert!(kelvin.is_finite() && (MIN_COLD_KELVIN..=MAX_COLD_KELVIN).contains(&kelvin));
        self.cold_kelvin = kelvin;
    }

    pub fn cold_kelvin(&self) -> f32 {
        self.cold_kelvin
    }

    pub fn hot_kelvin(&self) -> f32 {
        self.hot_kelvin
    }

    pub fn heat(&self) -> HeatTransfer {
        self.heat
    }

    pub fn entropy(&self) -> EntropyFlow {
        self.entropy
    }

    /// Bounces a particle off a stationary wall with the given outward unit
    /// normal. Particles already moving away from the wall are unchanged.
    ///
    /// With `side = None` (insulated wall), or when the accommodation draw
    /// fails, the bounce is a mirror reflection with no energy change.
    /// Otherwise the particle leaves with a new velocity drawn at that
    /// reservoir's temperature: a flux-weighted Maxwellian normal speed and
    /// Gaussian tangential components. Its change in kinetic energy is
    /// booked as heat from that reservoir, and that heat divided by the
    /// reservoir temperature as entropy flow.
    pub fn reflect(
        &mut self,
        particle: &mut Particle,
        outward_normal: Vec3,
        side: Option<ReservoirSide>,
    ) {
        let incoming_speed = particle.velocity.dot(outward_normal);
        if incoming_speed <= 0.0 {
            return;
        }

        if let Some(side) =
            side.filter(|_| self.random.gen_range(0.0_f32, 1.0_f32) < self.accommodation)
        {
            let temperature = match side {
                ReservoirSide::Hot => self.hot_kelvin / KELVIN_PER_UNIT as f32,
                ReservoirSide::Cold => self.cold_kelvin / KELVIN_PER_UNIT as f32,
            };
            let before = particle_kinetic_energy(particle.velocity);
            // Choose a well-conditioned tangent basis for diffuse reflection.
            const BASIS_ALIGNMENT_LIMIT: f32 = 0.9;
            let axis = if outward_normal.x.abs() < BASIS_ALIGNMENT_LIMIT {
                Vec3::X
            } else {
                Vec3::Y
            };
            let tangent = outward_normal.cross(axis).normalize();
            let bitangent = outward_normal.cross(tangent);
            let (u, v) = sample_gaussian_pair(temperature, &self.random);
            particle.velocity = -outward_normal * sample_normal_speed(temperature, &self.random)
                + tangent * u
                + bitangent * v;
            let heat = (particle_kinetic_energy(particle.velocity) - before) as f64;
            let entropy = heat / temperature as f64;
            match side {
                ReservoirSide::Hot => {
                    self.heat.hot += heat;
                    self.entropy.hot += entropy;
                }
                ReservoirSide::Cold => {
                    self.heat.cold += heat;
                    self.entropy.cold += entropy;
                }
            }
            return;
        }
        particle.velocity = mirrored(particle.velocity, outward_normal);
    }
}

/// `velocity` mirrored across a wall with the given outward normal, which
/// need not be exactly unit length. Computed in f64 and rounded once: in f32
/// the normal's rounding and the arithmetic shortened the speed slightly on
/// average, which over many bounces lost gas energy that no heat or work
/// accounted for (about 0.03 units per 300 s at 1,024 particles, where the
/// opening between the bores meets their curved walls).
fn mirrored(velocity: Vec3, outward_normal: Vec3) -> Vec3 {
    let normal = [outward_normal.x, outward_normal.y, outward_normal.z].map(f64::from);
    let length = normal.iter().map(|c| c * c).sum::<f64>().sqrt();
    let normal = normal.map(|c| c / length);
    let velocity = [velocity.x, velocity.y, velocity.z].map(f64::from);
    let along: f64 = velocity.iter().zip(&normal).map(|(v, n)| v * n).sum();
    let [x, y, z] = [0, 1, 2].map(|axis| (velocity[axis] - 2.0 * along * normal[axis]) as f32);
    Vec3::new(x, y, z)
}

#[cfg(test)]
#[path = "../../tests/unit/sim/reservoir.rs"]
mod tests;
