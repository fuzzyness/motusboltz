//! Recent heat exchanges at the thermal walls, for drawing heat sparks.
//!
//! Every wall impact that exchanges heat is kept for `SPARK_SECONDS` of
//! model time, with the heat it carried: positive where the particle left
//! the wall faster, negative where it left slower. Next to a hot wall most
//! exchanges speed particles up, next to a cold one most slow them down,
//! and at equal temperatures they balance. Recording changes nothing in the
//! physics.

use std::collections::VecDeque;

use macroquad::prelude::Vec3;

/// Model time each spark is kept, in seconds.
pub const SPARK_SECONDS: f64 = 0.3;
/// Most sparks kept; the oldest go first.
const MAX_SPARKS: usize = 600;

/// One heat exchange at a wall.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeatSpark {
    /// Model time of the impact.
    pub time: f64,
    /// Where the particle's center was as it left the wall.
    pub position: Vec3,
    /// Heat into the gas, in reduced units.
    pub heat: f32,
}

/// The heat exchanges of the last `SPARK_SECONDS`.
pub struct HeatSparks {
    /// The exchanges, oldest first.
    recent: VecDeque<HeatSpark>,
    /// Model time since the fill.
    time: f64,
}

impl HeatSparks {
    /// No sparks yet.
    pub fn new() -> Self {
        Self {
            recent: VecDeque::with_capacity(MAX_SPARKS),
            time: 0.0,
        }
    }

    /// Records `heat` into the gas from an impact at `position`.
    pub fn record(&mut self, position: Vec3, heat: f32) {
        if self.recent.len() == MAX_SPARKS {
            self.recent.pop_front();
        }
        let time = self.time;
        self.recent.push_back(HeatSpark {
            time,
            position,
            heat,
        });
    }

    /// Moves the clock on by `dt` and forgets sparks older than
    /// `SPARK_SECONDS`.
    pub fn advance(&mut self, dt: f64) {
        self.time += dt;
        while self
            .recent
            .front()
            .is_some_and(|spark| self.time - spark.time > SPARK_SECONDS)
        {
            self.recent.pop_front();
        }
    }

    /// Model time since the fill.
    pub fn time(&self) -> f64 {
        self.time
    }

    /// The sparks kept, oldest first.
    pub fn recent(&self) -> impl Iterator<Item = &HeatSpark> {
        self.recent.iter()
    }
}

#[cfg(test)]
#[path = "../../tests/unit/sim/sparks.rs"]
mod tests;
