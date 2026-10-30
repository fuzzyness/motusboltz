//! Left-column readings panel, in two views. Simple shows what the project
//! is about in plain words: where the energy goes each turn and what each
//! law says about it. Advanced shows every reading, with the
//! pressure-volume loops. The column scrolls, by wheel or by dragging its
//! scrollbar, whenever its content is taller than the window.
//!
//! The readings are rebuilt at most `REFRESH_SECONDS` apart, not every
//! frame, and the app repaints the panel only when what it shows changes
//! (see `needs_paint`).

mod advanced;
mod plot;
mod readout;
mod simple;

use macroquad::prelude::{
    Color,
    Rect,
    Vec2,
    screen_height, //
};

use crate::sim::engine::ConnectedChambers;
use crate::ui::button::{
    self,
    Hue, //
};
use crate::ui::layout::PANEL_WIDTH;
use crate::ui::pen::{
    Pen,
    Weight, //
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

pub const TITLE: &str = "MotusBoltz";

const MARGIN: f32 = 16.0;
const CONTENT_WIDTH: f32 = PANEL_WIDTH - 2.0 * MARGIN;
/// Seconds of wall time between text refreshes.
const REFRESH_SECONDS: f64 = 0.1;

// Header: the title, then the Simple and Advanced switch. It stays put
// while the content below it scrolls.
const TITLE_BASELINE: f32 = 36.0;
const TAB_TOP: f32 = 50.0;
const TAB_HEIGHT: f32 = 26.0;
const TAB_GAP: f32 = 6.0;
const HEADER_HEIGHT: f32 = TAB_TOP + TAB_HEIGHT + 4.0;

// Scrollbar, in the right margin.
const SCROLLBAR_WIDTH: f32 = 5.0;
const SCROLLBAR_INSET: f32 = 5.0;
const MIN_THUMB_HEIGHT: f32 = 24.0;

const TITLE_SIZE: f32 = 28.0;
const SECTION_SIZE: f32 = 13.0;
const LAW_SIZE: f32 = 15.0;
const BODY_SIZE: f32 = 15.0;
const SMALL_SIZE: f32 = 13.0;
const TAB_SIZE: f32 = 15.0;
const LINE: f32 = 19.0;
const SMALL_LINE: f32 = 16.0;
const SECTION_GAP: f32 = 14.0;
const READOUT_GAP: f32 = 10.0;
/// Smallest P-V plot in the Advanced view; on a short window the column
/// scrolls to reach it rather than shrinking it further.
const COMFORTABLE_PLOT_SIDE: f32 = 200.0;

/// The panel's background, drawn behind it every frame by the app:
/// translucent, so the scene shows through faintly.
pub const BACKGROUND: Color = Color::new(19.0 / 255.0, 29.0 / 255.0, 43.0 / 255.0, 0.62);
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
const TAB_COLOR: Color = Color::from_rgba(38, 52, 68, 255);
const SCROLL_TRACK_COLOR: Color = Color::from_rgba(38, 52, 68, 255);
const SCROLL_THUMB_COLOR: Color = Color::from_rgba(120, 140, 160, 255);

const LOOPS_HEADING: &str = "P-V LOOPS (LAST TURN)";
const LAW_WORK: &str = "Work: the area inside each loop";

/// Everything that changes how the panel looks. The app keeps the painted
/// panel as an image and repaints it only when this changes.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Look {
    refreshes: u64,
    mode: PanelMode,
    scroll: f32,
    status: Option<&'static str>,
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

const MODES: [PanelMode; 2] = [PanelMode::Simple, PanelMode::Advanced];

impl PanelMode {
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
        MARGIN + index as f32 * (width + TAB_GAP),
        TAB_TOP,
        width,
        TAB_HEIGHT,
    )
}

/// Pointer state for one frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct PanelPointer {
    pub position: Vec2,
    /// Wheel scroll this frame, in pixels; positive scrolls up.
    pub wheel_pixels: f32,
    /// The left button went down this frame.
    pub pressed: bool,
    /// The left button is held.
    pub down: bool,
}

