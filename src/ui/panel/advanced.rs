//! The Advanced view: every reading, in sections. The gas in each cylinder
//! as a table, the flywheel and its load, the exact per-turn equations
//! behind each law, the pressure-volume loops, and running totals with the
//! energy checks. Labels sit at the left and numbers line up at the right.

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

/// Right edges of the hot and cold columns of the gas table, from the
/// panel's text margin.
const HOT_RIGHT: f32 = 218.0;
const COLD_RIGHT: f32 = CONTENT_WIDTH;
const SECTION_GAP: f32 = 16.0;
const ROW: f32 = 20.0;

const GAS_HEADING: &str = "GAS NOW";
const FLYWHEEL_HEADING: &str = "FLYWHEEL";
const TOTALS_HEADING: &str = "SINCE THE GAS WAS FILLED";
pub const LEDGER_EQUATION: &str = "Q hot + Q cold = W + ΔU";
pub const ENTROPY_EQUATION: &str = "ΔS surr = −Q hot / T hot − Q cold / T cold";

/// A section heading below `top`; returns its baseline.
pub fn section(pen: &mut Pen, text: &str, x: f32, top: f32) -> f32 {
    let y = top + LINE;
    pen.fitted_heading(text, x, y, CONTENT_WIDTH, SECTION_SIZE, SECTION);
    y
}

/// A label at the left and a value right-aligned to the content's edge.
fn row(pen: &mut Pen, label: &str, value: &str, x: f32, y: f32, color: macroquad::prelude::Color) {
    let value_width = pen.measure(value, BODY_SIZE);
    pen.fitted(
        label,
        x,
        y,
        CONTENT_WIDTH - value_width - 12.0,
        BODY_SIZE,
        QUIET,
    );
    pen.text(value, x + CONTENT_WIDTH - value_width, y, BODY_SIZE, color);
}

/// Draws the sections above the loops below `top`; returns the last
/// baseline. The caller draws the loops and then `draw_totals`.
pub fn draw(pen: &mut Pen, readout: &Readout, x: f32, top: f32) -> f32 {
    let y = draw_gas(pen, readout, x, top);
    let y = draw_flywheel(pen, readout, x, y + SECTION_GAP);
    draw_turn(pen, readout, x, y + SECTION_GAP)
}

fn draw_gas(pen: &mut Pen, readout: &Readout, x: f32, top: f32) -> f32 {
    let now = &readout.now;
    let mut y = section(pen, GAS_HEADING, x, top);
    let header = |pen: &mut Pen, text: &str, right: f32, color| {
        let width = pen.measure_weight(text, SECTION_SIZE, Weight::Semibold);
        pen.heading(text, x + right - width, y, SECTION_SIZE, color);
    };
    header(pen, "Hot", HOT_RIGHT, HOT_COLOR);
    header(pen, "Cold", COLD_RIGHT, COLD_COLOR);
    let bar = |bar: Option<f64>| bar.map_or_else(|| "—".to_owned(), |bar| format!("{bar:.1} bar"));
    let rows = [
        (
            "Temperature",
            kelvin_text(now.hot.kelvin),
            kelvin_text(now.cold.kelvin),
        ),
        ("Pressure (estimate)", bar(now.hot.bar), bar(now.cold.bar)),
        (
            "Particles",
            now.hot.particles.to_string(),
            now.cold.particles.to_string(),
        ),
        (
            "Concentration",
            format!("{:.2} mol/L", now.hot.mol_per_l),
            format!("{:.2} mol/L", now.cold.mol_per_l),
        ),
    ];
    for (label, hot, cold) in rows {
        y += ROW;
        pen.fitted(label, x, y, HOT_RIGHT - 80.0, BODY_SIZE, QUIET);
        pen.text_right(&hot, x + HOT_RIGHT, y, BODY_SIZE, TEXT);
        pen.text_right(&cold, x + COLD_RIGHT, y, BODY_SIZE, TEXT);
    }
    for note in [
        format!(
            "{} particles = {:.3} mol ({} in the opening)",
            now.particles, now.moles, now.passage_particles
        ),
        "Pressure here is an ideal-gas estimate, P = nRT / V.".to_owned(),
    ] {
        y += SMALL_LINE;
        pen.fitted(&note, x, y, CONTENT_WIDTH, SMALL_SIZE, QUIET);
    }
    y
}

