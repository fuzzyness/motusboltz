//! The engine block: one steel piece around both bores and the opening
//! between them, drawn solid, with a round-cornered window cut into the
//! half that faces the camera so the gas stays in view from any side.
//!
//! The mesh is built once as a closed solid: every edge is shared by
//! exactly two triangles. The window is cut by the shader, which drops the
//! block's surfaces inside it; through the cut, any surface seen from
//! inside the metal is painted as a flat cut face, which shows the walls'
//! thickness. Drawing only; the physics uses `ChamberGeometry`.

use std::f32::consts::{
    FRAC_PI_2,
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

use super::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY,
    mix_color, //
};
use crate::sim::geometry::ChamberGeometry;

/// Steel around each bore beyond its radius, and under the floors.
pub const WALL_THICKNESS: f32 = 0.3;
const BASE_THICKNESS: f32 = 0.3;
/// Columns around each bore away from the opening, before the opening's
/// own columns and the block's corners are added.
const AROUND_SEGMENTS: usize = 96;
/// Steps across half of the opening's rim; its whole rim has twice this.
const RIM_STEPS: usize = 24;
/// The window: how far its sides reach past each cylinder's axis, as a
/// fraction of the bore radius; the metal left above and below it; and the
/// radius of its corners.
const WINDOW_REACH: f32 = 0.82;
const WINDOW_FRAME: f32 = 0.35;
const WINDOW_CORNER: f32 = 1.4;
/// Key light, from the upper front right in world space.
/// The key light, relative to the camera: mostly from above, a little
/// from the camera's side and from its left, so the outside the viewer
/// sees is always lit from any angle.
const LIGHT_UP: f32 = 0.8;
const LIGHT_TOWARD_CAMERA: f32 = 0.55;
const LIGHT_LEFT: f32 = 0.35;
/// Brightness of the surfaces inside the block (bore walls and floors,
/// the opening) relative to the outside, which takes the light first.
/// Stored in the vertex alpha, which the shader reads as shade.
const INTERIOR_SHADE: u8 = 150;
/// Triangles smaller than this (in square model units) are left out.
const MIN_TRIANGLE_AREA: f32 = 1e-9;

// Saturated, painted-looking colors in the style of a PS2 demo: copper on
// the hot side, cobalt on the cold side, a neutral steel where they meet,
// and teal for the insulated opening (the color of its old outline).
const STEEL: Color = Color::from_rgba(150, 160, 178, 255);
const OPENING_STEEL: Color = Color::from_rgba(52, 196, 168, 255);
const HOT_STEEL: Color = Color::from_rgba(232, 122, 58, 255);
const COLD_STEEL: Color = Color::from_rgba(58, 128, 236, 255);

const BLOCK_VERTEX: &str = r#"#version 100
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

const BLOCK_FRAGMENT: &str = r#"#version 100
precision mediump float;

varying lowp vec4 color;
varying lowp vec2 uv;
varying vec3 world;
varying vec3 surface_normal;

uniform sampler2D Texture;
// Camera position, and the horizontal unit vector from the block toward it.
uniform vec3 Eye;
uniform vec3 CutNormal;
// Window: half-width, middle height, half-height, corner radius.
uniform vec4 Window;
// Distance from the middle of the block to each bore's axis.
uniform float Offset;
uniform vec3 Light;

void main() {
    // Inside the window, and on the camera's side of a vertical plane
    // through the nearer bore's axis: cut away. Each cylinder is opened
    // through its own axis, the way engine cutaways are drawn.
    float across = dot(world, vec3(-CutNormal.z, 0.0, CutNormal.x));
    vec2 corner = abs(vec2(across, world.y - Window.y)) - (Window.xz - Window.w);
    float edge = length(max(corner, 0.0)) + min(max(corner.x, corner.y), 0.0) - Window.w;
    vec3 axis = vec3(world.x < 0.0 ? -Offset : Offset, 0.0, 0.0);
    if (edge < 0.0 && dot(world - axis, CutNormal) > 0.0) {
        discard;
    }
    vec3 n = normalize(surface_normal);
    vec3 toward_eye = normalize(Eye - world);
    float side = smoothstep(-0.6, 0.6, world.x);
    if (dot(n, toward_eye) < 0.0) {
        // Seen from inside the metal: the face of the cut.
        vec3 cut = mix(vec3(0.62, 0.30, 0.14), vec3(0.14, 0.30, 0.62), side);
        gl_FragColor = vec4(cut, 1.0);
        return;
    }
    float diffuse = max(dot(n, Light), 0.0);
    float facing = max(dot(n, toward_eye), 0.0);
    float shine = pow(max(dot(n, normalize(Light + toward_eye)), 0.0), 24.0);
    float rim = pow(1.0 - facing, 3.0);
    // Faint turned rings around the outside, so the paint is not flat.
    float rings = 0.94 + 0.06 * smoothstep(0.2, 0.8, abs(fract(world.y * 2.2) - 0.5) * 2.0);
    // Vertex alpha is shade: 1 outside, less on the interior surfaces.
    float shade = color.a;
    vec3 lit = shade * (color.rgb * rings * (0.32 + 0.6 * diffuse + 0.15 * facing)
        + vec3(0.5) * shine + vec3(0.55, 0.7, 1.0) * rim * 0.22);
    gl_FragColor = vec4(lit, 1.0) * texture2D(Texture, uv);
}"#;

