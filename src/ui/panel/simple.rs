//! The Simple view: what the project asks about, in the plainest form.
//!
//! It shows the gas temperatures and the flywheel's speed now, a diagram of
//! where the energy goes each turn (the helium is the system; the hot side,
//! the cold side and the flywheel are its surroundings), and one card per
//! law with the engine's own numbers as evidence. Colors match across the
//! view: orange for heat in, yellow for work, blue for heat out.

use macroquad::prelude::{
    Color,
    Rect,
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
    LEFT,
    PEARL,
    QUIET,
    SECTION,
    SECTION_SIZE,
    SMALL_SIZE,
    TEXT,
    TRACK,
    WARNING,
    WORK_COLOR,
    tile, //
};
use crate::ui::pen::{
    Pen,
    Weight, //
};

/// Distance between baselines.
const LINE: f32 = 20.0;
/// Distance between baselines of small text.
const SMALL_LINE: f32 = 17.0;
/// Space between sections.
const SECTION_GAP: f32 = 10.0;

/// Gap between the three "Right now" cards.
const CARD_GAP: f32 = 8.0;
/// Gap between the heading and the cards.
const CARD_TOP_GAP: f32 = 8.0;
/// Height of a card.
const CARD_HEIGHT: f32 = 52.0;
/// Corner radius of a card.
const CARD_RADIUS: f32 = 12.0;
/// Space between a card's edge and its text.
const CARD_PADDING: f32 = 10.0;
/// From a card's top to its label's baseline.
const CARD_LABEL_BASELINE: f32 = 20.0;
/// From a card's top to its value's baseline.
const CARD_VALUE_BASELINE: f32 = 42.0;
/// Text size of a card's value.
const CARD_VALUE_SIZE: f32 = 20.0;

/// Gap between the subtitle and the diagram.
///
/// The diagram has the hot side, the helium and the cold side in a column
/// joined by heat arrows, and a work arrow from the helium to the flywheel
/// at the right. Arrow widths follow the energy each carries.
const DIAGRAM_GAP: f32 = 12.0;
/// Width of the boxes in the column.
const BOX_WIDTH: f32 = 150.0;
/// Height of every box.
const BOX_HEIGHT: f32 = 40.0;
/// Corner radius of every box.
const BOX_RADIUS: f32 = 12.0;
/// Text size of a box's name.
const BOX_NAME_SIZE: f32 = 15.0;
/// From a box's top to its name's baseline.
const BOX_NAME_BASELINE: f32 = 18.0;
/// Text size of a box's tag.
const BOX_TAG_SIZE: f32 = 12.0;
/// From a box's top to its tag's baseline.
const BOX_TAG_BASELINE: f32 = 33.0;
/// Width of the flywheel's box.
const FLYWHEEL_WIDTH: f32 = 94.0;
/// Length of a heat arrow, the gap between boxes in the column.
const ARROW_LENGTH: f32 = 36.0;
/// Width of the arrow carrying the most energy.
const MAX_ARROW_WIDTH: f32 = 22.0;
/// Narrowest arrow.
const MIN_ARROW_WIDTH: f32 = 3.0;
/// Length of an arrow's head.
const HEAD_LENGTH: f32 = 9.0;
/// How far an arrow's head reaches past its shaft on each side.
const HEAD_OVERHANG: f32 = 5.0;
/// Gap between the widest heat arrow's head and its labels.
const ARROW_LABEL_GAP: f32 = 14.0;
/// From a heat arrow's middle up to its name's baseline.
const ARROW_NAME_RISE: f32 = 2.0;
/// From a heat arrow's middle down to its value's baseline.
const ARROW_VALUE_DROP: f32 = 14.0;
/// Inset of the work arrow's labels from its ends.
const WORK_LABEL_INSET: f32 = 6.0;
/// From the work arrow's middle up to its name's baseline.
const WORK_NAME_RISE: f32 = 10.0;
/// From the work arrow's middle down to its value's baseline.
const WORK_VALUE_DROP: f32 = 24.0;

