//! The two side drawers: the readings panel at the window's left edge and
//! the sliders at its right.
//!
//! Each is a rounded, see-through card, like frosted glass, edged with a
//! faint light rim. Beside its outer side, against the window's edge,
//! stands a slim tab of the same glass that opens and closes it. While the
//! drawer is out, the tab and the card are one shape, joined by curved
//! fillets; closed, the tab stands out of the window's edge through the
//! same fillets, as if it came from the side of the screen. The tab never moves, so the
//! pointer that opens a drawer can close it again where it is.
//!
//! A drawer slides out from its edge when it opens and back when it
//! closes. Resting the pointer on a closed drawer's tab peeks it: the
//! drawer slides all the way out over the scene, usable as if open, and
//! slides back once the pointer leaves it, unless a press that began in it
//! is still held. A click on the tab keeps the drawer open, stuck out,
//! which a light glow fading in round the tab shows; another click closes
//! it.

use std::f32::consts::{
    FRAC_PI_2,
    PI, //
};

use macroquad::prelude::{
    Color,
    Rect,
    Vec2,
    vec2, //
};

use crate::ui::button;
use crate::ui::layout::EDGE_TAB_WIDTH;
use crate::ui::motion::{
    advance,
    faded,
    smooth, //
};
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
/// Height of a drawer's button.
///
/// Its width is `EDGE_TAB_WIDTH`.
const TAB_HEIGHT: f32 = 136.0;
/// Radius of the tab's corners.
const TAB_RADIUS: f32 = 11.0;
/// Radius of the fillets that join the tab to its card, or to the
/// window's edge while the drawer is closed.
const FILLET: f32 = 9.0;
/// How far past the window's edge a closed tab's outline closes.
const BEYOND_EDGE: f32 = 4.0;
/// The glow of a stuck drawer, from the widest layer to the brightest:
/// each layer's width and color.
const STUCK_GLOW: [(f32, Color); 3] = [
    (9.0, Color::new(0.62, 0.84, 1.0, 0.12)),
    (5.0, Color::new(0.7, 0.88, 1.0, 0.22)),
    (2.0, Color::new(0.9, 0.97, 1.0, 0.85)),
];
/// How far the glow's trails reach along the card, fading to nothing.
const TRAIL_LENGTH: f32 = 70.0;
/// Pieces in a trail, each a little fainter than the one before.
const TRAIL_PIECES: usize = 24;
/// Color of the key on a tab.
const TAB_KEY: Color = Color::from_rgba(176, 190, 208, 255);
/// Text size of the edge button's label.
const LABEL_SIZE: f32 = 15.0;
/// Gap between the edge button's label and its key.
const LABEL_KEY_GAP: f32 = 7.0;

/// Seconds a drawer takes to slide all the way out or back.
pub const SLIDE_SECONDS: f32 = 0.25;

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

/// How far out of its edge a drawer has slid, and whether it is peeked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slide {
    /// How far out, from 0 (out of sight) to 1 (in its open place).
    out: f32,
    /// Whether the pointer has drawn the drawer out without opening it.
    peeking: bool,
}

impl Slide {
    /// A drawer at rest, out if `open`.
    pub fn new(open: bool) -> Self {
        Self {
            out: if open { 1.0 } else { 0.0 },
            peeking: false,
        }
    }

    /// Moves the slide on by `dt` seconds for a drawer that is `open` or
    /// not, with the pointer over its edge button (`on_tab`) or its card
    /// (`on_card`), and a press begun in it still `held`.
    pub fn update(&mut self, open: bool, on_tab: bool, on_card: bool, held: bool, dt: f32) {
        self.peeking = !open && (on_tab || (self.peeking && (on_card || held)));
        self.out = advance(self.out, open || self.peeking, dt, SLIDE_SECONDS);
    }

    /// Whether the drawer is peeked.
    pub fn peeking(&self) -> bool {
        self.peeking
    }

    /// Whether any of the drawer shows.
    pub fn shown(&self) -> bool {
        self.out > 0.0
    }

    /// Whether the drawer is all the way out, so its contents take input.
    pub fn settled_out(&self) -> bool {
        self.out >= 1.0
    }

    /// How far the drawer stands from its open place along x, toward its
    /// `edge`, for a slide `distance` long, or `None` out of sight.
    pub fn shift(&self, edge: Edge, distance: f32) -> Option<f32> {
        self.shown()
            .then(|| edge.outward() * distance * (1.0 - smooth(self.out)))
    }
}

/// How far a drawer's `card` at `edge` of a window `screen_width` wide
/// slides to leave the window.
pub fn slide_distance(card: Rect, edge: Edge, screen_width: f32) -> f32 {
    match edge {
        Edge::Left => card.right(),
        Edge::Right => screen_width - card.x,
    }
}

