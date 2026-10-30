//! The Simple view: what the project asks about, in the plainest form. The
//! gas temperatures and the flywheel's speed now, a diagram of where the
//! energy goes each turn (the helium is the system; the hot side, the cold
//! side and the flywheel are its surroundings), and one card per law with
//! the engine's own numbers as evidence. Colors match across the view:
//! orange for heat in, yellow for work, blue for heat out.

use macroquad::prelude::{
    Color,
    vec2, //
};

use super::readout::{
    Now,
    Readout,
    Turn,
    amount,
    kelvin_text,
    signed_number,
    speed_text, //
};
use super::{
    BODY_SIZE,
    COLD_COLOR,
    CONTENT_WIDTH,
    GOOD,
    HOT_COLOR,
    LAW,
    QUIET,
    SECTION,
    SECTION_SIZE,
    SMALL_SIZE,
    TEXT,
    TRACK,
    WARNING,
    WORK_COLOR, //
};
use crate::ui::pen::{
    Pen,
    Weight, //
};

const LINE: f32 = 20.0;
const SMALL_LINE: f32 = 17.0;
const SECTION_GAP: f32 = 13.0;

// "Right now": three cards side by side.
const CARD_GAP: f32 = 8.0;
const CARD_HEIGHT: f32 = 52.0;
const CARD_RADIUS: f32 = 8.0;
const CARD_PADDING: f32 = 10.0;
const CARD_VALUE_SIZE: f32 = 20.0;
const CARD: Color = Color::from_rgba(28, 41, 58, 255);

// The diagram: hot side, helium and cold side in a column joined by heat
// arrows, and a work arrow from the helium to the flywheel at the right.
// Arrow widths follow the energy each carries.
const BOX_WIDTH: f32 = 150.0;
const BOX_HEIGHT: f32 = 40.0;
const BOX_RADIUS: f32 = 7.0;
const BOX_NAME_SIZE: f32 = 15.0;
const BOX_TAG_SIZE: f32 = 12.0;
const FLYWHEEL_WIDTH: f32 = 94.0;
const ARROW_LENGTH: f32 = 40.0;
const MAX_ARROW_WIDTH: f32 = 22.0;
const MIN_ARROW_WIDTH: f32 = 3.0;
const HEAD_LENGTH: f32 = 9.0;
const HEAD_OVERHANG: f32 = 5.0;
const ARROW_LABEL_GAP: f32 = 14.0;
const HOT_BOX: Color = Color::from_rgba(92, 50, 30, 255);
const GAS_BOX: Color = Color::from_rgba(48, 64, 84, 255);
const COLD_BOX: Color = Color::from_rgba(24, 52, 84, 255);
const FLYWHEEL_BOX: Color = Color::from_rgba(78, 66, 28, 255);

// Law cards: a numbered badge, a heading, then the evidence.
const BADGE_RADIUS: f32 = 11.0;
const BADGE_SIZE: f32 = 14.0;
const INDENT: f32 = 30.0;
const HEADING_SIZE: f32 = 16.0;
const BADGE_TEXT: Color = Color::from_rgba(16, 26, 38, 255);
const BAR_HEIGHT: f32 = 10.0;
const MARKER_OVERHANG: f32 = 4.0;

const HEADING_NOW: &str = "RIGHT NOW";
const HEADING_ENERGY: &str = "WHERE THE ENERGY GOES EACH TURN";
pub const LAW_FIRST: &str = "Energy is conserved";
pub const LAW_SECOND: &str = "Heat flows from hot to cold on its own";
pub const LAW_THIRD: &str = "Absolute zero can never be reached";
const FIRST_RULE: &str = "Q hot = W + Q cold + ΔU";
const STATE_FUNCTION: &str = "ΔU is small next to Q hot: the gas ends each";
const STATE_FUNCTION_2: &str = "turn nearly as it began (a state function).";
const STILL_SETTLING: &str = "ΔU is not yet near 0: the gas is still settling.";
const SPONTANEOUS: &str = "Above zero, so this heat flow is spontaneous.";
const SETTLING_ENTROPY: &str = "Only the surroundings counted while the gas settles.";
const THIRD_RULE: &str = "Carnot limit = 1 − T cold / T hot";
const THIRD_NOTE: [&str; 2] = [
    "100% would need a cold side at 0 K,",
    "and nothing can be cooled all the way to 0 K.",
];
const NO_GAP: &str = "No temperature difference: no work.";

