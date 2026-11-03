//! The Reset and Tracers buttons: two small metal boxes on the table
//! beside the cold side, each with a big domed push button.
//!
//! Each box is nearly square, of brushed steel with rounded corners, a lid
//! on a base with a thin seam between them, and a screw hole in each corner
//! of the lid. A thick glossy black collar on top holds a see-through dome,
//! red for Reset and green for Tracers, and the lid's side is engraved with
//! its name. The boxes lie at their own angles, as if tossed there, out of
//! the way of the engine seen head on. A dome sinks while it is held down,
//! or for a moment when what it does is done by key or on screen, and
//! Tracers glows while the tracers are on. Drawing only: what a press does is the app's.

use std::f32::consts::FRAC_PI_2;

use macroquad::models::{
    Mesh,
    Vertex, //
};
use macroquad::prelude::{
    Camera3D,
    Color,
    Mat3,
    Material,
    UniformType,
    Vec2,
    Vec3,
    vec2,
    vec3, //
};

use super::block::{
    BlockShape,
    key_light, //
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
    load_clear_with, //
};

/// Fragment shader for the domes: tinted see-through plastic, lit like the
/// glass coil, more opaque toward its edges, and brighter and more solid
/// the more `Glow` (0 to 1) says the lamp inside is lit.
const DOME_FRAGMENT: &str = r#"#version 100
precision mediump float;

varying lowp vec4 color;
varying lowp vec2 uv;
varying vec3 world;
varying vec3 surface_normal;

uniform sampler2D Texture;
uniform vec3 Eye;
uniform vec3 Light;
uniform float Glow;

void main() {
    vec3 toward_eye = normalize(Eye - world);
    vec3 n = normalize(surface_normal);
    n = dot(n, toward_eye) < 0.0 ? -n : n;
    float facing = max(dot(n, toward_eye), 0.0);
    float diffuse = max(dot(n, Light), 0.0);
    float shine = pow(max(dot(n, normalize(Light + toward_eye)), 0.0), 50.0);
    float edge = pow(1.0 - facing, 2.0);
    vec3 lamp = mix(color.rgb, vec3(1.0), 0.5) * (0.95 + 0.45 * facing);
    vec3 lit = mix(color.rgb * (0.45 + 0.4 * diffuse), lamp, Glow) + vec3(0.8 * shine);
    float alpha = clamp(color.a * (0.75 + 0.5 * edge) + 0.3 * Glow + 0.5 * shine, 0.0, 1.0);
    gl_FragColor = vec4(lit, alpha);
}"#;

/// What a button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Switch {
    /// Refills the engine with fresh gas.
    Reset,
    /// Shows the tracers.
    Tracers,
}

