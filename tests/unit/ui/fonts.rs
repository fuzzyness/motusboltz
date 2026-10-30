//! Font tests for `src/ui/fonts.rs`.

use super::*;

/// Character map of a TrueType font: every code point with a glyph. Reads
/// the format 4 subtable of the `cmap` table, which covers the Basic
/// Multilingual Plane.
fn code_points(ttf: &[u8]) -> Vec<u32> {
    let u16_at = |at: usize| u16::from_be_bytes([ttf[at], ttf[at + 1]]) as usize;
    let u32_at =
        |at: usize| u32::from_be_bytes([ttf[at], ttf[at + 1], ttf[at + 2], ttf[at + 3]]) as usize;
    let tables = u16_at(4);
    let cmap = (0..tables)
        .map(|index| 12 + 16 * index)
        .find(|&record| &ttf[record..record + 4] == b"cmap")
        .map(|record| u32_at(record + 8))
        .expect("cmap table");
    let subtable = (0..u16_at(cmap + 2))
        .map(|index| cmap + u32_at(cmap + 4 + 8 * index + 4))
        .find(|&at| u16_at(at) == 4)
        .expect("format 4 subtable");
    let segments = u16_at(subtable + 6) / 2;
    let ends = subtable + 14;
    let starts = ends + 2 * segments + 2;
    let mut points = Vec::new();
    for segment in 0..segments {
        let (start, end) = (u16_at(starts + 2 * segment), u16_at(ends + 2 * segment));
        if start != 0xFFFF {
            points.extend(start as u32..=end as u32);
        }
    }
    points
}

#[test]
fn both_fonts_have_every_symbol_and_all_printable_ascii() {
    for ttf in [REGULAR_TTF, SEMIBOLD_TTF] {
        let points = code_points(ttf);
        for c in (' '..='~').chain(SYMBOLS.chars()) {
            assert!(points.contains(&(c as u32)), "missing {c:?}");
        }
    }
}

#[test]
fn drawable_accepts_ascii_and_the_symbols_only() {
    assert!(drawable("ΔS universe = +43.5 J/K → spontaneous"));
    assert!(!drawable("✓ done"));
    assert!(!drawable("tab\there"));
}
