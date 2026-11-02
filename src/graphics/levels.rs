//! How hot the burner and how cold the nitrogen look, following the walls
//! smoothly.
//!
//! When a wall's temperature moves, the flames, nitrogen, mist and background
//! glows grow or shrink over about a second instead of jumping. The walls
//! already move toward a new setting at a steady pace (see `ui::handles`);
//! the picture eases on top of that, so it never jumps.

use super::fire::heat_level;
use super::nitrogen::cold_level;

/// The time over which the picture covers about two thirds of the way to a
/// new setting.
const EASE_SECONDS: f32 = 0.45;
/// Longest step eased in one frame, so a stalled frame does not jump.
const MAX_STEP_SECONDS: f32 = 0.1;

/// Moves `current` toward `target` over `seconds`.
pub fn ease(current: f32, target: f32, seconds: f32) -> f32 {
    let share = 1.0 - (-seconds.clamp(0.0, MAX_STEP_SECONDS) / EASE_SECONDS).exp();
    current + (target - current) * share
}

/// The levels shown now, from 0 (nothing) to 1 (full).
///
/// See `heat_level` and `cold_level`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReservoirLevels {
    /// How hot the burner looks.
    pub heat: f32,
    /// How cold the nitrogen looks.
    pub cold: f32,
    /// The clock at the last update; `None` before the first, which starts
    /// at the settings as they are.
    seconds: Option<f64>,
}

impl ReservoirLevels {
    /// Follows the reservoirs at `hot_kelvin` and `cold_kelvin`, at
    /// `seconds` on the clock.
    pub fn follow(&mut self, hot_kelvin: f32, cold_kelvin: f32, seconds: f64) {
        let (heat, cold) = (heat_level(hot_kelvin), cold_level(cold_kelvin));
        match self.seconds {
            None => (self.heat, self.cold) = (heat, cold),
            Some(last) => {
                let step = (seconds - last) as f32;
                self.heat = ease(self.heat, heat, step);
                self.cold = ease(self.cold, cold, step);
            }
        }
        self.seconds = Some(seconds);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/levels.rs"]
mod tests;
