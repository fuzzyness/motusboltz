//! The on-screen controls.
//!
//! They are the Demos, Labels, Keys and scenery toggles centered at the
//! top, with the Demos menu dropping under them while it is open; a row of
//! Reset, Start or Pause, Slow and Tracers centered along the bottom, where
//! Start takes Pause's place whenever pressing it would help (Space does
//! whichever is shown); the hot, cold and helium sliders on a
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
use crate::ui::demos::{
    SCENARIOS,
    Scenario, //
};
use crate::ui::drawer::{
    self,
    Edge, //
};
use crate::ui::layout::{
    CONTROL_WIDTH,
    PANEL_INSET,
    PANEL_WIDTH,
    control_left, //
};
use crate::ui::pen::{
    Pen,
    grown, //
};
use crate::ui::slider::{
    self,
    Slider, //
};

/// Inset of the toggles and the bottom row from the free part's edges.
pub const MARGIN: f32 = 16.0;
/// Height of the toggles.
const TOGGLE_HEIGHT: f32 = 30.0;
/// Label size of the toggles.
const TOGGLE_LABEL_SIZE: f32 = 15.0;
/// Height of the bottom row.
const ROW_HEIGHT: f32 = 36.0;
/// Gap between neighboring buttons.
const ROW_GAP: f32 = 10.0;
/// Bottom row's distance from the bottom of the window.
const ROW_BOTTOM_MARGIN: f32 = 18.0;
/// Height of Hide all, whose middle lines up with the bottom row's.
const CORNER_HEIGHT: f32 = 28.0;
/// Label size of Hide all.
const CORNER_LABEL_SIZE: f32 = 14.0;
/// Smallest label size the bottom row shrinks to on a narrow scene.
const MIN_ROW_LABEL_SIZE: f32 = 12.0;
/// How far the sliders' card reaches past the sliders.
const CARD_PAD: f32 = 16.0;
/// Gap above each item of the Demos menu.
const MENU_GAP: f32 = 8.0;
/// The tip over the row when Start is offered with the sides at one
/// temperature.
const NO_GAP_TIP: &str = "Tip: drag Hot side up first, or the flywheel will run down.";
/// Text size of the tip.
const TIP_SIZE: f32 = 15.0;
/// Gap between the tip's baseline and the row.
const TIP_GAP: f32 = 16.0;
/// How far the shade behind the tip reaches past its text.
const TIP_PAD: f32 = 7.0;
/// Color of the tip.
const TIP_COLOR: Color = Color::from_rgba(200, 212, 225, 255);
/// The shade behind loose text over the sky.
pub const BACKING: Color = Color::new(0.06, 0.09, 0.15, 0.62);
/// Corner radius of the shade.
pub const BACKING_RADIUS: f32 = 12.0;
/// Bottom edge of the toggles, where a menu under them can start.
pub const TOGGLES_BOTTOM: f32 = MARGIN + TOGGLE_HEIGHT;

/// On-screen button actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Show or hide the panel.
    TogglePanel,
    /// Show or hide the sliders.
    ToggleSliders,
    /// Open or close the Demos menu.
    ToggleDemos,
    /// Show or hide the labels on the scene.
    ToggleLabels,
    /// Show or hide the keys list.
    ToggleKeys,
    /// Switch between the sky and the grid scenery.
    ToggleScenery,
    /// Hide or show everything.
    ToggleAll,
    /// Refill with fresh gas.
    ResetGas,
    /// Pause or resume.
    TogglePause,
    /// Turn slow motion on or off.
    ToggleSlow,
    /// Turn the tracers on or off.
    ToggleTracers,
    /// Start the engine.
    Start,
    /// Set up a demo.
    Demo(Scenario),
}

