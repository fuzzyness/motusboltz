//! The thermometers on the cylinders, for the second and third laws.
//!
//! A round dial thermometer stands on the block's top over the front of
//! each cylinder, its stem screwed into the head through a brass fitting,
//! its face toward the front: red-rimmed over the hot cylinder, blue over
//! the cold, each reading its wall's temperature in kelvin. The gap between
//! the two is what drives the engine, heat flowing from the hot side to
//! the cold (the second law); with no gap, the flywheel stops.
//!
//! The cold dial's scale runs down to absolute zero, but its needle stops
//! at a pin at 77 K, liquid nitrogen's boiling point, the coldest the
//! nitrogen can make the wall; the band below it is dark, out of reach,
//! as absolute zero is for anything (the third law).
//!
//! Drawing only: the readings are the walls' temperatures.

use std::f32::consts::PI;

use macroquad::models::Mesh;
use macroquad::prelude::{
    Camera3D,
    Color,
    Mat4,
    Vec2,
    Vec3,
    vec2,
    vec3, //
};

use super::block::{
    BlockShape,
    key_light, //
};
use super::clock::{
    NUMERAL_COLUMNS,
    NUMERAL_ROWS,
    digit, //
};
use super::gpu::{
    GpuMaterial,
    GpuMesh, //
};
use super::solid::{
    Frame,
    Solid,
    finish,
    load_metal,
    turn, //
};
use super::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY,
    draw_lit_placed, //
};
use crate::sim::reservoir::{
    MIN_COLD_KELVIN,
    ReservoirSide, //
};

/// How far each dial stands from the block's middle, in bore radii: over
/// the front of its cylinder, clear of the flywheel's stands.
const DIAL_ACROSS: f32 = 0.65;
/// How far toward the front each stem enters the block's top, in bore
/// radii: in the steel between the bore and the front face.
const DIAL_FORWARD: f32 = 1.0;
/// Radius of a dial's face, in bore radii.
const FACE_RADIUS: f32 = 0.27;
/// Width of the rim round a face, in bore radii.
const RIM_WIDTH: f32 = 0.045;
/// Depth of a dial's case, front to back, in bore radii.
const CASE_DEPTH: f32 = 0.08;
/// Height of the stem between the block's top and the case, in bore radii.
const STEM_HEIGHT: f32 = 0.12;
/// Radius of the stem, in bore radii.
const STEM_RADIUS: f32 = 0.025;
/// Radius of the brass fitting the stem is screwed through, in bore
/// radii.
const FITTING_RADIUS: f32 = 0.055;
/// Height of the fitting, in bore radii.
const FITTING_HEIGHT: f32 = 0.035;
/// Angle the scale sweeps round the face, in radians: three quarters of a
/// turn, from bottom left to bottom right.
const SWEEP: f32 = 1.5 * PI;
/// Inner end of the long ticks, as a share of the face's radius.
const MAJOR_TICK_INNER: f32 = 0.74;
/// Inner end of the short ticks, as a share of the face's radius.
const MINOR_TICK_INNER: f32 = 0.82;
/// Outer end of the ticks, as a share of the face's radius.
const TICK_OUTER: f32 = 0.92;
/// Half the width of a long tick, as a share of the face's radius.
const MAJOR_TICK_HALF: f32 = 0.022;
/// Half the width of a short tick, as a share of the face's radius.
const MINOR_TICK_HALF: f32 = 0.011;
/// Where the numbers stand, as a share of the face's radius.
const NUMBER_RING: f32 = 0.58;
/// Height of a number, as a share of the face's radius.
const NUMBER_HEIGHT: f32 = 0.12;
/// Room between a number's digits, in the digits' grid.
const NUMBER_GAP: f32 = 1.0;
/// Half the width of a digit's stroke, in the digits' grid.
const NUMBER_STROKE: f32 = 0.6;
/// Where the unit, K, stands below the middle, as a share of the face's
/// radius: in the gap between the scale's ends, below their numbers.
const UNIT_DROP: f32 = 0.66;
/// Height of the unit's letter, as a share of the face's radius.
const UNIT_HEIGHT: f32 = 0.14;
/// The out-of-reach band on the cold dial, from and to, as shares of the
/// face's radius.
const BAND_RADII: (f32, f32) = (0.74, 0.92);
/// Where the stop pin stands, as a share of the face's radius.
const STOP_RING: f32 = 0.68;
/// Radius of the stop pin, as a share of the face's radius.
const STOP_RADIUS: f32 = 0.04;
/// How far the stop pin stands off the face, in bore radii.
const STOP_HEIGHT: f32 = 0.03;
/// How far marks stand off the face, in bore radii.
const MARK_RAISE: f32 = 0.004;
/// How far the needle stands off the face, in bore radii.
const NEEDLE_RAISE: f32 = 0.045;
/// Length of the needle, as a share of the face's radius.
const NEEDLE_LENGTH: f32 = 0.84;
/// Length of the needle's tail, as a share of the face's radius.
const NEEDLE_TAIL: f32 = 0.2;
/// Half the needle's width at its root, as a share of the face's radius.
const NEEDLE_HALF: f32 = 0.045;
/// Radius of the cap over the needle's pin, as a share of the face's
/// radius.
const CAP_RADIUS: f32 = 0.09;
/// Height of the cap, in bore radii.
const CAP_HEIGHT: f32 = 0.02;
/// Steps round each arc of the band.
const BAND_STEPS: usize = 12;
/// Segments round the round parts.
const SEGMENTS: usize = 32;
/// Segments round the small round parts.
const SMALL_SEGMENTS: usize = 12;
/// Seconds for a needle to make up most of the way to a new reading.
const NEEDLE_SECONDS: f32 = 0.2;
/// The faces' enamel.
const FACE_ENAMEL: Color = finish(248, 244, 230, 0.25);
/// The ticks, numbers and unit.
const MARK_INK: Color = finish(32, 40, 72, 0.1);
/// The out-of-reach band.
const BAND_SHADE: Color = finish(92, 96, 108, 0.1);
/// The stop pin.
const STOP_RED: Color = finish(216, 40, 36, 0.6);
/// The hot dial's rim and needle.
const HOT_PAINT: Color = finish(214, 54, 40, 0.6);
/// The cold dial's rim and needle.
const COLD_PAINT: Color = finish(40, 96, 214, 0.6);
/// The cases' backs and sides.
const CASE_STEEL: Color = finish(70, 74, 84, 0.4);
/// The stems, fittings and caps.
const BRASS: Color = finish(206, 164, 72, 0.8);

