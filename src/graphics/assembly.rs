//! The moving parts above the block, as solid metal: an aluminium piston
//! in each bore (a hollow cup with ring grooves and a wrist pin), a steel
//! connecting rod from each piston to its crank, the crankshaft with its
//! webs and crank pins, the brass flywheel between the cranks, and two
//! painted bearing stands on the block that carry the shaft.
//!
//! Each part is built once, in its own position, and moved every frame to
//! the crank angle and piston heights the simulation reports: the pistons
//! slide, the rods swing and the shaft and flywheel turn. Drawing only;
//! the physics uses its own crank geometry, which these parts follow
//! exactly (each rod's ends sit on its wrist pin and crank pin).

use std::f32::consts::{
    FRAC_PI_2,
    PI,
    TAU, //
};
use std::ops::Range;

use macroquad::models::{Mesh, Vertex};
use macroquad::prelude::{
    Camera3D,
    Color,
    Mat3,
    Material,
    Vec3,
    Vec4,
    vec3, //
};

use super::block::{
    BlockShape,
    key_light, //
};
use super::solid::{
    Frame,
    Solid,
    draw_metal,
    finish,
    load_metal,
    turn,
    web_outline, //
};
use super::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY, //
};
use crate::sim::geometry::ChamberGeometry;
use crate::sim::pistons::PowerPistons;

// Sizes are fractions of the bore's radius, so the parts keep their
// proportions at any engine size.

/// Pistons: the gap to the bore wall, overall height above the face (the
/// crown faces the gas), wall and crown thickness, and the wrist pin's
/// height above the face and its radius. The pin rests in a bar across the
/// cup (its bosses), split in the middle for the rod.
const PISTON_CLEARANCE: f32 = 0.016;
const PISTON_HEIGHT: f32 = 0.4;
const PISTON_WALL: f32 = 0.05;
const PISTON_CROWN: f32 = 0.1;
const WRIST_PIN_HEIGHT: f32 = 0.24;
const WRIST_PIN_RADIUS: f32 = 0.065;
const BOSS_RADIUS: f32 = 0.12;
/// Three ring grooves near the crown: their heights, width and depth.
const RING_HEIGHTS: [f32; 3] = [0.06, 0.12, 0.18];
const RING_WIDTH: f32 = 0.022;
const RING_DEPTH: f32 = 0.014;

/// Connecting rods: thickness along the shaft, the radius of the eye
/// around each pin, and the shank's half-width at each end and half its
/// thickness.
const ROD_THICKNESS: f32 = 0.14;
const SMALL_END_RADIUS: f32 = 0.12;
const BIG_END_RADIUS: f32 = 0.17;
const SHANK_HALF_WIDTH: (f32, f32) = (0.065, 0.095);
const SHANK_HALF_THICK: f32 = 0.035;
/// Space between a rod and the webs or bosses beside it.
const SIDE_GAP: f32 = 0.02;

/// Crankshaft: the shaft's radius, the crank pin's, each web's thickness,
/// its radius around the shaft and around the pin, and how far the shaft
/// runs past each outer web.
const SHAFT_RADIUS: f32 = 0.075;
const CRANK_PIN_RADIUS: f32 = 0.08;
const WEB_THICKNESS: f32 = 0.1;
const WEB_SHAFT_RADIUS: f32 = 0.19;
const WEB_PIN_RADIUS: f32 = 0.15;
const SHAFT_STUB: f32 = 0.14;

/// Flywheel, in the middle of the shaft: outer radius, the rim's depth and
/// width, the hub's radius and width, and its spokes, tapering from the hub
/// out to the rim.
const FLYWHEEL_RADIUS: f32 = 1.25;
const RIM_DEPTH: f32 = 0.17;
const RIM_WIDTH: f32 = 0.2;
const RIM_CHAMFER: f32 = 0.025;
const HUB_RADIUS: f32 = 0.2;
const HUB_WIDTH: f32 = 0.28;
const SPOKES: usize = 6;
const SPOKE_HALF_WIDTH: (f32, f32) = (0.06, 0.04);
const SPOKE_HALF_THICK: f32 = 0.035;

