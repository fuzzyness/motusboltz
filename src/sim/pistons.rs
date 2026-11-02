//! Crankshaft, flywheel and the two power pistons: crank motion, the
//! impulse gas impacts give the shaft, drag, static friction, the constant
//! back pressure on the pistons, and the Start button.

use std::f32::consts::{
    FRAC_PI_2,
    PI, //
};

use crate::sim::cycle::{
    COLD,
    HOT, //
};
use crate::sim::gas::DYNAMICS_TIME_SCALE;
use crate::sim::geometry::ChamberGeometry;
use crate::sim::units::{
    DEFAULT_PARTICLES,
    METERS_PER_UNIT, //
};

/// Requested piston travel; `PowerPistons::new` uses less if the geometry
/// cannot fit it above the opening between the cylinders.
const STROKE_METERS: f64 = 0.048;
/// Least gap between a piston at bottom dead center and the top of the
/// opening, in particle radii, so the pistons never cover it.
const PASSAGE_CLEARANCE_RADII: f32 = 2.0;
/// Flywheel inertia before the time scale.
///
/// It is light enough that Start brings the engine straight to its running
/// speed (four times lighter than before v0.6, when a heated engine took
/// about 10 minutes to stop speeding up), and heavy enough to carry the
/// crank through compression: at 1200/100 K the speed varies about 6%
/// within a turn.
const REFERENCE_FLYWHEEL_INERTIA: f64 = 4_000.0;
/// Flywheel inertia, scaled with the square of the time scale like the
/// particle mass.
const FLYWHEEL_INERTIA: f64 =
    REFERENCE_FLYWHEEL_INERTIA * DYNAMICS_TIME_SCALE as f64 * DYNAMICS_TIME_SCALE as f64;
/// Connecting-rod length over crank radius.
const ROD_TO_CRANK_RATIO: f32 = 3.0;
/// Bearing friction torque per unit angular speed, before the time scale.
const REFERENCE_BEARING_DRAG: f32 = 0.15;
/// External load (fan-like) torque per unit squared angular speed, before
/// the time scale; its work is the engine's useful output.
///
/// Measured with 1,024 particles at 1200/100 K, this load takes about 95% of
/// the most power the engine can give (a third of it turned 1.25 times
/// faster but took only 65%) and holds efficiency near 1.3%. With a load 300
/// times lighter, as before v0.6, the engine sped up until friction alone
/// used its work, and efficiency faded to about 0.3%.
const REFERENCE_LOAD_DRAG: f32 = 24.0;
/// Bearing drag, scaled once with the time scale so drag torque at a given
/// crank phase is unchanged by it.
const BEARING_DRAG: f32 = REFERENCE_BEARING_DRAG * DYNAMICS_TIME_SCALE;
/// Load drag, scaled twice with the time scale for the same reason.
const LOAD_DRAG: f32 = REFERENCE_LOAD_DRAG * DYNAMICS_TIME_SCALE * DYNAMICS_TIME_SCALE;
/// Static friction holding a shaft at rest, as a share of `torque_scale`,
/// so it holds alike at every fill.
///
/// Without it the unheated gas acts as a spring on the crank and swings a
/// coasting wheel back and forth for minutes. Measured at 300/300 K, the gas
/// pushed with 0.4 to 1.25 of that torque where a swing turned around, so
/// 1.5 holds the wheel at its first turnaround. A heated engine runs on
/// about 0.1 to 0.4 of it on average and never slows to zero, so the
/// friction leaves it alone.
const STATIC_FRICTION_SHARE: f64 = 1.5;
/// Model time over which the gas's push on a held shaft is averaged before
/// it is weighed against static friction, so single impacts do not count.
const HOLD_WINDOW: f64 = 0.5;
/// Flywheel energy Start gives a shaft at rest at the default fill or below,
/// and the energy of the hidden kick from rest.
///
/// At the default fill it carries the crank past the first compression with
/// a wide margin and brings it close to the heated engine's running speed
/// (about 7 s per turn per unit of time scale from Start, settling near 6.5
/// at 1200/100 K).
const STARTER_ENERGY: f64 = 1_600.0;
/// Sign of the speed when the shaft turns in the heat-engine direction.
pub const FORWARD: f64 = -1.0;

