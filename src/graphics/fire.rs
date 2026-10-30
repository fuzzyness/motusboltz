//! The hot reservoir as a gas burner: a low U-shaped burner on the floor
//! around the hot half of the engine block, with flames that follow the
//! Hot side setting. At 300 K the burner is out; up to 1200 K its flames
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

// Sizes are fractions of the bore's radius, so the burner keeps its
// proportions at any engine size.

/// From the block's sides out to the middle of the burner.
const BURNER_GAP: f32 = 0.13;
const BURNER_WIDTH: f32 = 0.07;
const BURNER_HEIGHT: f32 = 0.05;
/// How far the burner's straight runs along the block's front and back
/// reach from the hot cylinder's axis toward the middle, as a share of the
/// distance between them.
const BURNER_REACH: f32 = 0.85;
const BURNER_SEGMENTS: usize = 64;
const BURNER_COLOR: Color = Color::from_rgba(92, 96, 106, 255);
/// Brightness of the burner's top, outer and inner faces and its ends.
const BURNER_TOP_SHADE: f32 = 1.0;
const BURNER_OUTER_SHADE: f32 = 0.72;
const BURNER_INNER_SHADE: f32 = 0.5;
const BURNER_END_SHADE: f32 = 0.6;

/// Flame tongues: how many, how often each one rises (per second), its
/// tallest reach at 1200 K, the share of that it keeps at the lowest
/// setting above 300 K, its half-width at the base, how much taller than
/// wide it is, and how far it leans in toward the wall as it rises.
const FLAMES: u32 = 260;
const FLAME_RATE: f32 = 1.4;
const FLAME_HEIGHT: f32 = 0.95;
const FLAME_LOWEST_SHARE: f32 = 0.25;
const FLAME_HALF_WIDTH: f32 = 0.13;
const FLAME_STRETCH: f32 = 1.9;
const FLAME_LEAN: f32 = 0.06;
/// Sideways flicker along the burner, and its rate.
const FLICKER: f32 = 0.08;
const FLICKER_RATE: f32 = 5.3;
/// Opacity of a flame's base: at the lowest setting and at 1200 K.
const FLAME_OPACITY_LOW: f32 = 0.3;
const FLAME_OPACITY_HIGH: f32 = 0.55;
/// A tongue is hottest at its root and coolest at its tip: its middle
/// color is reached this far up.
const FLAME_BODY_AGE: f32 = 0.35;
/// The root's color at the lowest setting and at 1200 K, then the body's
/// and the tip's.
const ROOT_LOW: Color = Color::from_rgba(255, 140, 40, 255);
const ROOT_HIGH: Color = Color::from_rgba(255, 238, 186, 255);
const BODY: Color = Color::from_rgba(255, 108, 24, 255);
const TIP: Color = Color::from_rgba(190, 40, 10, 255);

/// Small blue flames at the burner's ports, as on a gas burner.
const PORTS: u32 = 48;
const PORT_HALF_WIDTH: f32 = 0.03;
const PORT_COLOR: Color = Color::from_rgba(70, 120, 255, 255);
const PORT_OPACITY: f32 = 0.55;
const PORT_FLICKER_RATE: f32 = 9.0;

/// Sparks rising above the flames, shown more as the heat goes up.
const EMBERS: u32 = 32;
const EMBER_RATE: f32 = 0.45;
const EMBER_RISE: f32 = 1.6;
const EMBER_HALF_SIZE: f32 = 0.014;
const EMBER_COLOR: Color = Color::from_rgba(255, 210, 120, 255);
const EMBER_OPACITY: f32 = 0.9;

/// The flames' light on the floor around the hot half, centered on its
/// axis and reaching this many burner radii out.
const GLOW_REACH: f32 = 1.8;
const GLOW_COLOR: Color = Color::from_rgba(255, 110, 30, 255);
const GLOW_OPACITY: f32 = 0.45;
/// Just above the floor grid, so the two never fight for depth.
const ABOVE_FLOOR: f32 = 0.01;

/// Salts that keep each property's random numbers apart.
const SALT_PLACE: u32 = 1;
const SALT_PHASE: u32 = 2;
const SALT_REACH: u32 = 3;
const SALT_FLICKER: u32 = 4;
const SALT_EMBER: u32 = 5;

/// How hot the burner is, from 0 (off, at 300 K) to 1 (at 1200 K).
pub fn heat_level(hot_kelvin: f32) -> f32 {
    ((hot_kelvin - MIN_HOT_KELVIN) / (MAX_HOT_KELVIN - MIN_HOT_KELVIN)).clamp(0.0, 1.0)
}

pub struct Fire {
    /// The bore radius every size is a fraction of.
    unit: f32,
    floor: f32,
    path: HalfOutline,
    burner: Mesh,
    sprites: Sprites,
}