/// Bearing stands, one each side of the flywheel: the bearing's distance
/// from the middle, its radius and width; and the legs, from feet this far
/// to the front and back, with half their section.
const STAND_X: f32 = 0.22;
const BEARING_RADIUS: f32 = 0.16;
const BEARING_WIDTH: f32 = 0.12;
const FOOT_Z: f32 = 0.9;
const LEG_HALF_WIDTH: f32 = 0.05;
/// The pad each leg stands on: its height, half its length (front to
/// back) and half its width (along the shaft).
const FOOT_HEIGHT: f32 = 0.04;
const FOOT_HALF_LENGTH: f32 = 0.1;
const FOOT_HALF_WIDTH: f32 = 0.07;
const LEG_HALF_THICK: f32 = 0.04;
/// The crossbar between each stand's legs, as a share of their height.
const BRACE_HEIGHT: f32 = 0.35;

/// Facets around each round part.
const ROUND_SEGMENTS: usize = 40;
const SMALL_SEGMENTS: usize = 20;
const WEB_STEPS: usize = 12;

/// Finishes: color and polish (see `solid.rs`).
const ALUMINIUM: Color = finish(206, 211, 217, 0.55);
const ALUMINIUM_INSIDE: Color = finish(146, 151, 159, 0.25);
const RING_GROOVE: Color = finish(74, 78, 86, 0.2);
const STEEL: Color = finish(158, 164, 174, 0.7);
const BRIGHT_STEEL: Color = finish(200, 205, 214, 0.95);
const BRASS: Color = finish(214, 166, 78, 0.85);
const BRASS_DARK: Color = finish(176, 132, 58, 0.6);
const PAINT: Color = finish(62, 72, 90, 0.3);

/// The crank angle of each cylinder's crank relative to the shaft angle,
/// as in the simulation: the cold crank a quarter turn ahead.
const HOT_PHASE: f32 = 0.0;
const COLD_PHASE: f32 = FRAC_PI_2;

/// Rotation about the shaft (the x axis) by `angle`: a point straight above
/// the shaft turns toward +z.
fn about_shaft(angle: f32) -> Mat3 {
    let (sin, cos) = angle.sin_cos();
    Mat3::from_cols(Vec3::X, vec3(0.0, cos, sin), vec3(0.0, -sin, cos))
}

/// Where everything goes, from the block and the crank.
#[derive(Clone, Copy, Debug)]
struct Layout {
    unit: f32,
    /// Distance from the middle to each cylinder's axis.
    offset: f32,
    bore: f32,
    block_top: f32,
    shaft_y: f32,
    crank_radius: f32,
}

impl Layout {
    /// The crank pin of the cylinder at `x`, with its crank at `angle`.
    fn crank_pin(self, x: f32, angle: f32) -> Vec3 {
        let (sin, cos) = angle.sin_cos();
        vec3(
            x,
            self.shaft_y + self.crank_radius * cos,
            self.crank_radius * sin,
        )
    }

    fn wrist_pin(self, x: f32, face_y: f32) -> Vec3 {
        vec3(x, face_y + WRIST_PIN_HEIGHT * self.unit, 0.0)
    }

    /// Each web's distance from its crank's middle, along the shaft.
    fn web_middle(self) -> f32 {
        (ROD_THICKNESS / 2.0 + SIDE_GAP + WEB_THICKNESS / 2.0) * self.unit
    }
}

/// Where one part's vertices are in the mesh and how it moves.
#[derive(Clone, Debug)]
struct Part {
    vertices: Range<usize>,
    motion: Motion,
}

#[derive(Clone, Copy, Debug)]
enum Motion {
    /// Stays where it was built.
    Fixed,
    /// Turns with the shaft.
    Shaft,
    /// Slides up and down with the piston at this x.
    Piston { hot: bool },
    /// Runs from the wrist pin to the crank pin of this side; built
    /// standing straight up from the origin with its big end `length` up.
    Rod { hot: bool, length: f32 },
}

