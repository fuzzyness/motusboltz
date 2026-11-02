//! The readings panel, in two views, on a rounded, see-through card at the
//! window's left (see `drawer`).
//!
//! Simple shows what the project is about in
//! plain words: where the energy goes each turn and what each law says
//! about it. Advanced shows every reading, with the pressure-volume loops.
//! The card scrolls, by wheel or by dragging its scrollbar, whenever its
//! content is taller than the window. Its text has a soft shadow, so it
//! reads over whatever shows through the card.
//!
//! The readings are rebuilt at most `REFRESH_SECONDS` apart, not every
//! frame, and the app repaints the panel only when what it shows changes
//! (see `needs_paint`).

mod advanced;
mod plot;
pub mod readout;
mod simple;

use macroquad::prelude::{
    Color,
    Rect,
    Vec2, //
};

use crate::sim::engine::ConnectedChambers;
use crate::ui::button::{
    self,
    Hue, //
};
use crate::ui::layout::{
    PANEL_INSET,
    PANEL_LEFT,
    PANEL_WIDTH, //
};
use crate::ui::pen::{
    Pen,
    Weight,
    rounded_outline, //
};
use plot::{
    MAX_PLOT_SIDE,
    PLOT_BOTTOM,
    PLOT_LEFT,
    PlotRange,
    draw_plot,
    plot_range, //
};
use readout::{
    Readout,
    readout, //
};

/// Heading at the top of the panel.
pub const TITLE: &str = "MotusBoltz";

/// Room inside the card around its content.
const MARGIN: f32 = 16.0;
/// The content's left edge.
const LEFT: f32 = PANEL_LEFT + MARGIN;
/// Width of the content.
const CONTENT_WIDTH: f32 = PANEL_WIDTH - PANEL_LEFT - 2.0 * MARGIN;
/// Room kept under the content at the card's bottom, clear of its rounded
/// corners.
const BOTTOM_ROOM: f32 = 6.0;
/// Room under the content's last line when scrolled to the end.
const END_ROOM: f32 = 10.0;
/// Seconds of wall time between text refreshes.
const REFRESH_SECONDS: f64 = 0.1;

/// Baseline of the title.
///
/// The header, the title above the Simple and Advanced switch, stays put
/// while the content below it scrolls.
const TITLE_BASELINE: f32 = PANEL_INSET + 32.0;
/// Top of the view switch.
const TAB_TOP: f32 = PANEL_INSET + 44.0;
/// Height of the view switch.
const TAB_HEIGHT: f32 = 26.0;
/// Gap between the switch's halves.
const TAB_GAP: f32 = 6.0;
/// Height of the header, from the window's top.
const HEADER_HEIGHT: f32 = TAB_TOP + TAB_HEIGHT + 4.0;

/// Width of the scrollbar, in the card's right margin.
const SCROLLBAR_WIDTH: f32 = 5.0;
/// Gap between the scrollbar and the card's right edge.
const SCROLLBAR_INSET: f32 = 6.0;
/// Shortest scrollbar thumb.
const MIN_THUMB_HEIGHT: f32 = 24.0;

/// Text size of the title.
const TITLE_SIZE: f32 = 28.0;
/// Text size of section headings.
const SECTION_SIZE: f32 = 13.0;
/// Text size of a law in words.
const LAW_SIZE: f32 = 15.0;
/// Text size of running text.
const BODY_SIZE: f32 = 15.0;
/// Text size of notes.
const SMALL_SIZE: f32 = 13.0;
/// Text size of the view switch.
const TAB_SIZE: f32 = 15.0;
/// Distance between baselines.
const LINE: f32 = 19.0;
/// Distance between baselines of small text.
const SMALL_LINE: f32 = 16.0;
/// Space between sections.
const SECTION_GAP: f32 = 14.0;
/// Gap between the loop work and the plot.
const READOUT_GAP: f32 = 10.0;
/// Smallest P-V plot in the Advanced view.
///
/// On a short window the column scrolls to reach it rather than shrinking
/// it further.
const COMFORTABLE_PLOT_SIDE: f32 = 200.0;

