//! Slider-crank geometry, gas-driven flywheel, and a temporary starter motor.

use std::f32::consts::{PI, TAU};

use macroquad::prelude::{Vec3, vec3};

use crate::piston::MIN_VOLUME_FRACTION;
use crate::simulation::CHAMBER_HEIGHT;

/// Half the distance traveled by the piston between its stroke endpoints.
pub const CRANK_RADIUS: f32 = CHAMBER_HEIGHT * (1.0 - MIN_VOLUME_FRACTION) / 2.0;
/// A rod longer than the crank radius avoids a singular slider-crank geometry.
const CONNECTING_ROD_LENGTH: f32 = 2.0 * CRANK_RADIUS;
/// The horizontal crankshaft sits above the cylinder's original upper rim.
const SHAFT_HEIGHT: f32 = CHAMBER_HEIGHT / 2.0 - CRANK_RADIUS + CONNECTING_ROD_LENGTH;
/// Flywheel moment of inertia in reduced simulation units.
const FLYWHEEL_INERTIA: f32 = 150.0;
/// Speed maintained by the electric starter during one complete revolution.
const STARTER_ANGULAR_SPEED: f32 = 1.2;
/// One motor-driven revolution supplies a full compression-expansion stroke.
const STARTER_DRIVEN_ANGLE: f32 = TAU;
/// Viscous bearing resistance, in torque per angular velocity.
const BEARING_DRAG: f32 = 0.25;
/// Quadratic resisting torque standing in for a future fan load.
const FAN_DRAG: f32 = 1.0;
const ANGLE_SOLVER_STEPS: usize = 32;

/// A constrained piston and flywheel with one rotational degree of freedom.
///
/// The connecting rod constrains piston height to crank angle. Top-wall
/// collisions transfer momentum to the crank through dy/d(angle). The flywheel
/// stores the resulting energy; drag represents energy delivered to a load.
/// A temporary external motor supplies the work needed to start the cycle.
#[derive(Clone)]
pub struct Crankshaft {
    angle: f32,
    angular_velocity: f32,
    load_energy: f32,
    starter_work: f32,
    starter_angle_remaining: f32,
}

impl Crankshaft {
    /// Attaches to an existing piston height without teleporting the gas wall.
    pub fn at_height(height: f32) -> Self {
        let mut lower_angle = 0.0;
        let mut upper_angle = PI;

        // This half-turn maps monotonically from full to half volume.
        for _ in 0..ANGLE_SOLVER_STEPS {
            let middle = (lower_angle + upper_angle) / 2.0;
            if Self::height_at(middle) > height {
                lower_angle = middle;
            } else {
                upper_angle = middle;
            }
        }

        Self {
            angle: (lower_angle + upper_angle) / 2.0,
            angular_velocity: 0.0,
            load_energy: 0.0,
            starter_work: 0.0,
            starter_angle_remaining: 0.0,
        }
    }

    fn height_at(angle: f32) -> f32 {
        let (sin, cos) = angle.sin_cos();
        SHAFT_HEIGHT + CRANK_RADIUS * cos
            - (CONNECTING_ROD_LENGTH.powi(2) - CRANK_RADIUS.powi(2) * sin * sin).sqrt()
    }

    /// Mechanical advantage dy/d(angle); zero at either dead center.
    fn displacement_derivative(&self) -> f32 {
        let (sin, cos) = self.angle.sin_cos();
        let rod_vertical =
            (CONNECTING_ROD_LENGTH.powi(2) - CRANK_RADIUS.powi(2) * sin * sin).sqrt();
        -CRANK_RADIUS * sin + CRANK_RADIUS.powi(2) * sin * cos / rod_vertical
    }

    /// Engages the starter motor for one complete revolution.
    ///
    /// The motor enforces a fixed angular speed during its stroke and its
    /// energy input is tracked separately from work delivered by the gas.
    /// Without ignition, continued rotation after the stroke is not guaranteed.
    pub fn start(&mut self) {
        let energy_before = self.kinetic_energy();
        self.angular_velocity = STARTER_ANGULAR_SPEED;
        self.starter_work += self.kinetic_energy() - energy_before;
        self.starter_angle_remaining = STARTER_DRIVEN_ANGLE;
    }

    /// Advances the flywheel under drag and any active starter-motor drive.
    pub fn advance(&mut self, dt: f32) {
        let previous_energy = self.kinetic_energy();
        let speed = self.angular_velocity;
        let opposing_torque = BEARING_DRAG * speed + FAN_DRAG * speed * speed.abs();
        let new_speed = speed - opposing_torque * dt / FLYWHEEL_INERTIA;
        // Do not let the load numerically reverse the direction of rotation.
        self.angular_velocity = if speed.signum() == new_speed.signum() {
            new_speed
        } else {
            0.0
        };
        self.load_energy += (previous_energy - self.kinetic_energy()).max(0.0);

        if self.starter_angle_remaining > 0.0 {
            // The starter replaces energy lost to the gas and the load.
            // This is external motor work, never energy created by the gas.
            let energy_before_motor = self.kinetic_energy();
            self.angular_velocity = STARTER_ANGULAR_SPEED;
            self.starter_work += self.kinetic_energy() - energy_before_motor;
            self.starter_angle_remaining =
                (self.starter_angle_remaining - STARTER_ANGULAR_SPEED * dt).max(0.0);
        }

        self.angle = (self.angle + self.angular_velocity * dt).rem_euclid(TAU);
    }

    /// Inverse effective piston mass, measured along its vertical path.
    pub fn top_inverse_mass(&self) -> f32 {
        self.displacement_derivative().powi(2) / FLYWHEEL_INERTIA
    }

