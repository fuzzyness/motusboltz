//! The main loop.
//!
//! Each frame reads the input, drags the first-person figure, turns any
//! grabbed crank or valve or presses
//! a button or the regenerator's cartridge, steps the
//! simulation with the walls moving toward their settings, updates the
//! panel and camera, and draws the background, then the scene with its labels, the
//! panel on the left and the sliders on the right with the demo's banner
//! under them, and the buttons over the scene. Under the toggles at the
//! scene's top left go the frame rate (in the Advanced view) and the
//! warning when physics time is dropped.
//!
//! In first person, the frame turns the figure's head with the captured
//! mouse instead, and draws the scene from its eyes with only a crosshair
//! over it.
//!
//! In either view Esc opens the menu (see `menu`), which takes the mouse
//! while the scene runs on behind it.

use macroquad::miniquad::{
    date,
    window, //
};
use std::f32::consts::TAU;

use macroquad::prelude::{
    Camera,
    Camera3D,
    Color,
    KeyCode,
    Mat4,
    MouseButton,
    Rect,
    Vec2,
    Vec3,
    get_frame_time,
    is_key_pressed,
    is_mouse_button_down,
    is_mouse_button_pressed,
    mouse_position,
    next_frame,
    screen_height,
    screen_width,
    set_cursor_grab,
    show_mouse,
    vec2,
    vec3, //
};
use macroquad::rand::{
    gen_range,
    srand, //
};
use macroquad::time::get_time;