/// Flywheel energy Start gives a shaft at rest for `count` particles.
///
/// The first compression resists in proportion to the amount of gas: from
/// rest at 1200/300 K, the crank needed about 0.9 to 1.3 units per particle
/// to finish its first turn (measured before v0.6; the `one_start`
/// regression rechecks every fill). So above the default fill the energy
/// grows with the particle count, and below it the default is kept, so a
/// small fill still starts at a watchable speed.
pub fn starting_energy(count: usize) -> f64 {
    STARTER_ENERGY * (count as f64 / DEFAULT_PARTICLES as f64).max(1.0)
}

/// Which piston; the discriminants index per-cylinder arrays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PistonSide {
    /// The heated cylinder's piston.
    Hot = HOT as isize,
    /// The cooled cylinder's piston.
    Cold = COLD as isize,
}

impl PistonSide {
    /// Crank angle of this piston relative to the shaft angle: the cold
    /// piston follows the hot one a quarter turn behind.
    fn phase(self) -> f32 {
        match self {
            Self::Hot => 0.0,
            Self::Cold => FRAC_PI_2,
        }
    }
}

/// Height of a piston face and its vertical velocity, in model units.
#[derive(Clone, Copy, Debug)]
pub struct PistonState {
    /// Height of the face.
    pub face_y: f32,
    /// Vertical velocity of the face.
    pub velocity: f32,
}

/// Crankshaft, flywheel and both pistons.
///
/// The pistons follow one crank angle a quarter turn apart. Only gas
/// impacts, the back pressure, drag, Start and the kick change the shaft's
/// speed.
///
/// A shaft that comes to rest, or turns around, is held by static friction:
/// the pistons stand still as walls, and the shaft moves again only when
/// the gas and back pressure together push harder than the friction holds,
/// or Start or the kick turns it.
pub struct PowerPistons {
    /// Piston travel.
    stroke: f32,
    /// Connecting-rod length.
    rod_length: f32,
    /// Height of the crankshaft.
    shaft_y: f32,
    /// Area of one piston face.
    area: f32,
    /// Crank angle reduced to 0..2 pi.
    angle: f32,
    /// What `angle` rounded away, kept so the angle does not drift.
    angle_remainder: f64,
    /// Signed angular speed, in radians per second of model time.
    angular_velocity: f64,
    /// Speed Start brings the shaft up to.
    start_speed: f64,
    /// Signed crank rotation since the fill.
    total_angle: f64,
    /// Pressure on the back of both pistons, in reduced units.
    reference_pressure: f32,
    /// Held by static friction.
    held: bool,
    /// While held: the angular impulse the gas has given the shaft since the
    /// last weighing.
    held_impulse: f64,
    /// While held: the model time `held_impulse` was gathered over.
    held_time: f64,
    /// Sign of the speed when the last step began, to catch a turnaround.
    direction: f64,
    /// The hot piston.
    hot: PistonState,
    /// The cold piston.
    cold: PistonState,
    /// Flywheel energy added by Start and the kick since the fill.
    starter_work: f64,
    /// Work the back pressure has done on the shaft since the fill.
    back_pressure_work: f64,
    /// Flywheel energy taken by drag and friction since the fill.
    drag_loss: f64,
    /// The part of `drag_loss` taken by the load.
    load_work: f64,
    /// Upward impulse the gas has given each face since the last
    /// `take_gas_impulse`.
    gas_impulse: [f64; 2],
}

impl PowerPistons {
    /// Pistons at rest, the hot one at top dead center, in `geometry` for
    /// particles of `particle_radius`.
    ///
    /// # Panics
    ///
    /// Panics if the opening leaves no room for the pistons' travel.
    pub fn new(geometry: ChamberGeometry, particle_radius: f32) -> Self {
        let top = geometry.cylinder_height / 2.0;
        let room = top - geometry.passage_top() - PASSAGE_CLEARANCE_RADII * particle_radius;
        assert!(room > 0.0, "the opening leaves no room for the pistons");
        let stroke = ((STROKE_METERS / METERS_PER_UNIT) as f32).min(room);
        let crank = stroke / 2.0;
        let rod_length = ROD_TO_CRANK_RATIO * crank;
        let shaft_y = top + rod_length - crank;
        let cold_y = shaft_y - (rod_length * rod_length - crank * crank).sqrt();
        Self {
            stroke,
            rod_length,
            shaft_y,
            area: PI * geometry.cylinder_radius.powi(2),
            angle: 0.0,
            angle_remainder: 0.0,
            angular_velocity: 0.0,
            start_speed: (2.0 * STARTER_ENERGY / FLYWHEEL_INERTIA).sqrt(),
            total_angle: 0.0,
            reference_pressure: 0.0,
            held: true,
            held_impulse: 0.0,
            held_time: 0.0,
            direction: 0.0,
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
            load_work: 0.0,
            gas_impulse: [0.0; 2],
        }
    }