fn draw_flywheel(pen: &mut Pen, readout: &Readout, x: f32, top: f32) -> f32 {
    let now = &readout.now;
    let mut y = section(pen, FLYWHEEL_HEADING, x, top);
    let speed = format!("{} rpm", signed_number(now.turns_per_minute, 2));
    let rows = [
        ("Speed (model time)", speed),
        ("Turns so far", format!("{:.1}", now.turns)),
        ("Energy now", format!("{} J", amount(now.flywheel))),
        ("Given by Start", format!("{} J", amount(now.starter))),
        (
            "Delivered to the load",
            format!("{} J", amount(now.to_load)),
        ),
        ("Lost to friction", format!("{} J", amount(now.friction))),
    ];
    for (label, value) in rows {
        y += ROW;
        row(pen, label, &value, x, y, TEXT);
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

fn draw_turn(pen: &mut Pen, readout: &Readout, x: f32, top: f32) -> f32 {
    let Some(turn) = &readout.turn else {
        let y = section(pen, "EACH TURN", x, top) + LINE;
        pen.fitted(readout.waiting, x, y, CONTENT_WIDTH, BODY_SIZE, QUIET);
        return y;
    };
    let mut y = section(pen, &turn_heading(turn), x, top);
    y += LINE;
    pen.fitted(LEDGER_EQUATION, x, y, CONTENT_WIDTH, BODY_SIZE, TEXT);
    y += LINE;
    let ledger = format!(
        "{} = {} J",
        signed_sum(&[turn.hot_heat, turn.cold_heat]),
        signed_sum(&[turn.work, turn.stored])
    );
    pen.fitted(&ledger, x, y, CONTENT_WIDTH, BODY_SIZE, TEXT);
    if turn.residual.abs() > LEDGER_TOLERANCE_JOULES {
        y += SMALL_LINE;
        let text = format!("Unexpected: off by {} J", signed_number(turn.residual, 2));
        pen.fitted(&text, x, y, CONTENT_WIDTH, SMALL_SIZE, WARNING);
    }
    y += LINE + 6.0;
    let efficiency = match turn.efficiency {
        _ if turn.carnot == 0.0 => "Efficiency: none without a temperature difference".to_owned(),
        Some(value) => format!("Efficiency W / Q hot = {:.1}%", 100.0 * value),
        None => "Efficiency 0%: no net work out per turn".to_owned(),
    };
    pen.fitted(&efficiency, x, y, CONTENT_WIDTH, BODY_SIZE, TEXT);
    y += LINE;
    let carnot = format!(
        "Carnot limit 1 − {:.0} / {:.0} = {:.1}%",
        turn.cold_kelvin,
        turn.hot_kelvin,
        100.0 * turn.carnot
    );
    pen.fitted(&carnot, x, y, CONTENT_WIDTH, BODY_SIZE, TEXT);
    y += LINE + 6.0;
    pen.fitted(ENTROPY_EQUATION, x, y, CONTENT_WIDTH, BODY_SIZE, TEXT);
    y += LINE;
    let entropy = format!("= {} J/K per turn", signed_number(turn.entropy, 3));
    let color = if turn.entropy > 0.0 { GOOD } else { WARNING };
    pen.fitted(&entropy, x, y, CONTENT_WIDTH, BODY_SIZE, color);
    y += SMALL_LINE;
    let stored = signed_number(turn.stored, 0);
    let note = if turn.settled {
        format!("ΔU {stored} J is small, so ΔS universe ≈ ΔS surr")
    } else {
        format!("Gas still settling (ΔU {stored} J): ΔS gas not counted")
    };
    pen.fitted(&note, x, y, CONTENT_WIDTH, SMALL_SIZE, QUIET);
    y
}

/// Running totals since the fill and the energy checks; returns the last
/// baseline.
pub fn draw_totals(pen: &mut Pen, readout: &Readout, x: f32, top: f32) -> f32 {
    let now = &readout.now;
    let mut y = section(pen, TOTALS_HEADING, x, top);
    let rows = [
        (
            "Heat in at the hot side",
            format!("{} J", amount(now.heat_from_hot)),
        ),
        (
            "Heat in at the cold side",
            format!("{} J", amount(now.heat_from_cold)),
        ),
        (
            "Work done by the gas",
            format!("{} J", amount(now.gas_work)),
        ),
    ];
    for (label, value) in rows {
        y += ROW;
        row(pen, label, &value, x, y, TEXT);
    }
    y += SMALL_LINE + 2.0;
    let check = format!(
        "Energy checks (≈ 0 when nothing is lost): gas {} J, shaft {} J",
        signed_number(now.gas_error, 4),
        signed_number(now.shaft_error, 4)
    );
    pen.fitted(&check, x, y, CONTENT_WIDTH, SMALL_SIZE, QUIET);
    y
}
