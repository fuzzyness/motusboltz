//! The cold reservoir as a heap of ice packed around the cold half of the
//! engine block, with cold mist rolling off it, in the style of a
//! Dreamcast game: glossy blue blocks and crystal shards that catch the
//! light, frosted and edged in white, with sparkles twinkling on them.
//! All follow the Cold side setting: at 300 K there is none; down to
//! 100 K the heap grows block by block and the mist thickens. Like the mist off dry ice, most of it is heavier
//! than the air around it, so it sinks and spreads over the floor; a few
//! wisps rise. Drawing only: the physics knows nothing of it, and the ice
//! stands for the surroundings, not the gas.

use std::f32::consts::PI;

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

use super::block::BlockShape;
use super::outline::{
    Half,
    HalfOutline, //
};
use super::sprites::{
    Facing,
    SpriteMaterials,
    Sprites,
    age,
    smoothstep,
    unit_hash,
    with_alpha, //
};
use super::{
    mix_color,
    shade, //
};
use crate::sim::reservoir::{
    MAX_COLD_KELVIN,
    MIN_COLD_KELVIN, //
};

// Sizes are fractions of the bore's radius, so the heap keeps its
// proportions at any engine size.

/// How far the heap's straight runs along the block's front and back
/// reach from the cold cylinder's axis toward the middle, as a share of
/// the distance between them.
const HEAP_REACH: f32 = 0.85;
/// Rows of blocks: how many, their distance out from the block's side,
/// the smallest and largest width and height, how much they narrow toward
/// the top (a little for blocks, most of the way for shards), and the
/// coldness (see `cold_level`) at which they start to appear, plus a
/// random share on top so they come one by one.
struct Row {
    blocks: u32,
    out: f32,
    width: (f32, f32),
    height: (f32, f32),
    narrowing: f32,
    appears: f32,
    stacked: bool,
}

const ROWS: [Row; 3] = [
    // Against the wall: the first to form, and the largest.
    Row {
        blocks: 24,
        out: 0.0,
        width: (0.22, 0.32),
        height: (0.2, 0.3),
        narrowing: 0.25,
        appears: 0.0,
        stacked: false,
    },
    // A lower row outside it.
    Row {
        blocks: 18,
        out: 0.3,
        width: (0.15, 0.22),
        height: (0.12, 0.19),
        narrowing: 0.3,
        appears: 0.25,
        stacked: false,
    },
    // Crystal shards standing on the first row, last to come.
    Row {
        blocks: 14,
        out: 0.0,
        width: (0.1, 0.15),
        height: (0.14, 0.22),
        narrowing: 0.8,
        appears: 0.5,
        stacked: true,
    },
];
/// Spread of each row's start (as a share of the coldness range) and how
/// much colder it gets while a block grows from nothing to full size.
const APPEAR_SPREAD: f32 = 0.3;
const GROW_SPAN: f32 = 0.25;
/// Each top corner moves by up to this share of the block's size, and
/// blocks lean up to this angle, so the heap looks broken rather than
/// stacked like bricks.
const CORNER_JITTER: f32 = 0.14;
const MAX_TILT: f32 = 0.18;
/// Smallest block worth drawing, as a share of its full size.
const MIN_GROWTH: f32 = 0.02;
const FACES_PER_BLOCK: usize = 5;

/// Near white on top, bright blue on the sides, deep blue low down.
const ICE_TOP: Color = Color::from_rgba(226, 246, 255, 255);
const ICE_SIDE: Color = Color::from_rgba(118, 188, 255, 255);
const ICE_DEEP: Color = Color::from_rgba(38, 104, 226, 255);
/// Without the ice shader: the share of light every face gets, and how
/// much more the faces turned toward the light get, and the opacity.
const ICE_AMBIENT: f32 = 0.6;
const ICE_DIFFUSE: f32 = 0.45;
const ICE_OPACITY: f32 = 0.86;

/// Sparkles on the ice: at most how many, how often each twinkles (per
/// second), half their size, and their color.
const SPARKLES: u32 = 30;
const SPARKLE_RATE: f32 = 0.55;
const SPARKLE_HALF_SIZE: f32 = 0.075;
const SPARKLE_COLOR: Color = Color::from_rgba(214, 242, 255, 255);

