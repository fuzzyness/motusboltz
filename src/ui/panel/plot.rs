//! The pressure-volume plot: one loop per cylinder for the last full turn,
//! a faint trace of the turn in progress, and a dot for each cylinder now.

use macroquad::prelude::{
    Color,
    Rect,
    Vec2,
    vec2, //
};

use super::readout::PASCALS_PER_BAR;
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
/// Thickness of a finished loop's outline.
const LOOP_THICKNESS: f32 = 2.0;
/// Thickness of the trace of the turn in progress.
const TRACE_THICKNESS: f32 = 1.0;
/// Radius of the dot for each cylinder now.
const DOT_RADIUS: f32 = 4.0;
/// Opacity of a loop's fill.
const LOOP_FILL_ALPHA: f32 = 0.18;
/// Opacity of the trace.
const TRACE_ALPHA: f32 = 0.45;
/// Pressure axis limits are rounded out to multiples of this many bar.
const PRESSURE_STEP_BAR: f64 = 10.0;
/// Label of the volume axis.
const VOLUME_UNIT: &str = "V (cm³)";
/// Label of the pressure axis.
const PRESSURE_UNIT: &str = "bar";
/// The key under the plot, one line per loop.
pub const LOOP_KEY: [(&str, Color); 2] = [
    ("Hot loop: gas does work on the piston (+)", HOT_COLOR),
    ("Cold loop: piston does work on the gas (−)", COLD_COLOR),
];

/// Axis limits for the P-V plot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotRange {
    /// Volume, in cm³.
    pub volume: (f64, f64),
    /// Pressure, in bar.
    pub pressure: (f64, f64),
}

/// The plot's axis limits, or `None` before the crank has swept a single
/// bin.
///
/// Volume spans one cylinder's full stroke. Pressure spans the last loops
/// and the turn in progress, rounded out to `PRESSURE_STEP_BAR`.
pub fn plot_range(engine: &ConnectedChambers) -> Option<PlotRange> {
    let cycle = engine.cycle();
    let mut low = f64::INFINITY;
    let mut high = f64::NEG_INFINITY;
    for cylinder in [HOT, COLD] {
        let last = cycle.last_loop(cylinder).iter().copied();
        for point in last.chain(cycle.current_trace(cylinder)) {
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
    pen.rect(plot, TRACK);
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
            pen.line(pair[0], pair[1], TRACE_THICKNESS, faint);
        }
        if let Some(&now) = trace.last() {
            pen.circle(now, DOT_RADIUS, color);
        }
    }
}

/// A loop point as (volume in cm³, pressure in bar).
fn point_units(engine: &ConnectedChambers, point: LoopPoint) -> (f64, f64) {
    let bar = engine.units().pressure_pa(point.pressure) / PASCALS_PER_BAR;
    (volume_cm3(point.volume), bar)
}

/// Position of `value` within `range`, clamped to 0..=1.
fn fraction(value: f64, range: (f64, f64)) -> f32 {
    ((value - range.0) / (range.1 - range.0)).clamp(0.0, 1.0) as f32
}

/// Draws the axis limits and units, and the loop key under them.
fn draw_axis_labels(pen: &mut Pen, plot: Rect, range: PlotRange) {
    let left = plot.x - PLOT_LEFT;
    let below = plot.bottom() + SMALL_LINE;
    let low_volume = format!("{:.0}", range.volume.0);
    let high_volume = format!("{:.0}", range.volume.1);
    let volume_unit_left = plot.x + (plot.w - pen.measure(VOLUME_UNIT, SMALL_SIZE)) / 2.0;
    let high_volume_left = plot.right() - pen.measure(&high_volume, SMALL_SIZE);
    let labels = [
        (
            format!("{:.0}", range.pressure.1),
            left,
            plot.y + SMALL_SIZE,
        ),
        (format!("{:.0}", range.pressure.0), left, plot.bottom()),
        (PRESSURE_UNIT.to_owned(), left, plot.y + plot.h / 2.0),
        (VOLUME_UNIT.to_owned(), volume_unit_left, below),
        (high_volume, high_volume_left, below),
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

/// Draws a closed loop, lightly filled, with a solid outline.
fn draw_closed_loop(pen: &mut Pen, points: &[Vec2], color: Color) {
    if points.len() < 3 {
        return;
    }
    let fill = Color {
        a: LOOP_FILL_ALPHA,
        ..color
    };
    fill_polygon(pen, points, fill);
    for (&a, &b) in points.iter().zip(points.iter().cycle().skip(1)) {
        pen.line(a, b, LOOP_THICKNESS, color);
    }
}

/// Fills a closed polygon one pixel column at a time.
///
/// The even-odd rule is correct for concave and self-crossing outlines
/// alike.
fn fill_polygon(pen: &mut Pen, points: &[Vec2], color: Color) {
    let (left, right) = points
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(low, high), point| {
            (low.min(point.x), high.max(point.x))
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
            pen.rect(Rect::new(x, top, 1.0, bottom - top), color);
        }
        x += 1.0;
    }
}
