//! The energy strip: the first law as a picture, beside the scene.
//!
//! Two bars of equal length show where the gas's energy came from and went
//! over the recent turns. The top bar is what came in: heat from a wall,
//! work done on the gas, or energy the gas gave up. The bottom bar is what
//! went out: work on the pistons, heat to a wall, or energy the gas kept.
//! Each part has its own color, as in the panel: orange for heat at the hot
//! side, yellow for work, blue for heat at the cold side and gray for the
//! gas's own energy. The strip shows once a turn is done, in the column
//! under the sliders' card, out of the engine's way. In a window too short
//! for it there, it goes to the scene's bottom-left corner while the panel
//! is closed, and is left out while the panel's own first law is open.

use macroquad::prelude::{
    Color,
    Rect, //
};

use crate::sim::cycle::TurnSummary;
use crate::sim::engine::ConnectedChambers;
use crate::sim::units::HeliumCalibration;
use crate::ui::controls::{
    BACKING,
    BACKING_RADIUS,
    MARGIN,
    SceneArea,
    corner_button,
    row_top,
    slider_column, //
};
use crate::ui::panel::readout::amount;
use crate::ui::panel::{
    COLD_COLOR,
    HOT_COLOR,
    QUIET,
    WORK_COLOR, //
};
use crate::ui::pen::{
    Pen,
    Weight, //
};

/// Space between the strip in the scene and the bottom row, which leaves
/// room for the tip over Start.
const ABOVE_ROW: f32 = 44.0;
/// Space between the card's edge and its contents.
const PADDING: f32 = 10.0;
/// From the card's top to the heading's baseline.
const HEADING_BASELINE: f32 = 16.0;
/// From the card's top to the first bar.
const FIRST_BAR: f32 = 23.0;
/// Thickness of a bar.
const BAR_HEIGHT: f32 = 7.0;
/// Space between the bars.
const BAR_GAP: f32 = 5.0;
/// Width of the "In" and "Out" column.
const ROW_LABEL_WIDTH: f32 = 28.0;
/// From the last bar's bottom to the key's first baseline.
const KEY_DROP: f32 = 16.0;
/// Distance between the key's two baselines.
const KEY_LINE: f32 = 16.0;
/// Space under the key.
const KEY_ROOM: f32 = 7.0;
/// Height of the card.
const HEIGHT: f32 = FIRST_BAR + 2.0 * BAR_HEIGHT + BAR_GAP + KEY_DROP + KEY_LINE + KEY_ROOM;
/// Text size of the heading and the row labels.
const SMALL_SIZE: f32 = 11.0;
/// Text size of the key.
const KEY_SIZE: f32 = 13.0;
/// Least space between the key's two columns.
const KEY_COLUMN_GAP: f32 = 8.0;
/// Color of the gas's own energy.
const STORED_COLOR: Color = Color::from_rgba(150, 160, 178, 255);
/// The heading.
const HEADING: &str = "ENERGY EACH TURN";
/// What the bars show.
const LAW: &str = "in = out";

/// One term of the first law for one turn.
struct Term {
    /// Its name in the key.
    name: &'static str,
    /// Energy into the gas, in joules; negative when it leaves.
    joules: f64,
    /// Its color.
    color: Color,
}

/// The terms of `recent` turns, averaged per turn, in joules by `units`.
fn terms(recent: TurnSummary, units: HeliumCalibration) -> [Term; 4] {
    let joules = |reduced: f64| units.energy_joules(reduced);
    let change = recent.per_turn().change;
    [
        Term {
            name: "Q hot",
            joules: joules(change.hot_heat),
            color: HOT_COLOR,
        },
        Term {
            name: "W",
            joules: -joules(change.gas_work),
            color: WORK_COLOR,
        },
        Term {
            name: "Q cold",
            joules: joules(change.cold_heat),
            color: COLD_COLOR,
        },
        Term {
            name: "ΔU",
            joules: -joules(change.gas_energy),
            color: STORED_COLOR,
        },
    ]
}

/// Energy that came in and that went out, over `terms`.
fn totals(terms: &[Term; 4]) -> (f64, f64) {
    let incoming = terms.iter().map(|term| term.joules.max(0.0)).sum();
    let outgoing = terms.iter().map(|term| (-term.joules).max(0.0)).sum();
    (incoming, outgoing)
}

