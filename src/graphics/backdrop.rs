//! The background behind the engine, in one of two sceneries.
//!
//! The sky: a blue sky with drifting clouds above the horizon and a sea of clouds
//! below it, seen along each pixel's line of sight, so it turns with the
//! camera. The grid: deep blue, a little lighter toward the floor and
//! darker at the window's corners. Both add a warm glow behind the hot
//! cylinder and a cool one behind the cold cylinder, each as strong as its
//! reservoir is hot or cold, following the cylinders as the camera turns.
//! One full-window quad, so it costs one draw call; without its shader the
//! plain clear color stays. It is drawn after the solid parts of the scene
//! but behind them, at the far end of the depth range, so only the pixels
//! left uncovered are shaded, rather than every pixel only for the table and
//! the engine to cover most of them.
//!
//! The sky's clouds are value noise read from a small texture of random
//! values, which the GPU's own smoothing interpolates, rather than hashed
//! anew for every pixel: at most ten texture reads a pixel in place of
//! forty hashes.

use macroquad::models::{
    Mesh,
    Vertex,
    draw_mesh, //
};
use macroquad::prelude::{
    Camera,
    Camera3D,
    Comparison,
    FilterMode,
    Mat4,
    Material,
    MaterialParams,
    PipelineParams,
    ShaderSource,
    Texture2D,
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
use macroquad::rand::RandGenerator;

use super::block::BlockShape;
use super::levels::ReservoirLevels;
use super::table::ground;
use crate::sim::geometry::ChamberGeometry;

/// Vertex shader for the full-window quad: it places the corners by their
/// texture coordinates straight on the window, whatever camera is set, at
/// the far end of the depth range, behind everything.
///
/// `uv` runs from the window's top left (0, 0) to its bottom right (1, 1),
/// as the sky shader's lines of sight and the glows' spots expect.
const BACKDROP_VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;

varying vec2 uv;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = vec4(texcoord.x * 2.0 - 1.0, 1.0 - texcoord.y * 2.0, 1.0, 1.0);
    uv = texcoord;
}"#;

/// Fragment shader of the grid scenery.
///
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

/// Fragment shader of the sky scenery.
///
/// Each pixel's line of sight comes from `Unproject`: the camera's view and
/// projection inverted, with the camera moved to the origin, so a point
/// unprojected from the window is the line of sight itself. Taken from
/// the camera where it stands, two points a fraction of a centimeter
/// apart in front of it, among coordinates of tens of centimeters, lost
/// enough precision to the near plane of first person that the line of
/// sight wobbled from frame to frame as the camera moved, and the clouds,
/// read far along it, flickered. Above the horizon: the sky, brighter toward
/// the sun, with thin clouds on a high layer. Below it: the tops of a sea
/// of clouds on a layer under the engine, lit from the sun's side and
/// fading into haze in the distance. The noise is value noise whose values
/// at the lattice points are read from `Noise` (see `noise_bytes`), so the
/// texture's smoothing does the interpolating.
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
uniform sampler2D Noise;

const vec3 SUN = vec3(0.55, 0.42, -0.72);
const vec3 ZENITH = vec3(0.13, 0.36, 0.86);
const vec3 HORIZON = vec3(0.66, 0.82, 1.0);
const vec3 GAP = vec3(0.42, 0.58, 0.88);

// Cells across `Noise` before it repeats (see `NOISE_CELLS`).
const float NOISE_CELLS = 128.0;

// The values at a cell's corners eased together: the texture holds one
// cell more than it repeats after, so the four corners are neighbors, and
// reading between their texels at the eased position interpolates them.
float noise(vec2 p) {
    vec2 cell = mod(floor(p), NOISE_CELLS);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return texture2D(Noise, (cell + u + 0.5) / (NOISE_CELLS + 1.0)).r;
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
    vec4 sight = Unproject * vec4(ndc, 0.0, 1.0);
    vec3 ray = normalize(sight.xyz / sight.w);
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
    /// The other scenery.
    pub fn toggled(self) -> Self {
        match self {
            Self::Sky => Self::Grid,
            Self::Grid => Self::Sky,
        }
    }
}

