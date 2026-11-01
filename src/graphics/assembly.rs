//! The moving parts above the block, as solid metal.
//!
//! They are an aluminium piston in each bore (a hollow cup with ring grooves and a wrist pin), a steel
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

use macroquad::models::{
    Mesh,
    Vertex, //
};
use macroquad::prelude::{
    Camera3D,
    Color,
    Mat3,
    Material,
    Vec3,
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
    turn,
    web_outline, //
};
use super::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY,
    draw_lit, //
};
use crate::sim::geometry::ChamberGeometry;
use crate::sim::pistons::PowerPistons;

/// Gap between a piston and the bore wall.
///
/// Sizes are fractions of the bore's radius, so the parts keep their
/// proportions at any engine size.
const PISTON_CLEARANCE: f32 = 0.016;
/// A piston's overall height above its face, the crown, which faces the
/// gas.
const PISTON_HEIGHT: f32 = 0.4;
/// Thickness of a piston's wall.
const PISTON_WALL: f32 = 0.05;
/// Thickness of a piston's crown.
const PISTON_CROWN: f32 = 0.1;
/// Height of the wrist pin above the face.
const WRIST_PIN_HEIGHT: f32 = 0.24;
/// Radius of the wrist pin.
const WRIST_PIN_RADIUS: f32 = 0.065;
/// Radius of the bar across the cup the pin rests in (its bosses), which
/// is split in the middle for the rod.
const BOSS_RADIUS: f32 = 0.12;
/// How far the bosses reach into the piston's wall.
const BOSS_OVERLAP: f32 = 0.01;
/// Heights of the three ring grooves near the crown.
const RING_HEIGHTS: [f32; 3] = [0.06, 0.12, 0.18];
/// Width of a ring groove.
const RING_WIDTH: f32 = 0.022;
/// Depth of a ring groove.
const RING_DEPTH: f32 = 0.014;

/// Thickness of a connecting rod along the shaft.
const ROD_THICKNESS: f32 = 0.14;
/// Radius of the eye around the wrist pin.
const SMALL_END_RADIUS: f32 = 0.12;
/// Radius of the eye around the crank pin.
const BIG_END_RADIUS: f32 = 0.17;
/// Half-width of the shank at the small and the big end.
const SHANK_HALF_WIDTH: (f32, f32) = (0.065, 0.095);
/// Half the shank's thickness.
const SHANK_HALF_THICK: f32 = 0.035;
/// How far into each eye the shank reaches, as a share of the eye's
/// radius out from the pin.
const SHANK_REACH: f32 = 0.8;
/// Space between a rod and the webs or bosses beside it.
const SIDE_GAP: f32 = 0.02;

/// Radius of the shaft.
const SHAFT_RADIUS: f32 = 0.075;
/// Radius of a crank pin.
const CRANK_PIN_RADIUS: f32 = 0.08;
/// Thickness of a crank web.
const WEB_THICKNESS: f32 = 0.1;
/// Radius of a web around the shaft.
const WEB_SHAFT_RADIUS: f32 = 0.19;
/// Radius of a web around the crank pin.
const WEB_PIN_RADIUS: f32 = 0.15;
/// How far the shaft runs past each outer web.
const SHAFT_STUB: f32 = 0.14;

/// Outer radius of the flywheel, in the middle of the shaft.
const FLYWHEEL_RADIUS: f32 = 1.25;
/// Depth of the flywheel's rim.
const RIM_DEPTH: f32 = 0.17;
/// Width of the rim.
const RIM_WIDTH: f32 = 0.2;
/// Chamfer on the rim's edges.
const RIM_CHAMFER: f32 = 0.025;
/// Radius of the hub.
const HUB_RADIUS: f32 = 0.2;
/// Width of the hub.
const HUB_WIDTH: f32 = 0.28;
/// Spokes, tapering from the hub out to the rim.
const SPOKES: usize = 6;
/// Half-width of a spoke at the hub and at the rim.
const SPOKE_HALF_WIDTH: (f32, f32) = (0.06, 0.04);
/// Half a spoke's thickness.
const SPOKE_HALF_THICK: f32 = 0.035;
/// Where a spoke starts, as a share of the hub's radius, inside the hub.
const SPOKE_ROOT: f32 = 0.9;
/// How far a spoke reaches into the rim.
const SPOKE_OVERLAP: f32 = 0.02;

