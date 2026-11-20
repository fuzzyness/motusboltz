//! The desk clock: a yellow twin-bell alarm clock standing on the table
//! beyond the hot end, showing the computer's time.
//!
//! A toy, not a real one: a chunky round yellow case on two stubby silver
//! legs with big ball feet, a fat yellow rim round a plain white face, big
//! blue numerals at 12, 3, 6 and 9 and round dots for the other hours,
//! thick blue hands and a red second hand. Two round yellow bells sit on
//! its shoulders, a small silver handle loops up between them, and a
//! hammer in front of it rattles when the clock pauses or resumes the
//! engine. It stands beyond the cartridge, turned toward the
//! usual view, clear of the cylinders.
//!
//! Drawing only: the time it shows and what a press does are the app's
//! (see `ui::desk_clock`).

use std::f32::consts::{
    FRAC_PI_2,
    TAU, //
};

use macroquad::models::{
    Mesh,
    Vertex, //
};
use macroquad::prelude::{
    Camera3D,
    Color,
    Mat3,
    Mat4,
    Quat,
    Vec2,
    Vec3,
    vec3, //
};

use super::block::{
    BlockShape,
    key_light, //
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
    draw_lit,
    draw_lit_placed, //
};
use crate::ui::desk_clock::HandAngles;

