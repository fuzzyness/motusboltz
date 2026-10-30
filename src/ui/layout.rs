//! Screen layout: the readings panel on the left, the sliders on the right,
//! and the 3D scene behind both, filling the window. The engine and the
//! controls placed in the scene are centered on the window, in a span kept
//! clear of the panel (when shown) and the slider column, so hiding the
//! panel leaves everything where it was. Sizes are in logical points.

/// Width of the readings panel at the left edge.
pub const PANEL_WIDTH: f32 = 340.0;
/// Width of the slider tracks at the right edge.
pub const CONTROL_WIDTH: f32 = 260.0;
/// Gap between the controls and the right edge of the window.
pub const CONTROL_RIGHT_MARGIN: f32 = 32.0;
/// Room kept beside the slider tracks, for their handles and hit areas.
pub const CONTROL_PADDING_X: f32 = 18.0;
/// Width kept free for the controls at the right edge.
pub const CONTROL_COLUMN_WIDTH: f32 = CONTROL_WIDTH + CONTROL_RIGHT_MARGIN + CONTROL_PADDING_X;

/// Left edge of the slider tracks.
pub fn control_left(screen_width: f32) -> f32 {
    screen_width - CONTROL_WIDTH - CONTROL_RIGHT_MARGIN
}

/// Left edge of the uncovered scene: the panel's right edge, or the
/// window's. The buttons at the top left start here.
pub fn scene_left(show_panel: bool) -> f32 {
    if show_panel { PANEL_WIDTH } else { 0.0 }
}

/// Left edge and width of the span the engine and the centered controls
/// use: centered on the window and inset on both sides by the wider of the
/// panel (when shown) and the slider column, so neither covers them.
pub fn scene_span(screen_width: f32, show_panel: bool) -> (f32, f32) {
    let inset = scene_left(show_panel).max(CONTROL_COLUMN_WIDTH);
    (inset, (screen_width - 2.0 * inset).max(1.0))
}

#[cfg(test)]
#[path = "../../tests/unit/ui/layout.rs"]
mod tests;
