//! The menu Esc opens, in the orbit view and in first person.
//!
//! It pops up over the scene, which runs on behind it, and shrinks away
//! when closed. From the top: Resume, which closes it (and in first person
//! captures the mouse again), as Esc does; Options; Exit first person
//! while the view is the figure's; and, in the application but not on the
//! web page, set a little apart at the bottom, Exit simulation. Options
//! opens a page of its own with the mouse sensitivity for looking around
//! in first person, and Back.

use macroquad::prelude::{
    Color,
    Rect,
    Vec2,
    vec2, //
};

use crate::ui::button::{
    self,
    Hue,
    Label,
    Look, //
};
use crate::ui::controls::BACKING;
use crate::ui::draw_target::{
    draw_moved,
    draw_to_screen, //
};
use crate::ui::motion::{
    advance,
    bounce,
    smooth, //
};
use crate::ui::panel::{
    QUIET,
    TEXT, //
};
use crate::ui::pen::Pen;

/// Darkening over the scene while the menu is open.
const DIM: Color = Color::new(0.0, 0.0, 0.0, 0.35);
/// Seconds the menu takes to pop up, or to shrink away.
const POP_SECONDS: f32 = 0.25;
/// How far the menu overshoots its size as it pops up (see
/// `motion::bounce`).
const POP_OVERSHOOT: f32 = 1.4;
/// The menu's size as it starts to pop up, against its full size.
const POP_FROM: f32 = 0.5;
/// Extra space above Exit simulation, setting it apart from the rest, in
/// points.
const EXIT_GAP: f32 = 16.0;
/// Whether the simulation can be left from its menu: in the application,
/// but not on the web page, which the browser closes.
pub const CAN_EXIT: bool = !cfg!(target_arch = "wasm32");
/// The menu's title.
const TITLE: &str = "Menu";
/// The options page's title.
const OPTIONS_TITLE: &str = "Options";
/// Text size of the titles.
const TITLE_SIZE: f32 = 22.0;
/// Width of the buttons and the slider, in points.
const ROW_WIDTH: f32 = 250.0;
/// Height of a button, in points.
const BUTTON_HEIGHT: f32 = 40.0;
/// Height of the slider's row, its words and its track, in points.
const SLIDER_HEIGHT: f32 = 58.0;
/// Space between rows, and round them on the card, in points.
const GAP: f32 = 14.0;
/// Room above the rows for the title, in points.
const TITLE_ROOM: f32 = 44.0;
/// Text size of the buttons.
const LABEL_SIZE: f32 = 17.0;
/// Text size of the slider's words.
const SLIDER_TEXT_SIZE: f32 = 15.0;
/// Corner radius of the card.
const RADIUS: f32 = 16.0;
/// Height of the slider's track, in points.
const TRACK_HEIGHT: f32 = 6.0;
/// Radius of the slider's knob, in points.
const KNOB_RADIUS: f32 = 10.0;
/// Room under the slider's track, in points.
const TRACK_BOTTOM: f32 = 12.0;
/// The slider's track.
const TRACK: Color = Color::new(1.0, 1.0, 1.0, 0.18);
/// The slider's filled part and knob.
const KNOB: Color = Color::from_rgba(118, 212, 246, 255);
/// Lowest mouse sensitivity, against the usual.
pub const MIN_SENSITIVITY: f32 = 0.25;
/// Highest mouse sensitivity, against the usual.
pub const MAX_SENSITIVITY: f32 = 3.0;

/// One row of the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    /// Opens the options page.
    Options,
    /// Leaves first person for the orbit view.
    ExitFirstPerson,
    /// Closes the menu.
    Resume,
    /// Leaves the application.
    ExitSimulation,
    /// The mouse sensitivity, on the options page.
    Sensitivity,
    /// Back from the options page.
    Back,
}

/// What a press in the menu asks the app to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Leave first person.
    ExitFirstPerson,
    /// Close the menu.
    Resume,
    /// Leave the application.
    ExitSimulation,
}

/// The menu: open or not, which page, and the options it holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Menu {
    /// It shows.
    pub open: bool,
    /// The options page shows instead of the main one.
    options: bool,
    /// The mouse sensitivity for looking around, against the usual.
    pub sensitivity: f32,
    /// The slider's knob is held.
    sliding: bool,
    /// How far it has popped up, from 0 (gone) to 1 (full size).
    shown: f32,
    /// It offers Exit simulation (see `CAN_EXIT`).
    pub can_exit: bool,
}