/// All a drawer's `card` at `edge` of a window `screen_width` wide covers,
/// from the window's edge, its button included, to the card's inner side:
/// where the pointer keeps it peeked.
pub fn reach(card: Rect, edge: Edge, screen_width: f32) -> Rect {
    match edge {
        Edge::Left => Rect::new(0.0, card.y, card.right(), card.h),
        Edge::Right => Rect::new(card.x, card.y, screen_width - card.x, card.h),
    }
}

/// The button of a drawer's `card` at `edge` of a window `screen_width`
/// wide: against the window's edge, beside the card's outer side, level
/// with its middle.
pub fn tab(card: Rect, edge: Edge, screen_width: f32) -> Rect {
    let x = match edge {
        Edge::Left => 0.0,
        Edge::Right => screen_width - EDGE_TAB_WIDTH,
    };
    let middle = card.center().y;
    Rect::new(x, middle - TAB_HEIGHT / 2.0, EDGE_TAB_WIDTH, TAB_HEIGHT)
}

/// The outline of a drawer's `card` at `edge`, clockwise on screen, joined
/// to its `tab` when one is given.
///
/// The outline is built with the tab at the card's right, as the sliders'
/// is, then mirrored for the panel's, whose tab is at its left.
pub fn card_outline(card: Rect, edge: Edge, tab: Option<Rect>) -> Vec<Vec2> {
    let (left, top, right, bottom) = (card.x, card.y, card.right(), card.bottom());
    let radius = CARD_RADIUS.min(card.w / 2.0).min(card.h / 2.0);
    // Each arc's center, radius, and the quarter turns it runs between.
    let mut arcs = vec![
        (vec2(left + radius, top + radius), radius, 2.0, 3.0),
        (vec2(right - radius, top + radius), radius, 3.0, 4.0),
    ];
    if let Some(tab) = tab {
        let outer = right + tab.w - TAB_RADIUS;
        let (upper, lower) = (tab.y + TAB_RADIUS, tab.bottom() - TAB_RADIUS);
        let fillet = right + FILLET;
        let (above, below) = (tab.y - FILLET, tab.bottom() + FILLET);
        // Down the card's side into the upper fillet, which curves out
        // onto the tab's top, and back by the lower one.
        arcs.extend([
            (vec2(fillet, above), FILLET, 2.0, 1.0),
            (vec2(outer, upper), TAB_RADIUS, 3.0, 4.0),
            (vec2(outer, lower), TAB_RADIUS, 0.0, 1.0),
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
    if edge == Edge::Left {
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

/// The middle of a stuck drawer's glow, where its tab joins its `card` at
/// `edge`: from the card's outer side through the upper fillet, round the
/// tab's two corners at the window's edge and back through the lower
/// fillet, as the joined outline runs (see `card_outline`).
pub fn glow_path(card: Rect, tab: Rect, edge: Edge) -> Vec<Vec2> {
    // From the card's outer side, out toward the window's edge.
    let (side, out) = match edge {
        Edge::Left => (card.x, -1.0),
        Edge::Right => (card.right(), 1.0),
    };
    bump(side, out, tab)
}

/// A tab's outline standing out from a straight side at `base` along x,
/// the way `out` (-1 left, 1 right), as wide as `tab` and as high: down
/// the side into a fillet that curves out onto the tab's top, round its
/// two far corners, and back through the lower fillet onto the side.
///
/// The fillets reach past the tab's top and bottom by `FILLET`.
fn bump(base: f32, out: f32, tab: Rect) -> Vec<Vec2> {
    let (top, bottom) = (tab.y, tab.bottom());
    let fillet_x = base + out * FILLET;
    let corner_x = base + out * (tab.w - TAB_RADIUS);
    // Each arc's center, radius and the angles it runs between, for a tab
    // standing out to the right; one standing out to the left is mirrored
    // about the vertical.
    let mirror = |angle: f32| if out < 0.0 { PI - angle } else { angle };
    let arcs = [
        (vec2(fillet_x, top - FILLET), FILLET, PI, PI / 2.0),
        (
            vec2(corner_x, top + TAB_RADIUS),
            TAB_RADIUS,
            1.5 * PI,
            2.0 * PI,
        ),
        (
            vec2(corner_x, bottom - TAB_RADIUS),
            TAB_RADIUS,
            0.0,
            PI / 2.0,
        ),
        (vec2(fillet_x, bottom + FILLET), FILLET, 1.5 * PI, PI),
    ];
    let mut path = Vec::new();
    for (center, radius, start, end) in arcs {
        arc(&mut path, center, radius, mirror(start), mirror(end));
    }
    path
}

/// The trails a stuck drawer's glow fades away along, up and down its
/// `card`'s outer side from where the tab's fillets meet it: short pieces
/// end to end, each as its two ends and how strongly it shows, from 1
/// down toward 0.
pub fn glow_trails(card: Rect, tab: Rect, edge: Edge) -> Vec<(Vec2, Vec2, f32)> {
    let side = match edge {
        Edge::Left => card.x,
        Edge::Right => card.right(),
    };
    // The trails stop short of the card's own rounded corners.
    let (upper, lower) = (tab.y - FILLET, tab.bottom() + FILLET);
    let room = [
        (upper, -1.0, upper - (card.y + CARD_RADIUS)),
        (lower, 1.0, card.bottom() - CARD_RADIUS - lower),
    ];
    let piece = TRAIL_LENGTH / TRAIL_PIECES as f32;
    let mut pieces = Vec::new();
    for (from, way, room) in room {
        for index in 0..TRAIL_PIECES {
            let start = index as f32 * piece;
            let end = (start + piece).min(room);
            if start >= end {
                break;
            }
            let fade = 1.0 - (start + end) / 2.0 / TRAIL_LENGTH;
            let point = |along: f32| vec2(side, from + way * along);
            pieces.push((point(start), point(end), fade));
        }
    }
    pieces
}

/// The outline of the tab of a closed drawer at `edge`, in a window
/// `screen_width` wide: standing out of the window's edge as it stands out
/// of its card when open, through curved fillets there, rounded on its
/// side toward the scene.
///
/// The outline closes just beyond the window's edge, so its rim does not
/// show along it.
pub fn closed_tab_outline(tab: Rect, edge: Edge, screen_width: f32) -> Vec<Vec2> {
    // From the window's edge, in toward the scene.
    let (wall, inward) = match edge {
        Edge::Left => (0.0, 1.0),
        Edge::Right => (screen_width, -1.0),
    };
    let mut outline = bump(wall, inward, tab);
    let beyond = wall - inward * BEYOND_EDGE;
    outline.push(vec2(beyond, tab.bottom() + FILLET));
    outline.push(vec2(beyond, tab.y - FILLET));
    outline
}

/// Draws a drawer's `card` at `edge`, joined to its `tab` when one is
/// given.
pub fn draw_card(pen: &Pen, card: Rect, edge: Edge, tab: Option<Rect>) {
    let outline = card_outline(card, edge, tab);
    pen.fan(card.center(), &outline, CARD);
    pen.ring(&outline, RIM_WIDTH, RIM);
}

/// The look of a drawer's tab this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TabLook {
    /// How far the glow of a drawer stuck open has faded in, from 0 (none,
    /// closed or peeked) to 1.
    pub glow: f32,
    /// The card the tab is drawn as one with (see `draw_card`), while the
    /// drawer is all the way out, so only its words and glow go on top.
    pub joined: Option<Rect>,
}

/// Draws the tab `rect` of the drawer at `edge` in `look`, with `label`
/// and `key` reading upward.
///
/// While the drawer is stuck open and joined to its card, a glow runs
/// round the tab's outer border, through the fillets and up and down the
/// card's outer side, where it fades away (see `glow_path` and
/// `glow_trails`).
pub fn draw_tab(
    pen: &mut Pen,
    rect: Rect,
    edge: Edge,
    screen_width: f32,
    label: &str,
    key: &str,
    look: TabLook,
) {
    match look.joined {
        Some(card) if look.glow > 0.0 => draw_glow(pen, card, rect, edge, look.glow),
        Some(_) => {}
        None => {
            let outline = closed_tab_outline(rect, edge, screen_width);
            let wall = match edge {
                Edge::Left => rect.x,
                Edge::Right => rect.right(),
            };
            // Fanned from the wall, which sees the whole shape.
            pen.fan(vec2(wall, rect.center().y), &outline, CARD);
            pen.ring(&outline, RIM_WIDTH, RIM);
        }
    }
    // Upward text grows toward the screen's left from its baseline, so the
    // baseline sits right of the middle by half the letters' height.
    let middle_x = rect.center().x;
    let label_width = pen.measure_weight(label, LABEL_SIZE, Weight::Semibold);
    let key_width = pen.measure(key, LABEL_SIZE);
    let length = label_width + LABEL_KEY_GAP + key_width;
    let start = rect.y + (rect.h + length) / 2.0;
    let baseline = middle_x + pen.text_height("H", LABEL_SIZE) / 2.0;
    pen.heading_upward(label, baseline, start, LABEL_SIZE, button::LABEL_COLOR);
    let key_start = start - label_width - LABEL_KEY_GAP;
    pen.text_upward(key, baseline, key_start, LABEL_SIZE, TAB_KEY);
}

/// Draws the glow of a stuck drawer whose `tab` at `edge` is joined to its
/// `card`, faded in to `shown`.
fn draw_glow(pen: &Pen, card: Rect, tab: Rect, edge: Edge, shown: f32) {
    let path = glow_path(card, tab, edge);
    let trails = glow_trails(card, tab, edge);
    for (width, color) in STUCK_GLOW {
        pen.stroke(&path, width, faded(color, shown));
        for &(from, to, fade) in &trails {
            pen.stroke(&[from, to], width, faded(color, shown * fade));
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/drawer.rs"]
mod tests;
