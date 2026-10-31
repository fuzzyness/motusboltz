//! When to offer the Start button.
//!
//! The button shows whenever pressing it would help, that is while the
//! flywheel is at rest, has nearly stopped or turns backward, and not while
//! the engine keeps itself turning.

/// At or below this fraction of the starting speed the engine counts as
/// stopped (see `PowerPistons::start_speed_fraction`).
const STOPPED_FRACTION: f64 = 0.15;
/// At or above this fraction the engine counts as running, and the button
/// hides.
///
/// Between the two fractions the button keeps its state, so a speed that
/// wobbles near either line does not make it flicker.
const RUNNING_FRACTION: f64 = 0.3;
/// Wall time the engine must stay stopped before the button returns, in
/// seconds.
///
/// A shaft exactly at rest, as after a refill, shows it at once.
const SHOW_AFTER_SECONDS: f64 = 1.5;

/// Whether the Start button shows.
pub struct StartPrompt {
    /// Whether the button shows now.
    shown: bool,
    /// Wall time since which the engine has counted as stopped.
    stopped_since: Option<f64>,
}

impl StartPrompt {
    /// A shown prompt, since the engine starts at rest.
    pub fn new() -> Self {
        Self {
            shown: true,
            stopped_since: None,
        }
    }

    /// Updates from the shaft's `fraction` of the starting speed at wall
    /// time `now`.
    pub fn update(&mut self, fraction: f64, now: f64) {
        if fraction >= RUNNING_FRACTION {
            self.shown = false;
            self.stopped_since = None;
        } else if fraction == 0.0 {
            self.shown = true;
        } else if fraction <= STOPPED_FRACTION {
            let since = *self.stopped_since.get_or_insert(now);
            self.shown |= now - since >= SHOW_AFTER_SECONDS;
        } else {
            self.stopped_since = None;
        }
    }

    /// Whether the button shows.
    pub fn shown(&self) -> bool {
        self.shown
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/start_prompt.rs"]
mod tests;
