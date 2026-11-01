//! Motion for the controls and overlays: the curves and steps their small
//! animations share.
//!
//! Every animation here runs on wall time, so it keeps its pace in slow
//! motion and while the engine is paused.

use macroquad::prelude::Color;

/// `t` from 0 to 1 eased in and out, flat at both ends; outside that range
/// it holds the nearer end.
pub fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `progress` stepped `dt` seconds toward 1 if `on`, or toward 0, at a pace
/// that crosses the whole way in `seconds`.
pub fn advance(progress: f32, on: bool, dt: f32, seconds: f32) -> f32 {
    let step = dt / seconds;
    if on {
        (progress + step).min(1.0)
    } else {
        (progress - step).max(0.0)
    }
}

/// `value` moved toward `target` after `dt` seconds, closing all but a
/// share of 1/e of the gap every `seconds`.
pub fn approach(value: f32, target: f32, dt: f32, seconds: f32) -> f32 {
    target + (value - target) * (-dt / seconds).exp()
}

/// `color` with its opacity scaled by `share`.
pub fn faded(color: Color, share: f32) -> Color {
    Color {
        a: color.a * share.clamp(0.0, 1.0),
        ..color
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/motion.rs"]
mod tests;
