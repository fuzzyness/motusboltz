//! Soft round sprites for the flames, embers, frost and mist.
//!
//! Sprites are flat quads whose shader fades each one from its middle to
//! nothing at its edge, so hundreds of them cost one draw call and no
//! texture. Glowing sprites add their light to what is behind them; hazy
//! ones cover it partly.
//!
//! Miniquad 0.4.11 tests depth only for pipelines that also write it, so
//! the sprites write depth too: the shader drops their clear corners, and
//! each batch is drawn from the farthest sprite to the nearest, so none
//! hides one drawn after it.
//!
//! The effects built from them are animated from the clock alone (each
//! sprite's age is a fraction of its own repeating life), so they keep no
//! state between frames and look the same at any frame rate.

use macroquad::miniquad::{
    BlendFactor,
    BlendState,
    BlendValue,
    Equation, //
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
    Vec3,
    gl_use_default_material,
    gl_use_material,
    load_material,
    vec2, //
};

use super::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY, //
};

/// Vertex shader shared by every sprite.
const SPRITE_VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;

varying lowp vec4 color;
varying lowp vec2 uv;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    color = color0 / 255.0;
    uv = texcoord;
}"#;

/// Round sprite: full strength in the middle, falling smoothly to nothing
/// at the rim.
const SPRITE_FRAGMENT: &str = r#"#version 100
precision mediump float;

varying lowp vec4 color;
varying lowp vec2 uv;

uniform sampler2D Texture;

void main() {
    vec2 p = uv * 2.0 - 1.0;
    float fade = max(1.0 - dot(p, p), 0.0);
    float alpha = color.a * fade * fade;
    // Nearly clear: leave the depth alone, so nothing behind is hidden.
    if (alpha < 0.004) {
        discard;
    }
    gl_FragColor = vec4(color.rgb, alpha) * texture2D(Texture, uv);
}"#;

/// Flame tongue in the style of a Dreamcast game.
///
/// It is round at its root, drawn out to a point at its tip (`uv.y` runs
/// from root to tip), with a white-hot core low down and a soft edge.
const FLAME_FRAGMENT: &str = r#"#version 100
precision mediump float;

varying lowp vec4 color;
varying lowp vec2 uv;

uniform sampler2D Texture;

void main() {
    float across = abs(uv.x * 2.0 - 1.0);
    float up = uv.y;
    float width = pow(max(1.0 - up, 0.0), 0.75) * smoothstep(0.0, 0.3, up + 0.1);
    float edge = across / max(width, 0.001);
    float body = 1.0 - smoothstep(0.55, 1.0, edge);
    float core = (1.0 - smoothstep(0.0, 0.6, edge)) * (1.0 - smoothstep(0.15, 0.7, up));
    float alpha = color.a * body;
    if (alpha < 0.004) {
        discard;
    }
    vec3 rgb = mix(color.rgb, vec3(1.0, 0.97, 0.82), core * 0.85);
    // Weighted by alpha here, and covering only part of what is behind:
    // flames over flames build toward their own color, not white.
    gl_FragColor = vec4(rgb * alpha, alpha * 0.7) * texture2D(Texture, uv);
}"#;

/// Four-pointed twinkle with a round glow at its middle.
///
/// Dreamcast games drew sparkles on ice and sparks off a fire this way.
const SPARKLE_FRAGMENT: &str = r#"#version 100
precision mediump float;

varying lowp vec4 color;
varying lowp vec2 uv;

uniform sampler2D Texture;

void main() {
    vec2 p = uv * 2.0 - 1.0;
    vec2 a = abs(p);
    float rays = max(1.0 - a.x * 9.0, 0.0) * (1.0 - a.y) + max(1.0 - a.y * 9.0, 0.0) * (1.0 - a.x);
    float glow = exp(-dot(p, p) * 14.0);
    float alpha = color.a * clamp(rays + glow, 0.0, 1.0);
    if (alpha < 0.004) {
        discard;
    }
    vec3 rgb = mix(color.rgb, vec3(1.0), glow * 0.7);
    gl_FragColor = vec4(rgb, alpha) * texture2D(Texture, uv);
}"#;