/// The scale of `side`'s thermometer: its range in kelvin, from absolute
/// zero, how many steps the short ticks divide it into, every how many of
/// them a long tick and number stand, and where its needle stops.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scale {
    /// The temperature at the scale's far end.
    pub top: f32,
    /// Steps between short ticks across the scale.
    pub steps: usize,
    /// Steps between long ticks.
    pub major: usize,
    /// The least the needle reads: the stop pin, if there is one.
    pub stop: Option<f32>,
}

impl Scale {
    /// The scale of `side`'s thermometer: the hot one to 1200 K in hundreds,
    /// the cold one to 300 K in twenties, stopping at 77 K.
    pub fn of(side: ReservoirSide) -> Self {
        match side {
            ReservoirSide::Hot => Self {
                top: 1_200.0,
                steps: 12,
                major: 3,
                stop: None,
            },
            ReservoirSide::Cold => Self {
                top: 300.0,
                steps: 15,
                major: 5,
                stop: Some(MIN_COLD_KELVIN),
            },
        }
    }

    /// Where `kelvin` lies on the scale, from 0 at absolute zero to 1 at its
    /// far end, held to the stop pin and the far end.
    pub fn share(self, kelvin: f32) -> f32 {
        let least = self.stop.unwrap_or(0.0);
        kelvin.clamp(least, self.top) / self.top
    }
}

/// The needle's turn for `share` of the scale, in radians clockwise from
/// straight up.
pub fn needle_turn(share: f32) -> f32 {
    (share - 0.5) * SWEEP
}

/// The point on a face at `middle` of `radius`, `reach` of its radius out
/// toward `share` of the scale, standing `raise` off it.
fn on_face(middle: Vec3, radius: f32, share: f32, reach: f32, raise: f32) -> Vec3 {
    let turned = needle_turn(share);
    middle + vec3(turned.sin(), turned.cos(), 0.0) * reach * radius + Vec3::Z * raise
}

