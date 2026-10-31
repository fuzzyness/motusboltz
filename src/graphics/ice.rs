//! The cold reservoir as a heap of ice packed around the cold half of the
//! engine block, with cold mist rolling off it.
//!
//! In the style of a Dreamcast game, the heap is glossy blue blocks and
//! crystal shards that catch the light, frosted and edged in white, with
//! sparkles twinkling on them. All follow the Cold side setting: at 300 K
//! there is none; down to 100 K the heap grows block by block and the mist
//! thickens. Like the mist off dry ice, most of it is heavier than the air
//! around it, so it sinks and spreads over the floor; a few wisps rise.
//! Drawing only: the physics knows nothing of it, and the ice stands for
//! the surroundings, not the gas.

use std::f32::consts::PI;

use macroquad::models::{
    Mesh,
    Vertex, //
};
use macroquad::prelude::{
    Camera3D,
    Color,
    Material,
    Vec2,
    Vec3,
    Vec4, //
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
    draw_lit,
    load_lit,
    mix_color,
    shade, //
};
use crate::sim::reservoir::{
    MAX_COLD_KELVIN,
    MIN_COLD_KELVIN, //
};

/// How far the heap's straight runs along the block's front and back
/// reach from the cold cylinder's axis toward the middle, as a share of
/// the distance between them.
const HEAP_REACH: f32 = 0.85;

/// One row of blocks.
///
/// Sizes are fractions of the bore's radius, so the heap keeps its
/// proportions at any engine size.
struct Row {
    /// How many blocks.
    blocks: u32,
    /// Distance out from the block's side.
    out: f32,
    /// Smallest and largest width.
    width: (f32, f32),
    /// Smallest and largest height.
    height: (f32, f32),
    /// How much the blocks narrow toward the top: a little for blocks, most
    /// of the way for shards.
    narrowing: f32,
    /// Coldness (see `cold_level`) at which the blocks start to appear,
    /// before a random share on top so they come one by one.
    appears: f32,
    /// Whether the blocks stand on the first row's.
    stacked: bool,
}

/// The rows, from the wall out.
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
/// Spread of each row's start, as a share of the coldness range.
const APPEAR_SPREAD: f32 = 0.3;
/// How much colder it gets while a block grows from nothing to full size.
const GROW_SPAN: f32 = 0.25;
/// Most each top corner moves, as a share of the block's size.
///
/// Moved corners and leaning blocks make the heap look broken rather than
/// stacked like bricks.
const CORNER_JITTER: f32 = 0.14;
/// Steepest lean of a block, in radians.
const MAX_TILT: f32 = 0.18;
/// Smallest block worth drawing, as a share of its full size.
const MIN_GROWTH: f32 = 0.02;

/// Color of the ice on top, near white.
const ICE_TOP: Color = Color::from_rgba(226, 246, 255, 255);
/// Color of the ice on the sides, bright blue.
const ICE_SIDE: Color = Color::from_rgba(118, 188, 255, 255);
/// Color of the ice low down, deep blue.
const ICE_DEEP: Color = Color::from_rgba(38, 104, 226, 255);
/// Share of light every face gets without the ice shader.
const ICE_AMBIENT: f32 = 0.6;
/// How much more light the faces turned toward the light get without the
/// ice shader.
const ICE_DIFFUSE: f32 = 0.45;
/// Opacity of the ice without the ice shader.
const ICE_OPACITY: f32 = 0.86;

/// Most sparkles on the ice.
const SPARKLES: u32 = 30;
/// How often each sparkle twinkles, per second.
const SPARKLE_RATE: f32 = 0.55;
/// Half the size of a sparkle.
const SPARKLE_HALF_SIZE: f32 = 0.075;
/// Color of a sparkle.
const SPARKLE_COLOR: Color = Color::from_rgba(214, 242, 255, 255);

/// Fragment shader for glossy ice: the face's blue in soft light, a white edge where it turns
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