/// Draws the Simple view below `top`; returns the last baseline.
pub fn draw(pen: &mut Pen, readout: &Readout, x: f32, top: f32) -> f32 {
    let turn = readout.turn.as_ref();
    let y = draw_now(pen, readout, x, top);
    let y = draw_energy(pen, readout, x, y + SECTION_GAP);
    let y = draw_first_law(pen, turn, x, y + SECTION_GAP);
    let carnot = carnot(&readout.now);
    let y = draw_second_law(pen, turn, carnot, x, y + SECTION_GAP);
    draw_third_law(pen, &readout.now, carnot, x, y + SECTION_GAP)
}

/// A section heading below `top`; returns its baseline.
fn section(pen: &mut Pen, text: &str, x: f32, top: f32) -> f32 {
    let y = top + LINE;
    pen.fitted(text, x, y, CONTENT_WIDTH, SECTION_SIZE, SECTION);
    y
}

/// Gas temperatures and the flywheel's speed, as three cards. Returns the
/// cards' bottom edge.
fn draw_now(pen: &mut Pen, readout: &Readout, x: f32, top: f32) -> f32 {
    let now = &readout.now;
    let y = section(pen, HEADING_NOW, x, top);
    let width = (CONTENT_WIDTH - 2.0 * CARD_GAP) / 3.0;
    let card_top = y + 8.0;
    let speed = speed_text(now.turns_per_minute);
    let cards = [
        ("Hot gas", kelvin_text(now.hot.kelvin), HOT_COLOR),
        ("Cold gas", kelvin_text(now.cold.kelvin), COLD_COLOR),
        ("Flywheel", speed, TEXT),
    ];
    for (index, (label, value, color)) in cards.into_iter().enumerate() {
        let left = x + index as f32 * (width + CARD_GAP);
        pen.round_rect(left, card_top, width, CARD_HEIGHT, CARD_RADIUS, CARD);
        let inner = width - 2.0 * CARD_PADDING;
        pen.fitted(
            label,
            left + CARD_PADDING,
            card_top + 20.0,
            inner,
            SMALL_SIZE,
            QUIET,
        );
        let size = pen.fitted_size_weight(&value, inner, CARD_VALUE_SIZE, Weight::Semibold);
        pen.strong(&value, left + CARD_PADDING, card_top + 42.0, size, color);
    }
    card_top + CARD_HEIGHT
}

/// The energy heading, what the numbers average over, and the diagram.
/// Returns the diagram's bottom edge.
fn draw_energy(pen: &mut Pen, readout: &Readout, x: f32, top: f32) -> f32 {
    let mut y = section(pen, HEADING_ENERGY, x, top);
    y += SMALL_LINE;
    let subtitle = readout.turn.as_ref().map_or_else(
        || readout.waiting.to_owned(),
        |turn| {
            if turn.turns == 1 {
                format!("The last full turn ({:.0} s)", turn.seconds)
            } else {
                format!(
                    "Average of the last {} turns ({:.0} s each)",
                    turn.turns, turn.seconds
                )
            }
        },
    );
    pen.fitted(&subtitle, x, y, CONTENT_WIDTH, SMALL_SIZE, QUIET);
    draw_diagram(pen, readout, x, y + 12.0)
}

/// "4,900 J", or a dash before there is a number.
fn joules(energy: Option<f64>) -> String {
    energy.map_or_else(
        || "— J".to_owned(),
        |energy| format!("{} J", amount(energy)),
    )
}

/// A box with a name and, under it, a quieter tag.
fn labeled_box(
    pen: &mut Pen,
    (left, top, width): (f32, f32, f32),
    fill: Color,
    name: &str,
    name_color: Color,
    tag: &str,
) {
    pen.round_rect(left, top, width, BOX_HEIGHT, BOX_RADIUS, fill);
    let name_width = pen.measure_weight(name, BOX_NAME_SIZE, Weight::Semibold);
    pen.strong(
        name,
        left + (width - name_width) / 2.0,
        top + 18.0,
        BOX_NAME_SIZE,
        name_color,
    );
    let tag_width = pen.measure(tag, BOX_TAG_SIZE);
    pen.text(
        tag,
        left + (width - tag_width) / 2.0,
        top + 33.0,
        BOX_TAG_SIZE,
        QUIET,
    );
}

