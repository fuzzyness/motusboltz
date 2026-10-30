//! The pressure-volume plot: one loop per cylinder for the last full turn,
//! a faint trace of the turn in progress, and a dot for each cylinder now.

use macroquad::prelude::{
    Color,
    Rect,
    Vec2,
    vec2, //
};

use super::{
    COLD_COLOR,
    CONTENT_WIDTH,
    HOT_COLOR,
    QUIET,
    SMALL_LINE,
    SMALL_SIZE,
    TRACK, //
};
use crate::sim::cycle::{
    COLD,
    HOT,
    LoopPoint, //
};
use crate::sim::engine::ConnectedChambers;
use crate::sim::units::volume_cm3;
use crate::ui::pen::Pen;

/// Room left of the plot for pressure labels.
pub const PLOT_LEFT: f32 = 34.0;
/// Room below the plot for the volume labels and the two-line loop key.
pub const PLOT_BOTTOM: f32 = 3.0 * SMALL_LINE + 4.0;
/// Widest plot that fits the panel.
pub const MAX_PLOT_SIDE: f32 = CONTENT_WIDTH - PLOT_LEFT;
const LOOP_THICKNESS: f32 = 2.0;
const TRACE_THICKNESS: f32 = 1.0;
const DOT_RADIUS: f32 = 4.0;
const LOOP_FILL_ALPHA: f32 = 0.18;
const TRACE_ALPHA: f32 = 0.45;
/// Pressure axis limits are rounded out to multiples of this many bar.
const PRESSURE_STEP_BAR: f64 = 10.0;
const PASCALS_PER_BAR: f64 = 100_000.0;
const VOLUME_UNIT: &str = "V (cm³)";
const PRESSURE_UNIT: &str = "bar";
pub const LOOP_KEY: [(&str, Color); 2] = [
    ("Hot loop: gas does work on the piston (+)", HOT_COLOR),
    ("Cold loop: piston does work on the gas (−)", COLD_COLOR),
];

/// Axis limits for the P-V plot: volume in cm³ and pressure in bar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotRange {
    pub volume: (f64, f64),
    pub pressure: (f64, f64),
}

/// Volume spans one cylinder's full stroke. Pressure spans the last loops
/// and the turn in progress, rounded out to `PRESSURE_STEP_BAR`. `None`
/// before the crank has swept a single bin.
pub fn plot_range(engine: &ConnectedChambers) -> Option<PlotRange> {
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

/// Draws the plot in `plot`, with axis labels to its left and below it.
pub fn draw_plot(pen: &mut Pen, engine: &ConnectedChambers, plot: Rect, range: PlotRange) {
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
}

/// A loop point as (volume in cm3, pressure in bar).
fn point_units(engine: &ConnectedChambers, point: LoopPoint) -> (f64, f64) {
    (
        volume_cm3(point.volume),
        engine.pressure_pa(point.pressure) / PASCALS_PER_BAR,
    )
}

/// Position of `value` within `range`, clamped to 0..=1.
fn fraction(value: f64, range: (f64, f64)) -> f32 {
    ((value - range.0) / (range.1 - range.0)).clamp(0.0, 1.0) as f32
}

fn draw_axis_labels(pen: &mut Pen, plot: Rect, range: PlotRange) {
    let left = plot.x - PLOT_LEFT;
    let below = plot.bottom() + SMALL_LINE;
    let low_volume = format!("{:.0}", range.volume.0);
    let high_volume = format!("{:.0}", range.volume.1);
    let labels = [
        (
            format!("{:.0}", range.pressure.1),
            left,
            plot.y + SMALL_SIZE,
        ),
        (format!("{:.0}", range.pressure.0), left, plot.bottom()),
        (PRESSURE_UNIT.to_owned(), left, plot.y + plot.h / 2.0),
        (
            VOLUME_UNIT.to_owned(),
            plot.x + (plot.w - pen.measure(VOLUME_UNIT, SMALL_SIZE)) / 2.0,
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
        pen.fitted(text, left, below, CONTENT_WIDTH, SMALL_SIZE, color);
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