/// Where the middle of `side`'s dial's face is in a block of `shape`.
pub fn dial_middle(shape: BlockShape, side: ReservoirSide) -> Vec3 {
    let unit = shape.bore;
    let across = match side {
        ReservoirSide::Hot => -DIAL_ACROSS,
        ReservoirSide::Cold => DIAL_ACROSS,
    };
    let height = shape.top + (STEM_HEIGHT + RIM_WIDTH + FACE_RADIUS) * unit;
    vec3(
        across * unit,
        height,
        (DIAL_FORWARD + CASE_DEPTH / 2.0) * unit,
    )
}

/// Radius of a dial, to the outside of its rim, in a block of `shape`.
pub fn dial_radius(shape: BlockShape) -> f32 {
    (FACE_RADIUS + RIM_WIDTH) * shape.bore
}

/// Where `side`'s stem enters the block's top in a block of `shape`.
pub fn stem_foot(shape: BlockShape, side: ReservoirSide) -> Vec3 {
    let middle = dial_middle(shape, side);
    vec3(middle.x, shape.top, DIAL_FORWARD * shape.bore)
}

/// A flat stroke on a face facing +z, from `a` to `b`, `half` wide each
/// side and squared off past its ends by as much.
fn stroke(solid: &mut Solid, a: Vec3, b: Vec3, half: f32, color: Color) {
    let along = (b - a).normalize_or_zero() * half;
    let across = vec3(-along.y, along.x, 0.0);
    let corners = [
        a - along - across,
        b + along - across,
        b + along + across,
        a - along + across,
    ];
    solid.quad(corners, [Vec3::Z; 4], [color; 4]);
}

/// Strokes of the letter K in the digits' grid.
const LETTER_K: [((f32, f32), (f32, f32)); 3] = [
    ((0.0, 0.0), (0.0, 6.0)),
    ((4.0, 6.0), (0.0, 2.5)),
    ((1.2, 3.5), (4.0, 0.0)),
];

/// Adds `text` in strokes to a face facing +z, centered on `middle`,
/// `height` tall.
fn letters(solid: &mut Solid, text: &str, middle: Vec3, height: f32) {
    let grid = height / NUMERAL_ROWS;
    let advance = NUMERAL_COLUMNS + NUMBER_GAP;
    let count = text.chars().count() as f32;
    let left = -(count * advance - NUMBER_GAP) / 2.0;
    for (index, glyph) in text.chars().enumerate() {
        let start = left + index as f32 * advance;
        let strokes = if glyph == 'K' {
            &LETTER_K[..]
        } else {
            digit(glyph)
        };
        for &((x0, y0), (x1, y1)) in strokes {
            let at = |x: f32, y: f32| {
                middle + vec3((start + x) * grid, (y - NUMERAL_ROWS / 2.0) * grid, 0.0)
            };
            stroke(
                solid,
                at(x0, y0),
                at(x1, y1),
                NUMBER_STROKE * grid,
                MARK_INK,
            );
        }
    }
}

