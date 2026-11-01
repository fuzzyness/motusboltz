//! The main loop.
//!
//! Each frame reads the input, steps the simulation, updates the panel and
//! camera, and draws the background, then the scene with its labels, the
//! panel on the left and the sliders on the right with the demo's banner
//! under them, and the buttons over the scene. Under the toggles at the
//! scene's top left go the frame rate (in the Advanced view) and the
//! warning when physics time is dropped.

use macroquad::miniquad::date;
use macroquad::prelude::{
    Camera,
    Color,
    Mat4,
    MouseButton,
    Rect,
    Vec2,
    get_frame_time,
    is_mouse_button_down,
    is_mouse_button_pressed,
    mouse_position,
    next_frame,
    screen_height,
    screen_width, //
};
use macroquad::rand::{
    gen_range,
    srand, //
};
use macroquad::time::get_time;

use crate::graphics::{
    Scene,
    Scenery, //
};
use crate::sim::engine::ConnectedChambers;
use crate::sim::geometry::ChamberGeometry;
use crate::sim::units::{
    DEFAULT_CONCENTRATION,
    MAX_CONCENTRATION,
    MIN_CONCENTRATION, //
};
use crate::ui::camera::{
    OrbitCamera,
    Pointer, //
};
use crate::ui::controls::{
    self,
    Action,
    BACKING,
    BACKING_RADIUS,
    ButtonStates,
    ControlEvents,
    Controls,
    MARGIN,
    SceneArea,
    TOGGLES_BOTTOM, //
};
use crate::ui::demos::{
    self,
    Scenario, //
};
use crate::ui::drawer::{
    self,
    Edge, //
};
use crate::ui::energy_strip;
use crate::ui::fonts;
use crate::ui::frame_stats::{
    FrameStats,
    LIMITED_TEXT, //
};
use crate::ui::input::{
    CameraKeys,
    KeyCommands,
    shift_down,
    wheel_pixels, //
};
use crate::ui::layer::CachedLayer;
use crate::ui::layout::{
    PANEL_WIDTH,
    scene_left,
    scene_span, //
};
use crate::ui::panel::{
    EnginePanel,
    PanelPointer, //
};
use crate::ui::pen::Pen;
use crate::ui::scene_labels;
use crate::ui::shortcuts;
use crate::ui::start_prompt::StartPrompt;

/// Gap between the build label and Hide all, at its left.
const BUILD_LABEL_GAP: f32 = 14.0;
/// Room the keys list leaves above the bottom row of buttons.
const SHORTCUTS_BOTTOM_ROOM: f32 = 140.0;
/// Version and Git revision of this build (see build.rs), shown in the
/// bottom-right corner so it is always clear which build is running.
const BUILD_LABEL: &str = concat!(
    "v",
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("MOTUSBOLTZ_REVISION"),
    ")"
);
/// Text size of the build label.
const BUILD_LABEL_FONT_SIZE: f32 = 14.0;
/// Color of the build label.
const BUILD_LABEL_COLOR: Color = Color::from_rgba(120, 140, 160, 255);
/// First baseline of the corner readout, under the toggles.
const CORNER_TOP_Y: f32 = TOGGLES_BOTTOM + 24.0;
/// Text size of the corner readout.
const CORNER_FONT_SIZE: f32 = 16.0;
/// Distance between the corner readout's baselines.
const CORNER_LINE_HEIGHT: f32 = 20.0;
/// Color of the frame-rate line.
const CORNER_TEXT_COLOR: Color = Color::from_rgba(150, 165, 180, 255);
/// How far the shade behind loose text reaches past it at each end.
const BACKING_PAD: f32 = 7.0;
/// Height of the shade behind loose text, as a share of the text size,
/// before the padding.
const BACKING_HEIGHT_SHARE: f32 = 1.35;
/// The panel's warning color.
const WARNING_COLOR: Color = Color::from_rgba(255, 130, 110, 255);
/// Clock resolution used for the startup seed, in ticks per second.
const SEED_TICKS_PER_SECOND: f64 = 1.0e6;
/// Fills chosen with keys 1 to 5: 256, 512, 1,024, 1,536 and 2,048 particles.
const CONCENTRATION_PRESETS: [f32; 5] = [
    MIN_CONCENTRATION,
    0.25,
    DEFAULT_CONCENTRATION,
    0.75,
    MAX_CONCENTRATION,
];
/// Longest frame time the camera keys act over, so a stalled frame does
/// not swing the view.
const MAX_CAMERA_DT: f32 = 0.1;
/// Slow mode runs the simulation at this fraction of its normal pace, for a
/// closer look at single particles. It changes only how much model time
/// passes per frame, never the physics, and it also lightens the work per
/// frame.
const SLOW_MOTION_FACTOR: f32 = 0.25;
/// How many particles tracers follow: enough to catch both cylinders and the
/// opening, few enough to follow by eye.
const FOLLOWED_PARTICLES: usize = 5;
/// Extra zoom-out so the cylinders, crank and flywheel all fit the first view.
const CAMERA_FRAMING_MARGIN: f32 = 1.42;

