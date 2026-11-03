//! First-person mode: the view from a toy-sized figure standing on the
//! table, and its menu.
//!
//! The figure is dropped onto the table from its button (see `figure`) and
//! stands at a fixed spot at the front, facing the engine. The mouse,
//! captured by the window, turns its head. Esc lets the mouse go and opens
//! a small menu to resume or leave. Walking comes later.

use std::f32::consts::FRAC_PI_2;

use macroquad::prelude::{
    Camera3D,
    Color,
    Rect,
    Vec2,
    Vec3,
    vec2,
    vec3, //
};

use crate::graphics::block::BlockShape;
use crate::ui::button::{
    self,
    Hue,
    Label,
    Look, //
};
use crate::ui::controls::BACKING;
use crate::ui::panel::{
    QUIET,
    TEXT, //
};
use crate::ui::pen::Pen;

/// Height of the figure's eyes above the table, in bore radii: level with
/// the top of the cylinders, so the flywheel towers over it.
pub const EYE_HEIGHT: f32 = 6.0;
/// Where the figure stands, along the shaft toward the cold side, in bore
/// radii.
const START_ACROSS: f32 = 3.6;
/// Where the figure stands, toward the front, in bore radii.
const START_AHEAD: f32 = 4.4;
/// The point the figure faces at first, above the block's middle, in bore
/// radii above the table.
const FIRST_LOOK_HEIGHT: f32 = 2.5;
/// Turn per point of mouse movement, in radians.
const LOOK_SENSITIVITY: f32 = 0.0025;
/// Moves of the mouse ignored after it is captured: the first move the
/// captured mouse reports can carry a jump (the pointer sent to the
/// window's middle, or all the motion saved up before the capture), which
/// would whip the head round however the mouse is moved.
const SETTLE_MOVES: u8 = 2;
/// Most the head follows the mouse in one frame, in points, so a stray
/// jump in the pointer cannot whip it round either.
const MAX_LOOK_STEP: f32 = 120.0;
/// How far short of straight up or down the head stops, in radians.
const LOOK_MARGIN: f32 = 0.05;
/// Field of view, top to bottom, in radians: wider than the orbit view's,
/// as a game's first-person view is.
const FIELD_OF_VIEW: f32 = 1.05;
/// Nearest distance drawn, in model units (centimeters): close enough for
/// parts within reach.
const NEAR: f32 = 0.2;
/// Farthest distance drawn.
const FAR: f32 = 400.0;

/// Radius of the crosshair's dot, in points.
const CROSSHAIR_RADIUS: f32 = 2.5;
/// Radius of the crosshair's ring, in points.
const CROSSHAIR_RING: f32 = 4.5;
/// The crosshair's dot.
const CROSSHAIR: Color = Color::new(1.0, 1.0, 1.0, 0.85);
/// The ring around it, so it shows on light surfaces too.
const CROSSHAIR_SHADE: Color = Color::new(0.0, 0.0, 0.0, 0.35);
/// The hint at the bottom of the view.
const HINT: &str = "Esc: menu";
/// Text size of the hint.
const HINT_SIZE: f32 = 14.0;
/// Room under the hint, in points.
const HINT_BOTTOM: f32 = 22.0;
/// Room round the hint's text, in points.
const HINT_PAD: f32 = 7.0;

/// Darkening over the view while the menu is open.
const MENU_DIM: Color = Color::new(0.0, 0.0, 0.0, 0.35);
/// The menu's title.
const MENU_TITLE: &str = "First person";
/// Text size of the menu's title.
const MENU_TITLE_SIZE: f32 = 22.0;
/// Width of the menu's buttons, in points.
const MENU_BUTTON_WIDTH: f32 = 230.0;
/// Height of the menu's buttons, in points.
const MENU_BUTTON_HEIGHT: f32 = 40.0;
/// Space between the menu's buttons, and round them on the card.
const MENU_GAP: f32 = 14.0;
/// Room above the buttons for the title, in points.
const MENU_TITLE_ROOM: f32 = 44.0;
/// Text size of the menu's buttons.
const MENU_LABEL_SIZE: f32 = 17.0;
/// Corner radius of the menu's card.
const MENU_RADIUS: f32 = 16.0;

/// What the menu offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// Go on looking around.
    Resume,
    /// Leave first person for the orbit view.
    Exit,
}

impl Choice {
    /// The button's words and key.
    fn label(self) -> Label {
        match self {
            Self::Resume => Label {
                text: "Resume",
                key: "",
            },
            Self::Exit => Label {
                text: "Exit first person",
                key: "",
            },
        }
    }

    /// The button's color.
    fn hue(self) -> Hue {
        match self {
            Self::Resume => Hue::Mint,
            Self::Exit => Hue::Coral,
        }
    }
}

/// The figure standing on the table: where its eyes are and which way it
/// looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Walker {
    /// Its eyes.
    pub eye: Vec3,
    /// Its turn about the vertical, in radians; 0 looks toward -z (from the
    /// front toward the back).
    pub yaw: f32,
    /// Its look up (positive) or down, in radians.
    pub pitch: f32,
    /// The captured mouse's last position, once it has been read.
    last: Option<Vec2>,
    /// Moves of the mouse still to be ignored while the capture settles.
    settling: u8,
    /// The menu is open, and the mouse free.
    pub menu: bool,
}

