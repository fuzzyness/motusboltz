//! Text and shape drawing, in three modes.
//!
//! A pen draws to the screen, warms up (measures every string a frame will
//! draw, without drawing), or, in tests, records the text instead.
//!
//! The warm-up exists because of how Macroquad stores letters: each new
//! letter and size is added to one font texture, which is replaced by a
//! larger one when it fills up. Letters already queued for drawing in that
//! frame still point at the old texture and come out wrong, and the panel,
//! kept as an image, would keep the damage. Measuring adds letters to the
//! texture without queuing anything, so the app measures each frame's text
//! first and draws it second.

use std::f32::consts::{
    FRAC_PI_2,
    TAU, //
};

use macroquad::prelude::{
    Color,
    Font,
    Rect,
    TextParams,
    Vec2,
    draw_circle,
    draw_line,
    draw_rectangle,
    draw_text_ex,
    draw_triangle,
    get_internal_gl,
    measure_text,
    screen_dpi_scale,
    vec2, //
};

use crate::ui::fonts;

/// Approximate character width, in em, used when no font is available.
#[cfg(test)]
const OFFLINE_CHAR_WIDTH_EM: f32 = 0.55;
/// Approximate capital-letter height, in em, used when no font is available.
#[cfg(test)]
const OFFLINE_CHAR_HEIGHT_EM: f32 = 0.7;
/// Segments in each rounded corner.
const CORNER_SEGMENTS: usize = 6;
/// Smallest font size text is shrunk to when fitting a width.
const MIN_FITTED_SIZE: f32 = 9.0;
/// Width of the dark outline of headings and button labels, as a share of
/// the font size.
///
/// Headings and button labels get an outline and a drop shadow, as the
/// menus of Dreamcast games had.
const OUTLINE_SHARE: f32 = 0.08;
/// Narrowest outline, in points.
const MIN_OUTLINE: f32 = 1.2;
/// Offset of the drop shadow down and to the right, as a share of the font
/// size.
const SHADOW_SHARE: f32 = 0.12;
/// The dark navy of the outline and the drop shadow.
const OUTLINE_COLOR: Color = Color::new(0.04, 0.07, 0.2, 0.9);
/// Directions the outline is stamped in around each letter.
const OUTLINE_STEPS: usize = 8;
/// Text darker than this (by its average channel) gets no outline: a dark
/// outline would only blur it.
const OUTLINE_LIGHTNESS: f32 = 0.5;
/// A soft shadow, when one is set (see `set_shadow`), falls this far down
/// and to the right of light text.
const SOFT_SHADOW_OFFSET: f32 = 1.0;
/// The inner edge of a ring keeps at least this share of its width at a
/// sharp corner, so the corner does not spike inward.
const MIN_MITER: f32 = 0.3;

/// The two weights of the canvas font.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    /// For running text.
    Regular,
    /// For headings, buttons and key values.
    Semibold,
}

/// What a pen does with what it is asked to draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// Draws to the screen.
    Draw,
    /// Measures text only, adding its letters to the font texture.
    Warm,
    /// Records text only, for tests without a window.
    #[cfg(test)]
    Offline,
}

/// Draws to the screen, warms the font texture up, or (in tests without a
/// window) only records the text it would have drawn.
pub struct Pen {
    /// What the pen does.
    mode: Mode,
    /// A soft shadow under light text, for text over a see-through card.
    shadow: Option<Color>,
    /// Text recorded while offline.
    #[cfg(test)]
    pub texts: Vec<String>,
}

/// Whether text in `color` is light enough for an outline or a shadow.
fn light(color: Color) -> bool {
    (color.r + color.g + color.b) / 3.0 > OUTLINE_LIGHTNESS
}

/// `rect` grown by `by` on every side, or shrunk for a negative `by`.
pub fn grown(rect: Rect, by: f32) -> Rect {
    Rect::new(
        rect.x - by,
        rect.y - by,
        rect.w + 2.0 * by,
        rect.h + 2.0 * by,
    )
}

