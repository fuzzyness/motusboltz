//! Batched particle rendering with kinetic-energy colors and depth shading.

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

use crate::particle::Particle;
use crate::physics;
use crate::simulation::{
    COLLISION_FLASH_DURATION,
    INITIAL_SPEED_LIMIT, //
};

// Keep each batch within Macroquad's vertex and index buffer limits.
const PARTICLES_PER_BATCH: usize = 256;
const SPHERE_LATITUDE_SEGMENTS: usize = 4;
const SPHERE_LONGITUDE_SEGMENTS: usize = 6;

// These are kinetic-energy colors, not temperatures of individual particles.
const COLD_COLOR: Color = Color::from_rgba(60, 151, 255, 255);
const NORMAL_COLOR: Color = ORANGE;
const HOT_COLOR: Color = Color::from_rgba(255, 67, 62, 255);

// For independent velocities uniform in [-v_max, v_max], the expected
// kinetic energy in 3D is m * v_max^2 / 2. Keep this scale independent of
// the temperature slider, so heating the gas changes its color distribution.
const NORMAL_PARTICLE_ENERGY: f32 =
    crate::particle::PARTICLE_MASS * INITIAL_SPEED_LIMIT * INITIAL_SPEED_LIMIT / 2.0;
const HOT_ENERGY_MULTIPLIER: f32 = 3.0;

// Camera-space light components: right, up, and toward the viewer.
const LIGHT_DIRECTION: Vec3 = Vec3::new(0.4, 0.7, 0.6);
const AMBIENT_LIGHT: f32 = 0.35;
const DIFFUSE_LIGHT: f32 = 0.65;
const DEPTH_FADE_STRENGTH: f32 = 0.8;

// Impact markers are visual feedback, not extra heat or lost/gained energy.
const COLLISION_FLASH_COLOR: Color = Color::from_rgba(255, 252, 226, 255);
const COLLISION_FLASH_STRENGTH: f32 = 0.3;

pub const DRAW_CALL_VERTEX_CAPACITY: usize = 16_384;
pub const DRAW_CALL_INDEX_CAPACITY: usize = 60_000;

/// Radius and height of the cylindrical chamber in simulation units.
///
/// Both values are positive and refer to the chamber's inner dimensions.
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

/// Maps *individual thermal kinetic energy* to a fixed blue/orange/red scale.
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
        let mut positions = Vec::new();
        let mut indices = Vec::new();

        // Share one vertex at each pole.
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

/// Renders opaque spheres with reusable batched geometry.
///
/// Each frame updates particle color from thermal kinetic energy and impacts,
/// then applies camera-relative lighting and depth fading. The mesh buffers
/// are reused; the number of mesh submissions is unchanged.
pub struct ParticleRenderer {
    template: SphereTemplate,
    batches: Vec<Mesh>,
    particle_count: usize,
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
                // Colors and positions are overwritten in draw().
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
        assert_eq!(particles.len(), collision_flashes.len());
        self.resize_batches(particles.len());
        self.template.update_lighting(camera);

        let depth_fade = DepthFade::new(camera, chamber.radius, chamber.height);
        let mean_velocity = physics::mean_velocity(particles);

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
                let energy = physics::particle_kinetic_energy(thermal_velocity);
                let base_color = kinetic_energy_color(energy);
                let color = collision_color(base_color, flash);
                let fade = depth_fade.amount(particle.position);