/// Half the width of a box, in bore radii; it is square from above.
const BOX_HALF_WIDTH: f32 = 0.18;
/// Height of a box's base, in bore radii.
const BASE_HEIGHT: f32 = 0.07;
/// Height of a box's lid, in bore radii.
const LID_HEIGHT: f32 = 0.08;
/// How far the lid stands out past the base all round, making the seam, in
/// bore radii.
const SEAM: f32 = 0.004;
/// Radius of a box's rounded corners, in bore radii.
const CORNER_RADIUS: f32 = 0.03;
/// Steps around each rounded corner.
const CORNER_STEPS: usize = 4;
/// Radius of the screw holes in the lid's corners, in bore radii.
const HOLE_RADIUS: f32 = 0.014;
/// How far in from the lid's sides the screw holes are, in bore radii.
const HOLE_INSET: f32 = 0.035;
/// How far the screw holes' dark faces stand above the lid, so they never
/// fight it for depth, in bore radii.
const HOLE_RAISE: f32 = 0.001;
/// Radius of the inside of the collar, in bore radii.
const COLLAR_INNER: f32 = 0.105;
/// Radius of the outside of the collar, in bore radii.
const COLLAR_OUTER: f32 = 0.13;
/// Height of the collar above the lid, in bore radii.
const COLLAR_HEIGHT: f32 = 0.04;
/// Radius of a dome, in bore radii.
const DOME_RADIUS: f32 = 0.1;
/// How high a dome rises above the collar, unpressed, in bore radii.
const DOME_RISE: f32 = 0.06;
/// How far a dome reaches down into the collar, in bore radii.
const DOME_SKIRT: f32 = 0.03;
/// How far a pressed dome sinks, in bore radii.
const PRESS_DEPTH: f32 = 0.025;
/// Bands from a dome's rim to its top.
const DOME_BANDS: usize = 9;
/// Segments around round parts.
const SEGMENTS: usize = 32;
/// Seconds a dome takes to sink or rise.
const PRESS_SECONDS: f32 = 0.08;
/// Seconds a dome stays down when its switch flips without the mouse on it,
/// as a quick tap would hold it.
const TAP_SECONDS: f32 = 0.15;
/// Seconds a dome's glow takes to come on or go out.
const GLOW_SECONDS: f32 = 0.15;
/// How much bigger a box is than the sizes in bore radii below say.
const BOX_SCALE: f32 = 2.6;
/// Height of the engraved letters, in bore radii.
const LETTER_HEIGHT: f32 = 0.045;
/// Rows in the letters' grid, from the baseline to the cap height.
const LETTER_ROWS: f32 = 6.0;
/// Columns in a letter.
const LETTER_COLUMNS: f32 = 4.0;
/// Room between letters, in the letters' grid.
const LETTER_GAP: f32 = 1.6;
/// Half the width of a letter's stroke, in the letters' grid.
const STROKE_HALF: f32 = 0.45;
/// How far the engraving stands off the lid's face, so it never fights the
/// face for depth, in bore radii.
const ENGRAVING_RAISE: f32 = 0.0015;
/// The boxes' brushed steel.
const STEEL: Color = finish(178, 186, 200, 0.55);
/// The collars' glossy black.
const COLLAR_BLACK: Color = finish(20, 20, 24, 0.95);
/// The screw holes.
const HOLE_DARK: Color = finish(48, 52, 60, 0.1);
/// The engraved letters.
const ENGRAVED: Color = finish(58, 62, 72, 0.15);
/// The Reset dome's red; alpha is its opacity.
const RESET_RED: Color = Color::new(0.95, 0.1, 0.08, 0.72);
/// The Tracers dome's green; alpha is its opacity.
const TRACERS_GREEN: Color = Color::new(0.12, 0.85, 0.3, 0.72);

/// One box: what it switches, where it lies and how it is turned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonBox {
    /// What its button switches.
    pub switch: Switch,
    /// The middle of its underside, on the table.
    pub base: Vec3,
    /// How far it is turned from facing the front, counterclockwise seen
    /// from above, in radians.
    pub yaw: f32,
    /// The bore radius every size is a fraction of.
    unit: f32,
}

impl ButtonBox {
    /// The top of its dome, unpressed: the point to press.
    pub fn dome_top(self) -> Vec3 {
        let height = BASE_HEIGHT + LID_HEIGHT + COLLAR_HEIGHT + DOME_RISE;
        self.base + Vec3::Y * height * self.unit
    }

    /// `local`, a point built with the box's base at the origin and its
    /// engraved side toward +z, placed in the world.
    fn place(self, local: Vec3) -> Vec3 {
        self.base + self.turn() * local
    }

    /// The box's turn about the vertical.
    fn turn(self) -> Mat3 {
        Mat3::from_rotation_y(self.yaw)
    }
}

/// Where each box lies, as (switch, out past the block's cold end, toward
/// the front, turn), in bore radii and radians: one ahead of the other,
/// each turned its own way, the engraved sides toward the usual view.
const PLACES: [(Switch, f32, f32, f32); 2] = [
    (Switch::Reset, 1.45, 0.55, 0.45),
    (Switch::Tracers, 2.05, -0.95, 1.15),
];

