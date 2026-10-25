//! Every particle as a small shaded sphere, colored by its kinetic energy,
//! lit from the camera and faded with depth.

use std::f32::consts::{
    PI,
    TAU, //
};

use macroquad::models::{
    Mesh,
    Vertex,
    draw_mesh, //
};
use macroquad::prelude::{
    Camera3D,
    Color,
    ORANGE,
    Vec2,
    Vec3,
    vec3, //
};

use super::{
    CylinderDimensions,
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY,
    mix_color,
    shade, //
};
use crate::sim::engine::COLLISION_FLASH_DURATION;
use crate::sim::gas::{
    self,
    INITIAL_TEMPERATURE,
    Particle, //
};

// Keep each batch within Macroquad's vertex and index buffer limits.
const PARTICLES_PER_BATCH: usize = 256;
const SPHERE_LATITUDE_SEGMENTS: usize = 4;
const SPHERE_LONGITUDE_SEGMENTS: usize = 6;
// At high fill, use coarser spheres to reduce vertex work; every particle is
// still drawn.
const LOW_DETAIL_PARTICLE_THRESHOLD: usize = 1_536;
const LOW_DETAIL_LATITUDE_SEGMENTS: usize = 3;
const LOW_DETAIL_LONGITUDE_SEGMENTS: usize = 4;

// Colors encode each particle's kinetic energy. A single particle has no
// temperature; temperature is a property of many particles.
const COLD_COLOR: Color = Color::from_rgba(60, 151, 255, 255);
const NORMAL_COLOR: Color = ORANGE;
const HOT_COLOR: Color = Color::from_rgba(255, 67, 62, 255);

// Mean kinetic energy per particle at 300 K: 3 kT / 2 in reduced units.
const NORMAL_PARTICLE_ENERGY: f32 = 1.5 * INITIAL_TEMPERATURE;
const HOT_ENERGY_MULTIPLIER: f32 = 3.0;

// Camera-space light components: right, up, and toward the viewer.
const LIGHT_DIRECTION: Vec3 = Vec3::new(0.4, 0.7, 0.6);
const AMBIENT_LIGHT: f32 = 0.35;
const DIFFUSE_LIGHT: f32 = 0.65;
const DEPTH_FADE_STRENGTH: f32 = 0.8;

// Brief collision highlight; purely visual, it changes no energy.
const COLLISION_FLASH_COLOR: Color = Color::from_rgba(255, 252, 226, 255);
const COLLISION_FLASH_STRENGTH: f32 = 0.3;

/// Maps a particle's kinetic energy (reduced units) to blue, orange or red on
/// a fixed scale: blue below the 300 K average, orange at it, red at 3x.
fn kinetic_energy_color(energy: f32) -> Color {
    if energy <= NORMAL_PARTICLE_ENERGY {
        mix_color(COLD_COLOR, NORMAL_COLOR, energy / NORMAL_PARTICLE_ENERGY)
    } else {
        let hot_energy = NORMAL_PARTICLE_ENERGY * HOT_ENERGY_MULTIPLIER;
        let progress = (energy - NORMAL_PARTICLE_ENERGY) / (hot_energy - NORMAL_PARTICLE_ENERGY);

        mix_color(NORMAL_COLOR, HOT_COLOR, progress)
    }
}

/// Places the light in camera space so orbiting does not darken every sphere.
fn camera_light_direction(camera: &Camera3D) -> Vec3 {
    let forward = (camera.target - camera.position).normalize();
    let right = forward.cross(camera.up).normalize();
    let up = right.cross(forward).normalize();

    (right * LIGHT_DIRECTION.x + up * LIGHT_DIRECTION.y - forward * LIGHT_DIRECTION.z).normalize()
}

/// Calculates Lambert brightness from the current camera-relative light.
fn lambert_brightness(normal: Vec3, light_direction: Vec3) -> f32 {
    let diffuse = normal.dot(light_direction).max(0.0);
    (AMBIENT_LIGHT + DIFFUSE_LIGHT * diffuse).clamp(0.0, 1.0)
}

