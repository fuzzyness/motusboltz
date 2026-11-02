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
    vec2,
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
    ControlMotion,
    Controls,
    MARGIN,
    SceneArea,
    TOGGLES_BOTTOM, //
};
use crate::ui::demos::{
    self,
    Scenario, //
};
use crate::ui::draw_target::{
    draw_moved,
    draw_to_screen, //
};
use crate::ui::drawer::{
    self,
    Edge,
    Slide, //
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
use crate::ui::motion;
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
/// Gap between the cards and lines stacked above Hide all.
const STACK_GAP: f32 = 10.0;
/// Room under the readout's last baseline for its shade.
const READOUT_DESCENT: f32 = 6.0;
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
/// Longest frame time the animations step over, so a stalled frame does
/// not skip one.
const MAX_ANIMATION_DT: f32 = 0.1;
/// Seconds the labels take to grow out of their dots or shrink into them.
const LABELS_SECONDS: f32 = 0.3;
/// Seconds a stuck drawer's glow takes to fade in or out.
const GLOW_SECONDS: f32 = 0.4;
/// Seconds the keys list takes to grow out of the Keys toggle or shrink
/// back into it.
const KEYS_SECONDS: f32 = 0.2;
/// Seconds the Demos menu takes to drop all its items or fold them back.
const MENU_SECONDS: f32 = 0.3;
/// Seconds a demo's banner takes to sweep in from the window's right edge,
/// bounce and all.
const BANNER_SWEEP_SECONDS: f32 = 0.5;
/// How far the banner overshoots its place (see `motion::bounce`).
const BANNER_OVERSHOOT: f32 = 1.2;
/// Seconds into the sweep when the banner's words start to fade in.
const BANNER_WORDS_DELAY: f32 = 0.25;
/// Seconds the banner's words take to fade in.
const BANNER_FADE_SECONDS: f32 = 0.4;
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
    /// Seconds since the demo was set up, which sweep its banner in.
    demo_age: f32,
    /// How far the labels have grown out of their dots, from 0 to 1.
    labels_grown: f32,
    /// How far the keys list has grown out of its toggle, from 0 to 1.
    keys_grown: f32,
    /// How far the Demos menu has dropped, from 0 to 1.
    menu_dropped: f32,
    /// The panel's slide out of the left edge.
    panel_slide: Slide,
    /// The slider column's slide out of the right edge.
    sliders_slide: Slide,
    /// How far the glow of the panel, stuck open, has faded in.
    panel_glow: f32,
    /// The same for the slider column.
    sliders_glow: f32,
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
            demo_age: 0.0,
            labels_grown: 1.0,
            keys_grown: 0.0,
            menu_dropped: 0.0,
            panel_slide: Slide::new(true),
            sliders_slide: Slide::new(true),
            panel_glow: 1.0,
            sliders_glow: 1.0,
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

    /// Whether any of the panel shows this frame, open, peeked or sliding.
    fn panel_visible(&self) -> bool {
        self.panel_slide.shown() && !self.hide_all
    }

    /// Whether the panel is all the way out, open or peeked, so it takes
    /// the pointer.
    fn panel_live(&self) -> bool {
        self.panel_slide.settled_out() && !self.hide_all
    }

    /// How far the panel and the slider column stand from their open
    /// places along x, or `None` out of sight, in a window `width` by
    /// `height` points.
    fn drawer_shifts(&self, width: f32, height: f32) -> (Option<f32>, Option<f32>) {
        let panel = controls::panel_card(height);
        let sliders = controls::slider_column(width);
        (
            self.panel_slide
                .shift(Edge::Left, drawer::slide_distance(panel, Edge::Left, width)),
            self.sliders_slide.shift(
                Edge::Right,
                drawer::slide_distance(sliders, Edge::Right, width),
            ),
        )
    }

    /// Whether `pointer` is over a drawer that is all the way out, where
    /// it should not turn the camera.
    fn over_drawer(&self, pointer: Vec2) -> bool {
        let (width, height) = (screen_width(), screen_height());
        let panel = drawer::reach(controls::panel_card(height), Edge::Left, width);
        let sliders = drawer::reach(controls::slider_column(width), Edge::Right, width);
        !self.hide_all
            && ((self.panel_slide.settled_out() && panel.contains(pointer))
                || (self.sliders_slide.settled_out() && sliders.contains(pointer)))
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
            panel_peek: self.panel_slide.peeking(),
            sliders_peek: self.sliders_slide.peeking(),
            sliders_out: self.sliders_slide.settled_out(),
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
        let dt = get_frame_time().min(MAX_ANIMATION_DT);
        let events = self
            .controls
            .update(self.scene_area(), self.button_states(), dt);
        let pointer = Vec2::from(mouse_position());
        let panel_blocks = self.panel_live()
            && self.panel.handle_pointer(PanelPointer {
                position: Vec2::from(mouse_position()),
                wheel_pixels: wheel,
                pressed: is_mouse_button_pressed(MouseButton::Left),
                down: is_mouse_button_down(MouseButton::Left),
            });
        self.apply(keys, events);
        self.animate(dt);
        self.step();
        self.camera.update(
            CameraKeys::read(),
            Pointer {
                enabled: !(self.controls.blocks_camera()
                    || panel_blocks
                    || self.over_drawer(pointer)),
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
        // Keys press their buttons as a click would.
        let start_or_pause = if self.start_prompt.shown() {
            Action::Start
        } else {
            Action::TogglePause
        };
        let keyed = [
            (keys.reset_gas, Action::ResetGas),
            (keys.go, start_or_pause),
            (keys.tracers, Action::ToggleTracers),
            (keys.slow, Action::ToggleSlow),
            (keys.keys, Action::ToggleKeys),
            (keys.demos, Action::ToggleDemos),
            (keys.labels, Action::ToggleLabels),
            (keys.scenery, Action::ToggleScenery),
            (keys.hide_all, Action::ToggleAll),
        ];
        for (pressed, action) in keyed {
            if pressed {
                self.controls.press(action);
            }
        }
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
        self.demo_age = 0.0;
    }

    /// Moves the overlays' animations on by `dt` seconds of wall time: the
    /// drawers slide, peeked under the pointer; the labels, the keys list
    /// and the Demos menu grow or shrink; and the demo's banner fades in.
    fn animate(&mut self, dt: f32) {
        let pointer = Vec2::from(mouse_position());
        let held = is_mouse_button_down(MouseButton::Left);
        let (width, height) = (screen_width(), screen_height().max(1.0));
        let shown = !self.hide_all;
        let drawers = [
            (
                &mut self.panel_slide,
                self.show_panel,
                controls::panel_card(height),
                Edge::Left,
            ),
            (
                &mut self.sliders_slide,
                self.show_sliders,
                controls::slider_column(width),
                Edge::Right,
            ),
        ];
        for (slide, open, card, edge) in drawers {
            let tab = drawer::tab(card, edge, width);
            let on_tab = shown && tab.contains(pointer);
            let on_card = shown && drawer::reach(card, edge, width).contains(pointer);
            slide.update(open, on_tab, on_card, held, dt);
        }
        // A stuck drawer's glow fades in once it has slid all the way out.
        let stuck = (
            self.show_panel && self.panel_slide.settled_out(),
            self.show_sliders && self.sliders_slide.settled_out(),
        );
        self.panel_glow = motion::advance(self.panel_glow, stuck.0, dt, GLOW_SECONDS);
        self.sliders_glow = motion::advance(self.sliders_glow, stuck.1, dt, GLOW_SECONDS);
        let labels = self.show_labels && !self.show_demos;
        self.labels_grown = motion::advance(self.labels_grown, labels, dt, LABELS_SECONDS);
        self.keys_grown = motion::advance(self.keys_grown, self.show_keys, dt, KEYS_SECONDS);
        self.menu_dropped = motion::advance(self.menu_dropped, self.show_demos, dt, MENU_SECONDS);
        self.demo_age += dt;
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
        if self.panel_visible() {
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
        let panel_stale =
            self.panel_visible() && self.panel.needs_paint(self.status(), area.height);
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
        // Above Hide all, in the column under the sliders' card, stand the
        // frame-rate readout, then the energy strip, then the demo's
        // banner. A strip with no room there under the banner goes to the
        // scene (see `energy_strip`). The labels keep clear of all of it
        // and give way to the Demos menu while it is open.
        let shown = !self.hide_all;
        let width = area.window_width;
        let column = controls::slider_column(width);
        let ceiling = column.bottom() + MARGIN;
        let readout = if shown { self.readout() } else { Vec::new() };
        let readout_bottom = controls::corner_button(pen, area).y - STACK_GAP;
        let mut floor = readout_bottom;
        if !readout.is_empty() {
            floor -= readout.len() as f32 * CORNER_LINE_HEIGHT + STACK_GAP;
        }
        let demo = self.demo.filter(|_| shown);
        let banner_room = demo.map_or(0.0, |demo| {
            demos::banner(pen, width, demo, start, floor).h + STACK_GAP
        });
        let panel = self.panel_shown();
        let strip =
            energy_strip::place(area, &self.simulation, floor, ceiling + banner_room, panel)
                .filter(|_| shown);
        let in_column = strip.filter(|card| card.x == column.x);
        let banner_bottom = in_column.map_or(floor, |card| card.y - STACK_GAP);
        let banner = demo.map(|demo| demos::banner(pen, width, demo, start, banner_bottom));
        if self.labels_grown > 0.0 && shown {
            let cards: Vec<Rect> = banner.into_iter().chain(strip).collect();
            let grown = motion::smooth(self.labels_grown);
            scene_labels::draw(pen, &self.simulation, view, area, &cards, grown);
        }
        let (panel_shift, sliders_shift) = self.drawer_shifts(width, area.height);
        if let Some(shift) = panel_shift.filter(|_| shown) {
            let panel_area = Rect::new(0.0, 0.0, PANEL_WIDTH, area.height);
            if pen.is_warming() {
                if self.panel_layer.will_paint(panel_area, panel_stale) {
                    self.panel.draw(pen, &self.simulation, status, area.height);
                }
            } else {
                draw_moved(Vec2::ZERO, 1.0, vec2(shift, 0.0));
                // Settled out, the card and its tab are one shape.
                let card = controls::panel_card(area.height);
                let tab = drawer::tab(card, Edge::Left, width);
                drawer::draw_card(pen, card, Edge::Left, (shift == 0.0).then_some(tab));
                let (panel, simulation) = (&mut self.panel, &self.simulation);
                self.panel_layer.draw(panel_area, panel_stale, shift, || {
                    panel.draw(&mut Pen::screen(), simulation, status, area.height);
                });
                draw_to_screen();
            }
        }
        let motion = ControlMotion {
            panel_shift,
            sliders_shift,
            panel_glow: motion::smooth(self.panel_glow),
            sliders_glow: motion::smooth(self.sliders_glow),
            menu: self.menu_dropped,
        };
        self.controls.draw(pen, area, self.button_states(), motion);
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
            // Like a game's text box, it sweeps in from the right with a
            // slight bounce, then its words fade in.
            let age = self.demo_age;
            let swept = motion::bounce(age / BANNER_SWEEP_SECONDS, BANNER_OVERSHOOT);
            let shift = (1.0 - swept) * (width - column.x);
            let fade = motion::smooth((age - BANNER_WORDS_DELAY) / BANNER_FADE_SECONDS);
            demos::draw_banner(pen, width, demo, start, fade, banner_bottom, shift);
        }
        self.draw_readout(pen, &readout, column, readout_bottom);
        // The keys list grows down out of the Keys toggle.
        let grown = motion::smooth(self.keys_grown);
        if grown > 0.0 {
            let list_top = TOGGLES_BOTTOM + MARGIN / 2.0;
            let middle = controls::toggles_middle(area);
            let room = (
                area.width - 2.0 * MARGIN,
                area.height - TOGGLES_BOTTOM - SHORTCUTS_BOTTOM_ROOM,
            );
            let toggle = controls::place_buttons(pen, area, self.button_states())
                .into_iter()
                .find(|placed| placed.action == Action::ToggleKeys)
                .map_or(vec2(middle, list_top), |placed| {
                    vec2(placed.rect.center().x, placed.rect.bottom())
                });
            if pen.live() {
                draw_moved(toggle, grown, Vec2::ZERO);
            }
            shortcuts::draw(pen, middle, list_top, room.0, room.1);
            if pen.live() {
                draw_to_screen();
            }
        }
    }

    /// The readout's lines: the frame rate and physics time in the Advanced
    /// view, and the warning while it is held, each with its color.
    fn readout(&self) -> Vec<(String, Color)> {
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
        stats.into_iter().chain(warning).collect()
    }

    /// Draws the readout's `lines` right-aligned in `column`, the last
    /// one's lowest point on `bottom`.
    fn draw_readout(&self, pen: &mut Pen, lines: &[(String, Color)], column: Rect, bottom: f32) {
        let count = lines.len() as f32;
        let mut y = bottom - (count - 1.0) * CORNER_LINE_HEIGHT - READOUT_DESCENT;
        for (text, color) in lines {
            let size = pen.fitted_size(text, column.w, CORNER_FONT_SIZE);
            if self.scenery == Scenery::Sky {
                let width = pen.measure(text, size);
                backing(pen, column.right() - width, y, width, size);
            }
            pen.text_right(text, column.right(), y, size, *color);
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