/// Both boxes, Reset first, beside the cold end of a block of `shape`.
pub fn button_boxes(shape: BlockShape) -> [ButtonBox; 2] {
    let end = shape.offset + shape.outer;
    PLACES.map(|(switch, out, ahead, yaw)| ButtonBox {
        switch,
        base: vec3(end + out * shape.bore, shape.base, ahead * shape.bore),
        yaw,
        unit: shape.bore * BOX_SCALE,
    })
}

/// One stroke of a letter: from one point of its grid to another.
type Stroke = ((f32, f32), (f32, f32));

/// Strokes of each engraved letter, as segments between points of a grid
/// `LETTER_COLUMNS` wide and `LETTER_ROWS` high, from the baseline.
fn letter(glyph: char) -> &'static [Stroke] {
    match glyph {
        'P' => &[
            ((0.0, 0.0), (0.0, 6.0)),
            ((0.0, 6.0), (3.0, 6.0)),
            ((3.0, 6.0), (4.0, 5.0)),
            ((4.0, 5.0), (4.0, 4.0)),
            ((4.0, 4.0), (3.0, 3.0)),
            ((3.0, 3.0), (0.0, 3.0)),
        ],
        'R' => &[
            ((0.0, 0.0), (0.0, 6.0)),
            ((0.0, 6.0), (3.0, 6.0)),
            ((3.0, 6.0), (4.0, 5.0)),
            ((4.0, 5.0), (4.0, 4.0)),
            ((4.0, 4.0), (3.0, 3.0)),
            ((3.0, 3.0), (0.0, 3.0)),
            ((2.0, 3.0), (4.0, 0.0)),
        ],
        'A' => &[
            ((0.0, 0.0), (2.0, 6.0)),
            ((2.0, 6.0), (4.0, 0.0)),
            ((0.8, 2.4), (3.2, 2.4)),
        ],
        'U' => &[
            ((0.0, 6.0), (0.0, 1.0)),
            ((0.0, 1.0), (1.0, 0.0)),
            ((1.0, 0.0), (3.0, 0.0)),
            ((3.0, 0.0), (4.0, 1.0)),
            ((4.0, 1.0), (4.0, 6.0)),
        ],
        'S' => &[
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
        ],
        'E' => &[
            ((4.0, 6.0), (0.0, 6.0)),
            ((0.0, 6.0), (0.0, 0.0)),
            ((0.0, 0.0), (4.0, 0.0)),
            ((0.0, 3.0), (3.0, 3.0)),
        ],
        'T' => &[((0.0, 6.0), (4.0, 6.0)), ((2.0, 6.0), (2.0, 0.0))],
        'C' => &[
            ((4.0, 5.0), (3.0, 6.0)),
            ((3.0, 6.0), (1.0, 6.0)),
            ((1.0, 6.0), (0.0, 5.0)),
            ((0.0, 5.0), (0.0, 1.0)),
            ((0.0, 1.0), (1.0, 0.0)),
            ((1.0, 0.0), (3.0, 0.0)),
            ((3.0, 0.0), (4.0, 1.0)),
        ],
        _ => &[],
    }
}

/// The name engraved on a box.
fn name(switch: Switch) -> &'static str {
    match switch {
        Switch::Reset => "RESET",
        Switch::Tracers => "TRACERS",
    }
}

/// The color of a box's dome.
fn dome_color(switch: Switch) -> Color {
    match switch {
        Switch::Reset => RESET_RED,
        Switch::Tracers => TRACERS_GREEN,
    }
}

