//! The on-screen controls. Panel, Keys and scenery toggles at the top left
//! of the uncovered scene; a row of Reset, Pause, Slow and Tracers centered along
//! the bottom, with a large Start button above it whenever pressing it
//! would help; and in the column at the right, the hot, cold and helium
//! sliders. None of them sits on a background of its own, so the scene
//! shows around them.

use macroquad::prelude::{
    Color,
    MouseButton,
    Rect,
    Vec2,
    is_mouse_button_down,
    is_mouse_button_pressed,
    mouse_position, //
};

use crate::ui::button::{
    self,
    LABEL_SIZE,
    Label,
    Look, //
};
use crate::ui::layout::{
    CONTROL_WIDTH,
    control_left, //
};
use crate::ui::pen::Pen;
use crate::ui::slider::{
    self,
    Slider, //
};

/// Inset of the toggles and the bottom row from the free part's edges.
pub const MARGIN: f32 = 16.0;
const TOGGLE_HEIGHT: f32 = 30.0;
const TOGGLE_LABEL_SIZE: f32 = 15.0;
const ROW_HEIGHT: f32 = 36.0;
const ROW_GAP: f32 = 10.0;
/// Bottom row's distance from the bottom of the window.
const ROW_BOTTOM_MARGIN: f32 = 18.0;
const START_HEIGHT: f32 = 48.0;
const START_LABEL_SIZE: f32 = 21.0;
const START_EXTRA_WIDTH: f32 = 40.0;
/// Gap between the Start button and the row below it.
const START_GAP: f32 = 14.0;
/// Smallest label size the bottom row shrinks to on a narrow scene.
const MIN_ROW_LABEL_SIZE: f32 = 12.0;
/// The tip over Start when the sides are at one temperature, and its gap.
const NO_GAP_TIP: &str = "Tip: drag Hot side up first, or the flywheel will run down.";
const TIP_SIZE: f32 = 15.0;
const TIP_GAP: f32 = 12.0;
const TIP_COLOR: Color = Color::from_rgba(200, 212, 225, 255);
/// The shade behind the sliders and other loose text over the sky.
pub const BACKING: Color = Color::new(0.06, 0.09, 0.15, 0.62);
pub const BACKING_RADIUS: f32 = 12.0;

/// On-screen button actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    TogglePanel,
    ToggleKeys,
    ToggleScenery,
    ResetGas,
    TogglePause,
    ToggleSlow,
    ToggleTracers,
    Start,
}

const TOGGLES: [Action; 3] = [
    Action::TogglePanel,
    Action::ToggleKeys,
    Action::ToggleScenery,
];
const ROW: [Action; 4] = [
    Action::ResetGas,
    Action::TogglePause,
    Action::ToggleSlow,
    Action::ToggleTracers,
];

/// The settings that change which buttons show and how they look.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonStates {
    pub paused: bool,
    pub slow: bool,
    pub tracers: bool,
    pub panel: bool,
    pub keys: bool,
    /// Whether Start is offered (see `StartPrompt`).
    pub start: bool,
    /// Both sides are at the same temperature, so a tip shows over Start.
    pub no_gap: bool,
    /// The sky scenery is shown (the grid otherwise).
    pub sky: bool,
}

impl ButtonStates {
    /// Every setting on, which gives each button its other label.
    pub const ALL_ON: Self = Self {
        paused: true,
        slow: true,
        tracers: true,
        panel: true,
        keys: true,
        start: true,
        no_gap: true,
        sky: true,
    };
}

impl Action {
    /// Button text, with the key that does the same.
    pub fn label(self, states: ButtonStates) -> Label {
        let (text, key) = match self {
            Self::TogglePanel if states.panel => ("Hide panel", "G"),
            Self::TogglePanel => ("Show panel", "G"),
            Self::ToggleKeys => ("Keys", "H"),
            Self::ToggleScenery if states.sky => ("Grid view", "V"),
            Self::ToggleScenery => ("Sky view", "V"),
            Self::ResetGas => ("Reset", "T"),
            Self::TogglePause if states.paused => ("Resume", "Space"),
            Self::TogglePause => ("Pause", "Space"),
            Self::ToggleSlow => ("Slow", "L"),
            Self::ToggleTracers => ("Tracers", "F"),
            Self::Start => ("Start the engine", "Enter"),
        };
        Label { text, key }
    }

    fn look(self, states: ButtonStates) -> Look {
        let on = match self {
            Self::ToggleKeys => states.keys,
            Self::TogglePause => states.paused,
            Self::ToggleSlow => states.slow,
            Self::ToggleTracers => states.tracers,
            Self::Start => return Look::Primary,
            Self::TogglePanel | Self::ToggleScenery | Self::ResetGas => false,
        };
        if on { Look::On } else { Look::Plain }
    }
}

/// Where things go in the scene, in screen points (see `layout`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneArea {
    /// Left edge of the uncovered scene, where the toggles start.
    pub edge: f32,
    /// The centered span for the bottom row and Start.
    pub left: f32,
    pub width: f32,
    pub height: f32,
}

impl SceneArea {
    /// Right edge of the centered span.
    pub fn right(&self) -> f32 {
        self.left + self.width
    }
}

/// One button placed on screen.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    pub action: Action,
    pub rect: Rect,
    pub label: Label,
    pub look: Look,
    pub size: f32,
}

