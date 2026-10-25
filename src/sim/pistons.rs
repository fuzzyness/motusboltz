//! Crankshaft, flywheel and the two power pistons: crank motion, the
//! impulse gas impacts give the shaft, drag, the constant back pressure on
//! the pistons, and the Start button.

use std::f32::consts::{
    FRAC_PI_2,
    PI, //
};

use crate::sim::gas::DYNAMICS_TIME_SCALE;
use crate::sim::geometry::ChamberGeometry;
use crate::sim::units::{
    DEFAULT_CONNECTED_MARKERS,
    METERS_PER_UNIT, //
};

/// Requested piston travel; `PowerPistons::new` uses less if the geometry
/// cannot fit it with the required passage clearance.
const STROKE_METERS: f64 = 0.048;
/// Minimum gap between a piston at bottom dead center and the top of the
/// passage opening, in particle radii.
const PASSAGE_CLEARANCE_RADII: f32 = 2.0;
/// Flywheel inertia before pacing scaling, in reduced units. Large enough to
/// carry the crank through compression and to keep shaft motion slow compared
/// with gas transport through the passage and heating at the walls.
const REFERENCE_FLYWHEEL_INERTIA: f64 = 16_000.0;
const FLYWHEEL_INERTIA: f64 =
    REFERENCE_FLYWHEEL_INERTIA * (DYNAMICS_TIME_SCALE as f64) * (DYNAMICS_TIME_SCALE as f64);
/// Connecting-rod length divided by crank radius.
const ROD_TO_CRANK_RATIO: f32 = 3.0;
/// Bearing friction torque per unit angular speed, before pacing scaling.
const REFERENCE_BEARING_DRAG: f32 = 0.15;
/// External load (fan-like) torque per unit squared angular speed, before
/// pacing scaling.
const REFERENCE_LOAD_DRAG: f32 = 0.08;
// Scaled so drag torque at a given crank phase is unchanged by the pacing
// scale: the linear term scales once, the quadratic term twice.
const BEARING_DRAG: f32 = REFERENCE_BEARING_DRAG * DYNAMICS_TIME_SCALE;
const LOAD_DRAG: f32 = REFERENCE_LOAD_DRAG * DYNAMICS_TIME_SCALE * DYNAMICS_TIME_SCALE;
/// Flywheel energy Start gives a shaft at rest, at the default fill or
/// below, booked as starter work. At the default fill it carries the crank
/// past the first compression with a wide margin and is close to the
/// heated engine's running speed. The hidden additive kick adds a change in
/// speed worth this much from rest.
const STARTER_ENERGY: f64 = 1_600.0;

/// Flywheel energy Start gives a shaft at rest for `count` particles. The
/// first compression resists in proportion to the amount of gas: from rest
/// at 1200/300 K, the crank needed about 0.9 to 1.3 units per particle to
/// finish its first turn. So above the default fill the energy grows with
/// the particle count (3,200 at 2,048 particles), and below it the default
/// is kept, so a small fill still starts at a watchable speed.
pub fn starting_energy(count: usize) -> f64 {
    STARTER_ENERGY * (count as f64 / DEFAULT_CONNECTED_MARKERS as f64).max(1.0)
}

/// Which piston; the discriminants index per-cylinder arrays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PistonSide {
    Hot = crate::sim::cycle::HOT as isize,
    Cold = crate::sim::cycle::COLD as isize,
}

/// Direction of the additive kick. `Negative` is the heat-engine direction
/// and the only one the UI offers (on a hidden key); `Positive` exists for
/// mechanical tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KickDirection {
    #[cfg(test)]
    Positive,
    Negative,
}

impl KickDirection {
    fn sign(self) -> f32 {
        match self {
            #[cfg(test)]
            Self::Positive => 1.0,
            Self::Negative => -1.0,
        }
    }
}

/// Height of a piston face and its vertical velocity, in model units.
#[derive(Clone, Copy, Debug)]
pub struct PistonState {
    pub face_y: f32,
    pub velocity: f32,
}