impl Fire {
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
            sprites: Sprites::new(),
        }
    }

    /// The burner itself, solid; drawn with the engine.
    pub fn draw_burner(&self) {
        draw_mesh(&self.burner);
    }

    /// Lays out the flames for `heat` (see `heat_level`) at `seconds` on
    /// the clock, facing the camera.
    pub fn build(&mut self, heat: f32, seconds: f64, facing: Facing) {
        self.sprites.clear();
        if heat <= 0.0 {
            return;
        }
        let unit = self.unit;
        let top = self.floor + BURNER_HEIGHT * unit;
        let clock = seconds as f32;
        let place = |point: Vec2, y: f32| Vec3::new(point.x, y, point.y);
        self.sprites.flat(
            place(self.path.axis(), self.floor + ABOVE_FLOOR),
            GLOW_REACH * self.path.radius(),
            with_alpha(GLOW_COLOR, GLOW_OPACITY * heat),
        );

        for port in 0..PORTS {
            let (point, _) = self.path.at((port as f32 + 0.5) / PORTS as f32);
            let flicker = 0.85 + 0.15 * (clock * PORT_FLICKER_RATE + port as f32 * 2.1).sin();
            let half = PORT_HALF_WIDTH * unit * flicker;
            self.sprites.facing(
                facing,
                place(point, top + half),
                half,
                half * 1.4,
                with_alpha(PORT_COLOR, PORT_OPACITY * heat.sqrt()),
            );
        }

        let reach = FLAME_HEIGHT * unit * (FLAME_LOWEST_SHARE + (1.0 - FLAME_LOWEST_SHARE) * heat);
        let root = mix_color(ROOT_LOW, ROOT_HIGH, heat);
        let opacity = FLAME_OPACITY_LOW + (FLAME_OPACITY_HIGH - FLAME_OPACITY_LOW) * heat;
        let width = FLAME_HALF_WIDTH * unit * (0.6 + 0.4 * heat);
        let length = self.path.length();
        for flame in 0..FLAMES {
            let phase = unit_hash(flame, SALT_PHASE);
            let speed = 0.8 + 0.45 * unit_hash(flame, SALT_REACH);
            let life = age(seconds, FLAME_RATE * speed, phase);
            let flicker = FLICKER
                * unit
                * life
                * (clock * FLICKER_RATE + TAU * unit_hash(flame, SALT_FLICKER)).sin();
            let fraction = (flame as f32 + 0.5 + 0.8 * (unit_hash(flame, SALT_PLACE) - 0.5))
                / FLAMES as f32
                + flicker / length;
            let (point, out) = self.path.at(fraction);
            let tall = reach * (0.7 + 0.6 * unit_hash(flame, SALT_REACH));
            let half_width = width * (1.0 - 0.55 * life);
            let center = place(
                point - out * (FLAME_LEAN * unit * life),
                top + tall * life + half_width,
            );
            let color = if life < FLAME_BODY_AGE {
                mix_color(root, BODY, life / FLAME_BODY_AGE)
            } else {
                mix_color(BODY, TIP, (life - FLAME_BODY_AGE) / (1.0 - FLAME_BODY_AGE))
            };
            let alpha = opacity * smoothstep(0.0, 0.08, life) * (1.0 - life).powf(1.3);
            self.sprites.facing(
                facing,
                center,
                half_width,
                half_width * FLAME_STRETCH,
                with_alpha(color, alpha),
            );
        }

        let shown = (heat * EMBERS as f32).round() as u32;
        for ember in 0..shown {
            let life = age(seconds, EMBER_RATE, unit_hash(ember, SALT_EMBER));
            let drift = 0.04 * (clock * 2.0 + ember as f32).sin() * life;
            let (point, out) = self.path.at(unit_hash(ember, SALT_PLACE) + drift);
            let center = place(
                point + out * (0.1 * unit * life),
                top + EMBER_RISE * reach * life,
            );
            let half = EMBER_HALF_SIZE * unit;
            let alpha = EMBER_OPACITY * smoothstep(0.0, 0.1, life) * (1.0 - life);
            self.sprites
                .facing(facing, center, half, half, with_alpha(EMBER_COLOR, alpha));
        }
        self.sprites.finish(facing.forward);
    }

    pub fn sprites(&self) -> &Sprites {
        &self.sprites
    }
}

/// The burner: a low bar of rectangular section along the path.
fn burner_mesh(path: HalfOutline, unit: f32, floor: f32) -> Mesh {
    let half = BURNER_WIDTH * unit / 2.0;
    let top = floor + BURNER_HEIGHT * unit;
    let mut vertices = Vec::new();
    let mut indices: Vec<u16> = Vec::new();
    let mut quad = |corners: [Vec3; 4], brightness: f32| {
        let first = vertices.len() as u16;
        for corner in corners {
            vertices.push(Vertex::new2(
                corner,
                Vec2::ZERO,
                shade(BURNER_COLOR, brightness),
            ));
        }
        indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    };
    let section = |step: usize| {
        let (point, out) = path.at(step as f32 / BURNER_SEGMENTS as f32);
        let inner = point - out * half;
        let outer = point + out * half;
        [
            Vec3::new(inner.x, floor, inner.y),
            Vec3::new(inner.x, top, inner.y),
            Vec3::new(outer.x, top, outer.y),
            Vec3::new(outer.x, floor, outer.y),
        ]
    };
    for step in 0..BURNER_SEGMENTS {
        let [inner_low, inner_top, outer_top, outer_low] = section(step);
        let [
            next_inner_low,
            next_inner_top,
            next_outer_top,
            next_outer_low,
        ] = section(step + 1);
        quad(
            [inner_top, next_inner_top, next_outer_top, outer_top],
            BURNER_TOP_SHADE,
        );
        quad(
            [outer_low, outer_top, next_outer_top, next_outer_low],
            BURNER_OUTER_SHADE,
        );
        quad(
            [inner_low, next_inner_low, next_inner_top, inner_top],
            BURNER_INNER_SHADE,
        );
    }
    for step in [0, BURNER_SEGMENTS] {
        let [inner_low, inner_top, outer_top, outer_low] = section(step);
        quad(
            [inner_low, inner_top, outer_top, outer_low],
            BURNER_END_SHADE,
        );
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