/// Every button shown for `states`, placed in `area`.
pub fn place_buttons(pen: &Pen, area: SceneArea, states: ButtonStates) -> Vec<Placed> {
    let mut placed = Vec::new();
    let mut x = area.edge + MARGIN;
    for action in TOGGLES {
        let label = action.label(states);
        let width = widest(pen, action, TOGGLE_LABEL_SIZE);
        placed.push(Placed {
            action,
            rect: Rect::new(x, MARGIN, width, TOGGLE_HEIGHT),
            label,
            look: action.look(states),
            size: TOGGLE_LABEL_SIZE,
        });
        x += width + ROW_GAP;
    }

    // The bottom row, centered, shrinking its labels if the scene is narrow.
    let room = area.width - 2.0 * MARGIN;
    let natural: f32 = ROW
        .iter()
        .map(|&action| widest(pen, action, LABEL_SIZE))
        .sum::<f32>()
        + ROW_GAP * (ROW.len() - 1) as f32;
    let size = (LABEL_SIZE * (room / natural).min(1.0)).max(MIN_ROW_LABEL_SIZE);
    let widths: Vec<f32> = ROW
        .iter()
        .map(|&action| widest(pen, action, size))
        .collect();
    let total = widths.iter().sum::<f32>() + ROW_GAP * (ROW.len() - 1) as f32;
    let row_y = area.height - ROW_BOTTOM_MARGIN - ROW_HEIGHT;
    let mut x = area.left + (area.width - total) / 2.0;
    for (&action, width) in ROW.iter().zip(widths) {
        placed.push(Placed {
            action,
            rect: Rect::new(x, row_y, width, ROW_HEIGHT),
            label: action.label(states),
            look: action.look(states),
            size,
        });
        x += width + ROW_GAP;
    }

    if states.start {
        let label = Action::Start.label(states);
        let width = button::width(pen, label, START_LABEL_SIZE) + START_EXTRA_WIDTH;
        placed.push(Placed {
            action: Action::Start,
            rect: Rect::new(
                area.left + (area.width - width) / 2.0,
                row_y - START_GAP - START_HEIGHT,
                width,
                START_HEIGHT,
            ),
            label,
            look: Look::Primary,
            size: START_LABEL_SIZE,
        });
    }
    placed
}

/// The area behind the sliders for a window `width` points wide.
pub fn slider_column(width: f32) -> Rect {
    let left = control_left(width) - BACKING_PAD;
    let (top, bottom) = slider::column_span();
    Rect::new(
        left,
        top - BACKING_PAD,
        CONTROL_WIDTH + 2.0 * BACKING_PAD,
        bottom - top + 2.0 * BACKING_PAD,
    )
}

/// How far the shade reaches past what it is behind.
const BACKING_PAD: f32 = 14.0;

/// Width that fits the action's label in every state, so a button does not
/// change size when its label changes.
fn widest(pen: &Pen, action: Action, size: f32) -> f32 {
    [ButtonStates::default(), ButtonStates::ALL_ON]
        .map(|states| button::width(pen, action.label(states), size))
        .into_iter()
        .fold(0.0, f32::max)
}

/// Bottom edge of the toggles, where a menu under them can start.
pub const TOGGLES_BOTTOM: f32 = MARGIN + TOGGLE_HEIGHT;

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
    /// The button under the pointer, if any.
    hovered: Option<Action>,
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
            hovered: None,
            button_gesture: false,
        }
    }

    /// Reads the mouse for the buttons shown for `states` in `area`, and
    /// for the sliders.
    pub fn update(&mut self, area: SceneArea, states: ButtonStates) -> ControlEvents {
        let pointer = Vec2::from(mouse_position());
        self.hovered = place_buttons(&Pen::screen(), area, states)
            .into_iter()
            .find(|button| button.rect.contains(pointer))
            .map(|button| button.action);
        let action = self
            .hovered
            .filter(|_| is_mouse_button_pressed(MouseButton::Left));
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
        self.hovered.is_some()
            || self.button_gesture
            || self.hot.blocks_camera()
            || self.cold.blocks_camera()
            || self.concentration.blocks_camera()
    }

    pub fn draw(&self, pen: &mut Pen, area: SceneArea, states: ButtonStates) {
        for placed in place_buttons(pen, area, states) {
            if placed.action == Action::Start && states.no_gap {
                let size = pen.fitted_size(NO_GAP_TIP, area.width - 2.0 * MARGIN, TIP_SIZE);
                let width = pen.measure(NO_GAP_TIP, size);
                let x = area.left + (area.width - width) / 2.0;
                pen.text(NO_GAP_TIP, x, placed.rect.y - TIP_GAP, size, TIP_COLOR);
            }
            button::draw(
                pen,
                placed.rect,
                placed.label,
                placed.look,
                self.hovered == Some(placed.action),
                placed.size,
            );
        }
        if states.sky {
            // Over a bright sky, white labels need a shade behind them.
            let column = slider_column(macroquad::prelude::screen_width());
            pen.round_rect(
                column.x,
                column.y,
                column.w,
                column.h,
                BACKING_RADIUS,
                BACKING,
            );
        }
        self.hot.draw(pen);
        self.cold.draw(pen);
        self.concentration.draw(pen);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/controls.rs"]
mod tests;
