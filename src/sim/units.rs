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
/// Physical distance corresponding to one model distance unit (1 cm).
pub const METERS_PER_UNIT: f64 = 0.01;
/// One reduced unit of temperature represents 300 kelvin.
pub const KELVIN_PER_UNIT: f64 = 300.0;
const LITERS_PER_CUBIC_METER: f64 = 1_000.0;
const CUBIC_CENTIMETERS_PER_CUBIC_METER: f64 = 1.0e6;

/// Particle count corresponding to an initial system-wide concentration of
/// 1 mol/L; also the largest fill offered.
pub const MAX_SIMULATED_MARKERS: usize = 2_048;
pub const MIN_CONNECTED_MARKERS: usize = 256;
pub const DEFAULT_CONNECTED_MARKERS: usize = 1_024;
pub const MAX_INITIAL_CONCENTRATION_MOL_PER_L: f32 = 1.0;
pub const MIN_INITIAL_CONCENTRATION_MOL_PER_L: f32 = MAX_INITIAL_CONCENTRATION_MOL_PER_L
    * MIN_CONNECTED_MARKERS as f32
    / MAX_SIMULATED_MARKERS as f32;
pub const DEFAULT_INITIAL_CONCENTRATION_MOL_PER_L: f32 = MAX_INITIAL_CONCENTRATION_MOL_PER_L
    * DEFAULT_CONNECTED_MARKERS as f32
    / MAX_SIMULATED_MARKERS as f32;

/// Converts a volume in cubic model units to cubic centimeters.
pub fn volume_cm3(cubic_units: f64) -> f64 {
    cubic_units * METERS_PER_UNIT.powi(3) * CUBIC_CENTIMETERS_PER_CUBIC_METER
}

/// Running sums of particle velocities, for measuring a region's temperature
/// without collecting its particles. Temperature uses motion relative to the
/// region's mean velocity, so bulk flow does not count as heat.
#[derive(Clone, Copy, Debug, Default)]
pub struct KineticMoments {
    count: usize,
    velocity_sum: [f64; 3],
    speed_squared_sum: f64,
}

impl KineticMoments {
    pub fn record(&mut self, velocity: Vec3) {
        self.count += 1;
        let components = [velocity.x as f64, velocity.y as f64, velocity.z as f64];
        for (total, component) in self.velocity_sum.iter_mut().zip(components) {
            *total += component;
            self.speed_squared_sum += component * component;
        }
    }

    pub fn count(self) -> usize {
        self.count
    }

    /// Kinetic temperature in kelvin; `None` with fewer than two particles.
    pub fn temperature_kelvin(self) -> Option<f64> {
        if self.count < 2 {
            return None;
        }
        let count = self.count as f64;
        let mean_motion = self.velocity_sum.iter().map(|sum| sum * sum).sum::<f64>() / count;
        let thermal_motion = (self.speed_squared_sum - mean_motion).max(0.0);
        let reduced_temperature = thermal_motion / (DEGREES_OF_FREEDOM as f64 * (count - 1.0));
        Some(reduced_temperature * PARTICLE_MASS as f64 * KELVIN_PER_UNIT)
    }
}

/// Readings for one region. Pressure is the ideal-gas estimate nRT/V, not a
/// measured force on the walls.
#[derive(Clone, Copy, Debug)]
pub struct GasMeasurements {
    pub markers: usize,
    pub temperature_kelvin: Option<f64>,
    pub concentration_mol_per_l: f64,
    pub pressure_pa: Option<f64>,
}

/// Converts between simulation units and SI using a fixed amount of helium
/// per particle.
///
/// The amount is chosen so the maximum particle count fills the initial
/// volume at the maximum concentration (1 mol/L). More particles therefore
/// means more helium.
#[derive(Clone, Copy, Debug)]
pub struct HeliumCalibration {
    moles_per_marker: f64,
}

impl HeliumCalibration {
    /// Sets moles per particle = max concentration x volume / max particles.
    pub fn new(total_volume_m3: f64, max_markers: usize, max_concentration: f64) -> Self {
        assert!(max_markers > 0);
        assert!(total_volume_m3.is_finite() && total_volume_m3 > 0.0);
        assert!(max_concentration.is_finite() && max_concentration > 0.0);
        let total_volume_liters = total_volume_m3 * LITERS_PER_CUBIC_METER;
        Self {
            moles_per_marker: max_concentration * total_volume_liters / max_markers as f64,
        }
    }

    /// Helium amount represented by `markers` particles.
    pub fn total_moles(self, markers: usize) -> f64 {
        self.moles_per_marker * markers as f64
    }

    /// Converts reduced energy to joules: one reduced unit per particle is
    /// R x 300 K per mole.
    pub fn energy_joules(self, reduced_energy: impl Into<f64>) -> f64 {
        self.moles_per_marker * GAS_CONSTANT * KELVIN_PER_UNIT * reduced_energy.into()
    }

    /// Converts reduced entropy (energy per reduced temperature) to J/K. The
    /// 300 K factors of the energy and temperature scales cancel.
    pub fn entropy_joules_per_kelvin(self, reduced_entropy: f64) -> f64 {
        self.moles_per_marker * GAS_CONSTANT * reduced_entropy
    }

    /// Converts reduced pressure (energy per cubic model unit) to pascals.
    pub fn pressure_pa(self, reduced_pressure: f64) -> f64 {
        self.energy_joules(reduced_pressure) / METERS_PER_UNIT.powi(3)
    }

    /// Readings for a region's particles occupying `volume_m3`.
    pub fn measure(self, sample: KineticMoments, volume_m3: f64) -> GasMeasurements {
        assert!(volume_m3 > 0.0);
        let moles = self.total_moles(sample.count());
        let temperature_kelvin = sample.temperature_kelvin();
        let pressure_pa =
            temperature_kelvin.map(|temperature| moles * GAS_CONSTANT * temperature / volume_m3);
        GasMeasurements {
            markers: sample.count(),
            temperature_kelvin,
            concentration_mol_per_l: moles / (volume_m3 * LITERS_PER_CUBIC_METER),
            pressure_pa,
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/sim/units.rs"]
mod tests;