/// The soft shadow under the panel's light text.
const TEXT_SHADOW: Color = Color::new(0.02, 0.04, 0.1, 0.85);
/// Running text.
pub const TEXT: Color = Color::from_rgba(215, 226, 238, 255);
/// Labels and notes.
pub const QUIET: Color = Color::from_rgba(176, 190, 208, 255);
/// Section headings.
const SECTION: Color = Color::from_rgba(150, 172, 198, 255);
/// The laws.
const LAW: Color = Color::from_rgba(180, 224, 209, 255);
/// Warnings and the status.
const WARNING: Color = Color::from_rgba(255, 130, 110, 255);
/// Results as expected.
const GOOD: Color = Color::from_rgba(140, 220, 150, 255);
/// The empty part of a bar, and the plot's background.
pub const TRACK: Color = Color::new(0.7, 0.8, 0.95, 0.14);
/// Heat in, and the hot side.
pub const HOT_COLOR: Color = Color::from_rgba(255, 160, 90, 255);
/// Heat out, and the cold side.
pub const COLD_COLOR: Color = Color::from_rgba(95, 190, 255, 255);
/// Work, and the flywheel.
pub const WORK_COLOR: Color = Color::from_rgba(255, 218, 110, 255);
/// Tint of neutral tiles.
const PEARL: Color = Color::from_rgba(186, 200, 222, 255);
/// The scrollbar's track.
const SCROLL_TRACK_COLOR: Color = Color::new(0.75, 0.85, 1.0, 0.12);
/// The scrollbar's thumb.
const SCROLL_THUMB_COLOR: Color = Color::new(0.75, 0.85, 1.0, 0.55);
/// How much of a glass tile's tint goes into its fill (see `tile`).
const TILE_TINT: f32 = 0.24;
/// Opacity of a tile's fill.
const TILE_OPACITY: f32 = 0.55;
/// Opacity of a tile's rim.
const TILE_RIM_OPACITY: f32 = 0.45;
/// Width of a tile's rim.
const TILE_RIM_WIDTH: f32 = 1.2;
/// The sheen across a tile's top half.
const TILE_SHEEN: Color = Color::new(1.0, 1.0, 1.0, 0.05);
/// A tile's fill before the tint.
const TILE_BASE: Color = Color::new(0.05, 0.08, 0.14, 1.0);

/// Heading of the loops.
const LOOPS_HEADING: &str = "P-V LOOPS (LAST TURN)";
/// What the loops show.
const LAW_WORK: &str = "Work: the area inside each loop";

/// Everything that changes how the panel looks.
///
/// The app keeps the painted panel as an image and repaints it only when
/// this changes.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Appearance {
    /// Text rebuilds so far.
    refreshes: u64,
    /// The view.
    mode: PanelMode,
    /// How far the content is scrolled.
    scroll: f32,
    /// The status beside the title.
    status: Option<&'static str>,
    /// Height of the window.
    height: f32,
}

/// The panel's two views.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelMode {
    /// What the project asks about, in plain words.
    Simple,
    /// Every reading, with the P-V loops.
    Advanced,
}

/// The views, in the switch's order.
const MODES: [PanelMode; 2] = [PanelMode::Simple, PanelMode::Advanced];

impl PanelMode {
    /// The view's name on the switch.
    fn label(self) -> &'static str {
        match self {
            Self::Simple => "Simple",
            Self::Advanced => "Advanced",
        }
    }
}

/// The switch's half for `index`, in the header row.
fn tab_rect(index: usize) -> Rect {
    let width = (CONTENT_WIDTH - TAB_GAP * (MODES.len() - 1) as f32) / MODES.len() as f32;
    Rect::new(
        LEFT + index as f32 * (width + TAB_GAP),
        TAB_TOP,
        width,
        TAB_HEIGHT,
    )
}

