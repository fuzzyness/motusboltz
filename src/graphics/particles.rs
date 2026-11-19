//! Every particle as a small glossy sphere, colored by its kinetic energy,
//! lit from the camera and faded a little with depth.
//!
//! Where the shader can be built, each sphere is lit per pixel, with a
//! bright highlight and a rim of light, so even these few facets look round
//! and shiny, like the balls in a PS2 demo; otherwise each vertex is shaded
//! on the CPU.
//!
//! Where the graphics card can draw a mesh many times over in one go, the
//! sphere is sent to it once, and each frame only where each particle is,
//! how big and what color: 20 bytes a particle rather than a sphere's
//! worth of vertices. Otherwise the spheres are written out each frame.

use std::f32::consts::{
    PI,
    TAU, //
};

use macroquad::miniquad::{
    BufferLayout,
    Comparison,
    PipelineParams,
    UniformType,
    VertexAttribute,
    VertexFormat,
    VertexStep, //
};
use macroquad::models::{
    Mesh,
    Vertex, //
};
use macroquad::prelude::{
    Camera3D,
    Color,
    Vec2,
    Vec3,
    vec3, //
};
use macroquad::window::get_internal_gl;

use super::gpu::{
    GpuMaterial,
    GpuMesh,
    InstanceBuffer,
    vertex_attributes, //
};
use super::{
    CylinderDimensions,
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY,
    draw_lit,
    load_lit,
    mix_color,
    shade, //
};
use crate::sim::engine::{
    COLLISION_FLASH_DURATION,
    ConnectedChambers,
    PARTICLE_RADIUS, //
};
use crate::sim::gas::{
    self,
    INITIAL_TEMPERATURE, //
};

/// Particles in each mesh, which keeps each within Macroquad's vertex and
/// index buffer limits.
const PARTICLES_PER_BATCH: usize = 256;
/// Rings of a sphere from pole to pole.
const SPHERE_LATITUDE_SEGMENTS: usize = 4;
/// Segments around a sphere.
const SPHERE_LONGITUDE_SEGMENTS: usize = 6;
/// At and above this many particles the spheres are coarser, to reduce
/// vertex work; every particle is still drawn.
const LOW_DETAIL_PARTICLE_THRESHOLD: usize = 1_536;
/// Rings of a coarse sphere.
const LOW_DETAIL_LATITUDE_SEGMENTS: usize = 3;
/// Segments around a coarse sphere.
const LOW_DETAIL_LONGITUDE_SEGMENTS: usize = 4;

/// Color of a particle with no kinetic energy.
///
/// Colors encode each particle's kinetic energy. A single particle has no
/// temperature; temperature is a property of many particles.
const COLD_COLOR: Color = Color::from_rgba(30, 120, 255, 255);
/// Color of a particle with the mean energy at 300 K.
const NORMAL_COLOR: Color = Color::from_rgba(255, 150, 0, 255);
/// Color of a particle with `HOT_ENERGY_MULTIPLIER` times that energy.
const HOT_COLOR: Color = Color::from_rgba(255, 32, 48, 255);

/// Mean kinetic energy per particle at 300 K: 3 kT / 2 in reduced units.
const NORMAL_PARTICLE_ENERGY: f32 = 1.5 * INITIAL_TEMPERATURE;
/// Energy of the hot color, in multiples of the mean at 300 K.
const HOT_ENERGY_MULTIPLIER: f32 = 3.0;

/// The light in camera space: right, up, and toward the viewer.
const LIGHT_DIRECTION: Vec3 = Vec3::new(0.4, 0.7, 0.6);
/// Light that reaches every side of a sphere.
const AMBIENT_LIGHT: f32 = 0.35;
/// Light that falls on the side facing the light.
const DIFFUSE_LIGHT: f32 = 0.65;
/// How far the farthest particles fade toward the background.
const DEPTH_FADE_STRENGTH: f32 = 0.35;

/// Size of a followed particle with tracers on, in radii.
///
/// The followed particles are drawn larger at full brightness and shine;
/// the rest are muted, losing most of their color, much of their
/// brightness and nearly all their shine, so the followed ones and their
/// trails stand out over any background.
const FOLLOWED_SIZE: f32 = 1.6;
/// How far an unfollowed particle turns gray.
const UNFOLLOWED_GRAY: f32 = 0.7;
/// Brightness of an unfollowed particle.
const UNFOLLOWED_BRIGHTNESS: f32 = 0.5;
/// Shine of an unfollowed particle.
const UNFOLLOWED_SHINE: f32 = 0.1;