use crate::graphics::assembly::hand_crank;
use crate::graphics::block::BlockShape;
use crate::graphics::buttons::{
    DomeStates,
    button_boxes, //
};
use crate::graphics::cartridge;
use crate::graphics::table;
use crate::graphics::valves::valve_wheels;
use crate::graphics::{
    Dissolve,
    Scene,
    Scenery, //
};
use crate::sim::engine::ConnectedChambers;
use crate::sim::geometry::ChamberGeometry;
use crate::sim::pistons::FORWARD;
use crate::sim::reservoir::ReservoirSide;
use crate::sim::units::DEFAULT_CONCENTRATION;
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
    Slide,
    TabLook, //
};
use crate::ui::energy_strip;
use crate::ui::figure::{self, Drag};
use crate::ui::first_person::{
    self,
    Walker, //
};
use crate::ui::fonts;
use crate::ui::frame_stats::{
    FrameStats,
    LIMITED_TEXT, //
};
use crate::ui::handles::{
    Grab,
    Handle,
    Ray,
    VALVE_TURNS,
    pick,
    ramped,
    valve_kelvin,
    valve_opening,
    valve_turn,
    wheel_angle, //
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
use crate::ui::menu::{
    Item,
    Menu,
    Outcome, //
};
use crate::ui::motion;
use crate::ui::page;
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
const FOLLOWED_PARTICLES: usize = 6;
/// Longest frame time the animations step over, so a stalled frame does
/// not skip one.
const MAX_ANIMATION_DT: f32 = 0.1;
/// Seconds the labels take to grow out of their dots or shrink into them.
const LABELS_SECONDS: f32 = 0.3;
/// Seconds after the regenerator is taken out before the cartridge's label
/// may show: its trip back to the table, and a short pause once it lands.
const CARTRIDGE_TAG_DELAY: f32 = cartridge::TRIP_SECONDS + 0.5;
/// Seconds a stuck drawer's glow takes to fade in or out.
const GLOW_SECONDS: f32 = 0.4;
/// Seconds the keys list takes to grow out of the Keys toggle or shrink
/// back into it.
const KEYS_SECONDS: f32 = 0.2;
/// Seconds the Demos menu takes to drop all its items or fold them back.
const MENU_SECONDS: f32 = 0.3;
/// Seconds into the banner's slide when its words start to fade in.
const BANNER_WORDS_DELAY: f32 = 0.25;
/// The banner's tab's word.
const BANNER_TAB: &str = "Demo";
/// Seconds the banner's words take to fade in.
const BANNER_FADE_SECONDS: f32 = 0.4;
/// How the view dissolves into first person or back out: a little slower
/// and chunkier than a switch of scenery, rippling in waves.
const FIRST_PERSON_DISSOLVE: Dissolve = Dissolve {
    seconds: 0.80,
    cell: 2.5,
    waves: true,
};
/// The key that opens the menu.
const MENU_KEY: KeyCode = KeyCode::Escape;
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
    /// The hot and cold walls' settings, from the sliders or the valves;
    /// the walls move toward them (see `handles::ramped`).
    settings: (f32, f32),
    /// The first-person figure, while it is dragged toward the table.
    drag: Option<Drag>,
    /// The figure standing on the table, while the view is its own.
    walker: Option<Walker>,
    /// The menu Esc opens.
    menu: Menu,
    /// The crank, valve, button or cartridge held with the mouse, if any.
    grab: Option<Grab>,
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
    /// Seconds since the demo was set up, which fade its banner's words in.
    demo_age: f32,
    /// The demo's banner is open; its tab at the window's right edge opens
    /// and closes it, as the other drawers' do.
    show_banner: bool,
    /// The banner's slide out of the right edge.
    banner_slide: Slide,
    /// The banner's card as last drawn, open, if there is a demo.
    banner_card: Option<Rect>,
    /// How far each label has grown out of its dot, from 0 to 1, by key.
    tag_grown: Vec<(&'static str, f32)>,
    /// Seconds since the regenerator was last taken out, which the
    /// cartridge's label waits for (see `CARTRIDGE_TAG_DELAY`).
    cartridge_out: f32,
    /// How far each label has been moved aside to clear the others, eased.
    tag_shifts: scene_labels::Shifts,
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
    /// The same for the demo's banner.
    banner_glow: f32,
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
            // The readings and the settings start tucked away, so the engine
            // has the window; their tabs bring them out.
            show_panel: false,
            show_sliders: false,
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
            // The labels start off: each shows while its part is under the
            // pointer, and Labels shows them all.
            show_labels: false,
            show_demos: false,
            demo: Some(FIRST_DEMO),
            demo_age: 0.0,
            show_banner: true,
            banner_slide: Slide::new(true),
            banner_card: None,
            tag_grown: Vec::new(),
            cartridge_out: CARTRIDGE_TAG_DELAY,
            tag_shifts: scene_labels::Shifts::default(),
            keys_grown: 0.0,
            menu_dropped: 0.0,
            panel_slide: Slide::new(false),
            sliders_slide: Slide::new(false),
            panel_glow: 0.0,
            banner_glow: 0.0,
            sliders_glow: 0.0,
            scenery: Scenery::default(),
            start_prompt: StartPrompt::new(),
            frame_stats: FrameStats::new(),
            settings: (hot_kelvin, cold_kelvin),
            grab: None,
            drag: None,
            walker: None,
            menu: Menu::default(),
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

    /// Opens or closes the demo's banner when its tab is clicked at
    /// `pointer`, and returns whether the pointer is over the tab or the
    /// shown banner, where it should not turn the camera.
    fn use_banner_tab(&mut self, pointer: Vec2) -> bool {
        let width = screen_width();
        let Some(card) = self
            .banner_card
            .filter(|_| self.demo.is_some() && !self.hide_all)
        else {
            return false;
        };
        let on_tab = drawer::tab(card, Edge::Right, width).contains(pointer);
        if on_tab && is_mouse_button_pressed(MouseButton::Left) {
            self.show_banner = !self.show_banner;
        }
        let on_card = self.banner_slide.shown() && card.contains(pointer);
        on_tab || on_card
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
            sliders: self.show_sliders && !self.hide_all,
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
        // Esc opens the menu; while it is open, it has the mouse.
        if self.menu.open {
            self.use_menu();
        } else if is_key_pressed(MENU_KEY) {
            self.open_menu();
        }
        self.menu.animate(get_frame_time().min(MAX_ANIMATION_DT));
        if self.walker.is_some() {
            self.first_person_frame(keys);
            return;
        }
        if self.menu.open {
            // The scene runs on behind it.
            let dt = get_frame_time().min(MAX_ANIMATION_DT);
            self.animate(dt);
            self.step();
            self.draw();
            return;
        }
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
        let over_banner = self.use_banner_tab(pointer);
        let over_controls = self.controls.blocks_camera()
            || panel_blocks
            || self.over_drawer(pointer)
            || over_banner;
        let button = (
            is_mouse_button_down(MouseButton::Left),
            is_mouse_button_pressed(MouseButton::Left),
        );
        // The figure, held, takes the pointer from the parts and the view.
        let dragging = self.drag_figure(pointer, button, dt);
        let button = if dragging { (false, false) } else { button };
        self.turn_grabbed(pointer, button, over_controls, dt);
        self.apply(keys, events);
        self.animate(dt);
        self.step();
        self.camera.update(
            CameraKeys::read(),
            Pointer {
                enabled: !over_controls && self.grab.is_none() && !dragging,
                shift_down: shift_down(),
                wheel_pixels: wheel,
            },
            get_frame_time().min(MAX_CAMERA_DT),
            screen_height(),
        );
        self.draw();
    }

    /// Runs one frame in first person: the head turns with the captured
    /// mouse and the keys act as ever, unless the menu is open; the engine
    /// runs on either way.
    fn first_person_frame(&mut self, keys: KeyCommands) {
        let Some(mut walker) = self.walker else {
            return;
        };
        let dt = get_frame_time().min(MAX_ANIMATION_DT);
        let pointer = Vec2::from(mouse_position());
        let size = vec2(screen_width(), screen_height().max(1.0));
        if !self.menu.open {
            walker.look(pointer, self.menu.sensitivity);
            self.apply(keys, ControlEvents::default());
            // In the browser, a click captures the mouse again if it was
            // let go (see `use_menu`).
            page::arm_capture(true);
        }
        self.walker = Some(walker);
        self.animate(dt);
        self.step();
        let camera = walker.camera();
        self.render_scene(&camera);
        for pen in [&mut Pen::warm(), &mut Pen::screen()] {
            first_person::draw_view(pen, size.x, size.y);
            self.menu.draw(pen, size, pointer, true);
        }
    }

    /// Opens the menu; in first person, lets the mouse go.
    fn open_menu(&mut self) {
        self.menu.open();
        if let Some(walker) = self.walker.as_mut() {
            walker.release();
            capture_mouse(false);
        }
    }

    /// Takes the mouse for the open menu this frame and carries out what is
    /// pressed in it. Esc closes it too.
    fn use_menu(&mut self) {
        let first_person = self.walker.is_some();
        let pointer = Vec2::from(mouse_position());
        let size = vec2(screen_width(), screen_height().max(1.0));
        let button = (
            is_mouse_button_down(MouseButton::Left),
            is_mouse_button_pressed(MouseButton::Left),
        );
        let outcome = if is_key_pressed(MENU_KEY) {
            Some(Outcome::Resume)
        } else {
            self.menu.handle(pointer, button, size, first_person)
        };
        match outcome {
            Some(Outcome::ExitFirstPerson) => {
                self.menu.close();
                self.leave_first_person();
            }
            Some(Outcome::Resume) => {
                self.menu.close();
                if first_person {
                    capture_mouse(true);
                }
            }
            Some(Outcome::ExitSimulation) => window::order_quit(),
            None => {}
        }
        // In the browser, a press on Resume must capture the mouse from the
        // page's own handler of it (see `page`); so must any click on the
        // view in first person with the menu closed, in case the mouse was
        // let go (Esc closing the menu cannot capture it).
        let over_resume =
            self.menu.item_at(size.x, size.y, first_person, pointer) == Some(Item::Resume);
        page::arm_capture(self.walker.is_some() && (!self.menu.open || over_resume));
    }

    /// Picks up the figure pressed in its button at `pointer`, swings it
    /// along as the pointer moves over `dt` seconds, lights the table while
    /// it is over it, and drops it when the button is let go: onto the
    /// table, into first person, or anywhere else, back into its button.
    ///
    /// `button` is whether the left button is down, and whether it went
    /// down this frame. Returns whether the figure is held.
    fn drag_figure(&mut self, pointer: Vec2, button: (bool, bool), dt: f32) -> bool {
        let (down, pressed) = button;
        if self.drag.is_none() && self.grab.is_none() && pressed && !self.hide_all {
            let spot = figure::button_rect(&Pen::screen(), self.scene_area());
            if spot.contains(pointer) {
                self.drag = Some(Drag::new(pointer));
            }
        }
        let Some(mut drag) = self.drag else {
            self.scene.set_highlight(false);
            return false;
        };
        drag.follow(pointer, dt);
        let over = self.over_table(pointer);
        self.scene.set_highlight(over && down);
        // In the browser, the release must capture the mouse from the
        // page's own handler of it (see `page`).
        page::arm_capture(over && down);
        if down {
            self.drag = Some(drag);
        } else {
            self.drag = None;
            if over {
                self.enter_first_person();
            }
        }
        true
    }

    /// Whether `pointer` is over the table's top in the orbit view.
    fn over_table(&self, pointer: Vec2) -> bool {
        let matrix = self.camera.camera().matrix();
        let size = vec2(screen_width(), screen_height().max(1.0));
        let shape = BlockShape::new(self.simulation.geometry());
        let top = vec3(0.0, table::top_height(shape), 0.0);
        Ray::through(matrix, size, pointer)
            .and_then(|ray| ray.meet(top, Vec3::Y))
            .is_some_and(|point| table::over_top(shape, vec2(point.x, point.z)))
    }

    /// Dissolves into the figure's eyes, standing on the table, and
    /// captures the mouse.
    fn enter_first_person(&mut self) {
        let shape = BlockShape::new(self.simulation.geometry());
        self.walker = Some(Walker::standing(shape));
        self.grab = None;
        // The keys list and the Demos menu belong to the orbit view; they
        // are put away rather than found still open on the way back.
        self.show_keys = false;
        self.show_demos = false;
        self.scene.set_highlight(false);
        self.scene.dissolve(FIRST_PERSON_DISSOLVE);
        page::set_first_person(true);
        page::arm_capture(false);
        capture_mouse(true);
    }

    /// Dissolves back to the orbit view as it was left, and frees the mouse.
    fn leave_first_person(&mut self) {
        self.walker = None;
        // Told first, so the page does not take the mouse let go below for
        // a wish to open the menu.
        page::set_first_person(false);
        page::arm_capture(false);
        self.scene.dissolve(FIRST_PERSON_DISSOLVE);
        capture_mouse(false);
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
            self.toggle_tracers();
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
        // E puts the regenerator's cartridge in or takes it out.
        let regenerator = keys
            .regenerator
            .then(|| !self.simulation.regenerator().is_enabled());
        if let Some(on) = regenerator {
            self.simulation.set_regenerator(on);
        }
        // A new fill, temperature or regenerator ends the demo, whose banner
        // would no longer be true.
        let changed = [events.concentration, events.hot_kelvin, events.cold_kelvin];
        if changed.iter().any(Option::is_some) || regenerator.is_some() {
            self.demo = None;
        }
        if let Some(concentration) = events.concentration {
            self.simulation.set_concentration(concentration);
        }
        if let Some(kelvin) = events.hot_kelvin {
            self.settings.0 = kelvin;
        }
        if let Some(kelvin) = events.cold_kelvin {
            self.settings.1 = kelvin;
        }
    }

    /// Grabs a crank, valve, button or cartridge pressed at `pointer`
    /// (unless the press is `over_controls`), presses a button or the
    /// cartridge at once, lets go when the button is up, and turns a crank
    /// or valve held with the pointer over `dt` seconds of wall time.
    ///
    /// `button` is whether the left button is down, and whether it went
    /// down this frame.
    fn turn_grabbed(&mut self, pointer: Vec2, button: (bool, bool), over_controls: bool, dt: f32) {
        let (down, pressed) = button;
        if !down {
            self.grab = None;
            return;
        }
        let matrix = self.camera.camera().matrix();
        let size = vec2(screen_width(), screen_height().max(1.0));
        let geometry = self.simulation.geometry();
        let pistons = self.simulation.power_pistons();
        let crank = hand_crank(geometry, pistons);
        let shape = BlockShape::new(geometry);
        let wheels = valve_wheels(shape);
        if self.grab.is_none() && pressed && !over_controls {
            let [reset, tracers] = button_boxes(shape);
            let cartridge = if self.simulation.regenerator().is_enabled() {
                cartridge::seat(shape)
            } else {
                cartridge::rest(shape)
            };
            let handles = [
                (Handle::Crank, crank.handle(pistons.angle())),
                (Handle::Valve(ReservoirSide::Hot), wheels[0].center),
                (Handle::Valve(ReservoirSide::Cold), wheels[1].center),
                (Handle::ResetButton, reset.dome_top()),
                (Handle::TracersButton, tracers.dome_top()),
                (Handle::Cartridge, cartridge),
            ];
            self.grab = pick(&handles, matrix, size, pointer).map(Grab::new);
            if let Some(handle) = self.grab.map(|grab| grab.handle).filter(|h| h.presses()) {
                self.press(handle);
            }
        }
        let Some(mut grab) = self.grab else {
            return;
        };
        // A press may have changed the engine; look again.
        let pistons = self.simulation.power_pistons();
        let ray = Ray::through(matrix, size, pointer);
        match grab.handle {
            Handle::Crank => {
                // The plane the handle sweeps, square to the shaft.
                let plane = crank.handle(pistons.angle());
                let point = ray.and_then(|ray| ray.meet(plane, Vec3::X));
                let turn = grab.turned(point.map(|point| crank.angle_of(point)));
                let pace = if self.slow { SLOW_MOTION_FACTOR } else { 1.0 };
                let model_dt = (dt * pace) as f64;
                // The shaft's angle falls as it turns forward.
                let speed = turn as f64 * FORWARD / model_dt;
                if model_dt > 0.0 && speed > 0.0 && self.simulation.crank_engine(speed) {
                    self.paused = false;
                }
            }
            Handle::Valve(side) => {
                let wheel = match side {
                    ReservoirSide::Hot => wheels[0],
                    ReservoirSide::Cold => wheels[1],
                };
                let point = ray.and_then(|ray| ray.meet(wheel.center, Vec3::Y));
                let turn = grab.turned(point.map(|point| wheel_angle(wheel.center, point)));
                if turn != 0.0 {
                    let setting = match side {
                        ReservoirSide::Hot => &mut self.settings.0,
                        ReservoirSide::Cold => &mut self.settings.1,
                    };
                    let opening = valve_opening(side, *setting) + turn / (VALVE_TURNS * TAU);
                    *setting = valve_kelvin(side, opening);
                    self.controls.set_kelvin(self.settings.0, self.settings.1);
                    // The demo's banner would no longer be true.
                    self.demo = None;
                }
            }
            // Pressed once when grabbed; held, they only keep the camera
            // still.
            Handle::ResetButton | Handle::TracersButton | Handle::Cartridge => {}
        }
        self.grab = Some(grab);
    }

    /// Does what pressing `handle` does: Reset refills the gas and Tracers
    /// turns the tracers on or off, as their keys would, and the cartridge
    /// puts the regenerator in or takes it out, as E would.
    fn press(&mut self, handle: Handle) {
        match handle {
            Handle::ResetButton => {
                self.simulation.reset();
                self.controls.press(Action::ResetGas);
            }
            Handle::TracersButton => self.toggle_tracers(),
            Handle::Cartridge => {
                let on = !self.simulation.regenerator().is_enabled();
                self.simulation.set_regenerator(on);
                // The demo's banner would no longer be true.
                self.demo = None;
            }
            Handle::Crank | Handle::Valve(_) => {}
        }
    }

    /// Turns the tracers on or off.
    fn toggle_tracers(&mut self) {
        self.tracers = !self.tracers;
        if !self.tracers {
            self.simulation.stop_following();
        }
    }

    /// Sets up `scenario`: its temperatures on the walls and the sliders,
    /// its regenerator in or out, and fresh gas with the flywheel at rest,
    /// waiting for Start.
    fn set_up(&mut self, scenario: Scenario) {
        let (hot_kelvin, cold_kelvin) = scenario.kelvin();
        self.settings = (hot_kelvin, cold_kelvin);
        self.simulation.set_hot_kelvin(hot_kelvin);
        self.simulation.set_cold_kelvin(cold_kelvin);
        self.simulation.set_regenerator(scenario.regenerator());
        self.simulation.reset();
        self.controls.set_kelvin(hot_kelvin, cold_kelvin);
        self.paused = false;
        self.show_demos = false;
        self.demo = Some(scenario);
        self.demo_age = 0.0;
        self.show_banner = true;
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
        self.cartridge_out = if self.simulation.regenerator().is_enabled() {
            0.0
        } else {
            self.cartridge_out + dt
        };
        self.grow_labels(pointer, dt);
        self.keys_grown = motion::advance(self.keys_grown, self.show_keys, dt, KEYS_SECONDS);
        self.menu_dropped = motion::advance(self.menu_dropped, self.show_demos, dt, MENU_SECONDS);
        self.demo_age += dt;
        self.tag_shifts.tick(dt);
        let banner = self.banner_card.filter(|_| self.demo.is_some() && shown);
        let (on_tab, on_card) = banner.map_or((false, false), |card| {
            (
                drawer::tab(card, Edge::Right, width).contains(pointer),
                drawer::reach(card, Edge::Right, width).contains(pointer),
            )
        });
        self.banner_slide
            .update(self.show_banner, on_tab, on_card, held, dt);
        let stuck = self.show_banner && self.banner_slide.settled_out();
        self.banner_glow = motion::advance(self.banner_glow, stuck, dt, GLOW_SECONDS);
    }

    /// How far the label keyed `key` has grown out of its dot.
    fn tag_growth(&self, key: &str) -> f32 {
        self.tag_grown
            .iter()
            .find(|(seen, _)| *seen == key)
            .map_or(0.0, |&(_, grown)| grown)
    }

    /// Grows each label out of its dot, or shrinks it back, over `dt`
    /// seconds: all of them while Labels is on, and otherwise the label of
    /// the part under `pointer`, while the pointer is free over the scene.
    /// The Demos menu puts them all away.
    fn grow_labels(&mut self, pointer: Vec2, dt: f32) {
        let tags = scene_labels::tags(&self.simulation);
        let free = self.walker.is_none()
            && !self.menu.open
            && self.drag.is_none()
            && self.grab.is_none()
            && !self.hide_all
            && !self.controls.blocks_camera()
            && !self.over_drawer(pointer);
        let size = vec2(screen_width(), screen_height().max(1.0));
        let hovered = if free {
            scene_labels::hovered(&tags, self.camera.camera().matrix(), size, pointer)
        } else {
            vec![false; tags.len()]
        };
        self.tag_grown = tags
            .iter()
            .zip(hovered)
            .map(|(tag, hovered)| {
                let shown = (self.show_labels || hovered) && !self.show_demos;
                // The cartridge's tag waits until it is back on the table.
                let landed = tag.key != scene_labels::CARTRIDGE_KEY
                    || self.cartridge_out >= CARTRIDGE_TAG_DELAY;
                let grown = self.tag_growth(tag.key);
                let shown = shown && landed;
                (tag.key, motion::advance(grown, shown, dt, LABELS_SECONDS))
            })
            .collect();
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
        if !self.paused {
            // The walls move toward their settings in model time.
            let dt = pace * get_frame_time().min(MAX_ANIMATION_DT);
            let hot = ramped(self.simulation.hot_kelvin(), self.settings.0, dt);
            let cold = ramped(self.simulation.cold_kelvin(), self.settings.1, dt);
            self.simulation.set_hot_kelvin(hot);
            self.simulation.set_cold_kelvin(cold);
        }
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
        self.render_scene(&camera);

        // Measure every string first, then draw (see `Pen` for why).
        let panel_stale =
            self.panel_visible() && self.panel.needs_paint(self.status(), area.height);
        let view = camera.matrix();
        self.draw_overlay(&mut Pen::warm(), area, view, panel_stale);
        self.draw_overlay(&mut Pen::screen(), area, view, panel_stale);
        if self.menu.visible() {
            let pointer = Vec2::from(mouse_position());
            let size = vec2(area.window_width, area.height);
            for pen in [&mut Pen::warm(), &mut Pen::screen()] {
                self.menu.draw(pen, size, pointer, false);
            }
        }
    }

    /// Draws the scene seen from `camera`, the valves, buttons and table as
    /// the controls stand.
    fn render_scene(&mut self, camera: &Camera3D) {
        self.scene.set_valve_turns((
            valve_turn(ReservoirSide::Hot, self.settings.0),
            valve_turn(ReservoirSide::Cold, self.settings.1),
        ));
        let held = |handle| self.grab.is_some_and(|grab| grab.handle == handle);
        self.scene.set_paused(self.paused);
        self.scene.set_switches(DomeStates {
            on: [false, self.tracers],
            held: [held(Handle::ResetButton), held(Handle::TracersButton)],
        });
        self.scene.render(&self.simulation, camera, self.scenery);
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
        let grown: Vec<f32> = scene_labels::tags(&self.simulation)
            .iter()
            .map(|tag| motion::smooth(self.tag_growth(tag.key)))
            .collect();
        if grown.iter().any(|&grown| grown > 0.0) && shown {
            let open_banner = banner.filter(|_| self.banner_slide.shown());
            let cards: Vec<Rect> = open_banner.into_iter().chain(strip).collect();
            let (simulation, shifts) = (&self.simulation, &mut self.tag_shifts);
            scene_labels::draw(pen, simulation, view, area, &cards, &grown, shifts);
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
        // The figure's button sits left of Hide all, and the build label
        // left of it, level with Hide all's text.
        let spot = figure::button_rect(pen, area);
        let hovered = spot.contains(Vec2::from(mouse_position()));
        figure::draw_button(
            pen,
            spot,
            hovered && self.drag.is_none(),
            self.drag.is_some(),
        );
        let corner = controls::corner_button(pen, area);
        let label_right = spot.x - BUILD_LABEL_GAP;
        let label_y = corner.y + (corner.h + pen.text_height("H", BUILD_LABEL_FONT_SIZE)) / 2.0;
        let size = BUILD_LABEL_FONT_SIZE;
        if self.scenery == Scenery::Sky {
            let width = pen.measure(BUILD_LABEL, size);
            backing(pen, label_right - width, label_y, width, size);
        }
        pen.text_right(BUILD_LABEL, label_right, label_y, size, BUILD_LABEL_COLOR);
        if let (Some(demo), Some(card)) = (self.demo, banner) {
            // A drawer at the right edge: it slides out with its words
            // fading in, and its tab there closes it or brings it back.
            if !pen.is_warming() {
                self.banner_card = Some(card);
            }
            let distance = drawer::slide_distance(card, Edge::Right, width);
            let tab = drawer::tab(card, Edge::Right, width);
            let shift = self.banner_slide.shift(Edge::Right, distance);
            if let Some(shift) = shift {
                let placed = Rect {
                    x: card.x + shift,
                    ..card
                };
                let joined = (shift == 0.0).then_some(tab);
                drawer::draw_card(pen, placed, Edge::Right, joined);
                let fade =
                    motion::smooth((self.demo_age - BANNER_WORDS_DELAY) / BANNER_FADE_SECONDS);
                demos::draw_banner(pen, width, demo, start, fade, banner_bottom, shift);
            }
            let look = TabLook {
                glow: motion::smooth(self.banner_glow),
                joined: (shift == Some(0.0)).then_some(card),
            };
            drawer::draw_tab(pen, tab, Edge::Right, width, BANNER_TAB, "", look);
        } else if !pen.is_warming() {
            self.banner_card = None;
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
        // The dragged figure over everything.
        if let Some(drag) = self.drag {
            drag.draw(pen);
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

/// Captures the mouse for looking around, hiding the pointer, or lets it
/// go.
fn capture_mouse(captured: bool) {
    set_cursor_grab(captured);
    show_mouse(!captured);
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
