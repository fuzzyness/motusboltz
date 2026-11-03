//! The engine's own controls, grabbed and turned with the mouse: the hand
//! crank on the shaft and the two valves; and the parts pressed with it:
//! the Pause and Tracers buttons and the regenerator's cartridge.
//!
//! A press near a part's handle, on screen, grabs it; while the button is
//! held, the pointer is followed onto the plane the part turns in, and the
//! part turns with it. The crank drives the shaft forward as a ratchet (see
//! `PowerPistons::crank`). A valve's wheel sets its side's temperature: shut
//! for 300 K, fully open after `VALVE_TURNS` turns for the slider's far
//! end. The walls then move toward a new setting at
//! `RAMP_KELVIN_PER_SECOND`, as a burner or a nitrogen line takes time to
//! heat or cool them, whether it was set by a valve or a slider.

use std::f32::consts::{
    PI,
    TAU, //
};

use macroquad::prelude::{
    Mat4,
    Vec2,
    Vec3,
    vec2, //
};

use crate::sim::reservoir::{
    MAX_COLD_KELVIN,
    MAX_HOT_KELVIN,
    MIN_COLD_KELVIN,
    MIN_HOT_KELVIN,
    ReservoirSide, //
};
use crate::ui::scene_labels::project;

/// Turns of a valve's wheel from shut to fully open.
pub const VALVE_TURNS: f32 = 1.5;
/// How fast a wall's temperature moves toward its setting, in kelvin per
/// second of model time.
pub const RAMP_KELVIN_PER_SECOND: f32 = 150.0;
/// How near a handle, on screen, a press grabs it, in points.
const GRAB_RADIUS: f32 = 30.0;
/// Least slant between the pointer's ray and a part's plane, below which
/// the plane is seen edge on and the pointer cannot be followed onto it.
const MIN_SLANT: f32 = 1e-3;

/// A part that can be grabbed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// The hand crank on the shaft.
    Crank,
    /// The valve that feeds `ReservoirSide`'s wall.
    Valve(ReservoirSide),
    /// The Pause button.
    PauseButton,
    /// The Tracers button.
    TracersButton,
    /// The regenerator's cartridge, on the table or in the opening.
    Cartridge,
}

impl Handle {
    /// Whether it acts once when pressed, rather than turning while held.
    pub fn presses(self) -> bool {
        matches!(
            self,
            Self::PauseButton | Self::TracersButton | Self::Cartridge
        )
    }
}

/// The temperature range a valve sets, from shut to fully open.
fn valve_range(side: ReservoirSide) -> (f32, f32) {
    match side {
        ReservoirSide::Hot => (MIN_HOT_KELVIN, MAX_HOT_KELVIN),
        ReservoirSide::Cold => (MAX_COLD_KELVIN, MIN_COLD_KELVIN),
    }
}

/// How far `side`'s valve is open, from 0 (shut) to 1, for a setting of
/// `kelvin`.
pub fn valve_opening(side: ReservoirSide, kelvin: f32) -> f32 {
    let (shut, open) = valve_range(side);
    ((kelvin - shut) / (open - shut)).clamp(0.0, 1.0)
}

/// The setting `side`'s valve gives when it is `opening` open (see
/// `valve_opening`).
pub fn valve_kelvin(side: ReservoirSide, opening: f32) -> f32 {
    let (shut, open) = valve_range(side);
    shut + (open - shut) * opening.clamp(0.0, 1.0)
}

/// How far `side`'s wheel has turned from shut for a setting of `kelvin`,
/// in radians counterclockwise seen from above.
pub fn valve_turn(side: ReservoirSide, kelvin: f32) -> f32 {
    valve_opening(side, kelvin) * VALVE_TURNS * TAU
}

/// `current` moved toward `target` over `dt` seconds of model time at
/// `RAMP_KELVIN_PER_SECOND`, without passing it.
pub fn ramped(current: f32, target: f32, dt: f32) -> f32 {
    let step = RAMP_KELVIN_PER_SECOND * dt.max(0.0);
    current + (target - current).clamp(-step, step)
}

/// `angle` brought into -pi to pi, so a turn across the seam reads as the
/// short way round.
pub fn wrapped(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

/// A ray from the camera through a point on the screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    /// Where it starts, on the near plane.
    pub origin: Vec3,
    /// Its direction, a unit vector.
    pub direction: Vec3,
}

impl Ray {
    /// The ray through `pointer` on a screen of `size`, seen through
    /// `matrix` (projection times view), or `None` if the matrix cannot be
    /// undone.
    pub fn through(matrix: Mat4, size: Vec2, pointer: Vec2) -> Option<Self> {
        let undo = matrix.inverse();
        if !undo.is_finite() {
            return None;
        }
        let ndc = vec2(
            2.0 * pointer.x / size.x - 1.0,
            1.0 - 2.0 * pointer.y / size.y,
        );
        let at = |depth: f32| undo.project_point3(ndc.extend(depth));
        let (near, far) = (at(-1.0), at(1.0));
        let direction = (far - near).try_normalize()?;
        Some(Self {
            origin: near,
            direction,
        })
    }

    /// Where the ray meets the plane through `point` square to `normal`,
    /// or `None` if it runs along the plane or meets it behind its start.
    pub fn meet(self, point: Vec3, normal: Vec3) -> Option<Vec3> {
        let slant = self.direction.dot(normal);
        if slant.abs() < MIN_SLANT {
            return None;
        }
        let distance = (point - self.origin).dot(normal) / slant;
        (distance >= 0.0).then(|| self.origin + self.direction * distance)
    }
}

/// The handle among `handles` (each with the point to press) nearest
/// `pointer` on a screen of `size` seen through `matrix`, if one is within
/// `GRAB_RADIUS`.
pub fn pick(handles: &[(Handle, Vec3)], matrix: Mat4, size: Vec2, pointer: Vec2) -> Option<Handle> {
    handles
        .iter()
        .filter_map(|&(handle, point)| {
            let spot = project(matrix, size, point)?;
            Some((handle, spot.distance(pointer)))
        })
        .filter(|&(_, distance)| distance <= GRAB_RADIUS)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(handle, _)| handle)
}

/// The angle of `point` about a valve's wheel at `center`, counterclockwise
/// seen from above, as the wheel's turn is counted.
pub fn wheel_angle(center: Vec3, point: Vec3) -> f32 {
    let from = point - center;
    (-from.z).atan2(from.x)
}

/// A grabbed handle and where the pointer last was around it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grab {
    /// What is held.
    pub handle: Handle,
    /// The pointer's last angle about the part's axis, if it has been
    /// followed onto the part's plane.
    pub last: Option<f32>,
}

impl Grab {
    /// `handle`, just grabbed.
    pub fn new(handle: Handle) -> Self {
        Self { handle, last: None }
    }

    /// Takes the pointer's new `angle` about the part's axis (or `None` if
    /// it could not be followed this frame), and returns how far it turned
    /// since the last one, the short way round.
    pub fn turned(&mut self, angle: Option<f32>) -> f32 {
        let Some(angle) = angle else {
            return 0.0;
        };
        let turn = self.last.map_or(0.0, |last| wrapped(angle - last));
        self.last = Some(angle);
        turn
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/handles.rs"]
mod tests;