/// How far past the block's hot end the clock stands, to the middle of its
/// feet, in bore radii: beyond the cartridge.
const PLACE_OUT: f32 = 3.2;
/// How far toward the back it stands, in bore radii: a little behind the
/// cartridge, and not so far that the hot cylinder hides it from the
/// usual view.
const PLACE_BACK: f32 = 0.2;
/// How far it is turned from facing the front toward the usual view,
/// counterclockwise seen from above, in radians.
const PLACE_YAW: f32 = 0.7;
/// How much bigger the clock is than the sizes in bore radii below say.
const CLOCK_SCALE: f32 = 0.85;
/// Radius of the case, in bore radii.
const CASE_RADIUS: f32 = 1.05;
/// Half the case's depth, front to back, in bore radii.
const CASE_HALF_DEPTH: f32 = 0.45;
/// Radius of the rounding where the case's side meets its back, in bore
/// radii.
const CASE_ROUND: f32 = 0.3;
/// Where the rounding's middle point lies, as a share of its radius in
/// from the side and out from the back.
const CASE_ROUND_MIDDLE: f32 = 0.3;
/// How high the legs hold the case's bottom off the table, in bore radii.
const LIFT: f32 = 0.22;
/// Outer radius of the bezel, as a share of the case's.
const BEZEL_OUTER: f32 = 0.99;
/// Inner radius of the bezel, the dial's edge, as a share of the case's.
const BEZEL_INNER: f32 = 0.8;
/// How far the bezel stands proud of the case's front, in bore radii.
const BEZEL_PROUD: f32 = 0.14;
/// Height of the bezel's outer shoulder, as a share of how proud it stands.
const BEZEL_SHOULDER: f32 = 0.7;
/// Height of the bezel's inner lip, as a share of how proud it stands.
const BEZEL_LIP: f32 = 0.6;
/// How far the dial sits behind the bezel's top, in bore radii.
const DIAL_SINK: f32 = 0.1;
/// Radius of an hour's dot, as a share of the dial's radius.
const DOT_RADIUS: f32 = 0.07;
/// Hours between the numerals: 12, 3, 6 and 9 get one, the rest a dot.
const HOURS_PER_NUMERAL: usize = 3;
/// Radius of the circle the numerals' middles stand on, as a share of the
/// dial's.
const NUMERAL_RING: f32 = 0.64;
/// Height of a numeral, as a share of the dial's radius.
const NUMERAL_HEIGHT: f32 = 0.27;
/// Rows in the numerals' grid, from the baseline to the top.
const NUMERAL_ROWS: f32 = 6.0;
/// Columns in a digit.
const NUMERAL_COLUMNS: f32 = 4.0;
/// Room between the digits of a numeral, in the numerals' grid.
const NUMERAL_GAP: f32 = 1.2;
/// Half the width of a numeral's stroke, in the numerals' grid.
const NUMERAL_STROKE: f32 = 0.8;
/// How far the numerals and dots stand off the dial, in bore radii.
const MARK_RAISE: f32 = 0.012;
/// How far each hand stands off the dial, hour, minute, second, in bore
/// radii.
const HAND_RAISE: [f32; 3] = [0.03, 0.05, 0.07];
/// How far the cap over the hands' pin stands off the dial, in bore radii.
const CAP_RAISE: f32 = 0.085;
/// Height of the cap's shoulder, as a share of its height.
const CAP_SHOULDER: f32 = 0.5;
/// Radius of the cap's flat top, as a share of its radius.
const CAP_TOP: f32 = 0.6;
/// Length of the hour hand, as a share of the dial's radius.
const HOUR_LENGTH: f32 = 0.45;
/// Half the width of the hour hand at its root, as a share of the dial's
/// radius.
const HOUR_HALF: f32 = 0.09;
/// Length of the minute hand, as a share of the dial's radius.
const MINUTE_LENGTH: f32 = 0.68;
/// Half the width of the minute hand at its root, as a share of the dial's
/// radius.
const MINUTE_HALF: f32 = 0.07;
/// Length of the second hand, as a share of the dial's radius.
const SECOND_LENGTH: f32 = 0.72;
/// Length of the second hand's tail behind the pin, as a share of the
/// dial's radius.
const SECOND_TAIL: f32 = 0.16;
/// Half the width of the second hand, as a share of the dial's radius.
const SECOND_HALF: f32 = 0.028;
/// Share of a hand's root width left at its tip.
const HAND_TAPER: f32 = 0.75;
/// Half the thickness of a hand, in bore radii.
const HAND_THICK: f32 = 0.012;
/// Radius of the cap over the hands' pin, as a share of the dial's radius.
const CAP_RADIUS: f32 = 0.11;
/// Where each bell stands on the case's rim, as an angle from the right
/// in the face's plane, in radians; the other is mirrored.
const BELL_ANGLE: f32 = 0.9;
/// How far back from the case's middle the bells and their posts stand,
/// in bore radii.
const BELL_BACK: f32 = 0.06;
/// Length of a bell's post beyond the case, in bore radii.
const POST_LENGTH: f32 = 0.06;
/// Where a bell's post leaves the case, as a share of the case's radius
/// out from its middle: inside it, so no gap shows.
const POST_ROOT: f32 = 0.9;
/// Radius of a bell's post, in bore radii.
const POST_RADIUS: f32 = 0.08;
/// Radius of a bell's open rim, in bore radii.
const BELL_RADIUS: f32 = 0.52;
/// Height of a bell from its rim to its crown, in bore radii.
const BELL_HEIGHT: f32 = 0.48;
/// Bands from a bell's rim to its crown.
const BELL_BANDS: usize = 8;
/// Radius of the knob on each bell's crown, in bore radii.
const KNOB_RADIUS: f32 = 0.11;
/// How far each knob's middle stands past its bell's crown, as a share of
/// its radius: half sunk into the crown.
const KNOB_SINK: f32 = 0.5;
/// How far back from the case's middle the handle arches, in bore radii.
const HANDLE_BACK: f32 = 0.3;
/// Half the handle's width, where it meets the case, in bore radii.
const HANDLE_HALF_WIDTH: f32 = 0.36;
/// Height of the handle's top above the case's middle, in bore radii.
const HANDLE_TOP: f32 = 1.6;
/// Height above the case's middle where the handle's arch starts, in bore
/// radii.
const HANDLE_SPRING: f32 = 1.0;
/// Radius of the handle's wire, in bore radii.
const HANDLE_RADIUS: f32 = 0.075;
/// Where the handle meets the case, as a share of the case's radius out
/// from its middle.
const HANDLE_ROOT: f32 = 0.85;
/// Pieces the handle's arch is built of.
const HANDLE_PIECES: usize = 12;
/// Length of the hammer's rod above the case, in bore radii.
const HAMMER_LENGTH: f32 = 0.3;
/// How far the hammer's rod reaches down into the case, in bore radii.
const HAMMER_SINK: f32 = 0.1;
/// Radius of the hammer's rod, in bore radii.
const HAMMER_RADIUS: f32 = 0.05;
/// Half the length of the hammer's head, across, in bore radii.
const HAMMER_HEAD_HALF: f32 = 0.15;
/// Radius of the hammer's head, in bore radii.
const HAMMER_HEAD_RADIUS: f32 = 0.08;
/// Radius of a leg, in bore radii.
const LEG_RADIUS: f32 = 0.1;
/// Where each leg leaves the case, as an angle down from the right in the
/// face's plane, in radians; the other is mirrored.
const LEG_ANGLE: f32 = 0.85;
/// Where each leg leaves the case, as a share of the case's radius out
/// from its middle: inside it, so no gap shows.
const LEG_ROOT: f32 = 0.85;
/// How far out from the middle each foot stands, in bore radii.
const FOOT_OUT: f32 = 0.78;
/// Radius of a ball foot, in bore radii.
const FOOT_RADIUS: f32 = 0.15;
/// Hours around the dial.
const HOURS: usize = 12;
/// Segments around round parts.
const SEGMENTS: usize = 32;
/// Segments around thin round parts.
const THIN_SEGMENTS: usize = 12;
/// Bands around a ball.
const BALL_BANDS: usize = 6;
/// Seconds the hammer rattles between the bells.
pub const RING_SECONDS: f32 = 0.6;
/// Strikes of the hammer a second while it rattles.
const RING_RATE: f32 = 18.0;
/// How far the hammer swings each way as it rattles, in radians.
const RING_SWING: f32 = 0.3;
/// The case's, rim's and bells' glossy yellow.
const CASE_YELLOW: Color = finish(250, 200, 30, 0.7);
/// The legs', posts', knobs', handle's and hammer's silver, a toy's painted
/// plastic rather than chrome.
const CHROME: Color = finish(214, 220, 232, 0.5);
/// The face's white.
const FACE_WHITE: Color = finish(252, 250, 242, 0.1);
/// The numerals', dots' and hour and minute hands' blue.
const MARK_BLUE: Color = finish(36, 70, 180, 0.3);
/// The second hand's and the cap's red.
const HAND_RED: Color = finish(226, 50, 40, 0.4);