/// Radius of a law's numbered badge.
const BADGE_RADIUS: f32 = 11.0;
/// From a law heading's baseline up to its badge's middle.
const BADGE_RISE: f32 = 5.0;
/// Text size of a badge's number.
const BADGE_SIZE: f32 = 14.0;
/// Color of a badge's number.
const BADGE_TEXT: Color = Color::from_rgba(16, 26, 38, 255);
/// Indent of a law card's heading and evidence, clear of the badge.
const INDENT: f32 = 30.0;
/// Width of a law card's text.
const LAW_WIDTH: f32 = CONTENT_WIDTH - INDENT;
/// Text size of a law heading.
const HEADING_SIZE: f32 = 16.0;
/// Gap above the efficiency bar.
const BAR_GAP: f32 = 14.0;
/// Thickness of the efficiency bar.
const BAR_HEIGHT: f32 = 10.0;
/// How far the Carnot mark reaches past the bar.
const MARKER_OVERHANG: f32 = 4.0;
/// Thickness of the Carnot mark.
const MARKER_THICKNESS: f32 = 2.0;
/// Text size of the efficiency and Carnot readings.
const EFFICIENCY_SIZE: f32 = SMALL_SIZE + 1.0;

/// Heading of the cards.
const HEADING_NOW: &str = "RIGHT NOW";
/// Heading of the diagram.
const HEADING_ENERGY: &str = "WHERE THE ENERGY GOES EACH TURN";
/// The first law, in words.
pub const LAW_FIRST: &str = "Energy is conserved";
/// The second law, in words.
pub const LAW_SECOND: &str = "Heat flows from hot to cold on its own";
/// The third law, in words.
pub const LAW_THIRD: &str = "Absolute zero can never be reached";
/// The first law as the engine's ledger.
const FIRST_RULE: &str = "Q hot = W + Q cold + ΔU";
/// Note on a settled gas.
const STATE_FUNCTION: [&str; 2] = [
    "ΔU is small next to Q hot: the gas ends each",
    "turn nearly as it began (a state function).",
];
/// Note on a gas still settling.
const STILL_SETTLING: &str = "ΔU is not yet near 0: the gas is still settling.";
/// Note on a positive entropy change.
const SPONTANEOUS: &str = "Above zero, so this heat flow is spontaneous.";
/// Note on the entropy while the gas settles.
const SETTLING_ENTROPY: &str = "Only the surroundings counted while the gas settles.";
/// Note on an entropy change that is not positive.
const UNEXPECTED_ENTROPY: &str = "Unexpected: it should be above zero.";
/// The third law's bearing on efficiency.
const THIRD_RULE: &str = "Carnot limit = 1 − T cold / T hot";
/// Note on the third law.
const THIRD_NOTE: [&str; 2] = [
    "100% would need a cold side at 0 K,",
    "and nothing can be cooled all the way to 0 K.",
];
/// Shown in place of the efficiency without a temperature difference.
const NO_GAP: &str = "No temperature difference: no work.";

/// Draws the Simple view below `top` and returns the last baseline.
pub fn draw(pen: &mut Pen, readout: &Readout, top: f32) -> f32 {
    let turn = readout.turn.as_ref();
    let y = draw_now(pen, readout, top);
    let y = draw_energy(pen, readout, y + SECTION_GAP);
    let y = draw_first_law(pen, turn, y + SECTION_GAP);
    let carnot = carnot(&readout.now);
    let y = draw_second_law(pen, turn, carnot, y + SECTION_GAP);
    draw_third_law(pen, &readout.now, carnot, y + SECTION_GAP)
}

/// Draws a section heading below `top` and returns its baseline.
fn section(pen: &mut Pen, text: &str, top: f32) -> f32 {
    let y = top + LINE;
    pen.fitted(text, LEFT, y, CONTENT_WIDTH, SECTION_SIZE, SECTION);
    y
}

/// Draws the gas temperatures and the flywheel's speed as three cards, and
/// returns the cards' bottom edge.
fn draw_now(pen: &mut Pen, readout: &Readout, top: f32) -> f32 {
    let now = &readout.now;
    let card_top = section(pen, HEADING_NOW, top) + CARD_TOP_GAP;
    let width = (CONTENT_WIDTH - 2.0 * CARD_GAP) / 3.0;
    let cards = [
        ("Hot gas", kelvin_text(now.hot.kelvin), HOT_COLOR, HOT_COLOR),
        (
            "Cold gas",
            kelvin_text(now.cold.kelvin),
            COLD_COLOR,
            COLD_COLOR,
        ),
        (
            "Flywheel",
            speed_text(now.turns_per_minute),
            TEXT,
            WORK_COLOR,
        ),
    ];
    for (index, (label, value, color, tint)) in cards.into_iter().enumerate() {
        let left = LEFT + index as f32 * (width + CARD_GAP);
        tile(
            pen,
            Rect::new(left, card_top, width, CARD_HEIGHT),
            CARD_RADIUS,
            tint,
        );
        let (text_left, room) = (left + CARD_PADDING, width - 2.0 * CARD_PADDING);
        let label_y = card_top + CARD_LABEL_BASELINE;
        pen.fitted(label, text_left, label_y, room, SMALL_SIZE, QUIET);
        let size = pen.fitted_size_weight(&value, room, CARD_VALUE_SIZE, Weight::Semibold);
        pen.strong(
            &value,
            text_left,
            card_top + CARD_VALUE_BASELINE,
            size,
            color,
        );
    }
    card_top + CARD_HEIGHT
}