/// Returns a brief pale highlight that fades as the simulation advances.
fn collision_color(base: Color, remaining_seconds: f32) -> Color {
    let fraction = remaining_seconds / COLLISION_FLASH_DURATION;
    mix_color(
        base,
        COLLISION_FLASH_COLOR,
        fraction * COLLISION_FLASH_STRENGTH,
    )
}

/// Smooth depth fade for a vertical cylinder centered at the origin.
///
/// Using the chamber's projected extent keeps the fade stable as particles
/// move, and makes camera panning independent of particle brightness.
struct DepthFade {
    forward: Vec3,
    half_extent: f32,
}

impl DepthFade {
    fn new(camera: &Camera3D, radius: f32, height: f32) -> Self {
        assert!(radius.is_finite() && radius > 0.0);
        assert!(height.is_finite() && height > 0.0);

        let forward = (camera.target - camera.position).normalize();
        // The largest projected radius of a cylinder is the projected
        // circular cross-section plus its half-height contribution.
        let horizontal = (forward.x * forward.x + forward.z * forward.z).sqrt();
        let half_extent = radius * horizontal + height / 2.0 * forward.y.abs();

        Self {
            forward,
            half_extent,
        }
    }

    fn amount(&self, position: Vec3) -> f32 {
        let depth = position.dot(self.forward);
        let t = ((depth + self.half_extent) / (2.0 * self.half_extent)).clamp(0.0, 1.0);

        // Cubic smoothstep avoids sharp brightness changes at either end.
        let smooth_depth = t * t * (3.0 - 2.0 * t);
        DEPTH_FADE_STRENGTH * smooth_depth
    }
}

/// Unit sphere topology and reusable lighting coefficients for all batches.
struct SphereTemplate {
    positions: Vec<Vec3>,
    brightness: Vec<f32>,
    indices: Vec<u16>,
}

impl SphereTemplate {
    fn new() -> Self {
        Self::with_segments(SPHERE_LATITUDE_SEGMENTS, SPHERE_LONGITUDE_SEGMENTS)
    }

    fn with_segments(latitude_segments: usize, longitude_segments: usize) -> Self {
        let mut positions = Vec::new();
        let mut indices = Vec::new();

        // Share one vertex at each pole.
        let north = 0;
        positions.push(Vec3::Y);

        for latitude in 1..latitude_segments {
            let theta = PI * latitude as f32 / latitude_segments as f32;
            let (sin_theta, cos_theta) = theta.sin_cos();

            for longitude in 0..longitude_segments {
                let phi = TAU * longitude as f32 / longitude_segments as f32;
                let (sin_phi, cos_phi) = phi.sin_cos();

                positions.push(vec3(sin_theta * cos_phi, cos_theta, sin_theta * sin_phi));
            }
        }

        let south = positions.len();
        positions.push(Vec3::NEG_Y);

        // Ring indices exclude the two pole vertices.
        let ring_vertex = |latitude: usize, longitude: usize| -> usize {
            1 + (latitude - 1) * longitude_segments + longitude
        };

        for longitude in 0..longitude_segments {
            let next = (longitude + 1) % longitude_segments;

            indices.extend([
                north as u16,
                ring_vertex(1, next) as u16,
                ring_vertex(1, longitude) as u16,
            ]);
        }

        for latitude in 1..latitude_segments - 1 {
            for longitude in 0..longitude_segments {
                let next = (longitude + 1) % longitude_segments;
                let upper = ring_vertex(latitude, longitude) as u16;
                let upper_next = ring_vertex(latitude, next) as u16;
                let lower = ring_vertex(latitude + 1, longitude) as u16;
                let lower_next = ring_vertex(latitude + 1, next) as u16;

                indices.extend([upper, upper_next, lower]);
                indices.extend([upper_next, lower_next, lower]);
            }
        }

        for longitude in 0..longitude_segments {
            let next = (longitude + 1) % longitude_segments;

            indices.extend([
                south as u16,
                ring_vertex(latitude_segments - 1, longitude) as u16,
                ring_vertex(latitude_segments - 1, next) as u16,
            ]);
        }

        // Initialize the lighting buffer; update it for the current camera
        // once per frame, rather than once per particle or vertex copy.
        let brightness = vec![AMBIENT_LIGHT; positions.len()];

        Self {
            positions,
            brightness,
            indices,
        }
    }