/// Where the clock stands and how it is turned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClockPlace {
    /// The middle between its feet, on the table.
    pub base: Vec3,
    /// How far it is turned from facing the front, counterclockwise seen
    /// from above, in radians.
    pub yaw: f32,
    /// The bore radius every size is a fraction of.
    unit: f32,
}

impl ClockPlace {
    /// The middle of the dial, where the hands turn: the point to press.
    pub fn face(self) -> Vec3 {
        self.place(dial_center(self.unit))
    }

    /// The middle of the case.
    pub fn middle(self) -> Vec3 {
        self.place(case_center(self.unit))
    }

    /// The radius of the case.
    pub fn radius(self) -> f32 {
        CASE_RADIUS * self.unit
    }

    /// The height of the handle's top above the table.
    #[cfg(test)]
    pub fn height(self) -> f32 {
        case_center(self.unit).y + HANDLE_TOP * self.unit
    }

    /// The way the dial faces, a unit vector.
    #[cfg(test)]
    pub fn facing(self) -> Vec3 {
        self.turn() * Vec3::Z
    }

    /// `local`, a point built with the clock's base at the origin and its
    /// dial toward +z, placed in the world.
    fn place(self, local: Vec3) -> Vec3 {
        self.base + self.turn() * local
    }

    /// The clock's turn about the vertical.
    fn turn(self) -> Mat3 {
        Mat3::from_rotation_y(self.yaw)
    }

    /// The matrix that places a part built about the point `local`, turned
    /// `angle` clockwise about the dial's axis, as seen on the dial.
    fn placing(self, local: Vec3, angle: f32) -> Mat4 {
        let turn = Quat::from_rotation_y(self.yaw);
        Mat4::from_rotation_translation(turn, self.base)
            * Mat4::from_translation(local)
            * Mat4::from_rotation_z(-angle)
    }
}

/// Where the clock stands, beyond the hot end of a block of `shape`.
pub fn clock_place(shape: BlockShape) -> ClockPlace {
    let end = shape.offset + shape.outer;
    ClockPlace {
        base: vec3(
            -(end + PLACE_OUT * shape.bore),
            shape.base,
            -PLACE_BACK * shape.bore,
        ),
        yaw: PLACE_YAW,
        unit: shape.bore * CLOCK_SCALE,
    }
}

/// The middle of the case, with the base at the origin.
fn case_center(unit: f32) -> Vec3 {
    Vec3::Y * (LIFT + CASE_RADIUS) * unit
}

/// The middle of the dial's face, with the base at the origin.
fn dial_center(unit: f32) -> Vec3 {
    case_center(unit) + Vec3::Z * (CASE_HALF_DEPTH + BEZEL_PROUD - DIAL_SINK) * unit
}

/// The dial's radius.
fn dial_radius(unit: f32) -> f32 {
    BEZEL_INNER * CASE_RADIUS * unit
}

/// The point `radius` from `center` toward `angle` (from the right,
/// counterclockwise) in the face's plane.
fn in_face(center: Vec3, angle: f32, radius: f32) -> Vec3 {
    center + Vec3::from((Vec2::from_angle(angle) * radius, 0.0))
}

/// One stroke of a digit: from one point of its grid to another.
type Stroke = ((f32, f32), (f32, f32));

