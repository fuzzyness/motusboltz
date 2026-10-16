//! Batched low-polygon sphere rendering for the particle simulation.

use std::f32::consts::{
    PI,
    TAU, //
};

use macroquad::models::{
    Mesh,
    Vertex, //
    draw_mesh,
};
use macroquad::prelude::{
    Color,
    ORANGE, //
    Vec2,
    Vec3,
    vec3,
};

use crate::particle::Particle;

// Bound each mesh so all its vertex indices fit in `u16`.
const PARTICLES_PER_BATCH: usize = 256;
const SPHERE_LATITUDE_SEGMENTS: usize = 4;
const SPHERE_LONGITUDE_SEGMENTS: usize = 6;
const PARTICLE_COLOR: Color = ORANGE;

// Macroquad's default draw buffers are too small for a full batch.
// These capacities are also applied in `main.rs`.
pub const DRAW_CALL_VERTEX_CAPACITY: usize = 16_384;
pub const DRAW_CALL_INDEX_CAPACITY: usize = 60_000;

/// Unit sphere topology shared by every particle mesh.
struct SphereTemplate {
    positions: Vec<Vec3>,
    indices: Vec<u16>,
}

impl SphereTemplate {
    fn new() -> Self {
        let mut positions = Vec::new();
        let mut indices = Vec::new();

        // Share one vertex at each pole instead of duplicating pole vertices.
        let north = 0;
        positions.push(Vec3::Y);

        for latitude in 1..SPHERE_LATITUDE_SEGMENTS {
            let theta = PI * latitude as f32 / SPHERE_LATITUDE_SEGMENTS as f32;
            let (sin_theta, cos_theta) = theta.sin_cos();

            for longitude in 0..SPHERE_LONGITUDE_SEGMENTS {
                let phi = TAU * longitude as f32 / SPHERE_LONGITUDE_SEGMENTS as f32;
                let (sin_phi, cos_phi) = phi.sin_cos();

                positions.push(vec3(sin_theta * cos_phi, cos_theta, sin_theta * sin_phi));
            }
        }

        let south = positions.len();
        positions.push(Vec3::NEG_Y);

        // Ring indices exclude the two pole vertices.
        let ring_vertex = |latitude: usize, longitude: usize| -> usize {
            1 + (latitude - 1) * SPHERE_LONGITUDE_SEGMENTS + longitude
        };

        for longitude in 0..SPHERE_LONGITUDE_SEGMENTS {
            let next = (longitude + 1) % SPHERE_LONGITUDE_SEGMENTS;

            indices.extend([
                north as u16,
                ring_vertex(1, next) as u16,
                ring_vertex(1, longitude) as u16,
            ]);
        }

        for latitude in 1..SPHERE_LATITUDE_SEGMENTS - 1 {
            for longitude in 0..SPHERE_LONGITUDE_SEGMENTS {
                let next = (longitude + 1) % SPHERE_LONGITUDE_SEGMENTS;
                let upper = ring_vertex(latitude, longitude) as u16;
                let upper_next = ring_vertex(latitude, next) as u16;
                let lower = ring_vertex(latitude + 1, longitude) as u16;
                let lower_next = ring_vertex(latitude + 1, next) as u16;

                indices.extend([upper, upper_next, lower]);
                indices.extend([upper_next, lower_next, lower]);
            }
        }

        for longitude in 0..SPHERE_LONGITUDE_SEGMENTS {
            let next = (longitude + 1) % SPHERE_LONGITUDE_SEGMENTS;

            indices.extend([
                south as u16,
                ring_vertex(SPHERE_LATITUDE_SEGMENTS - 1, longitude) as u16,
                ring_vertex(SPHERE_LATITUDE_SEGMENTS - 1, next) as u16,
            ]);
        }

        Self { positions, indices }
    }
}

/// Draws particles using reusable meshes of low-polygon spheres.
///
/// Batch topology and allocation are rebuilt only when the particle count
/// changes. Each rendered frame updates positions, not indices.
pub struct ParticleRenderer {
    template: SphereTemplate,
    batches: Vec<Mesh>,
    particle_count: usize,
}