/// Color of the brief highlight after a collision, which is purely visual
/// and changes no energy.
const COLLISION_FLASH_COLOR: Color = Color::from_rgba(255, 252, 226, 255);
/// How far a fresh collision turns a particle toward the highlight.
const COLLISION_FLASH_STRENGTH: f32 = 0.3;

/// Lit per pixel: the energy color in diffuse light, a white highlight and
/// a rim of the same color toward the edges, both scaled by the shine in
/// the vertex alpha.
const PARTICLE_FRAGMENT: &str = r#"#version 100
precision mediump float;

varying lowp vec4 color;
varying lowp vec2 uv;
varying vec3 world;
varying vec3 surface_normal;

uniform sampler2D Texture;
uniform vec3 Eye;
uniform vec3 Light;

void main() {
    vec3 n = normalize(surface_normal);
    vec3 toward_eye = normalize(Eye - world);
    float diffuse = max(dot(n, Light), 0.0);
    float shine = pow(max(dot(n, normalize(Light + toward_eye)), 0.0), 36.0);
    float rim = pow(1.0 - max(dot(n, toward_eye), 0.0), 2.5);
    vec3 lit = color.rgb * (0.38 + 0.72 * diffuse) + (vec3(0.75 * shine) + color.rgb * rim * 0.35) * color.a;
    gl_FragColor = vec4(lit, 1.0) * texture2D(Texture, uv);
}"#;

/// Vertex shader for drawing the sphere at every particle at once: each
/// copy is moved to its particle's `center`, sized by `size` and colored
/// `tint`, alpha being the shine, and lit by `PARTICLE_FRAGMENT`.
const INSTANCED_VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec3 center;
attribute float size;
attribute vec4 tint;

varying lowp vec4 color;
varying lowp vec2 uv;
varying vec3 world;
varying vec3 surface_normal;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    // A unit sphere's points are its normals.
    world = center + position * size;
    surface_normal = position;
    color = tint / 255.0;
    uv = vec2(0.0);
    gl_Position = Projection * vec4(world, 1.0);
}"#;

/// One particle as the graphics card reads it for `INSTANCED_VERTEX`: the
/// second vertex buffer, stepped once per copy of the sphere.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Instance {
    /// Where its middle is.
    center: [f32; 3],
    /// Its radius.
    size: f32,
    /// Its color, alpha being its shine.
    tint: [u8; 4],
}

/// The attributes `Instance` holds, in its order.
const INSTANCE_ATTRIBUTES: [(&str, VertexFormat); 3] = [
    ("center", VertexFormat::Float3),
    ("size", VertexFormat::Float1),
    ("tint", VertexFormat::Byte4),
];

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

/// A particle's color muted for tracer mode: mostly gray and darker.
fn muted(color: Color) -> Color {
    let gray = 0.3 * color.r + 0.59 * color.g + 0.11 * color.b;
    let grayed = mix_color(
        color,
        Color::new(gray, gray, gray, color.a),
        UNFOLLOWED_GRAY,
    );
    shade(grayed, UNFOLLOWED_BRIGHTNESS)
}

/// Places the light in camera space so orbiting does not darken every sphere.
pub(super) fn camera_light_direction(camera: &Camera3D) -> Vec3 {
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
    /// The camera's line of sight.
    forward: Vec3,
    /// Half the cylinder's depth along the line of sight.
    half_extent: f32,
}

impl DepthFade {
    /// The fade across a cylinder of `radius` and `height` seen by
    /// `camera`.
    ///
    /// # Panics
    ///
    /// Panics if `radius` or `height` is not positive and finite.
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

    /// How far a particle at `position` fades toward the background.
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
    /// Points on the unit sphere, which are also its normals.
    positions: Vec<Vec3>,
    /// Lambert brightness of each point for the current camera.
    brightness: Vec<f32>,
    /// Triangles, three points each, wound outward.
    indices: Vec<u16>,
}

impl SphereTemplate {
    /// The full-detail sphere.
    fn new() -> Self {
        Self::with_segments(SPHERE_LATITUDE_SEGMENTS, SPHERE_LONGITUDE_SEGMENTS)
    }

