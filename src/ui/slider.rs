//! Horizontal slider widget, used for the hot and cold reservoir
//! temperatures and the helium concentration.

use macroquad::prelude::{
    Color,
    MouseButton,
    WHITE,
    is_mouse_button_down,
    is_mouse_button_pressed,
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
use crate::ui::pen::{
    Pen,
    Weight, //
};

const SLIDER_WIDTH: f32 = CONTROL_WIDTH;
const SLIDER_TRACK_HEIGHT: f32 = 6.0;
const SLIDER_HANDLE_RADIUS: f32 = 10.0;
/// The colored ring around the white handle.
const HANDLE_RING: f32 = 3.0;
const SLIDER_HIT_HALF_HEIGHT: f32 = 22.0;
const SLIDER_HIT_MARGIN_X: f32 = 14.0;
/// Area above and below the track that holds the label and the range
/// values; the camera ignores the mouse over it.
pub const AREA_ABOVE: f32 = 40.0;
const AREA_BELOW: f32 = 32.0;
const LABEL_SIZE: f32 = 20.0;
/// Baseline of the label and value above the track.
const LABEL_OFFSET_Y: f32 = 16.0;
const RANGE_SIZE: f32 = 15.0;
/// Baseline of the minimum and maximum values below the track.
const RANGE_OFFSET_Y: f32 = 26.0;
/// Track centers, top to bottom, starting near the top of the column now
/// that the buttons sit in the scene.
pub const HOT_Y: f32 = 68.0;
const SLIDER_SPACING: f32 = 90.0;
const COLD_Y: f32 = HOT_Y + SLIDER_SPACING;
const CONCENTRATION_Y: f32 = COLD_Y + SLIDER_SPACING;
/// The top of the first slider's labels and the bottom of the last one's
/// range, in screen points.
pub fn column_span() -> (f32, f32) {
    (HOT_Y - AREA_ABOVE, CONCENTRATION_Y + AREA_BELOW)
}

const KELVIN_STEP: f32 = 1.0;
const MOLARITY_STEP: f32 = 0.005;
const TRACK_COLOR: Color = Color::new(0.27, 0.33, 0.40, 0.9);
const VALUE_COLOR: Color = WHITE;
const LABEL_COLOR: Color = Color::from_rgba(185, 198, 212, 255);
const RANGE_TEXT_COLOR: Color = Color::from_rgba(120, 138, 158, 255);
const HOT_COLOR: Color = Color::from_rgba(255, 160, 90, 255);
const COLD_COLOR: Color = Color::from_rgba(95, 190, 255, 255);
const HELIUM_COLOR: Color = Color::from_rgba(180, 224, 209, 255);

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
    unit: &'static str,
    color: Color,
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
            ("Hot side", "K", HOT_COLOR),
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
            ("Cold side", "K", COLD_COLOR),
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
            ("Helium", "mol/L", HELIUM_COLOR),
            MIN_INITIAL_CONCENTRATION_MOL_PER_L,
            MAX_INITIAL_CONCENTRATION_MOL_PER_L,
            MOLARITY_STEP,
            3,
            CONCENTRATION_Y,
            value,
        )
    }

    /// A slider named `label`, its values in `unit`, filled with `color`.
    fn new(
        (label, unit, color): (&'static str, &'static str, Color),
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
            unit,
            color,
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
            && (self.y - AREA_ABOVE..=self.y + AREA_BELOW).contains(&y)
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

    /// The value with its unit, as shown beside the label.
    fn value_text(&self, value: f32) -> String {
        format!("{:.*} {}", self.precision, value, self.unit)
    }

    /// Label and value above the track, the range below it, and no
    /// background, so the scene shows around it.
    pub fn draw(&self, pen: &mut Pen) {
        let x = self.origin_x();
        let progress = (self.value - self.min) / (self.max - self.min);
        let handle_x = x + progress * SLIDER_WIDTH;
        let label_y = self.y - LABEL_OFFSET_Y;
        pen.heading(self.label, x, label_y, LABEL_SIZE, LABEL_COLOR);
        let value = self.value_text(self.value);
        let value_width = pen.measure_weight(&value, LABEL_SIZE, Weight::Semibold);
        pen.heading(
            &value,
            x + SLIDER_WIDTH - value_width,
            label_y,
            LABEL_SIZE,
            VALUE_COLOR,
        );
        let track_top = self.y - SLIDER_TRACK_HEIGHT / 2.0;
        let radius = SLIDER_TRACK_HEIGHT / 2.0;
        pen.round_rect(
            x,
            track_top,
            SLIDER_WIDTH,
            SLIDER_TRACK_HEIGHT,
            radius,
            TRACK_COLOR,
        );
        pen.round_rect(
            x,
            track_top,
            handle_x - x,
            SLIDER_TRACK_HEIGHT,
            radius,
            self.color,
        );
        pen.circle(handle_x, self.y, SLIDER_HANDLE_RADIUS, self.color);
        pen.circle(handle_x, self.y, SLIDER_HANDLE_RADIUS - HANDLE_RING, WHITE);
        let range_y = self.y + RANGE_OFFSET_Y;
        pen.text(
            &self.value_text(self.min),
            x,
            range_y,
            RANGE_SIZE,
            RANGE_TEXT_COLOR,
        );
        pen.text_right(
            &self.value_text(self.max),
            x + SLIDER_WIDTH,
            range_y,
            RANGE_SIZE,
            RANGE_TEXT_COLOR,
        );
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/slider.rs"]
mod tests;
