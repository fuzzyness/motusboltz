//! The full-screen button, a round one left of the figure's.
//!
//! It shows four corners pointing out while the simulation is in a window
//! or a page, and pointing in while it fills the screen. Natively the
//! simulation switches its own window; in the browser, the page does, from
//! the click itself (see `page`).

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
use crate::ui::controls::SceneArea;
use crate::ui::figure;
use crate::ui::pen::Pen;

/// Side of the round button, in points: the figure's.
const BUTTON_SIZE: f32 = 40.0;
/// Space between this button and the figure's, in points.
const BUTTON_GAP: f32 = 10.0;
/// How far each corner's tip stands from the middle, along each axis, as a
/// share of the button's side.
const CORNER_REACH: f32 = 0.22;
/// Length of each arm of a corner, as a share of the button's side.
const ARM: f32 = 0.13;
/// Thickness of the corners' strokes, in points.
const STROKE: f32 = 2.5;
/// Color of the corners.
const MARK: Color = Color::from_rgba(240, 244, 250, 255);

/// Where the button goes in `area`: left of the figure's, level with it.
pub fn button_rect(pen: &Pen, area: SceneArea) -> Rect {
    let figure = figure::button_rect(pen, area);
    Rect::new(
        figure.x - BUTTON_GAP - BUTTON_SIZE,
        figure.y,
        BUTTON_SIZE,
        BUTTON_SIZE,
    )
}

/// The strokes of the mark in a button `rect`: four corners pointing out
/// of the middle, or into it while `full`, each as its three points, the
/// tip in the middle.
pub fn corners(rect: Rect, full: bool) -> [[Vec2; 3]; 4] {
    let middle = rect.center();
    let reach = CORNER_REACH * rect.w;
    let arm = ARM * rect.w;
    [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(x, y)| {
        let tip = middle + vec2(x, y) * reach;
        // Pointing out, the arms run back toward the middle; pointing in,
        // away from it.
        let back = if full { 1.0 } else { -1.0 };
        let toward = |step: Vec2| tip + step * arm * back;
        [toward(vec2(x, 0.0)), tip, toward(vec2(0.0, y))]
    })
}

/// Draws the button in `rect`, lit while `hovered`, its mark for a
/// simulation that is `full` screen or not.
pub fn draw_button(pen: &Pen, rect: Rect, hovered: bool, full: bool) {
    let look = Look {
        hue: Hue::Pearl,
        on: false,
    };
    button::glossy(pen, rect, [rect.w / 2.0; 4], look, hovered);
    for corner in corners(rect, full) {
        pen.stroke(&corner, STROKE, MARK);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/full_screen.rs"]
mod tests;