/// The heat-engine diagram; returns the bottom of the cold box.
fn draw_diagram(pen: &mut Pen, readout: &Readout, x: f32, top: f32) -> f32 {
    let now = &readout.now;
    let turn = readout.turn.as_ref();
    let gas_top = top + BOX_HEIGHT + ARROW_LENGTH;
    let cold_top = gas_top + BOX_HEIGHT + ARROW_LENGTH;
    let center = x + BOX_WIDTH / 2.0;

    let heat_in = turn.map(|turn| turn.hot_heat);
    let heat_out = turn.map(|turn| -turn.cold_heat);
    let work = turn.map(|turn| turn.work);
    let largest = turn.map_or(0.0, |turn| turn.hot_heat.abs().max(turn.cold_heat.abs()));
    let width_for = |energy: Option<f64>| match energy {
        Some(energy) if largest > 0.0 => {
            (MAX_ARROW_WIDTH * (energy.abs() / largest) as f32).max(MIN_ARROW_WIDTH)
        }
        _ => MIN_ARROW_WIDTH,
    };

    let hot_name = format!("Hot side {:.0} K", now.hot_wall_kelvin);
    let cold_name = format!("Cold side {:.0} K", now.cold_wall_kelvin);
    labeled_box(
        pen,
        (x, top, BOX_WIDTH),
        HOT_BOX,
        &hot_name,
        HOT_COLOR,
        "surroundings",
    );
    labeled_box(
        pen,
        (x, gas_top, BOX_WIDTH),
        GAS_BOX,
        "Helium gas",
        TEXT,
        "the system",
    );
    labeled_box(
        pen,
        (x, cold_top, BOX_WIDTH),
        COLD_BOX,
        &cold_name,
        COLD_COLOR,
        "surroundings",
    );
    let flywheel_left = x + CONTENT_WIDTH - FLYWHEEL_WIDTH;
    labeled_box(
        pen,
        (flywheel_left, gas_top, FLYWHEEL_WIDTH),
        FLYWHEEL_BOX,
        "Flywheel",
        WORK_COLOR,
        "surroundings",
    );

    // Heat in and heat out, down the column, labeled at their right.
    let label_x = center + MAX_ARROW_WIDTH / 2.0 + HEAD_OVERHANG + ARROW_LABEL_GAP;
    for (start, energy, name, color) in [
        (top + BOX_HEIGHT, heat_in, "Heat in, Q hot", HOT_COLOR),
        (
            gas_top + BOX_HEIGHT,
            heat_out,
            "Heat out, Q cold",
            COLD_COLOR,
        ),
    ] {
        down_arrow(pen, center, start, ARROW_LENGTH, width_for(energy), color);
        let middle = start + ARROW_LENGTH / 2.0;
        pen.text(name, label_x, middle - 2.0, SMALL_SIZE, QUIET);
        pen.strong(&joules(energy), label_x, middle + 14.0, BODY_SIZE, color);
    }

    // Work, across to the flywheel, labeled above and below the arrow.
    let work_left = x + BOX_WIDTH;
    let work_length = flywheel_left - work_left;
    let middle = gas_top + BOX_HEIGHT / 2.0;
    right_arrow(
        pen,
        work_left,
        middle,
        work_length,
        width_for(work),
        WORK_COLOR,
    );
    let label_left = work_left + 6.0;
    let room = work_length - 12.0;
    pen.fitted(
        "Work, W",
        label_left,
        middle - 10.0,
        room,
        SMALL_SIZE,
        QUIET,
    );
    let size = pen.fitted_size_weight(&joules(work), room, BODY_SIZE, Weight::Semibold);
    pen.strong(&joules(work), label_left, middle + 24.0, size, WORK_COLOR);
    cold_top + BOX_HEIGHT
}