/// Draws the energy heading, what the numbers average over, and the
/// diagram, and returns the diagram's bottom edge.
fn draw_energy(pen: &mut Pen, readout: &Readout, top: f32) -> f32 {
    let y = section(pen, HEADING_ENERGY, top) + SMALL_LINE;
    let subtitle = match &readout.turn {
        None => readout.waiting.to_owned(),
        Some(turn) if turn.turns == 1 => format!("The last full turn ({:.0} s)", turn.seconds),
        Some(turn) => format!(
            "Average of the last {} turns ({:.0} s each)",
            turn.turns, turn.seconds
        ),
    };
    pen.fitted(&subtitle, LEFT, y, CONTENT_WIDTH, SMALL_SIZE, QUIET);
    draw_diagram(pen, readout, y + DIAGRAM_GAP)
}

/// An energy such as "4,900 J", or a dash before there is a number.
fn joules(energy: Option<f64>) -> String {
    energy.map_or_else(
        || "— J".to_owned(),
        |energy| format!("{} J", amount(energy)),
    )
}

/// One box of the diagram.
struct LabeledBox<'a> {
    /// Left edge.
    left: f32,
    /// Top edge.
    top: f32,
    /// Width.
    width: f32,
    /// Tint of the glass.
    tint: Color,
    /// What it stands for.
    name: &'a str,
    /// Color of the name.
    name_color: Color,
    /// Its part in the system, under the name.
    tag: &'a str,
}

impl LabeledBox<'_> {
    /// Draws the box: a glass tile with the name and, under it, a quieter
    /// tag.
    fn draw(&self, pen: &mut Pen) {
        let rect = Rect::new(self.left, self.top, self.width, BOX_HEIGHT);
        tile(pen, rect, BOX_RADIUS, self.tint);
        let centered = |width: f32| self.left + (self.width - width) / 2.0;
        let name_width = pen.measure_weight(self.name, BOX_NAME_SIZE, Weight::Semibold);
        let name_y = self.top + BOX_NAME_BASELINE;
        pen.strong(
            self.name,
            centered(name_width),
            name_y,
            BOX_NAME_SIZE,
            self.name_color,
        );
        let tag_x = centered(pen.measure(self.tag, BOX_TAG_SIZE));
        pen.text(
            self.tag,
            tag_x,
            self.top + BOX_TAG_BASELINE,
            BOX_TAG_SIZE,
            QUIET,
        );
    }
}

/// Draws the heat-engine diagram below `top` and returns the bottom of the
/// cold box.
fn draw_diagram(pen: &mut Pen, readout: &Readout, top: f32) -> f32 {
    let now = &readout.now;
    let turn = readout.turn.as_ref();
    let gas_top = top + BOX_HEIGHT + ARROW_LENGTH;
    let cold_top = gas_top + BOX_HEIGHT + ARROW_LENGTH;
    let center = LEFT + BOX_WIDTH / 2.0;

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
    let flywheel_left = LEFT + CONTENT_WIDTH - FLYWHEEL_WIDTH;
    let column = |top, tint, name, name_color, tag| LabeledBox {
        left: LEFT,
        top,
        width: BOX_WIDTH,
        tint,
        name,
        name_color,
        tag,
    };
    let boxes = [
        column(top, HOT_COLOR, &hot_name, HOT_COLOR, "surroundings"),
        column(gas_top, PEARL, "Helium gas", TEXT, "the system"),
        column(cold_top, COLD_COLOR, &cold_name, COLD_COLOR, "surroundings"),
        LabeledBox {
            left: flywheel_left,
            width: FLYWHEEL_WIDTH,
            ..column(gas_top, WORK_COLOR, "Flywheel", WORK_COLOR, "surroundings")
        },
    ];
    for labeled_box in &boxes {
        labeled_box.draw(pen);
    }

    // Heat in and heat out, down the column, labeled at their right.
    let label_x = center + MAX_ARROW_WIDTH / 2.0 + HEAD_OVERHANG + ARROW_LABEL_GAP;
    let heat_arrows = [
        (top + BOX_HEIGHT, heat_in, "Heat in, Q hot", HOT_COLOR),
        (
            gas_top + BOX_HEIGHT,
            heat_out,
            "Heat out, Q cold",
            COLD_COLOR,
        ),
    ];
    for (start, energy, name, color) in heat_arrows {
        down_arrow(pen, center, start, width_for(energy), color);
        let middle = start + ARROW_LENGTH / 2.0;
        pen.text(name, label_x, middle - ARROW_NAME_RISE, SMALL_SIZE, QUIET);
        pen.strong(
            &joules(energy),
            label_x,
            middle + ARROW_VALUE_DROP,
            BODY_SIZE,
            color,
        );
    }

    // Work, across to the flywheel, labeled above and below the arrow.
    let work_left = LEFT + BOX_WIDTH;
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
    let label_left = work_left + WORK_LABEL_INSET;
    let room = work_length - 2.0 * WORK_LABEL_INSET;
    pen.fitted(
        "Work, W",
        label_left,
        middle - WORK_NAME_RISE,
        room,
        SMALL_SIZE,
        QUIET,
    );
    let work = joules(work);
    let size = pen.fitted_size_weight(&work, room, BODY_SIZE, Weight::Semibold);
    pen.strong(
        &work,
        label_left,
        middle + WORK_VALUE_DROP,
        size,
        WORK_COLOR,
    );
    cold_top + BOX_HEIGHT
}

