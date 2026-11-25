//! The pine desk the engine stands on.
//!
//! The engine is a desk model, about 25 cm tall, so it stands on a small
//! student's desk of honey pine: a top with a rounded edge, overhanging a
//! little at the front and the ends; a solid end panel down to the ground
//! at its left; at its right a pedestal of three drawers from the top down
//! to an arched base (see `drawers.rs` for the pedestal and its drawers);
//! between them the knee space, a shallow pencil drawer across its top
//! and a rail across its back. A shader draws the wood's grain from each
//! point's place, so the desk needs no texture. Built once and drawn in
//! one call.
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
    UniformType,
    Vec2,
    Vec3,
    vec3, //
};

use super::block::{
    BlockShape,
    key_light, //
};
use super::drawers::{
    DrawerPlace,
    cabinet_place,
    cabinet_solid,
    drawer_solid, //
};
use super::gpu::{
    GpuMaterial,
    GpuMesh, //
};
use super::solid::Solid;
use super::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY,
    draw_lit,
    load_clear,
    load_lit, //
};

/// Fragment shader for the wood: soft rings of grain warped by noise, fine
/// streaks along it, and a soft varnish sheen. Each
/// vertex's alpha says which way its grain runs: 1 along the shaft (the
/// top, the rail and the drawers' fronts), 0 up and down (the end panel
/// and the pedestal).
pub(super) const WOOD_FRAGMENT: &str = r#"#version 100
precision mediump float;

varying lowp vec4 color;
varying lowp vec2 uv;
varying vec3 world;
varying vec3 surface_normal;

uniform sampler2D Texture;
uniform vec3 Eye;
uniform vec3 Light;
// How far the part has moved from where it was built, so its grain moves
// with it: a drawer's, as it is pulled out.
uniform vec3 Shift;

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
    // Along the grain and across it, in centimeters; upright grain's
    // rings are spaced wider, as on a pine panel's broad face.
    vec3 built = world - Shift;
    float flat_grain = step(0.5, color.a);
    float along = mix(built.y, built.x, flat_grain);
    float across = mix(0.45 * (built.x + built.z), built.z + 0.6 * built.y, flat_grain);
    float warp = 1.6 * noise(vec2(along * 0.08, across * 0.35))
        + 0.4 * noise(vec2(along * 0.25, across * 0.9));
    float ring = fract(across * 0.55 + warp);
    float late = smoothstep(0.55, 0.95, ring) * (1.0 - smoothstep(0.95, 1.0, ring));
    float streak = noise(vec2(along * 0.6, across * 9.0));
    vec3 wood = color.rgb * (1.0 - 0.14 * late) * (0.96 + 0.06 * streak);
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
/// Sizes are in bore radii, so the desk keeps its proportions at any
/// engine size.
pub const TOP_HALF_LENGTH: f32 = 10.0;
/// Half the top's depth, front to back.
pub const TOP_HALF_DEPTH: f32 = 7.0;
/// Height of the top's surface above the ground the desk stands on.
pub const TABLE_HEIGHT: f32 = 5.3;
/// Thickness of the top.
pub const TOP_THICKNESS: f32 = 0.55;
/// How far the top reaches past the desk's body at the front and the ends;
/// at the back the two are flush.
pub const OVERHANG: f32 = 0.3;
/// Thickness of the end panel at the left.
const END_PANEL: f32 = 0.3;
/// Height of the pencil drawer's front, across the top of the knee space.
const PENCIL_HEIGHT: f32 = 0.75;
/// How far back the pencil drawer reaches from the desk's front.
const PENCIL_DEPTH: f32 = 8.0;
/// Room left round the pencil drawer, between it and what it stands in.
const PENCIL_GAP: f32 = 0.06;
/// Height of the bottom of the rail across the back of the knee space.
const RAIL_LOW: f32 = 1.5;
/// Height of the rail's board.
const RAIL_HEIGHT: f32 = 0.6;
/// Thickness of the rail's board.
const RAIL_THICKNESS: f32 = 0.22;
/// How far in from the desk's back the rail stands, to its middle.
const RAIL_INSET: f32 = 0.6;
/// Segments round the top's rounded edge.
const EDGE_SEGMENTS: usize = 12;
/// Rings from pole to pole of the top's rounded corners.
const CORNER_RINGS: usize = 6;
/// Just below the block's underside, so the two never fight for depth.
const BELOW_BLOCK: f32 = 0.004;
/// The grain runs along the shaft (see the shader).
pub(super) const ALONG_SHAFT: f32 = 1.0;
/// The grain runs up and down (see the shader).
pub(super) const UPRIGHT: f32 = 0.0;
/// The top's wood, a honey pine; alpha is the grain's way.
const TOP_WOOD: Color = Color::new(0.94, 0.67, 0.35, ALONG_SHAFT);
/// The rail's wood, a little deeper.
const RAIL_WOOD: Color = Color::new(0.88, 0.61, 0.31, ALONG_SHAFT);
/// The end panel's and the pedestal's wood, its grain upright.
const PANEL_WOOD: Color = Color::new(0.92, 0.65, 0.33, UPRIGHT);

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

