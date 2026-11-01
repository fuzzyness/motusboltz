//! Tests for `src/ui/energy_strip.rs`: the bars balance, and the strip fits.

use super::*;
use crate::sim::geometry::DEFAULT_GEOMETRY;
use crate::sim::units::DEFAULT_PARTICLES;
use crate::ui::controls::TOGGLES_BOTTOM;
use crate::ui::fonts::tests::drawable;
use crate::ui::layout::{
    scene_left,
    scene_span, //
};

/// The calibration of the default fill.
fn units() -> HeliumCalibration {
    const SEED: u64 = 42;
    ConnectedChambers::with_geometry_seed(DEFAULT_PARTICLES, DEFAULT_GEOMETRY, SEED).units()
}

#[test]
fn what_comes_in_is_what_goes_out() {
    let terms = terms(TurnSummary::heated(), units());
    let (incoming, outgoing) = totals(&terms);
    assert!(incoming > 0.0);
    assert!((incoming - outgoing).abs() < 1e-9 * incoming);
    // The heated turn takes heat in at the hot side and does work.
    assert!(terms[0].joules > 0.0 && terms[1].joules < 0.0 && terms[2].joules < 0.0);
}

/// The scene of a window `width` by `height`, with the panel open or not.
fn area(width: f32, height: f32, panel: bool) -> SceneArea {
    let (left, span) = scene_span(width, panel, true);
    SceneArea {
        edge: scene_left(panel),
        left,
        width: span,
        window_width: width,
        height,
    }
}

/// The strip goes in the column under the sliders' card when it ends
/// above Hide all there, as in a tall window. Otherwise it goes in the
/// scene's bottom-left corner, above the bottom row and the tip, while the
/// panel is closed, and nowhere while it is open.
#[test]
fn the_strip_stays_out_of_the_engines_way() {
    for (width, height) in [(1440.0, 900.0), (1366.0, 620.0), (1280.0, 580.0)] {
        for panel in [true, false] {
            let area = area(width, height, panel);
            let corner = corner_button(&Pen::offline(), area).y;
            let column = slider_column(width);
            // Under the sliders' card alone, and under a long banner.
            for column_top in [column.bottom() + MARGIN, corner - MARGIN] {
                let Some(card) = card(area, column_top, corner, panel) else {
                    assert!(panel && column_top > column.bottom() + MARGIN);
                    continue;
                };
                if card.x == column.x {
                    assert!(card.y >= column_top && card.bottom() < corner);
                } else {
                    assert!(!panel && card.x >= area.edge && card.y > TOGGLES_BOTTOM);
                    assert!(card.bottom() < row_top(height));
                }
                assert!(card.right() <= width);
                let mut pen = Pen::offline();
                draw_terms(&mut pen, card, &terms(TurnSummary::heated(), units()));
                assert!(pen.texts.iter().all(|text| drawable(text)));
                assert!(pen.texts.iter().any(|text| text == "Q hot"));
            }
        }
    }
}