/// Draws an arrow `ARROW_LENGTH` long pointing down from (`center`, `top`).
fn down_arrow(pen: &Pen, center: f32, top: f32, width: f32, color: Color) {
    let shaft = ARROW_LENGTH - HEAD_LENGTH;
    pen.rect(Rect::new(center - width / 2.0, top, width, shaft), color);
    let half = width / 2.0 + HEAD_OVERHANG;
    let base = top + shaft;
    let tip = vec2(center, top + ARROW_LENGTH);
    pen.triangle(
        vec2(center - half, base),
        vec2(center + half, base),
        tip,
        color,
    );
}

/// Draws an arrow `length` long pointing right from (`left`, `middle`).
fn right_arrow(pen: &Pen, left: f32, middle: f32, length: f32, width: f32, color: Color) {
    let shaft = length - HEAD_LENGTH;
    pen.rect(Rect::new(left, middle - width / 2.0, shaft, width), color);
    let half = width / 2.0 + HEAD_OVERHANG;
    let base = left + shaft;
    let tip = vec2(left + length, middle);
    pen.triangle(
        vec2(base, middle - half),
        vec2(base, middle + half),
        tip,
        color,
    );
}

/// Draws a law's numbered badge and heading below `top` and returns the
/// heading's baseline.
fn law_heading(pen: &mut Pen, number: &str, text: &str, top: f32) -> f32 {
    let y = top + LINE;
    let badge_x = LEFT + BADGE_RADIUS;
    pen.circle(vec2(badge_x, y - BADGE_RISE), BADGE_RADIUS, LAW);
    let width = pen.measure_weight(number, BADGE_SIZE, Weight::Semibold);
    pen.strong(number, badge_x - width / 2.0, y, BADGE_SIZE, BADGE_TEXT);
    pen.fitted_heading(text, LEFT + INDENT, y, LAW_WIDTH, HEADING_SIZE, LAW);
    y
}