/// Draws a glass tile inside the card, with corners of `radius`.
///
/// The tile is a dark fill tinted toward `tint`, a thin rim in `tint`, and
/// a faint sheen across its top half.
fn tile(pen: &Pen, rect: Rect, radius: f32, tint: Color) {
    let mix = |base: f32, tint: f32| base + (tint - base) * TILE_TINT;
    let (r, g, b) = (
        mix(TILE_BASE.r, tint.r),
        mix(TILE_BASE.g, tint.g),
        mix(TILE_BASE.b, tint.b),
    );
    let outline = rounded_outline(rect, [radius; 4]);
    pen.fan(rect.center(), &outline, Color::new(r, g, b, TILE_OPACITY));
    let rim = Color {
        a: TILE_RIM_OPACITY,
        ..tint
    };
    pen.ring(&outline, TILE_RIM_WIDTH, rim);
    let sheen = Rect::new(rect.x, rect.y, rect.w, rect.h / 2.0);
    let top = radius.min(sheen.h);
    let sheen_outline = rounded_outline(sheen, [top, top, 0.0, 0.0]);
    pen.fan(sheen.center(), &sheen_outline, TILE_SHEEN);
}

/// Pointer state for one frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct PanelPointer {
    /// Where the pointer is.
    pub position: Vec2,
    /// Wheel scroll this frame, in pixels; positive scrolls up.
    pub wheel_pixels: f32,
    /// The left button went down this frame.
    pub pressed: bool,
    /// The left button is held.
    pub down: bool,
}

/// The readings panel.
pub struct EnginePanel {
    /// The numbers shown.
    readout: Readout,
    /// The P-V plot's axis limits, once there is something to plot.
    range: Option<PlotRange>,
    /// Wall time of the next text rebuild.
    next_refresh: f64,
    /// Text rebuilds so far, so a repaint follows each one.
    refreshes: u64,
    /// How the panel looked when last painted.
    painted: Option<Appearance>,
    /// The view.
    mode: PanelMode,
    /// How far the content is scrolled up, in pixels.
    scroll: f32,
    /// Content height from the last draw, for scrolling.
    content_height: f32,
    /// Visible height below the header from the last draw.
    viewport_height: f32,
    /// Where the scrollbar thumb was grabbed, relative to its top, while it
    /// is being dragged.
    thumb_grab: Option<f32>,
    /// A press that started on the panel is still held.
    gesture: bool,
}

impl EnginePanel {
    /// A panel in the Simple view, waiting for Start.
    pub fn new() -> Self {
        Self {
            readout: Readout {
                waiting: readout::WAITING_START,
                ..Default::default()
            },
            range: None,
            next_refresh: 0.0,
            refreshes: 0,
            painted: None,
            mode: PanelMode::Simple,
            scroll: 0.0,
            content_height: 0.0,
            viewport_height: 0.0,
            thumb_grab: None,
            gesture: false,
        }
    }

    /// Switches between the Simple and Advanced views.
    pub fn toggle_mode(&mut self) {
        self.select(match self.mode {
            PanelMode::Simple => PanelMode::Advanced,
            PanelMode::Advanced => PanelMode::Simple,
        });
    }

    /// Whether the Advanced view shows.
    pub fn advanced(&self) -> bool {
        self.mode == PanelMode::Advanced
    }

    /// Shows a view, scrolled to its top.
    fn select(&mut self, mode: PanelMode) {
        self.mode = mode;
        self.scroll = 0.0;
        self.next_refresh = 0.0;
    }

    /// Rebuilds the text if `REFRESH_SECONDS` have passed since the last
    /// rebuild, at wall time `now`.
    pub fn update(&mut self, engine: &ConnectedChambers, now: f64) {
        if now < self.next_refresh {
            return;
        }
        self.next_refresh = now + REFRESH_SECONDS;
        self.refresh(engine);
    }

    /// Rebuilds the text from `engine` now.
    fn refresh(&mut self, engine: &ConnectedChambers) {
        self.refreshes += 1;
        self.readout = readout(engine);
        self.range = plot_range(engine);
    }

