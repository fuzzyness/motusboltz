//! The on-screen controls. Keys and scenery toggles centered at the top; a
//! row of Reset, Start or Pause, Slow and Tracers centered along the
//! bottom, where Start takes Pause's place whenever pressing it would help
//! (Space does whichever is shown); the hot, cold and helium sliders on a
//! card at the right; a hump on that card and on the panel's that closes
//! each, leaving a tab at the window's edge that opens it again (see
//! `drawer`); and Hide all, tucked in the bottom-right corner, which hides
//! everything but itself.

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
    Hue,
    LABEL_SIZE,
    Label,
    Look, //
};
use crate::ui::drawer::{
    self,
    Edge, //
};
use crate::ui::layout::{
    CONTROL_WIDTH,
    EDGE_TAB_WIDTH,
    PANEL_INSET,
    PANEL_WIDTH,
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
/// Hide all, in the bottom-right corner: its height and label size. Its
/// middle lines up with the bottom row's.
pub const CORNER_HEIGHT: f32 = 28.0;
const CORNER_LABEL_SIZE: f32 = 14.0;
/// Smallest label size the bottom row shrinks to on a narrow scene.
const MIN_ROW_LABEL_SIZE: f32 = 12.0;
/// The tip over the row when Start is offered with the sides at one
/// temperature, and its gap.
const NO_GAP_TIP: &str = "Tip: drag Hot side up first, or the flywheel will run down.";
const TIP_SIZE: f32 = 15.0;
const TIP_GAP: f32 = 16.0;
/// How far the shade behind the tip reaches past its text.
const TIP_PAD: f32 = 7.0;
const TIP_COLOR: Color = Color::from_rgba(200, 212, 225, 255);
/// The shade behind the sliders and other loose text over the sky.
pub const BACKING: Color = Color::new(0.06, 0.09, 0.15, 0.62);
pub const BACKING_RADIUS: f32 = 12.0;

/// On-screen button actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    TogglePanel,
    ToggleSliders,
    ToggleKeys,
    ToggleScenery,
    ToggleAll,
    ResetGas,
    TogglePause,
    ToggleSlow,
    ToggleTracers,
    Start,
}

const TOGGLES: [Action; 2] = [Action::ToggleKeys, Action::ToggleScenery];
/// Start and Pause take turns in one place in the bottom row.
const SHARED: [Action; 2] = [Action::Start, Action::TogglePause];

/// The bottom row for `states`: Start while it is offered, Pause otherwise.
fn row(states: ButtonStates) -> [Action; 4] {
    [
        Action::ResetGas,
        if states.start {
            Action::Start
        } else {
            Action::TogglePause
        },
        Action::ToggleSlow,
        Action::ToggleTracers,
    ]
}

/// The settings that change which buttons show and how they look.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonStates {
    pub paused: bool,
    pub slow: bool,
    pub tracers: bool,
    pub panel: bool,
    pub keys: bool,
    /// Whether Start is offered, in Pause's place (see `StartPrompt`).
    pub start: bool,
    /// Both sides are at the same temperature, so a tip shows over Start.
    pub no_gap: bool,
    /// The sky scenery is shown (the grid otherwise).
    pub sky: bool,
    /// The slider column is open.
    pub sliders: bool,
    /// Everything but Hide all is hidden.
    pub hidden: bool,
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
        sliders: true,
        hidden: true,
    };
}

impl Action {
    /// Button text, with the key that does the same.
    pub fn label(self, states: ButtonStates) -> Label {
        let (text, key) = match self {
            Self::TogglePanel => ("Panel", "G"),
            Self::ToggleSliders => ("Sliders", "C"),
            Self::ToggleAll if states.hidden => ("Show all", "U"),
            Self::ToggleAll => ("Hide all", "U"),
            Self::ToggleKeys => ("Keys", "H"),
            Self::ToggleScenery if states.sky => ("Grid view", "V"),
            Self::ToggleScenery => ("Sky view", "V"),
            Self::ResetGas => ("Reset", "T"),
            Self::TogglePause if states.paused => ("Resume", "Space"),
            Self::TogglePause => ("Pause", "Space"),
            Self::ToggleSlow => ("Slow", "L"),
            Self::ToggleTracers => ("Tracers", "F"),
            Self::Start => ("Start", "Space"),
        };
        Label { text, key }
    }

    /// Each button in the bottom row has a color of its own, as on the
    /// Dreamcast's menu; the toggles share a pearl gray. Start always glows,
    /// to draw the eye.
    fn look(self, states: ButtonStates) -> Look {
        let (hue, on) = match self {
            Self::TogglePanel | Self::ToggleSliders | Self::ToggleScenery => (Hue::Pearl, false),
            Self::ToggleAll => (Hue::Pearl, states.hidden),
            Self::ToggleKeys => (Hue::Pearl, states.keys),
            Self::ResetGas => (Hue::Coral, false),
            Self::Start => (Hue::Amber, true),
            Self::TogglePause => (Hue::Mint, states.paused),
            Self::ToggleSlow => (Hue::Cyan, states.slow),
            Self::ToggleTracers => (Hue::Orchid, states.tracers),
        };
        Look { hue, on }
    }
}

