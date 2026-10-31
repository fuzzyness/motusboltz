//! The hot reservoir as a gas burner.
//!
//! The burner is low and U-shaped, on the floor around the hot half of the
//! engine block, with flames that follow the Hot side setting. At 300 K the burner is out; up to 1200 K its flames
//! grow taller and brighter and turn from orange toward yellow-white, the
//! way a flame or hot metal does as it heats. Drawing only: the physics
//! knows nothing of it, and the flames stand for the surroundings, not the
//! gas.

use std::f32::consts::TAU;

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
    MAX_HOT_KELVIN,
    MIN_HOT_KELVIN, //
};

/// From the block's sides out to the middle of the burner.
///
/// Sizes are fractions of the bore's radius, so the burner keeps its
/// proportions at any engine size.
const BURNER_GAP: f32 = 0.13;
/// Width of the burner.
const BURNER_WIDTH: f32 = 0.07;
/// Height of the burner.
const BURNER_HEIGHT: f32 = 0.05;
/// How far the burner's straight runs along the block's front and back
/// reach from the hot cylinder's axis toward the middle, as a share of the
/// distance between them.
const BURNER_REACH: f32 = 0.85;
/// Sections along the burner.
const BURNER_SEGMENTS: usize = 64;
/// Color of the burner.
const BURNER_COLOR: Color = Color::from_rgba(92, 96, 106, 255);
/// The burner's long faces, each between two corners of a section (see
/// `burner_mesh`), and their brightness: top, outer and inner.
const BURNER_FACES: [(usize, usize, f32); 3] = [(1, 2, 1.0), (2, 3, 0.72), (0, 1, 0.5)];
/// Brightness of the burner's ends.
const BURNER_END_SHADE: f32 = 0.6;

/// Small flame tongues along the burner.
const FLAMES: u32 = 150;
/// How often each tongue rises, per second.
const FLAME_RATE: f32 = 1.3;
/// Tallest reach of a tongue, at 1200 K.
const FLAME_HEIGHT: f32 = 0.9;
/// Share of the tallest reach a tongue keeps at the lowest setting above
/// 300 K.
const FLAME_LOWEST_SHARE: f32 = 0.25;
/// Half-width of a tongue at its root.
const FLAME_HALF_WIDTH: f32 = 0.15;
/// How much taller than wide a tongue is.
const FLAME_STRETCH: f32 = 2.4;
/// How far a tongue leans in toward the wall as it rises.
const FLAME_LEAN: f32 = 0.05;
/// Big tongues behind the small ones, fewer and slower, so the fire reads
/// as one wall of flame.
const BIG_FLAMES: u32 = 26;
/// Size of a big tongue against a small one.
const BIG_SCALE: f32 = 1.7;
/// Pace of a big tongue against a small one.
const BIG_PACE: f32 = 0.6;
/// Opacity of a big tongue against a small one.
const BIG_OPACITY: f32 = 0.45;
/// Sideways flicker along the burner.
const FLICKER: f32 = 0.08;
/// Rate of the flicker, in radians per second.
const FLICKER_RATE: f32 = 5.3;
/// Opacity of a tongue at the lowest setting.
const FLAME_OPACITY_LOW: f32 = 0.45;
/// Opacity of a tongue at 1200 K.
const FLAME_OPACITY_HIGH: f32 = 0.8;
/// Share of a tongue's life spent turning from its root color to orange.
///
/// A tongue is yellow when it leaves the burner, orange partway and red as
/// it dies; hotter settings start it whiter.
const FLAME_BODY_AGE: f32 = 0.4;
/// Root color at the lowest setting.
const ROOT_LOW: Color = Color::from_rgba(255, 170, 40, 255);
/// Root color at 1200 K.
const ROOT_HIGH: Color = Color::from_rgba(255, 228, 120, 255);
/// Color partway through a tongue's life.
const BODY: Color = Color::from_rgba(255, 110, 16, 255);
/// Color as a tongue dies.
const TIP: Color = Color::from_rgba(225, 36, 10, 255);

/// Small blue flames at the burner's ports, as on a gas burner.
const PORTS: u32 = 48;
/// Half-width of a port flame.
const PORT_HALF_WIDTH: f32 = 0.035;
/// Height of a port flame against its width.
const PORT_STRETCH: f32 = 1.4;
/// Color of a port flame.
const PORT_COLOR: Color = Color::from_rgba(60, 130, 255, 255);
/// Opacity of a port flame.
const PORT_OPACITY: f32 = 0.7;
/// Rate of a port flame's flicker, in radians per second.
const PORT_FLICKER_RATE: f32 = 9.0;
/// How far a port flame shrinks as it flickers.
const PORT_FLICKER: f32 = 0.15;

