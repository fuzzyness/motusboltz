//! 3D drawing of the particles, tracer trails, cylinders, the opening
//! between them, the pistons, rods, crankshaft and flywheel, the burner
//! and ice that stand for the two reservoirs, and the background. Drawing only;
//! nothing here affects the physics.

pub mod assembly;
pub mod backdrop;
pub mod block;
pub mod chamber;
pub mod cloud;
pub mod fire;
pub mod floor;
pub mod ice;
pub mod levels;
pub mod outline;
pub mod particles;
pub mod soft_view;
pub mod solid;
pub mod sprites;
pub mod trails;

use macroquad::prelude::{
    Camera3D,
    Color,
    clear_background, //
};

use crate::sim::engine::{
    ConnectedChambers,
    PARTICLE_RADIUS, //
};
use crate::ui::draw_target::{
    draw_to,
    draw_to_screen, //
};
use assembly::Assembly;
use backdrop::Backdrop;
pub use backdrop::Scenery;
use block::BlockShape;
use chamber::ConnectedChamberVisual;
use cloud::CloudFloor;
use fire::Fire;
use floor::FloorGrid;
use ice::Ice;
use levels::ReservoirLevels;
use particles::{
    ParticleRenderer,
    camera_light_direction, //
};
use soft_view::SoftView;
use sprites::{
    Facing,
    SpriteMaterials, //
};
use trails::TrailRenderer;

/// Macroquad's per-draw-call buffer sizes, set in the window configuration;
/// every mesh must fit within them.
pub const DRAW_CALL_VERTEX_CAPACITY: usize = 16_384;
pub const DRAW_CALL_INDEX_CAPACITY: usize = 60_000;

/// What distant particles fade toward in the sky: the haze near the
/// horizon, a little, so they stay bright.
const SKY_FADE: Color = Color::from_rgba(176, 204, 240, 255);

/// Radius and height, in model units, of a vertical cylinder centered on
/// the origin. Used for cylinder frames and for the depth-fade extent.
#[derive(Clone, Copy)]
pub struct CylinderDimensions {
    pub radius: f32,
    pub height: f32,
}

/// Mixes two opaque colors, clamping the interpolation fraction to [0, 1].
fn mix_color(first: Color, second: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);

    Color {
        r: first.r + (second.r - first.r) * amount,
        g: first.g + (second.g - first.g) * amount,
        b: first.b + (second.b - first.b) * amount,
        a: first.a,
    }
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
    chamber: ConnectedChamberVisual,
    floor: FloorGrid,
    assembly: Assembly,
    particles: ParticleRenderer,
    trails: TrailRenderer,
    fire: Fire,
    ice: Ice,
    /// `None` where the sprite shader cannot be built: the burner and ice
    /// still show, without flames or mist.
    sprite_materials: Option<SpriteMaterials>,
    backdrop: Backdrop,
    /// What the engine stands on in the sky scenery.
    cloud: CloudFloor,
    soft: SoftView,
    /// How strongly the burner, ice and glows show, easing after the
    /// sliders.
    levels: ReservoirLevels,
    /// The followed particles' indices this frame, reused.
    followed: Vec<usize>,
    /// A cylinder enclosing the whole apparatus, for the particle depth fade.
    extent: CylinderDimensions,
}

impl Scene {
    pub fn new(simulation: &ConnectedChambers) -> Self {
        let geometry = simulation.geometry();
        let shape = BlockShape::new(geometry);
        Self {
            chamber: ConnectedChamberVisual::new(geometry),
            floor: FloorGrid::new(shape.base),
            assembly: Assembly::new(geometry, simulation.power_pistons()),
            particles: ParticleRenderer::new(),
            trails: TrailRenderer::new(),
            fire: Fire::new(shape),
            ice: Ice::new(shape),
            sprite_materials: SpriteMaterials::load(),
            backdrop: Backdrop::new(geometry),
            cloud: CloudFloor::new(shape),
            soft: SoftView::new(),
            levels: ReservoirLevels::default(),
            followed: Vec::new(),
            extent: CylinderDimensions {
                radius: geometry.cylinder_offset + geometry.cylinder_radius,
                height: geometry.grid_side(),
            },
        }
    }

    /// Draws the background and the engine for one frame into a softened
    /// view (see `soft_view.rs`), or straight to the screen where that
    /// cannot be built, and leaves the screen camera set. The window is
    /// `width` by `height` in screen points at `dpi`; `background` is the
    /// clear color; `seconds` is the clock, which eases the reservoir
    /// effects toward the sliders and animates the flames, mist and clouds
    /// (so they keep moving while the simulation is paused).
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        simulation: &ConnectedChambers,
        camera: &Camera3D,
        scenery: Scenery,
        background: Color,
        seconds: f64,
        width: f32,
        height: f32,
        dpi: f32,
    ) {
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
            clear_background(background);
            camera.render_target = Some(target);
        }
        self.backdrop.draw(
            scenery,
            &camera,
            width,
            height,
            self.levels.heat,
            self.levels.cold,
            seconds,
        );
        draw_to(&camera);
        let fade = match scenery {
            Scenery::Sky => SKY_FADE,
            Scenery::Grid => background,
        };
        self.draw(simulation, &camera, scenery, fade, seconds);
        draw_to_screen();
        self.soft.present(width, height);
    }

    /// Draws the engine as it is now in `scenery`. The 3D camera must
    /// already be set; distant particles fade toward `background`. The flames and mist are
    /// animated by `seconds`, the clock, so they keep moving while the
    /// simulation is paused.
    fn draw(
        &mut self,
        simulation: &ConnectedChambers,
        camera: &Camera3D,
        scenery: Scenery,
        background: Color,
        seconds: f64,
    ) {
        let pistons = simulation.power_pistons();
        let facing = Facing::toward(camera);
        self.fire.build(self.levels.heat, seconds, facing);
        self.ice.build(
            self.levels.cold,
            seconds,
            camera_light_direction(camera),
            facing,
        );
        self.chamber.draw(camera);
        self.assembly.draw(pistons, camera);
        self.fire.draw_burner();
        self.ice.draw_solid();
        if scenery == Scenery::Sky {
            self.cloud.draw(camera, camera_light_direction(camera));
        }
        self.followed.clear();
        if let Some(tracers) = simulation.tracers() {
            self.followed.extend(tracers.indices());
        }
        self.particles.draw(
            &simulation.particles,
            simulation.collision_flashes(),
            &self.followed,
            PARTICLE_RADIUS,
            camera,
            self.extent,
            background,
        );
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
        if let Some(materials) = &self.sprite_materials {
            self.fire.sprites().draw(&materials.glow);
            self.ice.sprites().draw(&materials.haze);
        }
    }
}
