//! Text and shape drawing that tests can run without a window.

use macroquad::prelude::{
    Color,
    draw_circle,
    draw_line,
    draw_rectangle,
    draw_text,
    measure_text, //
};

/// Approximate character width, in em, used when no font is available.
const OFFLINE_CHAR_WIDTH_EM: f32 = 0.55;
/// Approximate capital-letter height, in em, used when no font is available.
const OFFLINE_CHAR_HEIGHT_EM: f32 = 0.7;

/// Draws to the screen or, in tests without a window, only records the
/// text it would have drawn, so layout and wording can be checked. Real
/// Macroquad cannot measure or draw anything without a window.
pub struct Pen {
    live: bool,
    /// Text recorded while not live.
    pub texts: Vec<String>,
}

impl Pen {
    pub fn screen() -> Self {
        Self {
            live: true,
            texts: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn offline() -> Self {
        Self {
            live: false,
            texts: Vec::new(),
        }
    }

    /// Width of `text` at font `size`.
    pub fn measure(&self, text: &str, size: f32) -> f32 {
        if self.live {
            measure_text(text, None, size as u16, 1.0).width
        } else {
            text.len() as f32 * size * OFFLINE_CHAR_WIDTH_EM
        }
    }

    /// Height of `text` above its baseline at font `size`.
    pub fn text_height(&self, text: &str, size: f32) -> f32 {
        if self.live {
            measure_text(text, None, size as u16, 1.0).height
        } else {
            size * OFFLINE_CHAR_HEIGHT_EM
        }
    }

    pub fn text(&mut self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        if self.live {
            draw_text(text, x, y, size, color);
        } else {
            self.texts.push(text.to_owned());
        }
    }

    /// The font size at most `size` at which `text` fits in `width`.
    pub fn fitted_size(&self, text: &str, width: f32, size: f32) -> f32 {
        size * (width / self.measure(text, size).max(1.0)).min(1.0)
    }

    /// Draws text left-aligned at `x`, shrinking it only if it would exceed
    /// `width`.
    pub fn fitted(&mut self, text: &str, x: f32, y: f32, width: f32, size: f32, color: Color) {
        let size = self.fitted_size(text, width, size);
        self.text(text, x, y, size, color);
    }

    pub fn rect(&self, x: f32, y: f32, width: f32, height: f32, color: Color) {
        if self.live {
            draw_rectangle(x, y, width, height, color);
        }
    }

    pub fn line(&self, x1: f32, y1: f32, x2: f32, y2: f32, thickness: f32, color: Color) {
        if self.live {
            draw_line(x1, y1, x2, y2, thickness, color);
        }
    }

    pub fn circle(&self, x: f32, y: f32, radius: f32, color: Color) {
        if self.live {
            draw_circle(x, y, radius, color);
        }
    }
}
