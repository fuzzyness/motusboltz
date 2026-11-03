//! The wooden table the engine stands on.
//!
//! The engine is a desk model, about 25 cm tall, so it stands on a small
//! table: a top of planks laid along the shaft, an apron under its edge,
//! and four square legs reaching down to the cloud or the grid. A shader
//! draws the wood's grain from each point's place, so the table needs no
//! texture. Built once and drawn in one call. Drawing only.

use macroquad::models::Mesh;
use macroquad::prelude::{
    Camera3D,
    Color,
    Material,
    Vec2,
    Vec3,
    vec2,
    vec3, //
};

use super::block::{
    BlockShape,
    key_light, //
};
use super::solid::Solid;
use super::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY,
    draw_lit,
    load_lit, //
};

/// Fragment shader for the wood: rings of grain warped by noise and fine
/// streaks along it, darker seams between the top's planks, and a soft
/// varnish sheen. Each vertex's alpha says which way its grain runs: 1
/// along the shaft (the top and the apron), 0 up and down (the legs).
const WOOD_FRAGMENT: &str = r#"#version 100
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
    if (dot(n, toward_eye) < 0.0) {
        n = -n;
    }
    // Along the grain and across it, in centimeters.
    float flat_grain = step(0.5, color.a);
    float along = mix(world.y, world.x, flat_grain);
    float across = mix(world.x + world.z, world.z + 0.6 * world.y, flat_grain);
    float warp = 1.6 * noise(vec2(along * 0.08, across * 0.35))
        + 0.4 * noise(vec2(along * 0.25, across * 0.9));
    float ring = fract(across * 0.55 + warp);
    float late = smoothstep(0.55, 0.95, ring) * (1.0 - smoothstep(0.95, 1.0, ring));
    float streak = noise(vec2(along * 0.6, across * 9.0));
    vec3 wood = color.rgb * (1.0 - 0.22 * late) * (0.93 + 0.12 * streak);
    // Planks 7 cm wide, seen on the top only.
    float plank = fract(world.z / 7.0 + 0.5);
    float seam = (1.0 - smoothstep(0.0, 0.012, plank) * smoothstep(1.0, 0.988, plank))
        * step(0.9, n.y) * flat_grain;
    wood *= 1.0 - 0.45 * seam;
    float diffuse = max(dot(n, Light), 0.0);
    float shine = 0.18 * pow(max(dot(n, normalize(Light + toward_eye)), 0.0), 30.0);
    gl_FragColor = vec4(wood * (0.42 + 0.6 * diffuse) + vec3(shine), 1.0);
}"#;

/// Half the top's length, along the shaft.
///
/// Sizes are in bore radii, so the table keeps its proportions at any
/// engine size.
pub const TOP_HALF_LENGTH: f32 = 8.0;
/// Half the top's depth, front to back.
pub const TOP_HALF_DEPTH: f32 = 5.5;
/// Height of the top's surface above the floor the legs stand on.
pub const TABLE_HEIGHT: f32 = 9.0;
/// Thickness of the top.
const TOP_THICKNESS: f32 = 0.9;
/// Just below the block's underside, so the two never fight for depth.
const BELOW_BLOCK: f32 = 0.004;
/// Half the width of a leg.
const LEG_HALF_WIDTH: f32 = 0.45;
/// How far in from the top's corners the legs stand, to their middles.
const LEG_INSET: f32 = 1.0;
/// Depth of the apron under the top's edge.
const APRON_DEPTH: f32 = 1.4;
/// Half the thickness of the apron's boards.
const APRON_HALF_THICKNESS: f32 = 0.16;
/// The grain runs along the shaft (see the shader).
const ALONG_SHAFT: f32 = 1.0;
/// The grain runs up and down (see the shader).
const UPRIGHT: f32 = 0.0;
/// The top's wood, a warm oak; alpha is the grain's way.
const TOP_WOOD: Color = Color::new(0.69, 0.47, 0.28, ALONG_SHAFT);
/// The apron's wood, a little darker.
const APRON_WOOD: Color = Color::new(0.6, 0.4, 0.24, ALONG_SHAFT);
/// The legs' wood.
const LEG_WOOD: Color = Color::new(0.6, 0.4, 0.24, UPRIGHT);

