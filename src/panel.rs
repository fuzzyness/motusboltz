//! Left-column panel. "Engine now" shows live readings; "Last full turn"
//! shows the per-turn energy ledger, efficiency against the Carnot limit,
//! entropy change and each cylinder's pressure-volume loop, each labeled
//! with the law of thermodynamics it demonstrates.
//!
//! Text is rebuilt at most `REFRESH_SECONDS` apart, not every frame. The
//! loops are read from the engine each frame, since they are short.

use macroquad::prelude::*;

use crate::cycle::{
    COLD,
    HOT,
    LoopPoint,
    TurnSummary,
    loop_work, //
};
use crate::engine::ConnectedChambers;
use crate::gas::GasMeasurements;

/// Width of the panel column, in screen pixels.
pub const COLUMN_WIDTH: f32 = 340.0;
const MARGIN: f32 = 16.0;
const CONTENT_WIDTH: f32 = COLUMN_WIDTH - 2.0 * MARGIN;
/// Seconds of wall time between text refreshes.
const REFRESH_SECONDS: f64 = 0.1;
const PASCALS_PER_BAR: f64 = 100_000.0;
const SECONDS_PER_MINUTE: f32 = 60.0;

const TITLE_SIZE: f32 = 28.0;
const SECTION_SIZE: f32 = 15.0;
const LAW_SIZE: f32 = 15.0;
const BODY_SIZE: f32 = 15.0;
const SMALL_SIZE: f32 = 13.0;
const LINE: f32 = 19.0;
const SMALL_LINE: f32 = 16.0;
const TITLE_BASELINE: f32 = 36.0;
const SECTION_GAP: f32 = 14.0;
const READOUT_GAP: f32 = 10.0;
const BAR_HEIGHT: f32 = 12.0;
const BAR_GAP: f32 = 4.0;
const BAR_LABEL_WIDTH: f32 = 34.0;
const LEGEND_SWATCH: f32 = 9.0;
const CARNOT_MARKER_OVERHANG: f32 = 3.0;
/// Room left of the P-V plot for pressure labels, and below it for the
/// volume labels and the two-line loop key.
const PLOT_LEFT: f32 = 34.0;
const PLOT_BOTTOM: f32 = 3.0 * SMALL_LINE + 4.0;
/// Below this the P-V plot is unreadable and is replaced by a note.
const MIN_PLOT_SIDE: f32 = 90.0;
const LOOP_THICKNESS: f32 = 2.0;
const TRACE_THICKNESS: f32 = 1.0;
const DOT_RADIUS: f32 = 4.0;
/// Pressure axis limits are rounded out to multiples of this many bar.
const PRESSURE_STEP_BAR: f64 = 10.0;
/// The ledger is shown as balanced to one decimal; a larger mismatch is
/// flagged. Measured mismatches are about 0.001 J per turn.
const LEDGER_TOLERANCE_JOULES: f64 = 0.05;

const PANEL: Color = Color::from_rgba(19, 29, 43, 245);
const TEXT: Color = Color::from_rgba(215, 226, 238, 255);
const QUIET: Color = Color::from_rgba(150, 165, 180, 255);
const SECTION: Color = Color::from_rgba(120, 140, 160, 255);
const LAW: Color = Color::from_rgba(180, 224, 209, 255);
const WARNING: Color = Color::from_rgba(255, 130, 110, 255);
const GOOD: Color = Color::from_rgba(140, 220, 150, 255);
const TRACK: Color = Color::from_rgba(52, 65, 81, 255);
const HOT_COLOR: Color = Color::from_rgba(255, 160, 90, 255);
const COLD_COLOR: Color = Color::from_rgba(95, 190, 255, 255);
const WORK_COLOR: Color = Color::from_rgba(255, 218, 110, 255);
const STORED_COLOR: Color = Color::from_rgba(150, 160, 175, 255);
const LOOP_FILL_ALPHA: f32 = 0.18;
const TRACE_ALPHA: f32 = 0.45;