impl Default for Menu {
    fn default() -> Self {
        Self {
            open: false,
            options: false,
            sensitivity: 1.0,
            sliding: false,
            shown: 0.0,
            can_exit: CAN_EXIT,
        }
    }
}

impl Menu {
    /// Opens it on its main page.
    pub fn open(&mut self) {
        self.open = true;
        self.options = false;
        self.sliding = false;
    }

    /// Closes it.
    pub fn close(&mut self) {
        self.open = false;
        self.sliding = false;
    }

    /// Pops it up, or shrinks it away, over `dt` seconds.
    pub fn animate(&mut self, dt: f32) {
        self.shown = advance(self.shown, self.open, dt, POP_SECONDS);
    }

    /// Whether any of it shows, open or shrinking away.
    pub fn visible(&self) -> bool {
        self.shown > 0.0
    }

    /// Its rows, top to bottom, in a window `width` by `height`, for the
    /// orbit view or `first_person`.
    pub fn rows(&self, width: f32, height: f32, first_person: bool) -> Vec<(Item, Rect)> {
        let items: Vec<Item> = if self.options {
            vec![Item::Sensitivity, Item::Back]
        } else {
            let mut items = vec![Item::Resume, Item::Options];
            if first_person {
                items.push(Item::ExitFirstPerson);
            }
            if self.can_exit {
                items.push(Item::ExitSimulation);
            }
            items
        };
        let row_height = |item: Item| match item {
            Item::Sensitivity => SLIDER_HEIGHT,
            _ => BUTTON_HEIGHT,
        };
        let gap_above = |item: Item| match item {
            Item::ExitSimulation => GAP + EXIT_GAP,
            _ => GAP,
        };
        let total = items.iter().map(|&item| row_height(item)).sum::<f32>()
            + items
                .iter()
                .skip(1)
                .map(|&item| gap_above(item))
                .sum::<f32>();
        let left = (width - ROW_WIDTH) / 2.0;
        let mut y = (height - total + TITLE_ROOM) / 2.0 - GAP;
        items
            .into_iter()
            .map(|item| {
                y += gap_above(item);
                let rect = Rect::new(left, y, ROW_WIDTH, row_height(item));
                y += rect.h;
                (item, rect)
            })
            .collect()
    }

    /// The row under `pointer`, if any.
    pub fn item_at(
        &self,
        width: f32,
        height: f32,
        first_person: bool,
        pointer: Vec2,
    ) -> Option<Item> {
        self.rows(width, height, first_person)
            .into_iter()
            .find(|(_, rect)| rect.contains(pointer))
            .map(|(item, _)| item)
    }

    /// Takes the mouse at `pointer` this frame (`button`: whether the left
    /// button is down, and whether it went down this frame) in a window
    /// `width` by `height`, and returns what a press asks for, if anything.
    /// Options and Back turn the page; the slider moves with a held knob.
    pub fn handle(
        &mut self,
        pointer: Vec2,
        button: (bool, bool),
        size: Vec2,
        first_person: bool,
    ) -> Option<Outcome> {
        let (down, pressed) = button;
        let rows = self.rows(size.x, size.y, first_person);
        if !down {
            self.sliding = false;
        }
        if self.sliding {
            if let Some((_, rect)) = rows.iter().find(|(item, _)| *item == Item::Sensitivity) {
                self.sensitivity = sensitivity_at(track(*rect), pointer.x);
            }
            return None;
        }
        if !pressed {
            return None;
        }
        let (item, rect) = rows.into_iter().find(|(_, rect)| rect.contains(pointer))?;
        match item {
            Item::Options => {
                self.options = true;
                None
            }
            Item::ExitFirstPerson => Some(Outcome::ExitFirstPerson),
            Item::Resume => Some(Outcome::Resume),
            Item::ExitSimulation => Some(Outcome::ExitSimulation),
            Item::Sensitivity => {
                self.sliding = true;
                self.sensitivity = sensitivity_at(track(rect), pointer.x);
                None
            }
            Item::Back => {
                self.options = false;
                None
            }
        }
    }