/// Strokes of each digit, as segments between points of a grid
/// `NUMERAL_COLUMNS` wide and `NUMERAL_ROWS` high, from the baseline.
fn digit(glyph: char) -> &'static [Stroke] {
    match glyph {
        '0' => &[
            ((1.0, 0.0), (3.0, 0.0)),
            ((3.0, 0.0), (4.0, 1.0)),
            ((4.0, 1.0), (4.0, 5.0)),
            ((4.0, 5.0), (3.0, 6.0)),
            ((3.0, 6.0), (1.0, 6.0)),
            ((1.0, 6.0), (0.0, 5.0)),
            ((0.0, 5.0), (0.0, 1.0)),
            ((0.0, 1.0), (1.0, 0.0)),
        ],
        '1' => &[((1.0, 5.0), (2.5, 6.0)), ((2.5, 6.0), (2.5, 0.0))],
        '2' => &[
            ((0.0, 5.0), (1.0, 6.0)),
            ((1.0, 6.0), (3.0, 6.0)),
            ((3.0, 6.0), (4.0, 5.0)),
            ((4.0, 5.0), (4.0, 4.0)),
            ((4.0, 4.0), (0.0, 0.0)),
            ((0.0, 0.0), (4.0, 0.0)),
        ],
        '3' => &[
            ((0.0, 5.0), (1.0, 6.0)),
            ((1.0, 6.0), (3.0, 6.0)),
            ((3.0, 6.0), (4.0, 5.0)),
            ((4.0, 5.0), (4.0, 4.0)),
            ((4.0, 4.0), (3.0, 3.0)),
            ((3.0, 3.0), (1.5, 3.0)),
            ((3.0, 3.0), (4.0, 2.0)),
            ((4.0, 2.0), (4.0, 1.0)),
            ((4.0, 1.0), (3.0, 0.0)),
            ((3.0, 0.0), (1.0, 0.0)),
            ((1.0, 0.0), (0.0, 1.0)),
        ],
        '4' => &[
            ((3.0, 0.0), (3.0, 6.0)),
            ((3.0, 6.0), (0.0, 2.0)),
            ((0.0, 2.0), (4.0, 2.0)),
        ],
        '5' => &[
            ((4.0, 6.0), (0.5, 6.0)),
            ((0.5, 6.0), (0.0, 3.5)),
            ((0.0, 3.5), (3.0, 3.5)),
            ((3.0, 3.5), (4.0, 2.5)),
            ((4.0, 2.5), (4.0, 1.0)),
            ((4.0, 1.0), (3.0, 0.0)),
            ((3.0, 0.0), (1.0, 0.0)),
            ((1.0, 0.0), (0.0, 1.0)),
        ],
        '6' => &[
            ((3.5, 6.0), (1.5, 6.0)),
            ((1.5, 6.0), (0.0, 4.0)),
            ((0.0, 4.0), (0.0, 1.0)),
            ((0.0, 1.0), (1.0, 0.0)),
            ((1.0, 0.0), (3.0, 0.0)),
            ((3.0, 0.0), (4.0, 1.0)),
            ((4.0, 1.0), (4.0, 2.5)),
            ((4.0, 2.5), (3.0, 3.5)),
            ((3.0, 3.5), (1.0, 3.5)),
            ((1.0, 3.5), (0.0, 2.5)),
        ],
        '7' => &[((0.0, 6.0), (4.0, 6.0)), ((4.0, 6.0), (1.5, 0.0))],
        '8' => &[
            ((1.0, 3.0), (0.0, 4.0)),
            ((0.0, 4.0), (0.0, 5.0)),
            ((0.0, 5.0), (1.0, 6.0)),
            ((1.0, 6.0), (3.0, 6.0)),
            ((3.0, 6.0), (4.0, 5.0)),
            ((4.0, 5.0), (4.0, 4.0)),
            ((4.0, 4.0), (3.0, 3.0)),
            ((3.0, 3.0), (1.0, 3.0)),
            ((1.0, 3.0), (0.0, 2.0)),
            ((0.0, 2.0), (0.0, 1.0)),
            ((0.0, 1.0), (1.0, 0.0)),
            ((1.0, 0.0), (3.0, 0.0)),
            ((3.0, 0.0), (4.0, 1.0)),
            ((4.0, 1.0), (4.0, 2.0)),
            ((4.0, 2.0), (3.0, 3.0)),
        ],
        '9' => &[
            ((0.5, 0.0), (2.5, 0.0)),
            ((2.5, 0.0), (4.0, 2.0)),
            ((4.0, 2.0), (4.0, 5.0)),
            ((4.0, 5.0), (3.0, 6.0)),
            ((3.0, 6.0), (1.0, 6.0)),
            ((1.0, 6.0), (0.0, 5.0)),
            ((0.0, 5.0), (0.0, 3.5)),
            ((0.0, 3.5), (1.0, 2.5)),
            ((1.0, 2.5), (3.0, 2.5)),
            ((3.0, 2.5), (4.0, 3.5)),
        ],
        _ => &[],
    }
}

