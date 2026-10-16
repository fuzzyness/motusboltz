//! Reusable screen-space sliders and simulation control buttons.

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

use crate::piston::MIN_VOLUME_FRACTION;
use crate::simulation::PARTICLE_COUNT_PRESETS;

/// Lowest selectable kinetic temperature in reduced simulation units.
const MIN_TEMPERATURE: f32 = 0.1;
/// Highest selectable kinetic temperature in reduced simulation units.
const MAX_TEMPERATURE: f32 = 4.0;

const SLIDER_WIDTH: f32 = 260.0;
const SLIDER_RIGHT_MARGIN: f32 = 32.0;
const CONTROLS_TOP_MARGIN: f32 = 24.0;
const SLIDER_VERTICAL_SPACING: f32 = 120.0;
const BUTTON_HEIGHT: f32 = 40.0;
const BUTTON_VERTICAL_GAP: f32 = 12.0;
const BUTTON_TO_SLIDER_GAP: f32 = 8.0;
pub const VOLUME_PERCENT_SCALE: f32 = 100.0;
const VOLUME_PERCENT_INCREMENT: f32 = 1.0;
const SLIDER_TRACK_HEIGHT: f32 = 8.0;
const SLIDER_HANDLE_RADIUS: f32 = 11.0;
const SLIDER_HIT_HALF_HEIGHT: f32 = 24.0;
const SLIDER_HIT_MARGIN_X: f32 = 14.0;

const PANEL_PADDING_X: f32 = 18.0;
const PANEL_PADDING_TOP: f32 = 52.0;
const PANEL_PADDING_BOTTOM: f32 = 43.0;
const STATS_BUTTON_Y: f32 = CONTROLS_TOP_MARGIN;
const MECHANISM_BUTTON_Y: f32 = STATS_BUTTON_Y + BUTTON_HEIGHT + BUTTON_VERTICAL_GAP;
const TEMPERATURE_TRACK_Y: f32 =
    MECHANISM_BUTTON_Y + BUTTON_HEIGHT + BUTTON_TO_SLIDER_GAP + PANEL_PADDING_TOP;
const PARTICLES_TRACK_Y: f32 = TEMPERATURE_TRACK_Y + SLIDER_VERTICAL_SPACING;
const VOLUME_TRACK_Y: f32 = PARTICLES_TRACK_Y + SLIDER_VERTICAL_SPACING;
const LABEL_OFFSET_Y: f32 = 21.0;
const RANGE_LABEL_OFFSET_Y: f32 = 36.0;
const LABEL_FONT_SIZE: f32 = 21.0;
const RANGE_FONT_SIZE: f32 = 17.0;
const TEMPERATURE_PRECISION: usize = 2;
const TEMPERATURE_RANGE_PRECISION: usize = 1;
const WHOLE_NUMBER_PRECISION: usize = 0;
const PARTICLE_COUNT_INCREMENT: f32 = 1.0;

// Stats sits above the row containing the mode and starter buttons.
const BUTTON_GAP_X: f32 = 10.0;
const BUTTON_FONT_SIZE: f32 = 18.0;
const BUTTON_WIDTH: f32 = (SLIDER_WIDTH - BUTTON_GAP_X) / 2.0;

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

#[derive(Clone, Copy)]
struct SliderConfig {
    label: &'static str,
    minimum: f32,
    maximum: f32,
    step: Option<f32>,
    precision: usize,
    range_precision: usize,
    track_y: f32,
}

const TEMPERATURE_CONFIG: SliderConfig = SliderConfig {
    label: "Set temperature",
    minimum: MIN_TEMPERATURE,
    maximum: MAX_TEMPERATURE,
    step: None,
    precision: TEMPERATURE_PRECISION,
    range_precision: TEMPERATURE_RANGE_PRECISION,
    track_y: TEMPERATURE_TRACK_Y,
};

