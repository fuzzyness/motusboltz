//! Demos: four ready-made settings, each showing one idea about heat
//! engines, and the banner that says what to watch.
//!
//! Choosing a demo sets both walls, refills the gas and leaves the flywheel
//! at rest, so every demo starts the same way: with Start. The banner sits
//! under the sliders' card, where the engine leaves open sky.

use macroquad::prelude::{
    Color,
    Rect, //
};

use crate::sim::reservoir::{
    MAX_HOT_KELVIN,
    MIN_COLD_KELVIN, //
};
use crate::ui::controls::{
    BACKING,
    BACKING_RADIUS,
    MARGIN,
    slider_column, //
};
use crate::ui::panel::{
    QUIET,
    TEXT, //
};
use crate::ui::pen::{
    Pen,
    Weight, //
};

/// Space between the banner's edge and its text.
const PADDING: f32 = 14.0;
/// Text size of the banner.
const TEXT_SIZE: f32 = 14.0;
/// Joins two words the banner must not break between, such as a number and
/// its unit; it is drawn as a space.
const NO_BREAK: char = '\u{a0}';
/// Distance between the banner's baselines.
const LINE: f32 = 19.0;
/// Extra space under the banner's title.
const TITLE_GAP: f32 = 4.0;
/// Space between a demo's name and its temperatures in the title.
const TITLE_SPACE: &str = "   ";
/// The banner's last line while Start is offered.
const PROMPT: &str = "Press Start (Space).";
/// Color of the prompt, Start's own amber.
const PROMPT_COLOR: Color = Color::from_rgba(255, 208, 132, 255);

/// A ready-made setting that shows one idea.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    /// Both walls at room temperature.
    NoDifference,
    /// A wide gap: the engine at its best.
    HeatEngine,
    /// A room-temperature hot side and a very cold one.
    ColdSink,
    /// A gap too small to beat friction.
    SmallGap,
    /// The widest gap the sliders allow.
    LargestDifference,
}

/// The demos, in the menu's order.
pub const SCENARIOS: [Scenario; 5] = [
    Scenario::NoDifference,
    Scenario::HeatEngine,
    Scenario::LargestDifference,
    Scenario::ColdSink,
    Scenario::SmallGap,
];

impl Scenario {
    /// The hot and cold walls' temperatures, in kelvin.
    pub fn kelvin(self) -> (f32, f32) {
        match self {
            Self::NoDifference => (300.0, 300.0),
            Self::HeatEngine => (1_200.0, 300.0),
            Self::ColdSink => (300.0, 100.0),
            Self::SmallGap => (400.0, 300.0),
            Self::LargestDifference => (MAX_HOT_KELVIN, MIN_COLD_KELVIN),
        }
    }

    /// The demo's name.
    pub fn title(self) -> &'static str {
        match self {
            Self::NoDifference => "No temperature difference",
            Self::HeatEngine => "Heat engine",
            Self::ColdSink => "Cold side only",
            Self::SmallGap => "Small gap",
            Self::LargestDifference => "Largest difference",
        }
    }

    /// The temperatures as the menu shows them, hot first.
    pub fn temperatures(self) -> &'static str {
        match self {
            Self::NoDifference => "300 / 300 K",
            Self::HeatEngine => "1200 / 300 K",
            Self::ColdSink => "300 / 100 K",
            Self::SmallGap => "400 / 300 K",
            Self::LargestDifference => "1200 / 77 K",
        }
    }

    /// What to watch.
    pub fn watch(self) -> &'static str {
        match self {
            Self::NoDifference => {
                "Both walls are at 300\u{a0}K, so nothing runs the engine. \
                 The flywheel does work on the gas instead, the gas gives it \
                 to the walls as heat, and the flywheel stops after several \
                 minutes."
            }
            Self::HeatEngine => {
                "Heat flows in at the hot wall (orange sparks) and out at the \
                 cold wall (blue sparks). The small part that comes in but \
                 does not go out becomes the work that keeps the flywheel \
                 turning."
            }
            Self::ColdSink => {
                "The hot side is only room temperature, but the cold side is \
                 100\u{a0}K. A temperature difference runs the engine even \
                 when neither side is hot: it keeps turning, more slowly than \
                 at 1200\u{a0}K."
            }
            Self::SmallGap => {
                "The walls are only 100\u{a0}K apart. The gas cannot do enough \
                 work each turn to keep the flywheel going, so it slows and \
                 stalls after several minutes. Compare Cold side only, \
                 200\u{a0}K apart."
            }
            Self::LargestDifference => {
                "A 1200\u{a0}K burner against liquid nitrogen at \
                 77\u{a0}K, the widest gap the sliders allow. The flywheel \
                 turns fastest here, and the Carnot limit is highest, \
                 about 94%, yet the engine still turns only a small part of the \
                 heat into work."
            }
        }
    }
}

