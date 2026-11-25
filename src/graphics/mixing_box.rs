//! The mixing box on the table, for the second law.
//!
//! A glass box with a brass frame stands on a dark plinth at the front of
//! the table, beyond the hot side, turned a little toward the usual view.
//! A steel divider stands across its middle, a grip on top, hot balls
//! (red) on its left and cold ones (blue) on its right. Lifted, the
//! divider slides up through a slot in the lid and the gases mix, each ball
//! turning purple as it comes to the box's one temperature; dropped again,
//! it sorts nothing back out. A small dial on the plinth's end, marked S,
//! shows the box's entropy rising as they mix and never falling back (see
//! `sim::mixing`).
//!
//! Drawing only: what a press does is the app's.

use std::f32::consts::{
    FRAC_PI_2,
    PI, //
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
    UniformType,
    Vec2,
    Vec3,
    Vec4,
    vec2,
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
use super::windows::GLASS_FRAGMENT;
use super::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY,
    draw_lit,
    draw_lit_placed,
    load_clear_with, //
};
use crate::sim::mixing::{
    BALL_RADIUS,
    DIVIDER_HALF,
    HALF_DEPTH,
    HALF_WIDTH,
    HEIGHT,
    MixingGas, //
};

/// Where the plinth's middle stands on the table, across and toward the
/// front, in bore radii: at the front, beyond the hot side.
const SPOT: Vec2 = vec2(-5.0, 3.3);
/// How far the box is turned toward the usual view, in radians.
const YAW: f32 = 0.3;
/// Height of the plinth, in bore radii.
const PLINTH_HEIGHT: f32 = 0.1;
/// How far the plinth reaches past the frame, in bore radii.
const PLINTH_LIP: f32 = 0.06;
/// Half the frame's bars' width, in bore radii.
const BAR_HALF: f32 = 0.03;
/// How far the divider stands above the lid when down, to its grip's
/// posts, in bore radii.
const DIVIDER_ABOVE: f32 = 0.08;
/// Height of the grip's bar above the divider's top, in bore radii.
const GRIP_RISE: f32 = 0.08;
/// Radius of the grip's bar and posts, in bore radii.
const GRIP_RADIUS: f32 = 0.018;
/// How far the gap in the lid reaches either side of the divider, in bore
/// radii.
const SLOT_HALF: f32 = 0.03;
/// How far the entropy dial's stand is past the frame's end, to its
/// middle, in bore radii.
const DIAL_OUT: f32 = 0.32;
/// Radius of the entropy dial's face, in bore radii.
const DIAL_RADIUS: f32 = 0.2;
/// Width of the dial's rim, in bore radii.
const DIAL_RIM: f32 = 0.035;
/// Depth of the dial's case, in bore radii.
const DIAL_DEPTH: f32 = 0.06;
/// Height of the dial's middle above the plinth, in bore radii.
const DIAL_HEIGHT: f32 = 0.38;
/// Radius of the dial's post, in bore radii.
const DIAL_POST: f32 = 0.022;
/// Angle the dial's scale sweeps, in radians.
const DIAL_SWEEP: f32 = 1.5 * PI;
/// Short ticks across the dial's scale.
const DIAL_STEPS: usize = 10;
/// Every how many short ticks a long one stands.
const DIAL_MAJOR: usize = 5;
/// Inner ends of the long and short ticks, and their outer ends, as
/// shares of the dial's radius.
const TICKS: (f32, f32, f32) = (0.7, 0.8, 0.9);
/// Half the width of a tick, as a share of the dial's radius.
const TICK_HALF: f32 = 0.025;
/// Height of the S, as a share of the dial's radius.
const LETTER_HEIGHT: f32 = 0.42;
/// Half the width of the S's stroke, in its grid.
const LETTER_STROKE: f32 = 0.6;
/// Rows of the S's grid.
const LETTER_ROWS: f32 = 6.0;
/// Columns of the S's grid.
const LETTER_COLUMNS: f32 = 4.0;
/// How far marks and the needle stand off the dial's face, in bore radii.
const MARK_RAISE: f32 = 0.004;
/// How far the needle stands off the face, in bore radii.
const NEEDLE_RAISE: f32 = 0.03;
/// Length of the needle, as a share of the dial's radius.
const NEEDLE_LENGTH: f32 = 0.82;
/// Half the needle's width at its root, as a share of the dial's radius.
const NEEDLE_HALF: f32 = 0.06;
/// Seconds for the needle to make up most of the way to a new reading.
const NEEDLE_SECONDS: f32 = 0.5;
/// Segments round the round parts.
const SEGMENTS: usize = 20;
/// Segments round a ball.
const BALL_SEGMENTS: usize = 8;
/// Bands from a ball's bottom to its top.
const BALL_BANDS: usize = 4;
/// The plinth.
const PLINTH_WOOD: Color = finish(58, 40, 30, 0.25);
/// The frame's bars, the grip and the dial's post.
const BRASS: Color = finish(206, 164, 72, 0.8);
/// The divider.
const DIVIDER_STEEL: Color = finish(150, 158, 172, 0.7);
/// The dial's face.
const FACE_ENAMEL: Color = finish(248, 244, 230, 0.25);
/// The dial's rim and needle.
const DIAL_PURPLE: Color = finish(146, 64, 196, 0.6);
/// The dial's ticks and letter.
const MARK_INK: Color = finish(32, 40, 72, 0.1);
/// The dial's case.
const CASE_STEEL: Color = finish(70, 74, 84, 0.4);
/// A ball at the hot side's first temperature.
const HOT_BALL: Color = finish(232, 60, 48, 0.9);
/// A ball between the two, at the box's one temperature.
const EVEN_BALL: Color = finish(168, 78, 206, 0.9);
/// A ball at the cold side's first temperature.
const COLD_BALL: Color = finish(56, 110, 240, 0.9);
/// The glass's tint.
const GLASS_TINT: Color = Color::from_rgba(210, 232, 255, 255);
/// The letter S's strokes in its grid.
const LETTER_S: [((f32, f32), (f32, f32)); 11] = [
    ((4.0, 5.0), (3.0, 6.0)),
    ((3.0, 6.0), (1.0, 6.0)),
    ((1.0, 6.0), (0.0, 5.0)),
    ((0.0, 5.0), (0.0, 4.0)),
    ((0.0, 4.0), (1.0, 3.0)),
    ((1.0, 3.0), (3.0, 3.0)),
    ((3.0, 3.0), (4.0, 2.0)),
    ((4.0, 2.0), (4.0, 1.0)),
    ((4.0, 1.0), (3.0, 0.0)),
    ((3.0, 0.0), (1.0, 0.0)),
    ((1.0, 0.0), (0.0, 1.0)),
];

