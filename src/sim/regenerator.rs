//! The regenerator: a stack of wire screens across the opening between the
//! cylinders that takes heat from gas on its way to the cold side and gives
//! it back to gas on its way to the hot side.
//!
//! Each screen is a plane of fine wire mesh across the opening with its own
//! temperature and heat capacity. Every particle crossing a screen touches
//! a wire and leaves it on the far side at the screen's temperature, like a
//! particle leaving a thermal wall, and the energy it gained or lost comes
//! out of or goes into the screen. Gas from the hot side warms the screens
//! it passes and arrives cooler; gas from the cold side takes that heat
//! back. The gas's trips through the opening no longer carry heat straight
//! from the hot wall to the cold one, so less heat is needed for the same
//! work.
//!
//! The screens trade heat only with the gas, so every joule they hold is
//! booked: the energy the gas gains from them is what they lose.

use macroquad::prelude::vec3;
use macroquad::rand::RandGenerator;

use crate::sim::gas::{
    Particle,
    particle_kinetic_energy,
    sample_gaussian_pair,
    sample_normal_speed, //
};
use crate::sim::geometry::ChamberGeometry;
use crate::sim::units::KELVIN_PER_UNIT;

/// Screens in the stack, spread evenly through the wall between the bores.
///
/// Heat leaking through the stack falls roughly as one over one more than
/// the screen count. At 1,200 and 300 K, 16 screens cut the hot wall's
/// heat per turn to about a fifth for the same work; 24 cut it only a
/// little further and take longer to warm.
pub const SCREEN_COUNT: usize = 16;
/// Heat capacity of one screen for each gas particle, in reduced units
/// (the gas itself holds 3/2 per particle).
///
/// Scaling with the fill keeps the time the screens take to warm or cool
/// the same at every fill, since the gas passing through them scales too.
const HEAT_CAPACITY_PER_PARTICLE: f64 = 0.5;

/// One screen, for drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    /// Horizontal position of the screen's plane.
    pub x: f32,
    /// The screen's temperature.
    pub kelvin: f32,
}

/// The stack of screens: whether it is in the opening, the temperature of
/// each screen, and the heat it has given the gas.
pub struct Regenerator {
    /// Whether the screens are in the opening; when they are not, gas
    /// passes freely and nothing here changes.
    enabled: bool,
    /// Horizontal position of each screen's plane, from the hot side
    /// (negative x) to the cold side.
    positions: [f32; SCREEN_COUNT],
    /// Temperature of each screen, in reduced units, in the same order.
    temperatures: [f64; SCREEN_COUNT],
    /// Heat capacity of one screen, in reduced units.
    heat_capacity: f64,
    /// Heat into the gas from the screens since the fill; negative when
    /// the screens took heat.
    heat: f64,
    /// Random stream for the speeds particles leave the screens with.
    random: RandGenerator,
}

impl Regenerator {
    /// A stack for `particle_count` particles in `geometry`, out of the
    /// opening, with every screen at `temperature` (reduced units) and the
    /// speeds particles leave with drawn from a generator seeded with
    /// `seed`.
    ///
    /// # Panics
    ///
    /// Panics if `particle_count` is zero or `temperature` is not positive
    /// and finite.
    pub fn new(
        geometry: ChamberGeometry,
        particle_count: usize,
        temperature: f64,
        seed: u64,
    ) -> Self {
        assert!(particle_count > 0);
        assert!(temperature.is_finite() && temperature > 0.0);
        let random = RandGenerator::new();
        random.srand(seed);
        let half_wall = geometry.shared_wall_thickness() / 2.0;
        let spacing = 2.0 * half_wall / SCREEN_COUNT as f32;
        Self {
            enabled: false,
            positions: std::array::from_fn(|index| -half_wall + (index as f32 + 0.5) * spacing),
            temperatures: [temperature; SCREEN_COUNT],
            heat_capacity: HEAT_CAPACITY_PER_PARTICLE * particle_count as f64,
            heat: 0.0,
            random,
        }
    }

    /// Whether the screens are in the opening.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Puts the screens in the opening or takes them out.
    ///
    /// Screens put in start in a straight line from `hot` at the hot end to
    /// `cold` at the cold end (reduced units), close to where running gas
    /// keeps them, so they settle within a few turns. Screens taken out
    /// keep their temperatures but no longer touch the gas.
    ///
    /// # Panics
    ///
    /// Panics if `hot` or `cold` is not positive and finite.
    pub fn set_enabled(&mut self, enabled: bool, hot: f64, cold: f64) {
        assert!(hot.is_finite() && hot > 0.0);
        assert!(cold.is_finite() && cold > 0.0);
        if enabled && !self.enabled {
            let last = (SCREEN_COUNT - 1) as f64;
            for (index, temperature) in self.temperatures.iter_mut().enumerate() {
                *temperature = hot + (cold - hot) * index as f64 / last;
            }
        }
        self.enabled = enabled;
    }

    /// Heat the screens have given the gas since the fill.
    pub fn heat(&self) -> f64 {
        self.heat
    }

    /// Each screen's position and temperature, from the hot side to the
    /// cold side.
    pub fn screens(&self) -> impl Iterator<Item = Screen> + '_ {
        self.positions
            .iter()
            .zip(self.temperatures)
            .map(|(&x, temperature)| Screen {
                x,
                kelvin: (temperature * KELVIN_PER_UNIT) as f32,
            })
    }

    /// Lets `particle`, which was at `previous_x` before its last move,
    /// trade heat with each screen it crossed on the way.
    ///
    /// The particle keeps moving the way it was going: each screen sends it
    /// on from the far side at the screen's temperature, with a
    /// flux-weighted Maxwellian speed across the screen and Gaussian speeds
    /// along it. Nothing happens while the screens are out of the opening.
    ///
    /// Returns the heat the screens gave the particle, or `None` if it
    /// crossed none.
    pub fn pass(&mut self, particle: &mut Particle, previous_x: f32) -> Option<f32> {
        let x = particle.position.x;
        if !self.enabled || x == previous_x {
            return None;
        }
        let forward = x > previous_x;
        let (low, high) = if forward {
            (previous_x, x)
        } else {
            (x, previous_x)
        };
        let direction = if forward { 1.0 } else { -1.0 };
        let mut gained = None;
        for step in 0..SCREEN_COUNT {
            let index = if forward {
                step
            } else {
                SCREEN_COUNT - 1 - step
            };
            let position = self.positions[index];
            if position <= low || position > high {
                continue;
            }
            let temperature = self.temperatures[index];
            let before = particle_kinetic_energy(particle.velocity);
            let across = sample_normal_speed(temperature as f32, &self.random);
            let (along_y, along_z) = sample_gaussian_pair(temperature as f32, &self.random);
            particle.velocity = vec3(direction * across, along_y, along_z);
            let heat = particle_kinetic_energy(particle.velocity) - before;
            self.temperatures[index] -= heat as f64 / self.heat_capacity;
            self.heat += heat as f64;
            *gained.get_or_insert(0.0) += heat;
        }
        gained
    }
}

#[cfg(test)]
#[path = "../../tests/unit/sim/regenerator.rs"]
mod tests;
