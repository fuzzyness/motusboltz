//! Drawing of the per-turn readouts: the first-law energy ledger, efficiency
//! against the Carnot limit, and the entropy change of the surroundings.

use macroquad::prelude::Color;

use super::readout::{
    TurnReadout,
    sum, //
};
use super::{
    BAR_GAP,
    BAR_HEIGHT,
    BODY_SIZE,
    COLD_COLOR,
    CONTENT_WIDTH,
    HOT_COLOR,
    LAW,
    LAW_SIZE,
    LINE,
    QUIET,
    SMALL_LINE,
    SMALL_SIZE,
    STORED_COLOR,
    TEXT,
    TRACK,
    WARNING,
    WORK_COLOR, //
};
use crate::ui::pen::Pen;

const BAR_LABEL_WIDTH: f32 = 34.0;
const LEGEND_SWATCH: f32 = 9.0;
const CARNOT_MARKER_OVERHANG: f32 = 3.0;
const CARNOT_MARKER_THICKNESS: f32 = 2.0;

pub const LAW_FIRST: &str = "1st law: energy is conserved";
pub const LAW_CARNOT: &str = "2nd law: no engine beats Carnot";
pub const LAW_ENTROPY: &str = "2nd law: entropy of the universe increases";
pub const THIRD_LAW_NOTE: &str = "3rd law: 100% would need T cold = 0 K";
const LEDGER_EQUATION: &str = "Q hot + Q cold = W + dU";
const ENTROPY_EQUATION: &str = "dS surr = -Q hot/T hot - Q cold/T cold";
const LEGEND: [(&str, Color); 4] = [
    ("Q hot", HOT_COLOR),
    ("Q cold", COLD_COLOR),
    ("W", WORK_COLOR),
    ("dU", STORED_COLOR),
];

/// Energy in and out of the gas per turn as two bars, a legend and the
/// ledger equation with its values. Returns the baseline of the last line.
pub fn draw_ledger(pen: &mut Pen, turn: &TurnReadout, x: f32, top: f32) -> f32 {
    let mut y = top + LINE;
    pen.fitted(LAW_FIRST, x, y, CONTENT_WIDTH, LAW_SIZE, LAW);
    let scale = sum(&turn.inflow).max(sum(&turn.outflow));
    let bar_width = CONTENT_WIDTH - BAR_LABEL_WIDTH;
    y += BAR_GAP;
    for (label, segments) in [("In", &turn.inflow), ("Out", &turn.outflow)] {
        pen.text(label, x, y + BAR_HEIGHT, SMALL_SIZE, QUIET);
        let mut left = x + BAR_LABEL_WIDTH;
        pen.rect(left, y, bar_width, BAR_HEIGHT, TRACK);
        for &(amount, color) in segments {
            let width = if scale > 0.0 {
                bar_width * (amount / scale) as f32
            } else {
                0.0
            };
            pen.rect(left, y, width, BAR_HEIGHT, color);
            left += width;
        }
        y += BAR_HEIGHT + BAR_GAP;
    }
    y += SMALL_LINE - BAR_GAP;
    let mut legend_x = x + BAR_LABEL_WIDTH;
    for (name, color) in LEGEND {
        pen.rect(
            legend_x,
            y - LEGEND_SWATCH,
            LEGEND_SWATCH,
            LEGEND_SWATCH,
            color,
        );
        legend_x += LEGEND_SWATCH + BAR_GAP;
        pen.text(name, legend_x, y, SMALL_SIZE, QUIET);
        legend_x += pen.measure(name, SMALL_SIZE) + 3.0 * BAR_GAP;
    }
    y += LINE;
    pen.fitted(LEDGER_EQUATION, x, y, CONTENT_WIDTH, BODY_SIZE, TEXT);
    y += LINE;
    pen.fitted(&turn.ledger_values, x, y, CONTENT_WIDTH, BODY_SIZE, TEXT);
    if let Some(warning) = &turn.ledger_warning {
        y += SMALL_LINE;
        pen.fitted(warning, x, y, CONTENT_WIDTH, SMALL_SIZE, WARNING);
    }
    y
}

/// Efficiency as a bar with the Carnot limit marked on it, and both values.
/// Returns the baseline of the last line.
pub fn draw_efficiency(pen: &mut Pen, turn: &TurnReadout, x: f32, top: f32) -> f32 {
    let mut y = top + LINE;
    pen.fitted(LAW_CARNOT, x, y, CONTENT_WIDTH, LAW_SIZE, LAW);
    y += BAR_GAP + CARNOT_MARKER_OVERHANG;
    pen.rect(x, y, CONTENT_WIDTH, BAR_HEIGHT, TRACK);
    pen.rect(
        x,
        y,
        CONTENT_WIDTH * turn.efficiency as f32,
        BAR_HEIGHT,
        WORK_COLOR,
    );
    let marker = x + CONTENT_WIDTH * turn.carnot as f32;
    pen.line(
        marker,
        y - CARNOT_MARKER_OVERHANG,
        marker,
        y + BAR_HEIGHT + CARNOT_MARKER_OVERHANG,
        CARNOT_MARKER_THICKNESS,
        TEXT,
    );
    y += BAR_HEIGHT + LINE;
    let (text, color) = &turn.efficiency_text;
    pen.fitted(text, x, y, CONTENT_WIDTH, BODY_SIZE, *color);
    y += LINE;
    pen.fitted(&turn.carnot_text, x, y, CONTENT_WIDTH, BODY_SIZE, TEXT);
    y += SMALL_LINE;
    pen.fitted(THIRD_LAW_NOTE, x, y, CONTENT_WIDTH, SMALL_SIZE, QUIET);
    y
}

/// The entropy change of the surroundings per turn and whether it is also
/// the change of the universe. Returns the baseline of the last line.
pub fn draw_entropy(pen: &mut Pen, turn: &TurnReadout, x: f32, top: f32) -> f32 {
    let mut y = top + LINE;
    pen.fitted(LAW_ENTROPY, x, y, CONTENT_WIDTH, LAW_SIZE, LAW);
    y += LINE;
    pen.fitted(ENTROPY_EQUATION, x, y, CONTENT_WIDTH, BODY_SIZE, TEXT);
    y += LINE;
    let (value, color) = &turn.entropy_value;
    pen.fitted(value, x, y, CONTENT_WIDTH, BODY_SIZE, *color);
    y += SMALL_LINE;
    let (note, color) = &turn.entropy_note;
    pen.fitted(note, x, y, CONTENT_WIDTH, SMALL_SIZE, *color);
    y
}