/// Distance of each bearing stand from the middle, one each side of the
/// flywheel.
const STAND_X: f32 = 0.22;
/// Radius of a bearing.
const BEARING_RADIUS: f32 = 0.16;
/// Width of a bearing.
const BEARING_WIDTH: f32 = 0.12;
/// Where the legs meet a bearing, below the shaft, in bearing radii.
const LEG_TOP_DROP: f32 = 0.8;
/// How far apart the legs meet a bearing, front and back, in bearing
/// radii.
const LEG_TOP_SPREAD: f32 = 0.5;
/// How far the feet are to the front and back.
const FOOT_Z: f32 = 0.9;
/// Half a leg's width in the lean, at its foot.
const LEG_HALF_WIDTH: f32 = 0.05;
/// A leg's width at its top against its foot.
const LEG_TAPER: f32 = 0.8;
/// Half a leg's thickness.
const LEG_HALF_THICK: f32 = 0.04;
/// Height of the pad each leg stands on.
const FOOT_HEIGHT: f32 = 0.06;
/// Half a pad's length, front to back.
const FOOT_HALF_LENGTH: f32 = 0.1;
/// Half a pad's width, along the shaft.
const FOOT_HALF_WIDTH: f32 = 0.07;
/// Height of the crossbar between each stand's legs, as a share of theirs.
const BRACE_HEIGHT: f32 = 0.35;
/// Width of the crossbar against a leg's.
const BRACE_WIDTH: f32 = 0.7;
/// Thickness of the crossbar against a leg's.
const BRACE_THICK: f32 = 0.8;

/// Facets around each large round part.
const ROUND_SEGMENTS: usize = 40;
/// Facets around each small round part.
const SMALL_SEGMENTS: usize = 20;
/// Steps around each end of a crank web.
const WEB_STEPS: usize = 12;

/// Aluminium, outside.
///
/// Finishes are a color and polish (see `solid.rs`), saturated in the
/// style of a PS2 demo.
const ALUMINIUM: Color = finish(214, 222, 236, 0.7);
/// Aluminium, inside the piston.
const ALUMINIUM_INSIDE: Color = finish(140, 150, 175, 0.3);
/// The bottom of a ring groove.
const RING_GROOVE: Color = finish(50, 56, 80, 0.2);
/// Steel.
const STEEL: Color = finish(140, 156, 190, 0.75);
/// Polished steel.
const BRIGHT_STEEL: Color = finish(210, 218, 235, 0.95);
/// Brass.
const BRASS: Color = finish(255, 186, 36, 0.85);
/// Brass, inside the rim.
const BRASS_DARK: Color = finish(214, 132, 20, 0.6);
/// The bearing stands' paint.
const PAINT: Color = finish(48, 72, 170, 0.35);

/// The hot crank's angle relative to the shaft angle, as in the
/// simulation.
const HOT_PHASE: f32 = 0.0;
/// The cold crank's angle relative to the shaft angle: a quarter turn
/// ahead.
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
    /// The bore radius every size is a fraction of.
    unit: f32,
    /// Distance from the middle to each cylinder's axis.
    offset: f32,
    /// Radius of each bore.
    bore: f32,
    /// Height of the block's top, where the piston faces are at rest.
    block_top: f32,
    /// Height of the shaft.
    shaft_y: f32,
    /// Distance from the shaft to each crank pin.
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

    /// The wrist pin of the cylinder at `x`, with its piston face at
    /// `face_y`.
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
    /// Its vertices in the mesh.
    vertices: Range<usize>,
    /// How it moves.
    motion: Motion,
}

/// How a part moves.
#[derive(Clone, Copy, Debug)]
enum Motion {
    /// Stays where it was built.
    Fixed,
    /// Turns with the shaft.
    Shaft,
    /// Slides up and down with the hot or the cold piston.
    Piston {
        /// Whether it is the hot piston.
        hot: bool,
    },
    /// Runs from the wrist pin to the crank pin of one side.
    Rod {
        /// Whether it is the hot side's rod.
        hot: bool,
        /// Distance between its pins as built, standing straight up from
        /// the origin with its big end up.
        length: f32,
    },
}

/// Where a moving part goes this frame.
///
/// Each point `p` of the part as built goes to `to + turn * (p - from)`,
/// after a point above `split` moves up by `stretch`.
struct Placement {
    /// Rotation of the part.
    turn: Mat3,
    /// The point turned about, as built.
    from: Vec3,
    /// Where that point goes.
    to: Vec3,
    /// Height above which points stretch.
    split: f32,
    /// How far points above `split` move up.
    stretch: f32,
}

impl Placement {
    /// Where the built point `point` goes.
    fn apply(&self, point: Vec3) -> Vec3 {
        let extra = if point.y > self.split {
            self.stretch
        } else {
            0.0
        };
        self.to + self.turn * (point - self.from + vec3(0.0, extra, 0.0))
    }
}

/// The pistons, rods, crankshaft, flywheel and stands.
pub struct Assembly {
    /// Where everything goes.
    layout: Layout,
    /// Every part as built, in place; the mesh holds them as posed.
    built: Vec<Vertex>,
    /// Every part as posed.
    mesh: Mesh,
    /// Where each part's vertices are, and how it moves.
    parts: Vec<Part>,
    /// The lit-metal material, if it can be built.
    material: Option<Material>,
}

impl Assembly {
    /// Builds every part for `geometry` and the crank of `pistons`.
    ///
    /// It needs a window, for the material; `build` does the rest without
    /// one.
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