const VOLUME_CONFIG: SliderConfig = SliderConfig {
    label: "Set volume (%)",
    minimum: MIN_VOLUME_FRACTION * VOLUME_PERCENT_SCALE,
    maximum: VOLUME_PERCENT_SCALE,
    step: Some(VOLUME_PERCENT_INCREMENT),
    precision: WHOLE_NUMBER_PRECISION,
    range_precision: WHOLE_NUMBER_PRECISION,
    track_y: VOLUME_TRACK_Y,
};

const PARTICLES_CONFIG: SliderConfig = SliderConfig {
    label: "Particles",
    minimum: PARTICLE_COUNT_PRESETS[0] as f32,
    maximum: PARTICLE_COUNT_PRESETS[PARTICLE_COUNT_PRESETS.len() - 1] as f32,
    step: Some(PARTICLE_COUNT_INCREMENT),
    precision: WHOLE_NUMBER_PRECISION,
    range_precision: WHOLE_NUMBER_PRECISION,
    track_y: PARTICLES_TRACK_Y,
};

/// Pointer events reported by a slider during a frame.
#[derive(Default)]
pub struct SliderUpdate {
    pub changed: bool,
    pub released: bool,
}

/// One draggable slider with its own value and screen-space hit region.
///
/// Call `update()` before camera input and use `blocks_camera()` to prevent
/// dragging a control from also orbiting or panning the chamber.
pub struct Slider {
    config: SliderConfig,
    value: f32,
    dragging: bool,
}

impl Slider {
    /// Creates an immediately applied kinetic-temperature control.
    pub fn temperature(initial_temperature: f32) -> Self {
        Self::new(TEMPERATURE_CONFIG, initial_temperature)
    }

    /// Creates a whole-particle-count control applied on mouse release.
    pub fn particles(initial_count: usize) -> Self {
        Self::new(PARTICLES_CONFIG, initial_count as f32)
    }

    /// Creates a whole-percent target volume control applied on release.
    pub fn volume(initial_percent: f32) -> Self {
        Self::new(VOLUME_CONFIG, initial_percent)
    }

    fn new(config: SliderConfig, value: f32) -> Self {
        assert!(config.maximum > config.minimum);

        Self {
            value: Self::clamp_value(config, value),
            config,
            dragging: false,
        }
    }

    fn clamp_value(config: SliderConfig, value: f32) -> f32 {
        let value = value.clamp(config.minimum, config.maximum);
        let Some(step) = config.step else {
            return value;
        };

        (config.minimum + ((value - config.minimum) / step).round() * step)
            .clamp(config.minimum, config.maximum)
    }

    /// Returns the selected value, which may still be pending mouse release.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Synchronizes the control with a keyboard-selected particle preset.
    pub fn set_value(&mut self, value: f32) {
        self.value = Self::clamp_value(self.config, value);
        self.dragging = false;
    }

    /// Processes pointer input without allocating or modifying the simulation.
    ///
    /// `changed` is true when the selected value changes while dragging.
    /// `released` is true on the frame an active drag ends, even if the cursor
    /// leaves the slider. The caller chooses when to apply the new value.
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

        let (mouse_x, _) = mouse_position();
        let (track_x, _) = self.track_origin();
        let new_value = self.value_at_x(mouse_x, track_x);
        let changed = new_value != self.value;
        self.value = new_value;

