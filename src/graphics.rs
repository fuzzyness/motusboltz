//! 3D drawing of the particles, tracer trails, cylinders, the opening
//! between them, the pistons, rods,
//! crankshaft and flywheel, the burner and the liquid-nitrogen coil that
//! stand for the two reservoirs, the valves that set them, the Reset and
//! Tracers buttons, the regenerator's cartridge, the table the engine
//! stands on, and the background.
//!
//! Drawing only; nothing here affects the physics.

pub mod assembly;
pub mod backdrop;
pub mod block;
pub mod buttons;
pub mod cartridge;
pub mod chamber;
pub mod cloud;
pub mod fire;
pub mod floor;
pub mod heat_sparks;
pub mod levels;
pub mod nitrogen;
pub mod outline;
pub mod particles;
pub mod soft_view;
pub mod solid;
pub mod sprites;
pub mod table;
pub mod trails;
pub mod valves;

use macroquad::miniquad::{
    BlendFactor,
    BlendState,
    BlendValue,
    Equation, //
};
use macroquad::models::{
    Mesh,
    draw_mesh, //
};
use macroquad::prelude::{
    Camera3D,
    Color,
    Comparison,
    Material,
    MaterialParams,
    PipelineParams,
    ShaderSource,
    UniformDesc,
    UniformType,
    Vec3,
    clear_background,
    gl_use_default_material,
    gl_use_material,
    load_material,
    screen_dpi_scale,
    screen_height,
    screen_width,
    vec2, //
};
use macroquad::time::get_time;

use crate::sim::engine::ConnectedChambers;
use crate::ui::draw_target::{
    draw_to,
    draw_to_screen, //
};
use assembly::Assembly;
use backdrop::Backdrop;
pub use backdrop::Scenery;
use block::BlockShape;
use buttons::{
    Buttons, //
    DomeStates,
};
use cartridge::Cartridge;
use chamber::ConnectedChamberVisual;
use cloud::CloudFloor;
use fire::Fire;
use floor::FloorGrid;
use heat_sparks::HeatSparks;
use levels::ReservoirLevels;
use nitrogen::Nitrogen;
use particles::{
    ParticleRenderer,
    camera_light_direction, //
};
use soft_view::{
    DITHER_CELL,
    SoftView,
    Wave, //
};
use sprites::{
    Facing,
    SpriteMaterials, //
};
use table::Table;
use trails::TrailRenderer;
use valves::Valves;

/// How one view dissolves into the next.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dissolve {
    /// Seconds it takes.
    pub seconds: f64,
    /// Side of the dither's cells, in points.
    pub cell: f32,
    /// Whether both views ripple in waves as it goes, most at its middle.
    pub waves: bool,
}

/// A switch of scenery's dissolve.
const SCENERY_DISSOLVE: Dissolve = Dissolve {
    seconds: SCENERY_DISSOLVE_SECONDS,
    cell: DITHER_CELL,
    waves: false,
};
/// Period of the waves' clock, in seconds, so the shader's floats stay
/// precise however long the app runs.
const WAVE_PERIOD: f64 = 1000.0;

/// Seconds the old scenery takes to dissolve into the new one.
const SCENERY_DISSOLVE_SECONDS: f64 = 0.35;
/// Macroquad's per-draw-call vertex buffer size, set in the window
/// configuration; every mesh must fit within it.
pub const DRAW_CALL_VERTEX_CAPACITY: usize = 16_384;
/// Macroquad's per-draw-call index buffer size (see
/// `DRAW_CALL_VERTEX_CAPACITY`).
pub const DRAW_CALL_INDEX_CAPACITY: usize = 60_000;

/// The background's middle tone.
///
/// It is the plain background where its shader cannot be built, and the
/// color distant particles fade toward over the grid.
pub const BACKGROUND: Color = Color::from_rgba(12, 18, 30, 255);

/// Vertex shader for surfaces lit per pixel.
///
/// It passes each point's world position and normal on to the fragment
/// shader, which reads the `Eye` and `Light` uniforms.
const LIT_VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
attribute vec4 normal;

varying lowp vec4 color;
varying lowp vec2 uv;
varying vec3 world;
varying vec3 surface_normal;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    vec4 placed = Model * vec4(position, 1.0);
    world = placed.xyz;
    surface_normal = normal.xyz;
    color = color0 / 255.0;
    uv = texcoord;
    gl_Position = Projection * placed;
}"#;

/// What distant particles fade toward in the sky: the haze near the
/// horizon, a little, so they stay bright.
const SKY_FADE: Color = Color::from_rgba(176, 204, 240, 255);

/// Radius and height, in model units, of a vertical cylinder centered on
/// the origin.
///
/// It is used for cylinder frames and for the depth-fade extent.
#[derive(Clone, Copy)]
pub struct CylinderDimensions {
    /// Radius.
    pub radius: f32,
    /// Height.
    pub height: f32,
}