/// Adds a round dot of `radius` to `solid` in the face's plane at
/// `center`, facing +z.
fn dot(solid: &mut Solid, center: Vec3, radius: f32) {
    solid.lathe(
        Frame::along(center, Vec3::Z),
        &[turn(radius, 0.0, MARK_BLUE), turn(0.0, 0.0, MARK_BLUE)],
        THIN_SEGMENTS,
    );
}

/// Adds `text`'s digits to `solid` in the face's plane, centered on
/// `middle`, each `height` tall.
fn numeral(solid: &mut Solid, text: &str, middle: Vec3, height: f32) {
    let grid = height / NUMERAL_ROWS;
    let advance = NUMERAL_COLUMNS + NUMERAL_GAP;
    let count = text.chars().count() as f32;
    let left = -(count * advance - NUMERAL_GAP) / 2.0;
    let half = NUMERAL_STROKE * grid;
    for (index, glyph) in text.chars().enumerate() {
        let start = left + index as f32 * advance;
        for &((x0, y0), (x1, y1)) in digit(glyph) {
            let at = |x: f32, y: f32| {
                middle + vec3((start + x) * grid, (y - NUMERAL_ROWS / 2.0) * grid, 0.0)
            };
            let (a, b) = (at(x0, y0), at(x1, y1));
            let along = (b - a).normalize_or_zero() * half;
            let across = vec3(-along.y, along.x, 0.0);
            let corners = [
                a - along - across,
                b + along - across,
                b + along + across,
                a - along + across,
            ];
            solid.quad(corners, [Vec3::Z; 4], [MARK_BLUE; 4]);
        }
    }
}

/// Adds a ball of `radius` at `center` to `solid`.
fn ball(solid: &mut Solid, center: Vec3, radius: f32, color: Color) {
    let outline: Vec<_> = (0..=BALL_BANDS)
        .map(|band| {
            let angle = std::f32::consts::PI * band as f32 / BALL_BANDS as f32;
            turn(radius * angle.sin(), -radius * angle.cos(), color)
        })
        .collect();
    solid.lathe(Frame::along(center, Vec3::Y), &outline, THIN_SEGMENTS);
}

/// Adds a round tube of `radius` along `path`, which lies in a plane
/// facing +z, to `solid`, smooth round its bends.
fn tube(solid: &mut Solid, path: &[Vec3], radius: f32, color: Color) {
    let ring = |index: usize| {
        let before = path[index.saturating_sub(1)];
        let after = path[(index + 1).min(path.len() - 1)];
        let tangent = (after - before).normalize_or_zero();
        let side = tangent.cross(Vec3::Z).normalize_or_zero();
        (0..=THIN_SEGMENTS)
            .map(|step| {
                let angle = TAU * step as f32 / THIN_SEGMENTS as f32;
                let normal = side * angle.cos() + Vec3::Z * angle.sin();
                (path[index] + normal * radius, normal)
            })
            .collect::<Vec<_>>()
    };
    let rings: Vec<_> = (0..path.len()).map(ring).collect();
    for pair in rings.windows(2) {
        for step in 0..THIN_SEGMENTS {
            let corners = [
                pair[0][step],
                pair[1][step],
                pair[1][step + 1],
                pair[0][step + 1],
            ];
            solid.quad(
                corners.map(|(point, _)| point),
                corners.map(|(_, normal)| normal),
                [color; 4],
            );
        }
    }
}

/// The points along the handle, from where it leaves the case on the right
/// over the top to where it meets the case on the left, with the base at
/// the origin.
pub(super) fn handle_path(unit: f32) -> Vec<Vec3> {
    let center = case_center(unit) - Vec3::Z * HANDLE_BACK * unit;
    let width = HANDLE_HALF_WIDTH * unit;
    let spring = HANDLE_SPRING * unit;
    let rise = (HANDLE_TOP - HANDLE_SPRING) * unit;
    // Where it leaves the case, straight below the arch's ends, a little
    // inside the case's rim so no gap shows.
    let radius = HANDLE_ROOT * CASE_RADIUS * unit;
    let foot = |side: f32| {
        let x = side * width.min(radius);
        center + vec3(x, (radius * radius - x * x).max(0.0).sqrt(), 0.0)
    };
    let mut path = vec![foot(1.0)];
    for piece in 0..=HANDLE_PIECES {
        let angle = std::f32::consts::PI * piece as f32 / HANDLE_PIECES as f32;
        path.push(center + vec3(width * angle.cos(), spring + rise * angle.sin(), 0.0));
    }
    path.push(foot(-1.0));
    path
}

