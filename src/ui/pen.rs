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

use std::f32::consts::FRAC_PI_2;

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
    /// Text recorded while offline.
    #[cfg_attr(not(test), allow(dead_code))]
    pub texts: Vec<String>,
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

impl Pen {
    pub fn screen() -> Self {
        Self {
            mode: Mode::Draw,
            texts: Vec::new(),
        }
    }

    /// A pen that measures everything it is asked to write and draws
    /// nothing.
    pub fn warm() -> Self {
        Self {
            mode: Mode::Warm,
            texts: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn offline() -> Self {
        Self {
            mode: Mode::Offline,
            texts: Vec::new(),
        }
    }

    pub fn is_warming(&self) -> bool {
        self.mode == Mode::Warm
    }

    fn live(&self) -> bool {
        self.mode == Mode::Draw
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
                draw_text_ex(
                    text,
                    x,
                    y,
                    TextParams {
                        font: font(weight),
                        font_size: whole(size),
                        color,
                        ..Default::default()
                    },
                );
            }
            Mode::Warm => {
                self.measure_weight(text, size, weight);
            }
            #[cfg(test)]
            Mode::Offline => self.texts.push(text.to_owned()),
        }
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

    /// Like `fitted`, in the semibold weight.
    pub fn fitted_strong(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        width: f32,
        size: f32,
        color: Color,
    ) {
        let size = self.fitted_size_weight(text, width, size, Weight::Semibold);
        self.strong(text, x, y, size, color);
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
        if !self.live() {
            return;
        }
        let radius = radius.min(width / 2.0).min(height / 2.0).max(0.0);
        let corners = [
            (vec2(x + width - radius, y + radius), -FRAC_PI_2),
            (vec2(x + width - radius, y + height - radius), 0.0),
            (vec2(x + radius, y + height - radius), FRAC_PI_2),
            (vec2(x + radius, y + radius), 2.0 * FRAC_PI_2),
        ];
        let outline: Vec<Vec2> = corners
            .into_iter()
            .flat_map(|(center, start)| {
                (0..=CORNER_SEGMENTS).map(move |step| {
                    let angle = start + FRAC_PI_2 * step as f32 / CORNER_SEGMENTS as f32;
                    center + radius * vec2(angle.cos(), angle.sin())
                })
            })
            .collect();
        let middle = vec2(x + width / 2.0, y + height / 2.0);
        for (index, &point) in outline.iter().enumerate() {
            let next = outline[(index + 1) % outline.len()];
            draw_triangle(middle, point, next, color);
        }
    }
}