/// The strip's card in `area`: in the column under the sliders' card from
/// `column_top` down, if it ends above Hide all at `corner_top`; or else in
/// the scene's bottom-left corner, unless the `panel` is open.
fn card(area: SceneArea, column_top: f32, corner_top: f32, panel: bool) -> Option<Rect> {
    let column = slider_column(area.window_width);
    if column_top + HEIGHT <= corner_top - MARGIN {
        Some(Rect::new(column.x, column_top, column.w, HEIGHT))
    } else if panel {
        None
    } else {
        let top = row_top(area.height) - ABOVE_ROW - HEIGHT;
        Some(Rect::new(area.edge + MARGIN, top, column.w, HEIGHT))
    }
}

/// Draws one bar filling `rect` for `total` joules, from the parts of
/// `terms` coming in, or with `incoming` false, going out.
fn bar(pen: &Pen, terms: &[Term; 4], incoming: bool, rect: Rect, total: f64) {
    let radius = BAR_HEIGHT / 2.0;
    let mut x = rect.x;
    for term in terms {
        let part = if incoming { term.joules } else { -term.joules };
        if part <= 0.0 {
            continue;
        }
        let width = (part / total) as f32 * rect.w;
        pen.round_rect(
            Rect {
                x,
                w: width,
                ..rect
            },
            radius.min(width / 2.0),
            term.color,
        );
        x += width;
    }
}

/// Where the strip goes in `area` once `engine` has done a turn, with the
/// column under the sliders' card free from `column_top` down and the
/// `panel` open or closed, if anywhere.
pub fn place(
    pen: &Pen,
    area: SceneArea,
    engine: &ConnectedChambers,
    column_top: f32,
    panel: bool,
) -> Option<Rect> {
    let corner_top = corner_button(pen, area).y;
    engine.cycle().recent()?;
    card(area, column_top, corner_top, panel)
}

/// Draws the strip for `engine` in `card` (see `place`).
pub fn draw(pen: &mut Pen, card: Rect, engine: &ConnectedChambers) {
    if let Some(recent) = engine.cycle().recent() {
        draw_terms(pen, card, &terms(recent, engine.units()));
    }
}

/// Draws the strip for `terms` in `card`.
fn draw_terms(pen: &mut Pen, card: Rect, terms: &[Term; 4]) {
    let (incoming, outgoing) = totals(terms);
    let total = incoming.max(outgoing);
    if total <= 0.0 {
        return;
    }
    let top = card.y;
    pen.round_rect(card, BACKING_RADIUS, BACKING);

    let left = card.x + PADDING;
    let inner = card.w - 2.0 * PADDING;
    let heading_y = top + HEADING_BASELINE;
    pen.text(HEADING, left, heading_y, SMALL_SIZE, QUIET);
    pen.text_right(LAW, left + inner, heading_y, SMALL_SIZE, QUIET);
    let bars_left = left + ROW_LABEL_WIDTH;
    let bars_width = inner - ROW_LABEL_WIDTH;
    for (row, (label, incoming)) in [("In", true), ("Out", false)].into_iter().enumerate() {
        let bar_top = top + FIRST_BAR + row as f32 * (BAR_HEIGHT + BAR_GAP);
        let label_y = bar_top + (BAR_HEIGHT + pen.text_height(label, SMALL_SIZE)) / 2.0;
        pen.text(label, left, label_y, SMALL_SIZE, QUIET);
        let rect = Rect::new(bars_left, bar_top, bars_width, BAR_HEIGHT);
        bar(pen, terms, incoming, rect, total);
    }

    // The key in two columns under the bars, each term's name and size:
    // the hot side and the work, then the cold side and the gas.
    let values: Vec<String> = terms
        .iter()
        .map(|term| format!(" {} J", amount(term.joules.abs())))
        .collect();
    let pieces: Vec<[(&str, Color, Weight); 2]> = terms
        .iter()
        .zip(&values)
        .map(|(term, value)| {
            [
                (term.name, term.color, Weight::Regular),
                (value.as_str(), term.color, Weight::Semibold),
            ]
        })
        .collect();
    let half = inner / 2.0;
    let widest = pieces
        .iter()
        .map(|piece| pen.runs_width(piece, KEY_SIZE))
        .fold(0.0, f32::max);
    let size = if widest > half - KEY_COLUMN_GAP {
        (KEY_SIZE * (half - KEY_COLUMN_GAP) / widest).floor()
    } else {
        KEY_SIZE
    };
    let first_y = top + FIRST_BAR + 2.0 * BAR_HEIGHT + BAR_GAP + KEY_DROP;
    for (index, piece) in pieces.iter().enumerate() {
        let (row, column) = (index / 2, index % 2);
        let x = left + column as f32 * half;
        pen.runs(piece, x, first_y + row as f32 * KEY_LINE, size);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/energy_strip.rs"]
mod tests;
