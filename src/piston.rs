//! Prescribed piston motion for controlled compression and expansion.

#[cfg(test)]
use std::f32::consts::PI;

use crate::simulation::CHAMBER_HEIGHT;
#[cfg(test)]
use crate::simulation::CHAMBER_RADIUS;

/// Minimum chamber height as a fraction of the fully extended volume.
pub const MIN_VOLUME_FRACTION: f32 = 0.5;
const MAX_VOLUME_FRACTION: f32 = 1.0;
/// Maximum piston travel speed in reduced distance units per second.
const MAX_PISTON_SPEED: f32 = 0.1;

/// Manual-mode piston that moves toward a selected target volume.
///
/// Its actual velocity is measured each physics step for collisions with the
/// moving boundary. Crankshaft mode uses a separate constrained mechanism.
pub struct Piston {
    position: f32,
    target: f32,
    velocity: f32,
}

impl Piston {
    /// Creates a piston at the cylinder's fully extended upper cap.
    pub fn new() -> Self {
        let top = CHAMBER_HEIGHT / 2.0;
        Self {
            position: top,
            target: top,
            velocity: 0.0,
        }
    }

    fn bottom() -> f32 {
        -CHAMBER_HEIGHT / 2.0
    }

    /// Selects a volume fraction, with full extension equal to one.
    pub fn set_volume_fraction(&mut self, fraction: f32) {
        assert!(fraction.is_finite());
        assert!((MIN_VOLUME_FRACTION..=MAX_VOLUME_FRACTION).contains(&fraction));
        self.target = Self::bottom() + CHAMBER_HEIGHT * fraction;
    }

    /// Moves toward the requested height and returns actual piston velocity.
    pub fn advance(&mut self, dt: f32) -> f32 {
        assert!(dt.is_finite() && dt > 0.0);
        let travel =
            (self.target - self.position).clamp(-MAX_PISTON_SPEED * dt, MAX_PISTON_SPEED * dt);
        self.position += travel;
        self.velocity = travel / dt;
        self.velocity
    }

    /// Stops a motorized piston at the current crank-driven height.
    pub fn park_at(&mut self, position: f32) {
        self.position = position;
        self.target = position;
        self.velocity = 0.0;
    }

    /// World-space Y position of the piston face.
    pub fn position(&self) -> f32 {
        self.position
    }

    /// Positive means expansion; negative means compression.
    pub fn velocity(&self) -> f32 {
        self.velocity
    }

    /// Distance between the bottom of the cylinder and the piston.
    #[cfg(test)]
    pub fn height(&self) -> f32 {
        self.position - Self::bottom()
    }

    /// Volume currently occupied by the gas.
    #[cfg(test)]
    pub fn volume(&self) -> f32 {
        PI * CHAMBER_RADIUS * CHAMBER_RADIUS * self.height()
    }

    /// User-selected volume fraction, which may still be in transit.
    pub fn target_volume_fraction(&self) -> f32 {
        (self.target - Self::bottom()) / CHAMBER_HEIGHT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_FRACTION: f32 = 0.75;
    const TEST_TIMESTEP: f32 = 0.1;
    const POSITION_TOLERANCE: f32 = 1e-5;
    const VELOCITY_TOLERANCE: f32 = 1e-6;

    #[test]
    fn piston_starts_at_full_volume() {
        let piston = Piston::new();
        assert!((piston.height() - CHAMBER_HEIGHT).abs() < POSITION_TOLERANCE);
        assert_eq!(piston.velocity(), 0.0);
    }

    #[test]
    fn piston_moves_at_bounded_speed_and_stops_at_target() {
        let mut piston = Piston::new();
        piston.set_volume_fraction(TEST_FRACTION);
        let old_position = piston.position();
        let velocity = piston.advance(TEST_TIMESTEP);
        assert!(velocity < 0.0);
        assert!(velocity.abs() <= MAX_PISTON_SPEED + VELOCITY_TOLERANCE);
        assert!(
            (piston.position() - old_position - velocity * TEST_TIMESTEP).abs()
                < POSITION_TOLERANCE
        );

        while piston.position() > piston.target {
            piston.advance(TEST_TIMESTEP);
        }

        assert!((piston.height() - CHAMBER_HEIGHT * TEST_FRACTION).abs() < POSITION_TOLERANCE);
        assert_eq!(piston.advance(TEST_TIMESTEP), 0.0);
    }

    #[test]
    fn piston_volume_matches_cylinder_cross_section() {
        let mut piston = Piston::new();
        piston.set_volume_fraction(MIN_VOLUME_FRACTION);
        while piston.position() > piston.target {
            piston.advance(TEST_TIMESTEP);
        }
        assert!(
            (piston.volume() - PI * CHAMBER_RADIUS.powi(2) * CHAMBER_HEIGHT * MIN_VOLUME_FRACTION)
                .abs()
                < POSITION_TOLERANCE
        );
    }
}