pub const TITLE: &str = "MotusBoltz";
const SECTION_NOW: &str = "ENGINE NOW";
const LAW_FIRST: &str = "1st law: energy is conserved";
const LAW_CARNOT: &str = "2nd law: no engine beats Carnot";
const LAW_ENTROPY: &str = "2nd law: entropy of the universe increases";
const LAW_WORK: &str = "Work: area inside each P-V loop (last turn)";
const LEDGER_EQUATION: &str = "Q hot + Q cold = W + dU";
const THIRD_LAW_NOTE: &str = "3rd law: 100% would need T cold = 0 K";
const ENTROPY_EQUATION: &str = "dS surr = -Q hot/T hot - Q cold/T cold";
const WAITING_START: &str = "Press Kick (A) to start the engine.";
const WAITING_TURN: &str = "Waiting for the crank to finish a full turn.";
const WAITING_NOTE: &str = "Readings use whole turns only.";
const LIMITED_TEXT: &str = "Physics time limited: computer too slow";
const PLOT_TOO_SMALL: &str = "Hide details (D) or enlarge the window for P-V loops";
const LOOP_KEY: [(&str, Color); 2] = [
    ("Hot loop: gas does work on the piston (+)", HOT_COLOR),
    ("Cold loop: piston does work on the gas (-)", COLD_COLOR),
];
const LEGEND: [(&str, Color); 4] = [
    ("Q hot", HOT_COLOR),
    ("Q cold", COLD_COLOR),
    ("W", WORK_COLOR),
    ("dU", STORED_COLOR),
];

/// A line of text with its color.
type Line = (String, Color);

/// Formatted readings for the recent turns.
struct TurnReadout {
    heading: String,
    /// Energy flowing in and out, as (amount, color) bar segments.
    inflow: Vec<(f64, Color)>,
    outflow: Vec<(f64, Color)>,
    ledger_values: String,
    /// Shown only if the ledger fails to balance, which would be a bug.
    ledger_warning: Option<String>,
    efficiency: f64,
    carnot: f64,
    efficiency_text: Line,
    carnot_text: String,
    entropy_value: Line,
    entropy_note: Line,
}

/// Axis limits for the P-V plot: volume in cm3 and pressure in bar.
#[derive(Clone, Copy, Debug, PartialEq)]
struct PlotRange {
    volume: (f64, f64),
    pressure: (f64, f64),
}

pub struct EnginePanel {
    now_lines: Vec<Line>,
    detail_lines: Vec<String>,
    turn: Option<TurnReadout>,
    waiting: &'static str,
    loop_text: String,
    range: Option<PlotRange>,
    next_refresh: f64,
    limited_since_refresh: bool,
    show_details: bool,
}

impl EnginePanel {
    pub fn new() -> Self {
        Self {
            now_lines: Vec::new(),
            detail_lines: Vec::new(),
            turn: None,
            waiting: WAITING_START,
            loop_text: String::new(),
            range: None,
            next_refresh: 0.0,
            limited_since_refresh: false,
            show_details: false,
        }
    }

    pub fn toggle_details(&mut self) {
        self.show_details = !self.show_details;
        self.next_refresh = 0.0;
    }

    pub fn details_shown(&self) -> bool {
        self.show_details
    }

    /// Whether `pointer` is over the panel column.
    pub fn blocks_camera(pointer: Vec2) -> bool {
        pointer.x <= COLUMN_WIDTH
    }

    /// Rebuilds the text if `REFRESH_SECONDS` have passed since the last
    /// rebuild. `limited` reports whether the physics dropped frame time.
    pub fn update(&mut self, engine: &ConnectedChambers, limited: bool, now: f64) {
        self.limited_since_refresh |= limited;
        if now < self.next_refresh {
            return;
        }
        self.next_refresh = now + REFRESH_SECONDS;
        self.refresh(engine, macroquad::time::get_fps());
    }

