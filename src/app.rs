//! The main loop. Each frame reads the input, steps the simulation, updates
//! the panel and camera, and draws the background, then the scene with the
//! panel on the left and the sliders on the right, and the buttons over the
//! scene. Under the toggles at the scene's top left go the frame rate (in
//! the Advanced view) and the warning when physics time is dropped.

use macroquad::prelude::{
    Color,
    MouseButton,
    Rect,
    Vec2,
    clear_background,
    get_frame_time,
    is_mouse_button_down,
    is_mouse_button_pressed,
    mouse_position,
    next_frame,
    screen_dpi_scale,
    screen_height,
    screen_width, //
};

use crate::graphics::{
    Scene,
    Scenery, //
};
use crate::sim::engine::ConnectedChambers;
use crate::sim::geometry::ChamberGeometry;
use crate::sim::pistons::KickDirection;
use crate::sim::units::{
    DEFAULT_INITIAL_CONCENTRATION_MOL_PER_L,
    MAX_INITIAL_CONCENTRATION_MOL_PER_L,
    MIN_INITIAL_CONCENTRATION_MOL_PER_L, //
};
use crate::ui::camera::{
    OrbitCamera,
    Pointer, //
};
use crate::ui::controls::{
    Action,
    BACKING,
    BACKING_RADIUS,
    ButtonStates,
    Controls,
    MARGIN,
    SceneArea,
    TOGGLES_BOTTOM, //
};
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
    CONTROL_RIGHT_MARGIN,
    PANEL_WIDTH,
    scene_left,
    scene_span, //
};
use crate::ui::panel::{
    self,
    EnginePanel,
    PanelPointer, //
};
use crate::ui::pen::Pen;
use crate::ui::shortcuts;
use crate::ui::start_prompt::StartPrompt;