/// A rounded square, `half` across, with corners of `radius`.
fn rounded_square(half: f32, radius: f32) -> Vec<Vec2> {
    let inner = half - radius;
    let mut outline = Vec::new();
    for (quarter, corner) in [
        vec2(inner, inner),
        vec2(-inner, inner),
        vec2(-inner, -inner),
        vec2(inner, -inner),
    ]
    .into_iter()
    .enumerate()
    {
        for step in 0..=CORNER_STEPS {
            let angle = FRAC_PI_2 * (quarter as f32 + step as f32 / CORNER_STEPS as f32);
            outline.push(corner + Vec2::from_angle(angle) * radius);
        }
    }
    outline
}

/// Builds a box's steel body, collar, screw holes and engraving, with its
/// base at the origin and its engraved side toward +z, in bore radii of
/// `unit`.
fn build_body(switch: Switch, unit: f32) -> Solid {
    let mut solid = Solid::new();
    let half = BOX_HALF_WIDTH * unit;
    let up = Frame::along(Vec3::ZERO, Vec3::Y);
    let lid_bottom = BASE_HEIGHT * unit;
    let lid_top = lid_bottom + LID_HEIGHT * unit;
    solid.plate(
        up,
        &rounded_square(half, CORNER_RADIUS * unit),
        lid_bottom,
        STEEL,
    );
    let lid_half = half + SEAM * unit;
    solid.plate(
        Frame {
            origin: Vec3::Y * lid_bottom,
            ..up
        },
        &rounded_square(lid_half, (CORNER_RADIUS + SEAM) * unit),
        LID_HEIGHT * unit,
        STEEL,
    );
    // The screw holes, dark discs in the lid's corners.
    let hole = lid_half - HOLE_INSET * unit;
    for corner in [
        vec2(hole, hole),
        vec2(-hole, hole),
        vec2(-hole, -hole),
        vec2(hole, -hole),
    ] {
        let center = vec3(corner.x, lid_top, corner.y);
        solid.cylinder(
            center,
            center + Vec3::Y * HOLE_RAISE * unit,
            HOLE_RADIUS * unit,
            SEGMENTS / 2,
            HOLE_DARK,
        );
    }
    // The collar: a thick ring with a rounded top.
    let (inner, outer) = (COLLAR_INNER * unit, COLLAR_OUTER * unit);
    let height = COLLAR_HEIGHT * unit;
    let shoulder = (outer - inner) / 2.0;
    solid.lathe(
        Frame::along(Vec3::Y * lid_top, Vec3::Y),
        &[
            turn(inner, 0.0, COLLAR_BLACK),
            turn(outer, 0.0, COLLAR_BLACK),
            turn(outer, height - shoulder, COLLAR_BLACK),
            turn(outer - shoulder / 2.0, height, COLLAR_BLACK),
            turn(inner + shoulder / 2.0, height, COLLAR_BLACK),
            turn(inner, height - shoulder, COLLAR_BLACK),
            turn(inner, 0.0, COLLAR_BLACK),
        ],
        SEGMENTS,
    );
    engrave(
        &mut solid,
        name(switch),
        lid_half,
        (lid_bottom + lid_top) / 2.0,
        unit,
    );
    solid
}

/// Engraves `text` across the box's +z face, `face` out from the middle,
/// centered at `middle_y`.
fn engrave(solid: &mut Solid, text: &str, face: f32, middle_y: f32, unit: f32) {
    let grid = LETTER_HEIGHT * unit / LETTER_ROWS;
    let advance = LETTER_COLUMNS + LETTER_GAP;
    let count = text.chars().count() as f32;
    let width = count * advance - LETTER_GAP;
    let left = -width / 2.0;
    let z = face + ENGRAVING_RAISE * unit;
    let half = STROKE_HALF * grid;
    for (index, glyph) in text.chars().enumerate() {
        let start = left + index as f32 * advance;
        for &((x0, y0), (x1, y1)) in letter(glyph) {
            let to_world = |x: f32, y: f32| {
                vec3(
                    (start + x) * grid,
                    middle_y + (y - LETTER_ROWS / 2.0) * grid,
                    z,
                )
            };
            let (a, b) = (to_world(x0, y0), to_world(x1, y1));
            let along = (b - a).normalize_or_zero() * half;
            let across = vec3(-along.y, along.x, 0.0);
            let corners = [
                a - along - across,
                b + along - across,
                b + along + across,
                a - along + across,
            ];
            solid.quad(corners, [Vec3::Z; 4], [ENGRAVED; 4]);
        }
    }
}