    /// A sphere of `latitude_segments` rings and `longitude_segments`
    /// segments around.
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

    /// Lights the points for `camera`, once per frame rather than once per
    /// particle.
    fn update_lighting(&mut self, camera: &Camera3D) {
        let light_direction = camera_light_direction(camera);

        for (&normal, brightness) in self.positions.iter().zip(&mut self.brightness) {
            *brightness = lambert_brightness(normal, light_direction);
        }
    }
}

/// How the particles are drawn, once tried.
enum Drawing {
    /// Not tried yet (it needs a window).
    Untried,
    /// The sphere many times over in one go.
    Instanced(Box<Instanced>),
    /// Written out each frame, lit by `material` where it can be built.
    Batches {
        /// The per-pixel lighting, if it can be built.
        material: Option<GpuMaterial>,
    },
}

/// What drawing the sphere many times over in one go takes.
struct Instanced {
    /// The shader.
    material: GpuMaterial,
    /// The sphere, full and coarse, on the graphics card.
    spheres: [GpuMesh; 2],
    /// Each particle's copy, written each frame.
    instances: InstanceBuffer,
    /// The copies being written.
    written: Vec<Instance>,
}

/// Draws every particle as a small shaded sphere: all at once where the
/// graphics card can, or 256 to a mesh.
///
/// Each frame works out each particle's color from its energy relative
/// to the gas's mean motion, plus collision highlight and depth fade, and
/// its size; written out, the spheres also take camera-relative lighting.
/// Written-out meshes are rebuilt only when the particle count changes.
pub struct ParticleRenderer {
    /// The sphere every particle is drawn as.
    template: SphereTemplate,
    /// How the particles are drawn.
    drawing: Drawing,
    /// Whether the shader lights the spheres, so the CPU does not.
    shaded: bool,
    /// The written-out meshes, `PARTICLES_PER_BATCH` particles each.
    batches: Vec<GpuMesh>,
    /// Particles the meshes hold.
    particle_count: usize,
    /// Whether the meshes use the coarse sphere.
    low_detail: bool,
    /// Which particles are followed this frame, by index.
    followed: Vec<bool>,
    /// A cylinder enclosing the whole apparatus, for the depth fade.
    extent: CylinderDimensions,
}

/// One particle's place, size and colors as worked out each frame.
#[derive(Clone, Copy)]
struct Look {
    /// Where its middle is.
    center: Vec3,
    /// Its radius.
    size: f32,
    /// Its color before the depth fade.
    color: Color,
    /// How far it fades toward the background.
    fade: f32,
    /// How much it shines, from 0 to 1.
    shine: f32,
}

impl ParticleRenderer {
    /// A renderer whose depth fade spans `extent`.
    ///
    /// # Panics
    ///
    /// Panics if a batch of spheres would not fit Macroquad's buffers.
    pub fn new(extent: CylinderDimensions) -> Self {
        let template = SphereTemplate::new();
        let vertices = template.positions.len() * PARTICLES_PER_BATCH;
        assert!(
            vertices <= u16::MAX as usize,
            "particle batch exceeds the 16-bit vertex index limit",
        );
        assert!(
            vertices < DRAW_CALL_VERTEX_CAPACITY,
            "particle batch exceeds Macroquad's vertex buffer capacity",
        );
        assert!(
            template.indices.len() * PARTICLES_PER_BATCH < DRAW_CALL_INDEX_CAPACITY,
            "particle batch exceeds Macroquad's index buffer capacity",
        );
        Self {
            template,
            drawing: Drawing::Untried,
            shaded: false,
            batches: Vec::new(),
            particle_count: 0,
            low_detail: false,
            followed: Vec::new(),
            extent,
        }
    }

    /// The way of drawing to use: all at once where the graphics card can
    /// and the shader can be built, written out otherwise.
    fn try_drawing() -> Drawing {
        let gl = unsafe { get_internal_gl() };
        let instancing = gl.quad_context.info().features.instancing;
        let material = instancing.then(instanced_material).flatten();
        match material {
            Some(material) => Drawing::Instanced(Box::new(Instanced {
                material,
                spheres: [
                    sphere_mesh(&SphereTemplate::new()),
                    sphere_mesh(&SphereTemplate::with_segments(
                        LOW_DETAIL_LATITUDE_SEGMENTS,
                        LOW_DETAIL_LONGITUDE_SEGMENTS,
                    )),
                ],
                instances: InstanceBuffer::new(),
                written: Vec::new(),
            })),
            None => Drawing::Batches {
                material: load_lit(PARTICLE_FRAGMENT, &[]),
            },
        }
    }