/// The outline of `rect` with its corners rounded, in order around it
/// clockwise on screen.
///
/// The radii are for the top left, top right, bottom right and bottom left
/// corners; zero leaves a corner square.
pub fn rounded_outline(rect: Rect, radii: [f32; 4]) -> Vec<Vec2> {
    let limit = (rect.w / 2.0).min(rect.h / 2.0).max(0.0);
    let radii = radii.map(|radius| radius.clamp(0.0, limit));
    // Each corner, the way in from it, and the quarter turn its arc starts
    // at, clockwise from the top left.
    let corners = [
        (rect.point(), vec2(1.0, 1.0), 2.0),
        (vec2(rect.right(), rect.y), vec2(-1.0, 1.0), 3.0),
        (vec2(rect.right(), rect.bottom()), vec2(-1.0, -1.0), 0.0),
        (vec2(rect.x, rect.bottom()), vec2(1.0, -1.0), 1.0),
    ];
    let mut outline = Vec::new();
    for ((corner, inward, quarter), radius) in corners.into_iter().zip(radii) {
        let start = quarter * FRAC_PI_2;
        let (center, end) = (corner + inward * radius, start + FRAC_PI_2);
        arc(&mut outline, center, radius, start, end);
    }
    outline
}

/// Adds the points of an arc around `center` from angle `start` to `end`.
///
/// Angles are in radians, clockwise on screen when `end` is larger. A point
/// that repeats the last one, as a square corner's would, is left out.
pub fn arc(outline: &mut Vec<Vec2>, center: Vec2, radius: f32, start: f32, end: f32) {
    for step in 0..=CORNER_SEGMENTS {
        let angle = start + (end - start) * step as f32 / CORNER_SEGMENTS as f32;
        let point = center + radius * vec2(angle.cos(), angle.sin());
        if outline
            .last()
            .is_none_or(|last: &Vec2| last.distance_squared(point) > 1e-6)
        {
            outline.push(point);
        }
    }
}

/// The font for `weight`, or `None` for Macroquad's built-in font.
fn font(weight: Weight) -> Option<&'static Font> {
    let fonts = fonts::get()?;
    match weight {
        Weight::Regular => fonts.regular.as_ref(),
        Weight::Semibold => fonts.semibold.as_ref().or(fonts.regular.as_ref()),
    }
}

/// The whole font size nearest `size`.
///
/// Font sizes are whole points, so measuring and drawing always ask the
/// font for the same letters.
fn whole(size: f32) -> u16 {
    size.round().max(1.0) as u16
}

/// Draws `text` with its baseline starting at (`x`, `y`), turned by
/// `rotation` radians about that point.
fn glyphs(text: &str, x: f32, y: f32, size: f32, color: Color, weight: Weight, rotation: f32) {
    draw_text_ex(
        text,
        x,
        y,
        TextParams {
            font: font(weight),
            font_size: whole(size),
            color,
            rotation,
            ..Default::default()
        },
    );
}

impl Pen {
    /// A pen in `mode`, with no shadow.
    fn new(mode: Mode) -> Self {
        Self {
            mode,
            shadow: None,
            #[cfg(test)]
            texts: Vec::new(),
        }
    }

    /// A pen that draws to the screen.
    pub fn screen() -> Self {
        Self::new(Mode::Draw)
    }

    /// A pen that measures everything it is asked to write and draws
    /// nothing.
    pub fn warm() -> Self {
        Self::new(Mode::Warm)
    }

    /// A pen that records the text it is asked to write, for tests without
    /// a window.
    #[cfg(test)]
    pub fn offline() -> Self {
        Self::new(Mode::Offline)
    }

    /// Whether the pen only measures.
    pub fn is_warming(&self) -> bool {
        self.mode == Mode::Warm
    }

    /// Whether the pen draws to the screen.
    fn live(&self) -> bool {
        self.mode == Mode::Draw
    }

    /// Gives light text drawn from now on a soft shadow in `color`, or
    /// with `None` takes it away.
    pub fn set_shadow(&mut self, color: Option<Color>) {
        self.shadow = color;
    }

    /// Width of `text` at font `size`.
    pub fn measure(&self, text: &str, size: f32) -> f32 {
        self.measure_weight(text, size, Weight::Regular)
    }

    /// Width of `text` at font `size` in `weight`.
    pub fn measure_weight(&self, text: &str, size: f32, weight: Weight) -> f32 {
        match self.mode {
            #[cfg(test)]
            Mode::Offline => {
                text.chars().count() as f32 * whole(size) as f32 * OFFLINE_CHAR_WIDTH_EM
            }
            _ => measure_text(text, font(weight), whole(size), 1.0).width,
        }
    }

    /// Height of `text` above its baseline at font `size`.
    pub fn text_height(&self, text: &str, size: f32) -> f32 {
        match self.mode {
            #[cfg(test)]
            Mode::Offline => whole(size) as f32 * OFFLINE_CHAR_HEIGHT_EM,
            _ => measure_text(text, font(Weight::Regular), whole(size), 1.0).offset_y,
        }
    }

    /// Draws `text` with its baseline starting at (`x`, `y`).
    pub fn text(&mut self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        self.text_weight(text, x, y, size, color, Weight::Regular);
    }

