//! The main loop. Each frame reads the input, steps the simulation, updates
//! the panel and camera, and draws the scene with the panel on the left and
//! the controls on the right. The scene's top-left corner carries the frame
//! rate (with Details on) and the warning when physics time is dropped.

use macroquad::prelude::{
    Color,
    MouseButton,
    Rect,
    Vec2,
    WHITE,
    clear_background,
    get_frame_time,
    is_mouse_button_down,
    is_mouse_button_pressed,
    mouse_position,
    next_frame,
    screen_dpi_scale,
    screen_height,
    screen_width,
    set_camera,
    set_default_camera, //
};

use crate::graphics::Scene;
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
    Controls, //
};
use crate::ui::frame_stats::{
    FrameStats,
    LIMITED_TEXT, //
};
use crate::ui::input::{
    CameraKeys,
    HELP_LINES,
    KeyCommands,
    shift_down,
    wheel_pixels, //
};
use crate::ui::layer::CachedLayer;
use crate::ui::layout::{
    CONTROL_RIGHT_MARGIN,
    PANEL_WIDTH,
    scene_span,
    scene_viewport, //
};
use crate::ui::panel::{
    EnginePanel,
    PanelPointer,
    TITLE, //
};
use crate::ui::pen::Pen;

const BACKGROUND_COLOR: Color = Color::from_rgba(12, 18, 30, 255);
const SCENE_TEXT_MARGIN: f32 = 20.0;
const TITLE_Y: f32 = 36.0;
const TITLE_FONT_SIZE: f32 = 28.0;
const HELP_FONT_SIZE: f32 = 16.0;
const HELP_LINE_HEIGHT: f32 = 20.0;
/// Baseline of the last help line above the bottom of the window.
const HELP_BOTTOM_Y: f32 = 24.0;
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
/// Corner readout: its first baseline, below the title when the panel is
/// hidden, and its line spacing.
const CORNER_TOP_Y: f32 = 30.0;
const CORNER_BELOW_TITLE_Y: f32 = TITLE_Y + 26.0;
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
/// Extra zoom-out so the cylinders, crank and flywheel all fit the first view.
const CAMERA_FRAMING_MARGIN: f32 = 1.25;

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
            frame_stats: FrameStats::new(),
            simulation,
        }
    }

    fn frame(&mut self) {
        let keys = KeyCommands::read();
        let wheel = wheel_pixels();
        let events = self.controls.update();
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
        if keys.pause || events.action == Some(Action::TogglePause) {
            self.paused = !self.paused;
        }
        if keys.start || events.action == Some(Action::Start) {
            self.simulation.start_engine();
        }
        if keys.kick {
            self.simulation.kick_flywheel(KickDirection::Negative);
        }
        if keys.details || events.action == Some(Action::ToggleDetails) {
            self.panel.toggle_details();
        }
        if keys.panel {
            self.show_panel = !self.show_panel;
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
        let limited = !self.paused && self.simulation.update(get_frame_time());
        let physics_seconds = macroquad::miniquad::date::now() - physics_start;
        let now = macroquad::time::get_time();
        self.frame_stats.record(now, physics_seconds, limited);
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
        let (scene_left, scene_width) = scene_span(screen_width(), self.show_panel);
        let scene_height = screen_height().max(1.0);
        let mut camera = self.camera.camera();
        camera.viewport = Some(scene_viewport(
            screen_width(),
            scene_height,
            self.show_panel,
            screen_dpi_scale(),
        ));
        camera.aspect = Some(scene_width / scene_height);
        clear_background(BACKGROUND_COLOR);
        set_camera(&camera);
        self.scene.draw(&self.simulation, &camera, BACKGROUND_COLOR);
        set_default_camera();

        let mut pen = Pen::screen();
        if self.show_panel {
            let stale = self.panel.needs_paint(self.paused, scene_height);
            let (panel, simulation, paused) = (&mut self.panel, &self.simulation, self.paused);
            self.panel_layer.draw(
                Rect::new(0.0, 0.0, PANEL_WIDTH, scene_height),
                stale,
                || panel.draw(simulation, paused),
            );
        } else {
            pen.text(TITLE, SCENE_TEXT_MARGIN, TITLE_Y, TITLE_FONT_SIZE, WHITE);
        }
        self.draw_corner(&mut pen, scene_left, scene_width);
        let mut help_y = screen_height() - HELP_BOTTOM_Y - HELP_LINE_HEIGHT;
        for line in HELP_LINES {
            pen.fitted(
                line,
                scene_left + SCENE_TEXT_MARGIN,
                help_y,
                scene_width - 2.0 * SCENE_TEXT_MARGIN,
                HELP_FONT_SIZE,
                WHITE,
            );
            help_y += HELP_LINE_HEIGHT;
        }
        let label_width = pen.measure(BUILD_LABEL, BUILD_LABEL_FONT_SIZE);
        pen.text(
            BUILD_LABEL,
            screen_width() - CONTROL_RIGHT_MARGIN - label_width,
            screen_height() - HELP_BOTTOM_Y,
            BUILD_LABEL_FONT_SIZE,
            BUILD_LABEL_COLOR,
        );
        self.controls.draw(self.paused, self.panel.details_shown());
    }

    /// The frame rate and physics time while Details is on, and the
    /// warning while it is held, at the scene's top-left in every view.
    fn draw_corner(&self, pen: &mut Pen, scene_left: f32, scene_width: f32) {
        let stats = self
            .panel
            .details_shown()
            .then(|| self.frame_stats.line())
            .flatten()
            .map(|line| (line, CORNER_TEXT_COLOR));
        let warning = self
            .frame_stats
            .limited(macroquad::time::get_time())
            .then(|| (LIMITED_TEXT.to_owned(), WARNING_COLOR));
        let mut y = if self.show_panel {
            CORNER_TOP_Y
        } else {
            CORNER_BELOW_TITLE_Y
        };
        for (text, color) in stats.into_iter().chain(warning) {
            pen.fitted(
                &text,
                scene_left + SCENE_TEXT_MARGIN,
                y,
                scene_width - 2.0 * SCENE_TEXT_MARGIN,
                CORNER_FONT_SIZE,
                color,
            );
            y += CORNER_LINE_HEIGHT;
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/app.rs"]
mod tests;
