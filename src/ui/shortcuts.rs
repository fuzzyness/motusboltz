//! The keyboard shortcuts list, which drops down centered under the Keys
//! button.

use macroquad::prelude::{
    Color,
    Rect,
    vec2, //
};

use crate::ui::input::SHORTCUTS;
use crate::ui::pen::{
    Pen,
    rounded_outline, //
};

/// Space between the card's edge and its text.
const PADDING: f32 = 14.0;
/// Heading above the bindings.
const TITLE: &str = "KEYBOARD SHORTCUTS";
/// Text size of the heading.
const TITLE_SIZE: f32 = 13.0;
/// Text size of the bindings.
const TEXT_SIZE: f32 = 15.0;
/// Distance between baselines.
const LINE: f32 = 21.0;
/// Gap between the keys and what they do.
const COLUMN_GAP: f32 = 16.0;
/// Corner radius of the card.
const RADIUS: f32 = 16.0;
/// Card fill.
const BACKGROUND: Color = Color::new(0.06, 0.1, 0.18, 0.85);
/// Card rim.
const RIM: Color = Color::new(0.85, 0.92, 1.0, 0.2);
/// Width of the card rim.
const RIM_WIDTH: f32 = 1.5;
/// Color of the heading.
const TITLE_COLOR: Color = Color::from_rgba(120, 140, 160, 255);
/// Color of the keys.
const KEY_COLOR: Color = Color::from_rgba(255, 200, 140, 255);
/// Color of what the keys do.
const TEXT_COLOR: Color = Color::from_rgba(215, 226, 238, 255);

/// Width of the widest of `texts` at `size`.
fn widest<'a>(pen: &Pen, texts: impl Iterator<Item = &'a str>, size: f32) -> f32 {
    texts
        .map(|text| pen.measure(text, size))
        .fold(0.0, f32::max)
}

/// Width and height of the list drawn at `scale`, and the width of its key
/// column.
fn extent(pen: &Pen, scale: f32) -> (f32, f32, f32) {
    let size = TEXT_SIZE * scale;
    let key_width = widest(pen, SHORTCUTS.iter().map(|binding| binding.0), size);
    let action_width = widest(pen, SHORTCUTS.iter().map(|binding| binding.1), size);
    let width = 2.0 * PADDING + key_width + COLUMN_GAP + action_width;
    let height = 2.0 * PADDING + LINE * scale * (SHORTCUTS.len() as f32 + 1.0);
    (width, height, key_width)
}

/// Draws the list centered on `middle` with its top at `y` and returns its
/// size.
///
/// The list shrinks if needed to fit `max_width` and `max_height`.
pub fn draw(pen: &mut Pen, middle: f32, y: f32, max_width: f32, max_height: f32) -> (f32, f32) {
    let (width, height, _) = extent(pen, 1.0);
    let scale = (max_width / width).min(max_height / height).min(1.0);
    let (width, height, key_width) = extent(pen, scale);
    let x = middle - width / 2.0;
    let outline = rounded_outline(Rect::new(x, y, width, height), [RADIUS; 4]);
    pen.fan(vec2(middle, y + height / 2.0), &outline, BACKGROUND);
    pen.ring(&outline, RIM_WIDTH, RIM);
    let left = x + PADDING;
    let mut baseline = y + PADDING + LINE * scale * 0.75;
    pen.text(TITLE, left, baseline, TITLE_SIZE * scale, TITLE_COLOR);
    for (keys, action) in SHORTCUTS {
        baseline += LINE * scale;
        pen.text(keys, left, baseline, TEXT_SIZE * scale, KEY_COLOR);
        let action_left = left + key_width + COLUMN_GAP;
        pen.text(action, action_left, baseline, TEXT_SIZE * scale, TEXT_COLOR);
    }
    (width, height)
}

#[cfg(test)]
#[path = "../../tests/unit/ui/shortcuts.rs"]
mod tests;
