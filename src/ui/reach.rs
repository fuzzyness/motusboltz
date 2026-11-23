//! What the figure can use in first person, and what E does to it.
//!
//! In first person the parts the mouse grabs in the orbit view are used
//! with E instead, as in Penumbra: whatever the crosshair is on, within an
//! arm's reach, is aimed at, and a caption under the crosshair says what
//! it is and what E does. Held, E grips a valve, the tank's knob or the
//! crank, and the mouse turns it (see `first_person::Walker::grip`); on
//! the desk clock, a tap turns slow motion on or off and a hold pauses.
//! Pressed, it picks up the regenerator's cartridge, or takes it out of
//! the opening, until pressed again, and presses the Reset and Tracers
//! buttons. The left mouse button does all E does.

use std::f32::consts::PI;

use macroquad::prelude::Vec3;

use crate::sim::reservoir::ReservoirSide;
use crate::ui::handles::Handle;

/// Farthest the figure reaches from its eyes, in bore radii: from the
/// table beside the block up to the crank's handle, and down to the
/// cartridge lying on the table.
pub const REACH: f32 = 3.6;
/// How far a valve's wheel turns for each point the mouse moves across,
/// in radians: the whole of a valve's turns in a long sweep of the mouse.
pub const TURN_PER_POINT: f32 = 0.012;
/// How fast a valve's wheel or the knob keeps turning while the view is
/// held at the edge of its room, in radians a second: half a turn a
/// second at the edge, up to three times that pushed hard against it
/// (see `first_person::Walker::grip_lean`).
pub const EDGE_TURN_PER_SECOND: f32 = PI;
/// How far round a valve's wheel aiming finds it, in the wheel's radii.
pub const VALVE_AIM: f32 = 2.0;
/// How far round a button's dome aiming finds it, in its box's half
/// widths.
pub const BUTTON_AIM: f32 = 1.3;
/// How far round the tank's knob aiming finds it, in bore radii.
pub const KNOB_AIM: f32 = 0.45;
/// How far round the crank's handle aiming finds it, in bore radii.
pub const CRANK_AIM: f32 = 0.6;
/// How far round the cartridge aiming finds it, in the opening's radii.
pub const CARTRIDGE_AIM: f32 = 1.3;
/// How far in front of the eyes the cartridge is carried, in bore radii.
pub const CARRY_AHEAD: f32 = 1.6;
/// How far below the middle of the view it is carried, in bore radii:
/// low, clear of the crosshair and its caption.
pub const CARRY_BELOW: f32 = 0.9;

/// Something within reach to use: what it is, the point to aim at, and how
/// far round that point aiming finds it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Usable {
    /// What it is.
    pub handle: Handle,
    /// The point to aim at.
    pub point: Vec3,
    /// How far round the point the line of sight finds it.
    pub radius: f32,
}

/// The one of `usables` the line of sight from `eye` along `forward` (a
/// unit vector) is on, within `reach` of the eyes: of those it passes
/// within their radius of, the one it passes nearest the middle of, for
/// its size.
pub fn aimed(eye: Vec3, forward: Vec3, usables: &[Usable], reach: f32) -> Option<Handle> {
    usables
        .iter()
        .filter_map(|usable| {
            let to = usable.point - eye;
            let along = to.dot(forward);
            let off = (to - forward * along).length();
            let near = along > 0.0 && to.length() <= reach + usable.radius;
            (near && off <= usable.radius).then_some((usable.handle, off / usable.radius))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(handle, _)| handle)
}

/// What the cartridge is doing, for its caption.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CartridgeState {
    /// Lying on the table.
    Lying,
    /// Seated in the opening.
    Seated,
}

/// The caption under the crosshair for `handle`, aimed at: what it is and
/// what E does to it. `cartridge` says where the cartridge is.
pub fn caption(handle: Handle, cartridge: CartridgeState) -> &'static str {
    match handle {
        Handle::Crank => "Hand crank \u{b7} hold E, then turn the mouse around it",
        Handle::Valve(ReservoirSide::Hot) => "Gas valve \u{b7} hold E, then move the mouse to turn",
        Handle::Valve(ReservoirSide::Cold) => {
            "Nitrogen valve \u{b7} hold E, then move the mouse to turn"
        }
        Handle::TankKnob => "Helium knob \u{b7} hold E, then move the mouse to turn",
        Handle::Clock => "Desk clock \u{b7} tap E: slow motion \u{b7} hold E: pause",
        Handle::ResetButton => "Reset \u{b7} tap E to refill the gas",
        Handle::TracersButton => "Tracers \u{b7} tap E",
        Handle::Cartridge => match cartridge {
            CartridgeState::Lying => "Regenerator \u{b7} press E to pick it up",
            CartridgeState::Seated => "Regenerator \u{b7} press E to take it out",
        },
    }
}

/// The caption under the crosshair while `handle` is in use.
pub fn using_caption(handle: Handle) -> &'static str {
    match handle {
        Handle::Crank => "Turn the mouse around the crank, either way",
        Handle::Valve(_) | Handle::TankKnob => "Move the mouse left or right to turn",
        Handle::Clock => "Hold E to pause, or let go for slow motion",
        Handle::Cartridge => "Bring it to the opening \u{b7} press E to drop it",
        Handle::ResetButton | Handle::TracersButton => "",
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/reach.rs"]
mod tests;
