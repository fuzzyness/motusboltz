//! The canvas fonts, Chakra Petch in two weights.
//!
//! Chakra Petch is a squared-off sans like the menus of late-1990s
//! Dreamcast and PC sports games, to match the scene. It also has the
//! symbols the readings use (Δ, →, −, ≈), which Macroquad's built-in font
//! lacks. The files are cut down to the characters drawn (see
//! `assets/fonts/OFL-ChakraPetch.txt`), to under 20 KB each.

use std::sync::OnceLock;

use macroquad::prelude::{
    Font,
    load_ttf_font_from_bytes, //
};

/// Medium, for running text: the font's Regular is a little thin for small
/// text on a projector.
const REGULAR_TTF: &[u8] = include_bytes!("../../assets/fonts/ChakraPetch-Medium.ttf");
/// Bold, for headings, buttons and key values.
const SEMIBOLD_TTF: &[u8] = include_bytes!("../../assets/fonts/ChakraPetch-Bold.ttf");

/// The two weights.
///
/// A weight that could not be loaded is `None`, and its text falls back to
/// Macroquad's built-in font.
pub struct Fonts {
    /// Weight for running text.
    pub regular: Option<Font>,
    /// Weight for headings, buttons and key values.
    pub semibold: Option<Font>,
}

/// The fonts, once `load` has run.
static FONTS: OnceLock<Fonts> = OnceLock::new();

/// Loads the fonts.
///
/// Loading needs the window, so the app calls this once at startup.
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

#[cfg(test)]
#[path = "../../tests/unit/ui/fonts.rs"]
pub(crate) mod tests;