/// Mixes two colors, clamping the interpolation fraction to [0, 1] and
/// keeping the first color's alpha.
fn mix_color(first: Color, second: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    Color {
        r: first.r + (second.r - first.r) * amount,
        g: first.g + (second.g - first.g) * amount,
        b: first.b + (second.b - first.b) * amount,
        a: first.a,
    }
}

/// A depth-tested material lit per pixel by `fragment`, with the `Eye` and
/// `Light` uniforms and the `extra` ones, or `None` if its shader cannot
/// be built.
fn load_lit(fragment: &'static str, extra: &[(&str, UniformType)]) -> Option<Material> {
    let pipeline_params = PipelineParams {
        depth_write: true,
        depth_test: Comparison::LessOrEqual,
        ..Default::default()
    };
    load_lit_with(fragment, extra, pipeline_params)
}

/// A see-through material lit per pixel by `fragment`, with the `Eye` and
/// `Light` uniforms: blended over what is drawn behind it by the alpha the
/// fragment gives, and hidden by what is in front. `None` if its shader
/// cannot be built.
///
/// It writes depth too, so it hides what is drawn after it behind it:
/// Miniquad tests depth only for pipelines that write it, and without the
/// test, glass and domes showed through the block.
fn load_clear(fragment: &'static str) -> Option<Material> {
    load_clear_with(fragment, &[])
}

/// A see-through material as `load_clear` makes, with the `extra` uniforms
/// too.
fn load_clear_with(fragment: &'static str, extra: &[(&str, UniformType)]) -> Option<Material> {
    let blend = BlendState::new(
        Equation::Add,
        BlendFactor::Value(BlendValue::SourceAlpha),
        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
    );
    let pipeline_params = PipelineParams {
        depth_write: true,
        depth_test: Comparison::LessOrEqual,
        color_blend: Some(blend),
        ..Default::default()
    };
    load_lit_with(fragment, extra, pipeline_params)
}

/// A material lit per pixel by `fragment` with `pipeline_params`, the
/// `Eye` and `Light` uniforms and the `extra` ones, or `None` if its
/// shader cannot be built.
fn load_lit_with(
    fragment: &'static str,
    extra: &[(&str, UniformType)],
    pipeline_params: PipelineParams,
) -> Option<Material> {
    let lighting = [("Eye", UniformType::Float3), ("Light", UniformType::Float3)];
    let uniforms = lighting
        .iter()
        .chain(extra)
        .map(|&(name, kind)| UniformDesc::new(name, kind))
        .collect();
    let source = ShaderSource::Glsl {
        vertex: LIT_VERTEX,
        fragment,
    };
    let params = MaterialParams {
        pipeline_params,
        uniforms,
        ..Default::default()
    };
    load_material(source, params).ok()
}

/// Draws `meshes` lit per pixel by `material`, seen from `camera` and lit
/// from `light`, or, with no material, in their own colors.
fn draw_lit<'a>(
    meshes: impl IntoIterator<Item = &'a Mesh>,
    material: Option<&Material>,
    camera: &Camera3D,
    light: Vec3,
) {
    if let Some(material) = material {
        material.set_uniform("Eye", camera.position);
        material.set_uniform("Light", light);
        gl_use_material(material);
    }
    for mesh in meshes {
        draw_mesh(mesh);
    }
    gl_use_default_material();
}

/// Scales a color's red, green and blue by `brightness`, keeping alpha.
fn shade(color: Color, brightness: f32) -> Color {
    Color {
        r: color.r * brightness,
        g: color.g * brightness,
        b: color.b * brightness,
        a: color.a,
    }
}