    /// Text in the semibold weight, for headings and key values.
    pub fn strong(&mut self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        self.text_weight(text, x, y, size, color, Weight::Semibold);
    }

    /// Text in `weight`.
    pub fn text_weight(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        color: Color,
        weight: Weight,
    ) {
        self.write(text, vec2(x, y), size, color, weight, 0.0);
    }

    /// Draws, measures or records `text` with its baseline starting at
    /// `start`, turned by `rotation` radians about that point.
    fn write(
        &mut self,
        text: &str,
        start: Vec2,
        size: f32,
        color: Color,
        weight: Weight,
        rotation: f32,
    ) {
        match self.mode {
            Mode::Draw => {
                let Vec2 { x, y } = start;
                if let Some(shadow) = self.shadow.filter(|_| light(color)) {
                    let shadow = Color {
                        a: shadow.a * color.a,
                        ..shadow
                    };
                    let offset = SOFT_SHADOW_OFFSET;
                    glyphs(text, x + offset, y + offset, size, shadow, weight, rotation);
                }
                glyphs(text, x, y, size, color, weight, rotation);
            }
            Mode::Warm => {
                self.measure_weight(text, size, weight);
            }
            #[cfg(test)]
            Mode::Offline => self.texts.push(text.to_owned()),
        }
    }

    /// A heading or button label: the heavy weight, with a dark outline and
    /// a drop shadow when its color is light.
    pub fn heading(&mut self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        self.outlined(text, x, y, size, color, 0.0);
    }

    /// Like `heading`, turned to read upward from (`x`, `y`), the start of
    /// its baseline.
    pub fn heading_upward(&mut self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        self.outlined(text, x, y, size, color, -FRAC_PI_2);
    }

    /// Like `text`, turned to read upward from (`x`, `y`), the start of its
    /// baseline.
    pub fn text_upward(&mut self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        self.write(text, vec2(x, y), size, color, Weight::Regular, -FRAC_PI_2);
    }

    /// Heavy text turned by `rotation`, outlined and shadowed when light.
    fn outlined(&mut self, text: &str, x: f32, y: f32, size: f32, color: Color, rotation: f32) {
        if !self.live() {
            self.strong(text, x, y, size, color);
            return;
        }
        let size = whole(size) as f32;
        if light(color) {
            let width = (size * OUTLINE_SHARE).max(MIN_OUTLINE);
            let dark = Color {
                a: OUTLINE_COLOR.a * color.a,
                ..OUTLINE_COLOR
            };
            let shadow = size * SHADOW_SHARE;
            let weight = Weight::Semibold;
            glyphs(text, x + shadow, y + shadow, size, dark, weight, rotation);
            for step in 0..OUTLINE_STEPS {
                let angle = TAU * step as f32 / OUTLINE_STEPS as f32;
                let (dx, dy) = (width * angle.cos(), width * angle.sin());
                glyphs(text, x + dx, y + dy, size, dark, weight, rotation);
            }
        }
        glyphs(text, x, y, size, color, Weight::Semibold, rotation);
    }

    /// Like `fitted`, as a heading.
    pub fn fitted_heading(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        width: f32,
        size: f32,
        color: Color,
    ) {
        let size = self.fitted_size_weight(text, width, size, Weight::Semibold);
        self.heading(text, x, y, size, color);
    }

    /// The whole font size, at most `size`, at which `text` fits in
    /// `width`.
    pub fn fitted_size(&self, text: &str, width: f32, size: f32) -> f32 {
        self.fitted_size_weight(text, width, size, Weight::Regular)
    }

    /// The whole font size, at most `size`, at which `text` in `weight`
    /// fits in `width`.
    pub fn fitted_size_weight(&self, text: &str, width: f32, size: f32, weight: Weight) -> f32 {
        let size = whole(size) as f32;
        let natural = self.measure_weight(text, size, weight).max(1.0);
        if natural <= width {
            size
        } else {
            (size * width / natural).floor().max(MIN_FITTED_SIZE)
        }
    }

    /// Draws text left-aligned at `x`, shrinking it only if it would exceed
    /// `width`.
    pub fn fitted(&mut self, text: &str, x: f32, y: f32, width: f32, size: f32, color: Color) {
        let size = self.fitted_size(text, width, size);
        self.text(text, x, y, size, color);
    }

