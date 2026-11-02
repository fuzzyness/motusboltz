//! Screen layout: the readings panel on the left, the sliders on the right,
//! and the 3D scene behind both, filling the window.
//!
//! The engine and the controls placed in the scene are centered on the
//! window, in a span kept clear of the panel and the slider column (or,
//! when they are closed, of a margin at the window's edges), so closing
//! either never moves the engine. Sizes are in logical points.

/// The panel's card stands this far in from the window's top and bottom
/// edges.
pub const PANEL_INSET: f32 = 12.0;
/// From the window's left edge to the left edge of the panel's card: room
/// for the panel's button beside it.
pub const PANEL_LEFT: f32 = EDGE_TAB_WIDTH;
/// From the window's left edge to the right edge of the panel's card.
pub const PANEL_WIDTH: f32 = 368.0;
/// Width of the button beside each drawer at the window's edge, and of the
/// margin kept at an edge whose drawer is closed.
pub const EDGE_TAB_WIDTH: f32 = 28.0;
/// Width of the slider tracks at the right edge.
pub const CONTROL_WIDTH: f32 = 260.0;
/// Gap between the slider tracks and the right edge of the window, which
/// holds the end of the sliders' card and their button beside it.
pub const CONTROL_RIGHT_MARGIN: f32 = 44.0;
/// Room kept beside the slider tracks, for their handles and hit areas.
pub const CONTROL_PADDING_X: f32 = 18.0;
/// Width kept free for the controls at the right edge.
pub const CONTROL_COLUMN_WIDTH: f32 = CONTROL_WIDTH + CONTROL_RIGHT_MARGIN + CONTROL_PADDING_X;

/// Left edge of the slider tracks.
pub fn control_left(screen_width: f32) -> f32 {
    screen_width - CONTROL_WIDTH - CONTROL_RIGHT_MARGIN
}

/// Left edge of the uncovered scene.
///
/// That is the panel's right edge, or the edge margin while the panel is
/// closed.
pub fn scene_left(show_panel: bool) -> f32 {
    if show_panel {
        PANEL_WIDTH
    } else {
        EDGE_TAB_WIDTH
    }
}

/// Left edge and width of the span the engine and the centered controls
/// use.
///
/// The span is centered on the window and inset on both sides by the wider
/// of what covers each edge, the panel or its tab at the left and the
/// slider column or its margin at the right.
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