const ICE_VERTEX: &str = r#"#version 100
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

/// Glossy ice: the face's blue in soft light, a white edge where it turns
/// away from the eye (as light skims the surface of ice), a tight
/// highlight, and a speckle of frost.
const ICE_FRAGMENT: &str = r#"#version 100
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

void main() {
    vec3 n = normalize(surface_normal);
    vec3 toward_eye = normalize(Eye - world);
    if (dot(n, toward_eye) < 0.0) {
        n = -n;
    }
    float diffuse = max(dot(n, Light), 0.0);
    float edge = pow(1.0 - max(dot(n, toward_eye), 0.0), 2.0);
    float shine = pow(max(dot(n, normalize(Light + toward_eye)), 0.0), 40.0);
    float frost = hash(floor((world.xz + world.y) * 18.0));
    vec3 lit = color.rgb * (0.55 + 0.5 * diffuse)
        + vec3(0.72, 0.9, 1.0) * edge * 0.55
        + vec3(0.9 * shine)
        + vec3(0.06) * frost;
    gl_FragColor = vec4(lit, 1.0) * texture2D(Texture, uv);
}"#;

/// Frost on the floor around the cold half, centered on its axis and
/// reaching this many times the heap's radius out at 100 K.
const FROST_REACH: f32 = 1.7;
const FROST_COLOR: Color = Color::from_rgba(205, 230, 255, 255);
const FROST_OPACITY: f32 = 0.3;
const ABOVE_FLOOR: f32 = 0.012;

/// Mist puffs: at most how many, the share of them shown at the first
/// sign of cold, how often each one is renewed (per second), how far it
/// spreads, its half-size when new and when old, its opacity at most at
/// the first sign of cold and at 100 K, and the share of puffs that rise.
const PUFFS: u32 = 150;
const FEWEST_PUFFS: f32 = 0.25;
const PUFF_RATE: f32 = 0.11;
const PUFF_SPREAD: f32 = 1.0;
const PUFF_SIZE_NEW: f32 = 0.14;
const PUFF_SIZE_OLD: f32 = 0.55;
const PUFF_OPACITY_LOW: f32 = 0.08;
const PUFF_OPACITY_HIGH: f32 = 0.2;
const RISING_SHARE: f32 = 0.25;
/// How much colder it gets while each new puff fades in, and while the
/// mist first fades in.
const REVEAL_SPAN: f32 = 0.08;
const FIRST_SHARE: f32 = 0.08;
const RISE: f32 = 0.45;
/// A sinking puff settles with this share of its size above the floor, so
/// it never sinks through.
const SETTLE: f32 = 0.55;
const MIST_COLOR: Color = Color::from_rgba(205, 228, 255, 255);

const SALT_PLACE: u32 = 11;
const SALT_WIDTH: u32 = 12;
const SALT_HEIGHT: u32 = 13;
const SALT_TURN: u32 = 14;
const SALT_APPEAR: u32 = 15;
const SALT_PUFF: u32 = 16;
const SALT_KIND: u32 = 17;
const SALT_DEPTH: u32 = 18;
const SALT_SPARKLE: u32 = 19;

/// How cold the ice is, from 0 (none, at 300 K) to 1 (at 100 K).
pub fn cold_level(cold_kelvin: f32) -> f32 {
    ((MAX_COLD_KELVIN - cold_kelvin) / (MAX_COLD_KELVIN - MIN_COLD_KELVIN)).clamp(0.0, 1.0)
}

/// One block of ice at full size.
#[derive(Clone, Copy, Debug)]
struct Block {
    /// Middle of its base, seen from above.
    place: Vec2,
    /// The block it rests on, if piled; it rests on the floor otherwise.
    on: Option<usize>,
    width: f32,
    depth: f32,
    height: f32,
    turn: f32,
    tilt: f32,
    /// Coldness at which it starts to grow.
    appears: f32,
    /// How much it narrows toward its top.
    narrowing: f32,
    /// Picks the random shape of its top.
    seed: u32,
}

pub struct Ice {
    unit: f32,
    floor: f32,
    /// The block's side, which the heap rests against.
    wall: HalfOutline,
    blocks: Vec<Block>,
    /// Each block's current height, rebuilt with the mesh.
    heights: Vec<f32>,
    solid: Mesh,
    /// The ice shader, made on the first draw (it needs a window);
    /// `Some(None)` where it cannot be built, and the faces are shaded on
    /// the CPU instead.
    material: Option<Option<Material>>,
    /// Frost and mist.
    mist: Sprites,
    sparkles: Sprites,
}