    /// Sets the constant pressure on the back of both pistons, in reduced
    /// units.
    ///
    /// The simulation uses the gas's initial pressure, so a pressurized but
    /// unheated gas cannot turn the crank just by pushing the pistons out.
    ///
    /// # Panics
    ///
    /// Panics if `pressure` is negative or not finite.
    pub fn set_reference_pressure(&mut self, pressure: f32) {
        assert!(pressure.is_finite() && pressure >= 0.0);
        self.reference_pressure = pressure;
    }

    /// Sets the flywheel energy Start gives a shaft at rest.
    ///
    /// # Panics
    ///
    /// Panics if `energy` is not positive and finite.
    pub fn set_starting_energy(&mut self, energy: f64) {
        assert!(energy.is_finite() && energy > 0.0);
        self.start_speed = (2.0 * energy / FLYWHEEL_INERTIA).sqrt();
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

    /// Height of the face on `side` at the current crank angle.
    fn face_height(&self, side: PistonSide) -> f32 {
        self.position_at(self.angle + side.phase())
    }

    /// How far the face on `side` moves per radian of shaft rotation now:
    /// the lever through which gas impacts turn the shaft.
    fn lever(&self, side: PistonSide) -> f32 {
        self.derivative_at(self.angle + side.phase())
    }

    /// Gas volume below both faces measured from height zero, in cubic
    /// model units; only its changes matter.
    fn swept_volume(&self) -> f64 {
        (self.area * (self.face_height(PistonSide::Hot) + self.face_height(PistonSide::Cold)))
            as f64
    }

    /// Moves both faces to the current crank angle; their velocity is the
    /// change over `dt`.
    fn update_faces(&mut self, dt: f32) {
        let heights = [PistonSide::Hot, PistonSide::Cold].map(|side| self.face_height(side));
        for (state, face_y) in [&mut self.hot, &mut self.cold].into_iter().zip(heights) {
            state.velocity = (face_y - state.face_y) / dt;
            state.face_y = face_y;
        }
    }

    /// Advances the crank by `dt`.
    ///
    /// Bearing and load drag act first, then the back-pressure torque (half
    /// before and half after rotating), then the faces move. Drag losses and
    /// back-pressure work are booked from the actual energy and volume
    /// changes. A shaft that stopped or turned around since the last step is
    /// held by static friction first, and a held shaft stays put until the
    /// push on it outweighs the friction.
    ///
    /// # Panics
    ///
    /// Panics if `dt` is not positive and finite.
    pub fn advance(&mut self, dt: f32) {
        assert!(dt.is_finite() && dt > 0.0);
        // Gas impacts since the last step are in the speed, so a turnaround
        // shows as a change of sign against the last step's.
        let direction = sign(self.angular_velocity);
        if !self.held && self.direction != 0.0 && direction != self.direction {
            self.hold();
        }
        if self.held {
            if !self.release(dt as f64) {
                return;
            }
            // Just let go: the next step sets the direction.
            self.direction = 0.0;
        } else {
            self.direction = direction;
        }

        let old_volume = self.swept_volume();
        let before = self.kinetic_energy();
        let speed = self.angular_velocity;
        let load = LOAD_DRAG as f64 * speed * speed.abs();
        let drag = BEARING_DRAG as f64 * speed + load;
        let next = speed - drag * dt as f64 / FLYWHEEL_INERTIA;
        // Drag stops the shaft rather than reversing it within one step.
        self.angular_velocity = if speed.signum() == next.signum() {
            next
        } else {
            0.0
        };
        let loss = (before - self.kinetic_energy()).max(0.0);
        self.drag_loss += loss;
        // Both torques act at the same speed, so the loss splits between
        // them in proportion to their torques.
        if drag != 0.0 {
            self.load_work += loss * load / drag;
        }

        self.angular_velocity += self.back_pressure_torque() * dt as f64 / (2.0 * FLYWHEEL_INERTIA);
        let rotation = self.angular_velocity * dt as f64;
        let angle =
            (self.angle as f64 + self.angle_remainder + rotation).rem_euclid(std::f64::consts::TAU);
        self.angle = angle as f32;
        self.angle_remainder = angle - self.angle as f64;
        self.total_angle += rotation;
        self.angular_velocity += self.back_pressure_torque() * dt as f64 / (2.0 * FLYWHEEL_INERTIA);
        self.update_faces(dt);
        // Booked from the displaced volume, apart from the torque above, so
        // the shaft's energy balance checks one against the other.
        self.back_pressure_work -=
            self.reference_pressure as f64 * (self.swept_volume() - old_volume);
    }

    /// Stops the shaft and holds it by static friction; what little energy
    /// the flywheel had left goes to friction.
    fn hold(&mut self) {
        self.drag_loss += self.kinetic_energy();
        self.angular_velocity = 0.0;
        self.held = true;
        self.held_impulse = 0.0;
        self.held_time = 0.0;
        self.hot.velocity = 0.0;
        self.cold.velocity = 0.0;
    }

    /// Adds `dt` of held time and, once per `HOLD_WINDOW`, weighs the push
    /// on the held shaft against static friction, letting go when the push
    /// wins.
    ///
    /// Returns whether the shaft was let go.
    fn release(&mut self, dt: f64) -> bool {
        self.held_time += dt;
        if self.held_time < HOLD_WINDOW {
            return false;
        }
        let push = self.held_impulse / self.held_time + self.back_pressure_torque();
        self.held_impulse = 0.0;
        self.held_time = 0.0;
        self.held = push.abs() <= STATIC_FRICTION_SHARE * self.torque_scale();
        !self.held
    }

    /// Frees a held shaft for Start or the kick.
    fn let_go(&mut self) {
        self.held = false;
        self.direction = 0.0;
    }

    /// The torque the back pressure gives through a half-stroke lever: the
    /// size of the pushes on the crank, which grows with the fill.
    fn torque_scale(&self) -> f64 {
        self.reference_pressure as f64 * self.area as f64 * self.stroke as f64 / 2.0
    }

    /// Torque from the constant back pressure on both pistons.
    fn back_pressure_torque(&self) -> f64 {
        let levers = self.lever(PistonSide::Hot) + self.lever(PistonSide::Cold);
        -(self.reference_pressure * self.area * levers) as f64
    }

    /// Bounces a particle moving up at `upward_speed` off the face on `side`
    /// and returns its new upward speed, applying the reaction to the shaft.
    ///
    /// The collision is elastic between the particle and the shaft's
    /// effective mass at the current crank angle. Particles moving away from
    /// the face pass through unchanged. A held piston acts as a stationary
    /// wall, and its push is gathered for weighing against static friction.
    pub fn collide(&mut self, side: PistonSide, upward_speed: f32, particle_mass: f32) -> f32 {
        let relative_speed = upward_speed - self.face_velocity(side);
        if relative_speed <= 0.0 {
            return upward_speed;
        }
        let lever = self.lever(side) as f64;
        let inverse_mass = if self.held {
            0.0
        } else {
            lever * lever / FLYWHEEL_INERTIA
        };
        let impulse = 2.0 * relative_speed as f64 / (1.0 / particle_mass as f64 + inverse_mass);
        self.gas_impulse[side as usize] += impulse;
        if self.held {
            self.held_impulse += impulse * lever;
        } else {
            self.angular_velocity += impulse * lever / FLYWHEEL_INERTIA;
        }
        (upward_speed as f64 - impulse / particle_mass as f64) as f32
    }

    /// Vertical velocity of the face on `side`.
    pub fn face_velocity(&self, side: PistonSide) -> f32 {
        (self.lever(side) as f64 * self.angular_velocity) as f32
    }

    /// Brings the shaft up to the starting speed in the heat-engine
    /// direction, like a starter motor that lets go at once.
    ///
    /// A resting, slow or backward-turning shaft is set to that speed, and a
    /// shaft already turning at least that fast is left alone, so repeated
    /// presses cannot overdrive the engine. The change in flywheel energy is
    /// booked as starter work, apart from gas work. Returns whether the
    /// speed changed.
    pub fn start(&mut self) -> bool {
        self.crank(self.start_speed)
    }

    /// Turns the shaft by hand at `speed` (radians per second of model
    /// time) in the heat-engine direction, like a crank with a ratchet.
    ///
    /// The crank catches only a shaft turning slower than the hand, and
    /// never drives it past the starting speed, so it can start the engine
    /// but not overdrive it, as Start cannot. The change in flywheel energy
    /// is booked as starter work. Returns whether the speed changed.
    pub fn crank(&mut self, speed: f64) -> bool {
        let speed = speed.clamp(0.0, self.start_speed);
        if self.angular_velocity * FORWARD >= speed {
            return false;
        }
        self.let_go();
        let before = self.kinetic_energy();
        self.angular_velocity = FORWARD * speed;
        self.starter_work += self.kinetic_energy() - before;
        true
    }

    /// Adds a fixed change in speed in the heat-engine direction, worth
    /// `STARTER_ENERGY` from rest.
    ///
    /// Unlike Start it can be repeated to overdrive the engine. The change
    /// in flywheel energy is booked as starter work.
    pub fn kick(&mut self) {
        self.let_go();
        let before = self.kinetic_energy();
        self.angular_velocity += FORWARD * (2.0 * STARTER_ENERGY / FLYWHEEL_INERTIA).sqrt();
        self.starter_work += self.kinetic_energy() - before;
    }

    /// The hot piston.
    pub fn hot(&self) -> PistonState {
        self.hot
    }

    /// The cold piston.
    pub fn cold(&self) -> PistonState {
        self.cold
    }

    /// Crank angle, reduced to 0..2 pi.
    pub fn angle(&self) -> f32 {
        self.angle
    }

    /// Signed angular speed, in radians per second of model time.
    pub fn angular_velocity(&self) -> f32 {
        self.angular_velocity as f32
    }

    /// Speed in the heat-engine direction as a share of the speed Start
    /// gives: 1 just after Start, 0 at rest, negative when turning backward.
    pub fn start_speed_fraction(&self) -> f64 {
        FORWARD * self.angular_velocity / self.start_speed
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

    /// Returns the upward impulse the gas has given each face (indexed hot,
    /// cold) since the last call, and starts over.
    pub fn take_gas_impulse(&mut self) -> [f64; 2] {
        std::mem::take(&mut self.gas_impulse)
    }

    /// Piston travel.
    pub fn stroke(&self) -> f32 {
        self.stroke
    }

    /// Height of the crankshaft.
    pub fn shaft_height(&self) -> f32 {
        self.shaft_y
    }

    /// Kinetic energy of the flywheel.
    pub fn kinetic_energy(&self) -> f64 {
        0.5 * FLYWHEEL_INERTIA * self.angular_velocity.powi(2)
    }

    /// Flywheel energy added by Start and the kick since the fill.
    pub fn starter_work(&self) -> f64 {
        self.starter_work
    }

    /// Work done on the shaft by the back pressure since the fill.
    pub fn back_pressure_work(&self) -> f64 {
        self.back_pressure_work
    }

    /// Flywheel energy taken by bearing friction, static friction and the
    /// load since the fill.
    pub fn drag_loss(&self) -> f64 {
        self.drag_loss
    }

    /// The part of `drag_loss` taken by the load: the engine's useful output
    /// since the fill.
    pub fn load_work(&self) -> f64 {
        self.load_work
    }

    /// Gas volume below both faces plus the opening between them, for
    /// `geometry`.
    pub fn volume_m3(&self, geometry: ChamberGeometry) -> f64 {
        self.cylinder_volume_m3(geometry, self.hot)
            + self.cylinder_volume_m3(geometry, self.cold)
            + geometry.passage_volume_m3()
    }

    /// Gas volume between one cylinder's floor and the face of `piston`.
    pub fn cylinder_volume_m3(&self, geometry: ChamberGeometry, piston: PistonState) -> f64 {
        let radius = geometry.cylinder_radius as f64 * METERS_PER_UNIT;
        let height = (piston.face_y + geometry.cylinder_height / 2.0) as f64 * METERS_PER_UNIT;
        std::f64::consts::PI * radius * radius * height
    }
}

/// The sign of `value`: -1, 0 or 1.
fn sign(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value.signum() }
}

#[cfg(test)]
#[path = "../../tests/unit/sim/pistons.rs"]
mod tests;