/// Where the mixing box stands: the middle of its inside's floor, turned
/// by `yaw` round the upright, with the block's size, in which its own
/// sizes are measured.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MixingBoxPlace {
    /// The middle of the inside's floor, on the plinth.
    pub floor: Vec3,
    /// How far it is turned, in radians, its front from +z toward +x.
    pub yaw: f32,
    /// The bore's radius, the unit of its sizes.
    pub unit: f32,
}

impl MixingBoxPlace {
    /// `local`, in bore radii from the middle of the inside's floor, x
    /// across, y up, z toward the front, in the world.
    pub fn at(self, local: Vec3) -> Vec3 {
        self.floor + Mat3::from_rotation_y(self.yaw) * (local * self.unit)
    }

    /// `local`, a way in the box's own terms, in the world.
    pub fn way(self, local: Vec3) -> Vec3 {
        Mat3::from_rotation_y(self.yaw) * local
    }

    /// The middle of the inside.
    pub fn middle(self) -> Vec3 {
        self.at(vec3(0.0, HEIGHT / 2.0, 0.0))
    }

    /// The top of the divider's grip, with the divider down.
    pub fn grip(self) -> Vec3 {
        self.at(vec3(0.0, divider_top() + GRIP_RISE, 0.0))
    }

    /// The ends of the line along the box's middle, across it, and how
    /// far round that line it and its plinth reach, from the dial's end
    /// to the other, for walking round it.
    pub fn footprint(self) -> (Vec3, Vec3, f32) {
        let depth = HALF_DEPTH + BAR_HALF + PLINTH_LIP;
        // The line's ends stand in from the plinth's ends by half its
        // depth, so the round ends reach past the frame's corners.
        let inset = depth / 2.0;
        (
            self.at(vec3(-plinth_left() + inset, 0.0, 0.0)),
            self.at(vec3(plinth_right() - inset, 0.0, 0.0)),
            depth * self.unit,
        )
    }