    fn refresh(&mut self, engine: &ConnectedChambers, fps: i32) {
        let regions = engine.measurements();
        let pistons = engine.power_pistons();
        self.now_lines = vec![
            (gas_line("Hot gas", regions.hot), HOT_COLOR),
            (gas_line("Cold gas", regions.cold), COLD_COLOR),
            (
                format!(
                    "Crank: {:+.2} model rpm | {:.1} turns",
                    pistons.angular_velocity() * SECONDS_PER_MINUTE / std::f32::consts::TAU,
                    pistons.revolutions().abs(),
                ),
                TEXT,
            ),
            (
                format!(
                    "Flywheel energy: {:.1} J",
                    engine.energy_joules(pistons.kinetic_energy())
                ),
                TEXT,
            ),
        ];
        if std::mem::take(&mut self.limited_since_refresh) {
            self.now_lines.push((LIMITED_TEXT.to_owned(), WARNING));
        }
        self.detail_lines.clear();
        if self.show_details {
            let heat = engine.heat();
            self.detail_lines = vec![
                format!(
                    "Helium: {} particles | {:.4} mol",
                    engine.particles.len(),
                    engine.total_moles()
                ),
                format!(
                    "Particles: hot {} | cold {} | passage {}",
                    regions.hot.markers, regions.cold.markers, regions.passage_markers
                ),
                format!(
                    "Concentration: hot {:.2} mol/L | cold {:.2} mol/L",
                    regions.hot.concentration_mol_per_l, regions.cold.concentration_mol_per_l
                ),
                format!(
                    "Heat into gas: hot {:+.1} J | cold {:+.1} J",
                    engine.energy_joules(heat.hot),
                    engine.energy_joules(heat.cold)
                ),
                format!(
                    "Gas work: {:+.1} J | kicks: {:+.1} J",
                    engine.energy_joules(engine.piston_work_by_gas()),
                    engine.energy_joules(pistons.starter_work())
                ),
                format!(
                    "Balance error: gas {:+.4} J | shaft {:+.4} J",
                    engine.energy_joules(engine.energy_balance_error()),
                    engine.energy_joules(engine.mechanical_balance_error())
                ),
                format!("Display: {fps} fps"),
            ];
        }

        let cycle = engine.cycle();
        self.turn = cycle.recent().map(|recent| turn_readout(engine, recent));
        self.waiting = if pistons.angular_velocity() == 0.0 && cycle.completed_turns() == 0 {
            WAITING_START
        } else {
            WAITING_TURN
        };
        let [hot_work, cold_work] =
            [HOT, COLD].map(|cylinder| engine.energy_joules(loop_work(cycle.last_loop(cylinder))));
        self.loop_text = if cycle.last_loop(HOT).is_empty() {
            "Dots: each cylinder's state now".to_owned()
        } else {
            format!(
                "Hot {hot_work:+.1} J + cold {cold_work:+.1} J = W {:+.1} J",
                hot_work + cold_work
            )
        };
        self.range = plot_range(engine);
    }

    /// Draws the column at the left edge, full height.
    pub fn draw(&self, engine: &ConnectedChambers, paused: bool) {
        self.draw_with_height(&mut Pen::screen(), engine, paused, screen_height());
    }

