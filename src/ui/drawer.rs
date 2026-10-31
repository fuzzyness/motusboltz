//! The two side drawers: the readings panel at the window's left edge and
//! the sliders at its right.
//!
//! Each is a rounded, see-through card, like frosted glass, with a hump on
//! its inner side holding a small glossy grip that closes it; closed, it
//! leaves a slim glossy button hugging the window's edge that opens it
//! again. The card and its hump are one shape, joined by curved fillets,
//! filled in one fan and edged with a faint light rim. The grip and the
//! edge button are drawn like the other buttons (see `button`), in pearl
//! gray.

use std::f32::consts::FRAC_PI_2;

use macroquad::prelude::{
    Color,
    Rect,
    Vec2,
    vec2, //
};

use crate::ui::button::{
    self,
    Hue,
    Look, //
};
use crate::ui::layout::EDGE_TAB_WIDTH;
use crate::ui::pen::{
    Pen,
    Weight,
    arc, //
};

/// Corner radius of the cards.
pub const CARD_RADIUS: f32 = 22.0;
/// The cards' fill, a see-through navy.
const CARD: Color = Color::new(0.06, 0.1, 0.18, 0.48);
/// The cards' rim.
const RIM: Color = Color::new(0.85, 0.92, 1.0, 0.2);
/// Width of the rim.
const RIM_WIDTH: f32 = 1.5;
/// How far the hump stands out from the card.
pub const HUMP_DEPTH: f32 = 26.0;
/// Height of the hump.
const HUMP_HEIGHT: f32 = 92.0;
/// Radius of the hump's outer corners.
const HUMP_RADIUS: f32 = 11.0;
/// Radius of the fillets that join the hump to the card.
const FILLET: f32 = 9.0;
/// Height of the button a closed drawer leaves at the window's edge.
///
/// Its width is `EDGE_TAB_WIDTH`.
const TAB_HEIGHT: f32 = 136.0;
/// Radius of the edge button's two inner corners.
const TAB_RADIUS: f32 = 13.0;
/// Width of the grip in an open drawer's hump.
const GRIP_WIDTH: f32 = 18.0;
/// Height of the grip.
const GRIP_HEIGHT: f32 = 64.0;
/// Half the height of the arrow on a grip or edge button.
const ARROW_HALF_HEIGHT: f32 = 5.5;
/// Length of the arrow.
const ARROW_LENGTH: f32 = 7.0;
/// The dark shadow under the arrow.
const ARROW_SHADOW: Color = Color::new(0.04, 0.07, 0.2, 0.8);
/// Offset of the arrow's shadow down and to the right.
const ARROW_SHADOW_OFFSET: f32 = 1.2;
/// Text size of the key letter on a grip.
const KEY_SIZE: f32 = 13.0;
/// Text size of the edge button's label.
const LABEL_SIZE: f32 = 15.0;
/// Gap between the edge button's label and its key.
const LABEL_KEY_GAP: f32 = 7.0;
/// From a grip's top to its arrow's middle.
const GRIP_ARROW_TOP: f32 = 17.0;
/// From a grip's bottom to its key's baseline.
const GRIP_KEY_BOTTOM: f32 = 12.0;
/// From an edge button's top to its arrow's middle.
const TAB_ARROW_TOP: f32 = 20.0;
/// From the edge button's arrow to its label.
const TAB_LABEL_GAP: f32 = 14.0;
/// The grip and the edge button are pearl buttons, never on.
const LOOK: Look = Look {
    hue: Hue::Pearl,
    on: false,
};

/// The window edge a drawer belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// The panel's edge.
    Left,
    /// The sliders' edge.
    Right,
}

impl Edge {
    /// The way toward the edge along x: -1 at the left, 1 at the right.
    fn outward(self) -> f32 {
        match self {
            Self::Left => -1.0,
            Self::Right => 1.0,
        }
    }
}

/// The hump on the inner side of a drawer's `card` at `edge`, at the
/// card's middle height.
pub fn hump(card: Rect, edge: Edge) -> Rect {
    let x = match edge {
        Edge::Left => card.right(),
        Edge::Right => card.x - HUMP_DEPTH,
    };
    let top = card.center().y - HUMP_HEIGHT / 2.0;
    Rect::new(x, top, HUMP_DEPTH, HUMP_HEIGHT)
}

/// The tab a closed drawer leaves at `edge` of a window `screen_width`
/// wide, centered on height `middle`.
pub fn tab(edge: Edge, screen_width: f32, middle: f32) -> Rect {
    let x = match edge {
        Edge::Left => 0.0,
        Edge::Right => screen_width - EDGE_TAB_WIDTH,
    };
    Rect::new(x, middle - TAB_HEIGHT / 2.0, EDGE_TAB_WIDTH, TAB_HEIGHT)
}