/// Reach of the frost on the floor around the cold half at 100 K, in
/// heap radii from its axis.
const FROST_REACH: f32 = 1.7;
/// Color of the frost.
const FROST_COLOR: Color = Color::from_rgba(205, 230, 255, 255);
/// Opacity of the frost at 100 K.
const FROST_OPACITY: f32 = 0.3;
/// Height of the frost above the floor, clear of the floor grid.
const ABOVE_FLOOR: f32 = 0.012;

/// Most mist puffs.
const PUFFS: u32 = 150;
/// Share of the puffs shown at the first sign of cold.
const FEWEST_PUFFS: f32 = 0.25;
/// How often each puff is renewed, per second.
const PUFF_RATE: f32 = 0.11;
/// How far a sinking puff spreads over the floor.
const PUFF_SPREAD: f32 = 1.0;
/// Half the size of a new puff.
const PUFF_SIZE_NEW: f32 = 0.14;
/// Half the size of an old puff.
const PUFF_SIZE_OLD: f32 = 0.55;
/// Most opacity of a puff at the first sign of cold.
const PUFF_OPACITY_LOW: f32 = 0.08;
/// Most opacity of a puff at 100 K.
const PUFF_OPACITY_HIGH: f32 = 0.2;
/// Share of the puffs that rise.
const RISING_SHARE: f32 = 0.25;
/// How much colder it gets while each new puff fades in.
const REVEAL_SPAN: f32 = 0.08;
/// How much colder it gets while the mist first fades in.
const FIRST_SHARE: f32 = 0.08;
/// How far a rising puff rises.
const RISE: f32 = 0.45;
/// Share of its size a sinking puff settles above the floor, so it never
/// sinks through.
const SETTLE: f32 = 0.55;
/// Color of the mist.
const MIST_COLOR: Color = Color::from_rgba(205, 228, 255, 255);

/// Salt for each block's and puff's place.
const SALT_PLACE: u32 = 11;
/// Salt for each block's width.
const SALT_WIDTH: u32 = 12;
/// Salt for each block's height.
const SALT_HEIGHT: u32 = 13;
/// Salt for each block's turn.
const SALT_TURN: u32 = 14;
/// Salt for each block's lean.
const SALT_TILT: u32 = SALT_TURN + 100;
/// Salt for when each block appears.
const SALT_APPEAR: u32 = 15;
/// Salt for each puff's and sparkle's phase.
const SALT_PUFF: u32 = 16;
/// Salt for whether a puff rises, and each sparkle's rate.
const SALT_KIND: u32 = 17;
/// Salt for each block's depth.
const SALT_DEPTH: u32 = 18;
/// Salt for which block a sparkle sits on.
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
    /// Width, across the wall.
    width: f32,
    /// Depth, out from the wall.
    depth: f32,
    /// Height.
    height: f32,
    /// Turn about the vertical, in radians.
    turn: f32,
    /// Lean about its own width, in radians.
    tilt: f32,
    /// Coldness at which it starts to grow.
    appears: f32,
    /// How much it narrows toward its top.
    narrowing: f32,
    /// Picks the random shape of its top.
    seed: u32,
}

/// The heap, its mist and its sparkles.
pub struct Ice {
    /// The bore radius every size is a fraction of.
    unit: f32,
    /// Height of the floor.
    floor: f32,
    /// The block's side, which the heap rests against.
    wall: HalfOutline,
    blocks: Vec<Block>,
    /// Each block's current height, rebuilt with the mesh.
    heights: Vec<f32>,
    /// The heap, as it is now.
    solid: Mesh,
    /// The ice shader, made on the first draw (it needs a window);
    /// `Some(None)` where it cannot be built, and the faces are shaded on
    /// the CPU instead.
    material: Option<Option<Material>>,
    /// Frost and mist.
    mist: Sprites,
    /// Sparkles on the blocks.
    sparkles: Sprites,
}

impl Ice {
    /// The heap around the cold half of a block of `shape`.
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