/// Cells across the sky's noise texture before it repeats; `SKY_FRAGMENT`
/// says the same.
///
/// The repeat lies far beyond the clouds' largest shapes, and each layer
/// of detail reads the texture at a scale that is no whole multiple of
/// another's, so it does not show.
const NOISE_CELLS: usize = 128;
/// Seed of the sky's noise, so the clouds have the same shapes on every
/// run.
const NOISE_SEED: u64 = 1_046;
/// Bytes per texel of an RGBA texture.
const BYTES_PER_TEXEL: usize = 4;

/// The sky's noise texture's bytes, RGBA: random values in red, green and
/// blue alike, opaque, `NOISE_CELLS` each way and then the first row and
/// column again, so every cell's four corners are side by side and the
/// noise repeats seamlessly.
fn noise_bytes() -> Vec<u8> {
    let random = RandGenerator::new();
    random.srand(NOISE_SEED);
    let values: Vec<u8> = (0..NOISE_CELLS * NOISE_CELLS)
        .map(|_| random.gen_range(0_u16, u16::from(u8::MAX) + 1) as u8)
        .collect();
    let side = NOISE_CELLS + 1;
    let mut bytes = Vec::with_capacity(side * side * BYTES_PER_TEXEL);
    for row in 0..side {
        for column in 0..side {
            let value = values[(row % NOISE_CELLS) * NOISE_CELLS + column % NOISE_CELLS];
            bytes.extend([value, value, value, u8::MAX]);
        }
    }
    bytes
}

/// How far below the floor the table stands on the sea of clouds lies,
/// in bore radii.
const SEA_DEPTH: f32 = 7.0;

/// Strength of the hot glow at full heat.
const HOT_GLOW: f32 = 0.38;
/// Strength of the cold glow at full cold.
const COLD_GLOW: f32 = 0.3;
/// A glow's radius against the cylinder's height on the screen.
const GLOW_REACH: f32 = 1.0;
/// Distance in front of the camera, in the cylinder's heights, inside
/// which its glow starts to fade.
///
/// A cylinder close beside the camera, as it is when walking past it in
/// first person, projects almost to infinity: its glow grew to cover the
/// whole sky, and as each step carried the cylinder to and fro across the
/// camera's side, flashed over it for a frame at a time. The glow is for
/// the engine seen from outside, and fades away as the camera comes close.
const GLOW_FADE_FAR: f32 = 1.5;
/// Distance in front of the camera, in the cylinder's heights, inside
/// which its glow is gone.
const GLOW_FADE_NEAR: f32 = 0.75;

/// Where a glow goes on the window and how far it reaches, both in shares
/// of the window (see the shader).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    /// Middle of the glow.
    pub center: Vec2,
    /// Reach of the glow.
    pub radius: f32,
    /// How much of its strength the glow keeps, from 0 to 1: less with the
    /// cylinder close to the camera.
    pub share: f32,
}

/// Projects the cylinder whose axis is at `axis_x`, from `bottom` to
/// `top`, through `matrix`; `None` if it is behind the camera or so close
/// in front that its glow is gone (see `GLOW_FADE_FAR`).
pub fn spot(matrix: Mat4, axis_x: f32, bottom: f32, top: f32) -> Option<Spot> {
    let height = (top - bottom).abs().max(f32::EPSILON);
    let ahead = (matrix * vec3(axis_x, (bottom + top) / 2.0, 0.0).extend(1.0)).w / height;
    let share = ((ahead - GLOW_FADE_NEAR) / (GLOW_FADE_FAR - GLOW_FADE_NEAR)).clamp(0.0, 1.0);
    if share <= 0.0 {
        return None;
    }
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
        share: share * share * (3.0 - 2.0 * share),
    })
}