/// Everything drawn in the 3D view, built once for the engine's geometry.
pub struct Scene {
    /// The cylinders and the opening between them.
    chamber: ConnectedChamberVisual,
    /// The see-through grid of the grid scenery.
    floor: FloorGrid,
    /// The wooden table the engine stands on.
    table: Table,
    /// The block, pistons, rods, crankshaft and flywheel.
    assembly: Assembly,
    /// The particles.
    particles: ParticleRenderer,
    /// The tracers' trails.
    trails: TrailRenderer,
    /// The burner.
    fire: Fire,
    /// The coil of liquid nitrogen.
    nitrogen: Nitrogen,
    /// The gas and nitrogen valves.
    valves: Valves,
    /// How far each valve's wheel is turned from shut, hot first, in
    /// radians counterclockwise seen from above.
    valve_turns: (f32, f32),
    /// The Reset and Tracers buttons.
    buttons: Buttons,
    /// Whether Reset and Tracers are on and their buttons held down.
    switches: DomeStates,
    /// The regenerator's cartridge, on the table or in the opening.
    cartridge: Cartridge,
    /// Twinkles where heat crosses the walls.
    heat_sparks: HeatSparks,
    /// `None` where the sprite shader cannot be built: the burner and nitrogen
    /// still show, without flames or mist.
    sprite_materials: Option<SpriteMaterials>,
    /// The sky or the dark background.
    backdrop: Backdrop,
    /// What the table stands on in the sky scenery.
    cloud: CloudFloor,
    /// The softened view the scene is drawn into.
    soft: SoftView,
    /// The view as it last stood in the other scenery, dissolving into
    /// the new one after a switch.
    previous: SoftView,
    /// The scenery last drawn, and when on the clock it was switched to.
    scenery: Option<(Scenery, f64)>,
    /// How strongly the burner, nitrogen and glows show, easing after the
    /// sliders.
    levels: ReservoirLevels,
    /// The table's top lights up, for dropping the first-person figure.
    highlight: bool,
    /// A dissolve from the last view to the next asked for (see
    /// `Dissolve`).
    dissolve: Option<Dissolve>,
    /// The engine is paused, which stills the flames, the sparks and the
    /// nitrogen's bubbles and smoke.
    paused: bool,
    /// The clock the flames, sparks, bubbles and smoke move by: wall time,
    /// stopped while the engine is paused.
    effects_seconds: f64,
    /// The wall clock at the last frame, if there was one.
    last_seconds: Option<f64>,
    /// The dissolve under way.
    dissolve_style: Dissolve,
}

impl Scene {
    /// Builds everything for the engine of `simulation`.
    pub fn new(simulation: &ConnectedChambers) -> Self {
        let geometry = simulation.geometry();
        let shape = BlockShape::new(geometry);
        let extent = CylinderDimensions {
            radius: geometry.cylinder_offset + geometry.cylinder_radius,
            height: geometry.grid_side(),
        };
        Self {
            chamber: ConnectedChamberVisual::new(geometry),
            floor: FloorGrid::new(table::ground(shape)),
            table: Table::new(shape),
            assembly: Assembly::new(geometry, simulation.power_pistons()),
            particles: ParticleRenderer::new(extent),
            trails: TrailRenderer::new(),
            fire: Fire::new(shape),
            nitrogen: Nitrogen::new(shape),
            valves: Valves::new(shape),
            valve_turns: (0.0, 0.0),
            buttons: Buttons::new(shape),
            switches: DomeStates::default(),
            cartridge: Cartridge::new(shape, simulation.regenerator().is_enabled()),
            heat_sparks: HeatSparks::new(geometry),
            sprite_materials: SpriteMaterials::load(),
            backdrop: Backdrop::new(geometry),
            cloud: CloudFloor::new(shape),
            soft: SoftView::new(),
            previous: SoftView::new(),
            scenery: None,
            levels: ReservoirLevels::default(),
            highlight: false,
            paused: false,
            effects_seconds: 0.0,
            last_seconds: None,
            dissolve: None,
            dissolve_style: SCENERY_DISSOLVE,
        }
    }

    /// Turns the valves' wheels, hot first, from shut, in radians
    /// counterclockwise seen from above, for the frames to come.
    pub fn set_valve_turns(&mut self, turns: (f32, f32)) {
        self.valve_turns = turns;
    }

    /// Lights the Reset and Tracers buttons while their switches are on
    /// and presses them in while held, as `states` says, for the frames to
    /// come.
    pub fn set_switches(&mut self, states: DomeStates) {
        self.switches = states;
    }

    /// Stills the flames, sparks and nitrogen while `paused`, for the frames
    /// to come.
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// Lights the table's top while `on`, for the frames to come.
    pub fn set_highlight(&mut self, on: bool) {
        self.highlight = on;
    }

    /// Dissolves the view as last drawn into the next one over `seconds`
    /// through the dither, as a switch of scenery does, with cells `cell`
    /// points wide.
    pub fn dissolve(&mut self, style: Dissolve) {
        self.dissolve = Some(style);
    }