    /// Rebuilds the meshes for `particle_count` particles, if that changed.
    fn resize_batches(&mut self, particle_count: usize) {
        if particle_count == self.particle_count {
            return;
        }
        self.set_detail(particle_count);
        let vertices_per_particle = self.template.positions.len();
        self.batches.clear();
        for start in (0..particle_count).step_by(PARTICLES_PER_BATCH) {
            let count = (particle_count - start).min(PARTICLES_PER_BATCH);
            let mut vertices = Vec::with_capacity(count * vertices_per_particle);
            let mut indices = Vec::with_capacity(count * self.template.indices.len());
            for particle_index in 0..count {
                // Colors and positions are overwritten in `write_batches`.
                for &position in &self.template.positions {
                    // A unit sphere's points are its normals.
                    let mut vertex = Vertex::new2(position, Vec2::ZERO, NORMAL_COLOR);
                    vertex.normal = position.extend(0.0);
                    vertices.push(vertex);
                }
                let base = (particle_index * vertices_per_particle) as u16;
                indices.extend(self.template.indices.iter().map(|&index| base + index));
            }
            self.batches.push(GpuMesh::new(Mesh {
                vertices,
                indices,
                texture: None,
            }));
        }
        self.particle_count = particle_count;
    }

    /// Uses the coarse sphere for `particle_count` particles or more than
    /// `LOW_DETAIL_PARTICLE_THRESHOLD`, the full one otherwise.
    fn set_detail(&mut self, particle_count: usize) {
        let low_detail = particle_count >= LOW_DETAIL_PARTICLE_THRESHOLD;
        if low_detail != self.low_detail {
            self.template = if low_detail {
                let (latitude, longitude) =
                    (LOW_DETAIL_LATITUDE_SEGMENTS, LOW_DETAIL_LONGITUDE_SEGMENTS);
                SphereTemplate::with_segments(latitude, longitude)
            } else {
                SphereTemplate::new()
            };
            self.low_detail = low_detail;
        }
    }

    /// Draws the particles of `simulation` for `camera`, the distant ones
    /// fading toward `background`.
    ///
    /// With tracers on, the followed particles stand out.
    pub fn draw(&mut self, simulation: &ConnectedChambers, camera: &Camera3D, background: Color) {
        if let Drawing::Untried = self.drawing {
            self.drawing = Self::try_drawing();
        }
        let light = camera_light_direction(camera);
        let looks = self.looks(simulation, camera);
        match &mut self.drawing {
            Drawing::Instanced(instanced) => {
                let Instanced {
                    material,
                    spheres,
                    instances,
                    written,
                } = &mut **instanced;
                written.clear();
                written.extend(looks.map(|look| {
                    let mut lit = mix_color(look.color, background, look.fade);
                    lit.a = look.shine;
                    Instance {
                        center: look.center.to_array(),
                        size: look.size,
                        tint: lit.into(),
                    }
                }));
                instances.write(written);
                let sphere = &spheres[usize::from(self.low_detail)];
                material.set_uniform("Eye", camera.position);
                material.set_uniform("Light", light);
                material.draw_instanced(sphere, instances);
            }
            Drawing::Batches { material } => {
                let material = material.as_ref();
                self.shaded = material.is_some();
                let looks: Vec<Look> = looks.collect();
                self.write_batches(&looks, camera, background);
                let Drawing::Batches { material } = &self.drawing else {
                    unreachable!();
                };
                draw_lit(&self.batches, material.as_ref(), camera, light);
            }
            Drawing::Untried => unreachable!("tried above"),
        }
    }