/// Builds `side`'s thermometer in a block of `shape`, but for its needle:
/// fitting, stem, case, rim, face, ticks, numbers, unit, and on the cold
/// one the out-of-reach band and the stop pin.
fn dial_solid(shape: BlockShape, side: ReservoirSide) -> Solid {
    let unit = shape.bore;
    let scale = Scale::of(side);
    let paint = match side {
        ReservoirSide::Hot => HOT_PAINT,
        ReservoirSide::Cold => COLD_PAINT,
    };
    let middle = dial_middle(shape, side);
    let foot = stem_foot(shape, side);
    let radius = FACE_RADIUS * unit;
    let rim = radius + RIM_WIDTH * unit;
    let back = foot.z - CASE_DEPTH * unit / 2.0;
    let face = middle.z;
    let mut solid = Solid::new();
    // The fitting on the block's top and the stem up into the case.
    solid.lathe(
        Frame::along(foot, Vec3::Y),
        &[
            turn(0.0, 0.0, BRASS),
            turn(FITTING_RADIUS * unit, 0.0, BRASS),
            turn(FITTING_RADIUS * unit, FITTING_HEIGHT * unit, BRASS),
            turn(0.0, FITTING_HEIGHT * unit, BRASS),
        ],
        SMALL_SEGMENTS / 2,
    );
    let case_bottom = middle.y - rim;
    solid.cylinder(
        foot,
        vec3(foot.x, case_bottom + RIM_WIDTH * unit, foot.z),
        STEM_RADIUS * unit,
        SMALL_SEGMENTS,
        BRASS,
    );
    // The case, from its back to the rim, which stands a little proud of
    // the face.
    let rim_raise = RIM_WIDTH * unit / 2.0;
    solid.lathe(
        Frame::along(vec3(middle.x, middle.y, back), Vec3::Z),
        &[
            turn(0.0, 0.0, CASE_STEEL),
            turn(rim, 0.0, CASE_STEEL),
            turn(rim, 0.0, paint),
            turn(rim, face - back, paint),
            turn(rim, face - back + rim_raise, paint),
            turn(radius, face - back + rim_raise, paint),
            turn(radius, face - back, FACE_ENAMEL),
            turn(0.0, face - back, FACE_ENAMEL),
        ],
        SEGMENTS,
    );
    let mark = MARK_RAISE * unit;
    let at_face = vec3(middle.x, middle.y, face);
    // The out-of-reach band, below the stop pin.
    if let Some(stop) = scale.stop {
        let end = scale.share(stop);
        let (inner, outer) = BAND_RADII;
        for step in 0..BAND_STEPS {
            let share = |step: usize| end * step as f32 / BAND_STEPS as f32;
            let (from, to) = (share(step), share(step + 1));
            let corners = [
                on_face(at_face, radius, from, inner, mark / 2.0),
                on_face(at_face, radius, from, outer, mark / 2.0),
                on_face(at_face, radius, to, outer, mark / 2.0),
                on_face(at_face, radius, to, inner, mark / 2.0),
            ];
            solid.quad(corners, [Vec3::Z; 4], [BAND_SHADE; 4]);
        }
        solid.lathe(
            Frame::along(on_face(at_face, radius, end, STOP_RING, 0.0), Vec3::Z),
            &[
                turn(STOP_RADIUS * radius, 0.0, STOP_RED),
                turn(STOP_RADIUS * radius, STOP_HEIGHT * unit, STOP_RED),
                turn(0.0, STOP_HEIGHT * unit, STOP_RED),
            ],
            SMALL_SEGMENTS,
        );
    }
    // The ticks, and a number at each long one.
    for step in 0..=scale.steps {
        let share = step as f32 / scale.steps as f32;
        let major = step % scale.major == 0;
        let (inner, half) = if major {
            (MAJOR_TICK_INNER, MAJOR_TICK_HALF)
        } else {
            (MINOR_TICK_INNER, MINOR_TICK_HALF)
        };
        let a = on_face(at_face, radius, share, inner, mark);
        let b = on_face(at_face, radius, share, TICK_OUTER, mark);
        stroke(&mut solid, a, b, half * radius, MARK_INK);
        if major {
            let kelvin = (scale.top * share).round() as u32;
            let place = on_face(at_face, radius, share, NUMBER_RING, mark);
            letters(
                &mut solid,
                &kelvin.to_string(),
                place,
                NUMBER_HEIGHT * radius,
            );
        }
    }
    let unit_place = at_face + vec3(0.0, -UNIT_DROP * radius, mark);
    letters(&mut solid, "K", unit_place, UNIT_HEIGHT * radius);
    solid
}

/// Builds a needle of a face of `radius`, pointing straight up from its
/// pin at the origin, facing +z, with the cap over its pin.
fn needle_solid(radius: f32, unit: f32, color: Color) -> Solid {
    let mut solid = Solid::new();
    let half = NEEDLE_HALF * radius;
    let tip = Vec2::Y * NEEDLE_LENGTH * radius;
    let tail = -Vec2::Y * NEEDLE_TAIL * radius;
    let corners =
        [vec2(-half, 0.0), tail, vec2(half, 0.0), tip].map(|point| Vec3::from((point, 0.0)));
    solid.quad(corners, [Vec3::Z; 4], [color; 4]);
    solid.lathe(
        Frame::along(Vec3::ZERO, Vec3::Z),
        &[
            turn(CAP_RADIUS * radius, 0.0, BRASS),
            turn(CAP_RADIUS * radius, CAP_HEIGHT * unit, BRASS),
            turn(0.0, CAP_HEIGHT * unit, BRASS),
        ],
        SMALL_SEGMENTS,
    );
    solid
}

