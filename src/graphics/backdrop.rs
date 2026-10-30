//! The background behind the engine: deep blue, a little lighter toward
//! the floor and darker at the window's corners, with a warm glow behind
//! the hot cylinder and a cool one behind the cold cylinder, each as
//! strong as its reservoir is hot or cold. The glows follow the cylinders
//! as the camera turns. One full-window quad drawn before the scene, so it
//! costs one draw call; without its shader the plain clear color stays.

use macroquad::models::{
    Mesh,
    Vertex,
    draw_mesh, //
};
use macroquad::prelude::{
    Camera,
    Camera3D,
    Mat4,
    Material,
    MaterialParams,
    ShaderSource,
    UniformDesc,
    UniformType,
    Vec2,
    Vec3,
    Vec4,
    WHITE,
    gl_use_default_material,
    gl_use_material,
    load_material,
    vec2,
    vec3, //
};

use crate::sim::geometry::ChamberGeometry;

const BACKDROP_VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;

varying vec2 uv;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    uv = texcoord;
}"#;

/// `uv` runs from the window's top left (0, 0) to its bottom right (1, 1).
/// Each glow is (x, y, radius, strength), with x and y in `uv` and the
/// radius as a share of the window's height.
const BACKDROP_FRAGMENT: &str = r#"#version 100
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif

varying vec2 uv;

uniform sampler2D Texture;
uniform vec4 Hot;
uniform vec4 Cold;
uniform float Aspect;

vec3 glow(vec4 spot, vec3 color) {
    vec2 d = (uv - spot.xy) * vec2(Aspect, 1.0) / max(spot.z, 0.001);
    return color * spot.w * exp(-2.2 * dot(d, d));
}

void main() {
    vec3 top = vec3(0.030, 0.043, 0.085);
    vec3 low = vec3(0.078, 0.106, 0.158);
    vec3 color = mix(top, low, smoothstep(0.0, 1.0, uv.y));
    vec2 corner = (uv - vec2(0.5, 0.55)) * vec2(Aspect, 1.0);
    color *= 1.0 - 0.45 * smoothstep(0.35, 1.15, length(corner));
    color += glow(Hot, vec3(1.0, 0.42, 0.12));
    color += glow(Cold, vec3(0.25, 0.56, 1.0));
    // A little noise, under one step of brightness, hides banding. This
    // pattern (interleaved gradient noise) needs no sine, whose accuracy
    // some GPUs lose for large arguments.
    vec2 pixel = floor(gl_FragCoord.xy);
    float noise = fract(52.9829189 * fract(dot(pixel, vec2(0.06711056, 0.00583715))));
    color += (noise - 0.5) / 255.0;
    gl_FragColor = vec4(color, 1.0) * texture2D(Texture, uv);
}"#;

/// Strength of each glow at full heat or cold.
const HOT_GLOW: f32 = 0.38;
const COLD_GLOW: f32 = 0.3;
/// A glow's radius against the cylinder's height on the screen.
const GLOW_REACH: f32 = 1.0;

/// Where a glow goes on the window and how far it reaches, both in shares
/// of the window (see the shader).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub center: Vec2,
    pub radius: f32,
}

/// Projects the cylinder whose axis is at `axis_x`, from `bottom` to
/// `top`, through `matrix`; `None` if it is behind the camera.
pub fn spot(matrix: Mat4, axis_x: f32, bottom: f32, top: f32) -> Option<Spot> {
    let project = |point: Vec3| {
        let clip = matrix * point.extend(1.0);
        (clip.w > f32::EPSILON).then(|| {
            let ndc = clip.truncate() / clip.w;
            vec2((ndc.x + 1.0) / 2.0, (1.0 - ndc.y) / 2.0)
        })
    };
    let middle = project(vec3(axis_x, (bottom + top) / 2.0, 0.0))?;
    let low = project(vec3(axis_x, bottom, 0.0))?;
    let high = project(vec3(axis_x, top, 0.0))?;
    Some(Spot {
        center: middle,
        radius: GLOW_REACH * (low - high).length(),
    })
}

pub struct Backdrop {
    material: Option<Material>,
    mesh: Mesh,
    geometry: ChamberGeometry,
}

impl Backdrop {
    pub fn new(geometry: ChamberGeometry) -> Self {
        let material = load_material(
            ShaderSource::Glsl {
                vertex: BACKDROP_VERTEX,
                fragment: BACKDROP_FRAGMENT,
            },
            MaterialParams {
                uniforms: vec![
                    UniformDesc::new("Hot", UniformType::Float4),
                    UniformDesc::new("Cold", UniformType::Float4),
                    UniformDesc::new("Aspect", UniformType::Float1),
                ],
                ..Default::default()
            },
        )
        .ok();
        Self {
            material,
            mesh: Mesh {
                vertices: Vec::new(),
                indices: vec![0, 1, 2, 0, 2, 3],
                texture: None,
            },
            geometry,
        }
    }

    /// Fills the `width` by `height` window, in screen points, with the
    /// screen camera set. `heat` and `cold` are the reservoirs' levels
    /// (see `heat_level` and `cold_level`).
    pub fn draw(&mut self, camera: &Camera3D, width: f32, height: f32, heat: f32, cold: f32) {
        let Some(material) = &self.material else {
            return;
        };
        let matrix = camera.matrix();
        let floor = self.geometry.floor();
        let top = self.geometry.cylinder_height / 2.0;
        let uniform = |axis_x: f32, strength: f32| {
            spot(matrix, axis_x, floor, top).map_or(Vec4::ZERO, |spot| {
                Vec4::new(spot.center.x, spot.center.y, spot.radius, strength)
            })
        };
        material.set_uniform(
            "Hot",
            uniform(self.geometry.hot_center_x(), HOT_GLOW * heat),
        );
        material.set_uniform(
            "Cold",
            uniform(self.geometry.cold_center_x(), COLD_GLOW * cold),
        );
        material.set_uniform("Aspect", width / height.max(1.0));
        self.mesh.vertices.clear();
        for (x, y) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            self.mesh.vertices.push(Vertex::new2(
                vec3(x * width, y * height, 0.0),
                vec2(x, y),
                WHITE,
            ));
        }
        gl_use_material(material);
        draw_mesh(&self.mesh);
        gl_use_default_material();
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/backdrop.rs"]
mod tests;