/// Height of the underside of the table's top under a block of `shape`.
pub fn underside(shape: BlockShape) -> f32 {
    top_height(shape) - TOP_THICKNESS * shape.bore
}

/// Height of the table's top surface under a block of `shape`.
pub fn top_height(shape: BlockShape) -> f32 {
    shape.base - BELOW_BLOCK
}

/// Height of the ground the desk stands on, under a block of `shape`.
pub fn ground(shape: BlockShape) -> f32 {
    shape.base - TABLE_HEIGHT * shape.bore
}

/// Where the desk's body stands at its four bottom corners, seen from
/// above (x, z), half an end panel in from each, under a block of
/// `shape`.
#[cfg(test)]
pub fn feet(shape: BlockShape) -> [Vec2; 4] {
    let inset = END_PANEL / 2.0;
    let x = (TOP_HALF_LENGTH - OVERHANG - inset) * shape.bore;
    let front = (TOP_HALF_DEPTH - OVERHANG - inset) * shape.bore;
    let back = -(TOP_HALF_DEPTH - inset) * shape.bore;
    [
        Vec2::new(-x, back),
        Vec2::new(x, back),
        Vec2::new(x, front),
        Vec2::new(-x, front),
    ]
}

/// A plain board of the desk, square to the axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Board {
    /// Its least corner.
    pub low: Vec3,
    /// Its most corner.
    pub high: Vec3,
}

/// The end panel at the desk's left, under a block of `shape`: from the
/// ground up to the top's underside, from the back to the front.
pub fn end_panel(shape: BlockShape) -> Board {
    let unit = shape.bore;
    let outside = -(TOP_HALF_LENGTH - OVERHANG) * unit;
    Board {
        low: vec3(outside, ground(shape), -TOP_HALF_DEPTH * unit),
        high: vec3(
            outside + END_PANEL * unit,
            underside(shape),
            (TOP_HALF_DEPTH - OVERHANG) * unit,
        ),
    }
}

/// The knee space's two sides, across: the end panel's inside and the
/// pedestal's, under a block of `shape`.
fn knee_space(shape: BlockShape) -> (f32, f32) {
    (end_panel(shape).high.x, cabinet_place(shape).left)
}

/// The rail across the back of the knee space, under a block of `shape`.
pub fn back_rail(shape: BlockShape) -> Board {
    let unit = shape.bore;
    let (left, right) = knee_space(shape);
    let middle_z = (RAIL_INSET - TOP_HALF_DEPTH) * unit;
    let half = RAIL_THICKNESS * unit / 2.0;
    let low = ground(shape) + RAIL_LOW * unit;
    Board {
        low: vec3(left, low, middle_z - half),
        high: vec3(right, low + RAIL_HEIGHT * unit, middle_z + half),
    }
}

/// The pencil drawer across the top of the knee space, under a block of
/// `shape`: its front flush with the pedestal's drawers, shut for good.
pub fn pencil_drawer(shape: BlockShape) -> DrawerPlace {
    let unit = shape.bore;
    let (left, right) = knee_space(shape);
    let gap = PENCIL_GAP * unit;
    let front = cabinet_place(shape).front;
    let top = underside(shape) - gap / 2.0;
    DrawerPlace {
        left: left + gap,
        right: right - gap,
        front,
        back: front - PENCIL_DEPTH * unit,
        bottom: top - PENCIL_HEIGHT * unit + gap,
        top,
        most: 0.0,
    }
}

