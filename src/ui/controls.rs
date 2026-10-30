//! The right-hand column: six buttons above the hot, cold and helium
//! sliders.

use macroquad::prelude::{
    Color,
    MouseButton,
    Rect,
    Vec2,
    WHITE,
    is_mouse_button_down,
    is_mouse_button_pressed,
    mouse_position,
    screen_width, //
};

use crate::ui::layout::{
    CONTROL_WIDTH,
    control_left, //
};
use crate::ui::pen::Pen;
use crate::ui::slider::Slider;

const CONTROL_TOP: f32 = 24.0;
const BUTTON_HEIGHT: f32 = 36.0;
const BUTTON_GAP: f32 = 10.0;
const BUTTON_GAP_X: f32 = 10.0;
const BUTTON_FONT_SIZE: f32 = 18.0;
/// Room kept between a button's label and its edges.
const BUTTON_TEXT_PADDING: f32 = 8.0;
const BUTTON_WIDTH: f32 = (CONTROL_WIDTH - BUTTON_GAP_X) / 2.0;
const BUTTON_COLUMNS: usize = 2;
const BUTTON_COLOR: Color = Color::from_rgba(38, 52, 68, 255);

/// On-screen button actions, in button order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    ResetGas,
    TogglePause,
    Start,
    ToggleDetails,
    ToggleSlow,
    ToggleTracers,
}

const BUTTON_ACTIONS: [Action; 6] = [
    Action::ResetGas,
    Action::TogglePause,
    Action::Start,
    Action::ToggleDetails,
    Action::ToggleSlow,
    Action::ToggleTracers,
];

/// The on/off settings that change button labels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonStates {
    pub paused: bool,
    pub details: bool,
    pub slow: bool,
    pub tracers: bool,
}

impl ButtonStates {
    /// Every setting on, which gives each button its other label.
    pub const ALL_ON: Self = Self {
        paused: true,
        details: true,
        slow: true,
        tracers: true,
    };
}

impl Action {
    /// Button label, which names the key that does the same.
    fn label(self, states: ButtonStates) -> &'static str {
        match self {
            Self::ResetGas => "Reset (T)",
            Self::TogglePause if states.paused => "Resume (Space)",
            Self::TogglePause => "Pause (Space)",
            Self::Start => "Start (Enter)",
            Self::ToggleDetails if states.details => "Less (I)",
            Self::ToggleDetails => "Details (I)",
            Self::ToggleSlow if states.slow => "Normal (L)",
            Self::ToggleSlow => "Slow (L)",
            Self::ToggleTracers if states.tracers => "No tracers (F)",
            Self::ToggleTracers => "Tracers (F)",
        }
    }
}

/// Button `index` in a two-column grid at the top of the control column.
fn button_rect(index: usize, screen_width: f32) -> Rect {
    let column = (index % BUTTON_COLUMNS) as f32;
    let row = (index / BUTTON_COLUMNS) as f32;
    Rect::new(
        control_left(screen_width) + column * (BUTTON_WIDTH + BUTTON_GAP_X),
        CONTROL_TOP + row * (BUTTON_HEIGHT + BUTTON_GAP),
        BUTTON_WIDTH,
        BUTTON_HEIGHT,
    )
}

/// What the controls did this frame. Slider values are reported when a
/// temperature changes and when the helium slider is released, since a
/// new fill restarts the gas.
#[derive(Clone, Copy, Debug, Default)]
pub struct ControlEvents {
    pub action: Option<Action>,
    pub hot_kelvin: Option<f32>,
    pub cold_kelvin: Option<f32>,
    pub concentration: Option<f32>,
}

pub struct Controls {
    hot: Slider,
    cold: Slider,
    concentration: Slider,
    /// The pointer is over a button.
    over_button: bool,
    /// A press that started on a button is still held, so dragging off the
    /// button does not turn the camera.
    button_gesture: bool,
}

impl Controls {
    pub fn new(hot_kelvin: f32, cold_kelvin: f32, concentration: f32) -> Self {
        Self {
            hot: Slider::hot_temperature(hot_kelvin),
            cold: Slider::cold_temperature(cold_kelvin),
            concentration: Slider::concentration(concentration),
            over_button: false,
            button_gesture: false,
        }
    }

    /// Reads the mouse for the buttons and sliders.
    pub fn update(&mut self) -> ControlEvents {
        let pointer = Vec2::from(mouse_position());
        let hovered = (0..BUTTON_ACTIONS.len())
            .find(|&index| button_rect(index, screen_width()).contains(pointer))
            .map(|index| BUTTON_ACTIONS[index]);
        let action = hovered.filter(|_| is_mouse_button_pressed(MouseButton::Left));
        self.over_button = hovered.is_some();
        if !is_mouse_button_down(MouseButton::Left) {
            self.button_gesture = false;
        }
        self.button_gesture |= action.is_some();

        let hot = self.hot.update();
        let cold = self.cold.update();
        let concentration = self.concentration.update();
        ControlEvents {
            action,
            hot_kelvin: hot.changed.then(|| self.hot.value()),
            cold_kelvin: cold.changed.then(|| self.cold.value()),
            concentration: concentration.released.then(|| self.concentration.value()),
        }
    }

    /// Moves the helium slider to a fill chosen another way (keys 1 to 5).
    pub fn set_concentration(&mut self, concentration: f32) {
        self.concentration.set_value(concentration);
    }

    /// Whether the camera should ignore the mouse this frame.
    pub fn blocks_camera(&self) -> bool {
        self.over_button
            || self.button_gesture
            || self.hot.blocks_camera()
            || self.cold.blocks_camera()
            || self.concentration.blocks_camera()
    }

    pub fn draw(&self, states: ButtonStates) {
        let mut pen = Pen::screen();
        let size = button_font_size(&pen);
        for (index, action) in BUTTON_ACTIONS.into_iter().enumerate() {
            let rect = button_rect(index, screen_width());
            let label = action.label(states);
            pen.rect(rect.x, rect.y, rect.w, rect.h, BUTTON_COLOR);
            pen.text(
                label,
                rect.x + (rect.w - pen.measure(label, size)) / 2.0,
                rect.y + (rect.h + pen.text_height(label, size)) / 2.0,
                size,
                WHITE,
            );
        }
        self.hot.draw();
        self.cold.draw();
        self.concentration.draw();
    }
}

/// One font size for every button label, small enough that the longest
/// label in any state fits, so the buttons match.
fn button_font_size(pen: &Pen) -> f32 {
    let width = BUTTON_WIDTH - 2.0 * BUTTON_TEXT_PADDING;
    BUTTON_ACTIONS
        .into_iter()
        .flat_map(|action| {
            [
                action.label(ButtonStates::default()),
                action.label(ButtonStates::ALL_ON),
            ]
        })
        .map(|label| pen.fitted_size(label, width, BUTTON_FONT_SIZE))
        .fold(BUTTON_FONT_SIZE, f32::min)
}

#[cfg(test)]
#[path = "../../tests/unit/ui/controls.rs"]
mod tests;