impl Ice {
    pub fn new(shape: BlockShape) -> Self {
        let unit = shape.bore;
        let wall = HalfOutline::new(Half::Cold, shape.offset, shape.outer, HEAP_REACH);
        let blocks = layout(wall, unit);
        Self {
            unit,
            floor: shape.base,
            wall,
            heights: vec![0.0; blocks.len()],
            blocks,
            solid: Mesh {
                vertices: Vec::new(),
                indices: Vec::new(),
                texture: None,
            },
            material: None,
            mist: Sprites::new(),
            sparkles: Sprites::new(),
        }
    }

    /// Whether the ice shader is ready, loading it the first time. Call
    /// before `build`, which shades the faces itself without it.
    pub fn prepare_shader(&mut self) -> bool {
        self.material.get_or_insert_with(load_ice).is_some()
    }

    /// The heap, solid, seen from `camera` and lit from `light`; drawn
    /// with the engine.
    pub fn draw_solid(&self, camera: &Camera3D, light: Vec3) {
        if self.solid.indices.is_empty() {
            return;
        }
        if let Some(Some(material)) = &self.material {
            material.set_uniform("Eye", camera.position);
            material.set_uniform("Light", light);
            gl_use_material(material);
        }
        draw_mesh(&self.solid);
        gl_use_default_material();
    }

    /// Draws the frost, mist and sparkles.
    pub fn draw(&self, materials: &SpriteMaterials) {
        self.mist.draw(&materials.haze);
        self.sparkles.draw(&materials.sparkle);
    }

    #[cfg(test)]
    fn mist(&self) -> &Sprites {
        &self.mist
    }