pub struct Assembly {
    layout: Layout,
    /// Every part as built, in place; the mesh holds them as posed.
    built: Vec<Vertex>,
    mesh: Mesh,
    parts: Vec<Part>,
    material: Option<Material>,
}

impl Assembly {
    /// Builds every part for `geometry` and the crank of `pistons`. Needs a
    /// window, for the material; `build` does the rest without one.
    pub fn new(geometry: ChamberGeometry, pistons: &PowerPistons) -> Self {
        let material = load_metal();
        let mut assembly = Self::build(geometry, pistons.stroke() / 2.0, pistons.shaft_height());
        if material.is_none() {
            // Drawn in plain colors, alpha means transparency: make it solid.
            for vertex in assembly.built.iter_mut().chain(&mut assembly.mesh.vertices) {
                vertex.color[3] = u8::MAX;
            }
        }
        assembly.material = material;
        assembly
    }

    fn build(geometry: ChamberGeometry, crank_radius: f32, shaft_y: f32) -> Self {
        let shape = BlockShape::new(geometry);
        let layout = Layout {
            unit: shape.bore,
            offset: shape.offset,
            bore: shape.bore,
            block_top: shape.top,
            shaft_y,
            crank_radius,
        };
        let mut solid = Solid::new();
        let mut parts = Vec::new();
        let mut add = |solid: &mut Solid, motion, build: &dyn Fn(&mut Solid)| {
            let start = solid.vertices.len();
            build(solid);
            parts.push(Part {
                vertices: start..solid.vertices.len(),
                motion,
            });
        };
        add(&mut solid, Motion::Shaft, &|solid| {
            crankshaft(solid, layout)
        });
        add(&mut solid, Motion::Shaft, &|solid| flywheel(solid, layout));
        add(&mut solid, Motion::Fixed, &|solid| stands(solid, layout));
        for hot in [true, false] {
            add(&mut solid, Motion::Piston { hot }, &|solid| {
                piston(solid, layout)
            });
            // The rod's length at top dead center; it is stretched to fit
            // the pins exactly as the crank turns (by under 2%).
            let length = layout.shaft_y + layout.crank_radius
                - layout.block_top
                - WRIST_PIN_HEIGHT * layout.unit;
            add(&mut solid, Motion::Rod { hot, length }, &|solid| {
                rod(solid, layout, length)
            });
        }
        assert!(solid.vertices.len() <= DRAW_CALL_VERTEX_CAPACITY);
        assert!(solid.indices.len() <= DRAW_CALL_INDEX_CAPACITY);
        let mut assembly = Self {
            layout,
            built: solid.vertices.clone(),
            mesh: Mesh {
                vertices: solid.vertices,
                indices: solid.indices,
                texture: None,
            },
            parts,
            material: None,
        };
        assembly.pose(0.0, shape.top, shape.top);
        assembly
    }

    /// Moves every part to the shaft `angle` and the piston faces at
    /// `hot_face` and `cold_face`.
    fn pose(&mut self, angle: f32, hot_face: f32, cold_face: f32) {
        let layout = self.layout;
        // Fixed parts stay as built (the mesh starts as a copy).
        for part in self
            .parts
            .iter()
            .filter(|part| !matches!(part.motion, Motion::Fixed))
        {
            let (turn, shift): (Mat3, Box<dyn Fn(Vec3) -> Vec3>) = match part.motion {
                Motion::Fixed => continue,
                Motion::Shaft => {
                    let axis = vec3(0.0, layout.shaft_y, 0.0);
                    let turn = about_shaft(angle);
                    (turn, Box::new(move |point| axis + turn * (point - axis)))
                }
                Motion::Piston { hot } => {
                    let rise = if hot { hot_face } else { cold_face } - layout.block_top;
                    let x = side_x(layout, hot);
                    // Built for the hot side; the cold piston is the same
                    // part moved across.
                    let across = x - side_x(layout, true);
                    (
                        Mat3::IDENTITY,
                        Box::new(move |point| point + vec3(across, rise, 0.0)),
                    )
                }
                Motion::Rod { hot, length } => {
                    let x = side_x(layout, hot);
                    let (phase, face) = if hot {
                        (HOT_PHASE, hot_face)
                    } else {
                        (COLD_PHASE, cold_face)
                    };
                    let small = layout.wrist_pin(x, face);
                    let big = layout.crank_pin(x, angle + phase);
                    let reach = big - small;
                    let lean = reach.z.atan2(reach.y);
                    let turn = about_shaft(lean);
                    let stretch = reach.length() - length;
                    (
                        turn,
                        Box::new(move |point| {
                            // The big end's half moves out to its pin.
                            let extra = if point.y > length / 2.0 { stretch } else { 0.0 };
                            vec3(x, small.y, small.z) + turn * (point + vec3(0.0, extra, 0.0))
                        }),
                    )
                }
            };
            for index in part.vertices.clone() {
                let built = self.built[index];
                let vertex = &mut self.mesh.vertices[index];
                vertex.position = shift(built.position);
                let normal = turn * built.normal.truncate();
                vertex.normal = Vec4::new(normal.x, normal.y, normal.z, 0.0);
            }
        }
    }

