//! Hot and cold reservoir temperature sliders and the helium concentration
//! slider.

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

use crate::gas::{
    MAX_HOT_KELVIN,
    MAX_INITIAL_CONCENTRATION_MOL_PER_L,
    MIN_HOT_KELVIN,
    MIN_INITIAL_CONCENTRATION_MOL_PER_L, //
};

const SLIDER_WIDTH: f32 = 260.0;
const SLIDER_RIGHT_MARGIN: f32 = 32.0;
const SLIDER_TRACK_HEIGHT: f32 = 8.0;
const SLIDER_HANDLE_RADIUS: f32 = 11.0;
const SLIDER_HIT_HALF_HEIGHT: f32 = 24.0;
const SLIDER_HIT_MARGIN_X: f32 = 14.0;
const PANEL_PADDING_X: f32 = 18.0;
pub(crate) const PANEL_PADDING_TOP: f32 = 52.0;
const PANEL_PADDING_BOTTOM: f32 = 43.0;
pub(crate) const HOT_Y: f32 = 200.0;
const COLD_Y: f32 = HOT_Y + 120.0;
const CONCENTRATION_Y: f32 = COLD_Y + 120.0;
const MOLARITY_INCREMENT: f32 = 0.005;
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
            1.0,
            0,
            HOT_Y,
            value,
        )
    }

    pub fn cold_temperature(value: f32) -> Self {
        const KELVIN_STEP: f32 = 1.0;
        Self::new(
            "Cold reservoir (K)",
            crate::gas::MIN_COLD_KELVIN,
            crate::gas::MAX_COLD_KELVIN,
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
            MOLARITY_INCREMENT,
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
        screen_width() - SLIDER_WIDTH - SLIDER_RIGHT_MARGIN
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
        (origin - PANEL_PADDING_X..=origin + SLIDER_WIDTH + PANEL_PADDING_X).contains(&x)
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
            x - PANEL_PADDING_X,
            self.y - PANEL_PADDING_TOP,
            SLIDER_WIDTH + 2.0 * PANEL_PADDING_X,
            PANEL_PADDING_TOP + PANEL_PADDING_BOTTOM,
            PANEL_COLOR,
        );
        draw_text(
            format!("{}: {:.*}", self.label, self.precision, self.value),
            x,
            self.y - 21.0,
            21.0,
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
        draw_text(&min, x, self.y + 36.0, 17.0, RANGE_TEXT_COLOR);
        let max_width = measure_text(&max, None, 17, 1.0).width;
        draw_text(
            &max,
            x + SLIDER_WIDTH - max_width,
            self.y + 36.0,
            17.0,
            RANGE_TEXT_COLOR,
        );
    }
}

#[cfg(test)]
#[path = "../tests/unit/slider.rs"]
mod tests;