/// Sparks rising above the flames as twinkles, shown more as the heat goes
/// up.
const EMBERS: u32 = 36;
/// How often each spark rises, per second.
const EMBER_RATE: f32 = 0.45;
/// How high a spark rises, against the flames' reach.
const EMBER_RISE: f32 = 1.7;
/// Half the size of a spark.
const EMBER_HALF_SIZE: f32 = 0.04;
/// Color of a spark.
const EMBER_COLOR: Color = Color::from_rgba(255, 200, 80, 255);
/// Opacity of a spark.
const EMBER_OPACITY: f32 = 0.95;
/// How much hotter it gets while each new spark fades in.
const REVEAL_SPAN: f32 = 0.08;
/// How much hotter it gets while the flames first fade in as the burner
/// lights.
const FIRST_SHARE: f32 = 0.08;

/// Reach of the flames' light on the floor around the hot half, in burner
/// radii from its axis.
const GLOW_REACH: f32 = 1.8;
/// Color of the glow.
const GLOW_COLOR: Color = Color::from_rgba(255, 110, 30, 255);
/// Opacity of the glow at 1200 K.
const GLOW_OPACITY: f32 = 0.45;
/// Height of the glow above the floor, just above the floor grid, so the
/// two never fight for depth.
const ABOVE_FLOOR: f32 = 0.01;

/// Salt for each sprite's place along the burner.
const SALT_PLACE: u32 = 1;
/// Salt for each sprite's phase.
const SALT_PHASE: u32 = 2;
/// Salt for each tongue's reach and speed.
const SALT_REACH: u32 = 3;
/// Salt for each tongue's flicker.
const SALT_FLICKER: u32 = 4;
/// Salt for each spark's phase.
const SALT_EMBER: u32 = 5;

/// How hot the burner is, from 0 (off, at 300 K) to 1 (at 1200 K).
pub fn heat_level(hot_kelvin: f32) -> f32 {
    ((hot_kelvin - MIN_HOT_KELVIN) / (MAX_HOT_KELVIN - MIN_HOT_KELVIN)).clamp(0.0, 1.0)
}

/// The burner and its flames.
pub struct Fire {
    /// The bore radius every size is a fraction of.
    unit: f32,
    /// Height of the floor.
    floor: f32,
    /// The line the burner follows.
    path: HalfOutline,
    /// The burner itself.
    burner: Mesh,
    /// The glow on the floor and the port flames, round.
    glows: Sprites,
    /// The flame tongues.
    flames: Sprites,
    /// The sparks.
    sparks: Sprites,
}

impl Fire {
    /// The burner around the hot half of a block of `shape`.
    pub fn new(shape: BlockShape) -> Self {
        let unit = shape.bore;
        let path = HalfOutline::new(
            Half::Hot,
            shape.offset,
            shape.outer + BURNER_GAP * unit,
            BURNER_REACH,
        );
        Self {
            unit,
            floor: shape.base,
            path,
            burner: burner_mesh(path, unit, shape.base),
            glows: Sprites::new(),
            flames: Sprites::new(),
            sparks: Sprites::new(),
        }
    }

    /// The burner itself, solid; drawn with the engine.
    pub fn draw_burner(&self) {
        draw_mesh(&self.burner);
    }