/// An arrow pointing down from (`center`, `top`), `length` long overall.
fn down_arrow(pen: &Pen, center: f32, top: f32, length: f32, width: f32, color: Color) {
    let shaft = length - HEAD_LENGTH;
    pen.rect(center - width / 2.0, top, width, shaft, color);
    let half = width / 2.0 + HEAD_OVERHANG;
    pen.triangle(
        vec2(center - half, top + shaft),
        vec2(center + half, top + shaft),
        vec2(center, top + length),
        color,
    );
}

/// An arrow pointing right from (`left`, `middle`), `length` long overall.
fn right_arrow(pen: &Pen, left: f32, middle: f32, length: f32, width: f32, color: Color) {
    let shaft = length - HEAD_LENGTH;
    pen.rect(left, middle - width / 2.0, shaft, width, color);
    let half = width / 2.0 + HEAD_OVERHANG;
    pen.triangle(
        vec2(left + shaft, middle - half),
        vec2(left + shaft, middle + half),
        vec2(left + length, middle),
        color,
    );
}

/// A law's numbered badge and heading; returns the heading's baseline.
fn law_heading(pen: &mut Pen, number: &str, text: &str, x: f32, top: f32) -> f32 {
    let y = top + LINE;
    pen.circle(x + BADGE_RADIUS, y - 5.0, BADGE_RADIUS, LAW);
    let width = pen.measure_weight(number, BADGE_SIZE, Weight::Semibold);
    pen.strong(
        number,
        x + BADGE_RADIUS - width / 2.0,
        y,
        BADGE_SIZE,
        BADGE_TEXT,
    );
    pen.fitted_heading(
        text,
        x + INDENT,
        y,
        CONTENT_WIDTH - INDENT,
        HEADING_SIZE,
        LAW,
    );
    y
}

fn draw_first_law(pen: &mut Pen, turn: Option<&Turn>, x: f32, top: f32) -> f32 {
    let left = x + INDENT;
    let room = CONTENT_WIDTH - INDENT;
    let mut y = law_heading(pen, "1", LAW_FIRST, x, top);
    y += LINE;
    pen.fitted(FIRST_RULE, left, y, room, BODY_SIZE, TEXT);
    y += LINE;
    let Some(turn) = turn else {
        pen.fitted("— = — + — + — J", left, y, room, BODY_SIZE, QUIET);
        return y;
    };
    let stored_sign = if turn.stored < 0.0 { " − " } else { " + " };
    let numbers = [
        amount(turn.hot_heat),
        amount(turn.work),
        amount(-turn.cold_heat),
        amount(turn.stored.abs()),
    ];
    let pieces = [
        (numbers[0].as_str(), HOT_COLOR, Weight::Semibold),
        (" = ", TEXT, Weight::Regular),
        (numbers[1].as_str(), WORK_COLOR, Weight::Semibold),
        (" + ", TEXT, Weight::Regular),
        (numbers[2].as_str(), COLD_COLOR, Weight::Semibold),
        (stored_sign, TEXT, Weight::Regular),
        (numbers[3].as_str(), TEXT, Weight::Semibold),
        (" J", TEXT, Weight::Regular),
    ];
    let natural = pen.runs_width(&pieces, BODY_SIZE);
    let size = if natural > room {
        (BODY_SIZE * room / natural).floor()
    } else {
        BODY_SIZE
    };
    pen.runs(&pieces, left, y, size);
    let notes: &[&str] = if turn.settled {
        &[STATE_FUNCTION, STATE_FUNCTION_2]
    } else {
        &[STILL_SETTLING]
    };
    for note in notes {
        y += SMALL_LINE;
        pen.fitted(note, left, y, room, SMALL_SIZE, QUIET);
    }
    y
}

