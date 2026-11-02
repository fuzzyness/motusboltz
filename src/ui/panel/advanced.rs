//! The Advanced view: every reading, in sections.
//!
//! The sections are the gas in each cylinder as a table, the flywheel and
//! its load, the exact per-turn equations behind each law, the
//! pressure-volume loops, and running totals with the energy checks.
//! Labels sit at the left and numbers line up at the right.

use macroquad::prelude::Color;

use super::readout::{
    LEDGER_TOLERANCE_JOULES,
    Readout,
    Turn,
    amount,
    kelvin_text,
    signed_number,
    signed_sum, //
};
use super::{
    BODY_SIZE,
    COLD_COLOR,
    CONTENT_WIDTH,
    GOOD,
    HOT_COLOR,
    LEFT,
    LINE,
    QUIET,
    SECTION,
    SECTION_SIZE,
    SMALL_LINE,
    SMALL_SIZE,
    TEXT,
    WARNING, //
};
use crate::ui::pen::{
    Pen,
    Weight, //
};

/// Right edge of the gas table's hot column, from the content's left edge.
const HOT_RIGHT: f32 = 218.0;
/// Right edge of the gas table's cold column.
const COLD_RIGHT: f32 = CONTENT_WIDTH;
/// Room for a gas-table label, clear of the hot column's numbers.
const TABLE_LABEL_WIDTH: f32 = HOT_RIGHT - 80.0;
/// Least gap between a row's label and its value.
const LABEL_VALUE_GAP: f32 = 12.0;
/// Space between sections.
const SECTION_GAP: f32 = 16.0;
/// Distance between the baselines of table rows.
const ROW: f32 = 20.0;
/// Extra space above each law's equations.
const EQUATION_GAP: f32 = 6.0;
/// Extra space above the energy checks.
const CHECK_GAP: f32 = 2.0;

/// Heading of the gas table.
const GAS_HEADING: &str = "GAS NOW";
/// Heading of the flywheel section.
const FLYWHEEL_HEADING: &str = "FLYWHEEL";
/// Heading of the running totals.
const TOTALS_HEADING: &str = "SINCE THE GAS WAS FILLED";
/// The first law, per turn.
const LEDGER_EQUATION: &str = "Q hot + Q cold = W + ΔU";
/// The entropy change of the surroundings, per turn.
const ENTROPY_EQUATION: &str = "ΔS surr = −Q hot / T hot − Q cold / T cold";

/// Draws a section heading below `top` and returns its baseline.
pub fn section(pen: &mut Pen, text: &str, top: f32) -> f32 {
    let y = top + LINE;
    pen.fitted_heading(text, LEFT, y, CONTENT_WIDTH, SECTION_SIZE, SECTION);
    y
}

/// Draws a label at the left and a value right-aligned to the content's
/// edge, on baseline `y`.
fn row(pen: &mut Pen, label: &str, value: &str, y: f32, color: Color) {
    let value_width = pen.measure(value, BODY_SIZE);
    let room = CONTENT_WIDTH - value_width - LABEL_VALUE_GAP;
    pen.fitted(label, LEFT, y, room, BODY_SIZE, QUIET);
    pen.text(
        value,
        LEFT + CONTENT_WIDTH - value_width,
        y,
        BODY_SIZE,
        color,
    );
}

/// Draws the sections above the loops below `top` and returns the last
/// baseline.
///
/// The caller draws the loops and then `draw_totals`.
pub fn draw(pen: &mut Pen, readout: &Readout, top: f32) -> f32 {
    let y = draw_gas(pen, readout, top);
    let y = draw_flywheel(pen, readout, y + SECTION_GAP);
    draw_turn(pen, readout, y + SECTION_GAP)
}

/// Draws the gas table below `top` and returns the last baseline.
fn draw_gas(pen: &mut Pen, readout: &Readout, top: f32) -> f32 {
    let now = &readout.now;
    let mut y = section(pen, GAS_HEADING, top);
    let header = |pen: &mut Pen, text: &str, right: f32, color| {
        let width = pen.measure_weight(text, SECTION_SIZE, Weight::Semibold);
        pen.heading(text, LEFT + right - width, y, SECTION_SIZE, color);
    };
    header(pen, "Hot", HOT_RIGHT, HOT_COLOR);
    header(pen, "Cold", COLD_RIGHT, COLD_COLOR);
    let bar = |bar: Option<f64>| bar.map_or_else(|| "—".to_owned(), |bar| format!("{bar:.1} bar"));
    let concentration = |mol_per_l: f64| format!("{mol_per_l:.2} mol/L");
    let (hot, cold) = (&now.hot, &now.cold);
    let rows = [
        (
            "Temperature",
            kelvin_text(hot.kelvin),
            kelvin_text(cold.kelvin),
        ),
        ("Pressure", bar(hot.bar), bar(cold.bar)),
        (
            "Particles",
            hot.particles.to_string(),
            cold.particles.to_string(),
        ),
        (
            "Concentration",
            concentration(hot.mol_per_l),
            concentration(cold.mol_per_l),
        ),
    ];
    for (label, hot, cold) in rows {
        y += ROW;
        pen.fitted(label, LEFT, y, TABLE_LABEL_WIDTH, BODY_SIZE, QUIET);
        pen.text_right(&hot, LEFT + HOT_RIGHT, y, BODY_SIZE, TEXT);
        pen.text_right(&cold, LEFT + COLD_RIGHT, y, BODY_SIZE, TEXT);
    }
    let amount_note = format!(
        "{} particles = {:.3} mol ({} in the opening)",
        now.particles, now.moles, now.passage_particles
    );
    let pressure_note = "Pressure: the gas's hits on each piston, over the last 2 s.";
    for note in [amount_note.as_str(), pressure_note] {
        y += SMALL_LINE;
        pen.fitted(note, LEFT, y, CONTENT_WIDTH, SMALL_SIZE, QUIET);
    }
    y
}

