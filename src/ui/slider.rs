//! Horizontal slider widget, used for the hot and cold reservoir
//! temperatures and the helium concentration.

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
    screen_width, //
};

use crate::sim::reservoir::{
    MAX_COLD_KELVIN,
    MAX_HOT_KELVIN,
    MIN_COLD_KELVIN,
    MIN_HOT_KELVIN, //
};
use crate::sim::units::{
    MAX_INITIAL_CONCENTRATION_MOL_PER_L,
    MIN_INITIAL_CONCENTRATION_MOL_PER_L, //
};
use crate::ui::layout::{
    CONTROL_PADDING_X,
    CONTROL_WIDTH,
    control_left, //
};

const SLIDER_WIDTH: f32 = CONTROL_WIDTH;
const SLIDER_TRACK_HEIGHT: f32 = 8.0;
const SLIDER_HANDLE_RADIUS: f32 = 11.0;
const SLIDER_HIT_HALF_HEIGHT: f32 = 24.0;
const SLIDER_HIT_MARGIN_X: f32 = 14.0;
/// Background above and below the track, which holds the label and the
/// range values.
pub const PANEL_PADDING_TOP: f32 = 52.0;
const PANEL_PADDING_BOTTOM: f32 = 43.0;
const LABEL_SIZE: f32 = 21.0;
/// Baseline of the label above the track.
const LABEL_OFFSET_Y: f32 = 21.0;
const RANGE_SIZE: f32 = 17.0;
/// Baseline of the minimum and maximum values below the track.
const RANGE_OFFSET_Y: f32 = 36.0;
/// Track centers, top to bottom.
pub const HOT_Y: f32 = 200.0;
const COLD_Y: f32 = HOT_Y + 120.0;
const CONCENTRATION_Y: f32 = COLD_Y + 120.0;
const KELVIN_STEP: f32 = 1.0;
const MOLARITY_STEP: f32 = 0.005;
const PANEL_COLOR: Color = Color::new(0.035, 0.060, 0.105, 0.88);
const TRACK_COLOR: Color = Color::new(0.27, 0.33, 0.40, 1.0);
const RANGE_TEXT_COLOR: Color = Color::new(0.72, 0.78, 0.85, 1.0);

/// What happened to a slider this frame.
#[derive(Default)]
pub struct SliderUpdate {
    pub changed: bool,
    pub released: bool,
}

/// Horizontal slider dragged with the left mouse button; values snap to
/// multiples of `step` above `min`.
pub struct Slider {
    label: &'static str,
    min: f32,
    max: f32,
    step: f32,
    precision: usize,
    y: f32,
    value: f32,
    dragging: bool,
}

impl Slider {
    pub fn hot_temperature(value: f32) -> Self {
        Self::new(
            "Hot reservoir (K)",
            MIN_HOT_KELVIN,
            MAX_HOT_KELVIN,
            KELVIN_STEP,
            0,
            HOT_Y,
            value,
        )
    }

    pub fn cold_temperature(value: f32) -> Self {
        Self::new(
            "Cold reservoir (K)",
            MIN_COLD_KELVIN,
            MAX_COLD_KELVIN,
            KELVIN_STEP,
            0,
            COLD_Y,
            value,
        )
    }

    pub fn concentration(value: f32) -> Self {
        Self::new(
            "Helium (mol/L)",
            MIN_INITIAL_CONCENTRATION_MOL_PER_L,
            MAX_INITIAL_CONCENTRATION_MOL_PER_L,
            MOLARITY_STEP,
            3,
            CONCENTRATION_Y,
            value,
        )
    }

    fn new(
        label: &'static str,
        min: f32,
        max: f32,
        step: f32,
        precision: usize,
        y: f32,
        value: f32,
    ) -> Self {
        assert!(min < max && step > 0.0);
        let mut slider = Self {
            label,
            min,
            max,
            step,
            precision,
            y,
            value: min,
            dragging: false,
        };
        slider.set_value(value);
        slider
    }

