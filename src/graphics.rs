//! 3D drawing of the particles, tracer trails, cylinders, the opening
//! between them, pistons, crank and flywheel, the burner and ice that
//! stand for the two reservoirs, and the background. Drawing only;
//! nothing here affects the physics.

pub mod backdrop;
pub mod block;
pub mod chamber;
pub mod fire;
pub mod floor;
pub mod ice;
pub mod outline;
pub mod particles;
pub mod shaft;
pub mod sprites;
pub mod trails;

use macroquad::prelude::{
    Camera3D,
    Color, //
};

use crate::sim::engine::{
    ConnectedChambers,
    PARTICLE_RADIUS, //
};
use backdrop::Backdrop;
use block::BlockShape;
use chamber::ConnectedChamberVisual;
use fire::{
    Fire,
    heat_level, //
};
use floor::FloorGrid;
use ice::{
    Ice,
    cold_level, //
};
use particles::{
    ParticleRenderer,
    camera_light_direction, //
};
use shaft::ShaftVisual;
use sprites::{
    Facing,
    SpriteMaterials, //
};
use trails::TrailRenderer;

/// Macroquad's per-draw-call buffer sizes, set in the window configuration;
/// every mesh must fit within them.
pub const DRAW_CALL_VERTEX_CAPACITY: usize = 16_384;
pub const DRAW_CALL_INDEX_CAPACITY: usize = 60_000;

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
    shaft: ShaftVisual,
    particles: ParticleRenderer,
    trails: TrailRenderer,
    fire: Fire,
    ice: Ice,
    /// `None` where the sprite shader cannot be built: the burner and ice
    /// still show, without flames or mist.
    sprite_materials: Option<SpriteMaterials>,
    backdrop: Backdrop,
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
            shaft: ShaftVisual::new(geometry),
            particles: ParticleRenderer::new(),
            trails: TrailRenderer::new(),
            fire: Fire::new(shape),
            ice: Ice::new(shape),
            sprite_materials: SpriteMaterials::load(),
            backdrop: Backdrop::new(geometry),
            followed: Vec::new(),
            extent: CylinderDimensions {
                radius: geometry.cylinder_offset + geometry.cylinder_radius,
                height: geometry.grid_side(),
            },
        }
    }

    /// Fills the window behind the engine. The screen camera must be set;
    /// `camera` is the 3D view the engine will be drawn with.
    pub fn draw_backdrop(
        &mut self,
        simulation: &ConnectedChambers,
        camera: &Camera3D,
        width: f32,
        height: f32,
    ) {
        self.backdrop.draw(
            camera,
            width,
            height,
            heat_level(simulation.hot_kelvin()),
            cold_level(simulation.cold_kelvin()),
        );
    }

    /// Draws the engine as it is now. The 3D camera must already be set;
    /// distant particles fade toward `background`. The flames and mist are
    /// animated by `seconds`, the clock, so they keep moving while the
    /// simulation is paused.
    pub fn draw(
        &mut self,
        simulation: &ConnectedChambers,
        camera: &Camera3D,
        background: Color,
        seconds: f64,
    ) {
        let pistons = simulation.power_pistons();
        let facing = Facing::toward(camera);
        self.fire
            .build(heat_level(simulation.hot_kelvin()), seconds, facing);
        self.ice.build(
            cold_level(simulation.cold_kelvin()),
            seconds,
            camera_light_direction(camera),
            facing,
        );
        self.chamber
            .draw(camera, pistons.hot().face_y, pistons.cold().face_y);
        self.shaft.draw(pistons);
        self.fire.draw_burner();
        self.ice.draw_solid();
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
        self.floor.draw();
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
