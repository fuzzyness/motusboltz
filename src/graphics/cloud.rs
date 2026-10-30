//! The cloud the engine stands on in the sky scenery: a cartoon cloud
//! built from flattened puffs, in the style of a Dreamcast game. Puffs
//! side by side make its gently rolling top at floor height, others bulge
//! a little along its edge, and more billow underneath. It is solid
//! geometry, so the engine sits on it cleanly from any angle; a soft
//! shader lights it in pale blue-white, bluer in the shade, mottled like
//! a painted texture, with a pale rim. Built once and drawn in one call.
//! Drawing only.

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
    Comparison,
    Material,
    MaterialParams,
    PipelineParams,
    ShaderSource,
    UniformDesc,
    UniformType,
    Vec2,
    Vec3,
    Vec4,
    gl_use_default_material,
    gl_use_material,
    load_material,
    vec3, //
};

use super::block::BlockShape;
use super::sprites::unit_hash;
use super::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY, //
};

const CLOUD_VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
attribute vec4 normal;

varying lowp vec4 color;
varying lowp vec2 uv;
varying vec3 world;
varying vec3 surface_normal;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    vec4 placed = Model * vec4(position, 1.0);
    world = placed.xyz;
    surface_normal = normal.xyz;
    color = color0 / 255.0;
    uv = texcoord;
    gl_Position = Projection * placed;
}"#;

/// Pale blue-white where the light falls and on top, a soft sky blue in
/// the shade (never grey), mottled by a little noise as a painted texture
/// would be, and a pale rim toward the edges.
const CLOUD_FRAGMENT: &str = r#"#version 100
precision mediump float;

varying lowp vec4 color;
varying lowp vec2 uv;
varying vec3 world;
varying vec3 surface_normal;

uniform sampler2D Texture;
uniform vec3 Eye;
uniform vec3 Light;