    /// The height of the lid's top above the table.
    pub fn top(self) -> f32 {
        self.at(vec3(0.0, HEIGHT + BAR_HALF, 0.0)).y
    }
}

/// Where the mixing box stands on the table under a block of `shape`.
pub fn mixing_box_place(shape: BlockShape) -> MixingBoxPlace {
    let unit = shape.bore;
    MixingBoxPlace {
        floor: vec3(
            SPOT.x * unit,
            shape.base + PLINTH_HEIGHT * unit,
            SPOT.y * unit,
        ),
        yaw: YAW,
        unit,
    }
}

/// Half the frame's width across, to the outside of its bars, in bore
/// radii.
fn frame_half_width() -> f32 {
    HALF_WIDTH + BAR_HALF
}

/// How far the plinth reaches left of the box's middle, in bore radii.
fn plinth_left() -> f32 {
    frame_half_width() + PLINTH_LIP
}

/// How far the plinth reaches right of the box's middle, past the dial,
/// in bore radii.
fn plinth_right() -> f32 {
    frame_half_width() + DIAL_OUT + DIAL_RADIUS + PLINTH_LIP
}

/// The height of the divider's top above the inside's floor, down, in bore
/// radii.
fn divider_top() -> f32 {
    HEIGHT + DIVIDER_ABOVE
}

/// The color of a ball `share` of the way from the cold side's first
/// temperature to the hot side's, through the purple of the two mixed.
pub fn ball_color(share: f32) -> Color {
    let blend = |from: Color, to: Color, share: f32| {
        Color::new(
            from.r + (to.r - from.r) * share,
            from.g + (to.g - from.g) * share,
            from.b + (to.b - from.b) * share,
            from.a + (to.a - from.a) * share,
        )
    };
    let share = share.clamp(0.0, 1.0);
    if share < 0.5 {
        blend(COLD_BALL, EVEN_BALL, 2.0 * share)
    } else {
        blend(EVEN_BALL, HOT_BALL, 2.0 * share - 1.0)
    }
}

