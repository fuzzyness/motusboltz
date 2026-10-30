//! Solid machine parts: building them from simple shapes (turned parts,
//! cylinders, tapered bars and flat plates) and drawing them as lit metal.
//!
//! Each vertex carries its surface normal and, in its color's alpha, how
//! polished the metal is there: 0 is a dull, painted or rough surface, 1 a
//! bright, polished one. The shader lights the parts from the same key
//! light as the engine block, adds a highlight that tightens with polish,
//! and reflects a simple room (dark floor, lighter above) in the metal.

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
    load_material, //
};

const METAL_VERTEX: &str = r#"#version 100
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

const METAL_FRAGMENT: &str = r#"#version 100
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
    // Light whichever side faces the camera, so no part shows a black back.
    if (dot(n, toward_eye) < 0.0) {
        n = -n;
    }
    float polish = color.a;
    float diffuse = max(dot(n, Light), 0.0);
    float shine = pow(max(dot(n, normalize(Light + toward_eye)), 0.0), mix(10.0, 80.0, polish))
        * mix(0.12, 0.8, polish);
    // The room seen in the metal: dark below, lighter above.
    vec3 seen = reflect(-toward_eye, n);
    vec3 room = mix(vec3(0.08, 0.10, 0.14), vec3(0.62, 0.67, 0.75), smoothstep(-0.3, 0.7, seen.y));
    float edge = pow(1.0 - max(dot(n, toward_eye), 0.0), 3.0);
    vec3 base = color.rgb;
    vec3 lit = base * (0.24 + 0.6 * diffuse)
        + base * room * (0.45 * polish)
        + room * edge * (0.3 * polish)
        + vec3(shine);
    gl_FragColor = vec4(lit, 1.0) * texture2D(Texture, uv);
}"#;

