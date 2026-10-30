//! Soft round sprites for the flames, embers, frost and mist: flat quads
//! whose shader fades each one from its middle to nothing at its edge, so
//! hundreds of them cost one draw call and no texture. Glowing sprites add
//! their light to what is behind them; hazy ones cover it partly.
//!
//! Miniquad 0.4.11 tests depth only for pipelines that also write it, so
//! the sprites write depth too: the shader drops their clear corners, and
//! each batch is drawn from the farthest sprite to the nearest, so none
//! hides one drawn after it.
//!
//! The effects built from them are animated from the clock alone (each
//! sprite's age is a fraction of its own repeating life), so they keep no
//! state between frames and look the same at any frame rate.

use std::cmp::Ordering;

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

/// Full strength in the middle, falling smoothly to nothing at the rim.
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

const VERTICES_PER_SPRITE: usize = 4;
const INDICES_PER_SPRITE: usize = 6;
/// The most sprites one mesh holds, within Macroquad's draw-call limits.
pub const MAX_SPRITES: usize = if DRAW_CALL_VERTEX_CAPACITY / VERTICES_PER_SPRITE
    < DRAW_CALL_INDEX_CAPACITY / INDICES_PER_SPRITE
{
    DRAW_CALL_VERTEX_CAPACITY / VERTICES_PER_SPRITE
} else {
    DRAW_CALL_INDEX_CAPACITY / INDICES_PER_SPRITE
};

/// The ways sprites mix with what is already drawn. All test depth, so
/// the engine hides what is behind them.
pub struct SpriteMaterials {
    /// Adds light: flames, embers and their glow on the floor.
    pub glow: Material,
    /// Covers partly: frost and mist.
    pub haze: Material,
}

impl SpriteMaterials {
    /// Builds the materials, or returns `None` if the shader cannot be
    /// built; the effects are then left out.
    pub fn load() -> Option<Self> {
        let material = |behind: BlendFactor, fragment| {
            load_material(
                ShaderSource::Glsl {
                    vertex: SPRITE_VERTEX,
                    fragment,
                },
                MaterialParams {
                    pipeline_params: sprite_pipeline(behind),
                    ..Default::default()
                },
            )
            .ok()
        };
        Some(Self {
            glow: material(BlendFactor::One, SPRITE_FRAGMENT)?,
            haze: material(
                BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                SPRITE_FRAGMENT,
            )?,
        })
    }
}

/// Depth tested (and so written; see the top of this file); color blended
/// as `source alpha` times the sprite plus `behind` times what is there.
/// The window's alpha is kept as it is.
fn sprite_pipeline(behind: BlendFactor) -> PipelineParams {
    PipelineParams {
        depth_test: Comparison::LessOrEqual,
        depth_write: true,
        color_blend: Some(BlendState::new(
            Equation::Add,
            BlendFactor::Value(BlendValue::SourceAlpha),
            behind,
        )),
        alpha_blend: Some(BlendState::new(
            Equation::Add,
            BlendFactor::Zero,
            BlendFactor::One,
        )),
        ..Default::default()
    }
}

/// The directions across and up the screen, and into it, in the scene,
/// so a sprite can face the camera and sprites can be drawn far to near.
#[derive(Clone, Copy, Debug)]
pub struct Facing {
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
}

impl Facing {
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
    center: Vec3,
    across: Vec3,
    along: Vec3,
    color: Color,
    /// Lying on the floor: drawn before every upright sprite.
    flat: bool,
}

/// A batch of sprites, rebuilt every frame in reused buffers: gathered
/// with `facing`, `flat` and `push`, then put in order by `finish`.
pub struct Sprites {
    gathered: Vec<Sprite>,
    mesh: Mesh,
}

impl Sprites {
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

    pub fn clear(&mut self) {
        self.gathered.clear();
        self.mesh.vertices.clear();
        self.mesh.indices.clear();
    }

    /// Sprites gathered so far.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.gathered.len()
    }

    /// A sprite centered on `center`, spanning `across` and `along` from
    /// its middle to its edges. Ignored once the batch is full or when it
    /// would not show.
    pub fn push(&mut self, center: Vec3, across: Vec3, along: Vec3, color: Color) {
        self.gather(Sprite {
            center,
            across,
            along,
            color,
            flat: false,
        });
    }

    fn gather(&mut self, sprite: Sprite) {
        if sprite.color.a > 0.0 && self.gathered.len() < MAX_SPRITES {
            self.gathered.push(sprite);
        }
    }

    /// A sprite facing the camera, `half_width` by `half_height`.
    pub fn facing(
        &mut self,
        facing: Facing,
        center: Vec3,
        half_width: f32,
        half_height: f32,
        color: Color,
    ) {
        self.push(
            center,
            facing.right * half_width,
            facing.up * half_height,
            color,
        );
    }

    /// A round sprite lying flat, such as a glow on the floor.
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
        self.gathered.sort_by(|first, second| {
            second.flat.cmp(&first.flat).then_with(|| {
                let depth = |sprite: &Sprite| sprite.center.dot(forward);
                depth(second)
                    .partial_cmp(&depth(first))
                    .unwrap_or(Ordering::Equal)
            })
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
            for (corner, uv) in [
                (-across - along, vec2(0.0, 0.0)),
                (across - along, vec2(1.0, 0.0)),
                (across + along, vec2(1.0, 1.0)),
                (-across + along, vec2(0.0, 1.0)),
            ] {
                self.mesh
                    .vertices
                    .push(Vertex::new2(center + corner, uv, color));
            }
            self.mesh
                .indices
                .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
        }
    }

    pub fn draw(&self, material: &Material) {
        if self.mesh.indices.is_empty() {
            return;
        }
        gl_use_material(material);
        draw_mesh(&self.mesh);
        gl_use_default_material();
    }

    /// Each sprite's middle and color, for the tests.
    #[cfg(test)]
    pub fn centers(&self) -> Vec<(Vec3, Color)> {
        self.gathered
            .iter()
            .map(|sprite| (sprite.center, sprite.color))
            .collect()
    }

    #[cfg(test)]
    pub fn mesh(&self) -> &Mesh {
        &self.mesh
    }
}

/// A repeatable number in [0, 1) for `index` and `salt`, so every sprite
/// keeps its own place, size and timing from frame to frame.
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

/// How far through its life a sprite is: the clock times its rate plus
/// its own offset, wrapped to [0, 1).
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