/// `solid` on the graphics card, solid in plain colors where there is no
/// lit-metal material, whose colors' alpha means polish.
///
/// # Panics
///
/// Panics if it would not fit Macroquad's buffers.
fn upload(solid: Solid, plain: bool) -> GpuMesh {
    assert!(solid.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
    assert!(solid.indices.len() < DRAW_CALL_INDEX_CAPACITY);
    let mut mesh = Mesh {
        vertices: solid.vertices,
        indices: solid.indices,
        texture: None,
    };
    if plain {
        for vertex in &mut mesh.vertices {
            vertex.color[3] = u8::MAX;
        }
    }
    GpuMesh::new(mesh)
}

/// One thermometer: its dial, its needle and where the needle points.
struct Dial {
    /// Which side's temperature it reads.
    side: ReservoirSide,
    /// Everything but the needle.
    body: GpuMesh,
    /// The needle, pointing straight up from its pin at the origin.
    needle: GpuMesh,
    /// Where the needle's pin is.
    pin: Vec3,
    /// Where on the scale the needle points, from 0 to 1.
    share: f32,
}

/// The thermometers on both cylinders.
pub struct Thermometers {
    /// The hot cylinder's and the cold's.
    dials: [Dial; 2],
    /// The clock at the last `follow`, if there was one.
    last_seconds: Option<f64>,
    /// The lit-metal material, if it can be built.
    material: Option<GpuMaterial>,
}

impl Thermometers {
    /// The thermometers on a block of `shape`, their needles at the far
    /// end of their stops.
    pub fn new(shape: BlockShape) -> Self {
        let material = load_metal();
        let plain = material.is_none();
        let unit = shape.bore;
        let dial = |side, paint| {
            let middle = dial_middle(shape, side);
            Dial {
                side,
                body: upload(dial_solid(shape, side), plain),
                needle: upload(needle_solid(FACE_RADIUS * unit, unit, paint), plain),
                pin: middle + Vec3::Z * NEEDLE_RAISE * unit,
                share: Scale::of(side).share(0.0),
            }
        };
        Self {
            dials: [
                dial(ReservoirSide::Hot, HOT_PAINT),
                dial(ReservoirSide::Cold, COLD_PAINT),
            ],
            last_seconds: None,
            material,
        }
    }

    /// Swings the needles toward the walls' temperatures, `hot` and `cold`
    /// in kelvin, at `seconds` on the clock.
    pub fn follow(&mut self, hot: f32, cold: f32, seconds: f64) {
        let dt = self
            .last_seconds
            .map_or(f32::INFINITY, |last| (seconds - last).max(0.0) as f32);
        self.last_seconds = Some(seconds);
        let keep = (-dt / NEEDLE_SECONDS).exp();
        for dial in &mut self.dials {
            let kelvin = match dial.side {
                ReservoirSide::Hot => hot,
                ReservoirSide::Cold => cold,
            };
            let target = Scale::of(dial.side).share(kelvin);
            dial.share = target + (dial.share - target) * keep;
        }
    }

    /// Where `side`'s needle points on its scale, from 0 to 1.
    #[cfg(test)]
    pub fn share(&self, side: ReservoirSide) -> f32 {
        self.dials
            .iter()
            .find(|dial| dial.side == side)
            .map_or(0.0, |dial| dial.share)
    }

    /// Draws both thermometers seen from `camera`.
    pub fn draw(&self, camera: &Camera3D) {
        let light = key_light(camera.position);
        let material = self.material.as_ref();
        for dial in &self.dials {
            draw_lit_placed([&dial.body], material, camera, light, Mat4::IDENTITY);
            let turned =
                Mat4::from_translation(dial.pin) * Mat4::from_rotation_z(-needle_turn(dial.share));
            draw_lit_placed([&dial.needle], material, camera, light, turned);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/thermometers.rs"]
mod tests;
