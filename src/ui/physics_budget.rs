//! How much model time a frame may run, so the physics never takes so
//! long that frames are missed.
//!
//! The physics costs a fairly steady amount of the computer's time per
//! second of model time, more with more gas and hotter walls. On a
//! computer too slow to run it in real time beside the drawing, running
//! every frame's whole time would make each frame miss the display's
//! refresh, and the frame rate would halve, which makes the next frame's
//! time, and its physics, longer still. So each frame runs at most the
//! model time that fits in a share of the refresh interval, at the cost
//! lately measured: the engine then runs a little slower than real time,
//! as the "physics time limited" warning says, but the frames keep coming
//! at the display's rate.

/// Most of a frame the physics may take, as a share of the display's
/// refresh interval; the rest is left for drawing.
const PHYSICS_SHARE: f64 = 0.5;
/// Share of the gap between a frame's measured cost and the lasting
/// estimate that the frame closes, so one slow frame barely moves it.
const COST_SMOOTHING: f64 = 0.1;

/// The physics' cost as lately measured, and the model time a frame may
/// run at it.
pub struct PhysicsBudget {
    /// Seconds of the computer's time each second of model time takes,
    /// once a frame has run some.
    cost: Option<f64>,
}

impl PhysicsBudget {
    /// A budget with nothing measured, which limits nothing.
    pub fn new() -> Self {
        Self { cost: None }
    }

    /// Records that running `model` seconds of model time took `seconds`
    /// of the computer's time.
    ///
    /// Frames that ran no model time say nothing about the cost and are
    /// ignored.
    pub fn record(&mut self, model: f32, seconds: f64) {
        if model.is_nan() || model <= 0.0 || !seconds.is_finite() {
            return;
        }
        let measured = seconds.max(0.0) / f64::from(model);
        self.cost = Some(match self.cost {
            Some(cost) => cost + (measured - cost) * COST_SMOOTHING,
            None => measured,
        });
    }

    /// The most model time a frame shown for `interval` seconds may run;
    /// unlimited until a cost has been measured.
    pub fn allowance(&self, interval: f32) -> f32 {
        match self.cost {
            Some(cost) if cost > 0.0 => (PHYSICS_SHARE * f64::from(interval) / cost) as f32,
            _ => f32::INFINITY,
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/physics_budget.rs"]
mod tests;