    /// Draws every part where `pistons` has the crank now.
    pub fn draw(&mut self, pistons: &PowerPistons, camera: &Camera3D) {
        self.pose(pistons.angle(), pistons.hot().face_y, pistons.cold().face_y);
        draw_metal(
            &self.mesh,
            self.material.as_ref(),
            camera,
            key_light(camera.position),
        );
    }
}

/// The flywheel's outer radius for an engine of `geometry`, for the tests
/// of the first view.
#[cfg(test)]
pub fn flywheel_radius(geometry: ChamberGeometry) -> f32 {
    FLYWHEEL_RADIUS * geometry.cylinder_radius
}

/// The x of the hot (left) or cold (right) cylinder's axis.
fn side_x(layout: Layout, hot: bool) -> f32 {
    if hot { -layout.offset } else { layout.offset }
}

/// The shaft from end to end, a pair of webs and a crank pin at each
/// cylinder, at shaft angle zero.
fn crankshaft(solid: &mut Solid, layout: Layout) {
    let unit = layout.unit;
    let y = layout.shaft_y;
    let web = layout.web_middle();
    let web_half = WEB_THICKNESS * unit / 2.0;
    let outer = layout.offset + web + web_half + SHAFT_STUB * unit;
    // Into the middle of each web, so no seam shows.
    let inner = layout.offset - web;
    // Main journals: through the middle (and the flywheel and bearings),
    // and a stub out past each outer web.
    solid.cylinder(
        vec3(-inner, y, 0.0),
        vec3(inner, y, 0.0),
        SHAFT_RADIUS * unit,
        SMALL_SEGMENTS,
        BRIGHT_STEEL,
    );
    for side in [-1.0, 1.0] {
        solid.cylinder(
            vec3(side * (layout.offset + web), y, 0.0),
            vec3(side * outer, y, 0.0),
            SHAFT_RADIUS * unit,
            SMALL_SEGMENTS,
            BRIGHT_STEEL,
        );
    }
    let outline = web_outline(
        (WEB_SHAFT_RADIUS * unit, WEB_PIN_RADIUS * unit),
        layout.crank_radius,
        WEB_STEPS,
    );
    for (hot, phase) in [(true, HOT_PHASE), (false, COLD_PHASE)] {
        let x = side_x(layout, hot);
        let pin = layout.crank_pin(x, phase);
        let toward_pin = (pin - vec3(x, y, 0.0)).normalize();
        for side in [-1.0, 1.0] {
            let start = x + side * web - web_half;
            let frame = Frame {
                origin: vec3(start, y, 0.0),
                axis: Vec3::X,
                u: toward_pin,
                v: Vec3::X.cross(toward_pin),
            };
            solid.plate(frame, &outline, 2.0 * web_half, STEEL);
        }
        solid.cylinder(
            pin - Vec3::X * web,
            pin + Vec3::X * web,
            CRANK_PIN_RADIUS * unit,
            SMALL_SEGMENTS,
            BRIGHT_STEEL,
        );
    }
}

