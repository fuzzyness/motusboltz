//! The little figure that takes the viewer into first person.
//!
//! It waits in a round button beside Hide all. Press it and drag: the
//! figure comes out and hangs from the pointer by its head, swinging the
//! way a held toy does as the pointer moves, its legs swinging a little
//! more. While it is over the table, the table lights up; let go there and
//! the view drops to the figure's eyes. Let go anywhere else and it goes
//! back to its button. The figure is our own: a round head and a stubby
//! body in warm yellow, outlined in dark brown.

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
use crate::ui::controls::{SceneArea, corner_button};
use crate::ui::pen::Pen;

/// Side of the round button, in points.
const BUTTON_SIZE: f32 = 40.0;
/// Space between the button and Hide all, in points.
const BUTTON_GAP: f32 = 10.0;
/// Height of the figure in its button, in points.
const RESTING_HEIGHT: f32 = 26.0;
/// Height of the figure while it is dragged, in points.
const DRAGGED_HEIGHT: f32 = 44.0;
/// The figure's proportions, as shares of its height from the top of its
/// head: the head's radius, the shoulders, the hips and the feet.
const HEAD_RADIUS: f32 = 0.15;
/// Where the shoulders are, down from the top of the head.
const SHOULDERS: f32 = 0.36;
/// Where the hips are.
const HIPS: f32 = 0.7;
/// Where the feet are.
const FEET: f32 = 1.0;
/// How far out each hand reaches from the middle.
const HAND_OUT: f32 = 0.22;
/// How far down each hand hangs.
const HANDS: f32 = 0.6;
/// How far out each foot stands from the middle.
const FOOT_OUT: f32 = 0.12;
/// Thickness of the body, as a share of the height.
const BODY_WIDTH: f32 = 0.2;
/// Thickness of the arms and legs, as a share of the height.
const LIMB_WIDTH: f32 = 0.1;
/// How much thicker the outline is than what it outlines, in points.
const OUTLINE: f32 = 2.5;
/// The figure's yellow.
const YELLOW: Color = Color::from_rgba(255, 200, 48, 255);
/// The figure's outline.
const OUTLINE_COLOR: Color = Color::from_rgba(92, 54, 20, 255);
/// The empty button's mark while its figure is out: a faint ring.
const EMPTY_MARK: Color = Color::new(1.0, 1.0, 1.0, 0.35);
/// Swing per point per second the pointer moves sideways, in radians.
const SWING_PER_SPEED: f32 = 0.0012;
/// Widest swing either way, in radians.
const MAX_SWING: f32 = 0.9;
/// Stiffness of the swing's spring, per second squared.
const SWING_STIFFNESS: f32 = 90.0;
/// Damping of the swing, per second.
const SWING_DAMPING: f32 = 9.0;
/// How much more the legs swing than the body.
const LEG_SWING: f32 = 0.5;

/// Where the figure's button goes in `area`: left of Hide all, level with
/// it.
pub fn button_rect(pen: &Pen, area: SceneArea) -> Rect {
    let corner = corner_button(pen, area);
    let middle = corner.center().y;
    Rect::new(
        corner.x - BUTTON_GAP - BUTTON_SIZE,
        middle - BUTTON_SIZE / 2.0,
        BUTTON_SIZE,
        BUTTON_SIZE,
    )
}

/// The figure while it is dragged: where it hangs and how it swings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    /// The pointer it hangs from.
    pub pointer: Vec2,
    /// Its swing from hanging straight, in radians; positive swings its
    /// feet to the left.
    pub swing: f32,
    /// How fast the swing changes, in radians per second.
    spin: f32,
}

impl Drag {
    /// The figure just lifted out at `pointer`.
    pub fn new(pointer: Vec2) -> Self {
        Self {
            pointer,
            swing: 0.0,
            spin: 0.0,
        }
    }

    /// Moves it with the pointer to `pointer` over `dt` seconds: moving
    /// sideways swings its feet back behind it, and it settles straight
    /// again once the pointer stops.
    pub fn follow(&mut self, pointer: Vec2, dt: f32) {
        if dt > 0.0 {
            let speed = (pointer.x - self.pointer.x) / dt;
            let target = (speed * SWING_PER_SPEED).clamp(-MAX_SWING, MAX_SWING);
            let pull = SWING_STIFFNESS * (target - self.swing) - SWING_DAMPING * self.spin;
            self.spin += pull * dt;
            self.swing = (self.swing + self.spin * dt).clamp(-MAX_SWING, MAX_SWING);
        }
        self.pointer = pointer;
    }

    /// Draws the figure hanging from the pointer.
    pub fn draw(&self, pen: &Pen) {
        draw_figure(pen, self.pointer, DRAGGED_HEIGHT, self.swing);
    }
}

/// Draws the button in `rect`, `hovered` or not, with the figure in it
/// unless it is `out` being dragged.
pub fn draw_button(pen: &Pen, rect: Rect, hovered: bool, out: bool) {
    let look = Look {
        hue: Hue::Pearl,
        on: false,
    };
    button::glossy(pen, rect, [rect.w / 2.0; 4], look, hovered);
    if out {
        pen.circle(rect.center(), rect.w * 0.18, EMPTY_MARK);
        return;
    }
    let top = rect.center() - vec2(0.0, RESTING_HEIGHT / 2.0);
    draw_figure(pen, top, RESTING_HEIGHT, 0.0);
}

/// Draws the figure `height` points tall hanging from `top`, the top of
/// its head, swung `swing` radians from straight down.
fn draw_figure(pen: &Pen, top: Vec2, height: f32, swing: f32) {
    let place = |across: f32, down: f32, turn: f32| {
        let (sin, cos) = turn.sin_cos();
        let local = vec2(across, down) * height;
        // Turning by a positive swing moves what hangs below to the left.
        top + vec2(local.x * cos - local.y * sin, local.x * sin + local.y * cos)
    };
    let leg_swing = swing * (1.0 + LEG_SWING);
    let head = place(0.0, HEAD_RADIUS, swing);
    let shoulders = place(0.0, SHOULDERS, swing);
    let hips = place(0.0, HIPS, swing);
    let hands = [-1.0, 1.0].map(|side| place(side * HAND_OUT, HANDS, swing));
    // Each leg runs from its hip to its foot, the feet swung further.
    let legs = [-1.0, 1.0].map(|side| {
        (
            place(side * FOOT_OUT / 2.0, HIPS, swing),
            place(side * FOOT_OUT, FEET, leg_swing),
        )
    });
    let (body, limb) = (BODY_WIDTH * height, LIMB_WIDTH * height);
    let radius = HEAD_RADIUS * height;
    // The outline first, then the yellow over it.
    for (grow, color) in [(OUTLINE, OUTLINE_COLOR), (0.0, YELLOW)] {
        for (hip, foot) in legs {
            limb_line(pen, hip, foot, limb + grow, color);
        }
        for hand in hands {
            limb_line(pen, shoulders, hand, limb + grow, color);
        }
        limb_line(pen, shoulders, hips, body + grow, color);
        pen.circle(head, radius + grow / 2.0, color);
    }
}

/// A rounded limb of `width` from `from` to `to`.
fn limb_line(pen: &Pen, from: Vec2, to: Vec2, width: f32, color: Color) {
    pen.line(from, to, width, color);
    pen.circle(from, width / 2.0, color);
    pen.circle(to, width / 2.0, color);
}

#[cfg(test)]
#[path = "../../tests/unit/ui/figure.rs"]
mod tests;