/// Crankshaft, flywheel and both pistons. The pistons follow one crank angle
/// a quarter turn apart: turning in the heat-engine direction, the cold
/// piston follows the hot one a quarter turn behind. Only gas impacts,
/// piston-back pressure, drag, Start and kicks change the shaft's speed.
pub struct PowerPistons {
    stroke: f32,
    rod_length: f32,
    shaft_y: f32,
    area: f32,
    angle: f32,
    angle_remainder: f64,
    angular_velocity: f64,
    inertia: f64,
    /// Speed Start brings the shaft up to, in the heat-engine direction.
    start_speed: f64,
    total_angle: f64,
    reference_pressure: f32,
    running: bool,
    hot: PistonState,
    cold: PistonState,
    starter_work: f64,
    back_pressure_work: f64,
    drag_loss: f64,
    gas_impulse: [f64; 2],
}

impl PowerPistons {
    pub fn new(geometry: ChamberGeometry, particle_radius: f32) -> Self {
        let top = geometry.cylinder_height / 2.0;
        let tube_top = -geometry.cylinder_height / 2.0 + geometry.connector_radius;
        let safe_stroke = top - tube_top - PASSAGE_CLEARANCE_RADII * particle_radius;
        assert!(
            safe_stroke > 0.0,
            "connecting passage leaves no piston clearance"
        );
        let stroke = ((STROKE_METERS / METERS_PER_UNIT) as f32).min(safe_stroke);
        let crank_radius = stroke / 2.0;
        let rod_length = ROD_TO_CRANK_RATIO * crank_radius;
        let shaft_y = top + rod_length - crank_radius;
        let cold_y = shaft_y - (rod_length * rod_length - crank_radius * crank_radius).sqrt();
        Self {
            stroke,
            rod_length,
            shaft_y,
            area: PI * geometry.cylinder_radius.powi(2),
            angle: 0.0,
            angle_remainder: 0.0,
            angular_velocity: 0.0,
            inertia: FLYWHEEL_INERTIA,
            start_speed: (2.0 * STARTER_ENERGY / FLYWHEEL_INERTIA).sqrt(),
            total_angle: 0.0,
            reference_pressure: 0.0,
            running: true,
            hot: PistonState {
                face_y: top,
                velocity: 0.0,
            },
            cold: PistonState {
                face_y: cold_y,
                velocity: 0.0,
            },
            starter_work: 0.0,
            back_pressure_work: 0.0,
            drag_loss: 0.0,
            gas_impulse: [0.0; 2],
        }
    }

    /// Sets the constant pressure pushing on the back of both pistons, in
    /// reduced units. The simulation uses the gas's initial pressure, so a
    /// pressurized but unheated gas cannot turn the crank just by pushing
    /// the pistons outward.
    pub fn set_reference_pressure(&mut self, pressure: f32) {
        assert!(pressure.is_finite() && pressure >= 0.0);
        self.reference_pressure = pressure;
    }

    /// Sets the flywheel energy Start gives a shaft at rest.
    pub fn set_starting_energy(&mut self, energy: f64) {
        assert!(energy.is_finite() && energy > 0.0);
        self.start_speed = (2.0 * energy / self.inertia).sqrt();
    }

    /// Height of a piston face when its crank is at `angle`; at angle zero
    /// the face is at top dead center.
    fn position_at(&self, angle: f32) -> f32 {
        let (sin, cos) = angle.sin_cos();
        let crank = self.stroke / 2.0;
        self.shaft_y + crank * cos - (self.rod_length.powi(2) - crank.powi(2) * sin * sin).sqrt()
    }

    /// Rate of change of `position_at` with crank angle.
    fn derivative_at(&self, angle: f32) -> f32 {
        let (sin, cos) = angle.sin_cos();
        let crank = self.stroke / 2.0;
        let rod_vertical = (self.rod_length.powi(2) - crank.powi(2) * sin * sin).sqrt();
        -crank * sin + crank.powi(2) * sin * cos / rod_vertical
    }

    /// Crank angle of each piston relative to the shaft angle.
    fn phase(side: PistonSide) -> f32 {
        match side {
            PistonSide::Hot => 0.0,
            PistonSide::Cold => FRAC_PI_2,
        }
    }

    /// How fast one face moves per radian of shaft rotation now; the lever
    /// arm through which gas impacts turn the shaft.
    fn displacement_derivative(&self, side: PistonSide) -> f32 {
        self.derivative_at(self.angle + Self::phase(side))
    }