/// The first view, framing a chamber of `geometry`.
pub fn initial_camera(geometry: ChamberGeometry) -> OrbitCamera {
    let scale = geometry.cylinder_radius / ChamberGeometry::scaled(1.0).cylinder_radius;
    OrbitCamera::with_scale(CAMERA_FRAMING_MARGIN * scale)
}

/// Seed from the wall clock, so every launch starts from a different gas.
///
/// `date::now` works natively and in the browser.
fn clock_seed() -> u64 {
    (date::now() * SEED_TICKS_PER_SECOND) as u64
}

/// The demo set up at launch.
const FIRST_DEMO: Scenario = Scenario::HeatEngine;

/// Runs the application.
///
/// It starts with a default fill at 300 K, the walls set for the first
/// demo and the flywheel at rest until the first Start.
pub async fn run() {
    srand(clock_seed());
    let mut app = App::new();
    loop {
        app.frame();
        next_frame().await;
    }
}

/// Everything the app keeps from frame to frame.
struct App {
    /// The engine.
    simulation: ConnectedChambers,
    /// The simulation is paused.
    paused: bool,
    /// Slow motion is on.
    slow: bool,
    /// Tracers are on; they stay on through refills, which choose new
    /// particles.
    tracers: bool,
    /// The keys list shows.
    show_keys: bool,
    /// The labels on the scene show.
    show_labels: bool,
    /// The Demos menu is open.
    show_demos: bool,
    /// The demo set up last, until a slider or a new fill changes its
    /// settings.
    demo: Option<Scenario>,
    /// The sky with a cloud to stand on, or the dark grid.
    scenery: Scenery,
    /// When Start shows.
    start_prompt: StartPrompt,
    /// The readings panel.
    panel: EnginePanel,
    /// The panel as last painted, repainted only when it changes.
    panel_layer: CachedLayer,
    /// The panel is open.
    show_panel: bool,
    /// The sliders are open.
    show_sliders: bool,
    /// Everything over the scene is hidden but Hide all itself.
    hide_all: bool,
    /// The view of the scene.
    camera: OrbitCamera,
    /// The 3D drawing.
    scene: Scene,
    /// The buttons and the sliders.
    controls: Controls,
    /// The frame-rate readout and the warning.
    frame_stats: FrameStats,
}

impl App {
    /// The app at launch, with the fonts loaded.
    fn new() -> Self {
        fonts::load();
        let mut simulation = ConnectedChambers::with_concentration(DEFAULT_CONCENTRATION);
        let (hot_kelvin, cold_kelvin) = FIRST_DEMO.kelvin();
        simulation.set_hot_kelvin(hot_kelvin);
        simulation.set_cold_kelvin(cold_kelvin);
        Self {
            panel: EnginePanel::new(),
            panel_layer: CachedLayer::new(),
            show_panel: true,
            show_sliders: true,
            hide_all: false,
            camera: initial_camera(simulation.geometry()),
            scene: Scene::new(&simulation),
            controls: Controls::new(
                simulation.hot_kelvin(),
                simulation.cold_kelvin(),
                DEFAULT_CONCENTRATION,
            ),
            paused: false,
            slow: false,
            tracers: false,
            show_keys: false,
            show_labels: true,
            show_demos: false,
            demo: Some(FIRST_DEMO),
            scenery: Scenery::default(),
            start_prompt: StartPrompt::new(),
            frame_stats: FrameStats::new(),
            simulation,
        }
    }