    /// Reacts to upward impulse from a gas molecule and returns shaft energy gained.
    pub fn apply_top_impulse(&mut self, impulse: f32) -> f32 {
        let energy_before = self.kinetic_energy();
        self.angular_velocity += impulse * self.displacement_derivative() / FLYWHEEL_INERTIA;
        self.kinetic_energy() - energy_before
    }

    pub fn piston_position(&self) -> f32 {
        Self::height_at(self.angle)
    }

    pub fn piston_velocity(&self) -> f32 {
        self.displacement_derivative() * self.angular_velocity
    }

    pub fn angular_velocity(&self) -> f32 {
        self.angular_velocity
    }

    pub fn kinetic_energy(&self) -> f32 {
        0.5 * FLYWHEEL_INERTIA * self.angular_velocity.powi(2)
    }

    pub fn load_energy(&self) -> f32 {
        self.load_energy
    }

    /// Signed energy supplied by the external starter motor.
    pub fn starter_work(&self) -> f32 {
        self.starter_work
    }

    /// Whether the starter is still driving the flywheel.
    #[cfg(test)]
    pub fn starter_active(&self) -> bool {
        self.starter_angle_remaining > 0.0
    }

    pub fn shaft_center(&self) -> Vec3 {
        vec3(0.0, SHAFT_HEIGHT, 0.0)
    }

    pub fn crank_pin(&self) -> Vec3 {
        let (sin, cos) = self.angle.sin_cos();
        vec3(CRANK_RADIUS * sin, SHAFT_HEIGHT + CRANK_RADIUS * cos, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POSITION_TOLERANCE: f32 = 1e-5;
    const ENERGY_TOLERANCE: f32 = 1e-4;
    const TEST_HEIGHT_FRACTION: f32 = 0.75;
    const TEST_STEP: f32 = 1.0 / 120.0;
    const TEST_TOP_IMPULSE: f32 = 2.0;

    #[test]
    fn geometry_matches_existing_piston_volume_range() {
        let top = Crankshaft::at_height(CHAMBER_HEIGHT / 2.0);
        let bottom = Crankshaft::at_height(0.0);
        let intermediate = Crankshaft::at_height(CHAMBER_HEIGHT * (TEST_HEIGHT_FRACTION - 0.5));
        assert!((top.piston_position() - CHAMBER_HEIGHT / 2.0).abs() < POSITION_TOLERANCE);
        assert!(bottom.piston_position().abs() < POSITION_TOLERANCE);
        assert!(
            (intermediate.piston_position() - CHAMBER_HEIGHT * (TEST_HEIGHT_FRACTION - 0.5)).abs()
                < POSITION_TOLERANCE
        );
    }

    #[test]
    fn starter_adds_energy_and_load_dissipates_it() {
        let mut crank = Crankshaft::at_height(CHAMBER_HEIGHT / 2.0);
        assert_eq!(crank.kinetic_energy(), 0.0);
        crank.start();
        let initial = crank.kinetic_energy();
        assert!(crank.starter_active());
        crank.advance(TEST_STEP);
        assert!((crank.kinetic_energy() - initial).abs() < ENERGY_TOLERANCE);
        assert!(crank.load_energy() > 0.0);
        assert!(crank.starter_work() > initial);
        assert!(
            (crank.kinetic_energy() + crank.load_energy() - crank.starter_work()).abs()
                < ENERGY_TOLERANCE
        );
    }

    #[test]
    fn starter_completes_a_revolution_with_gas_impulses() {
        const GAS_IMPULSE: f32 = 20.0;
        const MAX_STARTER_STEPS: usize = 1_000;
        const ANGLE_TOLERANCE: f32 = 2.0 * STARTER_ANGULAR_SPEED * TEST_STEP;
        const RELATIVE_ENERGY_TOLERANCE: f32 = 1e-5;

        let mut crank = Crankshaft::at_height(CHAMBER_HEIGHT / 2.0);
        crank.start();

        let mut steps = 0;
        let mut lowest_position = crank.piston_position();
        let mut gas_work = 0.0;
        while crank.starter_active() && steps < MAX_STARTER_STEPS {
            crank.advance(TEST_STEP);
            gas_work += crank.apply_top_impulse(GAS_IMPULSE);
            lowest_position = lowest_position.min(crank.piston_position());
            steps += 1;
        }

        assert!(!crank.starter_active());
        assert!(lowest_position <= POSITION_TOLERANCE);
        assert!(crank.angle.min(TAU - crank.angle) <= ANGLE_TOLERANCE);

        // Gas impulses oppose compression but assist expansion. Net starter
        // work can be negative because a speed-regulating motor also brakes.
        let stored_and_dissipated = crank.kinetic_energy() + crank.load_energy();
        let supplied = crank.starter_work() + gas_work;
        let tolerance = ENERGY_TOLERANCE
            + RELATIVE_ENERGY_TOLERANCE
                * (stored_and_dissipated + crank.starter_work().abs() + gas_work.abs());
        assert!(
            (stored_and_dissipated - supplied).abs() < tolerance,
            "crank energy balance: stored + load = {stored_and_dissipated}, starter + gas = {supplied}"
        );
    }

    #[test]
    fn impulse_changes_angular_momentum_by_crank_leverage() {
        let mut crank = Crankshaft::at_height(CHAMBER_HEIGHT * (TEST_HEIGHT_FRACTION - 0.5));
        let leverage = crank.displacement_derivative();
        let gained_energy = crank.apply_top_impulse(TEST_TOP_IMPULSE);
        assert!(
            (crank.angular_velocity() - TEST_TOP_IMPULSE * leverage / FLYWHEEL_INERTIA).abs()
                < ENERGY_TOLERANCE
        );
        assert!((gained_energy - crank.kinetic_energy()).abs() < ENERGY_TOLERANCE);
    }
}