/// Builds the case, bezel, dial, legs, bells and handle, with the base at
/// the origin and the dial toward +z, in bore radii of `unit`.
fn build_body(unit: f32) -> Solid {
    let mut solid = Solid::new();
    let center = case_center(unit);
    let radius = CASE_RADIUS * unit;
    let (depth, round) = (CASE_HALF_DEPTH * unit, CASE_ROUND * unit);
    // The case, turned about the dial's axis from its back to its front.
    let back = Frame::along(center - Vec3::Z * depth, Vec3::Z);
    let bezel_outer = BEZEL_OUTER * radius;
    solid.lathe(
        back,
        &[
            turn(0.0, 0.0, CASE_YELLOW),
            turn(radius - round, 0.0, CASE_YELLOW),
            turn(
                radius - CASE_ROUND_MIDDLE * round,
                CASE_ROUND_MIDDLE * round,
                CASE_YELLOW,
            ),
            turn(radius, round, CASE_YELLOW),
            turn(radius, 2.0 * depth, CASE_YELLOW),
            turn(bezel_outer, 2.0 * depth, CASE_YELLOW),
        ],
        SEGMENTS,
    );
    // The rim: a fat yellow ring standing proud of the front, rounded over.
    let front = Frame::along(center + Vec3::Z * depth, Vec3::Z);
    let (proud, inner) = (BEZEL_PROUD * unit, dial_radius(unit));
    let dial_at = proud - DIAL_SINK * unit;
    solid.lathe(
        front,
        &[
            turn(bezel_outer, 0.0, CASE_YELLOW),
            turn(bezel_outer, proud * BEZEL_SHOULDER, CASE_YELLOW),
            turn((bezel_outer + inner) / 2.0, proud, CASE_YELLOW),
            turn(inner, proud * BEZEL_LIP, CASE_YELLOW),
            turn(inner, dial_at, CASE_YELLOW),
        ],
        SEGMENTS,
    );
    // The face, plain white, with big numerals at the quarters and dots
    // at the other hours.
    let dial = dial_center(unit);
    let dial_radius = inner;
    solid.lathe(
        Frame::along(dial, Vec3::Z),
        &[
            turn(dial_radius, 0.0, FACE_WHITE),
            turn(0.0, 0.0, FACE_WHITE),
        ],
        SEGMENTS,
    );
    let marks = dial + Vec3::Z * MARK_RAISE * unit;
    for hour in 1..=HOURS {
        let angle = FRAC_PI_2 - TAU * hour as f32 / HOURS as f32;
        let spot = in_face(marks, angle, NUMERAL_RING * dial_radius);
        if hour % HOURS_PER_NUMERAL == 0 {
            numeral(
                &mut solid,
                &hour.to_string(),
                spot,
                NUMERAL_HEIGHT * dial_radius,
            );
        } else {
            dot(&mut solid, spot, DOT_RADIUS * dial_radius);
        }
    }
    // The legs, stubby, splayed out to big ball feet on the table.
    for side in [1.0, -1.0] {
        let angle = if side > 0.0 {
            -LEG_ANGLE
        } else {
            std::f32::consts::PI + LEG_ANGLE
        };
        let top = in_face(center, angle, radius * LEG_ROOT);
        let foot = vec3(side * FOOT_OUT * unit, FOOT_RADIUS * unit, 0.0);
        solid.cylinder(top, foot, LEG_RADIUS * unit, THIN_SEGMENTS, CHROME);
        ball(&mut solid, foot, FOOT_RADIUS * unit, CHROME);
    }
    // The bells, round, on short posts from the case's shoulders, open
    // toward it.
    let shoulder = center - Vec3::Z * BELL_BACK * unit;
    for side in [1.0, -1.0] {
        let angle = if side > 0.0 {
            BELL_ANGLE
        } else {
            std::f32::consts::PI - BELL_ANGLE
        };
        let out = Vec3::from((Vec2::from_angle(angle), 0.0));
        let root = shoulder + out * radius * POST_ROOT;
        let rim = shoulder + out * (radius + POST_LENGTH * unit);
        solid.cylinder(
            root,
            rim + out * BELL_HEIGHT * unit,
            POST_RADIUS * unit,
            THIN_SEGMENTS,
            CHROME,
        );
        let bell: Vec<_> = (0..=BELL_BANDS)
            .map(|band| {
                let share = band as f32 / BELL_BANDS as f32;
                let angle = FRAC_PI_2 * share;
                turn(
                    BELL_RADIUS * unit * angle.cos(),
                    BELL_HEIGHT * unit * angle.sin(),
                    CASE_YELLOW,
                )
            })
            .collect();
        solid.lathe(Frame::along(rim, out), &bell, SEGMENTS);
        ball(
            &mut solid,
            rim + out * (BELL_HEIGHT + KNOB_SINK * KNOB_RADIUS) * unit,
            KNOB_RADIUS * unit,
            CHROME,
        );
    }
    // The handle, a small thick loop between the bells, behind the hammer.
    tube(&mut solid, &handle_path(unit), HANDLE_RADIUS * unit, CHROME);
    solid
}