/// The toggles at the top.
const TOGGLES: [Action; 4] = [
    Action::ToggleDemos,
    Action::ToggleLabels,
    Action::ToggleKeys,
    Action::ToggleScenery,
];
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
    /// The engine is paused.
    pub paused: bool,
    /// Slow motion is on.
    pub slow: bool,
    /// The tracers are on.
    pub tracers: bool,
    /// The panel is open.
    pub panel: bool,
    /// The keys list is shown.
    pub keys: bool,
    /// The labels are shown.
    pub labels: bool,
    /// The Demos menu is open.
    pub demos: bool,
    /// The demo set up last, while its settings stand.
    pub demo: Option<Scenario>,
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
        labels: true,
        demos: true,
        demo: Some(Scenario::HeatEngine),
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
            Self::ToggleDemos => ("Demos", "M"),
            Self::ToggleLabels => ("Labels", "N"),
            Self::ToggleKeys => ("Keys", "H"),
            Self::ToggleScenery if states.sky => ("Grid view", "V"),
            Self::ToggleScenery => ("Sky view", "V"),
            Self::ResetGas => ("Reset", "T"),
            Self::TogglePause if states.paused => ("Resume", "Space"),
            Self::TogglePause => ("Pause", "Space"),
            Self::ToggleSlow => ("Slow", "L"),
            Self::ToggleTracers => ("Tracers", "F"),
            Self::Start => ("Start", "Space"),
            Self::Demo(scenario) => (scenario.title(), scenario.temperatures()),
        };
        Label { text, key }
    }

    /// How the button looks.
    ///
    /// Each button in the bottom row has a color of its own, as on the
    /// Dreamcast's menu; the toggles share a pearl gray. Start always glows,
    /// to draw the eye.
    fn look(self, states: ButtonStates) -> Look {
        let (hue, on) = match self {
            Self::TogglePanel | Self::ToggleSliders | Self::ToggleScenery => (Hue::Pearl, false),
            Self::ToggleAll => (Hue::Pearl, states.hidden),
            Self::ToggleDemos => (Hue::Pearl, states.demos),
            Self::ToggleLabels => (Hue::Pearl, states.labels),
            Self::ToggleKeys => (Hue::Pearl, states.keys),
            Self::Demo(scenario) => (Hue::Pearl, states.demo == Some(scenario)),
            Self::ResetGas => (Hue::Coral, false),
            Self::Start => (Hue::Amber, true),
            Self::TogglePause => (Hue::Mint, states.paused),
            Self::ToggleSlow => (Hue::Cyan, states.slow),
            Self::ToggleTracers => (Hue::Orchid, states.tracers),
        };
        Look { hue, on }
    }

    /// The button for this action in `rect`, with labels at `size`.
    fn placed(self, states: ButtonStates, rect: Rect, size: f32, shape: Shape) -> Placed {
        Placed {
            action: self,
            rect,
            label: self.label(states),
            look: self.look(states),
            size,
            shape,
        }
    }
}

/// Where things go in the scene, in screen points (see `layout`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneArea {
    /// Left edge of the uncovered scene, where the corner readout starts.
    pub edge: f32,
    /// Left edge of the centered span for the toggles and the bottom row.
    pub left: f32,
    /// Width of the centered span.
    pub width: f32,
    /// Width of the window.
    pub window_width: f32,
    /// Height of the window.
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
    /// A pill in the scene.
    Pill,
    /// The grip in the hump of an open drawer at an edge.
    Hump(Edge),
    /// The tab a closed drawer leaves at an edge.
    Tab(Edge),
}

/// One button placed on screen.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    /// What it does.
    pub action: Action,
    /// Where it is.
    pub rect: Rect,
    /// Its text.
    pub label: Label,
    /// How it looks.
    pub look: Look,
    /// Its label size.
    pub size: f32,
    /// How it is drawn.
    pub shape: Shape,
}

/// The panel's card in a window `height` points tall.
pub fn panel_card(height: f32) -> Rect {
    let width = PANEL_WIDTH - PANEL_INSET;
    Rect::new(PANEL_INSET, PANEL_INSET, width, height - 2.0 * PANEL_INSET)
}

/// The sliders' card for a window `width` points wide.
pub fn slider_column(width: f32) -> Rect {
    let (top, bottom) = slider::column_span();
    let left = control_left(width) - CARD_PAD;
    let size = (
        CONTROL_WIDTH + 2.0 * CARD_PAD,
        bottom - top + 2.0 * CARD_PAD,
    );
    Rect::new(left, top - CARD_PAD, size.0, size.1)
}