        SliderUpdate {
            changed,
            released: false,
        }
    }

    /// Keeps pointer gestures over the panel away from the orbit camera.
    pub fn blocks_camera(&self) -> bool {
        self.dragging || self.pointer_over_panel()
    }

    fn track_origin(&self) -> (f32, f32) {
        (
            screen_width() - SLIDER_WIDTH - SLIDER_RIGHT_MARGIN,
            self.config.track_y,
        )
    }

    fn pointer_over_track(&self) -> bool {
        let (x, y) = mouse_position();
        let (track_x, track_y) = self.track_origin();

        (track_x - SLIDER_HIT_MARGIN_X..=track_x + SLIDER_WIDTH + SLIDER_HIT_MARGIN_X).contains(&x)
            && (track_y - SLIDER_HIT_HALF_HEIGHT..=track_y + SLIDER_HIT_HALF_HEIGHT).contains(&y)
    }

    fn pointer_over_panel(&self) -> bool {
        let (x, y) = mouse_position();
        let (track_x, track_y) = self.track_origin();

        (track_x - PANEL_PADDING_X..=track_x + SLIDER_WIDTH + PANEL_PADDING_X).contains(&x)
            && (track_y - PANEL_PADDING_TOP..=track_y + PANEL_PADDING_BOTTOM).contains(&y)
    }

    fn value_at_x(&self, pointer_x: f32, track_x: f32) -> f32 {
        let progress = ((pointer_x - track_x) / SLIDER_WIDTH).clamp(0.0, 1.0);
        let value = self.config.minimum + progress * (self.config.maximum - self.config.minimum);
        Self::clamp_value(self.config, value)
    }

    /// Draws the control after restoring Macroquad's default 2D camera.
    pub fn draw(&self) {
        let (track_x, track_y) = self.track_origin();
        let progress =
            (self.value - self.config.minimum) / (self.config.maximum - self.config.minimum);
        let handle_x = track_x + progress * SLIDER_WIDTH;

        draw_rectangle(
            track_x - PANEL_PADDING_X,
            track_y - PANEL_PADDING_TOP,
            SLIDER_WIDTH + 2.0 * PANEL_PADDING_X,
            PANEL_PADDING_TOP + PANEL_PADDING_BOTTOM,
            PANEL_COLOR,
        );

        let label = format!(
            "{}: {:.*}",
            self.config.label, self.config.precision, self.value
        );
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

        let minimum = format!("{:.*}", self.config.range_precision, self.config.minimum);
        let maximum = format!("{:.*}", self.config.range_precision, self.config.maximum);
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

/// An action produced by the on-screen control buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlAction {
    SwitchMode,
    StartCrank,
    ToggleStats,
}

/// Stats above the mechanism buttons, all anchored to the top-right corner.
///
/// The starter is disabled in manual mode; keyboard shortcuts remain available.
#[derive(Default)]
pub struct ControlButtons {
    pressed: bool,
}

impl ControlButtons {
    fn origin() -> (f32, f32) {
        (
            screen_width() - SLIDER_WIDTH - SLIDER_RIGHT_MARGIN,
            CONTROLS_TOP_MARGIN,
        )
    }

    fn contains(x: f32, y: f32, bx: f32, by: f32, width: f32) -> bool {
        (bx..=bx + width).contains(&x) && (by..=by + BUTTON_HEIGHT).contains(&y)
    }

    fn action_at(
        x: f32,
        y: f32,
        origin_x: f32,
        origin_y: f32,
        crank_active: bool,
    ) -> Option<ControlAction> {
        let mechanism_y = origin_y + BUTTON_HEIGHT + BUTTON_VERTICAL_GAP;

        if Self::contains(x, y, origin_x, origin_y, SLIDER_WIDTH) {
            Some(ControlAction::ToggleStats)
        } else if Self::contains(x, y, origin_x, mechanism_y, BUTTON_WIDTH) {
            Some(ControlAction::SwitchMode)
        } else if crank_active
            && Self::contains(
                x,
                y,
                origin_x + BUTTON_WIDTH + BUTTON_GAP_X,
                mechanism_y,
                BUTTON_WIDTH,
            )
        {
            Some(ControlAction::StartCrank)
        } else {
            None
        }
    }

    /// Handles a click once and reserves the gesture from camera controls.
    pub fn update(&mut self, crank_active: bool) -> Option<ControlAction> {
        if !is_mouse_button_down(MouseButton::Left) {
            self.pressed = false;
        }

        if !is_mouse_button_pressed(MouseButton::Left) {
            return None;
        }

        let (x, y) = mouse_position();
        let (origin_x, origin_y) = Self::origin();
        let action = Self::action_at(x, y, origin_x, origin_y, crank_active);
        if action.is_some() {
            self.pressed = true;
        }
        action
    }

