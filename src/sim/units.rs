//! SI units: how much helium each particle stands for, the fill range, and
//! conversions from reduced simulation units to joules, kelvin, pascals and
//! cubic centimeters.

use macroquad::prelude::Vec3;

use crate::sim::gas::{
    DEGREES_OF_FREEDOM,
    PARTICLE_MASS, //
};

/// Molar gas constant, in joules per mole-kelvin.
pub const GAS_CONSTANT: f64 = 8.314_462_618_153_24;
/// Physical length of one model distance unit (1 cm).
pub const METERS_PER_UNIT: f64 = 0.01;
/// Temperature of one reduced unit of temperature.
pub const KELVIN_PER_UNIT: f64 = 300.0;
/// Liters in a cubic meter.
const LITERS_PER_CUBIC_METER: f64 = 1_000.0;
/// Cubic centimeters in a cubic meter.
const CUBIC_CENTIMETERS_PER_CUBIC_METER: f64 = 1.0e6;

/// Largest fill, which holds `MAX_CONCENTRATION` in the initial volume.
pub const MAX_PARTICLES: usize = 2_048;
/// Smallest fill.
pub const MIN_PARTICLES: usize = 256;
/// Fill at launch.
pub const DEFAULT_PARTICLES: usize = 1_024;
/// Concentration of the largest fill, in mol/L.
pub const MAX_CONCENTRATION: f32 = 1.0;
/// Concentration of the smallest fill, in mol/L.
pub const MIN_CONCENTRATION: f32 = MAX_CONCENTRATION * MIN_PARTICLES as f32 / MAX_PARTICLES as f32;
/// Concentration at launch, in mol/L.
pub const DEFAULT_CONCENTRATION: f32 =
    MAX_CONCENTRATION * DEFAULT_PARTICLES as f32 / MAX_PARTICLES as f32;

/// Converts a volume in cubic model units to cubic centimeters.
pub fn volume_cm3(cubic_units: f64) -> f64 {
    cubic_units * METERS_PER_UNIT.powi(3) * CUBIC_CENTIMETERS_PER_CUBIC_METER
}

/// Running sums of particle velocities, for a region's temperature without
/// collecting its particles.
///
/// The temperature uses motion relative to the region's mean velocity, so
/// bulk flow does not count as heat.
#[derive(Clone, Copy, Debug, Default)]
pub struct KineticMoments {
    /// Particles recorded.
    count: usize,
    /// Sum of their velocities, per axis.
    velocity_sum: [f64; 3],
    /// Sum of their squared speeds.
    speed_squared_sum: f64,
}

impl KineticMoments {
    /// Adds one particle moving at `velocity`.
    pub fn record(&mut self, velocity: Vec3) {
        self.count += 1;
        for (sum, component) in self.velocity_sum.iter_mut().zip(velocity.to_array()) {
            let component = component as f64;
            *sum += component;
            self.speed_squared_sum += component * component;
        }
    }

    /// Particles recorded.
    pub fn count(self) -> usize {
        self.count
    }

    /// Kinetic temperature in kelvin, or `None` with fewer than two
    /// particles.
    pub fn temperature_kelvin(self) -> Option<f64> {
        if self.count < 2 {
            return None;
        }
        let count = self.count as f64;
        let mean_motion = self.velocity_sum.iter().map(|sum| sum * sum).sum::<f64>() / count;
        let thermal_motion = (self.speed_squared_sum - mean_motion).max(0.0);
        let reduced = thermal_motion / (DEGREES_OF_FREEDOM as f64 * (count - 1.0));
        Some(reduced * PARTICLE_MASS as f64 * KELVIN_PER_UNIT)
    }
}

/// Readings for one region.
///
/// The pressure is the ideal-gas estimate nRT/V, not a measured force on the
/// walls.
#[derive(Clone, Copy, Debug)]
pub struct GasMeasurements {
    /// Particles in the region.
    pub count: usize,
    /// Kinetic temperature, if the region has at least two particles.
    pub temperature_kelvin: Option<f64>,
    /// Helium concentration, in mol/L.
    pub concentration_mol_per_l: f64,
    /// Ideal-gas pressure, when the temperature is known.
    pub pressure_pa: Option<f64>,
}

/// Converts between simulation units and SI with a fixed amount of helium
/// per particle.
///
/// The amount is chosen so the largest fill holds `MAX_CONCENTRATION` in the
/// initial volume, so more particles means more helium.
#[derive(Clone, Copy, Debug)]
pub struct HeliumCalibration {
    /// Helium each particle stands for, in moles.
    moles_per_particle: f64,
}

impl HeliumCalibration {
    /// Calibrates for an initial volume of `volume_m3`.
    ///
    /// # Panics
    ///
    /// Panics if `volume_m3` is not a positive, finite volume.
    pub fn new(volume_m3: f64) -> Self {
        assert!(volume_m3.is_finite() && volume_m3 > 0.0);
        let liters = volume_m3 * LITERS_PER_CUBIC_METER;
        Self {
            moles_per_particle: MAX_CONCENTRATION as f64 * liters / MAX_PARTICLES as f64,
        }
    }

    /// Helium amount represented by `count` particles.
    pub fn total_moles(self, count: usize) -> f64 {
        self.moles_per_particle * count as f64
    }

    /// Converts reduced energy to joules.
    ///
    /// One reduced unit per particle is R x 300 K per mole.
    pub fn energy_joules(self, reduced_energy: impl Into<f64>) -> f64 {
        self.moles_per_particle * GAS_CONSTANT * KELVIN_PER_UNIT * reduced_energy.into()
    }

    /// Converts reduced entropy (energy per reduced temperature) to J/K.
    ///
    /// The 300 K factors of the energy and temperature scales cancel.
    pub fn entropy_joules_per_kelvin(self, reduced_entropy: f64) -> f64 {
        self.moles_per_particle * GAS_CONSTANT * reduced_entropy
    }

    /// Converts reduced pressure (energy per cubic model unit) to pascals.
    pub fn pressure_pa(self, reduced_pressure: f64) -> f64 {
        self.energy_joules(reduced_pressure) / METERS_PER_UNIT.powi(3)
    }

    /// Readings for a region's particles filling `volume_m3`.
    ///
    /// # Panics
    ///
    /// Panics if `volume_m3` is not positive.
    pub fn measure(self, sample: KineticMoments, volume_m3: f64) -> GasMeasurements {
        assert!(volume_m3 > 0.0);
        let moles = self.total_moles(sample.count);
        let temperature_kelvin = sample.temperature_kelvin();
        GasMeasurements {
            count: sample.count,
            temperature_kelvin,
            concentration_mol_per_l: moles / (volume_m3 * LITERS_PER_CUBIC_METER),
            pressure_pa: temperature_kelvin.map(|kelvin| moles * GAS_CONSTANT * kelvin / volume_m3),
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/sim/units.rs"]
mod tests;
