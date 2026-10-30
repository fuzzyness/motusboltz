//! The canvas fonts: Fira Sans, the web page's own typeface, so the engine
//! and the page read as one piece. It also has the symbols the readings use
//! (Δ, →, −, ≈), which Macroquad's built-in font lacks.

use std::sync::OnceLock;

use macroquad::prelude::{
    Font,
    load_ttf_font_from_bytes, //
};

const REGULAR_TTF: &[u8] = include_bytes!("../../assets/fonts/FiraSans-Regular.ttf");
const SEMIBOLD_TTF: &[u8] = include_bytes!("../../assets/fonts/FiraSans-SemiBold.ttf");

/// Every character the canvas may draw: printable ASCII and these symbols,
/// all present in both font files. A test holds the UI text to this set.
#[cfg(test)]
pub const SYMBOLS: &str = "Δ→←↑↓≈−·×≥≤…—°²³";

/// The two weights, or `None` if a font could not be loaded, in which case
/// text falls back to Macroquad's built-in font.
pub struct Fonts {
    pub regular: Option<Font>,
    pub semibold: Option<Font>,
}

static FONTS: OnceLock<Fonts> = OnceLock::new();

/// Loads the fonts; needs the window, so the app calls it once at startup.
pub fn load() {
    FONTS.get_or_init(|| Fonts {
        regular: load_ttf_font_from_bytes(REGULAR_TTF).ok(),
        semibold: load_ttf_font_from_bytes(SEMIBOLD_TTF).ok(),
    });
}

/// The loaded fonts, if `load` has run.
pub fn get() -> Option<&'static Fonts> {
    FONTS.get()
}

/// Whether `text` uses only characters the canvas fonts have.
#[cfg(test)]
pub fn drawable(text: &str) -> bool {
    text.chars()
        .all(|c| (' '..='~').contains(&c) || SYMBOLS.contains(c))
}

#[cfg(test)]
#[path = "../../tests/unit/ui/fonts.rs"]
mod tests;