/// The block's dimensions, from the chamber's.
#[derive(Clone, Copy, Debug)]
pub struct BlockShape {
    /// Distance from the middle of the shared wall to each bore's axis.
    pub offset: f32,
    pub bore: f32,
    /// Radius of the block's rounded ends around each axis.
    pub outer: f32,
    pub floor: f32,
    pub top: f32,
    /// Height of the block's underside.
    pub base: f32,
    pub opening: f32,
    pub opening_y: f32,
}

impl BlockShape {
    pub fn new(geometry: ChamberGeometry) -> Self {
        Self {
            offset: geometry.cylinder_offset,
            bore: geometry.cylinder_radius,
            outer: geometry.cylinder_radius + WALL_THICKNESS,
            floor: geometry.floor(),
            top: geometry.cylinder_height / 2.0,
            base: geometry.floor() - BASE_THICKNESS,
            opening: geometry.passage_radius,
            opening_y: geometry.passage_center_y(),
        }
    }

    /// Where the opening's tube meets a bore wall, at angle `psi` around
    /// the opening's axis (0 at the top), on the hot side (`side` -1) or
    /// the cold side (1).
    fn rim_point(self, side: f32, psi: f32) -> Vec3 {
        let (sin, cos) = psi.sin_cos();
        let z = self.opening * sin;
        let wall = (self.bore * self.bore - z * z).sqrt();
        vec3(
            side * (self.offset - wall),
            self.opening_y + self.opening * cos,
            z,
        )
    }

    /// The window: half-width, middle height, half-height, corner radius.
    pub fn window(self) -> [f32; 4] {
        [
            self.offset + WINDOW_REACH * self.bore,
            (self.floor + self.top) / 2.0,
            (self.top - self.floor) / 2.0 - WINDOW_FRAME,
            WINDOW_CORNER,
        ]
    }
}

/// One column of the hot bore's wall, at one angle around its axis. Columns
/// facing the opening are split around it.
#[derive(Clone, Copy, Debug)]
struct Column {
    /// Angle around the axis, from +x toward +z.
    angle: f32,
    /// The wall below the opening (or below the middle, away from it) runs
    /// from the floor up to `lower`; the wall above it from `upper` to the
    /// top. Away from the opening, both points are the same.
    lower: Vec3,
    upper: Vec3,
}

/// Where the hot half's top face ends along a ray from the hot axis: on the
/// block's rounded end, on its front or back face, or at the plane between
/// the halves (x = 0).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Rim {
    Round,
    Face,
    Middle,
}

/// Builds the meshes once and draws them with the cutting shader.
pub struct EngineBlock {
    shape: BlockShape,
    mesh: Mesh,
    material: Material,
}

impl EngineBlock {
    /// Builds the block for `geometry`. Returns `None` if the shader cannot
    /// be built, so the caller can draw the plain frames instead.
    pub fn new(geometry: ChamberGeometry) -> Option<Self> {
        let shape = BlockShape::new(geometry);
        let material = load_material(
            ShaderSource::Glsl {
                vertex: BLOCK_VERTEX,
                fragment: BLOCK_FRAGMENT,
            },
            MaterialParams {
                pipeline_params: PipelineParams {
                    depth_write: true,
                    depth_test: Comparison::LessOrEqual,
                    ..Default::default()
                },
                uniforms: vec![
                    UniformDesc::new("Eye", UniformType::Float3),
                    UniformDesc::new("CutNormal", UniformType::Float3),
                    UniformDesc::new("Window", UniformType::Float4),
                    UniformDesc::new("Offset", UniformType::Float1),
                    UniformDesc::new("Light", UniformType::Float3),
                ],
                ..Default::default()
            },
        )
        .ok()?;
        Some(Self {
            shape,
            mesh: block_mesh(shape),
            material,
        })
    }