/// Height of the floor the table's legs stand on, under a block of
/// `shape`.
pub fn ground(shape: BlockShape) -> f32 {
    shape.base - TABLE_HEIGHT * shape.bore
}

/// Where the middles of the table's four feet stand, seen from above
/// (x, z), under a block of `shape`.
pub fn feet(shape: BlockShape) -> [Vec2; 4] {
    let x = (TOP_HALF_LENGTH - LEG_INSET) * shape.bore;
    let z = (TOP_HALF_DEPTH - LEG_INSET) * shape.bore;
    [vec2(-x, -z), vec2(x, -z), vec2(x, z), vec2(-x, z)]
}

/// The table under the engine.
pub struct Table {
    /// The top, the apron and the legs.
    mesh: Mesh,
    /// Made on the first draw (it needs a window); `Some(None)` where it
    /// cannot be built, and the table is drawn in plain colors.
    material: Option<Option<Material>>,
}

impl Table {
    /// The table under a block of `shape`.
    ///
    /// # Panics
    ///
    /// Panics if the table would not fit Macroquad's buffers.
    pub fn new(shape: BlockShape) -> Self {
        let solid = build(shape);
        assert!(solid.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
        assert!(solid.indices.len() < DRAW_CALL_INDEX_CAPACITY);
        Self {
            mesh: Mesh {
                vertices: solid.vertices,
                indices: solid.indices,
                texture: None,
            },
            material: None,
        }
    }

    /// Draws the table seen from `camera`.
    pub fn draw(&mut self, camera: &Camera3D) {
        let material = self.material.get_or_insert_with(|| {
            let material = load_lit(WOOD_FRAGMENT, &[]);
            if material.is_none() {
                // Drawn in plain colors, alpha means transparency.
                for vertex in &mut self.mesh.vertices {
                    vertex.color[3] = u8::MAX;
                }
            }
            material
        });
        let light = key_light(camera.position);
        draw_lit([&self.mesh], material.as_ref(), camera, light);
    }
}

/// The table's top, apron and legs under a block of `shape`.
fn build(shape: BlockShape) -> Solid {
    let unit = shape.bore;
    let mut solid = Solid::new();
    let top = shape.base - BELOW_BLOCK;
    let underside = top - TOP_THICKNESS * unit;
    let (length, depth) = (TOP_HALF_LENGTH * unit, TOP_HALF_DEPTH * unit);
    let middle_y = (top + underside) / 2.0;
    let half_thick = TOP_THICKNESS * unit / 2.0;
    solid.bar(
        vec3(-length, middle_y, 0.0),
        vec3(length, middle_y, 0.0),
        Vec3::Z,
        (depth, depth),
        half_thick,
        TOP_WOOD,
    );
    // The apron: a board under each edge, between the legs.
    let (reach_x, reach_z) = (length - LEG_INSET * unit, depth - LEG_INSET * unit);
    let apron_y = underside - APRON_DEPTH * unit / 2.0;
    let apron_half = APRON_DEPTH * unit / 2.0;
    let board = APRON_HALF_THICKNESS * unit;
    for side in [-1.0, 1.0] {
        solid.bar(
            vec3(-reach_x, apron_y, side * reach_z),
            vec3(reach_x, apron_y, side * reach_z),
            Vec3::Y,
            (apron_half, apron_half),
            board,
            APRON_WOOD,
        );
        solid.bar(
            vec3(side * reach_x, apron_y, -reach_z),
            vec3(side * reach_x, apron_y, reach_z),
            Vec3::Y,
            (apron_half, apron_half),
            board,
            APRON_WOOD,
        );
    }
    let leg = LEG_HALF_WIDTH * unit;
    for foot in feet(shape) {
        solid.bar(
            vec3(foot.x, ground(shape), foot.y),
            vec3(foot.x, underside, foot.y),
            Vec3::X,
            (leg, leg),
            leg,
            LEG_WOOD,
        );
    }
    solid
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/table.rs"]
mod tests;