    /// Draws the column for a window `height` pixels tall and returns the
    /// side of the P-V plot drawn, or zero if there was no room for it.
    fn draw_with_height(
        &self,
        pen: &mut Pen,
        engine: &ConnectedChambers,
        paused: bool,
        height: f32,
    ) -> f32 {
        pen.rect(0.0, 0.0, COLUMN_WIDTH, height, PANEL);
        let x = MARGIN;
        let mut y = TITLE_BASELINE;
        pen.fitted(TITLE, x, y, CONTENT_WIDTH, TITLE_SIZE, TEXT);
        let status = if paused { "Paused" } else { "Running" };
        let status_width = pen.measure(status, BODY_SIZE);
        pen.text(
            status,
            x + CONTENT_WIDTH - status_width,
            y,
            BODY_SIZE,
            if paused { WARNING } else { QUIET },
        );

        y += SECTION_GAP + LINE;
        pen.fitted(SECTION_NOW, x, y, CONTENT_WIDTH, SECTION_SIZE, SECTION);
        for (text, color) in &self.now_lines {
            y += LINE;
            pen.fitted(text, x, y, CONTENT_WIDTH, BODY_SIZE, *color);
        }
        for text in &self.detail_lines {
            y += SMALL_LINE;
            pen.fitted(text, x, y, CONTENT_WIDTH, SMALL_SIZE, QUIET);
        }

        y += SECTION_GAP + LINE;
        let Some(turn) = &self.turn else {
            pen.fitted("LAST FULL TURN", x, y, CONTENT_WIDTH, SECTION_SIZE, SECTION);
            for text in [self.waiting, WAITING_NOTE] {
                y += LINE;
                pen.fitted(text, x, y, CONTENT_WIDTH, BODY_SIZE, QUIET);
            }
            y += READOUT_GAP;
            return self.draw_loops(pen, engine, x, y, height);
        };
        pen.fitted(&turn.heading, x, y, CONTENT_WIDTH, SECTION_SIZE, SECTION);

        y = draw_ledger(pen, turn, x, y + LINE);
        y = draw_efficiency(pen, turn, x, y + READOUT_GAP);
        y = draw_entropy(pen, turn, x, y + READOUT_GAP);
        self.draw_loops(pen, engine, x, y + READOUT_GAP, height)
    }

    /// Draws the P-V plot below `top`, as large as fits above `bottom`, and
    /// returns its side, or zero if there was no room or no data yet.
    fn draw_loops(
        &self,
        pen: &mut Pen,
        engine: &ConnectedChambers,
        x: f32,
        top: f32,
        bottom: f32,
    ) -> f32 {
        let mut y = top + LINE;
        pen.fitted(LAW_WORK, x, y, CONTENT_WIDTH, LAW_SIZE, LAW);
        y += SMALL_LINE;
        pen.fitted(&self.loop_text, x, y, CONTENT_WIDTH, SMALL_SIZE, QUIET);
        let side = (CONTENT_WIDTH - PLOT_LEFT).min(bottom - y - MARGIN - PLOT_BOTTOM);
        if side < MIN_PLOT_SIDE {
            pen.fitted(
                PLOT_TOO_SMALL,
                x,
                y + LINE,
                CONTENT_WIDTH,
                SMALL_SIZE,
                QUIET,
            );
            return 0.0;
        }
        let Some(range) = self.range else {
            return 0.0;
        };
        let plot = Rect::new(x + PLOT_LEFT, y + READOUT_GAP, side, side);
        pen.rect(plot.x, plot.y, plot.w, plot.h, TRACK);
        draw_axis_labels(pen, plot, range);
        let to_screen = |point: LoopPoint| {
            let (volume, pressure) = point_units(engine, point);
            vec2(
                plot.x + plot.w * fraction(volume, range.volume),
                plot.bottom() - plot.h * fraction(pressure, range.pressure),
            )
        };
        let cycle = engine.cycle();
        for (cylinder, color) in [(HOT, HOT_COLOR), (COLD, COLD_COLOR)] {
            let points: Vec<Vec2> = cycle
                .last_loop(cylinder)
                .iter()
                .map(|&p| to_screen(p))
                .collect();
            draw_closed_loop(pen, &points, color);
            let trace: Vec<Vec2> = cycle.current_trace(cylinder).map(to_screen).collect();
            let faint = Color {
                a: TRACE_ALPHA,
                ..color
            };
            for pair in trace.windows(2) {
                pen.line(
                    pair[0].x,
                    pair[0].y,
                    pair[1].x,
                    pair[1].y,
                    TRACE_THICKNESS,
                    faint,
                );
            }
            if let Some(now) = trace.last() {
                pen.circle(now.x, now.y, DOT_RADIUS, color);
            }
        }
        side
    }
}