    /// Draws the block with its window cut toward `camera`.
    pub fn draw(&self, camera: &Camera3D) {
        let [half_width, middle, half_height, corner] = self.shape.window();
        self.material.set_uniform("Eye", camera.position);
        self.material
            .set_uniform("CutNormal", cut_normal(camera.position));
        self.material
            .set_uniform("Window", Vec4::new(half_width, middle, half_height, corner));
        self.material.set_uniform("Offset", self.shape.offset);
        self.material
            .set_uniform("Light", key_light(camera.position));
        gl_use_material(&self.material);
        draw_mesh(&self.mesh);
        gl_use_default_material();
    }
}

/// The direction toward the key light for a camera at `eye`.
pub(super) fn key_light(eye: Vec3) -> Vec3 {
    let toward = cut_normal(eye);
    let left = vec3(toward.z, 0.0, -toward.x);
    (Vec3::Y * LIGHT_UP + toward * LIGHT_TOWARD_CAMERA + left * LIGHT_LEFT).normalize()
}

/// `color` marked as an interior surface, for the shader's shade.
fn interior(color: Color) -> Color {
    Color {
        a: INTERIOR_SHADE as f32 / 255.0,
        ..color
    }
}

/// The horizontal direction from the block's middle toward the camera; the
/// window is cut on that side. Straight above or below, it faces the front.
fn cut_normal(eye: Vec3) -> Vec3 {
    let flat = vec3(eye.x, 0.0, eye.z);
    if flat.length_squared() > f32::EPSILON {
        flat.normalize()
    } else {
        Vec3::Z
    }
}

/// The hot bore's columns, in order of angle. The cold bore's are their
/// mirror image (x negated), so the two halves meet exactly at x = 0.
fn hot_columns(shape: BlockShape) -> Vec<Column> {
    let axis = vec3(-shape.offset, 0.0, 0.0);
    let psi = rim_angles();
    let mut columns = Vec::new();
    // Columns facing the opening, split around its rim. Column k's upper
    // point is the rim at psi[k] (upper half of the rim, from z = -r to
    // z = r); its lower point is the rim at the same z on the lower half.
    for (step, &upper_psi) in psi.iter().enumerate().take(RIM_STEPS + 1) {
        let lower_psi = psi[(2 * RIM_STEPS - step) % (2 * RIM_STEPS)];
        let upper = shape.rim_point(-1.0, upper_psi);
        let lower = shape.rim_point(-1.0, lower_psi);
        columns.push(Column {
            angle: (upper.z).atan2(upper.x - axis.x),
            lower,
            upper,
        });
    }
    // The widest angle the opening reaches; plain columns stay outside it.
    let reach = (shape.opening / shape.bore).asin() + 1e-3;
    let corner = shape.outer.atan2(shape.offset);
    let plain = (0..AROUND_SEGMENTS)
        .map(|segment| -PI + TAU * segment as f32 / AROUND_SEGMENTS as f32)
        .chain([corner, -corner, FRAC_PI_2, -FRAC_PI_2])
        .filter(|angle| angle.abs() > reach);
    for angle in plain {
        let (sin, cos) = angle.sin_cos();
        let point = axis + vec3(cos, 0.0, sin) * shape.bore;
        let middle = vec3(point.x, shape.opening_y, point.z);
        columns.push(Column {
            angle,
            lower: middle,
            upper: middle,
        });
    }
    columns.sort_by(|a, b| a.angle.total_cmp(&b.angle));
    columns.dedup_by(|a, b| (a.angle - b.angle).abs() < 1e-4);
    columns
}

/// Angles around the opening's axis for its rim: from -90 degrees (the
/// -z side) over the top to +90 and on round under the bottom.
fn rim_angles() -> Vec<f32> {
    (0..2 * RIM_STEPS)
        .map(|step| -FRAC_PI_2 + PI * step as f32 / RIM_STEPS as f32)
        .collect()
}

/// Where the hot half's top face ends along the ray from the hot axis
/// through `through`, and which edge it reaches.
fn hot_top_edge(shape: BlockShape, through: Vec3) -> (Vec3, Rim) {
    let axis = vec3(-shape.offset, 0.0, 0.0);
    let flat = vec3(through.x - axis.x, 0.0, through.z);
    let direction = flat.normalize();
    let (point, rim) = if direction.x <= 0.0 {
        (axis + direction * shape.outer, Rim::Round)
    } else {
        let to_face = if direction.z.abs() > f32::EPSILON {
            shape.outer / direction.z.abs()
        } else {
            f32::INFINITY
        };
        let to_middle = shape.offset / direction.x;
        if to_face < to_middle {
            (axis + direction * to_face, Rim::Face)
        } else {
            let mut point = axis + direction * to_middle;
            point.x = 0.0;
            (point, Rim::Middle)
        }
    };
    (vec3(point.x, shape.top, point.z), rim)
}