    /// Whether the panel is on screen this frame.
    fn panel_shown(&self) -> bool {
        self.show_panel && !self.hide_all
    }

    /// Where the buttons go this frame.
    fn scene_area(&self) -> SceneArea {
        let (left, width) = scene_span(screen_width(), self.show_panel, self.show_sliders);
        SceneArea {
            edge: scene_left(self.panel_shown()),
            left,
            width,
            window_width: screen_width(),
            height: screen_height().max(1.0),
        }
    }

    /// Which buttons show and how they look this frame.
    fn button_states(&self) -> ButtonStates {
        ButtonStates {
            paused: self.paused,
            slow: self.slow,
            tracers: self.tracers,
            panel: self.show_panel,
            keys: self.show_keys,
            labels: self.show_labels,
            demos: self.show_demos,
            demo: self.demo,
            start: self.start_prompt.shown(),
            // A demo's banner says what to expect instead of the tip.
            no_gap: self.simulation.hot_kelvin() <= self.simulation.cold_kelvin()
                && self.demo.is_none(),
            sky: self.scenery == Scenery::Sky,
            sliders: self.show_sliders,
            hidden: self.hide_all,
        }
    }

    /// What the panel shows beside its title: Paused, Slow motion, or
    /// nothing while the engine runs normally.
    fn status(&self) -> Option<&'static str> {
        if self.paused {
            Some("Paused")
        } else if self.slow {
            Some("Slow motion")
        } else {
            None
        }
    }

    /// Runs one frame: input, simulation, camera and drawing.
    fn frame(&mut self) {
        let keys = KeyCommands::read();
        let wheel = wheel_pixels();
        let events = self
            .controls
            .update(self.scene_area(), self.button_states());
        let panel_blocks = self.panel_shown()
            && self.panel.handle_pointer(PanelPointer {
                position: Vec2::from(mouse_position()),
                wheel_pixels: wheel,
                pressed: is_mouse_button_pressed(MouseButton::Left),
                down: is_mouse_button_down(MouseButton::Left),
            });
        self.apply(keys, events);
        self.step();
        self.camera.update(
            CameraKeys::read(),
            Pointer {
                enabled: !(self.controls.blocks_camera() || panel_blocks),
                shift_down: shift_down(),
                wheel_pixels: wheel,
            },
            get_frame_time().min(MAX_CAMERA_DT),
            screen_height(),
        );
        self.draw();
    }

    /// Carries out the keys pressed and the controls used this frame.
    fn apply(&mut self, keys: KeyCommands, events: ControlEvents) {
        let chose = |key: bool, action: Action| key || events.action == Some(action);
        if chose(keys.reset_gas, Action::ResetGas) {
            self.simulation.reset();
        }
        // Space does what the shared button shows: Start while it is
        // offered (which also lifts a pause), Pause or Resume otherwise.
        if events.action == Some(Action::Start) || (keys.go && self.start_prompt.shown()) {
            self.paused = false;
            self.simulation.start_engine();
        } else if chose(keys.go, Action::TogglePause) {
            self.paused = !self.paused;
        }
        if chose(keys.tracers, Action::ToggleTracers) {
            self.tracers = !self.tracers;
            if !self.tracers {
                self.simulation.stop_following();
            }
        }
        // The other toggles flip when their key or button is pressed.
        self.slow ^= chose(keys.slow, Action::ToggleSlow);
        self.show_panel ^= chose(keys.panel, Action::TogglePanel);
        self.show_sliders ^= chose(keys.sliders, Action::ToggleSliders);
        self.hide_all ^= chose(keys.hide_all, Action::ToggleAll);
        self.show_labels ^= chose(keys.labels, Action::ToggleLabels);
        // The keys list and the Demos menu drop into the same place, so
        // opening one closes the other.
        if chose(keys.keys, Action::ToggleKeys) {
            self.show_keys = !self.show_keys;
            self.show_demos &= !self.show_keys;
        }
        if chose(keys.demos, Action::ToggleDemos) {
            self.show_demos = !self.show_demos;
            self.show_keys &= !self.show_demos;
        }
        if let Some(Action::Demo(scenario)) = events.action {
            self.set_up(scenario);
        }
        if chose(keys.scenery, Action::ToggleScenery) {
            self.scenery = self.scenery.toggled();
        }
        if keys.kick {
            self.simulation.kick_flywheel();
        }
        if keys.details {
            self.panel.toggle_mode();
        }
        if keys.camera_reset {
            self.camera.reset();
        }
        // A new fill or temperature ends the demo, whose banner would no
        // longer be true.
        let changed = [events.concentration, events.hot_kelvin, events.cold_kelvin];
        if keys.fill_preset.is_some() || changed.iter().any(Option::is_some) {
            self.demo = None;
        }
        if let Some(preset) = keys.fill_preset {
            let concentration = CONCENTRATION_PRESETS[preset];
            self.simulation.set_concentration(concentration);
            self.controls.set_concentration(concentration);
        }
        if let Some(concentration) = events.concentration {
            self.simulation.set_concentration(concentration);
        }
        if let Some(kelvin) = events.hot_kelvin {
            self.simulation.set_hot_kelvin(kelvin);
        }
        if let Some(kelvin) = events.cold_kelvin {
            self.simulation.set_cold_kelvin(kelvin);
        }
    }

    /// Sets up `scenario`: its temperatures on the walls and the sliders,
    /// and fresh gas with the flywheel at rest, waiting for Start.
    fn set_up(&mut self, scenario: Scenario) {
        let (hot_kelvin, cold_kelvin) = scenario.kelvin();
        self.simulation.set_hot_kelvin(hot_kelvin);
        self.simulation.set_cold_kelvin(cold_kelvin);
        self.simulation.reset();
        self.controls.set_kelvin(hot_kelvin, cold_kelvin);
        self.paused = false;
        self.show_demos = false;
        self.demo = Some(scenario);
    }

    /// Steps the simulation by this frame's time and updates what follows
    /// from it: the frame stats, Start and the panel.
    fn step(&mut self) {
        let physics_start = date::now();
        // A refill (reset, new fill) clears the followed particles.
        if self.tracers && self.simulation.tracers().is_none() {
            let seed = gen_range(0, u64::MAX);
            self.simulation.follow_particles(FOLLOWED_PARTICLES, seed);
        }
        let pace = if self.slow { SLOW_MOTION_FACTOR } else { 1.0 };
        let limited = !self.paused && self.simulation.update(pace * get_frame_time());
        let physics_seconds = date::now() - physics_start;
        let now = get_time();
        self.frame_stats.record(now, physics_seconds, limited);
        let start_fraction = self.simulation.power_pistons().start_speed_fraction();
        self.start_prompt.update(start_fraction, now);
        if self.panel_shown() {
            self.panel.update(&self.simulation, now);
        }
    }

    /// Draws the scene and everything over it.
    fn draw(&mut self) {
        let area = self.scene_area();
        // The scene fills the window, behind the panel and the sliders; the
        // camera keeps the engine at the window's middle.
        let camera = self.camera.camera();
        self.scene.render(&self.simulation, &camera, self.scenery);

        // Measure every string first, then draw (see `Pen` for why).
        let panel_stale = self.panel_shown() && self.panel.needs_paint(self.status(), area.height);
        let view = camera.matrix();
        self.draw_overlay(&mut Pen::warm(), area, view, panel_stale);
        self.draw_overlay(&mut Pen::screen(), area, view, panel_stale);
    }

    /// Everything over the scene, seen through `view`: the labels, the
    /// panel, the controls, the energy strip, the build label, the demo's
    /// banner, the keys list and the corner readout.
    fn draw_overlay(&mut self, pen: &mut Pen, area: SceneArea, view: Mat4, panel_stale: bool) {
        let status = self.status();
        let start = self.start_prompt.shown();
        // The demo's banner and the energy strip stack under the sliders'
        // card. The labels keep clear of both and give way to the Demos
        // menu while it is open.
        let shown = !self.hide_all;
        let width = area.window_width;
        let banner = self
            .demo
            .filter(|_| shown)
            .map(|demo| demos::banner(pen, width, demo, start));
        let column_top = banner.unwrap_or(controls::slider_column(width)).bottom() + MARGIN;
        let panel = self.panel_shown();
        let strip =
            energy_strip::place(pen, area, &self.simulation, column_top, panel).filter(|_| shown);
        if self.show_labels && !self.show_demos && shown {
            let cards: Vec<Rect> = banner.into_iter().chain(strip).collect();
            scene_labels::draw(pen, &self.simulation, view, area, &cards);
        }
        if self.panel_shown() {
            let panel_area = Rect::new(0.0, 0.0, PANEL_WIDTH, area.height);
            drawer::draw_card(pen, controls::panel_card(area.height), Edge::Left, true);
            if pen.is_warming() {
                if self.panel_layer.will_paint(panel_area, panel_stale) {
                    self.panel.draw(pen, &self.simulation, status, area.height);
                }
            } else {
                let (panel, simulation) = (&mut self.panel, &self.simulation);
                self.panel_layer.draw(panel_area, panel_stale, || {
                    panel.draw(&mut Pen::screen(), simulation, status, area.height);
                });
            }
        }
        self.controls.draw(pen, area, self.button_states());
        if self.hide_all {
            return;
        }
        if let Some(card) = strip {
            energy_strip::draw(pen, card, &self.simulation);
        }
        // The build label sits left of Hide all, level with its text.
        let corner = controls::corner_button(pen, area);
        let label_right = corner.x - BUILD_LABEL_GAP;
        let label_y = corner.y + (corner.h + pen.text_height("H", BUILD_LABEL_FONT_SIZE)) / 2.0;
        let size = BUILD_LABEL_FONT_SIZE;
        if self.scenery == Scenery::Sky {
            let width = pen.measure(BUILD_LABEL, size);
            backing(pen, label_right - width, label_y, width, size);
        }
        pen.text_right(BUILD_LABEL, label_right, label_y, size, BUILD_LABEL_COLOR);
        if let Some(demo) = self.demo {
            demos::draw_banner(pen, width, demo, start);
        }
        // The keys list or the Demos menu drops down under the toggles,
        // and the corner readout goes under it.
        let mut corner_top = CORNER_TOP_Y;
        if self.show_demos {
            corner_top = controls::demo_menu_bottom() + CORNER_LINE_HEIGHT;
        }
        if self.show_keys {
            let list_top = TOGGLES_BOTTOM + MARGIN / 2.0;
            let middle = controls::toggles_middle(area);
            let room = (
                area.width - 2.0 * MARGIN,
                area.height - TOGGLES_BOTTOM - SHORTCUTS_BOTTOM_ROOM,
            );
            let (_, height) = shortcuts::draw(pen, middle, list_top, room.0, room.1);
            corner_top = list_top + height + CORNER_LINE_HEIGHT;
        }
        self.draw_corner(pen, area, corner_top);
    }

    /// Draws the frame rate and physics time in the Advanced view, and the
    /// warning while it is held, at the scene's left from `top` down.
    fn draw_corner(&self, pen: &mut Pen, area: SceneArea, top: f32) {
        let stats = self
            .panel
            .advanced()
            .then(|| self.frame_stats.line())
            .flatten()
            .map(|line| (line, CORNER_TEXT_COLOR));
        let warning = self
            .frame_stats
            .limited(get_time())
            .then(|| (LIMITED_TEXT.to_owned(), WARNING_COLOR));
        let mut y = top;
        let (left, room) = (area.edge + MARGIN, area.right() - area.edge - 2.0 * MARGIN);
        for (text, color) in stats.into_iter().chain(warning) {
            if self.scenery == Scenery::Sky {
                let size = pen.fitted_size(&text, room, CORNER_FONT_SIZE);
                let width = pen.measure(&text, size);
                backing(pen, left, y, width, size);
            }
            pen.fitted(&text, left, y, room, CORNER_FONT_SIZE, color);
            y += CORNER_LINE_HEIGHT;
        }
    }
}

/// Draws a shade behind one line of text at `x`, `width` wide at `size`,
/// with its baseline at `y`, so it reads over the bright sky.
fn backing(pen: &Pen, x: f32, y: f32, width: f32, size: f32) {
    let top = y - size - BACKING_PAD / 2.0;
    let height = size * BACKING_HEIGHT_SHARE + BACKING_PAD;
    let shade = Rect::new(x - BACKING_PAD, top, width + 2.0 * BACKING_PAD, height);
    pen.round_rect(shade, BACKING_RADIUS, BACKING);
}

#[cfg(test)]
#[path = "../tests/unit/app.rs"]
mod tests;
