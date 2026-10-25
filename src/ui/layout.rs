//! Screen layout: the readings panel on the left, the controls on the right,
//! and the 3D scene between them. Sizes are in logical points.

/// Width of the readings panel at the left edge.
pub const PANEL_WIDTH: f32 = 340.0;
/// Width of the buttons and slider tracks at the right edge.
pub const CONTROL_WIDTH: f32 = 260.0;
/// Gap between the controls and the right edge of the window.
pub const CONTROL_RIGHT_MARGIN: f32 = 32.0;
/// Padding of each slider's background beyond its track, left and right.
pub const CONTROL_PADDING_X: f32 = 18.0;
/// Width kept free for the controls at the right edge.
pub const CONTROL_COLUMN_WIDTH: f32 = CONTROL_WIDTH + CONTROL_RIGHT_MARGIN + CONTROL_PADDING_X;

/// Left edge of the buttons and slider tracks.
pub fn control_left(screen_width: f32) -> f32 {
    screen_width - CONTROL_WIDTH - CONTROL_RIGHT_MARGIN
}

/// Left edge and width of the 3D scene: between the panel (when shown)
/// and the control column.
pub fn scene_span(screen_width: f32, show_panel: bool) -> (f32, f32) {
    let left = if show_panel { PANEL_WIDTH } else { 0.0 };
    (left, (screen_width - left - CONTROL_COLUMN_WIDTH).max(1.0))
}

/// The scene's viewport in framebuffer pixels. Screen sizes are in logical
/// points, but Macroquad hands a camera viewport straight to OpenGL, which
/// counts physical pixels, so it is scaled by the display's DPI factor
/// (2 on a Retina screen with `high_dpi`).
pub fn scene_viewport(
    screen_width: f32,
    screen_height: f32,
    show_panel: bool,
    dpi_scale: f32,
) -> (i32, i32, i32, i32) {
    let (left, width) = scene_span(screen_width, show_panel);
    let pixels = |points: f32| (points * dpi_scale).round() as i32;
    (
        pixels(left),
        0,
        pixels(width),
        pixels(screen_height.max(1.0)),
    )
}

#[cfg(test)]
#[path = "../../tests/unit/ui/layout.rs"]
mod tests;