/// Builds a box's dome, unpressed, with the box's base at the origin, in
/// bore radii of `unit`.
fn build_dome(switch: Switch, unit: f32) -> Solid {
    let mut solid = Solid::new();
    let color = dome_color(switch);
    let seat = (BASE_HEIGHT + LID_HEIGHT + COLLAR_HEIGHT) * unit;
    let (radius, rise) = (DOME_RADIUS * unit, DOME_RISE * unit);
    let mut outline = vec![turn(radius, -DOME_SKIRT * unit, color)];
    for band in 0..=DOME_BANDS {
        let angle = FRAC_PI_2 * band as f32 / DOME_BANDS as f32;
        outline.push(turn(radius * angle.cos(), rise * angle.sin(), color));
    }
    solid.lathe(Frame::along(Vec3::Y * seat, Vec3::Y), &outline, SEGMENTS);
    solid
}

/// `solid`'s corners placed in the world for `box_`, as vertices.
fn placed(solid: Solid, box_: ButtonBox) -> (Vec<Vertex>, Vec<u16>) {
    let turn = box_.turn();
    let vertices = solid
        .vertices
        .into_iter()
        .map(|mut vertex| {
            vertex.position = box_.place(vertex.position);
            vertex.normal = (turn * vertex.normal.truncate()).extend(0.0);
            vertex
        })
        .collect();
    (vertices, solid.indices)
}

/// Whether the see-through material is ready.
enum Clear {
    /// Not built yet.
    Untried,
    /// Built.
    Ready(Material),
    /// Its shader could not be built; the domes are left out.
    Unavailable,
}

/// Both boxes.
pub struct Buttons {
    /// Both bodies, in the world, one mesh.
    bodies: Mesh,
    /// Each dome as built, unpressed, in the world.
    rest: [Vec<Vertex>; 2],
    /// Each dome as posed.
    domes: [Mesh; 2],
    /// How far each dome is pressed in, from 0 to 1.
    pressed: [f32; 2],
    /// How brightly each dome glows, from 0 to 1.
    glow: [f32; 2],
    /// Whether each switch was on at the last pose, if there was one.
    was_on: Option<[bool; 2]>,
    /// Seconds each dome has left of a tap.
    tapped: [f32; 2],
    /// The clock at the last pose, if there was one.
    last_seconds: Option<f64>,
    /// The lit-metal material, if it can be built.
    metal: Option<Material>,
    /// The domes' material, once tried.
    clear: Clear,
    /// The bore radius every size is a fraction of, times `BOX_SCALE`.
    unit: f32,
}

/// Whether each button's switch is on and whether its dome is held down
/// with the mouse, Reset first (Reset is never on).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DomeStates {
    /// Whether each switch is on.
    pub on: [bool; 2],
    /// Whether each dome is held down.
    pub held: [bool; 2],
}

impl Buttons {
    /// Builds both boxes beside a block of `shape`, unpressed.
    ///
    /// It needs a window, for the material; `build` does the rest without
    /// one.
    pub fn new(shape: BlockShape) -> Self {
        let mut buttons = Self::build(shape);
        buttons.metal = load_metal();
        if buttons.metal.is_none() {
            // Drawn in plain colors, alpha means transparency: make it solid.
            for vertex in &mut buttons.bodies.vertices {
                vertex.color[3] = u8::MAX;
            }
        }
        buttons
    }

