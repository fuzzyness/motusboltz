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

/// Lowest hot-reservoir temperature, equal to the highest cold one so both
/// reservoirs can be set equal.
pub const MIN_HOT_KELVIN: f32 = KELVIN_PER_UNIT as f32;
/// Highest hot-reservoir temperature.
pub const MAX_HOT_KELVIN: f32 = 1_200.0;
/// Lowest cold-reservoir temperature.
pub const MIN_COLD_KELVIN: f32 = 100.0;
/// Highest cold-reservoir temperature.
pub const MAX_COLD_KELVIN: f32 = KELVIN_PER_UNIT as f32;
/// Chance that a cylinder-wall impact exchanges heat with its reservoir.
const ACCOMMODATION_PROBABILITY: f32 = 0.5;
/// Largest share of a wall normal along x for which the x axis still makes
/// a well-conditioned tangent.
const BASIS_ALIGNMENT_LIMIT: f32 = 0.9;

/// Which reservoir a wall belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReservoirSide {
    /// The heated cylinder's side and floor.
    Hot,
    /// The cooled cylinder's side and floor.
    Cold,
}

/// A running total kept for each reservoir, in reduced units.
#[derive(Clone, Copy, Debug, Default)]
pub struct PerReservoir {
    /// The hot reservoir's share.
    pub hot: f64,
    /// The cold reservoir's share.
    pub cold: f64,
}

impl PerReservoir {
    /// Both reservoirs' shares together.
    pub fn total(self) -> f64 {
        self.hot + self.cold
    }

    /// The share of the reservoir on `side`.
    fn side_mut(&mut self, side: ReservoirSide) -> &mut f64 {
        match side {
            ReservoirSide::Hot => &mut self.hot,
            ReservoirSide::Cold => &mut self.cold,
        }
    }
}

/// Hot and cold wall temperatures, the random stream for wall impacts, and
/// the heat and entropy each reservoir has given the gas.
pub struct ThermalReservoirs {
    /// Hot wall temperature.
    hot_kelvin: f32,
    /// Cold wall temperature.
    cold_kelvin: f32,
    /// Chance that a wall impact exchanges heat (see
    /// `ACCOMMODATION_PROBABILITY`).
    accommodation: f32,
    /// Heat into the gas from each reservoir; negative when heat left it.
    heat: PerReservoir,
    /// Heat into the gas over the temperature of the reservoir it came
    /// from, summed over every exchange.
    ///
    /// Each reservoir's own entropy changes by minus its share, so the
    /// surroundings change by minus the total.
    entropy: PerReservoir,
    /// Random stream for wall impacts.
    random: RandGenerator,
}

impl ThermalReservoirs {
    /// Reservoirs at 300 K whose wall impacts draw from a generator seeded
    /// with `seed`.
    pub fn with_seed(seed: u64) -> Self {
        let random = RandGenerator::new();
        random.srand(seed);
        Self {
            hot_kelvin: MIN_HOT_KELVIN,
            cold_kelvin: MAX_COLD_KELVIN,
            accommodation: ACCOMMODATION_PROBABILITY,
            heat: PerReservoir::default(),
            entropy: PerReservoir::default(),
            random,
        }
    }

    /// Sets the hot wall temperature; the gas changes only as it hits it.
    ///
    /// # Panics
    ///
    /// Panics if `kelvin` is outside the hot range.
    pub fn set_hot_kelvin(&mut self, kelvin: f32) {
        assert!((MIN_HOT_KELVIN..=MAX_HOT_KELVIN).contains(&kelvin));
        self.hot_kelvin = kelvin;
    }

    /// Sets the cold wall temperature; the gas changes only as it hits it.
    ///
    /// # Panics
    ///
    /// Panics if `kelvin` is outside the cold range.
    pub fn set_cold_kelvin(&mut self, kelvin: f32) {
        assert!((MIN_COLD_KELVIN..=MAX_COLD_KELVIN).contains(&kelvin));
        self.cold_kelvin = kelvin;
    }

    /// The hot wall temperature.
    pub fn hot_kelvin(&self) -> f32 {
        self.hot_kelvin
    }

    /// The cold wall temperature.
    pub fn cold_kelvin(&self) -> f32 {
        self.cold_kelvin
    }

    /// Heat each reservoir has given the gas since the fill.
    pub fn heat(&self) -> PerReservoir {
        self.heat
    }

    /// Heat over reservoir temperature, per reservoir, since the fill.
    pub fn entropy(&self) -> PerReservoir {
        self.entropy
    }

    /// Bounces `particle` off a stationary wall with outward normal
    /// `outward_normal`; a particle already moving away is left alone.
    ///
    /// An insulated wall (`side` of `None`), or a reservoir wall that loses
    /// the accommodation draw, mirrors the particle without changing its
    /// energy. Otherwise the particle leaves at the reservoir's temperature,
    /// with a flux-weighted Maxwellian normal speed and Gaussian tangential
    /// components, and its change in energy is booked as heat, and that
    /// heat over the temperature as entropy.
    pub fn reflect(
        &mut self,
        particle: &mut Particle,
        outward_normal: Vec3,
        side: Option<ReservoirSide>,
    ) {
        if particle.velocity.dot(outward_normal) <= 0.0 {
            return;
        }
        let side = side.filter(|_| self.random.gen_range(0.0_f32, 1.0) < self.accommodation);
        let Some(side) = side else {
            particle.velocity = mirrored(particle.velocity, outward_normal);
            return;
        };
        let kelvin = match side {
            ReservoirSide::Hot => self.hot_kelvin,
            ReservoirSide::Cold => self.cold_kelvin,
        };
        let temperature = kelvin / KELVIN_PER_UNIT as f32;
        let before = particle_kinetic_energy(particle.velocity);
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
        *self.heat.side_mut(side) += heat;
        *self.entropy.side_mut(side) += heat / temperature as f64;
    }
}

/// `velocity` mirrored across a wall with outward normal `outward_normal`,
/// which need not be exactly unit length.
///
/// Computed in f64 and rounded once: in f32 the normal's rounding and the
/// arithmetic shortened the speed slightly on average, which over many
/// bounces lost gas energy that no heat or work accounted for (about 0.03
/// units per 300 s at 1,024 particles, where the opening between the bores
/// meets their curved walls).
fn mirrored(velocity: Vec3, outward_normal: Vec3) -> Vec3 {
    let normal = outward_normal.as_dvec3().normalize();
    let velocity = velocity.as_dvec3();
    (velocity - 2.0 * velocity.dot(normal) * normal).as_vec3()
}

#[cfg(test)]
#[path = "../../tests/unit/sim/reservoir.rs"]
mod tests;