    /// Draws pieces of text one after another from `x`, each in its own
    /// color and weight, and returns where the last one ends.
    pub fn runs(&mut self, pieces: &[(&str, Color, Weight)], x: f32, y: f32, size: f32) -> f32 {
        let mut x = x;
        for &(text, color, weight) in pieces {
            self.text_weight(text, x, y, size, color, weight);
            x += self.measure_weight(text, size, weight);
        }
        x
    }

    /// Total width of `pieces` drawn with `runs`.
    pub fn runs_width(&self, pieces: &[(&str, Color, Weight)], size: f32) -> f32 {
        pieces
            .iter()
            .map(|&(text, _, weight)| self.measure_weight(text, size, weight))
            .sum()
    }

    /// Draws text right-aligned to `right`.
    pub fn text_right(&mut self, text: &str, right: f32, y: f32, size: f32, color: Color) {
        let width = self.measure(text, size);
        self.text(text, right - width, y, size, color);
    }

    /// Limits what is drawn after it to `rect`, in screen points, or with
    /// `None` lifts the limit.
    ///
    /// Pixels count from the top left of the screen, or of the image being
    /// painted, which covers the same area.
    pub fn clip(&self, rect: Option<Rect>) {
        if !self.live() {
            return;
        }
        let dpi = screen_dpi_scale();
        let pixels = |points: f32| (points * dpi).round() as i32;
        // SAFETY: called on the main thread while drawing, which is where
        // Macroquad's own scissor calls run.
        let gl = unsafe { get_internal_gl() };
        gl.quad_gl.scissor(rect.map(|rect| {
            (
                pixels(rect.x),
                pixels(rect.y),
                pixels(rect.w),
                pixels(rect.h),
            )
        }));
    }

    /// Fills `rect`.
    pub fn rect(&self, rect: Rect, color: Color) {
        if self.live() {
            draw_rectangle(rect.x, rect.y, rect.w, rect.h, color);
        }
    }

    /// Draws a straight line from `from` to `to`.
    pub fn line(&self, from: Vec2, to: Vec2, thickness: f32, color: Color) {
        if self.live() {
            draw_line(from.x, from.y, to.x, to.y, thickness, color);
        }
    }

    /// Fills a circle.
    pub fn circle(&self, center: Vec2, radius: f32, color: Color) {
        if self.live() {
            draw_circle(center.x, center.y, radius, color);
        }
    }

    /// Fills a triangle.
    pub fn triangle(&self, a: Vec2, b: Vec2, c: Vec2, color: Color) {
        if self.live() {
            draw_triangle(a, b, c, color);
        }
    }

    /// Fills `rect` with rounded corners of `radius`.
    ///
    /// It is drawn as one fan of triangles so a translucent color blends
    /// evenly. A radius of half the height gives a pill.
    pub fn round_rect(&self, rect: Rect, radius: f32, color: Color) {
        self.fan(rect.center(), &rounded_outline(rect, [radius; 4]), color);
    }

    /// Fills the shape inside `outline`, its points in order around it, as
    /// one fan of triangles from `center`.
    ///
    /// Every point of the outline must be in plain view from `center`. A
    /// translucent color blends evenly.
    pub fn fan(&self, center: Vec2, outline: &[Vec2], color: Color) {
        if !self.live() {
            return;
        }
        for (index, &point) in outline.iter().enumerate() {
            let next = outline[(index + 1) % outline.len()];
            draw_triangle(center, point, next, color);
        }
    }

    /// Fills a band `width` wide just inside the closed `outline`.
    ///
    /// It is drawn as pieces that meet edge to edge, so a translucent color
    /// stays even.
    pub fn ring(&self, outline: &[Vec2], width: f32, color: Color) {
        if !self.live() || outline.len() < 3 {
            return;
        }
        let count = outline.len();
        // Twice the signed area, positive when the outline runs clockwise
        // on screen, which tells which side of each edge is inside.
        let area: f32 = (0..count)
            .map(|index| outline[index].perp_dot(outline[(index + 1) % count]))
            .sum();
        let inward = |from: Vec2, to: Vec2| {
            let along = (to - from).normalize_or_zero();
            vec2(-along.y, along.x) * area.signum()
        };
        let inner: Vec<Vec2> = (0..count)
            .map(|index| {
                let point = outline[index];
                let before = inward(outline[(index + count - 1) % count], point);
                let after = inward(point, outline[(index + 1) % count]);
                let middle = (before + after).normalize_or_zero();
                point + middle * width / middle.dot(after).max(MIN_MITER)
            })
            .collect();
        for index in 0..count {
            let next = (index + 1) % count;
            draw_triangle(outline[index], outline[next], inner[next], color);
            draw_triangle(outline[index], inner[next], inner[index], color);
        }
    }
}