/// Draws the first law's card below `top` and returns the last baseline.
fn draw_first_law(pen: &mut Pen, turn: Option<&Turn>, top: f32) -> f32 {
    let left = LEFT + INDENT;
    let mut y = law_heading(pen, "1", LAW_FIRST, top) + LINE;
    pen.fitted(FIRST_RULE, left, y, LAW_WIDTH, BODY_SIZE, TEXT);
    y += LINE;
    let Some(turn) = turn else {
        pen.fitted("— = — + — + — J", left, y, LAW_WIDTH, BODY_SIZE, QUIET);
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
    let size = if natural > LAW_WIDTH {
        (BODY_SIZE * LAW_WIDTH / natural).floor()
    } else {
        BODY_SIZE
    };
    pen.runs(&pieces, left, y, size);
    let notes: &[&str] = if turn.settled {
        &STATE_FUNCTION
    } else {
        &[STILL_SETTLING]
    };
    for note in notes {
        y += SMALL_LINE;
        pen.fitted(note, left, y, LAW_WIDTH, SMALL_SIZE, QUIET);
    }
    y
}

/// Draws the second law's card below `top`, with the efficiency against
/// the `carnot` limit, and returns the last baseline.
fn draw_second_law(pen: &mut Pen, turn: Option<&Turn>, carnot: f64, top: f32) -> f32 {
    let left = LEFT + INDENT;
    let mut y = law_heading(pen, "2", LAW_SECOND, top) + LINE;
    let entropy = |what: &str, turn: &Turn| {
        format!(
            "ΔS {what} = {} J/K per turn",
            signed_number(turn.entropy, 1)
        )
    };
    let (entropy, note, color) = match turn {
        None => (
            "ΔS universe = — J/K per turn".to_owned(),
            SPONTANEOUS,
            QUIET,
        ),
        Some(turn) if !turn.settled => (entropy("surroundings", turn), SETTLING_ENTROPY, TEXT),
        Some(turn) if turn.entropy > 0.0 => (entropy("universe", turn), SPONTANEOUS, GOOD),
        Some(turn) => (entropy("universe", turn), UNEXPECTED_ENTROPY, WARNING),
    };
    let size = pen.fitted_size_weight(&entropy, LAW_WIDTH, BODY_SIZE, Weight::Semibold);
    pen.strong(&entropy, left, y, size, color);
    y += SMALL_LINE;
    pen.fitted(note, left, y, LAW_WIDTH, SMALL_SIZE, QUIET);

    // Efficiency as a bar, with the Carnot limit marked on it.
    y += BAR_GAP;
    let efficiency = turn.and_then(|turn| turn.efficiency);
    let filled = efficiency.unwrap_or(0.0) as f32;
    let carnot = carnot as f32;
    let radius = BAR_HEIGHT / 2.0;
    let bar = Rect::new(left, y, LAW_WIDTH, BAR_HEIGHT);
    pen.round_rect(bar, radius, TRACK);
    if filled > 0.0 {
        let width = (LAW_WIDTH * filled).max(BAR_HEIGHT);
        pen.round_rect(Rect { w: width, ..bar }, radius, WORK_COLOR);
    }
    if carnot > 0.0 {
        let marker = left + LAW_WIDTH * carnot;
        let (top, bottom) = (y - MARKER_OVERHANG, y + BAR_HEIGHT + MARKER_OVERHANG);
        pen.line(
            vec2(marker, top),
            vec2(marker, bottom),
            MARKER_THICKNESS,
            TEXT,
        );
    }
    y += BAR_HEIGHT + LINE;
    if carnot <= 0.0 {
        pen.fitted(NO_GAP, left, y, LAW_WIDTH, SMALL_SIZE, QUIET);
        return y;
    }
    let efficiency = match (turn, efficiency) {
        (_, Some(value)) => format!("{:.1}%", 100.0 * value),
        (Some(_), None) => "0%".to_owned(),
        (None, None) => "—".to_owned(),
    };
    let measured = [
        ("Efficiency ", QUIET, Weight::Regular),
        (efficiency.as_str(), WORK_COLOR, Weight::Semibold),
    ];
    pen.runs(&measured, left, y, EFFICIENCY_SIZE);
    let limit = format!("{:.1}%", 100.0 * carnot);
    let limit = [
        ("Carnot limit ", QUIET, Weight::Regular),
        (limit.as_str(), TEXT, Weight::Semibold),
    ];
    let width = pen.runs_width(&limit, EFFICIENCY_SIZE);
    pen.runs(&limit, left + LAW_WIDTH - width, y, EFFICIENCY_SIZE);
    y
}

/// Draws the third law's card below `top`, with the `carnot` limit for the
/// settings now, and returns the last baseline.
fn draw_third_law(pen: &mut Pen, now: &Now, carnot: f64, top: f32) -> f32 {
    let left = LEFT + INDENT;
    let mut y = law_heading(pen, "3", LAW_THIRD, top) + LINE;
    pen.fitted(THIRD_RULE, left, y, LAW_WIDTH, BODY_SIZE, TEXT);
    y += LINE;
    let example = format!(
        "Here: 1 − {:.0} / {:.0} = {:.1}%",
        now.cold_wall_kelvin,
        now.hot_wall_kelvin,
        100.0 * carnot
    );
    pen.fitted(&example, left, y, LAW_WIDTH, BODY_SIZE, TEXT);
    for line in THIRD_NOTE {
        y += SMALL_LINE;
        pen.fitted(line, left, y, LAW_WIDTH, SMALL_SIZE, QUIET);
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
