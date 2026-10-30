//! The keyboard shortcuts list, which drops down under the Keys button.

use macroquad::prelude::Color;

use crate::ui::input::SHORTCUTS;
use crate::ui::pen::Pen;

const PADDING: f32 = 14.0;
const TITLE: &str = "KEYBOARD SHORTCUTS";
const TITLE_SIZE: f32 = 13.0;
const TEXT_SIZE: f32 = 15.0;
const LINE: f32 = 21.0;
const COLUMN_GAP: f32 = 16.0;
const RADIUS: f32 = 10.0;
const BACKGROUND: Color = Color::new(0.06, 0.09, 0.14, 0.92);
const TITLE_COLOR: Color = Color::from_rgba(120, 140, 160, 255);
const KEY_COLOR: Color = Color::from_rgba(255, 200, 140, 255);
const TEXT_COLOR: Color = Color::from_rgba(215, 226, 238, 255);

/// Width and height of the list at `size` (scaled from `TEXT_SIZE`).
fn extent(pen: &Pen, scale: f32) -> (f32, f32, f32) {
    let key_width = SHORTCUTS
        .iter()
        .map(|&(keys, _)| pen.measure(keys, TEXT_SIZE * scale))
        .fold(0.0, f32::max);
    let text_width = SHORTCUTS
        .iter()
        .map(|&(_, action)| pen.measure(action, TEXT_SIZE * scale))
        .fold(0.0, f32::max);
    let width = 2.0 * PADDING + key_width + COLUMN_GAP + text_width;
    let height = 2.0 * PADDING + LINE * scale * (SHORTCUTS.len() as f32 + 1.0);
    (width, height, key_width)
}

/// Draws the list with its top-left corner at (`x`, `y`), shrunk if needed
/// to fit `max_width` and `max_height`. Returns its size.
pub fn draw(pen: &mut Pen, x: f32, y: f32, max_width: f32, max_height: f32) -> (f32, f32) {
    let (width, height, _) = extent(pen, 1.0);
    let scale = (max_width / width).min(max_height / height).min(1.0);
    let (width, height, key_width) = extent(pen, scale);
    pen.round_rect(x, y, width, height, RADIUS, BACKGROUND);
    let left = x + PADDING;
    let mut baseline = y + PADDING + LINE * scale * 0.75;
    pen.text(TITLE, left, baseline, TITLE_SIZE * scale, TITLE_COLOR);
    for (keys, action) in SHORTCUTS {
        baseline += LINE * scale;
        pen.text(keys, left, baseline, TEXT_SIZE * scale, KEY_COLOR);
        pen.text(
            action,
            left + key_width + COLUMN_GAP,
            baseline,
            TEXT_SIZE * scale,
            TEXT_COLOR,
        );
    }
    (width, height)
}

#[cfg(test)]
#[path = "../../tests/unit/ui/shortcuts.rs"]
mod tests;