/// The table under the engine.
pub struct Table {
    /// The top, the end panel, the pedestal, the pencil drawer and the
    /// rail, on the graphics card.
    mesh: GpuMesh,
    /// Made on the first draw (it needs a window); `Some(None)` where it
    /// cannot be built, and the table is drawn in plain colors.
    material: Option<Option<GpuMaterial>>,
    /// The highlight over the top: its middle's four corners, then its
    /// rim's.
    highlight: GpuMesh,
    /// The highlight's see-through material, made on its first draw;
    /// `Some(None)` where it cannot be built, and the top is not lit.
    highlight_material: Option<Option<GpuMaterial>>,
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
            mesh: GpuMesh::new(Mesh {
                vertices: solid.vertices,
                indices: solid.indices,
                texture: None,
            }),
            material: None,
            highlight: GpuMesh::new(highlight_mesh(shape)),
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
        for (index, vertex) in self.highlight.edit().vertices.iter_mut().enumerate() {
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
            let material = load_lit(WOOD_FRAGMENT, &[("Shift", UniformType::Float3)]);
            if material.is_none() {
                // Drawn in plain colors, alpha means transparency.
                for vertex in &mut self.mesh.edit().vertices {
                    vertex.color[3] = u8::MAX;
                }
            }
            material
        });
        if let Some(material) = material {
            material.set_uniform("Shift", Vec3::ZERO);
        }
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

/// Adds `board` to `solid` in `wood`, its grain the way the wood's says:
/// along the shaft, or up and down.
fn board_solid(solid: &mut Solid, board: Board, wood: Color) {
    let middle = (board.low + board.high) / 2.0;
    let half = (board.high - board.low) / 2.0;
    if wood.a == ALONG_SHAFT {
        solid.bar(
            vec3(board.low.x, middle.y, middle.z),
            vec3(board.high.x, middle.y, middle.z),
            Vec3::Y,
            (half.y, half.y),
            half.z,
            wood,
        );
    } else {
        solid.bar(
            vec3(middle.x, board.low.y, middle.z),
            vec3(middle.x, board.high.y, middle.z),
            Vec3::Z,
            (half.z, half.z),
            half.x,
            wood,
        );
    }
}

/// Adds the desk's top to `solid`, under a block of `shape`: a slab with
/// a rounded edge along its front and ends, its back square.
fn top_solid(solid: &mut Solid, shape: BlockShape) {
    let unit = shape.bore;
    let top = top_height(shape);
    let edge = TOP_THICKNESS * unit / 2.0;
    let middle_y = top - edge;
    let (length, depth) = (TOP_HALF_LENGTH * unit, TOP_HALF_DEPTH * unit);
    let (ends, front) = (length - edge, depth - edge);
    let half_depth = (front + depth) / 2.0;
    let middle_z = (front - depth) / 2.0;
    solid.bar(
        vec3(-ends, middle_y, middle_z),
        vec3(ends, middle_y, middle_z),
        Vec3::Z,
        (half_depth, half_depth),
        edge,
        TOP_WOOD,
    );
    solid.cylinder(
        vec3(-ends, middle_y, front),
        vec3(ends, middle_y, front),
        edge,
        EDGE_SEGMENTS,
        TOP_WOOD,
    );
    for side in [-1.0, 1.0] {
        solid.cylinder(
            vec3(side * ends, middle_y, -depth),
            vec3(side * ends, middle_y, front),
            edge,
            EDGE_SEGMENTS,
            TOP_WOOD,
        );
        solid.ball(
            vec3(side * ends, middle_y, front),
            edge,
            CORNER_RINGS,
            TOP_WOOD,
        );
    }
}

/// The desk under a block of `shape`: its top, end panel, pedestal, pencil
/// drawer and back rail.
fn build(shape: BlockShape) -> Solid {
    let mut solid = Solid::new();
    top_solid(&mut solid, shape);
    board_solid(&mut solid, end_panel(shape), PANEL_WOOD);
    board_solid(&mut solid, back_rail(shape), RAIL_WOOD);
    cabinet_solid(&mut solid, cabinet_place(shape), PANEL_WOOD);
    solid.absorb(drawer_solid(pencil_drawer(shape), shape.bore));
    solid
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/table.rs"]
mod tests;
