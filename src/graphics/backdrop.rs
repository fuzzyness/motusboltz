//! The background behind the engine, in one of two sceneries. The sky: a
//! blue sky with drifting clouds above the horizon and a sea of clouds
//! below it, seen along each pixel's line of sight, so it turns with the
//! camera. The grid: deep blue, a little lighter toward the floor and
//! darker at the window's corners. Both add a warm glow behind the hot
//! cylinder and a cool one behind the cold cylinder, each as strong as its
//! reservoir is hot or cold, following the cylinders as the camera turns.
//! One full-window quad drawn before the scene, so it costs one draw call;
//! without its shader the plain clear color stays.

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

/// Each pixel's line of sight comes from `Unproject` (the camera's view
/// and projection, inverted). Above the horizon: the sky, brighter toward
/// the sun, with thin clouds on a high layer. Below it: the tops of a sea
/// of clouds on a layer under the engine, lit from the sun's side and
/// fading into haze in the distance. The noise is value noise from a hash
/// with no sine (see the grid's shader).
const SKY_FRAGMENT: &str = r#"#version 100
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
uniform mat4 Unproject;
uniform vec3 Eye;
uniform float Time;
uniform float Sea;

const vec3 SUN = vec3(0.55, 0.42, -0.72);
const vec3 ZENITH = vec3(0.13, 0.36, 0.86);
const vec3 HORIZON = vec3(0.66, 0.82, 1.0);
const vec3 GAP = vec3(0.42, 0.58, 0.88);

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

float fbm(vec2 p) {
    float sum = 0.0;
    float weight = 0.5;
    for (int octave = 0; octave < 5; octave++) {
        sum += weight * noise(p);
        p = p * 2.03 + vec2(17.1, 9.3);
        weight *= 0.5;
    }
    return sum;
}

vec3 glow(vec4 spot, vec3 color) {
    vec2 d = (uv - spot.xy) * vec2(Aspect, 1.0) / max(spot.z, 0.001);
    return color * spot.w * exp(-2.2 * dot(d, d));
}

void main() {
    vec2 ndc = vec2(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    vec4 near = Unproject * vec4(ndc, -1.0, 1.0);
    vec4 far = Unproject * vec4(ndc, 0.0, 1.0);
    vec3 ray = normalize(far.xyz / far.w - near.xyz / near.w);
    vec3 sun = normalize(SUN);
    float toward_sun = max(dot(ray, sun), 0.0);
    vec3 sky = mix(HORIZON, ZENITH, smoothstep(0.0, 0.6, ray.y));
    sky += vec3(1.0, 0.85, 0.6) * (0.25 * pow(toward_sun, 8.0) + 0.6 * pow(toward_sun, 200.0));
    vec3 color;
    if (ray.y > 0.0) {
        // Thin clouds high above, thinning toward the horizon.
        vec2 high = ray.xz / max(ray.y, 0.05) * 0.9 + vec2(Time * 0.01, 0.0);
        float cover = smoothstep(0.52, 0.78, fbm(high)) * smoothstep(0.02, 0.25, ray.y);
        color = mix(sky, vec3(1.0), cover * 0.85);
    } else {
        // The sea of clouds below: where this line of sight meets it.
        float distance = (Sea - Eye.y) / min(ray.y, -0.001);
        vec2 place = (Eye.xz + ray.xz * distance) * 0.045 + vec2(Time * 0.012, Time * 0.004);
        float density = fbm(place);
        float lit = fbm(place + sun.xz * 0.12);
        float cloud = smoothstep(0.4, 0.58, density);
        float shade = clamp(0.78 + (density - lit) * 2.2, 0.55, 1.08);
        color = mix(GAP, vec3(1.0, 0.99, 0.97) * shade, cloud);
        // Haze hides the far, streaky part of the sea.
        color = mix(color, HORIZON, 1.0 - exp(-distance * 0.007));
    }
    color += 0.55 * glow(Hot, vec3(1.0, 0.45, 0.15));
    color += 0.55 * glow(Cold, vec3(0.2, 0.5, 1.0));
    gl_FragColor = vec4(color, 1.0) * texture2D(Texture, uv);
}"#;

/// Which background and floor the engine is shown in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scenery {
    /// A blue sky above a sea of clouds, the engine standing on a cloud.
    #[default]
    Sky,
    /// Deep blue with a grid on the floor.
    Grid,
}

impl Scenery {
    pub fn toggled(self) -> Self {
        match self {
            Self::Sky => Self::Grid,
            Self::Grid => Self::Sky,
        }
    }
}

/// How far below the floor the sea of clouds lies, in bore radii.
const SEA_DEPTH: f32 = 7.0;

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
    grid: Option<Material>,
    sky: Option<Material>,
    mesh: Mesh,
    geometry: ChamberGeometry,
}

impl Backdrop {
    pub fn new(geometry: ChamberGeometry) -> Self {
        let material = |fragment, sky: bool| {
            let mut uniforms = vec![
                UniformDesc::new("Hot", UniformType::Float4),
                UniformDesc::new("Cold", UniformType::Float4),
                UniformDesc::new("Aspect", UniformType::Float1),
            ];
            if sky {
                uniforms.extend([
                    UniformDesc::new("Unproject", UniformType::Mat4),
                    UniformDesc::new("Eye", UniformType::Float3),
                    UniformDesc::new("Time", UniformType::Float1),
                    UniformDesc::new("Sea", UniformType::Float1),
                ]);
            }
            load_material(
                ShaderSource::Glsl {
                    vertex: BACKDROP_VERTEX,
                    fragment,
                },
                MaterialParams {
                    uniforms,
                    ..Default::default()
                },
            )
            .ok()
        };
        Self {
            grid: material(BACKDROP_FRAGMENT, false),
            sky: material(SKY_FRAGMENT, true),
            mesh: Mesh {
                vertices: Vec::new(),
                indices: vec![0, 1, 2, 0, 2, 3],
                texture: None,
            },
            geometry,
        }
    }

    /// Fills the `width` by `height` view, in screen points, in `scenery`,
    /// with a 2D camera over it set. `heat` and `cold` are the reservoirs'
    /// levels (see `heat_level` and `cold_level`); `seconds` drifts the
    /// clouds.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        scenery: Scenery,
        camera: &Camera3D,
        width: f32,
        height: f32,
        heat: f32,
        cold: f32,
        seconds: f64,
    ) {
        let material = match scenery {
            Scenery::Sky => self.sky.as_ref().or(self.grid.as_ref()),
            Scenery::Grid => self.grid.as_ref(),
        };
        let Some(material) = material else {
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
        if scenery == Scenery::Sky && self.sky.is_some() {
            material.set_uniform("Unproject", matrix.inverse());
            material.set_uniform("Eye", camera.position);
            // Wrapped, so the clouds stay smooth however long it runs.
            material.set_uniform("Time", (seconds % 100_000.0) as f32);
            material.set_uniform("Sea", floor - SEA_DEPTH * self.geometry.cylinder_radius);
        }
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