/// The plinth, the frame, the dial's case, face and marks, all in the box's
/// own terms in bore radii.
fn body_solid() -> Solid {
    let mut solid = Solid::new();
    let width = frame_half_width();
    let depth = HALF_DEPTH + BAR_HALF;
    // The plinth, under the frame and the dial's stand.
    solid.bar(
        vec3(-plinth_left(), -PLINTH_HEIGHT / 2.0, 0.0),
        vec3(plinth_right(), -PLINTH_HEIGHT / 2.0, 0.0),
        Vec3::Y,
        (PLINTH_HEIGHT / 2.0, PLINTH_HEIGHT / 2.0),
        depth + PLINTH_LIP,
        PLINTH_WOOD,
    );
    // The frame: four posts and the bars round the bottom and the top.
    let corner = |x: f32, z: f32, y: f32| vec3(x * width, y, z * depth);
    let half = (BAR_HALF, BAR_HALF);
    for (x, z) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        solid.bar(
            corner(x, z, 0.0),
            corner(x, z, HEIGHT + BAR_HALF),
            Vec3::X,
            half,
            BAR_HALF,
            BRASS,
        );
    }
    for y in [BAR_HALF, HEIGHT] {
        for z in [-1.0, 1.0] {
            solid.bar(
                corner(-1.0, z, y),
                corner(1.0, z, y),
                Vec3::Y,
                half,
                BAR_HALF,
                BRASS,
            );
        }
        for x in [-1.0, 1.0] {
            solid.bar(
                corner(x, -1.0, y),
                corner(x, 1.0, y),
                Vec3::Y,
                half,
                BAR_HALF,
                BRASS,
            );
        }
    }
    // The entropy dial on its post at the plinth's end.
    let dial = vec3(width + DIAL_OUT, DIAL_HEIGHT, 0.0);
    solid.cylinder(vec3(dial.x, 0.0, 0.0), dial, DIAL_POST, SEGMENTS / 2, BRASS);
    let rim = DIAL_RADIUS + DIAL_RIM;
    let back = -DIAL_DEPTH / 2.0;
    let face = DIAL_DEPTH / 2.0;
    solid.lathe(
        Frame::along(vec3(dial.x, dial.y, back), Vec3::Z),
        &[
            turn(0.0, 0.0, CASE_STEEL),
            turn(rim, 0.0, CASE_STEEL),
            turn(rim, 0.0, DIAL_PURPLE),
            turn(rim, face - back + DIAL_RIM / 2.0, DIAL_PURPLE),
            turn(DIAL_RADIUS, face - back + DIAL_RIM / 2.0, DIAL_PURPLE),
            turn(DIAL_RADIUS, face - back, FACE_ENAMEL),
            turn(0.0, face - back, FACE_ENAMEL),
        ],
        SEGMENTS,
    );
    let middle = vec3(dial.x, dial.y, face + MARK_RAISE);
    let on_face = |share: f32, reach: f32| {
        let turned = dial_turn(share);
        middle + vec3(turned.sin(), turned.cos(), 0.0) * reach * DIAL_RADIUS
    };
    let stroke = |solid: &mut Solid, a: Vec3, b: Vec3, half: f32| {
        let along = (b - a).normalize_or_zero() * half;
        let across = vec3(-along.y, along.x, 0.0);
        let corners = [
            a - along - across,
            b + along - across,
            b + along + across,
            a - along + across,
        ];
        solid.quad(corners, [Vec3::Z; 4], [MARK_INK; 4]);
    };
    let (long, short, outer) = TICKS;
    for step in 0..=DIAL_STEPS {
        let share = step as f32 / DIAL_STEPS as f32;
        let inner = if step % DIAL_MAJOR == 0 { long } else { short };
        let (a, b) = (on_face(share, inner), on_face(share, outer));
        stroke(&mut solid, a, b, TICK_HALF * DIAL_RADIUS);
    }
    let grid = LETTER_HEIGHT * DIAL_RADIUS / LETTER_ROWS;
    for ((x0, y0), (x1, y1)) in LETTER_S {
        let at = |x: f32, y: f32| {
            middle
                + vec3(
                    (x - LETTER_COLUMNS / 2.0) * grid,
                    (y - LETTER_ROWS / 2.0) * grid,
                    0.0,
                )
        };
        stroke(&mut solid, at(x0, y0), at(x1, y1), LETTER_STROKE * grid);
    }
    solid
}

/// The divider, down, with its grip, in the box's own terms.
fn divider_solid() -> Solid {
    let mut solid = Solid::new();
    let top = divider_top();
    solid.bar(
        vec3(0.0, 0.0, 0.0),
        vec3(0.0, top, 0.0),
        Vec3::Z,
        (HALF_DEPTH, HALF_DEPTH),
        DIVIDER_HALF,
        DIVIDER_STEEL,
    );
    let reach = HALF_DEPTH * 0.6;
    let bar_y = top + GRIP_RISE;
    solid.cylinder(
        vec3(0.0, bar_y, -reach),
        vec3(0.0, bar_y, reach),
        GRIP_RADIUS,
        SEGMENTS / 2,
        BRASS,
    );
    for z in [-reach, reach] {
        solid.cylinder(
            vec3(0.0, top, z),
            vec3(0.0, bar_y, z),
            GRIP_RADIUS,
            SEGMENTS / 2,
            BRASS,
        );
    }
    solid
}

/// The needle, pointing straight up from its pin at the origin, facing +z.
fn needle_solid() -> Solid {
    let mut solid = Solid::new();
    let half = NEEDLE_HALF * DIAL_RADIUS;
    let tip = vec3(0.0, NEEDLE_LENGTH * DIAL_RADIUS, 0.0);
    let corners = [
        vec3(-half, 0.0, 0.0),
        vec3(0.0, -half, 0.0),
        vec3(half, 0.0, 0.0),
        tip,
    ];
    solid.quad(corners, [Vec3::Z; 4], [DIAL_PURPLE; 4]);
    solid.lathe(
        Frame::along(Vec3::ZERO, Vec3::Z),
        &[
            turn(1.5 * half, 0.0, BRASS),
            turn(1.5 * half, MARK_RAISE * 4.0, BRASS),
            turn(0.0, MARK_RAISE * 4.0, BRASS),
        ],
        SEGMENTS / 2,
    );
    solid
}

/// The dial's needle's turn for `share` of its scale, in radians clockwise
/// from straight up.
pub fn dial_turn(share: f32) -> f32 {
    (share - 0.5) * DIAL_SWEEP
}