    /// Prevents button clicks and drags from orbiting the chamber.
    pub fn blocks_camera(&self) -> bool {
        let (x, y) = mouse_position();
        let (origin_x, origin_y) = Self::origin();
        self.pressed
            || Self::contains(x, y, origin_x, origin_y, SLIDER_WIDTH)
            || Self::contains(x, y, origin_x, MECHANISM_BUTTON_Y, BUTTON_WIDTH)
            || Self::contains(
                x,
                y,
                origin_x + BUTTON_WIDTH + BUTTON_GAP_X,
                MECHANISM_BUTTON_Y,
                BUTTON_WIDTH,
            )
    }

    fn draw_button(x: f32, y: f32, width: f32, label: &str, enabled: bool) {
        let background = if enabled { TRACK_COLOR } else { PANEL_COLOR };
        let foreground = if enabled { WHITE } else { RANGE_TEXT_COLOR };

        draw_rectangle(x, y, width, BUTTON_HEIGHT, background);
        let text_size = measure_text(label, None, BUTTON_FONT_SIZE as u16, 1.0);
        draw_text(
            label,
            x + (width - text_size.width) / 2.0,
            y + (BUTTON_HEIGHT + text_size.height) / 2.0,
            BUTTON_FONT_SIZE,
            foreground,
        );
    }