    /// Loads the ice shader the first time.
    ///
    /// Call it before `build`, which shades the faces itself without it.
    pub fn load_shader(&mut self) {
        self.material
            .get_or_insert_with(|| load_lit(ICE_FRAGMENT, &[]));
    }

    /// Draws the heap, solid, seen from `camera` and lit from `light`.
    ///
    /// It is drawn with the engine.
    pub fn draw_solid(&self, camera: &Camera3D, light: Vec3) {
        if !self.solid.indices.is_empty() {
            let material = self.material.as_ref().and_then(Option::as_ref);
            draw_lit([&self.solid], material, camera, light);
        }
    }

    /// Draws the frost, mist and sparkles.
    pub fn draw(&self, materials: &SpriteMaterials) {
        self.mist.draw(&materials.haze);
        self.sparkles.draw(&materials.sparkle);
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
            let center = Vec3::new(spot.x, y.max(self.floor + SETTLE * half), spot.y);
            let color = with_alpha(MIST_COLOR, alpha);
            self.mist.facing(facing, center, half, half, color);
        }
        self.mist.finish(facing.forward);

        // Sparkles twinkle on the tops of the blocks that show.
        let clock = seconds as f32;
        let last = self.blocks.len() - 1;
        for sparkle in 0..SPARKLES {
            let which = (unit_hash(sparkle, SALT_SPARKLE) * self.blocks.len() as f32) as usize;
            let (block, height) = (self.blocks[which.min(last)], self.heights[which.min(last)]);
            if height <= 0.0 {
                continue;
            }
            let rate = SPARKLE_RATE * (0.7 + 0.6 * unit_hash(sparkle, SALT_KIND));
            let life = age(seconds, rate, unit_hash(sparkle, SALT_PUFF));
            let flash = (PI * life).sin().powi(4);
            let shift = 0.3 * block.width * (unit_hash(sparkle, SALT_DEPTH) - 0.5);
            let half = SPARKLE_HALF_SIZE * unit * (0.6 + 0.4 * flash);
            let spin = 0.05 * unit * (clock + sparkle as f32).sin();
            let center = Vec3::new(
                block.place.x + shift + spin,
                self.floor + height + 0.02 * unit,
                block.place.y - shift,
            );
            let color = with_alpha(SPARKLE_COLOR, flash * smoothstep(0.0, FIRST_SHARE, cold));
            self.sparkles.facing(facing, center, half, half, color);
        }
        self.sparkles.finish(facing.forward);
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
                tilt: MAX_TILT * (2.0 * random(SALT_TILT) - 1.0),
                appears: row.appears + APPEAR_SPREAD * random(SALT_APPEAR),
                narrowing: row.narrowing,
                seed: index,
            });
        }
    }
    blocks
}

/// Adds one block, grown to `growth` of its full size, with its base at
/// height `base`.
///
/// `shaded` faces carry their normals for the ice shader; otherwise they
/// are shaded here from `light`.
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
    // The corners in order around the block, seen from above.
    let around = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    let low = around.map(|(x, z)| corner(x, false, z));
    let high = around.map(|(x, z)| corner(x, true, z));
    let middle = (low.iter().chain(&high).fold(Vec3::ZERO, |sum, &p| sum + p)) / 8.0;
    let mut face = |corners: [Vec3; 4], colors: [Color; 4]| {
        let center = corners.iter().fold(Vec3::ZERO, |sum, &p| sum + p) / 4.0;
        let (side, other) = (corners[1] - corners[0], corners[3] - corners[0]);
        let mut normal = side.cross(other).normalize_or_zero();
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
        let quad = [first, first + 1, first + 2, first, first + 2, first + 3];
        mesh.indices.extend(quad);
    };
    face(high, [ICE_TOP; 4]);
    let side_top = mix_color(ICE_SIDE, ICE_TOP, 0.35);
    for edge in 0..4 {
        let next = (edge + 1) % 4;
        let corners = [low[edge], low[next], high[next], high[edge]];
        face(corners, [ICE_DEEP, ICE_DEEP, side_top, side_top]);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/ice.rs"]
mod tests;
