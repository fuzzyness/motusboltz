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
const TRAIL_WIDTH: f32 = 0.07;
/// Pale yellow stands out against both the warm and the cool particles.
const TRAIL_COLOR: Color = Color::from_rgba(255, 238, 180, 255);
/// Opacity of the newest end of a trail.
const TRAIL_OPACITY: f32 = 0.95;
const VERTICES_PER_SEGMENT: usize = 4;
const INDICES_PER_SEGMENT: usize = 6;

/// The trail mesh, rebuilt every frame in reused buffers.
pub struct TrailRenderer {
    mesh: Mesh,
}

impl TrailRenderer {
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

    fn build(&mut self, tracers: &Tracers, particles: &[Particle], eye: Vec3) {
        self.mesh.vertices.clear();
        self.mesh.indices.clear();
        let now = tracers.time();
        for (index, points) in tracers.paths() {
            let current = PathPoint {
                time: now,
                position: particles[index].position,
            };
            let mut previous: Option<PathPoint> = None;
            for point in points.iter().copied().chain([current]) {
                if let Some(start) = previous {
                    self.add_segment(start, point, now, eye);
                }
                previous = Some(point);
            }
        }
    }

    /// One ribbon quad from `start` to `end`, turned to face `eye`, with
    /// each end's opacity falling with its age.
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
                a: TRAIL_OPACITY * freshness,
                ..TRAIL_COLOR
            }
        };
        let first = self.mesh.vertices.len() as u16;
        for (position, time) in [(start.position, start.time), (end.position, end.time)] {
            for offset in [side, -side] {
                self.mesh.vertices.push(Vertex::new2(
                    position + offset,
                    Vec2::ZERO,
                    color_at(time),
                ));
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
