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
    Vertex, //
};
use macroquad::prelude::{
    Camera3D,
    Color,
    Material,
    UniformType,
    Vec2,
    Vec3,
    Vec4,
    vec2,
    vec3, //
};

use super::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY,
    draw_lit,
    load_lit,
    mix_color, //
};
use crate::sim::geometry::ChamberGeometry;

/// Steel around each bore beyond its radius.
pub const WALL_THICKNESS: f32 = 0.3;
/// Steel under the floors.
pub const BASE_THICKNESS: f32 = 0.3;
/// Columns around each bore away from the opening, before the opening's
/// own columns and the block's corners are added.
const AROUND_SEGMENTS: usize = 96;
/// Steps across half of the opening's rim; its whole rim has twice this.
const RIM_STEPS: usize = 24;
/// How far the window's sides reach past each cylinder's axis, as a
/// fraction of the bore radius.
const WINDOW_REACH: f32 = 0.82;
/// Metal left above and below the window.
const WINDOW_FRAME: f32 = 0.35;
/// Radius of the window's corners.
const WINDOW_CORNER: f32 = 1.4;
/// How much the key light comes from above.
///
/// The key light is set relative to the camera: mostly from above, a little
/// from the camera's side and from its left, so the outside the viewer
/// sees is always lit from any angle.
const LIGHT_UP: f32 = 0.8;
/// How much the key light comes from the camera's side.
const LIGHT_TOWARD_CAMERA: f32 = 0.55;
/// How much the key light comes from the camera's left.
const LIGHT_LEFT: f32 = 0.35;
/// Brightness of the surfaces inside the block (bore walls and floors,
/// the opening) relative to the outside, which takes the light first.
/// Stored in the vertex alpha, which the shader reads as shade.
const INTERIOR_SHADE: u8 = 150;
/// Triangles smaller than this (in square model units) are left out.
const MIN_TRIANGLE_AREA: f32 = 1e-9;
/// Points nearer than the square root of this are the same point.
const SAME_POINT_SQUARED: f32 = 1e-10;
/// Columns nearer in angle than this are the same column.
const SAME_ANGLE: f32 = 1e-4;

/// Neutral steel, under the block.
///
/// The colors are saturated and painted-looking, in the style of a PS2
/// demo.
const STEEL: Color = Color::from_rgba(150, 160, 178, 255);
/// Teal, for the insulated opening (the color of its old outline).
const OPENING_STEEL: Color = Color::from_rgba(52, 196, 168, 255);
/// Copper, on the hot side.
pub(super) const HOT_STEEL: Color = Color::from_rgba(232, 122, 58, 255);
/// Cobalt, on the cold side.
pub(super) const COLD_STEEL: Color = Color::from_rgba(58, 128, 236, 255);

/// Fragment shader that cuts the window and lights the steel.
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
    /// Radius of each bore.
    pub bore: f32,
    /// Radius of the block's rounded ends around each axis.
    pub outer: f32,
    /// Height of the bores' floors.
    pub floor: f32,
    /// Height of the block's top.
    pub top: f32,
    /// Height of the block's underside.
    pub base: f32,
    /// Radius of the opening between the bores.
    pub opening: f32,
    /// Height of the opening's axis.
    pub opening_y: f32,
}

impl BlockShape {
    /// The block around the chamber of `geometry`.
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
    /// Top of the wall below the opening, or below the middle away from
    /// it, which runs up from the floor.
    lower: Vec3,
    /// Bottom of the wall above the opening, which runs up to the top.
    ///
    /// Away from the opening, it is the same point as `lower`.
    upper: Vec3,
}

/// Where the hot half's top face ends along a ray from the hot axis: on the
/// block's rounded end, on its front or back face, or at the plane between
/// the halves (x = 0).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Rim {
    /// The rounded end.
    Round,
    /// The front or back face.
    Face,
    /// The plane between the halves.
    Middle,
}

/// Builds the meshes once and draws them with the cutting shader.
pub struct EngineBlock {
    /// The block's dimensions.
    shape: BlockShape,
    /// The whole block.
    mesh: Mesh,
    /// The cutting shader.
    material: Material,
}

impl EngineBlock {
    /// Builds the block for `geometry`.
    ///
    /// Returns `None` if the shader cannot be built, so the caller can
    /// draw the plain frames instead.
    pub fn new(geometry: ChamberGeometry) -> Option<Self> {
        let shape = BlockShape::new(geometry);
        let window = [
            ("CutNormal", UniformType::Float3),
            ("Window", UniformType::Float4),
            ("Offset", UniformType::Float1),
        ];
        let material = load_lit(BLOCK_FRAGMENT, &window)?;
        Some(Self {
            shape,
            mesh: block_mesh(shape),
            material,
        })
    }