/// The flywheel, a spoked brass wheel in the middle of the shaft.
fn flywheel(solid: &mut Solid, layout: Layout) {
    let unit = layout.unit;
    let center = vec3(0.0, layout.shaft_y, 0.0);
    let (radius, inner) = (FLYWHEEL_RADIUS * unit, (FLYWHEEL_RADIUS - RIM_DEPTH) * unit);
    let (half, chamfer) = (RIM_WIDTH * unit / 2.0, RIM_CHAMFER * unit);
    let frame = Frame::along(center - Vec3::X * half, Vec3::X);
    let width = 2.0 * half;
    solid.lathe(
        frame,
        &[
            turn(inner, 0.0, BRASS_DARK),
            turn(radius - chamfer, 0.0, BRASS),
            turn(radius, chamfer, BRASS),
            turn(radius, width - chamfer, BRASS),
            turn(radius - chamfer, width, BRASS),
            turn(inner, width, BRASS_DARK),
            turn(inner, 0.0, BRASS_DARK),
        ],
        2 * ROUND_SEGMENTS,
    );
    let hub = HUB_WIDTH * unit / 2.0;
    solid.lathe(
        Frame::along(center - Vec3::X * hub, Vec3::X),
        &[
            turn(SHAFT_RADIUS * unit, 0.0, BRASS),
            turn(HUB_RADIUS * unit, 0.0, BRASS),
            turn(HUB_RADIUS * unit, 2.0 * hub, BRASS),
            turn(SHAFT_RADIUS * unit, 2.0 * hub, BRASS),
        ],
        SMALL_SEGMENTS,
    );
    for spoke in 0..SPOKES {
        let direction =
            about_shaft(TAU * spoke as f32 / SPOKES as f32 + PI / SPOKES as f32) * Vec3::Y;
        solid.bar(
            center + direction * (HUB_RADIUS * unit * 0.9),
            center + direction * (inner + 0.02 * unit),
            Vec3::X.cross(direction),
            (SPOKE_HALF_WIDTH.0 * unit, SPOKE_HALF_WIDTH.1 * unit),
            SPOKE_HALF_THICK * unit,
            BRASS,
        );
    }
}

/// Two bearing stands, one each side of the flywheel, standing on the
/// block's top between the bores.
fn stands(solid: &mut Solid, layout: Layout) {
    let unit = layout.unit;
    let bearing_low = layout.shaft_y - BEARING_RADIUS * unit * 0.8;
    for side in [-1.0, 1.0] {
        let x = side * STAND_X * unit;
        let half = BEARING_WIDTH * unit / 2.0;
        solid.lathe(
            Frame::along(vec3(x - half, layout.shaft_y, 0.0), Vec3::X),
            &[
                turn(SHAFT_RADIUS * unit, 0.0, PAINT),
                turn(BEARING_RADIUS * unit, 0.0, PAINT),
                turn(BEARING_RADIUS * unit, 2.0 * half, PAINT),
                turn(SHAFT_RADIUS * unit, 2.0 * half, PAINT),
            ],
            SMALL_SEGMENTS,
        );
        let pad = FOOT_HEIGHT * unit;
        let feet = [-1.0, 1.0].map(|front| vec3(x, layout.block_top, front * FOOT_Z * unit));
        for foot in feet {
            solid.bar(
                foot,
                foot + Vec3::Y * pad,
                Vec3::Z,
                (FOOT_HALF_LENGTH * unit, FOOT_HALF_LENGTH * unit),
                FOOT_HALF_WIDTH * unit,
                PAINT,
            );
        }
        // The legs rise from the pads, whose tops their corners stay above.
        let feet = feet.map(|foot| foot + Vec3::Y * (pad + LEG_HALF_WIDTH * unit));
        let tops =
            [-1.0, 1.0].map(|front| vec3(x, bearing_low, front * 0.5 * BEARING_RADIUS * unit));
        for (foot, top) in feet.into_iter().zip(tops) {
            solid.bar(
                foot,
                top,
                Vec3::X.cross(top - foot),
                (LEG_HALF_WIDTH * unit, LEG_HALF_WIDTH * unit * 0.8),
                LEG_HALF_THICK * unit,
                PAINT,
            );
        }
        let brace = |index: usize| feet[index].lerp(tops[index], BRACE_HEIGHT);
        solid.bar(
            brace(0),
            brace(1),
            Vec3::Y,
            (LEG_HALF_WIDTH * unit * 0.7, LEG_HALF_WIDTH * unit * 0.7),
            LEG_HALF_THICK * unit * 0.8,
            PAINT,
        );
    }
}

