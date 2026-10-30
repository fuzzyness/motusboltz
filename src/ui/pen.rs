//! Text and shape drawing, in three modes: drawing to the screen, warming
//! up (measuring every string a frame will draw, without drawing), and
//! offline for tests, which records the text instead.
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
/// Headings and button labels get a dark outline and a drop shadow, as the
/// menus of Dreamcast games had: the outline's width as a share of the font
/// size (but at least `MIN_OUTLINE` points), the shadow's offset down and to
/// the right as a share of the size, and the dark navy both are drawn in.
const OUTLINE_SHARE: f32 = 0.08;
const MIN_OUTLINE: f32 = 1.2;
const SHADOW_SHARE: f32 = 0.12;
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
    Regular,
    Semibold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Draw,
    Warm,
    #[cfg(test)]
    Offline,
}

/// Draws to the screen, warms the font texture up, or (in tests without a
/// window) only records the text it would have drawn.
pub struct Pen {
    mode: Mode,
    /// A soft shadow under light text, for text over a see-through card.
    shadow: Option<Color>,
    /// Text recorded while offline.
    #[cfg_attr(not(test), allow(dead_code))]
    pub texts: Vec<String>,
}

/// Whether text in `color` is light enough for an outline or a shadow.
fn light(color: Color) -> bool {
    (color.r + color.g + color.b) / 3.0 > OUTLINE_LIGHTNESS
}

/// The outline of a rectangle with its corners rounded by `radii` (top
/// left, top right, bottom right, bottom left; zero leaves a corner
/// square), in order around it clockwise on screen.
pub fn rounded_outline(rect: Rect, radii: [f32; 4]) -> Vec<Vec2> {
    let limit = (rect.w / 2.0).min(rect.h / 2.0).max(0.0);
    let [top_left, top_right, bottom_right, bottom_left] = radii.map(|r| r.clamp(0.0, limit));
    let corners = [
        (
            vec2(rect.x + top_left, rect.y + top_left),
            top_left,
            2.0 * FRAC_PI_2,
        ),
        (
            vec2(rect.right() - top_right, rect.y + top_right),
            top_right,
            3.0 * FRAC_PI_2,
        ),
        (
            vec2(rect.right() - bottom_right, rect.bottom() - bottom_right),
            bottom_right,
            0.0,
        ),
        (
            vec2(rect.x + bottom_left, rect.bottom() - bottom_left),
            bottom_left,
            FRAC_PI_2,
        ),
    ];
    let mut outline = Vec::new();
    for (center, radius, start) in corners {
        arc(&mut outline, center, radius, start, start + FRAC_PI_2);
    }
    outline
}

/// Adds the points of an arc around `center` from angle `start` to `end`
/// (radians, clockwise on screen when `end` is larger), leaving out a point
/// that repeats the last one, as a square corner's would.
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
    pub fn screen() -> Self {
        Self {
            mode: Mode::Draw,
            shadow: None,
            texts: Vec::new(),
        }
    }

    /// A pen that measures everything it is asked to write and draws
    /// nothing.
    pub fn warm() -> Self {
        Self {
            mode: Mode::Warm,
            shadow: None,
            texts: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn offline() -> Self {
        Self {
            mode: Mode::Offline,
            shadow: None,
            texts: Vec::new(),
        }
    }

    pub fn is_warming(&self) -> bool {
        self.mode == Mode::Warm
    }

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

    pub fn text(&mut self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        self.text_weight(text, x, y, size, color, Weight::Regular);
    }

    /// Text in the semibold weight, for headings and key values.
    pub fn strong(&mut self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        self.text_weight(text, x, y, size, color, Weight::Semibold);
    }

    pub fn text_weight(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        color: Color,
        weight: Weight,
    ) {
        match self.mode {
            Mode::Draw => {
                if let Some(shadow) = self.shadow.filter(|_| light(color)) {
                    let shadow = Color {
                        a: shadow.a * color.a,
                        ..shadow
                    };
                    let offset = SOFT_SHADOW_OFFSET;
                    glyphs(text, x + offset, y + offset, size, shadow, weight, 0.0);
                }
                glyphs(text, x, y, size, color, weight, 0.0);
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
        match self.mode {
            Mode::Draw => glyphs(text, x, y, size, color, Weight::Regular, -FRAC_PI_2),
            Mode::Warm => {
                self.measure(text, size);
            }
            #[cfg(test)]
            Mode::Offline => self.texts.push(text.to_owned()),
        }
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

    /// Limits what is drawn after it to `rect` (in screen points), or with
    /// `None` lifts the limit. Pixels count from the top left of the
    /// screen, or of the image being painted, which covers the same area.
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

    pub fn rect(&self, x: f32, y: f32, width: f32, height: f32, color: Color) {
        if self.live() {
            draw_rectangle(x, y, width, height, color);
        }
    }

    pub fn line(&self, x1: f32, y1: f32, x2: f32, y2: f32, thickness: f32, color: Color) {
        if self.live() {
            draw_line(x1, y1, x2, y2, thickness, color);
        }
    }

    pub fn circle(&self, x: f32, y: f32, radius: f32, color: Color) {
        if self.live() {
            draw_circle(x, y, radius, color);
        }
    }

    pub fn triangle(&self, a: Vec2, b: Vec2, c: Vec2, color: Color) {
        if self.live() {
            draw_triangle(a, b, c, color);
        }
    }

    /// A filled rectangle with rounded corners of `radius`, drawn as one fan
    /// of triangles so a translucent color blends evenly. A radius of half
    /// the height gives a pill.
    pub fn round_rect(&self, x: f32, y: f32, width: f32, height: f32, radius: f32, color: Color) {
        let rect = Rect::new(x, y, width, height);
        self.fan(rect.center(), &rounded_outline(rect, [radius; 4]), color);
    }

    /// Fills the shape inside `outline` (its points in order around it) as
    /// one fan of triangles from `center`, from which every point of the
    /// outline must be in plain view. A translucent color blends evenly.
    pub fn fan(&self, center: Vec2, outline: &[Vec2], color: Color) {
        if !self.live() {
            return;
        }
        for (index, &point) in outline.iter().enumerate() {
            let next = outline[(index + 1) % outline.len()];
            draw_triangle(center, point, next, color);
        }
    }

    /// A band `width` wide just inside the closed `outline`, drawn as
    /// pieces that meet edge to edge, so a translucent color stays even.
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