/// Where things go in the scene, in screen points (see `layout`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneArea {
    /// Left edge of the uncovered scene, where the corner readout starts.
    pub edge: f32,
    /// The centered span for the toggles and the bottom row.
    pub left: f32,
    pub width: f32,
    /// The whole window.
    pub window_width: f32,
    pub height: f32,
}

impl SceneArea {
    /// Right edge of the centered span.
    pub fn right(&self) -> f32 {
        self.left + self.width
    }
}

/// How a button is drawn: a pill, the hump on an open drawer's card, or
/// the tab a closed drawer leaves at the window's edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Pill,
    Hump(Edge),
    Tab(Edge),
}

/// One button placed on screen.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    pub action: Action,
    pub rect: Rect,
    pub label: Label,
    pub look: Look,
    pub size: f32,
    pub shape: Shape,
}

/// The panel's card in a window `height` points tall.
pub fn panel_card(height: f32) -> Rect {
    Rect::new(
        PANEL_INSET,
        PANEL_INSET,
        PANEL_WIDTH - PANEL_INSET,
        height - 2.0 * PANEL_INSET,
    )
}

/// Hide all, in the bottom-right corner of `area`.
pub fn corner_button(pen: &Pen, area: SceneArea) -> Rect {
    let width = widest(pen, Action::ToggleAll, CORNER_LABEL_SIZE);
    let middle = area.height - ROW_BOTTOM_MARGIN - ROW_HEIGHT / 2.0;
    Rect::new(
        area.window_width - MARGIN - width,
        middle - CORNER_HEIGHT / 2.0,
        width,
        CORNER_HEIGHT,
    )
}

/// The button that closes a drawer on `card` at `edge` (its hump) while
/// it is `open`, or opens it (the tab at the window's edge).
fn drawer_button(
    action: Action,
    states: ButtonStates,
    card: Rect,
    edge: Edge,
    open: bool,
    window_width: f32,
) -> Placed {
    let (rect, shape) = if open {
        (drawer::hump(card, edge), Shape::Hump(edge))
    } else {
        // The tab stays level with the card's middle.
        let middle = card.center().y;
        (
            drawer::tab(edge, window_width, EDGE_TAB_WIDTH, middle),
            Shape::Tab(edge),
        )
    };
    Placed {
        action,
        rect,
        label: action.label(states),
        look: action.look(states),
        size: LABEL_SIZE,
        shape,
    }
}

/// Every button shown for `states`, placed in `area`.
pub fn place_buttons(pen: &Pen, area: SceneArea, states: ButtonStates) -> Vec<Placed> {
    let corner = Placed {
        action: Action::ToggleAll,
        rect: corner_button(pen, area),
        label: Action::ToggleAll.label(states),
        look: Action::ToggleAll.look(states),
        size: CORNER_LABEL_SIZE,
        shape: Shape::Pill,
    };
    if states.hidden {
        return vec![corner];
    }
    let mut placed = vec![
        drawer_button(
            Action::TogglePanel,
            states,
            panel_card(area.height),
            Edge::Left,
            states.panel,
            area.window_width,
        ),
        drawer_button(
            Action::ToggleSliders,
            states,
            slider_column(area.window_width),
            Edge::Right,
            states.sliders,
            area.window_width,
        ),
        corner,
    ];

    // The toggles, centered at the top.
    let widths = TOGGLES.map(|action| widest(pen, action, TOGGLE_LABEL_SIZE));
    let total = widths.iter().sum::<f32>() + ROW_GAP * (TOGGLES.len() - 1) as f32;
    let mut x = area.left + (area.width - total) / 2.0;
    for (action, width) in TOGGLES.into_iter().zip(widths) {
        placed.push(Placed {
            action,
            rect: Rect::new(x, MARGIN, width, TOGGLE_HEIGHT),
            label: action.label(states),
            look: action.look(states),
            size: TOGGLE_LABEL_SIZE,
            shape: Shape::Pill,
        });
        x += width + ROW_GAP;
    }

    // The bottom row, centered, shrinking its labels if the scene is narrow.
    let row = row(states);
    let room = area.width - 2.0 * MARGIN;
    let natural: f32 = row
        .iter()
        .map(|&action| widest(pen, action, LABEL_SIZE))
        .sum::<f32>()
        + ROW_GAP * (row.len() - 1) as f32;
    let size = (LABEL_SIZE * (room / natural).min(1.0)).max(MIN_ROW_LABEL_SIZE);
    let widths: Vec<f32> = row
        .iter()
        .map(|&action| widest(pen, action, size))
        .collect();
    let total = widths.iter().sum::<f32>() + ROW_GAP * (row.len() - 1) as f32;
    let row_y = area.height - ROW_BOTTOM_MARGIN - ROW_HEIGHT;
    let mut x = area.left + (area.width - total) / 2.0;
    for (&action, width) in row.iter().zip(widths) {
        placed.push(Placed {
            action,
            rect: Rect::new(x, row_y, width, ROW_HEIGHT),
            label: action.label(states),
            look: action.look(states),
            size,
            shape: Shape::Pill,
        });
        x += width + ROW_GAP;
    }
    placed
}