    fn quantized(&self, value: f32) -> f32 {
        (self.min + ((value.clamp(self.min, self.max) - self.min) / self.step).round() * self.step)
            .clamp(self.min, self.max)
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn set_value(&mut self, value: f32) {
        self.value = self.quantized(value);
        self.dragging = false;
    }

    fn origin_x(&self) -> f32 {
        control_left(screen_width())
    }

    fn pointer_over_track(&self) -> bool {
        let (x, y) = mouse_position();
        let origin = self.origin_x();
        (origin - SLIDER_HIT_MARGIN_X..=origin + SLIDER_WIDTH + SLIDER_HIT_MARGIN_X).contains(&x)
            && (self.y - SLIDER_HIT_HALF_HEIGHT..=self.y + SLIDER_HIT_HALF_HEIGHT).contains(&y)
    }

    fn pointer_over_panel(&self) -> bool {
        let (x, y) = mouse_position();
        let origin = self.origin_x();
        (origin - CONTROL_PADDING_X..=origin + SLIDER_WIDTH + CONTROL_PADDING_X).contains(&x)
            && (self.y - PANEL_PADDING_TOP..=self.y + PANEL_PADDING_BOTTOM).contains(&y)
    }

    fn value_at_x(&self, x: f32, origin: f32) -> f32 {
        self.quantized(
            self.min + ((x - origin) / SLIDER_WIDTH).clamp(0.0, 1.0) * (self.max - self.min),
        )
    }

    /// Handles mouse input. A drag starts only on the track; the value
    /// follows the pointer until the button is released.
    pub fn update(&mut self) -> SliderUpdate {
        if !is_mouse_button_down(MouseButton::Left) {
            let released = self.dragging;
            self.dragging = false;
            return SliderUpdate {
                released,
                ..Default::default()
            };
        }
        if is_mouse_button_pressed(MouseButton::Left) && self.pointer_over_track() {
            self.dragging = true;
        }
        if !self.dragging {
            return SliderUpdate::default();
        }
        let (x, _) = mouse_position();
        let next = self.value_at_x(x, self.origin_x());
        let changed = next != self.value;
        self.value = next;
        SliderUpdate {
            changed,
            released: false,
        }
    }

    /// Whether the camera should ignore the mouse: dragging, or pointer over
    /// this slider's panel.
    pub fn blocks_camera(&self) -> bool {
        self.dragging || self.pointer_over_panel()
    }

    pub fn draw(&self) {
        let x = self.origin_x();
        let progress = (self.value - self.min) / (self.max - self.min);
        let handle_x = x + progress * SLIDER_WIDTH;
        draw_rectangle(
            x - CONTROL_PADDING_X,
            self.y - PANEL_PADDING_TOP,
            SLIDER_WIDTH + 2.0 * CONTROL_PADDING_X,
            PANEL_PADDING_TOP + PANEL_PADDING_BOTTOM,
            PANEL_COLOR,
        );
        draw_text(
            format!("{}: {:.*}", self.label, self.precision, self.value),
            x,
            self.y - LABEL_OFFSET_Y,
            LABEL_SIZE,
            WHITE,
        );
        draw_rectangle(
            x,
            self.y - SLIDER_TRACK_HEIGHT / 2.0,
            SLIDER_WIDTH,
            SLIDER_TRACK_HEIGHT,
            TRACK_COLOR,
        );
        draw_rectangle(
            x,
            self.y - SLIDER_TRACK_HEIGHT / 2.0,
            handle_x - x,
            SLIDER_TRACK_HEIGHT,
            ORANGE,
        );
        draw_circle(handle_x, self.y, SLIDER_HANDLE_RADIUS, WHITE);
        let min = format!("{:.*}", self.precision, self.min);
        let max = format!("{:.*}", self.precision, self.max);
        draw_text(
            &min,
            x,
            self.y + RANGE_OFFSET_Y,
            RANGE_SIZE,
            RANGE_TEXT_COLOR,
        );
        let max_width = measure_text(&max, None, RANGE_SIZE as u16, 1.0).width;
        draw_text(
            &max,
            x + SLIDER_WIDTH - max_width,
            self.y + RANGE_OFFSET_Y,
            RANGE_SIZE,
            RANGE_TEXT_COLOR,
        );
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/slider.rs"]
mod tests;