    /// Handles clicks on the view switch, the wheel and the scrollbar.
    ///
    /// Returns whether the camera should ignore the pointer: it is over the
    /// panel, or a press that started here is still held.
    pub fn handle_pointer(&mut self, pointer: PanelPointer) -> bool {
        let over = (0.0..=PANEL_WIDTH).contains(&pointer.position.x)
            && pointer.position.y <= HEADER_HEIGHT + self.viewport_height + BOTTOM_ROOM;
        if !pointer.down {
            self.thumb_grab = None;
            self.gesture = false;
        }
        if pointer.pressed && over {
            self.gesture = true;
            self.press(pointer.position);
        }
        if let (Some(grab), true) = (self.thumb_grab, pointer.down) {
            self.drag_thumb(pointer.position.y - grab);
        }
        if over && pointer.wheel_pixels != 0.0 {
            self.scroll_to(self.scroll - pointer.wheel_pixels);
        }
        over || self.gesture
    }

    /// Handles a press at `position` over the panel.
    fn press(&mut self, position: Vec2) {
        if let Some(index) = (0..MODES.len()).find(|&index| tab_rect(index).contains(position)) {
            self.select(MODES[index]);
            return;
        }
        let (Some(track), Some(thumb)) = (self.scroll_track(), self.thumb_rect()) else {
            return;
        };
        if thumb.contains(position) {
            self.thumb_grab = Some(position.y - thumb.y);
        } else if track.contains(position) {
            // A click on the track pages toward it.
            let page = if position.y < thumb.y {
                -self.viewport_height
            } else {
                self.viewport_height
            };
            self.scroll_to(self.scroll + page);
        }
    }

    /// Scrolls so the thumb's top is at `thumb_top`.
    fn drag_thumb(&mut self, thumb_top: f32) {
        let (Some(track), Some(thumb)) = (self.scroll_track(), self.thumb_rect()) else {
            return;
        };
        let travel = (track.h - thumb.h).max(1.0);
        self.scroll_to((thumb_top - track.y) / travel * self.max_scroll());
    }

    /// The farthest the content scrolls.
    fn max_scroll(&self) -> f32 {
        (self.content_height - self.viewport_height).max(0.0)
    }

    /// Scrolls to `scroll`, kept within range.
    fn scroll_to(&mut self, scroll: f32) {
        self.scroll = scroll.clamp(0.0, self.max_scroll());
    }

    /// The scrollbar's track, when the content overflows.
    fn scroll_track(&self) -> Option<Rect> {
        let left = PANEL_WIDTH - SCROLLBAR_INSET - SCROLLBAR_WIDTH;
        let track = Rect::new(left, HEADER_HEIGHT, SCROLLBAR_WIDTH, self.viewport_height);
        (self.max_scroll() > 0.0).then_some(track)
    }

    /// The scrollbar's thumb, when the content overflows.
    ///
    /// It is as tall as the visible fraction of the content, and as far
    /// down as the content is scrolled.
    fn thumb_rect(&self) -> Option<Rect> {
        let track = self.scroll_track()?;
        let height = (track.h * self.viewport_height / self.content_height)
            .clamp(MIN_THUMB_HEIGHT.min(track.h), track.h);
        let top = track.y + (track.h - height) * self.scroll / self.max_scroll();
        Some(Rect::new(track.x, top, track.w, height))
    }

    /// Whether the panel looks different from when it was last painted, at
    /// a window `height` points tall.
    ///
    /// If so, it counts as painted from now.
    pub fn needs_paint(&mut self, status: Option<&'static str>, height: f32) -> bool {
        let look = Appearance {
            refreshes: self.refreshes,
            mode: self.mode,
            scroll: self.scroll,
            status,
            height,
        };
        let changed = self.painted != Some(look);
        self.painted = Some(look);
        changed
    }