    /// Each particle of `simulation`'s place, size and colors for `camera`.
    fn looks<'a>(
        &mut self,
        simulation: &'a ConnectedChambers,
        camera: &Camera3D,
    ) -> impl Iterator<Item = Look> + 'a {
        let particles = &simulation.particles;
        let flashes = simulation.collision_flashes();
        self.set_detail(particles.len());
        self.followed.clear();
        self.followed.resize(particles.len(), false);
        let tracers = simulation.tracers();
        for index in tracers.into_iter().flat_map(|tracers| tracers.indices()) {
            self.followed[index] = true;
        }
        let tracing = tracers.is_some();
        let depth_fade = DepthFade::new(camera, self.extent.radius, self.extent.height);
        let mean_velocity = gas::mean_velocity(particles);
        let followed = self.followed.clone();
        particles.iter().enumerate().map(move |(index, particle)| {
            // Ignore the gas's bulk drift: only relative kinetic energy
            // should determine its color.
            let energy = gas::particle_kinetic_energy(particle.velocity - mean_velocity);
            let color = collision_color(kinetic_energy_color(energy), flashes[index]);
            let fade = || depth_fade.amount(particle.position);
            let (color, fade, size, shine) = match (tracing, followed[index]) {
                (true, true) => (color, 0.0, PARTICLE_RADIUS * FOLLOWED_SIZE, 1.0),
                (true, false) => (muted(color), fade(), PARTICLE_RADIUS, UNFOLLOWED_SHINE),
                (false, _) => (color, fade(), PARTICLE_RADIUS, 1.0),
            };
            Look {
                center: particle.position,
                size,
                color,
                fade,
                shine,
            }
        })
    }

    /// Writes the spheres of `looks` out into the meshes, lit for `camera`
    /// on the CPU unless the shader lights them, fading toward
    /// `background`.
    fn write_batches(&mut self, looks: &[Look], camera: &Camera3D, background: Color) {
        self.resize_batches(looks.len());
        self.template.update_lighting(camera);
        let vertices_per_particle = self.template.positions.len();
        for (index, look) in looks.iter().enumerate() {
            let batch = self.batches[index / PARTICLES_PER_BATCH].edit();
            let first = index % PARTICLES_PER_BATCH * vertices_per_particle;
            let vertices = &mut batch.vertices[first..first + vertices_per_particle];
            let template = self
                .template
                .positions
                .iter()
                .zip(&self.template.brightness);
            if self.shaded {
                // The shader lights each pixel, so every point of the
                // sphere takes one color, worked out once. It reads alpha
                // as shine.
                let mut lit = mix_color(look.color, background, look.fade);
                lit.a = look.shine;
                let lit: [u8; 4] = lit.into();
                for (vertex, (&offset, _)) in vertices.iter_mut().zip(template) {
                    vertex.position = look.center + offset * look.size;
                    vertex.color = lit;
                }
            } else {
                for (vertex, (&offset, &brightness)) in vertices.iter_mut().zip(template) {
                    vertex.position = look.center + offset * look.size;
                    // Drawn plainly, alpha would be transparency.
                    let mut lit = mix_color(shade(look.color, brightness), background, look.fade);
                    lit.a = 1.0;
                    vertex.color = lit.into();
                }
            }
        }
    }
}

/// The material that draws the sphere at every particle at once, or
/// `None` if its shader cannot be built.
fn instanced_material() -> Option<GpuMaterial> {
    let layout = [
        BufferLayout::default(),
        BufferLayout {
            step_func: VertexStep::PerInstance,
            ..Default::default()
        },
    ];
    let attributes: Vec<VertexAttribute> = vertex_attributes()
        .into_iter()
        .chain(
            INSTANCE_ATTRIBUTES
                .iter()
                .map(|&(name, format)| VertexAttribute::with_buffer(name, format, 1)),
        )
        .collect();
    let params = PipelineParams {
        depth_write: true,
        depth_test: Comparison::LessOrEqual,
        ..Default::default()
    };
    let lighting = [("Eye", UniformType::Float3), ("Light", UniformType::Float3)];
    GpuMaterial::with_layout(
        INSTANCED_VERTEX,
        PARTICLE_FRAGMENT,
        &lighting,
        params,
        &layout,
        &attributes,
    )
}

/// `template` as a mesh of one sphere at the origin, of radius 1.
fn sphere_mesh(template: &SphereTemplate) -> GpuMesh {
    let vertices = template
        .positions
        .iter()
        .map(|&position| {
            let mut vertex = Vertex::new2(position, Vec2::ZERO, NORMAL_COLOR);
            vertex.normal = position.extend(0.0);
            vertex
        })
        .collect();
    GpuMesh::new(Mesh {
        vertices,
        indices: template.indices.clone(),
        texture: None,
    })
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/particles.rs"]
mod tests;