/// Draws the flywheel section below `top` and returns the last baseline.
fn draw_flywheel(pen: &mut Pen, readout: &Readout, top: f32) -> f32 {
    let now = &readout.now;
    let mut y = section(pen, FLYWHEEL_HEADING, top);
    let joules = |energy: f64| format!("{} J", amount(energy));
    let rows = [
        (
            "Speed (model time)",
            format!("{} rpm", signed_number(now.turns_per_minute, 2)),
        ),
        ("Turns so far", format!("{:.1}", now.turns)),
        ("Energy now", joules(now.flywheel)),
        ("Given by Start", joules(now.starter)),
        ("Delivered to the load", joules(now.to_load)),
        ("Lost to friction", joules(now.friction)),
    ];
    for (label, value) in rows {
        y += ROW;
        row(pen, label, &value, y, TEXT);
    }
    y
}

/// The heading for the per-turn section.
fn turn_heading(turn: &Turn) -> String {
    if turn.turns == 1 {
        format!("LAST FULL TURN ({:.0} S)", turn.seconds)
    } else {
        format!(
            "AVERAGE OF LAST {} TURNS ({:.0} S EACH)",
            turn.turns, turn.seconds
        )
    }
}

/// Draws the per-turn equations below `top` and returns the last baseline.
fn draw_turn(pen: &mut Pen, readout: &Readout, top: f32) -> f32 {
    let Some(turn) = &readout.turn else {
        let y = section(pen, "EACH TURN", top) + LINE;
        pen.fitted(readout.waiting, LEFT, y, CONTENT_WIDTH, BODY_SIZE, QUIET);
        return y;
    };
    let line = |pen: &mut Pen, text: &str, y: f32, size: f32, color: Color| {
        pen.fitted(text, LEFT, y, CONTENT_WIDTH, size, color);
    };
    let mut y = section(pen, &turn_heading(turn), top) + LINE;
    line(pen, LEDGER_EQUATION, y, BODY_SIZE, TEXT);
    y += LINE;
    let heat = signed_sum(&[turn.hot_heat, turn.cold_heat]);
    let ledger = format!("{heat} = {} J", signed_sum(&[turn.work, turn.stored]));
    line(pen, &ledger, y, BODY_SIZE, TEXT);
    if turn.residual.abs() > LEDGER_TOLERANCE_JOULES {
        y += SMALL_LINE;
        let text = format!("Unexpected: off by {} J", signed_number(turn.residual, 2));
        line(pen, &text, y, SMALL_SIZE, WARNING);
    }
    y += LINE + EQUATION_GAP;
    let efficiency = match turn.efficiency {
        _ if turn.carnot == 0.0 => "Efficiency: none without a temperature difference".to_owned(),
        Some(value) => format!("Efficiency W / Q hot = {:.1}%", 100.0 * value),
        None => "Efficiency 0%: no net work out per turn".to_owned(),
    };
    line(pen, &efficiency, y, BODY_SIZE, TEXT);
    y += LINE;
    let carnot = format!(
        "Carnot limit 1 − {:.0} / {:.0} = {:.1}%",
        turn.cold_kelvin,
        turn.hot_kelvin,
        100.0 * turn.carnot
    );
    line(pen, &carnot, y, BODY_SIZE, TEXT);
    y += LINE + EQUATION_GAP;
    line(pen, ENTROPY_EQUATION, y, BODY_SIZE, TEXT);
    y += LINE;
    let entropy = format!("= {} J/K per turn", signed_number(turn.entropy, 3));
    let color = if turn.entropy > 0.0 { GOOD } else { WARNING };
    line(pen, &entropy, y, BODY_SIZE, color);
    y += SMALL_LINE;
    let stored = signed_number(turn.stored, 0);
    let note = if turn.settled {
        format!("ΔU {stored} J is small, so ΔS universe ≈ ΔS surr")
    } else {
        format!("Gas still settling (ΔU {stored} J): ΔS gas not counted")
    };
    line(pen, &note, y, SMALL_SIZE, QUIET);
    y
}

/// Draws the running totals since the fill and the energy checks below
/// `top`, and returns the last baseline.
pub fn draw_totals(pen: &mut Pen, readout: &Readout, top: f32) -> f32 {
    let now = &readout.now;
    let mut y = section(pen, TOTALS_HEADING, top);
    let joules = |energy: f64| format!("{} J", amount(energy));
    let rows = [
        ("Heat in at the hot side", joules(now.heat_from_hot)),
        ("Heat in at the cold side", joules(now.heat_from_cold)),
        ("Work done by the gas", joules(now.gas_work)),
    ];
    for (label, value) in rows {
        y += ROW;
        row(pen, label, &value, y, TEXT);
    }
    y += SMALL_LINE + CHECK_GAP;
    let check = format!(
        "Energy checks (≈ 0 when nothing is lost): gas {} J, shaft {} J",
        signed_number(now.gas_error, 4),
        signed_number(now.shaft_error, 4)
    );
    pen.fitted(&check, LEFT, y, CONTENT_WIDTH, SMALL_SIZE, QUIET);
    y
}