    /// Builds every part for `geometry`, with cranks of `crank_radius` on a
    /// shaft at `shaft_y`, posed at rest.
    ///
    /// # Panics
    ///
    /// Panics if the parts would not fit Macroquad's buffers.
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
        for part in &self.parts {
            let placement = match part.motion {
                // Fixed parts stay as built (the mesh starts as a copy).
                Motion::Fixed => continue,
                Motion::Shaft => {
                    let axis = vec3(0.0, layout.shaft_y, 0.0);
                    Placement {
                        turn: about_shaft(angle),
                        from: axis,
                        to: axis,
                        split: f32::INFINITY,
                        stretch: 0.0,
                    }
                }
                Motion::Piston { hot } => {
                    let rise = if hot { hot_face } else { cold_face } - layout.block_top;
                    // Built for the hot side; the cold piston is the same
                    // part moved across.
                    let across = side_x(layout, hot) - side_x(layout, true);
                    Placement {
                        turn: Mat3::IDENTITY,
                        from: Vec3::ZERO,
                        to: vec3(across, rise, 0.0),
                        split: f32::INFINITY,
                        stretch: 0.0,
                    }
                }
                Motion::Rod { hot, length } => {
                    let x = side_x(layout, hot);
                    let (phase, face) = if hot {
                        (HOT_PHASE, hot_face)
                    } else {
                        (COLD_PHASE, cold_face)
                    };
                    let small = layout.wrist_pin(x, face);
                    let reach = layout.crank_pin(x, angle + phase) - small;
                    // The big end's half moves out to its pin.
                    Placement {
                        turn: about_shaft(reach.z.atan2(reach.y)),
                        from: Vec3::ZERO,
                        to: vec3(x, small.y, small.z),
                        split: length / 2.0,
                        stretch: reach.length() - length,
                    }
                }
            };
            for index in part.vertices.clone() {
                let built = self.built[index];
                let vertex = &mut self.mesh.vertices[index];
                vertex.position = placement.apply(built.position);
                let normal = placement.turn * built.normal.truncate();
                vertex.normal = normal.extend(0.0);
            }
        }
    }

    /// Draws every part where `pistons` has the crank now.
    pub fn draw(&mut self, pistons: &PowerPistons, camera: &Camera3D) {
        self.pose(pistons.angle(), pistons.hot().face_y, pistons.cold().face_y);
        let light = key_light(camera.position);
        draw_lit([&self.mesh], self.material.as_ref(), camera, light);
    }
}

/// The flywheel's outer radius for an engine of `geometry`.
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
            center + direction * (HUB_RADIUS * unit * SPOKE_ROOT),
            center + direction * (inner + SPOKE_OVERLAP * unit),
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
    let bearing_low = layout.shaft_y - BEARING_RADIUS * unit * LEG_TOP_DROP;
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
        let tops = [-1.0, 1.0].map(|front| {
            vec3(
                x,
                bearing_low,
                front * LEG_TOP_SPREAD * BEARING_RADIUS * unit,
            )
        });
        let feet =
            [0, 1].map(|index| leg_start(feet[index], tops[index], pad, LEG_HALF_WIDTH * unit));
        for (foot, top) in feet.into_iter().zip(tops) {
            solid.bar(
                foot,
                top,
                Vec3::X.cross(top - foot),
                (LEG_HALF_WIDTH * unit, LEG_HALF_WIDTH * unit * LEG_TAPER),
                LEG_HALF_THICK * unit,
                PAINT,
            );
        }
        let brace = |index: usize| feet[index].lerp(tops[index], BRACE_HEIGHT);
        solid.bar(
            brace(0),
            brace(1),
            Vec3::Y,
            (
                LEG_HALF_WIDTH * unit * BRACE_WIDTH,
                LEG_HALF_WIDTH * unit * BRACE_WIDTH,
            ),
            LEG_HALF_THICK * unit * BRACE_THICK,
            PAINT,
        );
    }
}

/// Where a leg leaning from a pad at `foot` (on the block) up to `top`
/// starts, for a pad `pad` tall and a leg `half_width` across in the lean.
///
/// The leg is sunk into the pad so that its square-cut end's higher edge
/// meets the pad's top, leaving no gap under it, while the lower edge stays
/// inside the pad.
fn leg_start(foot: Vec3, top: Vec3, pad: f32, half_width: f32) -> Vec3 {
    let lean = (top - foot).normalize();
    let rise = half_width * (1.0 - lean.y * lean.y).max(0.0).sqrt();
    foot + Vec3::Y * (pad - rise)
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
            pin + Vec3::X * side * (inside + BOSS_OVERLAP * unit),
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
        Vec3::Y * (SMALL_END_RADIUS * unit * SHANK_REACH),
        Vec3::Y * (length - BIG_END_RADIUS * unit * SHANK_REACH),
        Vec3::Z,
        (SHANK_HALF_WIDTH.0 * unit, SHANK_HALF_WIDTH.1 * unit),
        SHANK_HALF_THICK * unit,
        STEEL,
    );
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/assembly.rs"]
mod tests;