/// The glass: the front, back and ends, and the lid either side of the
/// divider's slot, each as a panel whose texture coordinates run from -1
/// to 1 across it, in the box's own terms.
fn glass_mesh() -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut panel = |corners: [Vec3; 4], out: Vec3| {
        let first = vertices.len() as u16;
        let uvs = [
            vec2(-1.0, -1.0),
            vec2(1.0, -1.0),
            vec2(1.0, 1.0),
            vec2(-1.0, 1.0),
        ];
        for (corner, uv) in corners.into_iter().zip(uvs) {
            let mut vertex = Vertex::new2(corner, uv, GLASS_TINT);
            vertex.normal = Vec4::new(out.x, out.y, out.z, 0.0);
            vertices.push(vertex);
        }
        indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    };
    let (x, y, z) = (HALF_WIDTH, HEIGHT, HALF_DEPTH);
    for side in [-1.0_f32, 1.0] {
        // Front and back.
        panel(
            [
                vec3(-x, 0.0, side * z),
                vec3(x, 0.0, side * z),
                vec3(x, y, side * z),
                vec3(-x, y, side * z),
            ],
            Vec3::Z * side,
        );
        // The ends.
        panel(
            [
                vec3(side * x, 0.0, -z),
                vec3(side * x, 0.0, z),
                vec3(side * x, y, z),
                vec3(side * x, y, -z),
            ],
            Vec3::X * side,
        );
        // The lid, this side of the slot.
        let inner = side * (DIVIDER_HALF + SLOT_HALF);
        panel(
            [
                vec3(inner, y, -z),
                vec3(side * x, y, -z),
                vec3(side * x, y, z),
                vec3(inner, y, z),
            ],
            Vec3::Y,
        );
    }
    Mesh {
        vertices,
        indices,
        texture: None,
    }
}

/// `solid` on the graphics card as it is, solid in plain colors where
/// there is no lit-metal material, whose colors' alpha means polish.
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

/// `vertices`, in the box's own terms in bore radii, placed in the world
/// at `place`.
fn place_vertices(vertices: &mut [Vertex], place: MixingBoxPlace) {
    for vertex in vertices {
        vertex.position = place.at(vertex.position);
        let normal = place.way(vertex.normal.truncate());
        vertex.normal = Vec4::new(normal.x, normal.y, normal.z, 0.0);
    }
}

/// `solid`, in the box's own terms, placed at `place` and on the graphics
/// card.
fn upload_placed(mut solid: Solid, place: MixingBoxPlace, plain: bool) -> GpuMesh {
    place_vertices(&mut solid.vertices, place);
    upload(solid, plain)
}

/// One ball, round its middle at the origin, in `color`.
fn ball_solid(solid: &mut Solid, middle: Vec3, radius: f32, color: Color) {
    let outline: Vec<_> = (0..=BALL_BANDS)
        .map(|band| {
            let angle = -FRAC_PI_2 + PI * band as f32 / BALL_BANDS as f32;
            turn(radius * angle.cos(), radius * (angle.sin() + 1.0), color)
        })
        .collect();
    let bottom = middle - Vec3::Y * radius;
    solid.lathe(Frame::along(bottom, Vec3::Y), &outline, BALL_SEGMENTS);
}

/// The mixing box: its gas, and what draws it.
pub struct MixingBox {
    /// Where it stands.
    place: MixingBoxPlace,
    /// The gas inside and the divider.
    gas: MixingGas,
    /// The plinth, frame and dial.
    body: GpuMesh,
    /// The divider, down.
    divider: GpuMesh,
    /// The dial's needle, pointing straight up from its pin at the origin.
    needle: GpuMesh,
    /// The balls, written out each frame.
    balls: GpuMesh,
    /// The glass, placed.
    glass: GpuMesh,
    /// Where on its scale the dial's needle points, from 0 to 1.
    reading: f32,
    /// The clock at the last `follow`, if there was one.
    last_seconds: Option<f64>,
    /// The lit-metal material, if it can be built.
    metal: Option<GpuMaterial>,
    /// The glass's material: untried, ready, or not to be had.
    glass_material: Option<Option<GpuMaterial>>,
}