fn draw_second_law(pen: &mut Pen, turn: Option<&Turn>, carnot: f64, x: f32, top: f32) -> f32 {
    let left = x + INDENT;
    let room = CONTENT_WIDTH - INDENT;
    let mut y = law_heading(pen, "2", LAW_SECOND, x, top);
    y += LINE;
    let (entropy, note, color) = match turn {
        None => (
            "ΔS universe = — J/K per turn".to_owned(),
            SPONTANEOUS,
            QUIET,
        ),
        Some(turn) if !turn.settled => (
            format!(
                "ΔS surroundings = {} J/K per turn",
                signed_number(turn.entropy, 1)
            ),
            SETTLING_ENTROPY,
            TEXT,
        ),
        Some(turn) if turn.entropy > 0.0 => (
            format!(
                "ΔS universe = {} J/K per turn",
                signed_number(turn.entropy, 1)
            ),
            SPONTANEOUS,
            GOOD,
        ),
        Some(turn) => (
            format!(
                "ΔS universe = {} J/K per turn",
                signed_number(turn.entropy, 1)
            ),
            "Unexpected: it should be above zero.",
            WARNING,
        ),
    };
    let size = pen.fitted_size_weight(&entropy, room, BODY_SIZE, Weight::Semibold);
    pen.strong(&entropy, left, y, size, color);
    y += SMALL_LINE;
    pen.fitted(note, left, y, room, SMALL_SIZE, QUIET);

    // Efficiency as a bar, with the Carnot limit marked on it.
    y += 14.0;
    let efficiency = turn.and_then(|turn| turn.efficiency).unwrap_or(0.0) as f32;
    let carnot = carnot as f32;
    let radius = BAR_HEIGHT / 2.0;
    pen.round_rect(left, y, room, BAR_HEIGHT, radius, TRACK);
    if efficiency > 0.0 {
        let filled = (room * efficiency).max(BAR_HEIGHT);
        pen.round_rect(left, y, filled, BAR_HEIGHT, radius, WORK_COLOR);
    }
    if carnot > 0.0 {
        let marker = left + room * carnot;
        let (top, bottom) = (y - MARKER_OVERHANG, y + BAR_HEIGHT + MARKER_OVERHANG);
        pen.line(marker, top, marker, bottom, 2.0, TEXT);
    }
    y += BAR_HEIGHT + LINE;
    match turn {
        _ if carnot <= 0.0 => pen.fitted(NO_GAP, left, y, room, SMALL_SIZE, QUIET),
        _ => {
            let efficiency = turn.and_then(|turn| turn.efficiency).map_or_else(
                || turn.map_or("—".to_owned(), |_| "0%".to_owned()),
                |value| format!("{:.1}%", 100.0 * value),
            );
            let limit = format!("{:.1}%", 100.0 * carnot);
            pen.runs(
                &[
                    ("Efficiency ", QUIET, Weight::Regular),
                    (&efficiency, WORK_COLOR, Weight::Semibold),
                ],
                left,
                y,
                SMALL_SIZE + 1.0,
            );
            let pieces = [
                ("Carnot limit ", QUIET, Weight::Regular),
                (limit.as_str(), TEXT, Weight::Semibold),
            ];
            let width = pen.runs_width(&pieces, SMALL_SIZE + 1.0);
            pen.runs(&pieces, left + room - width, y, SMALL_SIZE + 1.0);
        }
    }
    y
}

fn draw_third_law(pen: &mut Pen, now: &Now, carnot: f64, x: f32, top: f32) -> f32 {
    let left = x + INDENT;
    let room = CONTENT_WIDTH - INDENT;
    let mut y = law_heading(pen, "3", LAW_THIRD, x, top);
    y += LINE;
    pen.fitted(THIRD_RULE, left, y, room, BODY_SIZE, TEXT);
    y += LINE;
    let example = format!(
        "Here: 1 − {:.0} / {:.0} = {:.1}%",
        now.cold_wall_kelvin,
        now.hot_wall_kelvin,
        100.0 * carnot
    );
    pen.fitted(&example, left, y, room, BODY_SIZE, TEXT);
    for line in THIRD_NOTE {
        y += SMALL_LINE;
        pen.fitted(line, left, y, room, SMALL_SIZE, QUIET);
    }
    y
}

/// The Carnot limit for the sliders' temperatures now, zero without a
/// difference.
fn carnot(now: &Now) -> f64 {
    if now.hot_wall_kelvin > now.cold_wall_kelvin {
        1.0 - now.cold_wall_kelvin as f64 / now.hot_wall_kelvin as f64
    } else {
        0.0
    }
}
