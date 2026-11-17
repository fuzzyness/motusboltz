//! The helium port: where the pipe from the tank on the table meets the
//! cold cylinder's wall, low down at its outer end, above the nitrogen
//! coil.
//!
//! Gas comes in through it from the tank and goes back out through it, a
//! particle at a time (see `ConnectedChambers::set_concentration`). The
//! port is only a place on the wall: particles bounce off the wall there
//! as anywhere else. New ones start just inside it, heading in; leaving
//! ones are taken from those that come near it.
//!
//! The cold side is where real engines are filled, away from the heat, and
//! the tank's gas, at room temperature, is what that side holds by
//! default, so filling disturbs the engine least there.

use macroquad::prelude::{
    Vec3,
    vec3, //
};

use crate::sim::geometry::ChamberGeometry;

/// Where round the cold cylinder the port is, in radians from straight
/// out along +x, toward the back (-z): beside the outer end, clear of the
/// buttons in front, and in sight from the usual view.
const PORT_ANGLE: f32 = -0.5;
/// Height of the port's middle above the cylinder's floor, in bore radii:
/// above the nitrogen coil.
const PORT_HEIGHT: f32 = 0.62;
/// Radius of the port's opening, in bore radii.
const PORT_RADIUS: f32 = 0.11;
/// How far from the middle of the outlet a particle is taken back into
/// the tank, in bore radii; the outlet's middle is this far in from the
/// mouth too, so all of it is inside the bore.
const CAPTURE_REACH: f32 = 0.8;
/// Particles the pump moves each second of model time, either way: the
/// whole range of the fill in about three seconds.
pub const FLOW_PER_SECOND: f32 = 600.0;
/// Temperature of the gas in the tank, room temperature.
pub const TANK_KELVIN: f32 = 300.0;

/// The port on the cold cylinder's wall.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Port {
    /// The cold cylinder's axis, seen from above (x, z).
    pub axis: (f32, f32),
    /// The way out of the cylinder through the port, level, unit length.
    pub outward: Vec3,
    /// Height of the port's middle.
    pub height: f32,
    /// Radius of the bore the port opens from.
    pub bore: f32,
    /// Radius of the port's opening.
    pub radius: f32,
    /// How far from the middle of the outlet (see `outlet`) a particle is
    /// taken back.
    pub capture: f32,
}

impl Port {
    /// The port of a chamber of `geometry`.
    pub fn new(geometry: ChamberGeometry) -> Self {
        let bore = geometry.cylinder_radius;
        let (sin, cos) = PORT_ANGLE.sin_cos();
        Self {
            axis: (geometry.cold_center_x(), 0.0),
            outward: vec3(cos, 0.0, sin),
            height: geometry.floor() + PORT_HEIGHT * bore,
            bore,
            radius: PORT_RADIUS * bore,
            capture: CAPTURE_REACH * bore,
        }
    }

    /// The point on the port's middle line `reach` from the cylinder's
    /// axis: the bore at the inner face of the wall, beyond it outside.
    pub fn at(self, reach: f32) -> Vec3 {
        vec3(self.axis.0, self.height, self.axis.1) + self.outward * reach
    }

    /// Where a particle of `radius` starts as it comes in, on the middle
    /// line: as near the wall as lets it start anywhere across the opening
    /// and still clear of the wall.
    pub fn mouth(self, particle_radius: f32) -> Vec3 {
        let clear = self.bore - particle_radius * MOUTH_INSET;
        self.at((clear * clear - self.radius * self.radius).max(0.0).sqrt())
    }

    /// The middle of the outlet, the round space just inside the port
    /// that leaving particles are taken from.
    pub fn outlet(self, particle_radius: f32) -> Vec3 {
        self.mouth(particle_radius) - self.outward * self.capture
    }

    /// Two unit vectors across the opening, square to each other and to
    /// the way out.
    pub fn across(self) -> (Vec3, Vec3) {
        (self.outward.cross(Vec3::Y).normalize(), Vec3::Y)
    }

    /// A spot in the inlet, the cone a new particle of `radius` may start
    /// in, from three numbers from 0 to 1: how deep in from the mouth, how
    /// far round, and how far out from the middle line.
    ///
    /// The cone widens inward from the opening, as a jet does, so new
    /// particles find room among the gas instead of crowding the opening.
    pub fn inlet_spot(self, particle_radius: f32, deep: f32, round: f32, out: f32) -> Vec3 {
        let depth = deep * INLET_DEPTH * self.bore;
        let spread = (self.radius + depth * INLET_SPREAD) * out.sqrt();
        let (side, up) = self.across();
        let (sin, cos) = (round * std::f32::consts::TAU).sin_cos();
        self.mouth(particle_radius) - self.outward * depth + (side * cos + up * sin) * spread
    }

    /// Farthest a spot in the inlet lies from the mouth.
    #[cfg(test)]
    pub fn inlet_reach(self) -> f32 {
        let depth = INLET_DEPTH * self.bore;
        depth.hypot(self.radius + depth * INLET_SPREAD)
    }
}

/// How deep the inlet reaches in from the mouth, in bore radii.
const INLET_DEPTH: f32 = 0.5;
/// How much the inlet widens for each step in, as a share of the step.
const INLET_SPREAD: f32 = 0.8;

/// How far in from the bore a new particle's center keeps, in particle
/// radii: a hair more than one, so it starts clear of the wall.
const MOUTH_INSET: f32 = 1.05;

/// Where each of `len` particles goes once those in `leaving` are taken
/// out and the rest close up in order: its new index, or `None` if it
/// left.
pub fn closed_up(len: usize, leaving: &[usize]) -> Vec<Option<usize>> {
    let mut gone = vec![false; len];
    for &index in leaving {
        gone[index] = true;
    }
    let mut next = 0;
    gone.into_iter()
        .map(|left| {
            (!left).then(|| {
                next += 1;
                next - 1
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "../../tests/unit/sim/port.rs"]
mod tests;