    /// Draws the menu over a window `width` by `height`, the button under
    /// `pointer` lit, for the orbit view or `first_person`.
    pub fn draw(&self, pen: &mut Pen, size: Vec2, pointer: Vec2, first_person: bool) {
        if !self.visible() {
            return;
        }
        let (width, height) = (size.x, size.y);
        let dim = Color {
            a: DIM.a * smooth(self.shown),
            ..DIM
        };
        pen.rect(Rect::new(0.0, 0.0, width, height), dim);
        // It pops up from the middle with a little bounce, and shrinks back
        // into it.
        if pen.live() {
            let grown = POP_FROM + (1.0 - POP_FROM) * bounce(self.shown, POP_OVERSHOOT);
            draw_moved(size / 2.0, grown, Vec2::ZERO);
        }
        self.draw_card(pen, size, pointer, first_person);
        if pen.live() {
            draw_to_screen();
        }
    }

    /// Draws the card, its title and its rows, at full size.
    fn draw_card(&self, pen: &mut Pen, size: Vec2, pointer: Vec2, first_person: bool) {
        let width = size.x;
        let height = size.y;
        let rows = self.rows(width, height, first_person);
        let (first, last) = (rows[0].1, rows[rows.len() - 1].1);
        let card = Rect::new(
            first.x - GAP,
            first.y - TITLE_ROOM - GAP,
            first.w + 2.0 * GAP,
            last.bottom() - first.y + TITLE_ROOM + 2.0 * GAP,
        );
        pen.round_rect(card, RADIUS, BACKING);
        let title = if self.options { OPTIONS_TITLE } else { TITLE };
        let title_width = pen.measure(title, TITLE_SIZE);
        pen.heading(
            title,
            (width - title_width) / 2.0,
            first.y - GAP,
            TITLE_SIZE,
            TEXT,
        );
        for (item, rect) in rows {
            if item == Item::Sensitivity {
                self.draw_slider(pen, rect);
                continue;
            }
            let look = Look {
                hue: hue(item),
                on: false,
            };
            let hovered = rect.contains(pointer);
            let label = Label {
                text: text(item),
                key: if item == Item::Resume { "Esc" } else { "" },
            };
            button::draw(pen, rect, label, look, hovered, LABEL_SIZE);
        }
    }

    /// Draws the mouse-sensitivity slider filling `rect`.
    fn draw_slider(&self, pen: &mut Pen, rect: Rect) {
        let baseline = rect.y + SLIDER_TEXT_SIZE;
        pen.text(
            "Mouse sensitivity",
            rect.x,
            baseline,
            SLIDER_TEXT_SIZE,
            TEXT,
        );
        let value = format!("{:.2}\u{d7}", self.sensitivity);
        pen.text_right(&value, rect.right(), baseline, SLIDER_TEXT_SIZE, QUIET);
        let line = track(rect);
        pen.round_rect(line, line.h / 2.0, TRACK);
        let share = (self.sensitivity - MIN_SENSITIVITY) / (MAX_SENSITIVITY - MIN_SENSITIVITY);
        let knob = vec2(line.x + share * line.w, line.center().y);
        let filled = Rect::new(line.x, line.y, knob.x - line.x, line.h);
        pen.round_rect(filled, line.h / 2.0, KNOB);
        pen.circle(knob, KNOB_RADIUS, KNOB);
    }
}

/// The slider's track in its row `rect`.
fn track(rect: Rect) -> Rect {
    Rect::new(
        rect.x + KNOB_RADIUS,
        rect.bottom() - TRACK_BOTTOM - TRACK_HEIGHT / 2.0,
        rect.w - 2.0 * KNOB_RADIUS,
        TRACK_HEIGHT,
    )
}

/// The sensitivity with the knob at `x` along `track`.
fn sensitivity_at(track: Rect, x: f32) -> f32 {
    let share = ((x - track.x) / track.w).clamp(0.0, 1.0);
    MIN_SENSITIVITY + share * (MAX_SENSITIVITY - MIN_SENSITIVITY)
}

/// A button's words.
fn text(item: Item) -> &'static str {
    match item {
        Item::Options => "Options",
        Item::ExitFirstPerson => "Exit first person",
        Item::Resume => "Resume",
        Item::ExitSimulation => "Exit simulation",
        Item::Sensitivity => "Mouse sensitivity",
        Item::Back => "Back",
    }
}

/// A button's color.
fn hue(item: Item) -> Hue {
    match item {
        Item::Options | Item::Back | Item::Sensitivity => Hue::Pearl,
        Item::ExitFirstPerson | Item::ExitSimulation => Hue::Coral,
        Item::Resume => Hue::Mint,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/menu.rs"]
mod tests;