/// Mirror image across the plane between the halves.
fn mirror(point: Vec3) -> Vec3 {
    vec3(-point.x, point.y, point.z)
}

/// Collects triangles with per-vertex normals and colors, leaving out
/// slivers.
struct Builder {
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
}

impl Builder {
    fn vertex(&mut self, position: Vec3, normal: Vec3, color: Color) -> u16 {
        let mut vertex = Vertex::new2(position, Vec2::ZERO, color);
        vertex.normal = Vec4::new(normal.x, normal.y, normal.z, 0.0);
        self.vertices.push(vertex);
        (self.vertices.len() - 1) as u16
    }

    fn triangle(&mut self, a: u16, b: u16, c: u16) {
        let [pa, pb, pc] = [a, b, c].map(|index| self.vertices[index as usize].position);
        if (pb - pa).cross(pc - pa).length() / 2.0 > MIN_TRIANGLE_AREA {
            self.indices.extend([a, b, c]);
        }
    }

    /// A quad with corners in order around it.
    fn quad(&mut self, corners: [(Vec3, Vec3, Color); 4]) {
        let [a, b, c, d] =
            corners.map(|(position, normal, color)| self.vertex(position, normal, color));
        self.triangle(a, b, c);
        self.triangle(a, c, d);
    }
}

/// Steel tinted by side: warm on the hot side, cool on the cold side,
/// blending across the shared wall.
fn side_steel(x: f32, shape: BlockShape) -> Color {
    let wall = shape.offset - shape.bore;
    let t = ((x + wall) / (2.0 * wall)).clamp(0.0, 1.0);
    mix_color(HOT_STEEL, COLD_STEEL, t * t * (3.0 - 2.0 * t))
}