/// The sliders' card for a window `width` points wide.
pub fn slider_column(width: f32) -> Rect {
    let left = control_left(width) - CARD_PAD;
    let (top, bottom) = slider::column_span();
    Rect::new(
        left,
        top - CARD_PAD,
        CONTROL_WIDTH + 2.0 * CARD_PAD,
        bottom - top + 2.0 * CARD_PAD,
    )
}

/// How far the sliders' card reaches past the sliders.
const CARD_PAD: f32 = 16.0;

/// Width that fits the action's label in every state, and in the shared
/// place both Start's and Pause's, so a button does not change size when
/// its label changes.
fn widest(pen: &Pen, action: Action, size: f32) -> f32 {
    let actions: &[Action] = if SHARED.contains(&action) {
        &SHARED
    } else {
        &[action]
    };
    actions
        .iter()
        .flat_map(|&action| {
            [ButtonStates::default(), ButtonStates::ALL_ON]
                .map(|states| button::width(pen, action.label(states), size))
        })
        .fold(0.0, f32::max)
}

/// Bottom edge of the toggles, where a menu under them can start.
pub const TOGGLES_BOTTOM: f32 = MARGIN + TOGGLE_HEIGHT;

/// Middle of the toggles across, in `area`, where the keys list centers.
pub fn toggles_middle(area: SceneArea) -> f32 {
    area.left + area.width / 2.0
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
    /// The button under the pointer, if any.
    hovered: Option<Action>,
    /// A press that started on a button is still held, so dragging off the
    /// button does not turn the camera.
    button_gesture: bool,
    /// The sliders were shown at the last update, so they can be dragged.
    sliders_shown: bool,
}

impl Controls {
    pub fn new(hot_kelvin: f32, cold_kelvin: f32, concentration: f32) -> Self {
        Self {
            hot: Slider::hot_temperature(hot_kelvin),
            cold: Slider::cold_temperature(cold_kelvin),
            concentration: Slider::concentration(concentration),
            hovered: None,
            button_gesture: false,
            sliders_shown: true,
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

        let sliders = states.sliders && !states.hidden;
        self.sliders_shown = sliders;
        let (hot, cold, concentration) = if sliders {
            (
                self.hot.update(),
                self.cold.update(),
                self.concentration.update(),
            )
        } else {
            Default::default()
        };
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
        let sliders = self.sliders_shown
            && (self.hot.blocks_camera()
                || self.cold.blocks_camera()
                || self.concentration.blocks_camera());
        self.hovered.is_some() || self.button_gesture || sliders
    }

    /// Draws the slider card and the sliders (when shown), then every
    /// button. The panel's card, with its hump, is the app's to draw, under
    /// the panel; the hump's mark comes here.
    pub fn draw(&self, pen: &mut Pen, area: SceneArea, states: ButtonStates) {
        if states.sliders && !states.hidden {
            drawer::draw_card(pen, slider_column(area.window_width), Edge::Right, true);
            self.hot.draw(pen);
            self.cold.draw(pen);
            self.concentration.draw(pen);
        }
        for placed in place_buttons(pen, area, states) {
            if placed.action == Action::Start && states.no_gap {
                let size = pen.fitted_size(NO_GAP_TIP, area.width - 2.0 * MARGIN, TIP_SIZE);
                let width = pen.measure(NO_GAP_TIP, size);
                let x = area.left + (area.width - width) / 2.0;
                let baseline = placed.rect.y - TIP_GAP;
                // A shade behind it, so it reads over the bright cloud.
                let height = pen.text_height(NO_GAP_TIP, size);
                pen.round_rect(
                    x - TIP_PAD,
                    baseline - height - TIP_PAD,
                    width + 2.0 * TIP_PAD,
                    height + 2.0 * TIP_PAD,
                    BACKING_RADIUS,
                    BACKING,
                );
                pen.text(NO_GAP_TIP, x, baseline, size, TIP_COLOR);
            }
            let hovered = self.hovered == Some(placed.action);
            let label = placed.label;
            match placed.shape {
                Shape::Pill => {
                    button::draw(pen, placed.rect, label, placed.look, hovered, placed.size);
                }
                Shape::Hump(edge) => {
                    drawer::draw_hump_mark(pen, placed.rect, edge, label.key, hovered);
                }
                Shape::Tab(edge) => {
                    drawer::draw_tab(pen, placed.rect, edge, label.text, label.key, hovered);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/controls.rs"]
mod tests;