                for ((vertex, &offset), &brightness) in vertices
                    .iter_mut()
                    .zip(&self.template.positions)
                    .zip(&self.template.brightness)
                {
                    vertex.position = particle.position + offset * radius;

                    let shaded = Color {
                        r: color.r * brightness,
                        g: color.g * brightness,
                        b: color.b * brightness,
                        a: color.a,
                    };
                    vertex.color = mix_color(shaded, background, fade).into();
                }
            }

            draw_mesh(batch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::simulation::{
        CHAMBER_HEIGHT,
        CHAMBER_RADIUS,
        PARTICLE_COUNT_PRESETS, //
    };

    const VERTICES_PER_TRIANGLE: usize = 3;
    const TEST_BACKGROUND: Color = Color::from_rgba(12, 18, 30, 255);
    const TEST_CAMERA_POSITION: Vec3 = Vec3::new(0.0, 0.0, 8.0);
    const TEST_CAMERA_TRANSLATION: Vec3 = Vec3::new(1.0, -2.0, 3.0);
    const FLOAT_TOLERANCE: f32 = 1e-6;

    fn test_camera() -> Camera3D {
        Camera3D {
            position: TEST_CAMERA_POSITION,
            target: Vec3::ZERO,
            up: Vec3::Y,
            ..Default::default()
        }
    }

    #[test]
    fn sphere_mesh_indices_are_valid_and_face_outward() {
        let sphere = SphereTemplate::new();
        let (triangles, remainder) = sphere.indices.as_chunks::<VERTICES_PER_TRIANGLE>();

        assert!(remainder.is_empty());

        for triangle in triangles {
            let a = sphere.positions[triangle[0] as usize];
            let b = sphere.positions[triangle[1] as usize];
            let c = sphere.positions[triangle[2] as usize];

            let outward = (b - a).cross(c - a).dot(a + b + c);
            assert!(outward > 0.0, "sphere triangle is inverted or degenerate");
        }
    }

    #[test]
    fn shading_matches_template_vertices() {
        let mut sphere = SphereTemplate::new();
        sphere.update_lighting(&test_camera());
        assert_eq!(sphere.positions.len(), sphere.brightness.len());

        for &brightness in &sphere.brightness {
            assert!((AMBIENT_LIGHT..=1.0).contains(&brightness));
        }
    }

    #[test]
    fn visible_surface_brightness_is_stable_while_orbiting() {
        let front_camera = test_camera();
        let back_camera = Camera3D {
            position: -TEST_CAMERA_POSITION,
            ..test_camera()
        };
        let front_light = camera_light_direction(&front_camera);
        let back_light = camera_light_direction(&back_camera);

        let front_brightness = lambert_brightness(Vec3::Z, front_light);
        let back_brightness = lambert_brightness(Vec3::NEG_Z, back_light);

        assert!((front_brightness - back_brightness).abs() < FLOAT_TOLERANCE);
        assert!(front_brightness > AMBIENT_LIGHT);
    }

    #[test]
    fn kinetic_colors_use_an_absolute_energy_scale() {
        let low = kinetic_energy_color(0.0);
        let middle = kinetic_energy_color(NORMAL_PARTICLE_ENERGY);
        let high = kinetic_energy_color(NORMAL_PARTICLE_ENERGY * HOT_ENERGY_MULTIPLIER);

        assert_eq!(low, COLD_COLOR);
        assert_eq!(middle, NORMAL_COLOR);
        assert_eq!(high, HOT_COLOR);
        assert_eq!(kinetic_energy_color(f32::MAX), HOT_COLOR);

        let between = kinetic_energy_color(NORMAL_PARTICLE_ENERGY / 2.0);
        assert!(between.b > NORMAL_COLOR.b);
        assert!(between.r < NORMAL_COLOR.r);
    }

    #[test]
    fn collision_highlight_fades_without_changing_alpha() {
        let base = kinetic_energy_color(NORMAL_PARTICLE_ENERGY);
        let just_collided = collision_color(base, COLLISION_FLASH_DURATION);
        let expired = collision_color(base, 0.0);

        assert!(just_collided.g > base.g);
        assert!(just_collided.b > base.b);
        assert_eq!(just_collided.a, base.a);
        assert_eq!(expired, base);
    }

    #[test]
    fn far_particles_fade_more_than_near_particles() {
        let fade = DepthFade::new(&test_camera(), CHAMBER_RADIUS, CHAMBER_HEIGHT);
        let near = vec3(0.0, 0.0, CHAMBER_RADIUS);
        let far = vec3(0.0, 0.0, -CHAMBER_RADIUS);
        let near_amount = fade.amount(near);
        let far_amount = fade.amount(far);

        assert!(near_amount.abs() < FLOAT_TOLERANCE);
        assert!((far_amount - DEPTH_FADE_STRENGTH).abs() < FLOAT_TOLERANCE);
        assert!(near_amount < far_amount);

        let near_color = mix_color(NORMAL_COLOR, TEST_BACKGROUND, near_amount);
        let far_color = mix_color(NORMAL_COLOR, TEST_BACKGROUND, far_amount);
        assert!(near_color.r > far_color.r);
        assert_eq!(far_color.a, NORMAL_COLOR.a);
    }

    #[test]
    fn panning_does_not_change_depth_fade() {
        let first = test_camera();
        let second = Camera3D {
            position: first.position + TEST_CAMERA_TRANSLATION,
            target: first.target + TEST_CAMERA_TRANSLATION,
            ..test_camera()
        };
        let near = vec3(0.0, 0.0, CHAMBER_RADIUS);
        let first_fade = DepthFade::new(&first, CHAMBER_RADIUS, CHAMBER_HEIGHT);
        let second_fade = DepthFade::new(&second, CHAMBER_RADIUS, CHAMBER_HEIGHT);

        assert!((first_fade.amount(near) - second_fade.amount(near)).abs() < FLOAT_TOLERANCE);
    }

    #[test]
    fn depth_fade_uses_cylindrical_projection() {
        let camera = Camera3D {
            position: vec3(0.0, TEST_CAMERA_POSITION.y + CHAMBER_HEIGHT, CHAMBER_RADIUS),
            target: Vec3::ZERO,
            ..test_camera()
        };
        let fade = DepthFade::new(&camera, CHAMBER_RADIUS, CHAMBER_HEIGHT);
        let projected_radius = CHAMBER_RADIUS
            * (fade.forward.x * fade.forward.x + fade.forward.z * fade.forward.z).sqrt()
            + CHAMBER_HEIGHT / 2.0 * fade.forward.y.abs();

        assert!((fade.half_extent - projected_radius).abs() < FLOAT_TOLERANCE);
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