impl ParticleRenderer {
    /// Creates an empty renderer and validates the mesh batch limits.
    pub fn new() -> Self {
        let template = SphereTemplate::new();

        assert!(
            template.positions.len() * PARTICLES_PER_BATCH <= u16::MAX as usize,
            "particle batch exceeds the 16-bit vertex index limit",
        );
        assert!(
            template.positions.len() * PARTICLES_PER_BATCH < DRAW_CALL_VERTEX_CAPACITY,
            "particle batch exceeds Macroquad's vertex buffer capacity",
        );
        assert!(
            template.indices.len() * PARTICLES_PER_BATCH < DRAW_CALL_INDEX_CAPACITY,
            "particle batch exceeds Macroquad's index buffer capacity",
        );

        Self {
            template,
            batches: Vec::new(),
            particle_count: 0,
        }
    }

    fn resize_batches(&mut self, particle_count: usize) {
        if particle_count == self.particle_count {
            return;
        }

        self.batches.clear();
        let mut remaining = particle_count;

        while remaining > 0 {
            let count = remaining.min(PARTICLES_PER_BATCH);
            let vertices_per_particle = self.template.positions.len();
            let mut vertices = Vec::with_capacity(count * vertices_per_particle);
            let mut indices = Vec::with_capacity(count * self.template.indices.len());

            for particle_index in 0..count {
                // Initial coordinates are placeholders; `draw()` replaces them.
                for &position in &self.template.positions {
                    vertices.push(Vertex::new2(position, Vec2::ZERO, PARTICLE_COLOR));
                }

                let base = (particle_index * vertices_per_particle) as u16;
                indices.extend(self.template.indices.iter().map(|&index| base + index));
            }

            self.batches.push(Mesh {
                vertices,
                indices,
                texture: None,
            });

            remaining -= count;
        }

        self.particle_count = particle_count;
    }

    /// Updates all mesh vertices and submits the batches for rendering.
    pub fn draw(&mut self, particles: &[Particle], radius: f32) {
        self.resize_batches(particles.len());

        for (batch, chunk) in self
            .batches
            .iter_mut()
            .zip(particles.chunks(PARTICLES_PER_BATCH))
        {
            for (particle, vertices) in chunk.iter().zip(
                batch
                    .vertices
                    .chunks_exact_mut(self.template.positions.len()),
            ) {
                for (vertex, &offset) in vertices.iter_mut().zip(&self.template.positions) {
                    vertex.position = particle.position + offset * radius;
                }
            }

            draw_mesh(batch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::simulation::PARTICLE_COUNT_PRESETS;

    #[test]
    fn sphere_mesh_indices_are_valid_and_face_outward() {
        let sphere = SphereTemplate::new();

        for triangle in sphere.indices.chunks_exact(3) {
            let a = sphere.positions[triangle[0] as usize];
            let b = sphere.positions[triangle[1] as usize];
            let c = sphere.positions[triangle[2] as usize];

            let outward = (b - a).cross(c - a).dot(a + b + c);
            assert!(outward > 0.0, "sphere triangle is inverted or degenerate");
        }
    }

    #[test]
    fn batches_cover_all_particles_without_overflowing_indices() {
        let mut renderer = ParticleRenderer::new();
        let particle_count = *PARTICLE_COUNT_PRESETS
            .last()
            .expect("presets must not be empty");
        renderer.resize_batches(particle_count);

        let total_particles: usize = renderer
            .batches
            .iter()
            .map(|mesh| mesh.vertices.len() / renderer.template.positions.len())
            .sum();

        assert_eq!(total_particles, particle_count);

        for mesh in &renderer.batches {
            assert!(mesh.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
            assert!(mesh.indices.len() < DRAW_CALL_INDEX_CAPACITY);
            assert!(
                mesh.indices
                    .iter()
                    .all(|&index| (index as usize) < mesh.vertices.len())
            );
        }
    }

    #[test]
    fn resizing_rebuilds_only_the_batch_layout() {
        let mut renderer = ParticleRenderer::new();

        renderer.resize_batches(PARTICLES_PER_BATCH);
        assert_eq!(renderer.batches.len(), 1);

        renderer.resize_batches(PARTICLES_PER_BATCH + 1);
        assert_eq!(renderer.batches.len(), 2);
        assert_eq!(renderer.particle_count, PARTICLES_PER_BATCH + 1);
    }
}
