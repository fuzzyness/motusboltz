//! 3D drawing of the particles, tracer trails, cylinders, the opening
//! between them, pistons, crank and flywheel. Drawing only; nothing here
//! affects the physics.

pub mod block;
pub mod chamber;
pub mod floor;
pub mod particles;
pub mod shaft;
pub mod trails;

use macroquad::prelude::{
    Camera3D,
    Color, //
};

use crate::sim::engine::{
    ConnectedChambers,
    PARTICLE_RADIUS, //
};
use block::BlockShape;
use chamber::ConnectedChamberVisual;
use floor::FloorGrid;
use particles::ParticleRenderer;
use shaft::ShaftVisual;
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
    /// The followed particles' indices this frame, reused.
    followed: Vec<usize>,
    /// A cylinder enclosing the whole apparatus, for the particle depth fade.
    extent: CylinderDimensions,
}

impl Scene {
    pub fn new(simulation: &ConnectedChambers) -> Self {
        let geometry = simulation.geometry();
        Self {
            chamber: ConnectedChamberVisual::new(geometry),
            floor: FloorGrid::new(BlockShape::new(geometry).base),
            shaft: ShaftVisual::new(geometry),
            particles: ParticleRenderer::new(),
            trails: TrailRenderer::new(),
            followed: Vec::new(),
            extent: CylinderDimensions {
                radius: geometry.cylinder_offset + geometry.cylinder_radius,
                height: geometry.grid_side(),
            },
        }
    }

    /// Draws the engine as it is now. The 3D camera must already be set;
    /// distant particles fade toward `background`.
    pub fn draw(&mut self, simulation: &ConnectedChambers, camera: &Camera3D, background: Color) {
        let pistons = simulation.power_pistons();
        self.chamber
            .draw(camera, pistons.hot().face_y, pistons.cold().face_y);
        self.shaft.draw(pistons);
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
    }
}