    /// Draws the column at the left edge of a window `height` points tall,
    /// with `status` (such as "Paused") beside the title.
    ///
    /// Returns the side of the P-V plot drawn, or zero if none was, and
    /// records the content height for scrolling.
    pub fn draw(
        &mut self,
        pen: &mut Pen,
        engine: &ConnectedChambers,
        status: Option<&str>,
        height: f32,
    ) -> f32 {
        self.viewport_height = (height - HEADER_HEIGHT - PANEL_INSET - BOTTOM_ROOM).max(0.0);
        self.scroll_to(self.scroll);
        pen.set_shadow(Some(TEXT_SHADOW));
        // Content scrolled up under the header is cut off there.
        let viewport = Rect::new(
            PANEL_LEFT,
            HEADER_HEIGHT,
            PANEL_WIDTH - PANEL_LEFT,
            self.viewport_height,
        );
        pen.clip(Some(viewport));

        let origin = HEADER_HEIGHT - self.scroll;
        let (bottom, plot_side) = match self.mode {
            PanelMode::Simple => (simple::draw(pen, &self.readout, origin), 0.0),
            PanelMode::Advanced => {
                let y = advanced::draw(pen, &self.readout, origin);
                let (y, side) = self.draw_loops(pen, engine, y + SECTION_GAP, origin);
                let bottom = advanced::draw_totals(pen, &self.readout, y + SECTION_GAP);
                (bottom, side)
            }
        };
        pen.clip(None);
        self.content_height = bottom - origin + END_ROOM;
        self.scroll_to(self.scroll);

        self.draw_header(pen, status);
        if let (Some(track), Some(thumb)) = (self.scroll_track(), self.thumb_rect()) {
            let radius = SCROLLBAR_WIDTH / 2.0;
            pen.round_rect(track, radius, SCROLL_TRACK_COLOR);
            pen.round_rect(thumb, radius, SCROLL_THUMB_COLOR);
        }
        pen.set_shadow(None);
        plot_side
    }

    /// Title, status and the view switch, above the scrolling content.
    fn draw_header(&self, pen: &mut Pen, status: Option<&str>) {
        pen.fitted_heading(TITLE, LEFT, TITLE_BASELINE, CONTENT_WIDTH, TITLE_SIZE, TEXT);
        if let Some(status) = status {
            let right = LEFT + CONTENT_WIDTH;
            pen.text_right(status, right, TITLE_BASELINE, BODY_SIZE, WARNING);
        }
        for (index, mode) in MODES.into_iter().enumerate() {
            let tab = tab_rect(index);
            let selected = mode == self.mode;
            // The chosen view is a glossy pill like the buttons; the other
            // is a quiet glass tile.
            if selected {
                let look = button::Look {
                    hue: Hue::Cyan,
                    on: false,
                };
                button::pill(pen, tab, look, false);
            } else {
                tile(pen, tab, TAB_HEIGHT / 2.0, PEARL);
            }
            let label = mode.label();
            let size = pen.fitted_size_weight(label, tab.w, TAB_SIZE, Weight::Semibold);
            let left = tab.x + (tab.w - pen.measure_weight(label, size, Weight::Semibold)) / 2.0;
            let baseline = tab.y + (tab.h + pen.text_height(label, size)) / 2.0;
            let color = if selected { TEXT } else { QUIET };
            pen.heading(label, left, baseline, size, color);
        }
    }

    /// Draws the P-V plot below `top`, for content that starts at `origin`.
    ///
    /// The plot is as large as fits the visible height, but at least
    /// `COMFORTABLE_PLOT_SIDE`, scrolling if needed. Returns the last
    /// baseline and the plot's side, which is zero before there is anything
    /// to plot.
    fn draw_loops(
        &self,
        pen: &mut Pen,
        engine: &ConnectedChambers,
        top: f32,
        origin: f32,
    ) -> (f32, f32) {
        let mut y = advanced::section(pen, LOOPS_HEADING, top) + LINE;
        pen.fitted(LAW_WORK, LEFT, y, CONTENT_WIDTH, LAW_SIZE, LAW);
        y += SMALL_LINE;
        let loop_text = &self.readout.loop_text;
        pen.fitted(loop_text, LEFT, y, CONTENT_WIDTH, SMALL_SIZE, QUIET);
        let Some(range) = self.range else {
            return (y, 0.0);
        };
        // Measured from the content's own top, so scrolling cannot change
        // the plot's size.
        let room = self.viewport_height - (y - origin) - READOUT_GAP - PLOT_BOTTOM - MARGIN;
        let side = room.clamp(COMFORTABLE_PLOT_SIDE, MAX_PLOT_SIDE);
        let plot = Rect::new(LEFT + PLOT_LEFT, y + READOUT_GAP, side, side);
        draw_plot(pen, engine, plot, range);
        (plot.bottom() + PLOT_BOTTOM, side)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/panel.rs"]
mod tests;