    fn update_lighting(&mut self, camera: &Camera3D) {
        let light_direction = camera_light_direction(camera);

        for (&normal, brightness) in self.positions.iter().zip(&mut self.brightness) {
            *brightness = lambert_brightness(normal, light_direction);
        }
    }
}

/// Draws every particle as a small shaded sphere, 256 per mesh.
///
/// Each frame rewrites vertex positions and colors in place: color from the
/// particle's energy relative to the gas's mean motion, plus collision
/// highlight, camera-relative lighting and depth fade. Meshes are rebuilt
/// only when the particle count changes.
pub struct ParticleRenderer {
    template: SphereTemplate,
    batches: Vec<Mesh>,
    particle_count: usize,
    low_detail: bool,
}

impl ParticleRenderer {
    /// Creates the sphere template and checks the batch capacity limits.
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
            low_detail: false,
        }
    }

    fn resize_batches(&mut self, particle_count: usize) {
        if particle_count == self.particle_count {
            return;
        }

        let low_detail = particle_count >= LOW_DETAIL_PARTICLE_THRESHOLD;
        if low_detail != self.low_detail {
            self.template = if low_detail {
                SphereTemplate::with_segments(
                    LOW_DETAIL_LATITUDE_SEGMENTS,
                    LOW_DETAIL_LONGITUDE_SEGMENTS,
                )
            } else {
                SphereTemplate::new()
            };
            self.low_detail = low_detail;
        }
        self.batches.clear();
        let mut remaining = particle_count;

        while remaining > 0 {
            let count = remaining.min(PARTICLES_PER_BATCH);
            let vertices_per_particle = self.template.positions.len();
            let mut vertices = Vec::with_capacity(count * vertices_per_particle);
            let mut indices = Vec::with_capacity(count * self.template.indices.len());

            for particle_index in 0..count {
                // Colors and positions are overwritten in prepare().
                for &position in &self.template.positions {
                    vertices.push(Vertex::new2(position, Vec2::ZERO, NORMAL_COLOR));
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

    /// Updates vertex coordinates and colors, then submits each mesh batch.
    pub fn draw(
        &mut self,
        particles: &[Particle],
        collision_flashes: &[f32],
        radius: f32,
        camera: &Camera3D,
        chamber: CylinderDimensions,
        background: Color,
    ) {
        self.prepare(
            particles,
            collision_flashes,
            radius,
            camera,
            chamber,
            background,
        );
        for batch in &self.batches {
            draw_mesh(batch);
        }
    }

    /// Updates reusable CPU buffers separately from GPU submission.
    fn prepare(
        &mut self,
        particles: &[Particle],
        collision_flashes: &[f32],
        radius: f32,
        camera: &Camera3D,
        chamber: CylinderDimensions,
        background: Color,
    ) {
        assert_eq!(particles.len(), collision_flashes.len());
        self.resize_batches(particles.len());
        self.template.update_lighting(camera);

        let depth_fade = DepthFade::new(camera, chamber.radius, chamber.height);
        let mean_velocity = gas::mean_velocity(particles);

        for ((batch, chunk), flash_chunk) in self
            .batches
            .iter_mut()
            .zip(particles.chunks(PARTICLES_PER_BATCH))
            .zip(collision_flashes.chunks(PARTICLES_PER_BATCH))
        {
            for ((particle, vertices), &flash) in chunk
                .iter()
                .zip(
                    batch
                        .vertices
                        .chunks_exact_mut(self.template.positions.len()),
                )
                .zip(flash_chunk)
            {
                // Ignore the gas's bulk drift: only relative kinetic energy
                // should determine its color.
                let thermal_velocity = particle.velocity - mean_velocity;
                let energy = gas::particle_kinetic_energy(thermal_velocity);
                let base_color = kinetic_energy_color(energy);
                let color = collision_color(base_color, flash);
                let fade = depth_fade.amount(particle.position);

                for ((vertex, &offset), &brightness) in vertices
                    .iter_mut()
                    .zip(&self.template.positions)
                    .zip(&self.template.brightness)
                {
                    vertex.position = particle.position + offset * radius;

                    vertex.color = mix_color(shade(color, brightness), background, fade).into();
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/particles.rs"]
mod tests;
