//! The cold reservoir as a heap of ice packed around the cold half of the
//! engine block, with cold mist rolling off it. Both follow the Cold side setting:
//! at 300 K there is none; down to 100 K the heap grows block by block and
//! the mist thickens. Like the mist off dry ice, most of it is heavier
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
    Color,
    Vec2,
    Vec3, //
};

use super::block::BlockShape;
use super::outline::{
    Half,
    HalfOutline, //
};
use super::sprites::{
    Facing,
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
/// Rows of blocks: how many, their distance out from the block's side, the smallest and largest width and height, and the coldness
/// (see `cold_level`) at which they start to appear, plus a random share
/// on top so they come one by one.
struct Row {
    blocks: u32,
    out: f32,
    width: (f32, f32),
    height: (f32, f32),
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
        appears: 0.0,
        stacked: false,
    },
    // A lower row outside it.
    Row {
        blocks: 18,
        out: 0.3,
        width: (0.15, 0.22),
        height: (0.12, 0.19),
        appears: 0.25,
        stacked: false,
    },
    // Blocks piled on the first row, last to come.
    Row {
        blocks: 11,
        out: 0.0,
        width: (0.13, 0.19),
        height: (0.11, 0.16),
        appears: 0.5,
        stacked: true,
    },
];
/// Spread of each row's start (as a share of the coldness range) and how
/// much colder it gets while a block grows from nothing to full size.
const APPEAR_SPREAD: f32 = 0.3;
const GROW_SPAN: f32 = 0.25;
/// Blocks narrow toward the top by this share, each top corner moves by
/// up to this share of the block's size, and blocks lean up to this
/// angle, so the heap looks broken rather than stacked like bricks.
const TOP_NARROWING: f32 = 0.25;
const CORNER_JITTER: f32 = 0.14;
const MAX_TILT: f32 = 0.18;
/// Smallest block worth drawing, as a share of its full size.
const MIN_GROWTH: f32 = 0.02;
const FACES_PER_BLOCK: usize = 5;

/// Pale on top, blue on the sides, deeper blue low down, slightly see
/// through.
const ICE_TOP: Color = Color::from_rgba(206, 234, 252, 255);
const ICE_SIDE: Color = Color::from_rgba(136, 190, 234, 255);
const ICE_DEEP: Color = Color::from_rgba(70, 128, 198, 255);
const ICE_OPACITY: f32 = 0.86;
/// Shading: the share of light every face gets, and how much more the
/// faces turned toward the light get.
const ICE_AMBIENT: f32 = 0.6;
const ICE_DIFFUSE: f32 = 0.45;

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
const RISE: f32 = 0.45;
/// A sinking puff settles with this share of its size above the floor, so
/// it never sinks through.
const SETTLE: f32 = 0.55;
const MIST_COLOR: Color = Color::from_rgba(214, 232, 250, 255);

const SALT_PLACE: u32 = 11;
const SALT_WIDTH: u32 = 12;
const SALT_HEIGHT: u32 = 13;
const SALT_TURN: u32 = 14;
const SALT_APPEAR: u32 = 15;
const SALT_PUFF: u32 = 16;
const SALT_KIND: u32 = 17;
const SALT_DEPTH: u32 = 18;

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
    sprites: Sprites,
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
            sprites: Sprites::new(),
        }
    }

    /// The heap, solid; drawn with the engine.
    pub fn draw_solid(&self) {
        if !self.solid.indices.is_empty() {
            draw_mesh(&self.solid);
        }
    }

    pub fn sprites(&self) -> &Sprites {
        &self.sprites
    }

    /// Builds the heap and the mist for `cold` (see `cold_level`) at
    /// `seconds` on the clock, lit from `light` and facing the camera.
    pub fn build(&mut self, cold: f32, seconds: f64, light: Vec3, facing: Facing) {
        self.solid.vertices.clear();
        self.solid.indices.clear();
        self.sprites.clear();
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
            add_block(&mut self.solid, block, base, growth, light);
        }

        let unit = self.unit;
        let axis = self.wall.axis();
        self.sprites.flat(
            Vec3::new(axis.x, self.floor + ABOVE_FLOOR, axis.y),
            FROST_REACH * self.wall.radius() * (0.6 + 0.4 * cold),
            with_alpha(FROST_COLOR, FROST_OPACITY * cold),
        );

        let heap_top = self.heights.iter().copied().fold(0.0, f32::max);
        let shown = (PUFFS as f32 * (FEWEST_PUFFS + (1.0 - FEWEST_PUFFS) * cold)).ceil() as u32;
        let peak = PUFF_OPACITY_LOW + (PUFF_OPACITY_HIGH - PUFF_OPACITY_LOW) * cold;
        for puff in 0..shown.min(PUFFS) {
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
            let alpha = peak * (PI * life).sin();
            let spot = point + outward * out;
            self.sprites.facing(
                facing,
                Vec3::new(spot.x, y.max(self.floor + SETTLE * half), spot.y),
                half,
                half,
                with_alpha(MIST_COLOR, alpha),
            );
        }
        self.sprites.finish(facing.forward);
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
                seed: index,
            });
        }
    }
    blocks
}

/// Adds one block, grown to `growth` of its full size, with its base at
/// height `base`.
fn add_block(mesh: &mut Mesh, block: Block, base: f32, growth: f32, light: Vec3) {
    let half_width = block.width * growth / 2.0;
    let half_depth = block.depth * growth / 2.0;
    let height = block.height * growth;
    let narrow = 1.0 - TOP_NARROWING;
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
            let lit = shade(color, brightness.min(1.0));
            mesh.vertices.push(Vertex::new2(
                position,
                Vec2::ZERO,
                with_alpha(lit, ICE_OPACITY),
            ));
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

#[cfg(test)]
#[path = "../../tests/unit/graphics/ice.rs"]
mod tests;