/// The lit-metal material, or `None` if its shader cannot be built.
pub fn load_metal() -> Option<Material> {
    load_material(
        ShaderSource::Glsl {
            vertex: METAL_VERTEX,
            fragment: METAL_FRAGMENT,
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

/// Draws `mesh` as lit metal seen from `camera`, lit from `light`; or, with
/// no material, in its plain colors.
pub fn draw_metal(mesh: &Mesh, material: Option<&Material>, camera: &Camera3D, light: Vec3) {
    let Some(material) = material else {
        draw_mesh(mesh);
        return;
    };
    material.set_uniform("Eye", camera.position);
    material.set_uniform("Light", light);
    gl_use_material(material);
    draw_mesh(mesh);
    gl_use_default_material();
}

/// A color and how polished the surface is (see the top of this file).
pub const fn finish(red: u8, green: u8, blue: u8, polish: f32) -> Color {
    Color::new(
        red as f32 / 255.0,
        green as f32 / 255.0,
        blue as f32 / 255.0,
        polish,
    )
}

/// Directions for building a part: `axis` and two directions square to
/// it and to each other.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub origin: Vec3,
    pub axis: Vec3,
    pub u: Vec3,
    pub v: Vec3,
}

impl Frame {
    /// A frame at `origin` along `axis` (which need not be a unit vector).
    pub fn along(origin: Vec3, axis: Vec3) -> Self {
        let axis = axis.normalize();
        let helper = if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
        let u = axis.cross(helper).normalize();
        Self {
            origin,
            axis,
            u,
            v: axis.cross(u),
        }
    }

    /// The point `along` the axis, `radius` out toward `angle` around it.
    fn point(self, along: f32, radius: f32, angle: f32) -> Vec3 {
        self.origin + self.axis * along + self.radial(angle) * radius
    }

    fn radial(self, angle: f32) -> Vec3 {
        let (sin, cos) = angle.sin_cos();
        self.u * cos + self.v * sin
    }
}

/// One point of a turned part's outline: its distance from the axis, its
/// place along the axis, and its color.
#[derive(Clone, Copy, Debug)]
pub struct Turn {
    pub radius: f32,
    pub along: f32,
    pub color: Color,
}

pub const fn turn(radius: f32, along: f32, color: Color) -> Turn {
    Turn {
        radius,
        along,
        color,
    }
}

/// Triangles with per-vertex normals and finishes, for a mesh.
pub struct Solid {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

impl Solid {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn vertex(&mut self, position: Vec3, normal: Vec3, color: Color) -> u16 {
        let mut vertex = Vertex::new2(position, Vec2::ZERO, color);
        vertex.normal = Vec4::new(normal.x, normal.y, normal.z, 0.0);
        self.vertices.push(vertex);
        (self.vertices.len() - 1) as u16
    }

    /// A four-sided face with corners in order around it.
    fn quad(&mut self, corners: [Vec3; 4], normals: [Vec3; 4], colors: [Color; 4]) {
        let [a, b, c, d] = [0, 1, 2, 3].map(|i| self.vertex(corners[i], normals[i], colors[i]));
        self.indices.extend([a, b, c, a, c, d]);
    }

    /// A part turned about `frame.axis`: each pair of outline points sweeps
    /// a band around the axis, shaded smoothly around it and evenly along
    /// it, so edges between bands stay crisp.
    pub fn lathe(&mut self, frame: Frame, outline: &[Turn], segments: usize) {
        for pair in outline.windows(2) {
            let (start, end) = (pair[0], pair[1]);
            let (radial, along) = (end.radius - start.radius, end.along - start.along);
            let length = (radial * radial + along * along).sqrt();
            if length <= f32::EPSILON {
                continue;
            }
            // Square to the outline, in the plane through the axis.
            let (out, ahead) = (along / length, -radial / length);
            // One ring of vertices at each end of the band, shared by its
            // faces around the axis.
            let first = self.vertices.len() as u16;
            for step in 0..=segments {
                let angle = std::f32::consts::TAU * step as f32 / segments as f32;
                let normal = frame.radial(angle) * out + frame.axis * ahead;
                self.vertex(
                    frame.point(start.along, start.radius, angle),
                    normal,
                    start.color,
                );
                self.vertex(frame.point(end.along, end.radius, angle), normal, end.color);
            }
            for step in 0..segments as u16 {
                let (low, high) = (first + 2 * step, first + 2 * step + 1);
                let (next_low, next_high) = (low + 2, high + 2);
                self.indices
                    .extend([low, next_low, next_high, low, next_high, high]);
            }
        }
    }

    /// A closed round bar from `start` to `end`.
    pub fn cylinder(&mut self, start: Vec3, end: Vec3, radius: f32, segments: usize, color: Color) {
        let length = start.distance(end);
        self.lathe(
            Frame::along(start, end - start),
            &[
                turn(0.0, 0.0, color),
                turn(radius, 0.0, color),
                turn(radius, length, color),
                turn(0.0, length, color),
            ],
            segments,
        );
    }

    /// A closed bar of rectangular section from `start` to `end`, `across`
    /// one way (half-widths at each end) and `thick` the other (half the
    /// thickness). `side` sets which way is across.
    pub fn bar(
        &mut self,
        start: Vec3,
        end: Vec3,
        side: Vec3,
        across: (f32, f32),
        thick: f32,
        color: Color,
    ) {
        let axis = (end - start).normalize();
        let wide = (side - axis * side.dot(axis)).normalize();
        let deep = axis.cross(wide);
        let corners = |center: Vec3, half: f32| {
            [
                center - wide * half - deep * thick,
                center + wide * half - deep * thick,
                center + wide * half + deep * thick,
                center - wide * half + deep * thick,
            ]
        };
        let low = corners(start, across.0);
        let high = corners(end, across.1);
        for edge in 0..4 {
            let next = (edge + 1) % 4;
            let face = [low[edge], low[next], high[next], high[edge]];
            let normal = (face[1] - face[0]).cross(face[3] - face[0]).normalize();
            self.quad(face, [normal; 4], [color; 4]);
        }
        self.quad(low, [-axis; 4], [color; 4]);
        self.quad(high, [axis; 4], [color; 4]);
    }

    /// A flat plate: the convex `outline` (in `frame.u`, `frame.v`) raised
    /// `thickness` along `frame.axis` from `frame.origin`.
    pub fn plate(&mut self, frame: Frame, outline: &[Vec2], thickness: f32, color: Color) {
        let place = |point: Vec2, along: f32| {
            frame.origin + frame.u * point.x + frame.v * point.y + frame.axis * along
        };
        let middle = outline.iter().copied().sum::<Vec2>() / outline.len() as f32;
        for (along, normal) in [(0.0, -frame.axis), (thickness, frame.axis)] {
            let center = self.vertex(place(middle, along), normal, color);
            let first = self.vertices.len() as u16;
            for &point in outline {
                self.vertex(place(point, along), normal, color);
            }
            let count = outline.len() as u16;
            for index in 0..count {
                self.indices
                    .extend([center, first + index, first + (index + 1) % count]);
            }
        }
        for (index, &point) in outline.iter().enumerate() {
            let next = outline[(index + 1) % outline.len()];
            let edge = next - point;
            let out = Vec2::new(edge.y, -edge.x).normalize();
            let normal = frame.u * out.x + frame.v * out.y;
            self.quad(
                [
                    place(point, 0.0),
                    place(next, 0.0),
                    place(next, thickness),
                    place(point, thickness),
                ],
                [normal; 4],
                [color; 4],
            );
        }
    }
}

/// The outline around two circles, as a flat crank web is shaped: the
/// first of `radii` at the origin and the second at `reach` along +x.
pub fn web_outline(radii: (f32, f32), reach: f32, steps: usize) -> Vec<Vec2> {
    // The straight sides touch both circles where the radius makes this
    // angle with the line between their centers.
    let tangent = ((radii.0 - radii.1) / reach).clamp(-1.0, 1.0).acos();
    let mut outline = Vec::new();
    // Around the far circle from one side to the other, then back around
    // the near circle.
    for step in 0..=steps {
        let angle = -tangent + 2.0 * tangent * step as f32 / steps as f32;
        outline.push(Vec2::new(reach, 0.0) + Vec2::from_angle(angle) * radii.1);
    }
    for step in 0..=steps {
        let angle = tangent + (std::f32::consts::TAU - 2.0 * tangent) * step as f32 / steps as f32;
        outline.push(Vec2::from_angle(angle) * radii.0);
    }
    outline
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/solid.rs"]
mod tests;