/// Builds the hammer, upright, with the top of the case at the origin, in
/// bore radii of `unit`.
fn build_hammer(unit: f32) -> Solid {
    let mut solid = Solid::new();
    let top = Vec3::Y * HAMMER_LENGTH * unit;
    solid.cylinder(
        -Vec3::Y * HAMMER_SINK * unit,
        top,
        HAMMER_RADIUS * unit,
        THIN_SEGMENTS,
        CHROME,
    );
    let half = Vec3::X * HAMMER_HEAD_HALF * unit;
    solid.cylinder(
        top - half,
        top + half,
        HAMMER_HEAD_RADIUS * unit,
        THIN_SEGMENTS,
        CHROME,
    );
    solid
}

/// Builds a hand pointing at 12 from the dial's middle at the origin,
/// `length` long with a `tail` behind the pin, `half` wide at its root and
/// tapering a little toward its tip, in `color`, in the face's plane.
fn build_hand(length: f32, tail: f32, half: f32, thick: f32, color: Color) -> Solid {
    let mut solid = Solid::new();
    let (root, tip) = (vec3(0.0, -tail, 0.0), vec3(0.0, length, 0.0));
    solid.bar(root, tip, Vec3::X, (half, half * HAND_TAPER), thick, color);
    solid
}

/// Builds the cap over the hands' pin, at the origin.
fn build_cap(radius: f32, raise: f32) -> Solid {
    let mut solid = Solid::new();
    solid.lathe(
        Frame::along(Vec3::ZERO, Vec3::Z),
        &[
            turn(radius, 0.0, HAND_RED),
            turn(radius, raise * CAP_SHOULDER, HAND_RED),
            turn(radius * CAP_TOP, raise, HAND_RED),
            turn(0.0, raise, HAND_RED),
        ],
        THIN_SEGMENTS,
    );
    solid
}

/// `solid`'s corners placed in the world for `place`, as a mesh.
fn placed(solid: Solid, place: ClockPlace) -> Mesh {
    let turn = place.turn();
    let vertices = solid
        .vertices
        .into_iter()
        .map(|mut vertex| {
            vertex.position = place.place(vertex.position);
            vertex.normal = (turn * vertex.normal.truncate()).extend(0.0);
            vertex
        })
        .collect::<Vec<Vertex>>();
    Mesh {
        vertices,
        indices: solid.indices,
        texture: None,
    }
}

/// `solid` as a mesh, as built.
fn unplaced(solid: Solid) -> Mesh {
    Mesh {
        vertices: solid.vertices,
        indices: solid.indices,
        texture: None,
    }
}

/// The desk clock.
pub struct Clock {
    /// Where it stands.
    place: ClockPlace,
    /// The case, dial, legs, bells and handle, in the world.
    body: GpuMesh,
    /// The hammer, upright about its pivot.
    hammer: GpuMesh,
    /// The hour, minute and second hands, pointing at 12 about the pin.
    hands: [GpuMesh; 3],
    /// The cap over the pin.
    cap: GpuMesh,
    /// Seconds left of the hammer's rattle.
    ringing: f32,
    /// The clock at the last draw, if there was one.
    last_seconds: Option<f64>,
    /// The lit-metal material, if it can be built.
    metal: Option<GpuMaterial>,
}