    /// Lays out the flames for `heat` (see `heat_level`) at `seconds` on
    /// the clock, facing the camera.
    pub fn build(&mut self, heat: f32, seconds: f64, facing: Facing) {
        for sprites in [&mut self.glows, &mut self.flames, &mut self.sparks] {
            sprites.clear();
        }
        if heat <= 0.0 {
            return;
        }
        let unit = self.unit;
        let top = self.floor + BURNER_HEIGHT * unit;
        let clock = seconds as f32;
        // The flames fade in from nothing as the burner lights.
        let kindle = smoothstep(0.0, FIRST_SHARE, heat);
        let place = |point: Vec2, y: f32| Vec3::new(point.x, y, point.y);
        self.glows.flat(
            place(self.path.axis(), self.floor + ABOVE_FLOOR),
            GLOW_REACH * self.path.radius(),
            with_alpha(GLOW_COLOR, GLOW_OPACITY * heat),
        );

        for port in 0..PORTS {
            let (point, _) = self.path.at((port as f32 + 0.5) / PORTS as f32);
            let wave = (clock * PORT_FLICKER_RATE + port as f32 * 2.1).sin();
            let half = PORT_HALF_WIDTH * unit * (1.0 - PORT_FLICKER + PORT_FLICKER * wave);
            let color = with_alpha(PORT_COLOR, PORT_OPACITY * kindle * heat.sqrt());
            let center = place(point, top + half);
            self.glows
                .facing(facing, center, half, half * PORT_STRETCH, color);
        }

        let reach = FLAME_HEIGHT * unit * (FLAME_LOWEST_SHARE + (1.0 - FLAME_LOWEST_SHARE) * heat);
        let root = mix_color(ROOT_LOW, ROOT_HIGH, heat);
        let opacity =
            kindle * (FLAME_OPACITY_LOW + (FLAME_OPACITY_HIGH - FLAME_OPACITY_LOW) * heat);
        let width = FLAME_HALF_WIDTH * unit * (0.6 + 0.4 * heat);
        let length = self.path.length();
        // The big tongues first, then the small ones; each set gets its own
        // random numbers: (count, first index, scale, pace, opacity).
        let sets = [
            (BIG_FLAMES, 0, BIG_SCALE, BIG_PACE, BIG_OPACITY),
            (FLAMES, BIG_FLAMES, 1.0, 1.0, 1.0),
        ];
        for (count, first, scale, pace, strength) in sets {
            for slot in 0..count {
                let flame = first + slot;
                let phase = unit_hash(flame, SALT_PHASE);
                let speed = 0.8 + 0.45 * unit_hash(flame, SALT_REACH);
                let life = age(seconds, FLAME_RATE * pace * speed, phase);
                let wave = (clock * FLICKER_RATE + TAU * unit_hash(flame, SALT_FLICKER)).sin();
                let flicker = FLICKER * unit * life * wave;
                let jitter = 0.8 * (unit_hash(flame, SALT_PLACE) - 0.5);
                let fraction = (slot as f32 + 0.5 + jitter) / count as f32 + flicker / length;
                let (point, out) = self.path.at(fraction);
                let tall = scale * reach * (0.7 + 0.6 * unit_hash(flame, SALT_REACH));
                let half_width = scale * width * (1.0 - 0.45 * life);
                let half_height = half_width * FLAME_STRETCH;
                // The tongue's root rises from the burner as it lives.
                let center = place(
                    point - out * (FLAME_LEAN * unit * life),
                    top + 0.5 * tall * life + half_height * 0.8,
                );
                let color = if life < FLAME_BODY_AGE {
                    mix_color(root, BODY, life / FLAME_BODY_AGE)
                } else {
                    mix_color(BODY, TIP, (life - FLAME_BODY_AGE) / (1.0 - FLAME_BODY_AGE))
                };
                let alpha =
                    strength * opacity * smoothstep(0.0, 0.1, life) * (1.0 - life).powf(1.2);
                let color = with_alpha(color, alpha);
                self.flames
                    .facing(facing, center, half_width, half_height, color);
            }
        }

        for ember in 0..EMBERS {
            // Each spark fades in at its own heat, so more come smoothly.
            let appears = ember as f32 / EMBERS as f32;
            let presence = smoothstep(appears, appears + REVEAL_SPAN, heat);
            if presence <= 0.0 {
                break;
            }
            let life = age(seconds, EMBER_RATE, unit_hash(ember, SALT_EMBER));
            let drift = 0.04 * (clock * 2.0 + ember as f32).sin() * life;
            let (point, out) = self.path.at(unit_hash(ember, SALT_PLACE) + drift);
            let height = top + EMBER_RISE * reach * life;
            let center = place(point + out * (0.1 * unit * life), height);
            // Each spark twinkles as it rises.
            let twinkle = 0.75 + 0.25 * (clock * 11.0 + ember as f32 * 1.7).sin();
            let half = EMBER_HALF_SIZE * unit * twinkle;
            let alpha = EMBER_OPACITY * presence * smoothstep(0.0, 0.1, life) * (1.0 - life);
            let color = with_alpha(EMBER_COLOR, alpha);
            self.sparks.facing(facing, center, half, half, color);
        }
        for sprites in [&mut self.glows, &mut self.flames, &mut self.sparks] {
            sprites.finish(facing.forward);
        }
    }

    /// Draws the glow, flames and sparks, which add their light to the
    /// scene.
    pub fn draw(&self, materials: &SpriteMaterials) {
        self.glows.draw(&materials.glow);
        self.flames.draw(&materials.flame);
        self.sparks.draw(&materials.sparkle);
    }
}

/// The burner: a low bar of rectangular section along `path`, standing on
/// `floor`.
fn burner_mesh(path: HalfOutline, unit: f32, floor: f32) -> Mesh {
    let half = BURNER_WIDTH * unit / 2.0;
    let top = floor + BURNER_HEIGHT * unit;
    let mut vertices = Vec::new();
    let mut indices: Vec<u16> = Vec::new();
    let mut quad = |corners: [Vec3; 4], brightness: f32| {
        let first = vertices.len() as u16;
        let color = shade(BURNER_COLOR, brightness);
        for corner in corners {
            vertices.push(Vertex::new2(corner, Vec2::ZERO, color));
        }
        indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    };
    // A section's corners: inner low, inner top, outer top and outer low.
    let section = |step: usize| {
        let (point, out) = path.at(step as f32 / BURNER_SEGMENTS as f32);
        let (inner, outer) = (point - out * half, point + out * half);
        [
            Vec3::new(inner.x, floor, inner.y),
            Vec3::new(inner.x, top, inner.y),
            Vec3::new(outer.x, top, outer.y),
            Vec3::new(outer.x, floor, outer.y),
        ]
    };
    for step in 0..BURNER_SEGMENTS {
        let (this, next) = (section(step), section(step + 1));
        // Each face runs from one edge of this section to the same edge of
        // the next.
        for (from, to, brightness) in BURNER_FACES {
            quad([this[from], next[from], next[to], this[to]], brightness);
        }
    }
    for step in [0, BURNER_SEGMENTS] {
        quad(section(step), BURNER_END_SHADE);
    }
    Mesh {
        vertices,
        indices,
        texture: None,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/fire.rs"]
mod tests;