impl Walker {
    /// The figure at its starting spot on the table of a block of `shape`,
    /// facing the engine.
    pub fn standing(shape: BlockShape) -> Self {
        let unit = shape.bore;
        let eye = vec3(
            START_ACROSS * unit,
            shape.base + EYE_HEIGHT * unit,
            START_AHEAD * unit,
        );
        let target = vec3(0.0, shape.base + FIRST_LOOK_HEIGHT * unit, 0.0);
        let look = (target - eye).normalize();
        Self {
            eye,
            yaw: (-look.x).atan2(-look.z),
            pitch: look.y.asin(),
            last: None,
            settling: SETTLE_MOVES,
            menu: false,
        }
    }

    /// The way it looks, a unit vector.
    pub fn forward(&self) -> Vec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        vec3(-sin_yaw * cos_pitch, sin_pitch, -cos_yaw * cos_pitch)
    }

    /// Turns the head by how far the captured mouse moved to `pointer`
    /// since the last call. The first call after a capture only notes where
    /// it is, the first moves after it are ignored, and a single frame's
    /// turn is held to `MAX_LOOK_STEP`.
    pub fn look(&mut self, pointer: Vec2) {
        let moved = self.last.map(|last| pointer - last);
        if let Some(moved) = moved.filter(|moved| *moved != Vec2::ZERO) {
            if self.settling > 0 {
                self.settling -= 1;
                self.last = Some(pointer);
                return;
            }
            let moved = moved.clamp_length_max(MAX_LOOK_STEP);
            self.yaw -= moved.x * LOOK_SENSITIVITY;
            let limit = FRAC_PI_2 - LOOK_MARGIN;
            self.pitch = (self.pitch - moved.y * LOOK_SENSITIVITY).clamp(-limit, limit);
        }
        self.last = Some(pointer);
    }

    /// Opens the menu, forgetting the mouse, which is let go.
    pub fn open_menu(&mut self) {
        self.menu = true;
        self.last = None;
    }

    /// Closes the menu; the mouse is captured again, from wherever it is.
    pub fn close_menu(&mut self) {
        self.menu = false;
        self.last = None;
        self.settling = SETTLE_MOVES;
    }

    /// The camera at its eyes.
    pub fn camera(&self) -> Camera3D {
        Camera3D {
            position: self.eye,
            target: self.eye + self.forward(),
            up: Vec3::Y,
            fovy: FIELD_OF_VIEW,
            z_near: NEAR,
            z_far: FAR,
            ..Default::default()
        }
    }
}

/// The menu's buttons in a window `width` by `height` points, each with
/// what it does.
pub fn menu_buttons(width: f32, height: f32) -> [(Choice, Rect); 2] {
    let left = (width - MENU_BUTTON_WIDTH) / 2.0;
    let top = height / 2.0 - MENU_BUTTON_HEIGHT - MENU_GAP / 2.0 + MENU_TITLE_ROOM / 2.0;
    [Choice::Resume, Choice::Exit].map(|choice| {
        let row = match choice {
            Choice::Resume => 0.0,
            Choice::Exit => 1.0,
        };
        let y = top + row * (MENU_BUTTON_HEIGHT + MENU_GAP);
        (
            choice,
            Rect::new(left, y, MENU_BUTTON_WIDTH, MENU_BUTTON_HEIGHT),
        )
    })
}

/// The choice under `pointer` in a window `width` by `height`, if any.
pub fn menu_choice(width: f32, height: f32, pointer: Vec2) -> Option<Choice> {
    menu_buttons(width, height)
        .into_iter()
        .find(|(_, rect)| rect.contains(pointer))
        .map(|(choice, _)| choice)
}

/// Draws the crosshair in the middle of a window `width` by `height`, and
/// the hint under it.
pub fn draw_view(pen: &mut Pen, width: f32, height: f32) {
    let middle = vec2(width, height) / 2.0;
    pen.circle(middle, CROSSHAIR_RING, CROSSHAIR_SHADE);
    pen.circle(middle, CROSSHAIR_RADIUS, CROSSHAIR);
    let text = pen.measure(HINT, HINT_SIZE);
    let baseline = height - HINT_BOTTOM;
    let left = (width - text) / 2.0;
    let shade = Rect::new(
        left - HINT_PAD,
        baseline - HINT_SIZE - HINT_PAD / 2.0,
        text + 2.0 * HINT_PAD,
        HINT_SIZE + 2.0 * HINT_PAD,
    );
    pen.round_rect(shade, shade.h / 2.0, BACKING);
    pen.text(HINT, left, baseline, HINT_SIZE, QUIET);
}

/// Draws the menu over a window `width` by `height`, the button under
/// `pointer` lit.
pub fn draw_menu(pen: &mut Pen, width: f32, height: f32, pointer: Vec2) {
    pen.rect(Rect::new(0.0, 0.0, width, height), MENU_DIM);
    let buttons = menu_buttons(width, height);
    let first = buttons[0].1;
    let last = buttons[buttons.len() - 1].1;
    let card = Rect::new(
        first.x - MENU_GAP,
        first.y - MENU_TITLE_ROOM - MENU_GAP,
        first.w + 2.0 * MENU_GAP,
        last.bottom() - first.y + MENU_TITLE_ROOM + 2.0 * MENU_GAP,
    );
    pen.round_rect(card, MENU_RADIUS, BACKING);
    let title = pen.measure(MENU_TITLE, MENU_TITLE_SIZE);
    pen.heading(
        MENU_TITLE,
        (width - title) / 2.0,
        first.y - MENU_GAP,
        MENU_TITLE_SIZE,
        TEXT,
    );
    for (choice, rect) in buttons {
        let look = Look {
            hue: choice.hue(),
            on: false,
        };
        let hovered = rect.contains(pointer);
        button::draw(pen, rect, choice.label(), look, hovered, MENU_LABEL_SIZE);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/first_person.rs"]
mod tests;