/// The background's middle tone: the plain background where its shader
/// cannot be built, and the color distant particles fade toward.
const BACKGROUND_COLOR: Color = Color::from_rgba(12, 18, 30, 255);
/// Baseline of the build label above the bottom of the window.
const BUILD_LABEL_BOTTOM_Y: f32 = 24.0;
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
const BUILD_LABEL_FONT_SIZE: f32 = 14.0;
const BUILD_LABEL_COLOR: Color = Color::from_rgba(120, 140, 160, 255);
/// Corner readout: its first baseline, under the toggles, and its line
/// spacing.
const CORNER_TOP_Y: f32 = TOGGLES_BOTTOM + 24.0;
const CORNER_FONT_SIZE: f32 = 16.0;
const CORNER_LINE_HEIGHT: f32 = 20.0;
const CORNER_TEXT_COLOR: Color = Color::from_rgba(150, 165, 180, 255);
/// The panel's warning color.
const WARNING_COLOR: Color = Color::from_rgba(255, 130, 110, 255);
/// Clock resolution used for the startup seed, in ticks per second.
const SEED_TICKS_PER_SECOND: f64 = 1.0e6;
/// Fills chosen with keys 1 to 5: 256, 512, 1,024, 1,536 and 2,048 particles.
const CONCENTRATION_PRESETS: [f32; 5] = [
    MIN_INITIAL_CONCENTRATION_MOL_PER_L,
    0.25,
    DEFAULT_INITIAL_CONCENTRATION_MOL_PER_L,
    0.75,
    MAX_INITIAL_CONCENTRATION_MOL_PER_L,
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

fn initial_camera(geometry: ChamberGeometry) -> OrbitCamera {
    OrbitCamera::with_scale(
        CAMERA_FRAMING_MARGIN * geometry.cylinder_radius
            / ChamberGeometry::scaled(1.0).cylinder_radius,
    )
}

/// Seed from the wall clock, so every launch starts from a different gas.
/// `miniquad::date::now` works natively and in the browser.
fn clock_seed() -> u64 {
    (macroquad::miniquad::date::now() * SEED_TICKS_PER_SECOND) as u64
}

/// Runs the application: a default fill at 300 K with both reservoirs at
/// 300 K and the flywheel at rest until the first Start.
pub async fn run() {
    macroquad::rand::srand(clock_seed());
    let mut app = App::new();
    loop {
        app.frame();
        next_frame().await;
    }
}

struct App {
    simulation: ConnectedChambers,
    paused: bool,
    slow: bool,
    /// Tracers stay on through refills, which choose new particles.
    tracers: bool,
    show_keys: bool,
    /// The sky with a cloud to stand on, or the dark grid.
    scenery: Scenery,
    start_prompt: StartPrompt,
    panel: EnginePanel,
    /// The panel as last painted, repainted only when it changes.
    panel_layer: CachedLayer,
    show_panel: bool,
    camera: OrbitCamera,
    scene: Scene,
    controls: Controls,
    frame_stats: FrameStats,
}

impl App {
    fn new() -> Self {
        fonts::load();
        let simulation =
            ConnectedChambers::with_initial_concentration(DEFAULT_INITIAL_CONCENTRATION_MOL_PER_L);
        Self {
            panel: EnginePanel::new(),
            panel_layer: CachedLayer::new(),
            show_panel: true,
            camera: initial_camera(simulation.geometry()),
            scene: Scene::new(&simulation),
            controls: Controls::new(
                simulation.hot_kelvin(),
                simulation.cold_kelvin(),
                DEFAULT_INITIAL_CONCENTRATION_MOL_PER_L,
            ),
            paused: false,
            slow: false,
            tracers: false,
            show_keys: false,
            scenery: Scenery::default(),
            start_prompt: StartPrompt::new(),
            frame_stats: FrameStats::new(),
            simulation,
        }
    }

    /// Where the buttons go this frame.
    fn scene_area(&self) -> SceneArea {
        let (left, width) = scene_span(screen_width(), self.show_panel);
        SceneArea {
            edge: scene_left(self.show_panel),
            left,
            width,
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
            start: self.start_prompt.shown(),
            no_gap: self.simulation.hot_kelvin() <= self.simulation.cold_kelvin(),
            sky: self.scenery == Scenery::Sky,
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

    fn frame(&mut self) {
        let keys = KeyCommands::read();
        let wheel = wheel_pixels();
        let events = self
            .controls
            .update(self.scene_area(), self.button_states());
        let panel_blocks = self.show_panel
            && self.panel.handle_pointer(PanelPointer {
                position: Vec2::from(mouse_position()),
                wheel_pixels: wheel,
                pressed: is_mouse_button_pressed(MouseButton::Left),
                down: is_mouse_button_down(MouseButton::Left),
            });

        if keys.reset_gas || events.action == Some(Action::ResetGas) {
            self.simulation.reset();
        }
        // Space does what the shared button shows: Start while it is
        // offered (which also lifts a pause), Pause or Resume otherwise.
        let offered = self.start_prompt.shown();
        if events.action == Some(Action::Start) || (keys.go && offered) {
            self.paused = false;
            self.simulation.start_engine();
        } else if events.action == Some(Action::TogglePause) || keys.go {
            self.paused = !self.paused;
        }
        if keys.slow || events.action == Some(Action::ToggleSlow) {
            self.slow = !self.slow;
        }
        if keys.tracers || events.action == Some(Action::ToggleTracers) {
            self.tracers = !self.tracers;
            if !self.tracers {
                self.simulation.stop_following();
            }
        }
        if keys.kick {
            self.simulation.kick_flywheel(KickDirection::Negative);
        }
        if keys.details {
            self.panel.toggle_mode();
        }
        if keys.panel || events.action == Some(Action::TogglePanel) {
            self.show_panel = !self.show_panel;
        }
        if keys.keys || events.action == Some(Action::ToggleKeys) {
            self.show_keys = !self.show_keys;
        }
        if keys.scenery || events.action == Some(Action::ToggleScenery) {
            self.scenery = self.scenery.toggled();
        }
        if keys.camera_reset {
            self.camera.reset();
        }
        if let Some(preset) = keys.fill_preset {
            let concentration = CONCENTRATION_PRESETS[preset];
            self.simulation.set_initial_concentration(concentration);
            self.controls.set_concentration(concentration);
        }
        if let Some(concentration) = events.concentration {
            self.simulation.set_initial_concentration(concentration);
        }
        if let Some(kelvin) = events.hot_kelvin {
            self.simulation.set_hot_kelvin(kelvin);
        }
        if let Some(kelvin) = events.cold_kelvin {
            self.simulation.set_cold_kelvin(kelvin);
        }

        let physics_start = macroquad::miniquad::date::now();
        // A refill (reset, new fill) clears the followed particles.
        if self.tracers && self.simulation.tracers().is_none() {
            self.simulation
                .follow_particles(FOLLOWED_PARTICLES, macroquad::rand::gen_range(0, u64::MAX));
        }
        let pace = if self.slow { SLOW_MOTION_FACTOR } else { 1.0 };
        let limited = !self.paused && self.simulation.update(pace * get_frame_time());
        let physics_seconds = macroquad::miniquad::date::now() - physics_start;
        let now = macroquad::time::get_time();
        self.frame_stats.record(now, physics_seconds, limited);
        self.start_prompt
            .update(self.simulation.power_pistons().start_speed_fraction(), now);
        if self.show_panel {
            self.panel.update(&self.simulation, now);
        }

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

    fn draw(&mut self) {
        let area = self.scene_area();
        // The scene fills the window, behind the panel and the sliders; the
        // camera keeps the engine at the window's middle.
        let camera = self.camera.camera();
        clear_background(BACKGROUND_COLOR);
        self.scene.render(
            &self.simulation,
            &camera,
            self.scenery,
            BACKGROUND_COLOR,
            macroquad::time::get_time(),
            screen_width(),
            screen_height(),
            screen_dpi_scale(),
        );

        // Measure every string first, then draw (see `Pen` for why).
        let panel_stale = self.show_panel && self.panel.needs_paint(self.status(), area.height);
        self.draw_overlay(&mut Pen::warm(), area, panel_stale);
        self.draw_overlay(&mut Pen::screen(), area, panel_stale);
    }

    /// Everything over the scene: the panel, the corner readout, the build
    /// label, the controls and the keys list.
    fn draw_overlay(&mut self, pen: &mut Pen, area: SceneArea, panel_stale: bool) {
        let status = self.status();
        if self.show_panel {
            let panel_area = Rect::new(0.0, 0.0, PANEL_WIDTH, area.height);
            pen.rect(0.0, 0.0, PANEL_WIDTH, area.height, panel::BACKGROUND);
            if pen.is_warming() {
                if self.panel_layer.will_paint(panel_area, panel_stale) {
                    self.panel.draw(pen, &self.simulation, status);
                }
            } else {
                let (panel, simulation) = (&mut self.panel, &self.simulation);
                self.panel_layer.draw(panel_area, panel_stale, || {
                    panel.draw(&mut Pen::screen(), simulation, status)
                });
            }
        }
        let label_right = screen_width() - CONTROL_RIGHT_MARGIN;
        let label_y = screen_height() - BUILD_LABEL_BOTTOM_Y;
        if self.scenery == Scenery::Sky {
            let width = pen.measure(BUILD_LABEL, BUILD_LABEL_FONT_SIZE);
            backing(
                pen,
                label_right - width,
                label_y,
                width,
                BUILD_LABEL_FONT_SIZE,
            );
        }
        pen.text_right(
            BUILD_LABEL,
            label_right,
            label_y,
            BUILD_LABEL_FONT_SIZE,
            BUILD_LABEL_COLOR,
        );
        self.controls.draw(pen, area, self.button_states());
        // The keys list drops down under the toggles; the corner readout
        // goes under the list while it is open.
        let mut corner_top = CORNER_TOP_Y;
        if self.show_keys {
            let list_top = TOGGLES_BOTTOM + MARGIN / 2.0;
            let (_, height) = shortcuts::draw(
                pen,
                area.edge + MARGIN,
                list_top,
                area.right() - area.edge - 2.0 * MARGIN,
                area.height - TOGGLES_BOTTOM - SHORTCUTS_BOTTOM_ROOM,
            );
            corner_top = list_top + height + CORNER_LINE_HEIGHT;
        }
        self.draw_corner(pen, area, corner_top);
    }

    /// The frame rate and physics time in the Advanced view, and the
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
            .limited(macroquad::time::get_time())
            .then(|| (LIMITED_TEXT.to_owned(), WARNING_COLOR));
        let mut y = top;
        let room = area.right() - area.edge - 2.0 * MARGIN;
        for (text, color) in stats.into_iter().chain(warning) {
            if self.scenery == Scenery::Sky {
                let size = pen.fitted_size(&text, room, CORNER_FONT_SIZE);
                let width = pen.measure(&text, size);
                backing(pen, area.edge + MARGIN, y, width, size);
            }
            pen.fitted(&text, area.edge + MARGIN, y, room, CORNER_FONT_SIZE, color);
            y += CORNER_LINE_HEIGHT;
        }
    }
}

/// A shade behind one line of text at `x` with its baseline at `y`, so it
/// reads over the bright sky.
fn backing(pen: &Pen, x: f32, y: f32, width: f32, size: f32) {
    const PAD: f32 = 7.0;
    pen.round_rect(
        x - PAD,
        y - size - PAD / 2.0,
        width + 2.0 * PAD,
        size * 1.35 + PAD,
        BACKING_RADIUS,
        BACKING,
    );
}

#[cfg(test)]
#[path = "../tests/unit/app.rs"]
mod tests;
