//! The wooden table the engine stands on.
//!
//! The engine is a desk model, about 25 cm tall, so it stands on a small
//! table: a top of planks laid along the shaft, an apron under its edge,
//! and four square legs reaching down to the cloud or the grid. A shader
//! draws the wood's grain from each point's place, so the table needs no
//! texture. Built once and drawn in one call.
//!
//! While the first-person figure is dragged over it, the top lights up, a
//! soft glow with a brighter rim pulsing gently, to show it can be dropped
//! there. Drawing only.

use macroquad::models::{
    Mesh,
    Vertex, //
};
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
    load_clear,
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

/// Fragment shader for the highlight: its own color, unlit, blended over
/// the top.
const HIGHLIGHT_FRAGMENT: &str = r#"#version 100
precision mediump float;

varying lowp vec4 color;
varying lowp vec2 uv;
varying vec3 world;
varying vec3 surface_normal;

uniform sampler2D Texture;
uniform vec3 Eye;
uniform vec3 Light;

void main() {
    gl_FragColor = color * texture2D(Texture, uv);
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

/// The highlight's color; its opacity is set as it glows.
const HIGHLIGHT_COLOR: Color = Color::new(1.0, 0.92, 0.5, 1.0);
/// Opacity of the highlight's middle at full glow.
const HIGHLIGHT_FILL: f32 = 0.42;
/// Opacity of the highlight's rim at full glow.
const HIGHLIGHT_RIM: f32 = 0.95;
/// Width of the highlight's rim, in bore radii.
const HIGHLIGHT_RIM_WIDTH: f32 = 0.8;
/// How far above the top the highlight lies, so it never fights the top
/// for depth: the softened view's 16-bit depth buffer tells surfaces apart
/// only a few hundredths of a unit apart at the usual distance.
const HIGHLIGHT_RAISE: f32 = 0.15;
/// Pulses of the highlight per second.
const HIGHLIGHT_PULSE_RATE: f32 = 1.4;
/// How far the pulse dims the highlight at its faintest, as a share.
const HIGHLIGHT_PULSE: f32 = 0.3;
/// Seconds the highlight takes to come on or go out.
const HIGHLIGHT_SECONDS: f32 = 0.15;

/// Whether `point`, seen from above (x, z), is over the table's top under a
/// block of `shape`.
pub fn over_top(shape: BlockShape, point: Vec2) -> bool {
    point.x.abs() <= TOP_HALF_LENGTH * shape.bore && point.y.abs() <= TOP_HALF_DEPTH * shape.bore
}

/// Height of the table's top surface under a block of `shape`.
pub fn top_height(shape: BlockShape) -> f32 {
    shape.base - BELOW_BLOCK
}

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
    /// The highlight over the top: its middle's four corners, then its
    /// rim's.
    highlight: Mesh,
    /// The highlight's see-through material, made on its first draw;
    /// `Some(None)` where it cannot be built, and the top is not lit.
    highlight_material: Option<Option<Material>>,
    /// How strongly the highlight shows, from 0 to 1.
    glow: f32,
    /// The clock at the last highlight, if there was one.
    last_seconds: Option<f64>,
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
            highlight: highlight_mesh(shape),
            highlight_material: None,
            glow: 0.0,
            last_seconds: None,
        }
    }

    /// Lights the top while `on`, easing in and out, pulsing gently, at
    /// `seconds` on the clock, seen from `camera`; drawn see-through, right
    /// after the table and before what stands on it.
    pub fn draw_highlight(&mut self, on: bool, seconds: f64, camera: &Camera3D) {
        let dt = self
            .last_seconds
            .map_or(0.0, |last| (seconds - last).max(0.0) as f32);
        self.last_seconds = Some(seconds);
        let step = dt / HIGHLIGHT_SECONDS;
        let target = if on { 1.0 } else { 0.0 };
        self.glow += (target - self.glow).clamp(-step, step);
        if self.glow <= 0.0 {
            return;
        }
        let pulse = 1.0
            - HIGHLIGHT_PULSE
                * (0.5
                    - 0.5 * (std::f32::consts::TAU * HIGHLIGHT_PULSE_RATE * seconds as f32).cos());
        let strength = self.glow * pulse;
        for (index, vertex) in self.highlight.vertices.iter_mut().enumerate() {
            let opacity = if index < 4 {
                HIGHLIGHT_FILL
            } else {
                HIGHLIGHT_RIM
            };
            vertex.color = Color {
                a: opacity * strength,
                ..HIGHLIGHT_COLOR
            }
            .into();
        }
        let material = self
            .highlight_material
            .get_or_insert_with(|| load_clear(HIGHLIGHT_FRAGMENT));
        if let Some(material) = material {
            let light = key_light(camera.position);
            draw_lit([&self.highlight], Some(material), camera, light);
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

/// The highlight over the top of the table under a block of `shape`: a
/// middle and a rim around it, colored when drawn.
fn highlight_mesh(shape: BlockShape) -> Mesh {
    let y = top_height(shape) + HIGHLIGHT_RAISE;
    let (length, depth) = (TOP_HALF_LENGTH * shape.bore, TOP_HALF_DEPTH * shape.bore);
    let rim = HIGHLIGHT_RIM_WIDTH * shape.bore;
    let corners = |x: f32, z: f32| {
        [
            vec3(-x, y, -z),
            vec3(x, y, -z),
            vec3(x, y, z),
            vec3(-x, y, z),
        ]
    };
    let inner = corners(length - rim, depth - rim);
    let outer = corners(length, depth);
    let vertices: Vec<Vertex> = inner
        .into_iter()
        .chain(outer)
        .map(|position| Vertex::new2(position, Vec2::ZERO, HIGHLIGHT_COLOR))
        .collect();
    // The middle, then a band from each inner edge to its outer edge.
    let mut indices = vec![0, 1, 2, 0, 2, 3];
    for side in 0..4u16 {
        let next = (side + 1) % 4;
        indices.extend([side, next, 4 + next, side, 4 + next, 4 + side]);
    }
    Mesh {
        vertices,
        indices,
        texture: None,
    }
}

/// The table's top, apron and legs under a block of `shape`.
fn build(shape: BlockShape) -> Solid {
    let unit = shape.bore;
    let mut solid = Solid::new();
    let top = top_height(shape);
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