/// One "Engine now" line: temperature and ideal-gas pressure estimate.
fn gas_line(name: &str, sample: GasMeasurements) -> String {
    let temperature = sample
        .temperature_kelvin
        .map_or_else(|| "--".to_owned(), |kelvin| format!("{kelvin:.0} K"));
    let pressure = sample.pressure_pa.map_or_else(
        || "--".to_owned(),
        |pa| format!("~{:.1} bar", pa / PASCALS_PER_BAR),
    );
    format!("{name}: {temperature} | {pressure}")
}

/// Formats the recent turns, averaged per turn.
fn turn_readout(engine: &ConnectedChambers, recent: TurnSummary) -> TurnReadout {
    let turns = recent.turns;
    let turn = recent.per_turn();
    let joules = |reduced: f64| engine.energy_joules(reduced);
    let (hot, cold, work, stored) = (
        joules(turn.hot_heat),
        joules(turn.cold_heat),
        joules(turn.gas_work),
        joules(turn.gas_energy_change),
    );
    let mut inflow = Vec::new();
    let mut outflow = Vec::new();
    // Each term goes on the side it flows: a positive heat is into the gas,
    // positive work and stored energy are out of the heat supplied.
    for (amount, color, into_gas) in [
        (hot, HOT_COLOR, true),
        (cold, COLD_COLOR, true),
        (work, WORK_COLOR, false),
        (stored, STORED_COLOR, false),
    ] {
        let side = if (amount > 0.0) == into_gas {
            &mut inflow
        } else {
            &mut outflow
        };
        if amount != 0.0 {
            side.push((amount.abs(), color));
        }
    }

    let heading = if turns == 1 {
        format!("LAST FULL TURN ({:.0} s)", turn.duration)
    } else {
        format!(
            "AVERAGE OF LAST {turns} TURNS ({:.0} s each)",
            turn.duration
        )
    };
    let carnot = turn.carnot_limit();
    let efficiency = turn.efficiency().filter(|_| turn.gas_work > 0.0);
    let efficiency_text = if carnot == 0.0 {
        (
            "Efficiency: none without a temperature difference".to_owned(),
            QUIET,
        )
    } else if let Some(value) = efficiency {
        (
            format!("Efficiency W / Q hot = {:.1}%", 100.0 * value),
            TEXT,
        )
    } else {
        ("Efficiency 0%: no net work out this turn".to_owned(), QUIET)
    };
    let carnot_text = format!(
        "Carnot limit 1 - {:.0}/{:.0} = {:.1}%",
        turn.cold_kelvin,
        turn.hot_kelvin,
        100.0 * carnot
    );

    let entropy = engine.entropy_joules_per_kelvin(turn.surroundings_entropy_change());
    let entropy_value = (
        format!("= {entropy:+.3} J/K per turn"),
        if entropy > 0.0 { GOOD } else { WARNING },
    );
    let entropy_note = if !turn.is_settled() {
        (
            format!("Not settled (dU {stored:+.0} J): dS gas not counted"),
            QUIET,
        )
    } else if entropy > 0.0 {
        (
            "Gas back to start, so dS universe = dS surr".to_owned(),
            QUIET,
        )
    } else {
        ("Unexpected: dS surr is not positive".to_owned(), WARNING)
    };

    TurnReadout {
        heading,
        inflow,
        outflow,
        ledger_values: format!(
            "{} = {} J",
            signed_sum(&[hot, cold]),
            signed_sum(&[work, stored])
        ),
        ledger_warning: {
            let residual = joules(turn.first_law_residual());
            (residual.abs() > LEDGER_TOLERANCE_JOULES)
                .then(|| format!("Unexpected: off by {residual:+.2} J"))
        },
        efficiency: efficiency.unwrap_or(0.0).clamp(0.0, 1.0),
        carnot,
        efficiency_text,
        carnot_text,
        entropy_value,
        entropy_note,
    }
}