    /// Builds the heap and the mist for `cold` (see `cold_level`) at
    /// `seconds` on the clock, lit from `light` and facing the camera.
    pub fn build(&mut self, cold: f32, seconds: f64, light: Vec3, facing: Facing) {
        self.solid.vertices.clear();
        self.solid.indices.clear();
        self.mist.clear();
        self.sparkles.clear();
        if cold <= 0.0 {
            self.heights.fill(0.0);
            return;
        }
        for index in 0..self.blocks.len() {
            let block = self.blocks[index];
            let growth = smoothstep(block.appears, block.appears + GROW_SPAN, cold);
            let base = match block.on {
                Some(below) => self.floor + self.heights[below],
                None => self.floor,
            };
            if growth < MIN_GROWTH || block.on.is_some_and(|below| self.heights[below] <= 0.0) {
                self.heights[index] = 0.0;
                continue;
            }
            self.heights[index] = base - self.floor + block.height * growth;
            let shaded = matches!(self.material, Some(Some(_)));
            add_block(&mut self.solid, block, base, growth, light, shaded);
        }

        let unit = self.unit;
        let axis = self.wall.axis();
        self.mist.flat(
            Vec3::new(axis.x, self.floor + ABOVE_FLOOR, axis.y),
            FROST_REACH * self.wall.radius() * (0.6 + 0.4 * cold),
            with_alpha(FROST_COLOR, FROST_OPACITY * cold),
        );

        let heap_top = self.heights.iter().copied().fold(0.0, f32::max);

        // The mist fades in from nothing at the first sign of cold.
        let peak = smoothstep(0.0, FIRST_SHARE, cold)
            * (PUFF_OPACITY_LOW + (PUFF_OPACITY_HIGH - PUFF_OPACITY_LOW) * cold);
        for puff in 0..PUFFS {
            // The first share of puffs shows at the first sign of cold; each
            // of the rest fades in at its own coldness, so the mist
            // thickens smoothly.
            let appears = (puff as f32 / PUFFS as f32 - FEWEST_PUFFS) / (1.0 - FEWEST_PUFFS);
            let presence = smoothstep(appears, appears + REVEAL_SPAN, cold);
            if presence <= 0.0 {
                break;
            }
            let life = age(seconds, PUFF_RATE, unit_hash(puff, SALT_PUFF));
            let (point, outward) = self.wall.at(unit_hash(puff, SALT_PLACE));
            let start_out = 0.1 * unit * (1.0 + 2.0 * unit_hash(puff, SALT_DEPTH));
            let start_y = self.floor + heap_top * (0.3 + 0.7 * unit_hash(puff, SALT_HEIGHT));
            let half = unit
                * (PUFF_SIZE_NEW + (PUFF_SIZE_OLD - PUFF_SIZE_NEW) * life)
                * (0.7 + 0.3 * cold);
            let rising = unit_hash(puff, SALT_KIND) < RISING_SHARE;
            let (out, y) = if rising {
                (start_out + 0.3 * unit * life, start_y + RISE * unit * life)
            } else {
                let settled = self.floor + SETTLE * half;
                let spread = PUFF_SPREAD * unit * (0.6 + 0.8 * unit_hash(puff, SALT_WIDTH));
                (
                    start_out + spread * life,
                    start_y + (settled - start_y) * smoothstep(0.0, 0.45, life),
                )
            };
            let alpha = peak * presence * (PI * life).sin();
            let spot = point + outward * out;
            self.mist.facing(
                facing,
                Vec3::new(spot.x, y.max(self.floor + SETTLE * half), spot.y),
                half,
                half,
                with_alpha(MIST_COLOR, alpha),
            );
        }
        self.mist.finish(facing.forward);

        // Sparkles twinkle on the tops of the blocks that show.
        let clock = seconds as f32;
        for sparkle in 0..SPARKLES {
            let which = (unit_hash(sparkle, SALT_SPARKLE) * self.blocks.len() as f32) as usize;
            let height = self.heights[which.min(self.blocks.len() - 1)];
            if height <= 0.0 {
                continue;
            }
            let block = self.blocks[which.min(self.blocks.len() - 1)];
            let life = age(
                seconds,
                SPARKLE_RATE * (0.7 + 0.6 * unit_hash(sparkle, SALT_KIND)),
                unit_hash(sparkle, SALT_PUFF),
            );
            let flash = (PI * life).sin().powi(4);
            let shift = 0.3 * block.width * (unit_hash(sparkle, SALT_DEPTH) - 0.5);
            let half = SPARKLE_HALF_SIZE * unit * (0.6 + 0.4 * flash);
            let spin = 0.05 * unit * (clock + sparkle as f32).sin();
            self.sparkles.facing(
                facing,
                Vec3::new(
                    block.place.x + shift + spin,
                    self.floor + height + 0.02 * unit,
                    block.place.y - shift,
                ),
                half,
                half,
                with_alpha(SPARKLE_COLOR, flash * smoothstep(0.0, FIRST_SHARE, cold)),
            );
        }
        self.sparkles.finish(facing.forward);
    }

    #[cfg(test)]
    fn solid(&self) -> &Mesh {
        &self.solid
    }
}

/// Places every block of the heap at full size, row by row along `wall`.
fn layout(wall: HalfOutline, unit: f32) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut first_row = Vec::new();
    let mut index = 0_u32;
    for (row_index, row) in ROWS.iter().enumerate() {
        for slot in 0..row.blocks {
            index += 1;
            let random = |salt| unit_hash(index, salt);
            let span = |(low, high): (f32, f32), salt| unit * (low + (high - low) * random(salt));
            let width = span(row.width, SALT_WIDTH);
            let depth = span(row.width, SALT_DEPTH);
            let height = span(row.height, SALT_HEIGHT);
            let along = (slot as f32 + 0.5 + 0.5 * (random(SALT_PLACE) - 0.5)) / row.blocks as f32;
            let (point, out) = wall.at(along);
            let angle = out.y.atan2(out.x);
            let (on, place) = if row.stacked {
                let below: usize = first_row[(along * first_row.len() as f32) as usize];
                let under: Block = blocks[below];
                (Some(below), under.place)
            } else {
                // Half the diagonal, so no turn pushes a corner into the wall.
                let distance = row.out * unit + width.hypot(depth) / 2.0;
                (None, point + out * distance)
            };
            if row_index == 0 {
                first_row.push(blocks.len());
            }
            blocks.push(Block {
                place,
                on,
                width,
                depth,
                height,
                turn: angle + 0.5 * (random(SALT_TURN) - 0.5),
                tilt: MAX_TILT * (2.0 * random(SALT_TURN + 100) - 1.0),
                appears: row.appears + APPEAR_SPREAD * random(SALT_APPEAR),
                narrowing: row.narrowing,
                seed: index,
            });
        }
    }
    blocks
}

