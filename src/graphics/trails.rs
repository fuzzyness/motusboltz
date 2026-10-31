//! Tracer trails: each followed particle's recent path as a thin ribbon
//! that faces the camera and fades out with age.

use macroquad::models::{
    Mesh,
    Vertex,
    draw_mesh, //
};
use macroquad::prelude::{
    Camera3D,
    Color,
    Vec2,
    Vec3, //
};

use super::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY, //
};
use crate::sim::gas::Particle;
use crate::sim::tracers::{
    PathPoint,
    TRAIL_SECONDS,
    Tracers, //
};

/// Ribbon width, in model units (a particle is 0.26 across).
const TRAIL_WIDTH: f32 = 0.1;
/// Color of the newest end of a trail.
///
/// Bright yellow stands out against the muted particles and against both
/// the sky and the dark grid.
const TRAIL_COLOR: Color = Color::from_rgba(255, 224, 40, 255);
/// Corners of one ribbon quad.
const VERTICES_PER_SEGMENT: usize = 4;
/// Indices of one ribbon quad's two triangles.
const INDICES_PER_SEGMENT: usize = 6;

/// The trail mesh, rebuilt every frame in reused buffers.
pub struct TrailRenderer {
    /// The ribbons.
    mesh: Mesh,
}

impl TrailRenderer {
    /// A renderer with nothing built yet.
    pub fn new() -> Self {
        Self {
            mesh: Mesh {
                vertices: Vec::new(),
                indices: Vec::new(),
                texture: None,
            },
        }
    }

    /// Draws every followed particle's path, from its oldest kept point to
    /// where the particle is now.
    pub fn draw(&mut self, tracers: &Tracers, particles: &[Particle], camera: &Camera3D) {
        self.build(tracers, particles, camera.position);
        if !self.mesh.indices.is_empty() {
            draw_mesh(&self.mesh);
        }
    }

    /// Rebuilds the ribbons, turned to face `eye`.
    fn build(&mut self, tracers: &Tracers, particles: &[Particle], eye: Vec3) {
        self.mesh.vertices.clear();
        self.mesh.indices.clear();
        let now = tracers.time();
        for (index, points) in tracers.paths() {
            let current = PathPoint {
                time: now,
                position: particles[index].position,
            };
            let mut previous = None;
            for point in points.iter().copied().chain([current]) {
                if let Some(start) = previous.replace(point) {
                    self.add_segment(start, point, now, eye);
                }
            }
        }
    }

    /// Adds one ribbon quad from `start` to `end`, turned to face `eye`.
    ///
    /// Each end's opacity falls with its age at time `now`.
    fn add_segment(&mut self, start: PathPoint, end: PathPoint, now: f64, eye: Vec3) {
        let along = end.position - start.position;
        let middle = (start.position + end.position) / 2.0;
        let side = along.cross(eye - middle).normalize_or_zero() * (TRAIL_WIDTH / 2.0);
        if side == Vec3::ZERO
            || self.mesh.vertices.len() + VERTICES_PER_SEGMENT > DRAW_CALL_VERTEX_CAPACITY
            || self.mesh.indices.len() + INDICES_PER_SEGMENT > DRAW_CALL_INDEX_CAPACITY
        {
            return;
        }
        let color_at = |time: f64| {
            let freshness = (1.0 - (now - time) / TRAIL_SECONDS).clamp(0.0, 1.0) as f32;
            Color {
                a: freshness,
                ..TRAIL_COLOR
            }
        };
        let first = self.mesh.vertices.len() as u16;
        for point in [start, end] {
            for offset in [side, -side] {
                let vertex =
                    Vertex::new2(point.position + offset, Vec2::ZERO, color_at(point.time));
                self.mesh.vertices.push(vertex);
            }
        }
        self.mesh
            .indices
            .extend([first, first + 1, first + 2, first + 1, first + 3, first + 2]);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/trails.rs"]
mod tests;