/// Top of the bottom row in a window `height` points tall.
pub fn row_top(height: f32) -> f32 {
    height - ROW_BOTTOM_MARGIN - ROW_HEIGHT
}

/// Hide all, in the bottom-right corner of `area`.
pub fn corner_button(pen: &Pen, area: SceneArea) -> Rect {
    let width = widest(pen, Action::ToggleAll, CORNER_LABEL_SIZE);
    let middle = row_top(area.height) + ROW_HEIGHT / 2.0;
    let (left, top) = (
        area.window_width - MARGIN - width,
        middle - CORNER_HEIGHT / 2.0,
    );
    Rect::new(left, top, width, CORNER_HEIGHT)
}

/// The button for the drawer at `edge`: the grip in its hump while it is
/// open, or the tab at the window's edge while it is closed.
fn drawer_button(edge: Edge, states: ButtonStates, area: SceneArea) -> Placed {
    let (action, card, open) = match edge {
        Edge::Left => (Action::TogglePanel, panel_card(area.height), states.panel),
        Edge::Right => (
            Action::ToggleSliders,
            slider_column(area.window_width),
            states.sliders,
        ),
    };
    let (rect, shape) = if open {
        (drawer::hump(card, edge), Shape::Hump(edge))
    } else {
        // The tab stays level with the card's middle.
        let middle = card.center().y;
        (
            drawer::tab(edge, area.window_width, middle),
            Shape::Tab(edge),
        )
    };
    action.placed(states, rect, LABEL_SIZE, shape)
}

/// Widths of `actions` with labels at `size`, and their total side by
/// side.
fn row_widths(pen: &Pen, actions: &[Action], size: f32) -> (Vec<f32>, f32) {
    let widths: Vec<f32> = actions
        .iter()
        .map(|&action| widest(pen, action, size))
        .collect();
    let total = widths.iter().sum::<f32>() + ROW_GAP * (actions.len() - 1) as f32;
    (widths, total)
}

/// Every button shown for `states`, placed in `area`.
pub fn place_buttons(pen: &Pen, area: SceneArea, states: ButtonStates) -> Vec<Placed> {
    let corner_rect = corner_button(pen, area);
    let corner = Action::ToggleAll.placed(states, corner_rect, CORNER_LABEL_SIZE, Shape::Pill);
    if states.hidden {
        return vec![corner];
    }
    let mut placed = vec![
        drawer_button(Edge::Left, states, area),
        drawer_button(Edge::Right, states, area),
        corner,
    ];
    // Places `actions` side by side, centered across the area.
    let mut centered = |actions: &[Action], top: f32, height: f32, size: f32| {
        let (widths, total) = row_widths(pen, actions, size);
        let mut x = area.left + (area.width - total) / 2.0;
        for (&action, width) in actions.iter().zip(widths) {
            let rect = Rect::new(x, top, width, height);
            placed.push(action.placed(states, rect, size, Shape::Pill));
            x += width + ROW_GAP;
        }
    };
    centered(&TOGGLES, MARGIN, TOGGLE_HEIGHT, TOGGLE_LABEL_SIZE);
    // The bottom row shrinks its labels if the scene is narrow.
    let row = row(states);
    let room = area.width - 2.0 * MARGIN;
    let (_, natural) = row_widths(pen, &row, LABEL_SIZE);
    let size = (LABEL_SIZE * (room / natural).min(1.0)).max(MIN_ROW_LABEL_SIZE);
    let row_top = row_top(area.height);
    centered(&row, row_top, ROW_HEIGHT, size);
    if states.demos {
        placed.extend(demo_menu(pen, area, states));
    }
    placed
}