    /// Moves both faces to the current crank angle; velocity is the change
    /// over `dt`.
    fn update_faces(&mut self, dt: f32) {
        let hot_y = self.position_at(self.angle + Self::phase(PistonSide::Hot));
        let cold_y = self.position_at(self.angle + Self::phase(PistonSide::Cold));
        self.hot.velocity = (hot_y - self.hot.face_y) / dt;
        self.cold.velocity = (cold_y - self.cold.face_y) / dt;
        self.hot.face_y = hot_y;
        self.cold.face_y = cold_y;
    }

    /// Advances the crank by `dt`: applies bearing and load drag, then the
    /// piston-back pressure torque (half before and half after rotating),
    /// then moves the faces. Drag losses and back-pressure work are booked
    /// from the actual energy and volume changes. A held shaft does nothing.
    pub fn advance(&mut self, dt: f32) {
        assert!(dt.is_finite() && dt > 0.0);
        if !self.running {
            self.hot.velocity = 0.0;
            self.cold.velocity = 0.0;
            return;
        }
        let old_volume =
            self.area * (self.position_at(self.angle) + self.position_at(self.angle + FRAC_PI_2));
        let before = self.kinetic_energy();
        let speed = self.angular_velocity;
        let drag = BEARING_DRAG as f64 * speed + LOAD_DRAG as f64 * speed * speed.abs();
        let next = speed - drag * dt as f64 / self.inertia;
        self.angular_velocity = if speed.signum() == next.signum() {
            next
        } else {
            0.0
        };
        // Drag stops the shaft rather than reversing it within one step.
        self.drag_loss += (before - self.kinetic_energy()).max(0.0);

        self.angular_velocity +=
            self.back_pressure_torque() as f64 * dt as f64 / (2.0 * self.inertia);
        let rotation = self.angular_velocity * dt as f64;
        let next_angle =
            (self.angle as f64 + self.angle_remainder + rotation).rem_euclid(std::f64::consts::TAU);
        self.angle = next_angle as f32;
        self.angle_remainder = next_angle - self.angle as f64;
        self.total_angle += rotation;
        self.angular_velocity +=
            self.back_pressure_torque() as f64 * dt as f64 / (2.0 * self.inertia);
        self.update_faces(dt);
        let new_volume =
            self.area * (self.position_at(self.angle) + self.position_at(self.angle + FRAC_PI_2));
        // Booked from the displaced volume, independently of the torque above,
        // so the shaft energy balance can check one against the other.
        self.back_pressure_work -=
            self.reference_pressure as f64 * (new_volume as f64 - old_volume as f64);
    }

    /// Torque from the constant back pressure on both pistons.
    fn back_pressure_torque(&self) -> f32 {
        -self.reference_pressure
            * self.area
            * (self.displacement_derivative(PistonSide::Hot)
                + self.displacement_derivative(PistonSide::Cold))
    }

    /// Returns a particle's new upward velocity after hitting a piston face,
    /// and applies the reaction impulse to the shaft. The collision is
    /// elastic between the particle and the shaft's effective mass at the
    /// current crank angle. Particles moving away from the face pass
    /// through unchanged; a held piston acts as a stationary wall.
    pub fn collide(&mut self, side: PistonSide, upward_speed: f32, particle_mass: f32) -> f32 {
        let wall_speed = self.face_velocity(side);
        let relative_speed = upward_speed - wall_speed;
        if relative_speed <= 0.0 {
            return upward_speed;
        }
        let inverse_mass = if self.running {
            (self.displacement_derivative(side) as f64).powi(2) / self.inertia
        } else {
            0.0
        };
        let impulse = 2.0 * relative_speed as f64 / (1.0 / particle_mass as f64 + inverse_mass);
        let new_speed = upward_speed as f64 - impulse / particle_mass as f64;
        if inverse_mass > 0.0 {
            self.angular_velocity +=
                impulse * self.displacement_derivative(side) as f64 / self.inertia;
            self.gas_impulse[side as usize] += impulse;
        }
        new_speed as f32
    }

    /// Vertical velocity of one piston face; zero while the shaft is held.
    pub fn face_velocity(&self, side: PistonSide) -> f32 {
        if self.running {
            (self.displacement_derivative(side) as f64 * self.angular_velocity) as f32
        } else {
            0.0
        }
    }