/// Writes terms as a sum with explicit signs, e.g. "2347.1 - 2284.8".
fn signed_sum(terms: &[f64]) -> String {
    let mut text = String::new();
    for (index, &term) in terms.iter().enumerate() {
        match (index, term < 0.0) {
            (0, false) => text += &format!("{term:.1}"),
            (0, true) => text += &format!("-{:.1}", -term),
            (_, false) => text += &format!(" + {term:.1}"),
            (_, true) => text += &format!(" - {:.1}", -term),
        }
    }
    text
}

fn draw_ledger(pen: &mut Pen, turn: &TurnReadout, x: f32, top: f32) -> f32 {
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

fn draw_efficiency(pen: &mut Pen, turn: &TurnReadout, x: f32, top: f32) -> f32 {
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
        LOOP_THICKNESS,
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

fn draw_entropy(pen: &mut Pen, turn: &TurnReadout, x: f32, top: f32) -> f32 {
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

fn sum(segments: &[(f64, Color)]) -> f64 {
    segments.iter().map(|&(amount, _)| amount).sum()
}

/// A loop point as (volume in cm3, pressure in bar).
fn point_units(engine: &ConnectedChambers, point: LoopPoint) -> (f64, f64) {
    (
        engine.volume_cm3(point.volume),
        engine.pressure_pa(point.pressure) / PASCALS_PER_BAR,
    )
}

/// Position of `value` within `range`, clamped to 0..=1.
fn fraction(value: f64, range: (f64, f64)) -> f32 {
    ((value - range.0) / (range.1 - range.0)).clamp(0.0, 1.0) as f32
}

/// Volume spans one cylinder's full stroke. Pressure spans the last loops
/// and the turn in progress, rounded out to `PRESSURE_STEP_BAR`.
fn plot_range(engine: &ConnectedChambers) -> Option<PlotRange> {
    let cycle = engine.cycle();
    let mut low = f64::INFINITY;
    let mut high = f64::NEG_INFINITY;
    for cylinder in [HOT, COLD] {
        let points = cycle
            .last_loop(cylinder)
            .iter()
            .copied()
            .chain(cycle.current_trace(cylinder));
        for point in points {
            let (_, bar) = point_units(engine, point);
            low = low.min(bar);
            high = high.max(bar);
        }
    }
    if !(low.is_finite() && high.is_finite()) {
        return None;
    }
    let low = (low / PRESSURE_STEP_BAR).floor() * PRESSURE_STEP_BAR;
    let high = ((high / PRESSURE_STEP_BAR).ceil() * PRESSURE_STEP_BAR).max(low + PRESSURE_STEP_BAR);
    Some(PlotRange {
        volume: engine.cylinder_volume_range_cm3(),
        pressure: (low.max(0.0), high),
    })
}

fn draw_axis_labels(pen: &mut Pen, plot: Rect, range: PlotRange) {
    let left = plot.x - PLOT_LEFT;
    let below = plot.bottom() + SMALL_LINE;
    let low_volume = format!("{:.0}", range.volume.0);
    let high_volume = format!("{:.0}", range.volume.1);
    let unit = "V (cm3)";
    let labels = [
        (
            format!("{:.0}", range.pressure.1),
            left,
            plot.y + SMALL_SIZE,
        ),
        (format!("{:.0}", range.pressure.0), left, plot.bottom()),
        ("bar".to_owned(), left, plot.y + plot.h / 2.0),
        (
            unit.to_owned(),
            plot.x + (plot.w - pen.measure(unit, SMALL_SIZE)) / 2.0,
            below,
        ),
        (
            high_volume.clone(),
            plot.right() - pen.measure(&high_volume, SMALL_SIZE),
            below,
        ),
        (low_volume, plot.x, below),
    ];
    for (text, x, y) in labels {
        pen.text(&text, x, y, SMALL_SIZE, QUIET);
    }
    let mut below = below;
    for (text, color) in LOOP_KEY {
        below += SMALL_LINE;
        pen.fitted(
            text,
            plot.x - PLOT_LEFT,
            below,
            CONTENT_WIDTH,
            SMALL_SIZE,
            color,
        );
    }
}

/// A closed loop, lightly filled, with a solid outline.
fn draw_closed_loop(pen: &mut Pen, points: &[Vec2], color: Color) {
    if points.len() < 3 {
        return;
    }
    fill_polygon(
        pen,
        points,
        Color {
            a: LOOP_FILL_ALPHA,
            ..color
        },
    );
    for (&a, &b) in points.iter().zip(points.iter().cycle().skip(1)) {
        pen.line(a.x, a.y, b.x, b.y, LOOP_THICKNESS, color);
    }
}

/// Fills a closed polygon one pixel column at a time with the even-odd
/// rule, which is correct for concave and self-crossing outlines alike.
fn fill_polygon(pen: &mut Pen, points: &[Vec2], color: Color) {
    let (left, right) = points
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p.x), hi.max(p.x))
        });
    let mut crossings = Vec::with_capacity(points.len());
    let mut x = left.floor();
    while x < right {
        let center = x + 0.5;
        crossings.clear();
        for (&a, &b) in points.iter().zip(points.iter().cycle().skip(1)) {
            // Half-open test, so a shared vertex is counted once.
            if (a.x <= center) != (b.x <= center) {
                crossings.push(a.y + (center - a.x) / (b.x - a.x) * (b.y - a.y));
            }
        }
        crossings.sort_by(f32::total_cmp);
        for &[top, bottom] in crossings.as_chunks::<2>().0 {
            pen.rect(x, top, 1.0, bottom - top, color);
        }
        x += 1.0;
    }
}