/// `text` broken at its spaces into lines no wider than `width` at `size`,
/// never between words joined by `NO_BREAK`.
///
/// A word wider than `width` gets a line of its own.
pub fn wrap(pen: &Pen, text: &str, width: f32, size: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split(' ').filter(|word| !word.is_empty()) {
        let word = word.replace(NO_BREAK, " ");
        match lines.last_mut() {
            Some(line) if pen.measure(&format!("{line} {word}"), size) <= width => {
                line.push(' ');
                line.push_str(&word);
            }
            _ => lines.push(word),
        }
    }
    lines
}

/// Whether the temperatures fit beside the name of `scenario` in a line
/// `width` wide.
fn title_fits(pen: &Pen, scenario: Scenario, width: f32) -> bool {
    let name = pen.measure_weight(scenario.title(), TEXT_SIZE, Weight::Semibold);
    let temperatures = pen.measure(
        &format!("{TITLE_SPACE}{}", scenario.temperatures()),
        TEXT_SIZE,
    );
    name + temperatures <= width
}

/// The banner's lines under the name of `scenario`, `width` wide, with
/// their colors: the temperatures if they do not fit beside the name, what
/// to watch, and the prompt while `start` is offered.
fn lines(pen: &Pen, scenario: Scenario, width: f32, start: bool) -> Vec<(String, Color)> {
    let temperatures =
        (!title_fits(pen, scenario, width)).then(|| (scenario.temperatures().to_owned(), QUIET));
    let watch = wrap(pen, scenario.watch(), width, TEXT_SIZE)
        .into_iter()
        .map(|line| (line, TEXT));
    let prompt = start.then(|| (PROMPT.to_owned(), PROMPT_COLOR));
    temperatures
        .into_iter()
        .chain(watch)
        .chain(prompt)
        .collect()
}

/// Width of the banner's lines in a window `width` points wide.
fn text_width(width: f32) -> f32 {
    card(width, 0).w - 2.0 * PADDING
}

/// The banner's card for `count` lines of text under the title, in a
/// window `width` points wide: under the sliders' card, as wide.
///
/// It keeps its place while the sliders are closed, clear of their tab.
pub fn card(width: f32, count: usize) -> Rect {
    let column = slider_column(width);
    let height = 2.0 * PADDING + TITLE_GAP + LINE * count as f32 + TEXT_SIZE;
    Rect::new(column.x, column.bottom() + MARGIN, column.w, height)
}

/// The banner's card for `scenario` in a window `width` points wide, with
/// the prompt while `start` is offered.
pub fn banner(pen: &Pen, width: f32, scenario: Scenario, start: bool) -> Rect {
    card(width, lines(pen, scenario, text_width(width), start).len())
}

/// Draws the banner for `scenario` in a window `width` points wide, ending
/// with the prompt while `start` is offered.
pub fn draw_banner(pen: &mut Pen, width: f32, scenario: Scenario, start: bool) {
    let room = text_width(width);
    let lines = lines(pen, scenario, room, start);
    let card = card(width, lines.len());
    pen.round_rect(card, BACKING_RADIUS, BACKING);
    let (left, right) = (card.x + PADDING, card.right() - PADDING);
    let mut y = card.y + PADDING + pen.text_height("H", TEXT_SIZE);
    pen.strong(scenario.title(), left, y, TEXT_SIZE, TEXT);
    if title_fits(pen, scenario, room) {
        pen.text_right(scenario.temperatures(), right, y, TEXT_SIZE, QUIET);
    }
    y += TITLE_GAP;
    for (line, color) in &lines {
        y += LINE;
        pen.text(line, left, y, TEXT_SIZE, *color);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/demos.rs"]
mod tests;