    /// Brings the shaft up to the starting speed in the heat-engine
    /// direction, like a starter motor that lets go at once: a resting, slow
    /// or backward-turning shaft is set to that speed, and a shaft already
    /// turning at least that fast is left alone, so repeated presses cannot
    /// overdrive the engine. The change in flywheel energy is booked as
    /// starter work, apart from gas work. Returns whether the speed changed.
    pub fn start(&mut self) -> bool {
        let target = KickDirection::Negative.sign() as f64 * self.start_speed;
        if self.angular_velocity <= target {
            return false;
        }
        let before = self.kinetic_energy();
        self.angular_velocity = target;
        self.starter_work += self.kinetic_energy() - before;
        true
    }

    /// Adds a fixed change in angular speed, not a sustained motor, and can
    /// be repeated to overdrive the engine. The resulting change in flywheel
    /// energy is booked as starter work, apart from gas work. From rest this
    /// adds `STARTER_ENERGY`.
    pub fn kick(&mut self, direction: KickDirection) {
        let kick_speed = (2.0 * STARTER_ENERGY / self.inertia).sqrt() as f32;
        let before = self.kinetic_energy();
        self.angular_velocity += (direction.sign() * kick_speed) as f64;
        let work = self.kinetic_energy() - before;
        self.starter_work += work;
    }

    pub fn hot(&self) -> PistonState {
        self.hot
    }
    pub fn cold(&self) -> PistonState {
        self.cold
    }
    pub fn angle(&self) -> f32 {
        self.angle
    }
    /// Signed angular speed in radians per second of model time; zero while
    /// held.
    pub fn angular_velocity(&self) -> f32 {
        if !self.running {
            0.0
        } else {
            self.angular_velocity as f32
        }
    }
    /// Signed crank rotation since the fill, in turns.
    pub fn revolutions(&self) -> f64 {
        self.total_angle / std::f64::consts::TAU
    }
    /// Signed crank rotation since the fill, in radians.
    pub fn total_angle(&self) -> f64 {
        self.total_angle
    }
    /// Area of one piston face, in square model units.
    pub fn face_area(&self) -> f32 {
        self.area
    }
    /// Returns the upward impulse the gas has given each moving piston face
    /// (indexed hot, cold) since the last call, and resets it.
    pub fn take_gas_impulse(&mut self) -> [f64; 2] {
        std::mem::take(&mut self.gas_impulse)
    }
    pub fn stroke(&self) -> f32 {
        self.stroke
    }
    pub fn shaft_height(&self) -> f32 {
        self.shaft_y
    }
    pub fn running(&self) -> bool {
        self.running
    }
    pub fn set_running(&mut self, running: bool) {
        self.running = running;
        if !running {
            self.hot.velocity = 0.0;
            self.cold.velocity = 0.0;
        }
    }
    pub fn kinetic_energy(&self) -> f64 {
        0.5 * self.inertia * self.angular_velocity.powi(2)
    }
    /// Flywheel energy added by Start and kicks since the fill.
    pub fn starter_work(&self) -> f64 {
        self.starter_work
    }
    /// Work done on the shaft by the constant back pressure since the fill.
    pub fn back_pressure_work(&self) -> f64 {
        self.back_pressure_work
    }
    /// Total flywheel energy removed by bearing friction and the external
    /// load since the last refill.
    pub fn drag_loss(&self) -> f64 {
        self.drag_loss
    }

    /// Gas volume below both piston faces plus the passage.
    pub fn volume_m3(&self, geometry: ChamberGeometry) -> f64 {
        self.cylinder_volume_m3(geometry, self.hot)
            + self.cylinder_volume_m3(geometry, self.cold)
            + geometry.connector_volume_m3()
    }

    /// Gas volume between one cylinder's floor and the given piston face.
    pub fn cylinder_volume_m3(&self, geometry: ChamberGeometry, piston: PistonState) -> f64 {
        let radius_m = geometry.cylinder_radius as f64 * METERS_PER_UNIT;
        let height_m = (piston.face_y + geometry.cylinder_height / 2.0) as f64 * METERS_PER_UNIT;
        std::f64::consts::PI * radius_m * radius_m * height_m
    }
}

#[cfg(test)]
#[path = "../../tests/unit/sim/pistons.rs"]
mod tests;