    /// Draws the block with its window cut toward `camera`.
    pub fn draw(&self, camera: &Camera3D) {
        let [half_width, middle, half_height, corner] = self.shape.window();
        let window = Vec4::new(half_width, middle, half_height, corner);
        self.material
            .set_uniform("CutNormal", cut_normal(camera.position));
        self.material.set_uniform("Window", window);
        self.material.set_uniform("Offset", self.shape.offset);
        let light = key_light(camera.position);
        draw_lit([&self.mesh], Some(&self.material), camera, light);
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

/// Whether `point`, seen from a camera at `eye`, lies in line with the
/// block's window, in front of it or behind it, where the window is cut
/// toward that camera: inside the window's outline, widened by `margin`
/// each side, measured across the view and up.
pub(super) fn in_window(shape: BlockShape, eye: Vec3, point: Vec3, margin: f32) -> bool {
    let normal = cut_normal(eye);
    let [half_width, middle, half_height, corner] = shape.window();
    let half_width = half_width + margin;
    let across = point.dot(vec3(-normal.z, 0.0, normal.x));
    let inset = vec2(across.abs(), (point.y - middle).abs())
        - vec2(half_width - corner, half_height - corner);
    inset.max(Vec2::ZERO).length() + inset.x.max(inset.y).min(0.0) - corner < 0.0
}

/// The horizontal direction from the block's middle toward the camera; the
/// window is cut on that side. Straight above or below, it faces the front.
pub(super) fn cut_normal(eye: Vec3) -> Vec3 {
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
    columns.dedup_by(|a, b| (a.angle - b.angle).abs() < SAME_ANGLE);
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
    /// Every corner, with its normal and color.
    vertices: Vec<Vertex>,
    /// Triangles, three corners each.
    indices: Vec<u16>,
}

impl Builder {
    /// Adds a corner and returns its index.
    fn vertex(&mut self, position: Vec3, normal: Vec3, color: Color) -> u16 {
        let mut vertex = Vertex::new2(position, Vec2::ZERO, color);
        vertex.normal = Vec4::new(normal.x, normal.y, normal.z, 0.0);
        self.vertices.push(vertex);
        (self.vertices.len() - 1) as u16
    }

    /// Adds a triangle unless it is a sliver.
    fn triangle(&mut self, a: u16, b: u16, c: u16) {
        let [pa, pb, pc] = [a, b, c].map(|index| self.vertices[index as usize].position);
        if (pb - pa).cross(pc - pa).length() / 2.0 > MIN_TRIANGLE_AREA {
            self.indices.extend([a, b, c]);
        }
    }

    /// Adds a quad with corners in order around it.
    fn quad(&mut self, corners: [Vec3; 4], normals: [Vec3; 4], colors: [Color; 4]) {
        let [a, b, c, d] = [0, 1, 2, 3].map(|i| self.vertex(corners[i], normals[i], colors[i]));
        self.triangle(a, b, c);
        self.triangle(a, c, d);
    }
}

/// Steel tinted by the side of `point`: warm on the hot side, cool on the
/// cold side, blending across the shared wall.
fn side_steel(point: Vec3, shape: BlockShape) -> Color {
    let wall = shape.offset - shape.bore;
    let t = ((point.x + wall) / (2.0 * wall)).clamp(0.0, 1.0);
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
            let on_floor = |point: Vec3| vec3(point.x, shape.floor, point.z);
            let on_top = |point: Vec3| vec3(point.x, shape.top, point.z);
            // Bore wall below and above the opening (or the middle).
            let corners = [on_floor(lower), on_floor(next_lower), next_lower, lower];
            let normals = [lower, next_lower, next_lower, lower].map(toward_axis);
            builder.quad(corners, normals, [bore_color; 4]);
            let corners = [upper, next_upper, on_top(next_upper), on_top(upper)];
            let normals = [upper, next_upper, next_upper, upper].map(toward_axis);
            builder.quad(corners, normals, [bore_color; 4]);
            // Bore floor: a fan from the axis.
            let [center, a, b] =
                [axis, lower, next_lower].map(|p| builder.vertex(on_floor(p), Vec3::Y, bore_color));
            builder.triangle(center, a, b);
            // Top face, from the bore's rim out to the block's edge.
            let (edge, _) = hot_top_edge(shape, column.upper);
            let (next_edge, _) = hot_top_edge(shape, next.upper);
            let corners = [
                on_top(upper),
                on_top(next_upper),
                place(next_edge),
                place(edge),
            ];
            let colors = corners.map(|corner| side_steel(corner, shape));
            builder.quad(corners, [Vec3::Y; 4], colors);
        }
    }

    // The block's outer wall and underside, around the edge points that
    // lie on its rounded ends and flat faces.
    let outline = outline(shape, &hot);
    let center = builder.vertex(vec3(0.0, shape.base, 0.0), -Vec3::Y, STEEL);
    for (index, &(point, normal)) in outline.iter().enumerate() {
        let (next, next_normal) = outline[(index + 1) % outline.len()];
        let low = |p: Vec3| vec3(p.x, shape.base, p.z);
        let corners = [low(point), low(next), next, point];
        let colors = corners.map(|corner| side_steel(corner, shape));
        builder.quad(corners, [normal, next_normal, next_normal, normal], colors);
        let a = builder.vertex(low(point), -Vec3::Y, colors[0]);
        let b = builder.vertex(low(next), -Vec3::Y, colors[1]);
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
        let ends = [(-1.0, angle), (1.0, angle), (1.0, next), (-1.0, next)];
        let corners = ends.map(|(side, psi)| shape.rim_point(side, psi));
        let normals = ends.map(|(_, psi)| inward(psi));
        builder.quad(corners, normals, [interior(OPENING_STEEL); 4]);
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
    let same = |a: Vec3, b: Vec3| a.distance_squared(b) <= SAME_POINT_SQUARED;
    let mut loop_points: Vec<(Vec3, Vec3)> = Vec::new();
    for item in hot_half.into_iter().chain(cold_half) {
        if loop_points
            .last()
            .is_none_or(|&(last, _)| !same(last, item.0))
        {
            loop_points.push(item);
        }
    }
    if let [first, .., last] = loop_points.as_slice()
        && same(first.0, last.0)
    {
        loop_points.pop();
    }
    loop_points
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/block.rs"]
mod tests;
