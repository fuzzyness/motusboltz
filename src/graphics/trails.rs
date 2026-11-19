//! Tracer trails: each followed particle's recent path as a thin ribbon
//! that faces the camera and fades out with age.

use macroquad::models::{
    Mesh,
    Vertex, //
};
use macroquad::prelude::{
    Camera3D,
    Color,
    Mat4,
    Vec2,
    Vec3, //
};

use super::gpu::GpuMesh;
use super::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY,
    draw_plain, //
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
    /// The ribbons, sent to the graphics card again each frame.
    mesh: GpuMesh,
}

impl TrailRenderer {
    /// A renderer with nothing built yet.
    pub fn new() -> Self {
        Self {
            mesh: GpuMesh::empty(),
        }
    }

    /// Draws every followed particle's path, from its oldest kept point to
    /// where the particle is now.
    pub fn draw(&mut self, tracers: &Tracers, particles: &[Particle], camera: &Camera3D) {
        self.build(tracers, particles, camera.position);
        draw_plain([&self.mesh], Mat4::IDENTITY);
    }

    /// Rebuilds the ribbons, turned to face `eye`.
    fn build(&mut self, tracers: &Tracers, particles: &[Particle], eye: Vec3) {
        let mesh = self.mesh.edit();
        mesh.vertices.clear();
        mesh.indices.clear();
        let now = tracers.time();
        for (index, points) in tracers.paths() {
            let current = PathPoint {
                time: now,
                position: particles[index].position,
            };
            let mut previous = None;
            for point in points.iter().copied().chain([current]) {
                if let Some(start) = previous.replace(point) {
                    add_segment(mesh, start, point, now, eye);
                }
            }
        }
    }
}

/// Adds to `mesh` one ribbon quad from `start` to `end`, turned to face
/// `eye`.
///
/// Each end's opacity falls with its age at time `now`.
fn add_segment(mesh: &mut Mesh, start: PathPoint, end: PathPoint, now: f64, eye: Vec3) {
    {
        let along = end.position - start.position;
        let middle = (start.position + end.position) / 2.0;
        let side = along.cross(eye - middle).normalize_or_zero() * (TRAIL_WIDTH / 2.0);
        if side == Vec3::ZERO
            || mesh.vertices.len() + VERTICES_PER_SEGMENT > DRAW_CALL_VERTEX_CAPACITY
            || mesh.indices.len() + INDICES_PER_SEGMENT > DRAW_CALL_INDEX_CAPACITY
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
        let first = mesh.vertices.len() as u16;
        for point in [start, end] {
            for offset in [side, -side] {
                let vertex =
                    Vertex::new2(point.position + offset, Vec2::ZERO, color_at(point.time));
                mesh.vertices.push(vertex);
            }
        }
        mesh.indices
            .extend([first, first + 1, first + 2, first + 1, first + 3, first + 2]);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/trails.rs"]
mod tests;