/// Corners of a sprite.
const VERTICES_PER_SPRITE: usize = 4;
/// Indices of a sprite's two triangles.
const INDICES_PER_SPRITE: usize = 6;
/// The most sprites one mesh holds, within Macroquad's draw-call limits.
pub const MAX_SPRITES: usize = {
    let by_vertices = DRAW_CALL_VERTEX_CAPACITY / VERTICES_PER_SPRITE;
    let by_indices = DRAW_CALL_INDEX_CAPACITY / INDICES_PER_SPRITE;
    if by_vertices < by_indices {
        by_vertices
    } else {
        by_indices
    }
};

/// The ways sprites mix with what is already drawn.
///
/// All test depth, so the engine hides what is behind them.
pub struct SpriteMaterials {
    /// Adds light, round: the flames' glow on the floor and the burner's
    /// blue port flames.
    pub glow: Material,
    /// Covers partly, round: frost and mist.
    pub haze: Material,
    /// Adds light, shaped as a flame tongue, while partly covering what is
    /// behind, so many flames overlapping stay orange and yellow instead
    /// of burning out to white.
    pub flame: Material,
    /// Adds light, shaped as a twinkle: sparks and sparkles.
    pub sparkle: Material,
    /// Like `sparkle`, drawn over everything, for heat sparks at the far
    /// wall of a bore, where the gas in front would hide them.
    pub overlay: Material,
}

impl SpriteMaterials {
    /// Builds the materials, or returns `None` if the shader cannot be
    /// built.
    ///
    /// Without them, the effects are left out.
    pub fn load() -> Option<Self> {
        let alpha = BlendFactor::Value(BlendValue::SourceAlpha);
        let behind = BlendFactor::OneMinusValue(BlendValue::SourceAlpha);
        let load = |pipeline_params, fragment| {
            let source = ShaderSource::Glsl {
                vertex: SPRITE_VERTEX,
                fragment,
            };
            let params = MaterialParams {
                pipeline_params,
                ..Default::default()
            };
            load_material(source, params).ok()
        };
        let material = |source, behind, fragment| load(sprite_pipeline(source, behind), fragment);
        let overlay = PipelineParams {
            depth_test: Comparison::Always,
            depth_write: false,
            ..sprite_pipeline(alpha, BlendFactor::One)
        };
        Some(Self {
            glow: material(alpha, BlendFactor::One, SPRITE_FRAGMENT)?,
            haze: material(alpha, behind, SPRITE_FRAGMENT)?,
            flame: material(BlendFactor::One, behind, FLAME_FRAGMENT)?,
            sparkle: material(alpha, BlendFactor::One, SPARKLE_FRAGMENT)?,
            overlay: load(overlay, SPARKLE_FRAGMENT)?,
        })
    }
}

/// The pipeline for sprites blended as `source` times the sprite plus
/// `behind` times what is there.
///
/// Sprites are depth tested, and so written (see the top of this file).
/// The window's alpha is kept as it is.
fn sprite_pipeline(source: BlendFactor, behind: BlendFactor) -> PipelineParams {
    let keep_alpha = BlendState::new(Equation::Add, BlendFactor::Zero, BlendFactor::One);
    PipelineParams {
        depth_test: Comparison::LessOrEqual,
        depth_write: true,
        color_blend: Some(BlendState::new(Equation::Add, source, behind)),
        alpha_blend: Some(keep_alpha),
        ..Default::default()
    }
}

/// The directions across and up the screen, and into it, in the scene.
///
/// With them a sprite can face the camera, and sprites can be drawn far to
/// near.
#[derive(Clone, Copy, Debug)]
pub struct Facing {
    /// Across the screen, to the right.
    pub right: Vec3,
    /// Up the screen.
    pub up: Vec3,
    /// Into the screen.
    pub forward: Vec3,
}

impl Facing {
    /// Facing `camera`.
    pub fn toward(camera: &Camera3D) -> Self {
        Self::along((camera.target - camera.position).normalize_or_zero())
    }

    /// Facing a camera that looks along `forward`.
    pub fn along(forward: Vec3) -> Self {
        let right = forward.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);
        Self {
            right,
            up: right.cross(forward).normalize_or_zero(),
            forward,
        }
    }
}

/// One sprite before it is put in the mesh.
#[derive(Clone, Copy, Debug)]
struct Sprite {
    /// Middle.
    center: Vec3,
    /// From the middle to the left and right edges.
    across: Vec3,
    /// From the middle to the bottom and top edges.
    along: Vec3,
    /// Color and opacity.
    color: Color,
    /// Lying on the floor: drawn before every upright sprite.
    flat: bool,
}

