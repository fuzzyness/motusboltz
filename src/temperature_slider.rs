//! Mouse-controlled temperature slider in screen-space coordinates.

use macroquad::prelude::{
    Color,
    MouseButton,
    ORANGE,
    WHITE,
    draw_circle,
    draw_rectangle,
    draw_text,
    is_mouse_button_down,
    is_mouse_button_pressed,
    measure_text,
    mouse_position,
    screen_height,
    screen_width, //
};

/// Minimum selectable kinetic temperature, in reduced units (`k_B = 1`).
pub const MIN_TEMPERATURE: f32 = 0.1;
/// Maximum selectable kinetic temperature, in reduced units (`k_B = 1`).
pub const MAX_TEMPERATURE: f32 = 4.0;

const SLIDER_WIDTH: f32 = 260.0;
const SLIDER_RIGHT_MARGIN: f32 = 32.0;
const SLIDER_BOTTOM_MARGIN: f32 = 105.0;
const SLIDER_TRACK_HEIGHT: f32 = 8.0;
const SLIDER_HANDLE_RADIUS: f32 = 11.0;
const SLIDER_HIT_HALF_HEIGHT: f32 = 24.0;
const SLIDER_HIT_MARGIN_X: f32 = 14.0;

const PANEL_PADDING_X: f32 = 18.0;
const PANEL_PADDING_TOP: f32 = 52.0;
const PANEL_PADDING_BOTTOM: f32 = 43.0;
const LABEL_OFFSET_Y: f32 = 21.0;
const RANGE_LABEL_OFFSET_Y: f32 = 36.0;
const LABEL_FONT_SIZE: f32 = 21.0;
const RANGE_FONT_SIZE: f32 = 17.0;
const TEMPERATURE_PRECISION: usize = 2;
const RANGE_PRECISION: usize = 1;

const PANEL_COLOR: Color = Color {
    r: 0.035,
    g: 0.060,
    b: 0.105,
    a: 0.88,
};
const TRACK_COLOR: Color = Color {
    r: 0.27,
    g: 0.33,
    b: 0.40,
    a: 1.0,
};
const RANGE_TEXT_COLOR: Color = Color {
    r: 0.72,
    g: 0.78,
    b: 0.85,
    a: 1.0,
};

/// A temperature control that reserves pointer gestures from the orbit camera.
///
/// `update` must run before camera input, allowing `blocks_camera` to keep
/// pointer presses on the slider from orbiting or panning the chamber.
pub struct TemperatureSlider {
    value: f32,
    dragging: bool,
}

impl TemperatureSlider {
    /// Starts the control at the gas's measured initial temperature.
    pub fn new(initial_temperature: f32) -> Self {
        Self {
            value: initial_temperature.clamp(MIN_TEMPERATURE, MAX_TEMPERATURE),
            dragging: false,
        }
    }

    /// Returns the selected temperature in reduced simulation units.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Processes pointer input and returns a newly selected temperature.
    ///
    /// Dragging remains active after leaving the slider until mouse release.
    pub fn update(&mut self) -> Option<f32> {
        let (mouse_x, _) = mouse_position();
        let (track_x, _) = track_origin();

        if !is_mouse_button_down(MouseButton::Left) {
            self.dragging = false;
        }

        if is_mouse_button_pressed(MouseButton::Left) && pointer_over_slider() {
            self.dragging = true;
        }

        if !self.dragging {
            return None;
        }

        let new_value = value_at_x(mouse_x, track_x);
        if new_value == self.value {
            return None;
        }

        self.value = new_value;
        Some(new_value)
    }

    /// Whether pointer gestures should be withheld from the orbit camera.
    pub fn blocks_camera(&self) -> bool {
        self.dragging || pointer_over_panel()
    }