    /// Builds both boxes beside a block of `shape`, unpressed, without a
    /// material.
    ///
    /// # Panics
    ///
    /// Panics if they would not fit Macroquad's buffers.
    fn build(shape: BlockShape) -> Self {
        let boxes = button_boxes(shape);
        let mut bodies = empty_mesh();
        for box_ in boxes {
            let (vertices, indices) = placed(build_body(box_.switch, box_.unit), box_);
            let start = bodies.vertices.len() as u16;
            bodies.vertices.extend(vertices);
            bodies
                .indices
                .extend(indices.into_iter().map(|corner| corner + start));
        }
        assert!(bodies.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
        assert!(bodies.indices.len() < DRAW_CALL_INDEX_CAPACITY);
        let built = boxes.map(|box_| placed(build_dome(box_.switch, box_.unit), box_));
        let rest = [built[0].0.clone(), built[1].0.clone()];
        let domes = built.map(|(vertices, indices)| Mesh {
            vertices,
            indices,
            texture: None,
        });
        Self {
            bodies,
            rest,
            domes,
            pressed: [0.0; 2],
            last_seconds: None,
            metal: None,
            clear: Clear::Untried,
            unit: shape.bore * BOX_SCALE,
            glow: [0.0; 2],
            was_on: None,
            tapped: [0.0; 2],
        }
    }

    /// Sinks each dome while it is `held` or for a tap after its switch
    /// flips, lets it rise otherwise, and lights it while its switch is on,
    /// at `seconds` on the clock.
    fn pose(&mut self, states: DomeStates, seconds: f64) {
        let dt = self
            .last_seconds
            .map_or(0.0, |last| (seconds - last).max(0.0) as f32);
        self.last_seconds = Some(seconds);
        let was_on = self.was_on.replace(states.on).unwrap_or(states.on);
        let unit = self.unit;
        for (index, was_on) in was_on.into_iter().enumerate() {
            if was_on != states.on[index] {
                self.tapped[index] = TAP_SECONDS;
            }
            self.tapped[index] = (self.tapped[index] - dt).max(0.0);
            let down = states.held[index] || self.tapped[index] > 0.0;
            let pressed = &mut self.pressed[index];
            *pressed = approach(*pressed, down, dt / PRESS_SECONDS);
            let sink = Vec3::Y * *pressed * PRESS_DEPTH * unit;
            let glow = &mut self.glow[index];
            *glow = approach(*glow, states.on[index], dt / GLOW_SECONDS);
            let rest = &self.rest[index];
            for (vertex, rest) in self.domes[index].vertices.iter_mut().zip(rest) {
                vertex.position = rest.position - sink;
            }
        }
    }

    /// Draws both boxes' bodies seen from `camera`, with the solid parts.
    pub fn draw_bodies(&self, camera: &Camera3D) {
        let light = key_light(camera.position);
        draw_lit([&self.bodies], self.metal.as_ref(), camera, light);
    }

    /// Draws both domes seen from `camera` as `states` press and light
    /// them, at `seconds` on the clock; call it after everything solid.
    pub fn draw_domes(&mut self, states: DomeStates, seconds: f64, camera: &Camera3D) {
        self.pose(states, seconds);
        if let Clear::Untried = self.clear {
            self.clear = load_clear_with(DOME_FRAGMENT, &[("Glow", UniformType::Float1)])
                .map_or(Clear::Unavailable, Clear::Ready);
        }
        let Clear::Ready(material) = &self.clear else {
            return;
        };
        let light = key_light(camera.position);
        for (dome, glow) in self.domes.iter().zip(self.glow) {
            material.set_uniform("Glow", glow);
            draw_lit([dome], Some(material), camera, light);
        }
    }
}

/// `value` moved toward 1 if `up`, or 0 otherwise, by at most `step`.
fn approach(value: f32, up: bool, step: f32) -> f32 {
    let target = if up { 1.0 } else { 0.0 };
    value + (target - value).clamp(-step, step)
}

/// An empty mesh.
fn empty_mesh() -> Mesh {
    Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        texture: None,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/buttons.rs"]
mod tests;