float hash(vec2 p) {
    vec3 q = fract(vec3(p.xyx) * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
               mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

void main() {
    vec3 n = normalize(surface_normal);
    vec3 toward_eye = normalize(Eye - world);
    float light = max(dot(n, Light), 0.0);
    float up = n.y * 0.5 + 0.5;
    vec3 shade = mix(vec3(0.52, 0.64, 0.9), vec3(0.93, 0.95, 1.0), smoothstep(0.0, 0.9, light * 0.6 + up * 0.5));
    vec2 place = world.xz * 0.5 + vec2(world.y * 0.7, 0.0);
    float mottle = 0.6 * noise(place) + 0.4 * noise(place * 2.7);
    shade *= 0.9 + 0.12 * mottle;
    float rim = pow(1.0 - max(dot(n, toward_eye), 0.0), 2.0);
    vec3 lit = color.rgb * shade + vec3(0.1, 0.12, 0.16) * rim;
    gl_FragColor = vec4(lit, 1.0) * texture2D(Texture, uv);
}"#;

// Sizes are fractions of the bore's radius, so the cloud keeps its
// proportions at any engine size.

/// Half the cloud across (along the shaft) and front to back.
const HALF_LENGTH: f32 = 5.0;
const HALF_DEPTH: f32 = 3.6;
/// Every puff is squashed to this share of its width in height, so the
/// top rolls gently instead of bulging.
const FLATTEN: f32 = 0.35;
/// The puffs that make the top: their radius and spacing. Their tops sit
/// just under the floor.
const TOP_RADIUS: f32 = 1.7;
const TOP_SPACING: f32 = 1.3;
const BELOW_FLOOR: f32 = 0.02;
/// How far each top puff strays from its place in the grid, as a share of
/// the spacing, and how much its size varies, so they do not look tiled.
const TOP_JITTER: f32 = 0.3;
const TOP_SIZE_SPREAD: f32 = 0.25;
/// The top fills the outline out to this share of it (in squared terms),
/// leaving the edge to the rings.
const TOP_FILL: f32 = 0.85;

/// Rings of puffs around and under the top: how many, how far out (as a
/// share of the cloud's size), their smallest and largest radius, and how
/// high their middles sit against the floor (as a share of their height,
/// then a drop in bore radii).
struct Ring {
    puffs: u32,
    reach: f32,
    radius: (f32, f32),
    rise: f32,
    drop: f32,
}

const RINGS: [Ring; 3] = [
    // Low bumps along the edge, rising a little above the top.
    Ring {
        puffs: 30,
        reach: 1.0,
        radius: (0.7, 1.0),
        rise: -0.7,
        drop: 0.0,
    },
    // Billows under the edge.
    Ring {
        puffs: 22,
        reach: 0.9,
        radius: (1.0, 1.4),
        rise: -1.0,
        drop: 0.25,
    },
    // The underside.
    Ring {
        puffs: 12,
        reach: 0.55,
        radius: (1.3, 1.7),
        rise: -1.0,
        drop: 0.8,
    },
];
const PUFF_COLOR: Color = Color::from_rgba(240, 244, 252, 255);
/// Facets: rings from pole to pole, and around.
const LATITUDES: usize = 7;
const LONGITUDES: usize = 12;

const SALT_ANGLE: u32 = 21;
const SALT_RADIUS: u32 = 22;
const SALT_REACH: u32 = 23;

/// One puff: its middle and its radius across (its height is `FLATTEN`
/// of that).
#[derive(Clone, Copy, Debug)]
struct Puff {
    center: Vec3,
    radius: f32,
}

pub struct CloudFloor {
    mesh: Mesh,
    /// Made on the first draw (it needs a window); `Some(None)` where it
    /// cannot be built, and the cloud is drawn flat white.
    material: Option<Option<Material>>,
}

impl CloudFloor {
    pub fn new(shape: BlockShape) -> Self {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for puff in puffs(shape) {
            add_sphere(&mut vertices, &mut indices, puff);
        }
        assert!(vertices.len() <= DRAW_CALL_VERTEX_CAPACITY);
        assert!(indices.len() <= DRAW_CALL_INDEX_CAPACITY);
        Self {
            mesh: Mesh {
                vertices,
                indices,
                texture: None,
            },
            material: None,
        }
    }

    /// Draws the cloud seen from `camera`, lit from `light`.
    pub fn draw(&mut self, camera: &Camera3D, light: Vec3) {
        let material = self.material.get_or_insert_with(load_cloud);
        if let Some(material) = material {
            material.set_uniform("Eye", camera.position);
            material.set_uniform("Light", light);
            gl_use_material(material);
        }
        draw_mesh(&self.mesh);
        gl_use_default_material();
    }
}

/// Every puff: a grid of them inside the outline for the top, then the
/// rings around and under it.
fn puffs(shape: BlockShape) -> Vec<Puff> {
    let unit = shape.bore;
    let (length, depth) = (HALF_LENGTH * unit, HALF_DEPTH * unit);
    let mut puffs = Vec::new();
    let radius = TOP_RADIUS * unit;
    let spacing = TOP_SPACING * unit;
    let (columns, rows) = (
        (length / spacing).ceil() as i32,
        (depth / spacing).ceil() as i32 + 1,
    );
    for row in -rows..=rows {
        for column in -columns..=columns {
            // Every other row shifted half a step, so the puffs pack.
            let x = (column as f32 + 0.5 * (row & 1) as f32) * spacing;
            let z = row as f32 * spacing * 0.87;
            if (x / length).powi(2) + (z / depth).powi(2) > TOP_FILL {
                continue;
            }
            let seed = ((row + rows) * (2 * columns + 1) + column + columns) as u32 + 1_000;
            let jitter = |salt| TOP_JITTER * spacing * (2.0 * unit_hash(seed, salt) - 1.0);
            let (x, z) = (x + jitter(SALT_ANGLE), z + jitter(SALT_REACH));
            let radius = radius * (1.0 + TOP_SIZE_SPREAD * (unit_hash(seed, SALT_RADIUS) - 0.5));
            puffs.push(Puff {
                center: vec3(x, shape.base - BELOW_FLOOR - FLATTEN * radius, z),
                radius,
            });
        }
    }
    let mut index = 0;
    for ring in &RINGS {
        for slot in 0..ring.puffs {
            index += 1;
            let random = |salt| unit_hash(index, salt);
            let angle = TAU * (slot as f32 + 0.6 * random(SALT_ANGLE)) / ring.puffs as f32;
            let reach = ring.reach * (0.94 + 0.1 * random(SALT_REACH));
            let radius =
                unit * (ring.radius.0 + (ring.radius.1 - ring.radius.0) * random(SALT_RADIUS));
            puffs.push(Puff {
                center: vec3(
                    length * reach * angle.cos(),
                    shape.base + ring.rise * FLATTEN * radius - ring.drop * unit,
                    depth * reach * angle.sin(),
                ),
                radius,
            });
        }
    }
    puffs
}

/// A flattened puff. A sphere squashed in height keeps its normals square
/// to the surface by stretching them the other way.
fn add_sphere(vertices: &mut Vec<Vertex>, indices: &mut Vec<u16>, puff: Puff) {
    let first = vertices.len() as u16;
    for latitude in 0..=LATITUDES {
        let theta = PI * latitude as f32 / LATITUDES as f32;
        for longitude in 0..=LONGITUDES {
            let phi = TAU * longitude as f32 / LONGITUDES as f32;
            let round = vec3(
                theta.sin() * phi.cos(),
                theta.cos(),
                theta.sin() * phi.sin(),
            );
            let squash = vec3(1.0, FLATTEN, 1.0);
            let normal = (round / squash).normalize();
            let mut vertex = Vertex::new2(
                puff.center + round * squash * puff.radius,
                Vec2::ZERO,
                PUFF_COLOR,
            );
            vertex.normal = Vec4::new(normal.x, normal.y, normal.z, 0.0);
            vertices.push(vertex);
        }
    }
    let row = (LONGITUDES + 1) as u16;
    for latitude in 0..LATITUDES as u16 {
        for longitude in 0..LONGITUDES as u16 {
            let a = first + latitude * row + longitude;
            let b = a + row;
            indices.extend([a, b, a + 1, a + 1, b, b + 1]);
        }
    }
}

fn load_cloud() -> Option<Material> {
    load_material(
        ShaderSource::Glsl {
            vertex: CLOUD_VERTEX,
            fragment: CLOUD_FRAGMENT,
        },
        MaterialParams {
            pipeline_params: PipelineParams {
                depth_write: true,
                depth_test: Comparison::LessOrEqual,
                ..Default::default()
            },
            uniforms: vec![
                UniformDesc::new("Eye", UniformType::Float3),
                UniformDesc::new("Light", UniformType::Float3),
            ],
            ..Default::default()
        },
    )
    .ok()
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/cloud.rs"]
mod tests;