/// The Demos menu for `states`, a column of pills dropping from the Demos
/// toggle, the first of the toggles, in `area`.
fn demo_menu(pen: &Pen, area: SceneArea, states: ButtonStates) -> impl Iterator<Item = Placed> {
    let (_, toggles) = row_widths(pen, &TOGGLES, TOGGLE_LABEL_SIZE);
    let width = SCENARIOS
        .iter()
        .map(|&scenario| widest(pen, Action::Demo(scenario), TOGGLE_LABEL_SIZE))
        .fold(0.0, f32::max);
    let toggles_left = area.left + (area.width - toggles) / 2.0;
    let left = toggles_left.min(area.right() - MARGIN - width);
    SCENARIOS
        .into_iter()
        .enumerate()
        .map(move |(index, scenario)| {
            let top = TOGGLES_BOTTOM + MENU_GAP + index as f32 * (TOGGLE_HEIGHT + MENU_GAP);
            let rect = Rect::new(left, top, width, TOGGLE_HEIGHT);
            Action::Demo(scenario).placed(states, rect, TOGGLE_LABEL_SIZE, Shape::Pill)
        })
}

/// Bottom of the Demos menu while it is open.
pub fn demo_menu_bottom() -> f32 {
    TOGGLES_BOTTOM + SCENARIOS.len() as f32 * (MENU_GAP + TOGGLE_HEIGHT)
}

/// Width that fits the action's label in every state.
///
/// In the shared place, the width fits both Start's and Pause's, so a
/// button does not change size when its label changes.
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

/// Middle of the toggles across, in `area`, where the keys list centers.
pub fn toggles_middle(area: SceneArea) -> f32 {
    area.left + area.width / 2.0
}

/// What the controls did this frame.
///
/// Slider values are reported when a temperature changes and when the
/// helium slider is released, since a new fill restarts the gas.
#[derive(Clone, Copy, Debug, Default)]
pub struct ControlEvents {
    /// The button pressed, if any.
    pub action: Option<Action>,
    /// The new hot-side temperature, in kelvin.
    pub hot_kelvin: Option<f32>,
    /// The new cold-side temperature, in kelvin.
    pub cold_kelvin: Option<f32>,
    /// The new helium concentration, in mol/L.
    pub concentration: Option<f32>,
}

/// The buttons and the sliders.
pub struct Controls {
    /// The hot-side temperature.
    hot: Slider,
    /// The cold-side temperature.
    cold: Slider,
    /// The helium concentration.
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
    /// Controls with the sliders at the given settings.
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

    /// Moves the temperature sliders to walls set another way (a demo).
    pub fn set_kelvin(&mut self, hot_kelvin: f32, cold_kelvin: f32) {
        self.hot.set_value(hot_kelvin);
        self.cold.set_value(cold_kelvin);
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
    /// button.
    ///
    /// The panel's card, with its hump, is the app's to draw, under the
    /// panel; the hump's grip comes here.
    pub fn draw(&self, pen: &mut Pen, area: SceneArea, states: ButtonStates) {
        if states.sliders && !states.hidden {
            drawer::draw_card(pen, slider_column(area.window_width), Edge::Right, true);
            self.hot.draw(pen);
            self.cold.draw(pen);
            self.concentration.draw(pen);
        }
        for placed in place_buttons(pen, area, states) {
            if placed.action == Action::Start && states.no_gap {
                draw_tip(pen, area, placed.rect.y);
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

/// Draws the tip over Start, centered across `area`, above `row_top`.
fn draw_tip(pen: &mut Pen, area: SceneArea, row_top: f32) {
    let size = pen.fitted_size(NO_GAP_TIP, area.width - 2.0 * MARGIN, TIP_SIZE);
    let width = pen.measure(NO_GAP_TIP, size);
    let x = area.left + (area.width - width) / 2.0;
    let baseline = row_top - TIP_GAP;
    // A shade behind it, so it reads over the bright cloud.
    let height = pen.text_height(NO_GAP_TIP, size);
    let shade = grown(Rect::new(x, baseline - height, width, height), TIP_PAD);
    pen.round_rect(shade, BACKING_RADIUS, BACKING);
    pen.text(NO_GAP_TIP, x, baseline, size, TIP_COLOR);
}

#[cfg(test)]
#[path = "../../tests/unit/ui/controls.rs"]
mod tests;
