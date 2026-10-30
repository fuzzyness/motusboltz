//! On-screen buttons in the style of the Dreamcast's own menu: glossy
//! pastel pills, each in its own color, with a bright rim, a sheen across
//! the top and an outlined label beside the key that does the same.

use macroquad::prelude::{
    Color,
    Rect, //
};

use crate::ui::pen::{
    Pen,
    Weight,
    rounded_outline, //
};

/// Label size, and the gap between a label and its key.
pub const LABEL_SIZE: f32 = 16.0;
const KEY_GAP: f32 = 8.0;
/// Room between the text and the button's ends.
const PADDING_X: f32 = 16.0;
/// Width of the bright rim around the face.
const RIM: f32 = 2.5;
/// A button that is on glows: how far the glow reaches past the rim, and
/// how opaque it is. Its rim also turns this far toward white.
const GLOW: f32 = 4.0;
const GLOW_OPACITY: f32 = 0.5;
const ON_RIM_WHITENING: f32 = 0.6;
/// The sheen across the top of the face, in layers that each lighten a
/// little more, so it fades in rather than starting at a hard edge: each
/// layer's height as a share of the face's, and how far it stays in from
/// the face's ends (as a share of the face's height).
const SHEEN_LAYERS: [(f32, f32); 3] = [(0.52, 0.22), (0.4, 0.3), (0.28, 0.4)];
const SHEEN: Color = Color::new(1.0, 1.0, 1.0, 0.1);
/// How far the face turns toward white while on and while hovered.
const ON_LIFT: f32 = 0.08;
const HOVER_LIFT: f32 = 0.12;
pub const LABEL_COLOR: Color = Color::from_rgba(252, 253, 255, 255);
/// The key is drawn in a deep shade of the face, quieter than the label.
const KEY_SHADE: f32 = 0.42;

/// A button's color. The Dreamcast's menu gave Play, File, Music and
/// Settings one each; here each button in the bottom row has its own, and
/// the toggles share a pearl gray like the console itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hue {
    Pearl,
    Coral,
    Amber,
    Mint,
    Cyan,
    Orchid,
}

impl Hue {
    /// The face and the rim around it.
    fn colors(self) -> (Color, Color) {
        let (face, rim) = match self {
            Self::Pearl => ((128, 142, 166), (202, 212, 230)),
            Self::Coral => ((214, 118, 100), (250, 186, 168)),
            Self::Amber => ((232, 140, 48), (255, 208, 132)),
            Self::Mint => ((46, 160, 124), (128, 230, 178)),
            Self::Cyan => ((44, 150, 200), (118, 212, 246)),
            Self::Orchid => ((186, 84, 182), (238, 152, 232)),
        };
        let color = |(r, g, b)| Color::from_rgba(r, g, b, 255);
        (color(face), color(rim))
    }
}

/// How a button looks: its color, and whether it is on (Keys shown, the
/// engine paused, Slow or Tracers on) or calls for a press (Start).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look {
    pub hue: Hue,
    pub on: bool,
}

/// `color` moved toward white by `share`, keeping its opacity.
fn whiten(color: Color, share: f32) -> Color {
    let toward = |channel: f32| channel + (1.0 - channel) * share;
    Color::new(toward(color.r), toward(color.g), toward(color.b), color.a)
}

/// Draws the pill of a button filling `rect` (see `glossy`).
pub fn pill(pen: &Pen, rect: Rect, look: Look, hovered: bool) {
    glossy(pen, rect, [rect.h / 2.0; 4], look, hovered);
}

/// Draws a button's shape filling `rect`, with corners of `radii` (top
/// left, top right, bottom right, bottom left): a glow if it is on, the
/// rim, the face, and the sheen, across the top of a wide shape or down
/// the left of a tall one.
pub fn glossy(pen: &Pen, rect: Rect, radii: [f32; 4], look: Look, hovered: bool) {
    let (face, rim) = look.hue.colors();
    if look.on {
        let glow = Rect::new(
            rect.x - GLOW,
            rect.y - GLOW,
            rect.w + 2.0 * GLOW,
            rect.h + 2.0 * GLOW,
        );
        let glow_color = Color {
            a: GLOW_OPACITY,
            ..rim
        };
        shape(pen, glow, radii.map(|radius| radius + GLOW), glow_color);
    }
    let rim = if look.on {
        whiten(rim, ON_RIM_WHITENING)
    } else {
        rim
    };
    shape(pen, rect, radii, rim);
    let lift = if look.on { ON_LIFT } else { 0.0 } + if hovered { HOVER_LIFT } else { 0.0 };
    let inner = Rect::new(
        rect.x + RIM,
        rect.y + RIM,
        rect.w - 2.0 * RIM,
        rect.h - 2.0 * RIM,
    );
    let inner_radii = radii.map(|radius| (radius - RIM).max(0.0));
    shape(pen, inner, inner_radii, whiten(face, lift));
    let wide = inner.w >= inner.h;
    let across = if wide { inner.h } else { inner.w };
    for (share, inset) in SHEEN_LAYERS {
        let (thickness, inset) = (across * share, across * inset);
        let sheen = if wide {
            Rect::new(
                inner.x + inset,
                inner.y + 1.0,
                inner.w - 2.0 * inset,
                thickness,
            )
        } else {
            Rect::new(
                inner.x + 1.0,
                inner.y + inset,
                thickness,
                inner.h - 2.0 * inset,
            )
        };
        shape(pen, sheen, [thickness / 2.0; 4], SHEEN);
    }
}

/// Fills `rect` with corners of `radii` in `color`.
fn shape(pen: &Pen, rect: Rect, radii: [f32; 4], color: Color) {
    pen.fan(rect.center(), &rounded_outline(rect, radii), color);
}

/// The color of a button's key: a deep shade of its face.
pub fn key_color(look: Look) -> Color {
    let (face, _) = look.hue.colors();
    Color::new(
        face.r * KEY_SHADE,
        face.g * KEY_SHADE,
        face.b * KEY_SHADE,
        1.0,
    )
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
    pill(pen, rect, look, hovered);
    let text_width = pen.measure_weight(label.text, size, Weight::Semibold);
    let total = text_width + KEY_GAP + pen.measure(label.key, size);
    let left = rect.x + (rect.w - total) / 2.0;
    let baseline = rect.y + (rect.h + pen.text_height("H", size)) / 2.0;
    pen.heading(label.text, left, baseline, size, LABEL_COLOR);
    pen.text(
        label.key,
        left + text_width + KEY_GAP,
        baseline,
        size,
        key_color(look),
    );
}