impl Clock {
    /// Builds the clock beyond the hot end of a block of `shape`.
    ///
    /// It needs a window, for the material; `build` does the rest without
    /// one.
    pub fn new(shape: BlockShape) -> Self {
        let mut clock = Self::build(shape);
        clock.metal = load_metal();
        if clock.metal.is_none() {
            // Drawn in plain colors, alpha means transparency: make it solid.
            let meshes = [&mut clock.body, &mut clock.hammer, &mut clock.cap];
            for mesh in meshes.into_iter().chain(&mut clock.hands) {
                for vertex in &mut mesh.edit().vertices {
                    vertex.color[3] = u8::MAX;
                }
            }
        }
        clock
    }

    /// Builds the clock beyond the hot end of a block of `shape`, without a
    /// material.
    ///
    /// # Panics
    ///
    /// Panics if it would not fit Macroquad's buffers.
    fn build(shape: BlockShape) -> Self {
        let place = clock_place(shape);
        let unit = place.unit;
        let body = placed(build_body(unit), place);
        assert!(body.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
        assert!(body.indices.len() < DRAW_CALL_INDEX_CAPACITY);
        let dial = dial_radius(unit);
        let thick = HAND_THICK * unit;
        let hands = [
            build_hand(HOUR_LENGTH * dial, 0.0, HOUR_HALF * dial, thick, MARK_BLUE),
            build_hand(
                MINUTE_LENGTH * dial,
                0.0,
                MINUTE_HALF * dial,
                thick,
                MARK_BLUE,
            ),
            build_hand(
                SECOND_LENGTH * dial,
                SECOND_TAIL * dial,
                SECOND_HALF * dial,
                thick,
                HAND_RED,
            ),
        ];
        Self {
            place,
            body: GpuMesh::new(body),
            hammer: GpuMesh::new(unplaced(build_hammer(unit))),
            hands: hands.map(|hand| GpuMesh::new(unplaced(hand))),
            cap: GpuMesh::new(unplaced(build_cap(
                CAP_RADIUS * dial,
                (CAP_RAISE - HAND_RAISE[2]) * unit,
            ))),
            ringing: 0.0,
            last_seconds: None,
            metal: None,
        }
    }

    /// Rattles the hammer between the bells, as the clock pauses or resumes
    /// the engine.
    pub fn ring(&mut self) {
        self.ringing = RING_SECONDS;
    }

    /// The hammer's swing from upright now, in radians, clockwise as seen
    /// on the dial.
    fn swing(&self) -> f32 {
        if self.ringing <= 0.0 {
            return 0.0;
        }
        let rung = RING_SECONDS - self.ringing;
        let fade = self.ringing / RING_SECONDS;
        RING_SWING * fade * (TAU * RING_RATE * rung).sin()
    }

    /// Draws the clock seen from `camera`, its hands at `hands`, at
    /// `seconds` on the clock that moves the hammer.
    pub fn draw(&mut self, hands: HandAngles, seconds: f64, camera: &Camera3D) {
        let dt = self
            .last_seconds
            .map_or(0.0, |last| (seconds - last).max(0.0) as f32);
        self.last_seconds = Some(seconds);
        self.ringing = (self.ringing - dt).max(0.0);
        let light = key_light(camera.position);
        let metal = self.metal.as_ref();
        draw_lit([&self.body], metal, camera, light);
        let unit = self.place.unit;
        let top = case_center(unit) + Vec3::Y * CASE_RADIUS * unit;
        let hammer = self.place.placing(top, self.swing());
        draw_lit_placed([&self.hammer], metal, camera, light, hammer);
        let dial = dial_center(unit);
        let angles = [hands.hour, hands.minute, hands.second];
        for ((hand, angle), raise) in self.hands.iter().zip(angles).zip(HAND_RAISE) {
            let at = dial + Vec3::Z * raise * unit;
            draw_lit_placed([hand], metal, camera, light, self.place.placing(at, angle));
        }
        let cap = self
            .place
            .placing(dial + Vec3::Z * HAND_RAISE[2] * unit, 0.0);
        draw_lit_placed([&self.cap], metal, camera, light, cap);
    }
}

/// The place of the clock, for its tests.
#[cfg(test)]
impl Clock {
    /// Where it stands.
    fn place(&self) -> ClockPlace {
        self.place
    }
}

/// A point on the dial at `angle`
/// clockwise from 12 and `share` of the dial's radius out, in the world,
/// for tests of where the hands point.
#[cfg(test)]
fn on_dial(place: ClockPlace, angle: f32, share: f32) -> Vec3 {
    let dial = dial_center(place.unit);
    let point = dial + vec3(angle.sin(), angle.cos(), 0.0) * share * dial_radius(place.unit);
    place.place(point)
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/clock.rs"]
mod tests;