/// Adds one block, grown to `growth` of its full size, with its base at
/// height `base`. `shaded` faces carry their normals for the ice shader;
/// otherwise they are shaded here from `light`.
fn add_block(mesh: &mut Mesh, block: Block, base: f32, growth: f32, light: Vec3, shaded: bool) {
    let half_width = block.width * growth / 2.0;
    let half_depth = block.depth * growth / 2.0;
    let height = block.height * growth;
    let narrow = 1.0 - block.narrowing;
    let (sin_turn, cos_turn) = block.turn.sin_cos();
    let (sin_tilt, cos_tilt) = block.tilt.sin_cos();
    let place = |x: f32, y: f32, z: f32| {
        // Lean about the block's own width, then turn it about the vertical.
        let (y, z) = (y * cos_tilt - z * sin_tilt, y * sin_tilt + z * cos_tilt);
        Vec3::new(
            block.place.x + x * cos_turn - z * sin_turn,
            base + y.max(0.0),
            block.place.y + x * sin_turn + z * cos_turn,
        )
    };
    let corner = |sign_x: f32, top: bool, sign_z: f32| {
        if !top {
            return place(sign_x * half_width, 0.0, sign_z * half_depth);
        }
        // Each top corner moves its own way, the same every frame.
        let which = block.seed * 8 + (2.0 * sign_x + sign_z + 3.0) as u32;
        let jitter = |salt| CORNER_JITTER * (2.0 * unit_hash(which, salt) - 1.0);
        place(
            sign_x * half_width * (narrow + jitter(SALT_WIDTH)),
            height * (1.0 + jitter(SALT_HEIGHT)),
            sign_z * half_depth * (narrow + jitter(SALT_DEPTH)),
        )
    };
    let low = [
        corner(-1.0, false, -1.0),
        corner(1.0, false, -1.0),
        corner(1.0, false, 1.0),
        corner(-1.0, false, 1.0),
    ];
    let high = [
        corner(-1.0, true, -1.0),
        corner(1.0, true, -1.0),
        corner(1.0, true, 1.0),
        corner(-1.0, true, 1.0),
    ];
    let middle = (low.iter().chain(&high).fold(Vec3::ZERO, |sum, &p| sum + p)) / 8.0;
    let mut face = |corners: [Vec3; 4], colors: [Color; 4]| {
        let center = corners.iter().fold(Vec3::ZERO, |sum, &p| sum + p) / 4.0;
        let mut normal = (corners[1] - corners[0])
            .cross(corners[3] - corners[0])
            .normalize_or_zero();
        if normal.dot(center - middle) < 0.0 {
            normal = -normal;
        }
        let brightness = ICE_AMBIENT + ICE_DIFFUSE * normal.dot(light).max(0.0);
        let first = mesh.vertices.len() as u16;
        for (position, color) in corners.into_iter().zip(colors) {
            let lit = if shaded {
                color
            } else {
                with_alpha(shade(color, brightness.min(1.0)), ICE_OPACITY)
            };
            let mut vertex = Vertex::new2(position, Vec2::ZERO, lit);
            vertex.normal = Vec4::new(normal.x, normal.y, normal.z, 0.0);
            mesh.vertices.push(vertex);
        }
        mesh.indices
            .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    };
    face(high, [ICE_TOP; 4]);
    let side_top = mix_color(ICE_SIDE, ICE_TOP, 0.35);
    for edge in 0..4 {
        let next = (edge + 1) % 4;
        face(
            [low[edge], low[next], high[next], high[edge]],
            [ICE_DEEP, ICE_DEEP, side_top, side_top],
        );
    }
    debug_assert_eq!(mesh.vertices.len() % (4 * FACES_PER_BLOCK), 0);
}

fn load_ice() -> Option<Material> {
    load_material(
        ShaderSource::Glsl {
            vertex: ICE_VERTEX,
            fragment: ICE_FRAGMENT,
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

#[cfg(test)]
#[path = "../../tests/unit/graphics/ice.rs"]
mod tests;