    /// Draws the control after restoring Macroquad's default 2D camera.
    pub fn draw(&self) {
        let (track_x, track_y) = track_origin();
        let progress = (self.value - MIN_TEMPERATURE) / (MAX_TEMPERATURE - MIN_TEMPERATURE);
        let handle_x = track_x + progress * SLIDER_WIDTH;

        draw_rectangle(
            track_x - PANEL_PADDING_X,
            track_y - PANEL_PADDING_TOP,
            SLIDER_WIDTH + 2.0 * PANEL_PADDING_X,
            PANEL_PADDING_TOP + PANEL_PADDING_BOTTOM,
            PANEL_COLOR,
        );

        let label = format!("Set temperature: {:.*}", TEMPERATURE_PRECISION, self.value);
        draw_text(
            &label,
            track_x,
            track_y - LABEL_OFFSET_Y,
            LABEL_FONT_SIZE,
            WHITE,
        );

        draw_rectangle(
            track_x,
            track_y - SLIDER_TRACK_HEIGHT / 2.0,
            SLIDER_WIDTH,
            SLIDER_TRACK_HEIGHT,
            TRACK_COLOR,
        );
        draw_rectangle(
            track_x,
            track_y - SLIDER_TRACK_HEIGHT / 2.0,
            handle_x - track_x,
            SLIDER_TRACK_HEIGHT,
            ORANGE,
        );
        draw_circle(handle_x, track_y, SLIDER_HANDLE_RADIUS, WHITE);

        let minimum = format!("{:.*}", RANGE_PRECISION, MIN_TEMPERATURE);
        let maximum = format!("{:.*}", RANGE_PRECISION, MAX_TEMPERATURE);
        let maximum_width = measure_text(&maximum, None, RANGE_FONT_SIZE as u16, 1.0).width;
        draw_text(
            &minimum,
            track_x,
            track_y + RANGE_LABEL_OFFSET_Y,
            RANGE_FONT_SIZE,
            RANGE_TEXT_COLOR,
        );
        draw_text(
            &maximum,
            track_x + SLIDER_WIDTH - maximum_width,
            track_y + RANGE_LABEL_OFFSET_Y,
            RANGE_FONT_SIZE,
            RANGE_TEXT_COLOR,
        );
    }
}

fn track_origin() -> (f32, f32) {
    (
        screen_width() - SLIDER_WIDTH - SLIDER_RIGHT_MARGIN,
        screen_height() - SLIDER_BOTTOM_MARGIN,
    )
}

fn pointer_over_slider() -> bool {
    let (x, y) = mouse_position();
    let (track_x, track_y) = track_origin();

    (track_x - SLIDER_HIT_MARGIN_X..=track_x + SLIDER_WIDTH + SLIDER_HIT_MARGIN_X).contains(&x)
        && (track_y - SLIDER_HIT_HALF_HEIGHT..=track_y + SLIDER_HIT_HALF_HEIGHT).contains(&y)
}

fn pointer_over_panel() -> bool {
    let (x, y) = mouse_position();
    let (track_x, track_y) = track_origin();

    (track_x - PANEL_PADDING_X..=track_x + SLIDER_WIDTH + PANEL_PADDING_X).contains(&x)
        && (track_y - PANEL_PADDING_TOP..=track_y + PANEL_PADDING_BOTTOM).contains(&y)
}

fn value_at_x(pointer_x: f32, track_x: f32) -> f32 {
    let progress = ((pointer_x - track_x) / SLIDER_WIDTH).clamp(0.0, 1.0);
    MIN_TEMPERATURE + progress * (MAX_TEMPERATURE - MIN_TEMPERATURE)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_TRACK_ORIGIN: f32 = 100.0;
    const TEST_TOLERANCE: f32 = 1e-5;
    const MIDPOINT_FRACTION: f32 = 0.5;

    #[test]
    fn pointer_position_maps_to_bounded_temperature() {
        assert_eq!(
            value_at_x(TEST_TRACK_ORIGIN, TEST_TRACK_ORIGIN),
            MIN_TEMPERATURE
        );
        assert_eq!(
            value_at_x(TEST_TRACK_ORIGIN + SLIDER_WIDTH, TEST_TRACK_ORIGIN),
            MAX_TEMPERATURE
        );
        assert_eq!(
            value_at_x(TEST_TRACK_ORIGIN - SLIDER_WIDTH, TEST_TRACK_ORIGIN),
            MIN_TEMPERATURE
        );
        assert_eq!(
            value_at_x(TEST_TRACK_ORIGIN + 2.0 * SLIDER_WIDTH, TEST_TRACK_ORIGIN),
            MAX_TEMPERATURE
        );

        let midpoint = value_at_x(
            TEST_TRACK_ORIGIN + MIDPOINT_FRACTION * SLIDER_WIDTH,
            TEST_TRACK_ORIGIN,
        );
        let expected = (MIN_TEMPERATURE + MAX_TEMPERATURE) * MIDPOINT_FRACTION;
        assert!((midpoint - expected).abs() < TEST_TOLERANCE);
    }

    #[test]
    fn initial_value_is_clamped_to_slider_range() {
        assert_eq!(TemperatureSlider::new(0.0).value(), MIN_TEMPERATURE);
        assert_eq!(
            TemperatureSlider::new(MAX_TEMPERATURE + MIN_TEMPERATURE).value(),
            MAX_TEMPERATURE
        );
    }
}