    /// Draws mechanism actions and the diagnostics visibility toggle.
    pub fn draw(&self, crank_active: bool, show_stats: bool) {
        let (x, _) = Self::origin();
        let stats_label = if show_stats { "Hide stats" } else { "Stats" };
        Self::draw_button(x, STATS_BUTTON_Y, SLIDER_WIDTH, stats_label, true);

        let mode_label = if crank_active {
            "Manual mode"
        } else {
            "Crank mode"
        };
        Self::draw_button(x, MECHANISM_BUTTON_Y, BUTTON_WIDTH, mode_label, true);
        Self::draw_button(
            x + BUTTON_WIDTH + BUTTON_GAP_X,
            MECHANISM_BUTTON_Y,
            BUTTON_WIDTH,
            "Start crank",
            crank_active,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_buttons_are_separate_and_starter_needs_crank_mode() {
        const ORIGIN_X: f32 = 100.0;
        const ORIGIN_Y: f32 = 200.0;

        let mode_x = ORIGIN_X + BUTTON_WIDTH / 2.0;
        let starter_x = ORIGIN_X + BUTTON_WIDTH + BUTTON_GAP_X + BUTTON_WIDTH / 2.0;
        let center_y = ORIGIN_Y + BUTTON_HEIGHT + BUTTON_VERTICAL_GAP + BUTTON_HEIGHT / 2.0;

        assert_eq!(
            ControlButtons::action_at(mode_x, center_y, ORIGIN_X, ORIGIN_Y, false),
            Some(ControlAction::SwitchMode),
        );
        assert_eq!(
            ControlButtons::action_at(starter_x, center_y, ORIGIN_X, ORIGIN_Y, false),
            None,
        );
        assert_eq!(
            ControlButtons::action_at(starter_x, center_y, ORIGIN_X, ORIGIN_Y, true),
            Some(ControlAction::StartCrank),
        );
        assert_eq!(
            ControlButtons::action_at(
                ORIGIN_X + SLIDER_WIDTH / 2.0,
                ORIGIN_Y + BUTTON_HEIGHT / 2.0,
                ORIGIN_X,
                ORIGIN_Y,
                false,
            ),
            Some(ControlAction::ToggleStats),
        );
        assert_eq!(
            ControlButtons::action_at(
                ORIGIN_X + SLIDER_WIDTH / 2.0,
                ORIGIN_Y + BUTTON_HEIGHT / 2.0,
                ORIGIN_X,
                ORIGIN_Y,
                true,
            ),
            Some(ControlAction::ToggleStats),
        );
    }

    const TEST_TOLERANCE: f32 = 1e-5;
    const TEST_TRACK_X: f32 = 100.0;
    const MIDPOINT_FRACTION: f32 = 0.5;
    const TEST_OFFSET: f32 = 100.0;
    const TEST_FRACTION: f32 = 0.373;

    #[test]
    fn temperature_range_and_midpoint_are_preserved() {
        let slider = Slider::temperature(MIN_TEMPERATURE);
        let midpoint = slider.value_at_x(
            TEST_TRACK_X + MIDPOINT_FRACTION * SLIDER_WIDTH,
            TEST_TRACK_X,
        );
        let expected = (MIN_TEMPERATURE + MAX_TEMPERATURE) * MIDPOINT_FRACTION;

        assert!((midpoint - expected).abs() < TEST_TOLERANCE);
        assert_eq!(
            slider.value_at_x(TEST_TRACK_X - TEST_OFFSET, TEST_TRACK_X),
            MIN_TEMPERATURE
        );
        assert_eq!(
            slider.value_at_x(TEST_TRACK_X + SLIDER_WIDTH + TEST_OFFSET, TEST_TRACK_X),
            MAX_TEMPERATURE
        );
        assert_eq!(Slider::temperature(0.0).value(), MIN_TEMPERATURE);
    }

    #[test]
    fn particle_count_is_whole_and_bounded() {
        let mut slider = Slider::particles(PARTICLE_COUNT_PRESETS[0]);
        let value = slider.value_at_x(TEST_TRACK_X + TEST_FRACTION * SLIDER_WIDTH, TEST_TRACK_X);
        assert_eq!(value.fract(), 0.0);
        assert_eq!(
            slider.value_at_x(TEST_TRACK_X - TEST_OFFSET, TEST_TRACK_X),
            PARTICLES_CONFIG.minimum
        );
        assert_eq!(
            slider.value_at_x(TEST_TRACK_X + SLIDER_WIDTH + TEST_OFFSET, TEST_TRACK_X),
            PARTICLES_CONFIG.maximum
        );

        slider.set_value(PARTICLE_COUNT_PRESETS[PARTICLE_COUNT_PRESETS.len() - 1] as f32);
        assert_eq!(slider.value(), PARTICLES_CONFIG.maximum);
    }

    // Controls are top-anchored and the optional volume control is last.
    const _: () = {
        assert!(STATS_BUTTON_Y == CONTROLS_TOP_MARGIN);
        assert!(MECHANISM_BUTTON_Y >= STATS_BUTTON_Y + BUTTON_HEIGHT);
        assert!(
            TEMPERATURE_TRACK_Y - PANEL_PADDING_TOP
                >= MECHANISM_BUTTON_Y + BUTTON_HEIGHT + BUTTON_TO_SLIDER_GAP
        );
        assert!(PARTICLES_TRACK_Y - PANEL_PADDING_TOP > TEMPERATURE_TRACK_Y + PANEL_PADDING_BOTTOM);
        assert!(VOLUME_TRACK_Y - PANEL_PADDING_TOP > PARTICLES_TRACK_Y + PANEL_PADDING_BOTTOM);
    };

    #[test]
    fn volume_is_bounded_to_whole_percentages() {
        let slider = Slider::volume(VOLUME_PERCENT_SCALE);
        let midpoint = slider.value_at_x(
            TEST_TRACK_X + MIDPOINT_FRACTION * SLIDER_WIDTH,
            TEST_TRACK_X,
        );
        assert_eq!(midpoint.fract(), 0.0);
        assert_eq!(slider.value(), VOLUME_PERCENT_SCALE);
        assert_eq!(Slider::volume(0.0).value(), VOLUME_CONFIG.minimum);
    }
}
