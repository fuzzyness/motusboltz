//! Screen layout: the readings panel on the left, the sliders on the right,
//! and the 3D scene behind both, filling the window. The engine and the
//! controls placed in the scene are centered on the window, in a span kept
//! clear of the panel and the slider column (or, when they are closed, of
//! the tabs left at the window's edges), so closing either never moves the
//! engine. Sizes are in logical points.

/// The panel's card stands this far in from the window's left, top and
/// bottom edges.
pub const PANEL_INSET: f32 = 12.0;
/// From the window's left edge to the right edge of the panel's card.
pub const PANEL_WIDTH: f32 = 352.0;
/// Width of the tab a closed panel or slider column leaves at the edge.
pub const EDGE_TAB_WIDTH: f32 = 28.0;
/// Width of the slider tracks at the right edge.
pub const CONTROL_WIDTH: f32 = 260.0;
/// Gap between the controls and the right edge of the window.
pub const CONTROL_RIGHT_MARGIN: f32 = 28.0;
/// Room kept beside the slider tracks, for their handles and hit areas.
pub const CONTROL_PADDING_X: f32 = 18.0;
/// Width kept free for the controls at the right edge.
pub const CONTROL_COLUMN_WIDTH: f32 = CONTROL_WIDTH + CONTROL_RIGHT_MARGIN + CONTROL_PADDING_X;

/// Left edge of the slider tracks.
pub fn control_left(screen_width: f32) -> f32 {
    screen_width - CONTROL_WIDTH - CONTROL_RIGHT_MARGIN
}

/// Left edge of the uncovered scene: the panel's right edge, or that of
/// the tab a closed panel leaves. The corner readout starts here.
pub fn scene_left(show_panel: bool) -> f32 {
    if show_panel {
        PANEL_WIDTH
    } else {
        EDGE_TAB_WIDTH
    }
}

/// Left edge and width of the span the engine and the centered controls
/// use: centered on the window and inset on both sides by the wider of
/// what covers each edge, the panel or its tab at the left and the slider
/// column or its tab at the right.
pub fn scene_span(screen_width: f32, show_panel: bool, show_sliders: bool) -> (f32, f32) {
    let left = scene_left(show_panel);
    let right = if show_sliders {
        CONTROL_COLUMN_WIDTH
    } else {
        EDGE_TAB_WIDTH
    };
    let inset = left.max(right);
    (inset, (screen_width - 2.0 * inset).max(1.0))
}

#[cfg(test)]
#[path = "../../tests/unit/ui/layout.rs"]
mod tests;