pub struct EnginePanel {
    readout: Readout,
    range: Option<PlotRange>,
    next_refresh: f64,
    /// Text rebuilds so far, so a repaint follows each one.
    refreshes: u64,
    /// How the panel looked when last painted.
    painted: Option<Look>,
    mode: PanelMode,
    /// How far the content is scrolled up, in pixels.
    scroll: f32,
    /// Content and visible heights from the last draw, for scrolling.
    content_height: f32,
    viewport_height: f32,
    /// Where the scrollbar thumb was grabbed, relative to its top, while it
    /// is being dragged.
    thumb_grab: Option<f32>,
    /// A press that started on the panel is still held.
    gesture: bool,
}

impl EnginePanel {
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

    fn refresh(&mut self, engine: &ConnectedChambers) {
        self.refreshes += 1;
        self.readout = readout(engine);
        self.range = plot_range(engine);
    }

    /// Handles clicks on the view switch, the wheel and the scrollbar. Returns whether the
    /// camera should ignore the pointer: it is over the panel, or a press
    /// that started here is still held.
    pub fn handle_pointer(&mut self, pointer: PanelPointer) -> bool {
        let over = (0.0..=PANEL_WIDTH).contains(&pointer.position.x);
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

    fn max_scroll(&self) -> f32 {
        (self.content_height - self.viewport_height).max(0.0)
    }

    fn scroll_to(&mut self, scroll: f32) {
        self.scroll = scroll.clamp(0.0, self.max_scroll());
    }

    /// The scrollbar's track, when the content overflows.
    fn scroll_track(&self) -> Option<Rect> {
        (self.max_scroll() > 0.0).then(|| {
            Rect::new(
                PANEL_WIDTH - SCROLLBAR_INSET - SCROLLBAR_WIDTH,
                HEADER_HEIGHT,
                SCROLLBAR_WIDTH,
                self.viewport_height,
            )
        })
    }

    /// The scrollbar's thumb: as tall as the visible fraction of the
    /// content, and as far down as the content is scrolled.
    fn thumb_rect(&self) -> Option<Rect> {
        let track = self.scroll_track()?;
        let height = (track.h * self.viewport_height / self.content_height)
            .clamp(MIN_THUMB_HEIGHT.min(track.h), track.h);
        let top = track.y + (track.h - height) * self.scroll / self.max_scroll();
        Some(Rect::new(track.x, top, track.w, height))
    }

    /// Whether the panel looks different from when it was last painted, at
    /// a window `height` points tall; if so, it counts as painted from now.
    pub fn needs_paint(&mut self, status: Option<&'static str>, height: f32) -> bool {
        let look = Look {
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

    /// Draws the column at the left edge, full height, with `status` (such
    /// as "Paused") beside the title.
    pub fn draw(&mut self, pen: &mut Pen, engine: &ConnectedChambers, status: Option<&str>) {
        self.draw_with_height(pen, engine, status, screen_height());
    }

    /// Draws the column for a window `height` pixels tall, scrolled by the
    /// current offset, and returns the side of the P-V plot drawn, or zero
    /// if none was. Also records the content height for scrolling.
    fn draw_with_height(
        &mut self,
        pen: &mut Pen,
        engine: &ConnectedChambers,
        status: Option<&str>,
        height: f32,
    ) -> f32 {
        self.viewport_height = (height - HEADER_HEIGHT).max(0.0);
        self.scroll_to(self.scroll);
        // Content scrolled up under the header is cut off there.
        pen.clip(Some(Rect::new(
            0.0,
            HEADER_HEIGHT,
            PANEL_WIDTH,
            self.viewport_height,
        )));

        let origin = HEADER_HEIGHT - self.scroll;
        let x = MARGIN;
        let (bottom, plot_side) = match self.mode {
            PanelMode::Simple => (simple::draw(pen, &self.readout, x, origin), 0.0),
            PanelMode::Advanced => {
                let y = advanced::draw(pen, &self.readout, x, origin);
                let (y, side) = self.draw_loops(
                    pen,
                    engine,
                    x,
                    y + SECTION_GAP,
                    origin,
                    COMFORTABLE_PLOT_SIDE,
                );
                (
                    advanced::draw_totals(pen, &self.readout, x, y + SECTION_GAP),
                    side,
                )
            }
        };
        pen.clip(None);
        self.content_height = bottom - origin + MARGIN;
        self.scroll_to(self.scroll);

        self.draw_header(pen, status);
        if let (Some(track), Some(thumb)) = (self.scroll_track(), self.thumb_rect()) {
            pen.rect(track.x, track.y, track.w, track.h, SCROLL_TRACK_COLOR);
            pen.rect(thumb.x, thumb.y, thumb.w, thumb.h, SCROLL_THUMB_COLOR);
        }
        plot_side
    }

    /// Title, status and the view switch, above the scrolling content.
    fn draw_header(&self, pen: &mut Pen, status: Option<&str>) {
        pen.fitted_heading(
            TITLE,
            MARGIN,
            TITLE_BASELINE,
            CONTENT_WIDTH,
            TITLE_SIZE,
            TEXT,
        );
        if let Some(status) = status {
            pen.text_right(
                status,
                MARGIN + CONTENT_WIDTH,
                TITLE_BASELINE,
                BODY_SIZE,
                WARNING,
            );
        }
        for (index, mode) in MODES.into_iter().enumerate() {
            let tab = tab_rect(index);
            let selected = mode == self.mode;
            // The chosen view is a glossy pill like the buttons; the other
            // stays a flat, dark one.
            if selected {
                let look = button::Look {
                    hue: Hue::Cyan,
                    on: false,
                };
                button::pill(pen, tab, look, false);
            } else {
                pen.round_rect(tab.x, tab.y, tab.w, tab.h, TAB_HEIGHT / 2.0, TAB_COLOR);
            }
            let label = mode.label();
            let size = pen.fitted_size_weight(label, tab.w, TAB_SIZE, Weight::Semibold);
            pen.heading(
                label,
                tab.x + (tab.w - pen.measure_weight(label, size, Weight::Semibold)) / 2.0,
                tab.y + (tab.h + pen.text_height(label, size)) / 2.0,
                size,
                if selected { TEXT } else { QUIET },
            );
        }
    }

    /// The P-V plot below `top`: as large as fits the visible height, but
    /// at least `min_side`, scrolling if needed. Returns the last baseline
    /// and the plot's side, which is zero before there is anything to plot.
    fn draw_loops(
        &self,
        pen: &mut Pen,
        engine: &ConnectedChambers,
        x: f32,
        top: f32,
        origin: f32,
        min_side: f32,
    ) -> (f32, f32) {
        let mut y = advanced::section(pen, LOOPS_HEADING, x, top);
        y += LINE;
        pen.fitted(LAW_WORK, x, y, CONTENT_WIDTH, LAW_SIZE, LAW);
        y += SMALL_LINE;
        pen.fitted(
            &self.readout.loop_text,
            x,
            y,
            CONTENT_WIDTH,
            SMALL_SIZE,
            QUIET,
        );
        let Some(range) = self.range else {
            return (y, 0.0);
        };
        // Measured from the content's own top, so scrolling cannot change
        // the plot's size.
        let room = self.viewport_height - (y - origin) - READOUT_GAP - PLOT_BOTTOM - MARGIN;
        let side = room.max(min_side).min(MAX_PLOT_SIDE);
        let plot = Rect::new(x + PLOT_LEFT, y + READOUT_GAP, side, side);
        draw_plot(pen, engine, plot, range);
        (plot.bottom() + PLOT_BOTTOM, side)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/panel.rs"]
mod tests;