/// The whole block as one closed mesh.
fn block_mesh(shape: BlockShape) -> Mesh {
    let hot = hot_columns(shape);
    let mut builder = Builder {
        vertices: Vec::new(),
        indices: Vec::new(),
    };
    for side in [-1.0_f32, 1.0] {
        let place = |point: Vec3| if side < 0.0 { point } else { mirror(point) };
        let axis = vec3(side * shape.offset, 0.0, 0.0);
        let bore_color = interior(if side < 0.0 { HOT_STEEL } else { COLD_STEEL });
        let toward_axis = |point: Vec3| {
            let flat = vec3(axis.x - point.x, 0.0, axis.z - point.z);
            flat.normalize()
        };
        for (index, column) in hot.iter().enumerate() {
            let next = &hot[(index + 1) % hot.len()];
            let [lower, upper, next_lower, next_upper] =
                [column.lower, column.upper, next.lower, next.upper].map(place);
            let at = |point: Vec3, y: f32| vec3(point.x, y, point.z);
            // Bore wall below and above the opening (or the middle).
            builder.quad([
                (at(lower, shape.floor), toward_axis(lower), bore_color),
                (
                    at(next_lower, shape.floor),
                    toward_axis(next_lower),
                    bore_color,
                ),
                (next_lower, toward_axis(next_lower), bore_color),
                (lower, toward_axis(lower), bore_color),
            ]);
            builder.quad([
                (upper, toward_axis(upper), bore_color),
                (next_upper, toward_axis(next_upper), bore_color),
                (
                    at(next_upper, shape.top),
                    toward_axis(next_upper),
                    bore_color,
                ),
                (at(upper, shape.top), toward_axis(upper), bore_color),
            ]);
            // Bore floor: a fan from the axis.
            let center = builder.vertex(at(axis, shape.floor), Vec3::Y, bore_color);
            let a = builder.vertex(at(lower, shape.floor), Vec3::Y, bore_color);
            let b = builder.vertex(at(next_lower, shape.floor), Vec3::Y, bore_color);
            builder.triangle(center, a, b);
            // Top face, from the bore's rim out to the block's edge.
            let (edge, _) = hot_top_edge(shape, column.upper);
            let (next_edge, _) = hot_top_edge(shape, next.upper);
            let (edge, next_edge) = (place(edge), place(next_edge));
            builder.quad([
                (at(upper, shape.top), Vec3::Y, side_steel(upper.x, shape)),
                (
                    at(next_upper, shape.top),
                    Vec3::Y,
                    side_steel(next_upper.x, shape),
                ),
                (next_edge, Vec3::Y, side_steel(next_edge.x, shape)),
                (edge, Vec3::Y, side_steel(edge.x, shape)),
            ]);
        }
    }

    // The block's outer wall and underside, around the edge points that
    // lie on its rounded ends and flat faces.
    let outline = outline(shape, &hot);
    let center = builder.vertex(vec3(0.0, shape.base, 0.0), -Vec3::Y, STEEL);
    for (index, &(point, normal)) in outline.iter().enumerate() {
        let (next, next_normal) = outline[(index + 1) % outline.len()];
        let low = |p: Vec3| vec3(p.x, shape.base, p.z);
        builder.quad([
            (low(point), normal, side_steel(point.x, shape)),
            (low(next), next_normal, side_steel(next.x, shape)),
            (next, next_normal, side_steel(next.x, shape)),
            (point, normal, side_steel(point.x, shape)),
        ]);
        let a = builder.vertex(low(point), -Vec3::Y, side_steel(point.x, shape));
        let b = builder.vertex(low(next), -Vec3::Y, side_steel(next.x, shape));
        builder.triangle(center, b, a);
    }

    // The opening's tube, from one bore wall to the other.
    let psi = rim_angles();
    for (step, &angle) in psi.iter().enumerate() {
        let next = psi[(step + 1) % psi.len()];
        let inward = |angle: f32| {
            let (sin, cos) = angle.sin_cos();
            vec3(0.0, -cos, -sin)
        };
        builder.quad([
            (
                shape.rim_point(-1.0, angle),
                inward(angle),
                interior(OPENING_STEEL),
            ),
            (
                shape.rim_point(1.0, angle),
                inward(angle),
                interior(OPENING_STEEL),
            ),
            (
                shape.rim_point(1.0, next),
                inward(next),
                interior(OPENING_STEEL),
            ),
            (
                shape.rim_point(-1.0, next),
                inward(next),
                interior(OPENING_STEEL),
            ),
        ]);
    }

    assert!(builder.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
    assert!(builder.indices.len() < DRAW_CALL_INDEX_CAPACITY);
    Mesh {
        vertices: builder.vertices,
        indices: builder.indices,
        texture: None,
    }
}

/// The block's outline at the top, in order around it: the hot half's
/// edge points on its rounded end and faces (from the front corner round
/// the back), then the cold half's mirror image. Each comes with its
/// outward normal.
fn outline(shape: BlockShape, hot: &[Column]) -> Vec<(Vec3, Vec3)> {
    let axis = vec3(-shape.offset, 0.0, 0.0);
    let edges: Vec<(f32, Vec3, Rim)> = hot
        .iter()
        .map(|column| {
            let (point, rim) = hot_top_edge(shape, column.upper);
            (column.angle, point, rim)
        })
        .collect();
    // Hot half: from the front corner (angle `corner`) up to pi, then from
    // -pi to the back corner. The corner columns end exactly where a face
    // meets the middle plane, so they count as face points.
    let corner = shape.outer.atan2(shape.offset) - 1e-4;
    let hot_half: Vec<(Vec3, Vec3)> = edges
        .iter()
        .filter(|(angle, ..)| *angle >= corner)
        .chain(edges.iter().filter(|(angle, ..)| *angle <= -corner))
        .map(|&(_, point, rim)| {
            let normal = match rim {
                Rim::Round => vec3(point.x - axis.x, 0.0, point.z).normalize(),
                Rim::Face | Rim::Middle => vec3(0.0, 0.0, point.z.signum()),
            };
            (point, normal)
        })
        .collect();
    // Cold half: the mirror image, walked from the back corner round to the
    // front one.
    let cold_half: Vec<(Vec3, Vec3)> = hot_half
        .iter()
        .rev()
        .map(|&(point, normal)| (mirror(point), mirror(normal)))
        .collect();
    let mut loop_points: Vec<(Vec3, Vec3)> = Vec::new();
    for item in hot_half.into_iter().chain(cold_half) {
        if loop_points
            .last()
            .is_none_or(|(last, _)| last.distance_squared(item.0) > 1e-10)
        {
            loop_points.push(item);
        }
    }
    if loop_points.len() > 1
        && loop_points[0]
            .0
            .distance_squared(loop_points[loop_points.len() - 1].0)
            <= 1e-10
    {
        loop_points.pop();
    }
    loop_points
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/block.rs"]
mod tests;