/// A piston for the hot bore, with its crown (the face) at the block's
/// top: a cup with three ring grooves, and a wrist pin across it in a bar
/// that is split for the rod.
fn piston(solid: &mut Solid, layout: Layout) {
    let unit = layout.unit;
    let x = side_x(layout, true);
    let face = layout.block_top;
    let outside = layout.bore - PISTON_CLEARANCE * unit;
    let inside = outside - PISTON_WALL * unit;
    let height = PISTON_HEIGHT * unit;
    let mut outline = vec![turn(0.0, 0.0, ALUMINIUM), turn(outside, 0.0, ALUMINIUM)];
    for ring in RING_HEIGHTS {
        let (low, high) = (
            (ring - RING_WIDTH / 2.0) * unit,
            (ring + RING_WIDTH / 2.0) * unit,
        );
        let groove = outside - RING_DEPTH * unit;
        outline.extend([
            turn(outside, low, ALUMINIUM),
            turn(groove, low, RING_GROOVE),
            turn(groove, high, RING_GROOVE),
            turn(outside, high, ALUMINIUM),
        ]);
    }
    outline.extend([
        turn(outside, height, ALUMINIUM),
        turn(inside, height, ALUMINIUM),
        turn(inside, PISTON_CROWN * unit, ALUMINIUM_INSIDE),
        turn(0.0, PISTON_CROWN * unit, ALUMINIUM_INSIDE),
    ]);
    solid.lathe(
        Frame::along(vec3(x, face, 0.0), Vec3::Y),
        &outline,
        2 * ROUND_SEGMENTS,
    );
    let pin = layout.wrist_pin(x, face);
    let slot = (ROD_THICKNESS / 2.0 + SIDE_GAP) * unit;
    for side in [-1.0, 1.0] {
        solid.cylinder(
            pin + Vec3::X * side * slot,
            pin + Vec3::X * side * (inside + 0.01 * unit),
            BOSS_RADIUS * unit,
            SMALL_SEGMENTS,
            ALUMINIUM,
        );
    }
    solid.cylinder(
        pin - Vec3::X * slot,
        pin + Vec3::X * slot,
        WRIST_PIN_RADIUS * unit,
        SMALL_SEGMENTS,
        BRIGHT_STEEL,
    );
}

/// A connecting rod standing up from the origin, its small end around the
/// wrist pin there and its big end around the crank pin `length` above.
fn rod(solid: &mut Solid, layout: Layout, length: f32) {
    let unit = layout.unit;
    let half = ROD_THICKNESS * unit / 2.0;
    for (center, radius) in [
        (Vec3::ZERO, SMALL_END_RADIUS),
        (Vec3::Y * length, BIG_END_RADIUS),
    ] {
        solid.cylinder(
            center - Vec3::X * half,
            center + Vec3::X * half,
            radius * unit,
            SMALL_SEGMENTS,
            STEEL,
        );
    }
    solid.bar(
        Vec3::Y * (SMALL_END_RADIUS * unit * 0.8),
        Vec3::Y * (length - BIG_END_RADIUS * unit * 0.8),
        Vec3::Z,
        (SHANK_HALF_WIDTH.0 * unit, SHANK_HALF_WIDTH.1 * unit),
        SHANK_HALF_THICK * unit,
        STEEL,
    );
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/assembly.rs"]
mod tests;