impl MixingBox {
    /// The mixing box on the table under a block of `shape`, sorted.
    pub fn new(shape: BlockShape) -> Self {
        let place = mixing_box_place(shape);
        let metal = load_metal();
        let plain = metal.is_none();
        let mut glass = glass_mesh();
        place_vertices(&mut glass.vertices, place);
        // The needle turns about its pin, in the box's turn, so it is
        // placed when drawn; only its size is the block's.
        let mut needle = needle_solid();
        for vertex in &mut needle.vertices {
            vertex.position *= place.unit;
        }
        Self {
            place,
            gas: MixingGas::new(),
            body: upload_placed(body_solid(), place, plain),
            divider: upload_placed(divider_solid(), place, plain),
            needle: upload(needle, plain),
            balls: GpuMesh::new(Mesh {
                vertices: Vec::new(),
                indices: Vec::new(),
                texture: None,
            }),
            glass: GpuMesh::new(glass),
            reading: 0.0,
            last_seconds: None,
            metal,
            glass_material: None,
        }
    }

    /// Where it stands.
    pub fn place(&self) -> MixingBoxPlace {
        self.place
    }

    /// Lifts the divider if it is down, or drops it if it is up.
    pub fn toggle(&mut self) {
        self.gas.toggle();
    }

    /// Whether the divider is up, or on its way up.
    pub fn raised(&self) -> bool {
        self.gas.raised()
    }

    /// Sorts the gases back out, the divider down.
    pub fn sort(&mut self) {
        self.gas.sort();
        self.reading = self.gas.entropy_share();
    }

    /// Runs the gas on to `seconds` on the effects' clock and swings the
    /// dial's needle toward its entropy.
    pub fn follow(&mut self, seconds: f64) {
        let dt = self
            .last_seconds
            .map_or(0.0, |last| (seconds - last).max(0.0) as f32);
        self.last_seconds = Some(seconds);
        self.gas.advance(dt);
        let keep = (-dt / NEEDLE_SECONDS).exp();
        let target = self.gas.entropy_share();
        self.reading = target + (self.reading - target) * keep;
    }

    /// Draws the plinth, frame, dial, divider and balls seen from
    /// `camera`; the glass comes later, with everything see-through.
    pub fn draw(&mut self, camera: &Camera3D) {
        let light = key_light(camera.position);
        let metal = self.metal.as_ref();
        let place = self.place;
        let mut solid = Solid::new();
        let (hot, cold) = MixingGas::first_warmths();
        for ball in self.gas.balls() {
            let share = (ball.warmth - cold) / (hot - cold);
            let at = place.at(ball.position);
            ball_solid(&mut solid, at, BALL_RADIUS * place.unit, ball_color(share));
        }
        let mesh = self.balls.edit();
        mesh.vertices = solid.vertices;
        mesh.indices = solid.indices;
        if metal.is_none() {
            for vertex in &mut mesh.vertices {
                vertex.color[3] = u8::MAX;
            }
        }
        draw_lit([&self.body, &self.balls], metal, camera, light);
        let lift = self.gas.lift() * (HEIGHT + BALL_RADIUS) * place.unit;
        let raised = Mat4::from_translation(Vec3::Y * lift);
        draw_lit_placed([&self.divider], metal, camera, light, raised);
        let pin = place.at(vec3(
            frame_half_width() + DIAL_OUT,
            DIAL_HEIGHT,
            DIAL_DEPTH / 2.0 + NEEDLE_RAISE,
        ));
        let turned = Mat4::from_translation(pin)
            * Mat4::from_rotation_y(place.yaw)
            * Mat4::from_rotation_z(-dial_turn(self.reading));
        draw_lit_placed([&self.needle], metal, camera, light, turned);
    }

    /// Draws the glass seen from `camera`, over everything behind it.
    pub fn draw_glass(&mut self, camera: &Camera3D) {
        let material = self.glass_material.get_or_insert_with(|| {
            load_clear_with(GLASS_FRAGMENT, &[("Pane", UniformType::Float4)])
        });
        let Some(material) = material.as_ref() else {
            return;
        };
        // Whole panels: no corners cut, no seal.
        material.set_uniform("Pane", Vec4::new(1.0, 1.0, 0.0, 0.0));
        draw_lit(
            [&self.glass],
            Some(material),
            camera,
            key_light(camera.position),
        );
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/mixing_box.rs"]
mod tests;