/// The outline of a drawer's `card` at `edge`, clockwise on screen, with
/// its hump when `with_hump` is set.
///
/// The outline is built with the hump at the right, then mirrored for a
/// drawer at the right edge.
pub fn card_outline(card: Rect, edge: Edge, with_hump: bool) -> Vec<Vec2> {
    let (left, top, right, bottom) = (card.x, card.y, card.right(), card.bottom());
    let radius = CARD_RADIUS.min(card.w / 2.0).min(card.h / 2.0);
    // Each arc's center, radius, and the quarter turns it runs between.
    let mut arcs = vec![
        (vec2(left + radius, top + radius), radius, 2.0, 3.0),
        (vec2(right - radius, top + radius), radius, 3.0, 4.0),
    ];
    if with_hump {
        let hump = hump(card, Edge::Left);
        let outer = hump.right() - HUMP_RADIUS;
        let (upper, lower) = (hump.y + HUMP_RADIUS, hump.bottom() - HUMP_RADIUS);
        let fillet = right + FILLET;
        let (above, below) = (hump.y - FILLET, hump.bottom() + FILLET);
        // Down the card's side into the upper fillet, which curves out
        // onto the hump's top, and back by the lower one.
        arcs.extend([
            (vec2(fillet, above), FILLET, 2.0, 1.0),
            (vec2(outer, upper), HUMP_RADIUS, 3.0, 4.0),
            (vec2(outer, lower), HUMP_RADIUS, 0.0, 1.0),
            (vec2(fillet, below), FILLET, 3.0, 2.0),
        ]);
    }
    arcs.extend([
        (vec2(right - radius, bottom - radius), radius, 0.0, 1.0),
        (vec2(left + radius, bottom - radius), radius, 1.0, 2.0),
    ]);
    let mut outline = Vec::new();
    for (center, radius, start, end) in arcs {
        let (start, end) = (start * FRAC_PI_2, end * FRAC_PI_2);
        arc(&mut outline, center, radius, start, end);
    }
    if edge == Edge::Right {
        // Mirrored across the card's middle; reversed, so it still runs
        // clockwise.
        let middle = card.center().x;
        outline = outline
            .into_iter()
            .rev()
            .map(|point| vec2(2.0 * middle - point.x, point.y))
            .collect();
    }
    outline
}

/// Draws a drawer's `card` at `edge`, with its hump when `with_hump` is
/// set.
pub fn draw_card(pen: &Pen, card: Rect, edge: Edge, with_hump: bool) {
    let outline = card_outline(card, edge, with_hump);
    pen.fan(card.center(), &outline, CARD);
    pen.ring(&outline, RIM_WIDTH, RIM);
}

/// Draws the arrow on a grip or edge button, centered on `middle`.
///
/// It points along x by `toward` (-1 left, 1 right), white over a dark
/// shadow.
fn arrow(pen: &Pen, middle: Vec2, toward: f32) {
    let points = |middle: Vec2| {
        let tip = middle + vec2(toward * ARROW_LENGTH / 2.0, 0.0);
        let back = middle.x - toward * ARROW_LENGTH / 2.0;
        (
            tip,
            vec2(back, middle.y - ARROW_HALF_HEIGHT),
            vec2(back, middle.y + ARROW_HALF_HEIGHT),
        )
    };
    let (a, b, c) = points(middle + Vec2::splat(ARROW_SHADOW_OFFSET));
    pen.triangle(a, b, c, ARROW_SHADOW);
    let (a, b, c) = points(middle);
    pen.triangle(a, b, c, button::LABEL_COLOR);
}

/// The grip in an open drawer's `hump` (see `hump`), centered in the part
/// that stands out from the card.
pub fn grip(hump: Rect) -> Rect {
    let left = hump.x + (hump.w - GRIP_WIDTH) / 2.0;
    Rect::new(
        left,
        hump.center().y - GRIP_HEIGHT / 2.0,
        GRIP_WIDTH,
        GRIP_HEIGHT,
    )
}

/// Draws the grip in an open drawer's `hump` at `edge`.
///
/// The grip is a glossy pearl button with an arrow toward `edge`, to close
/// the drawer, over its `key`.
pub fn draw_hump_mark(pen: &mut Pen, hump: Rect, edge: Edge, key: &str, hovered: bool) {
    let grip = grip(hump);
    button::pill(pen, grip, LOOK, hovered);
    let middle_x = grip.center().x;
    arrow(pen, vec2(middle_x, grip.y + GRIP_ARROW_TOP), edge.outward());
    let key_left = middle_x - pen.measure_weight(key, KEY_SIZE, Weight::Semibold) / 2.0;
    let baseline = grip.bottom() - GRIP_KEY_BOTTOM;
    pen.heading(key, key_left, baseline, KEY_SIZE, button::LABEL_COLOR);
}

/// Draws the button `rect` that a closed drawer leaves at `edge`.
///
/// The button is glossy pearl, flat against the edge and rounded on its
/// inner side, with an arrow pointing in, to open the drawer, over `label`
/// and `key`, reading upward.
pub fn draw_tab(pen: &mut Pen, rect: Rect, edge: Edge, label: &str, key: &str, hovered: bool) {
    let radii = match edge {
        Edge::Left => [0.0, TAB_RADIUS, TAB_RADIUS, 0.0],
        Edge::Right => [TAB_RADIUS, 0.0, 0.0, TAB_RADIUS],
    };
    button::glossy(pen, rect, radii, LOOK, hovered);
    let middle_x = rect.center().x;
    arrow(pen, vec2(middle_x, rect.y + TAB_ARROW_TOP), -edge.outward());
    // Upward text grows toward the screen's left from its baseline, so the
    // baseline sits right of the middle by half the letters' height.
    let label_width = pen.measure_weight(label, LABEL_SIZE, Weight::Semibold);
    let key_width = pen.measure(key, LABEL_SIZE);
    let length = label_width + LABEL_KEY_GAP + key_width;
    let room_top = rect.y + TAB_ARROW_TOP + TAB_LABEL_GAP;
    let start = room_top + (rect.bottom() - room_top + length) / 2.0;
    let baseline = middle_x + pen.text_height("H", LABEL_SIZE) / 2.0;
    pen.heading_upward(label, baseline, start, LABEL_SIZE, button::LABEL_COLOR);
    let key_start = start - label_width - LABEL_KEY_GAP;
    pen.text_upward(
        key,
        baseline,
        key_start,
        LABEL_SIZE,
        button::key_color(LOOK),
    );
}

#[cfg(test)]
#[path = "../../tests/unit/ui/drawer.rs"]
mod tests;
