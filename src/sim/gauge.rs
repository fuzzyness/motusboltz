//! Pressure gauges: the gas's pressure on each piston, measured from the
//! impulse its particles give the face, as a real gauge would.
//!
//! Each gauge averages the impulse over the last `GAUGE_SECONDS` of model
//! time, older hits counting for less and less, and divides it by that
//! time and the face's area. Unlike the ideal-gas law, the reading needs no
//! model of the gas, so it includes what the law leaves out: the particles'
//! own size, and gas still flowing through the opening.

/// Model seconds the gauges average over: every `GAUGE_SECONDS`, older hits
/// count for 1/e as much. Two seconds keeps a sparse fill's reading steady
/// to a few percent while following a turn of about 20 seconds.
pub const GAUGE_SECONDS: f64 = 2.0;
/// Share of `GAUGE_SECONDS` a gauge must have watched before it reads.
const FIRST_READING_SHARE: f64 = 0.5;

/// One piston's gauge.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PressureGauge {
    /// Impulse on the face, each hit weighted by how recent it is.
    impulse: f64,
    /// Model time watched, weighted the same way.
    time: f64,
}

impl PressureGauge {
    /// A gauge that has watched nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `impulse` given to the face over the `dt` seconds just passed.
    pub fn record(&mut self, impulse: f64, dt: f64) {
        let keep = (-dt / GAUGE_SECONDS).exp();
        self.impulse = self.impulse * keep + impulse;
        self.time = self.time * keep + dt;
    }

    /// Mean pressure on a face of `face_area`, in reduced units, once the
    /// gauge has watched long enough to read.
    pub fn pressure(&self, face_area: f64) -> Option<f64> {
        (self.time >= FIRST_READING_SHARE * GAUGE_SECONDS)
            .then(|| self.impulse / (self.time * face_area))
    }
}

#[cfg(test)]
#[path = "../../tests/unit/sim/gauge.rs"]
mod tests;
