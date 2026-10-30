//! On-screen buttons: rounded, softly translucent pills over the scene,
//! each with a label and the key that does the same in a quieter color.

use macroquad::prelude::{
    Color,
    Rect, //
};

use crate::ui::pen::{
    Pen,
    Weight, //
};

/// Label size, and the gap between a label and its key.
pub const LABEL_SIZE: f32 = 16.0;
const KEY_GAP: f32 = 8.0;
/// Room between the text and the button's ends.
const PADDING_X: f32 = 14.0;

/// How a button looks: ordinary, switched on, or the one main action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Plain,
    On,
    Primary,
}

struct Colors {
    fill: Color,
    hover: Color,
    label: Color,
    key: Color,
}

impl Look {
    fn colors(self) -> Colors {
        match self {
            Self::Plain => Colors {
                fill: Color::new(0.15, 0.20, 0.27, 0.82),
                hover: Color::new(0.21, 0.28, 0.37, 0.92),
                label: Color::from_rgba(225, 233, 242, 255),
                key: Color::from_rgba(130, 148, 168, 255),
            },
            Self::On => Colors {
                fill: Color::new(0.20, 0.38, 0.47, 0.92),
                hover: Color::new(0.25, 0.45, 0.55, 0.96),
                label: Color::from_rgba(240, 248, 252, 255),
                key: Color::from_rgba(170, 205, 220, 255),
            },
            Self::Primary => Colors {
                fill: Color::from_rgba(255, 160, 90, 255),
                hover: Color::from_rgba(255, 184, 125, 255),
                label: Color::from_rgba(24, 22, 26, 255),
                key: Color::from_rgba(110, 62, 30, 255),
            },
        }
    }
}

/// A button's text: what it does, and its key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Label {
    pub text: &'static str,
    pub key: &'static str,
}

/// Width a button needs for `label` at `size`.
pub fn width(pen: &Pen, label: Label, size: f32) -> f32 {
    pen.measure_weight(label.text, size, Weight::Semibold)
        + KEY_GAP
        + pen.measure(label.key, size)
        + 2.0 * PADDING_X
}

/// Draws a button filling `rect`, with its text centered.
pub fn draw(pen: &mut Pen, rect: Rect, label: Label, look: Look, hovered: bool, size: f32) {
    let colors = look.colors();
    pen.round_rect(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        rect.h / 2.0,
        if hovered { colors.hover } else { colors.fill },
    );
    let text_width = pen.measure_weight(label.text, size, Weight::Semibold);
    let total = text_width + KEY_GAP + pen.measure(label.key, size);
    let left = rect.x + (rect.w - total) / 2.0;
    let baseline = rect.y + (rect.h + pen.text_height("H", size)) / 2.0;
    pen.heading(label.text, left, baseline, size, colors.label);
    pen.text(
        label.key,
        left + text_width + KEY_GAP,
        baseline,
        size,
        colors.key,
    );
}
