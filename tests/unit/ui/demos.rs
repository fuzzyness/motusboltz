//! Tests for `src/ui/demos.rs`: the demos' settings, and the banner's fit.

use super::*;
use crate::sim::reservoir::{
    MAX_COLD_KELVIN,
    MAX_HOT_KELVIN,
    MIN_COLD_KELVIN,
    MIN_HOT_KELVIN, //
};
use crate::ui::controls::{
    SceneArea,
    corner_button,
    row_top, //
};
use crate::ui::fonts::tests::drawable;
use crate::ui::layout::scene_span;

#[test]
fn every_demo_is_in_the_sliders_range_and_says_its_temperatures() {
    for scenario in SCENARIOS {
        let (hot, cold) = scenario.kelvin();
        assert!((MIN_HOT_KELVIN..=MAX_HOT_KELVIN).contains(&hot));
        assert!((MIN_COLD_KELVIN..=MAX_COLD_KELVIN).contains(&cold));
        assert_eq!(scenario.temperatures(), format!("{hot} / {cold} K"));
        let watch = scenario.watch().replace(NO_BREAK, " ");
        let texts = [scenario.title(), scenario.temperatures(), &watch];
        assert!(texts.into_iter().all(drawable));
    }
}

#[test]
fn wrapped_lines_keep_every_word_and_fit() {
    let pen = Pen::offline();
    let text = Scenario::HeatEngine.watch();
    let width = pen.measure("What comes in but does not go out", TEXT_SIZE);
    let lines = wrap(&pen, text, width, TEXT_SIZE);
    assert!(lines.len() > 1);
    assert!(
        lines
            .iter()
            .all(|line| pen.measure(line, TEXT_SIZE) <= width)
    );
    let words: Vec<&str> = lines.iter().flat_map(|line| line.split(' ')).collect();
    assert_eq!(words, text.split_whitespace().collect::<Vec<_>>());
}

#[test]
fn a_number_keeps_its_unit_on_its_line() {
    let pen = Pen::offline();
    let text = Scenario::ColdSink.watch();
    for width in [120.0, 160.0, 200.0, 240.0, 280.0] {
        let lines = wrap(&pen, text, width, TEXT_SIZE);
        assert!(lines.iter().all(|line| !line.starts_with("K")));
        assert!(lines.iter().all(|line| !line.contains(NO_BREAK)));
    }
}

/// The banner, with its prompt, clears the scene's span, Hide all and the
/// bottom row in the smallest window.
#[test]
fn the_banner_fits_beside_the_scene_above_the_bottom_row() {
    for (width, height) in [(1440.0, 900.0), (1280.0, 580.0)] {
        let (left, span) = scene_span(width, true, true);
        let area = SceneArea {
            edge: left,
            left,
            width: span,
            window_width: width,
            height,
        };
        let corner = corner_button(&Pen::offline(), area);
        for scenario in SCENARIOS {
            let mut pen = Pen::offline();
            draw_banner(&mut pen, width, scenario, true);
            let lines = lines(&pen, scenario, text_width(width), true);
            let card = card(width, lines.len());
            assert!(card.x >= area.right() && card.right() <= width);
            assert!(card.bottom() < row_top(height).min(corner.y));
            assert!(pen.texts.iter().any(|text| text == PROMPT));
            assert!(pen.texts.iter().any(|text| text == scenario.temperatures()));
            assert!(pen.texts.iter().all(|text| drawable(text)));
        }
    }
}