/// A batch of sprites, rebuilt every frame in reused buffers.
///
/// Sprites are gathered with `facing`, `flat` and `push`, then put in order
/// by `finish`.
pub struct Sprites {
    /// The sprites gathered this frame.
    gathered: Vec<Sprite>,
    /// The sprites in order, as quads.
    mesh: Mesh,
}

impl Sprites {
    /// An empty batch.
    pub fn new() -> Self {
        Self {
            gathered: Vec::new(),
            mesh: Mesh {
                vertices: Vec::new(),
                indices: Vec::new(),
                texture: None,
            },
        }
    }

    /// Empties the batch.
    pub fn clear(&mut self) {
        self.gathered.clear();
        self.mesh.vertices.clear();
        self.mesh.indices.clear();
    }

    /// Adds a sprite centered on `center`, spanning `across` and `along`
    /// from its middle to its edges.
    ///
    /// It is ignored once the batch is full or when it would not show.
    pub fn push(&mut self, center: Vec3, across: Vec3, along: Vec3, color: Color) {
        self.gather(Sprite {
            center,
            across,
            along,
            color,
            flat: false,
        });
    }

    /// Adds `sprite` if it shows and there is room.
    fn gather(&mut self, sprite: Sprite) {
        if sprite.color.a > 0.0 && self.gathered.len() < MAX_SPRITES {
            self.gathered.push(sprite);
        }
    }

    /// Adds a sprite facing the camera, `half_width` by `half_height`.
    pub fn facing(
        &mut self,
        facing: Facing,
        center: Vec3,
        half_width: f32,
        half_height: f32,
        color: Color,
    ) {
        let (across, along) = (facing.right * half_width, facing.up * half_height);
        self.push(center, across, along, color);
    }

    /// Adds a round sprite lying flat, such as a glow on the floor.
    pub fn flat(&mut self, center: Vec3, half_size: f32, color: Color) {
        self.gather(Sprite {
            center,
            across: Vec3::X * half_size,
            along: Vec3::Z * half_size,
            color,
            flat: true,
        });
    }

    /// Builds the mesh: flat sprites first, then the rest from the
    /// farthest along `forward` to the nearest.
    pub fn finish(&mut self, forward: Vec3) {
        let depth = |sprite: &Sprite| sprite.center.dot(forward);
        self.gathered.sort_by(|first, second| {
            let farther = depth(second).total_cmp(&depth(first));
            second.flat.cmp(&first.flat).then(farther)
        });
        self.mesh.vertices.clear();
        self.mesh.indices.clear();
        for sprite in &self.gathered {
            let first = self.mesh.vertices.len() as u16;
            let Sprite {
                center,
                across,
                along,
                color,
                ..
            } = *sprite;
            for uv in [
                vec2(0.0, 0.0),
                vec2(1.0, 0.0),
                vec2(1.0, 1.0),
                vec2(0.0, 1.0),
            ] {
                // From the middle to this corner: -1 or 1 times each side.
                let corner = (2.0 * uv.x - 1.0) * across + (2.0 * uv.y - 1.0) * along;
                self.mesh
                    .vertices
                    .push(Vertex::new2(center + corner, uv, color));
            }
            self.mesh
                .indices
                .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
        }
    }

    /// Draws the batch with `material`.
    pub fn draw(&self, material: &Material) {
        if self.mesh.indices.is_empty() {
            return;
        }
        gl_use_material(material);
        draw_mesh(&self.mesh);
        gl_use_default_material();
    }
}

/// A repeatable number in [0, 1) for `index` and `salt`.
///
/// With it every sprite keeps its own place, size and timing from frame to
/// frame.
pub fn unit_hash(index: u32, salt: u32) -> f32 {
    let mut x = index
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(salt.wrapping_mul(0x85EB_CA6B));
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    (x >> 8) as f32 / (1_u32 << 24) as f32
}

/// How far through its life a sprite is, in [0, 1).
///
/// The age is the clock times the sprite's rate plus its own offset,
/// wrapped.
pub fn age(seconds: f64, cycles_per_second: f32, offset: f32) -> f32 {
    (seconds * cycles_per_second as f64 + offset as f64).rem_euclid(1.0) as f32
}

/// 0 below `low`, 1 above `high`, and a smooth S-curve between.
pub fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
    let t = ((x - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `color` with its opacity set to `alpha`.
pub fn with_alpha(color: Color, alpha: f32) -> Color {
    Color {
        a: alpha.clamp(0.0, 1.0),
        ..color
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/sprites.rs"]
mod tests;