    /// Draws the background and the engine for one frame, and leaves the
    /// screen camera set.
    ///
    /// The frame is drawn into a softened view (see `soft_view.rs`), or
    /// straight to the screen where that cannot be built. For a moment
    /// after the scenery switches, the last view of the old one dissolves
    /// over the new one through a dither. The clock eases
    /// the reservoir effects toward the sliders and animates the flames,
    /// mist and clouds, so they keep moving while the simulation is paused.
    pub fn render(&mut self, simulation: &ConnectedChambers, camera: &Camera3D, scenery: Scenery) {
        let (width, height, dpi) = (screen_width(), screen_height(), screen_dpi_scale());
        let seconds = get_time();
        // A switch of scenery, or a dissolve asked for, keeps the old view,
        // drawn last frame, and draws the new one into the other.
        match self.scenery {
            Some((shown, _)) if shown != scenery || self.dissolve.is_some() => {
                std::mem::swap(&mut self.soft, &mut self.previous);
                self.scenery = Some((scenery, seconds));
                self.dissolve_style = self.dissolve.take().unwrap_or(SCENERY_DISSOLVE);
            }
            None => self.scenery = Some((scenery, f64::NEG_INFINITY)),
            Some(_) => {}
        }
        clear_background(BACKGROUND);
        self.levels
            .follow(simulation.hot_kelvin(), simulation.cold_kelvin(), seconds);
        // Macroquad's camera is not `Clone`; copy it field by field.
        let mut camera = Camera3D {
            position: camera.position,
            target: camera.target,
            up: camera.up,
            fovy: camera.fovy,
            aspect: camera.aspect,
            projection: camera.projection,
            render_target: None,
            viewport: camera.viewport,
            z_near: camera.z_near,
            z_far: camera.z_far,
        };
        if let Some(target) = self.soft.target(width, height, dpi) {
            draw_to(&SoftView::flat_camera(&target, width, height));
            clear_background(BACKGROUND);
            camera.render_target = Some(target);
        }
        let size = vec2(width, height);
        self.backdrop
            .draw(scenery, &camera, size, self.levels, seconds);
        draw_to(&camera);
        let fade = match scenery {
            Scenery::Sky => SKY_FADE,
            Scenery::Grid => BACKGROUND,
        };
        self.draw(simulation, &camera, scenery, fade, seconds);
        draw_to_screen();
        let since = self.scenery.map_or(f64::INFINITY, |(_, at)| seconds - at);
        let style = self.dissolve_style;
        let dissolved = (since / style.seconds) as f32;
        // Waves swell and settle over the dissolve, strongest halfway.
        let wave = Wave {
            strength: if style.waves && dissolved < 1.0 {
                (std::f32::consts::PI * dissolved).sin()
            } else {
                0.0
            },
            time: (seconds % WAVE_PERIOD) as f32,
        };
        self.soft.present(width, height, wave);
        if dissolved < 1.0 {
            self.previous
                .present_dithered(width, height, dpi, style.cell, 1.0 - dissolved, wave);
        }
    }

    /// Draws the engine as it is now in `scenery`.
    ///
    /// The 3D camera must already be set; distant particles fade toward
    /// `background`. The flames and mist are animated by `seconds`, the
    /// clock, so they keep moving while the simulation is paused.
    fn draw(
        &mut self,
        simulation: &ConnectedChambers,
        camera: &Camera3D,
        scenery: Scenery,
        background: Color,
        seconds: f64,
    ) {
        let facing = Facing::toward(camera);
        let light = camera_light_direction(camera);
        // The effects' clock stands still while the engine is paused.
        let step = self
            .last_seconds
            .map_or(0.0, |last| (seconds - last).max(0.0));
        self.last_seconds = Some(seconds);
        if !self.paused {
            self.effects_seconds += step;
        }
        let effects = self.effects_seconds;
        self.fire.build(self.levels.heat, effects, facing);
        self.nitrogen
            .build(self.levels.cold, effects, facing, camera.position);
        self.heat_sparks.build(simulation, camera, effects);
        self.cartridge
            .follow(simulation.regenerator().is_enabled(), seconds);
        self.chamber.draw(camera);
        self.fire.draw_burner();
        self.table.draw(camera);
        // Lit right over the table, before what stands on it, which then
        // covers it.
        self.table.draw_highlight(self.highlight, seconds, camera);
        self.valves.draw(self.valve_turns, camera);
        self.buttons.draw_bodies(camera);
        self.cartridge.draw(camera);
        if scenery == Scenery::Sky {
            self.cloud.draw(camera, light);
        }
        self.particles.draw(simulation, camera, background);
        // The seated cartridge shows faintly through the gas, but not
        // through the pistons, which come after.
        self.cartridge.draw_ghost();
        self.assembly.draw(simulation.power_pistons(), camera);
        // The coil after everything solid, which shows through it.
        self.nitrogen.draw_coil(camera);
        self.buttons.draw_domes(self.switches, seconds, camera);
        // The see-through grid and trails after everything solid, so they
        // blend over it.
        if scenery == Scenery::Grid {
            self.floor.draw();
        }
        if let Some(tracers) = simulation.tracers() {
            self.trails.draw(tracers, &simulation.particles, camera);
        }
        // Light and haze last: they are tested against everything solid
        // but hide nothing.
        // The sparks stay out of the way of the followed particles while
        // the tracers are on.
        if let Some(materials) = &self.sprite_materials {
            self.fire.draw(materials);
            self.nitrogen.draw(materials);
            if simulation.tracers().is_none() {
                self.heat_sparks.draw(materials);
            }
        }
    }
}