/// Draws to the screen or, in tests without a window, only records the
/// text it would have drawn, so layout and wording can be checked. Real
/// Macroquad cannot measure or draw anything without a window.
pub(crate) struct Pen {
    live: bool,
    /// Text recorded while not live.
    texts: Vec<String>,
}

/// Approximate character width, in em, used when no font is available.
const OFFLINE_CHAR_WIDTH_EM: f32 = 0.55;

impl Pen {
    fn screen() -> Self {
        Self {
            live: true,
            texts: Vec::new(),
        }
    }

    #[cfg(test)]
    fn offline() -> Self {
        Self {
            live: false,
            texts: Vec::new(),
        }
    }

    fn measure(&self, text: &str, size: f32) -> f32 {
        if self.live {
            measure_text(text, None, size as u16, 1.0).width
        } else {
            text.len() as f32 * size * OFFLINE_CHAR_WIDTH_EM
        }
    }

    fn text(&mut self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        if self.live {
            draw_text(text, x, y, size, color);
        } else {
            self.texts.push(text.to_owned());
        }
    }

    /// Draws text left-aligned at `x`, shrinking it only if it would exceed
    /// `width`.
    fn fitted(&mut self, text: &str, x: f32, y: f32, width: f32, size: f32, color: Color) {
        let measured = self.measure(text, size);
        self.text(
            text,
            x,
            y,
            size * (width / measured.max(1.0)).min(1.0),
            color,
        );
    }

    fn rect(&self, x: f32, y: f32, width: f32, height: f32, color: Color) {
        if self.live {
            draw_rectangle(x, y, width, height, color);
        }
    }

    fn line(&self, x1: f32, y1: f32, x2: f32, y2: f32, thickness: f32, color: Color) {
        if self.live {
            draw_line(x1, y1, x2, y2, thickness, color);
        }
    }

    fn circle(&self, x: f32, y: f32, radius: f32, color: Color) {
        if self.live {
            draw_circle(x, y, radius, color);
        }
    }
}

/// Draws text on screen left-aligned at `x`, shrinking it only if it would
/// exceed `width`.
pub fn fitted_text(text: &str, x: f32, y: f32, width: f32, size: f32, color: Color) {
    Pen::screen().fitted(text, x, y, width, size, color);
}

#[cfg(test)]
#[path = "../tests/unit/panel.rs"]
mod tests;