/// The background quad and its two shaders.
pub struct Backdrop {
    /// The grid scenery's shader, if it can be built.
    grid: Option<Material>,
    /// The sky scenery's shader, if it can be built.
    sky: Option<Material>,
    /// The sky's noise, made with its shader. Macroquad frees a texture
    /// once nothing holds it, so the shader's hold is not enough.
    noise: Option<Texture2D>,
    /// The full-window quad.
    mesh: Mesh,
    /// Where the cylinders are, for the glows.
    geometry: ChamberGeometry,
}

impl Backdrop {
    /// The background for the engine of `geometry`.
    pub fn new(geometry: ChamberGeometry) -> Self {
        let material = |fragment, sky: bool| {
            let textures = if sky {
                vec!["Noise".to_owned()]
            } else {
                Vec::new()
            };
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
                    // Tested against what is drawn, and so writing depth
                    // too (Miniquad tests depth only for pipelines that
                    // write it), at the far end where nothing is.
                    pipeline_params: PipelineParams {
                        depth_write: true,
                        depth_test: Comparison::LessOrEqual,
                        ..Default::default()
                    },
                    uniforms,
                    textures,
                },
            )
            .ok()
        };
        let sky = material(SKY_FRAGMENT, true);
        let noise = sky.as_ref().map(|_| {
            let side = (NOISE_CELLS + 1) as u16;
            let noise = Texture2D::from_rgba8(side, side, &noise_bytes());
            noise.set_filter(FilterMode::Linear);
            noise
        });
        Self {
            grid: material(BACKDROP_FRAGMENT, false),
            sky,
            noise,
            mesh: Mesh {
                vertices: Vec::new(),
                indices: vec![0, 1, 2, 0, 2, 3],
                texture: None,
            },
            geometry,
        }
    }

    /// Fills whatever is still empty of the view of `size`, in screen
    /// points, seen from `camera`, in `scenery`.
    ///
    /// The camera must be set, and everything solid drawn: the quad is
    /// drawn behind it. Anything see-through goes after, so it blends over
    /// the background. The glows follow the reservoirs' `levels`; `seconds`
    /// drifts the clouds.
    pub fn draw(
        &mut self,
        scenery: Scenery,
        camera: &Camera3D,
        size: Vec2,
        levels: ReservoirLevels,
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
                Vec4::new(
                    spot.center.x,
                    spot.center.y,
                    spot.radius,
                    strength * spot.share,
                )
            })
        };
        let hot = uniform(self.geometry.hot_center_x(), HOT_GLOW * levels.heat);
        let cold = uniform(self.geometry.cold_center_x(), COLD_GLOW * levels.cold);
        material.set_uniform("Hot", hot);
        material.set_uniform("Cold", cold);
        material.set_uniform("Aspect", size.x / size.y.max(1.0));
        if let (Scenery::Sky, Some(_), Some(noise)) = (scenery, &self.sky, &self.noise) {
            material.set_texture("Noise", noise.clone());
            // The view turned but not moved: the camera at the origin.
            let turned = matrix * Mat4::from_translation(camera.position);
            material.set_uniform("Unproject", turned.inverse());
            material.set_uniform("Eye", camera.position);
            // Wrapped, so the clouds stay smooth however long it runs.
            material.set_uniform("Time", (seconds % 100_000.0) as f32);
            let ground = ground(BlockShape::new(self.geometry));
            material.set_uniform("Sea", ground - SEA_DEPTH * self.geometry.cylinder_radius);
        }
        self.mesh.vertices.clear();
        // The shader places the corners by their texture coordinates.
        for corner in [
            vec2(0.0, 0.0),
            vec2(1.0, 0.0),
            vec2(1.0, 1.0),
            vec2(0.0, 1.0),
        ] {
            self.mesh
                .vertices
                .push(Vertex::new2(Vec3::ZERO, corner, WHITE));
        }
        gl_use_material(material);
        draw_mesh(&self.mesh);
        gl_use_default_material();
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/backdrop.rs"]
mod tests;
