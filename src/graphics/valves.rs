//! The two valves that set the reservoirs: a gas valve feeding the burner
//! on the hot side and a valve on the liquid-nitrogen line feeding the
//! coil on the cold side.
//!
//! Each stands on the floor in front of its half of the block: a brass
//! body on a pipe running back to the burner or the coil's inlet, a stem, and a
//! handwheel on top, red for the gas and blue for the nitrogen. The wheel
//! turns as the valve opens, counterclockwise seen from above, as real
//! valves open. Drawing only: what a turn of the wheel does is the app's
//! (see `ui::handles`).

use std::f32::consts::TAU;

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
use super::nitrogen::coil_inlet_x;
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
    draw_lit, //
};
use crate::sim::reservoir::ReservoirSide;

/// How far in front of the block's front face each valve stands, in bore
/// radii.
const VALVE_OUT: f32 = 0.6;
/// Height of the pipe's axis above the floor, in bore radii.
pub(super) const PIPE_HEIGHT: f32 = 0.08;
/// Radius of the pipe, in bore radii.
const PIPE_RADIUS: f32 = 0.028;
/// How far the pipe reaches into the block's face, past the burner or the
/// coil, in bore radii.
const PIPE_REACH: f32 = 0.1;
/// Radius of the valve's body, in bore radii.
const BODY_RADIUS: f32 = 0.07;
/// Length of the valve's body along the pipe, in bore radii.
const BODY_LENGTH: f32 = 0.22;
/// Height of the stem's top above the floor, in bore radii.
const STEM_TOP: f32 = 0.5;
/// Radius of the stem, in bore radii.
const STEM_RADIUS: f32 = 0.022;
/// Radius of the handwheel to the middle of its rim, in bore radii.
const WHEEL_RADIUS: f32 = 0.24;
/// Half the thickness of the handwheel's rim, in bore radii.
const RIM_HALF: f32 = 0.028;
/// Spokes in each handwheel.
const SPOKES: usize = 4;
/// Half the width of a spoke, in bore radii.
const SPOKE_HALF: f32 = 0.018;
/// Radius of the hub, in bore radii.
const HUB_RADIUS: f32 = 0.05;
/// Segments around round parts.
const SEGMENTS: usize = 28;
/// The gas valve's handwheel.
const GAS_RED: Color = finish(214, 48, 40, 0.4);
/// The nitrogen valve's handwheel.
const NITROGEN_BLUE: Color = finish(48, 112, 232, 0.4);
/// The bodies.
const BRASS: Color = finish(240, 176, 52, 0.8);
/// The pipes and stems.
const STEEL: Color = finish(160, 172, 196, 0.7);

/// Where a valve's handwheel is, for grabbing it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ValveWheel {
    /// Which reservoir the valve feeds.
    pub side: ReservoirSide,
    /// The middle of the wheel; it turns about the vertical through it.
    pub center: Vec3,
    /// Radius of the wheel, to the middle of its rim.
    pub radius: f32,
}

/// Both valves' handwheels, hot first, for a block of `shape`.
pub fn valve_wheels(shape: BlockShape) -> [ValveWheel; 2] {
    [ReservoirSide::Hot, ReservoirSide::Cold].map(|side| {
        let (x, z) = valve_place(shape, side);
        ValveWheel {
            side,
            center: vec3(x, shape.base + STEM_TOP * shape.bore, z),
            radius: WHEEL_RADIUS * shape.bore,
        }
    })
}

/// Where the valve for `side` stands on the floor, seen from above (x, z).
fn valve_place(shape: BlockShape, side: ReservoirSide) -> (f32, f32) {
    let x = match side {
        ReservoirSide::Hot => -shape.offset,
        ReservoirSide::Cold => coil_inlet_x(shape),
    };
    (x, shape.outer + VALVE_OUT * shape.bore)
}

/// One valve's parts as built, with its wheel shut.
struct Built {
    /// Every vertex, the wheel's last.
    vertices: Vec<Vertex>,
    /// Its triangles, indexing its own vertices.
    indices: Vec<u16>,
    /// Where the wheel's vertices start.
    wheel_start: usize,
}

/// Both valves, their wheels turned to how open each is.
pub struct Valves {
    /// The wheels' places.
    wheels: [ValveWheel; 2],
    /// Each valve as built, hot first.
    built: [Built; 2],
    /// Both valves as posed, one mesh.
    mesh: Mesh,
    /// Where each valve's vertices start in the mesh.
    starts: [usize; 2],
    /// The lit-metal material, if it can be built.
    material: Option<Material>,
}

impl Valves {
    /// Builds both valves for a block of `shape`, shut.
    ///
    /// It needs a window, for the material; `build` does the rest without
    /// one.
    pub fn new(shape: BlockShape) -> Self {
        let mut valves = Self::build(shape);
        valves.material = load_metal();
        if valves.material.is_none() {
            // Drawn in plain colors, alpha means transparency: make it solid.
            for vertex in &mut valves.mesh.vertices {
                vertex.color[3] = u8::MAX;
            }
        }
        valves
    }

    /// Builds both valves for a block of `shape`, shut, without a
    /// material.
    ///
    /// # Panics
    ///
    /// Panics if they would not fit Macroquad's buffers.
    fn build(shape: BlockShape) -> Self {
        let wheels = valve_wheels(shape);
        let built = wheels.map(|wheel| build_valve(shape, wheel));
        let mut mesh = Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture: None,
        };
        let mut starts = [0; 2];
        for (valve, start) in built.iter().zip(&mut starts) {
            *start = mesh.vertices.len();
            mesh.vertices.extend_from_slice(&valve.vertices);
            mesh.indices
                .extend(valve.indices.iter().map(|&corner| corner + *start as u16));
        }
        assert!(mesh.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
        assert!(mesh.indices.len() < DRAW_CALL_INDEX_CAPACITY);
        Self {
            wheels,
            built,
            mesh,
            starts,
            material: None,
        }
    }

    /// Turns the hot valve's wheel `turns.0` from shut and the cold one's
    /// `turns.1`, in radians counterclockwise seen from above.
    fn pose(&mut self, turns: (f32, f32)) {
        for (index, angle) in [turns.0, turns.1].into_iter().enumerate() {
            let (center, valve) = (self.wheels[index].center, &self.built[index]);
            // A positive turn about +y is counterclockwise seen from above.
            let turn = Mat3::from_rotation_y(angle);
            let start = self.starts[index];
            for (offset, built) in valve.vertices.iter().enumerate().skip(valve.wheel_start) {
                let vertex = &mut self.mesh.vertices[start + offset];
                vertex.position = center + turn * (built.position - center);
                vertex.normal = (turn * built.normal.truncate()).extend(0.0);
            }
        }
    }

    /// Draws both valves seen from `camera`, their wheels turned as `pose`
    /// turns them.
    pub fn draw(&mut self, turns: (f32, f32), camera: &Camera3D) {
        self.pose(turns);
        let light = key_light(camera.position);
        draw_lit([&self.mesh], self.material.as_ref(), camera, light);
    }
}

/// The valve with `wheel` as built, for a block of `shape`, with its wheel
/// shut.
fn build_valve(shape: BlockShape, wheel: ValveWheel) -> Built {
    let unit = shape.bore;
    let mut solid = Solid::new();
    let (x, z) = (wheel.center.x, wheel.center.z);
    let pipe_y = shape.base + PIPE_HEIGHT * unit;
    // The pipe runs back to the block, through the burner or the coil.
    let face = shape.outer - PIPE_REACH * unit;
    solid.cylinder(
        vec3(x, pipe_y, z),
        vec3(x, pipe_y, face),
        PIPE_RADIUS * unit,
        SEGMENTS,
        STEEL,
    );
    let half = BODY_LENGTH * unit / 2.0;
    solid.cylinder(
        vec3(x, pipe_y, z + half),
        vec3(x, pipe_y, z - half),
        BODY_RADIUS * unit,
        SEGMENTS,
        BRASS,
    );
    solid.cylinder(
        vec3(x, pipe_y, z),
        wheel.center,
        STEM_RADIUS * unit,
        SEGMENTS,
        STEEL,
    );
    // The wheel, last, so it can turn on its own.
    let wheel_start = solid.vertices.len();
    let paint = match wheel.side {
        ReservoirSide::Hot => GAS_RED,
        ReservoirSide::Cold => NITROGEN_BLUE,
    };
    let (rim, ring) = (wheel.radius, RIM_HALF * unit);
    let base = wheel.center - Vec3::Y * ring;
    solid.lathe(
        Frame::along(base, Vec3::Y),
        &[
            turn(rim - ring, 0.0, paint),
            turn(rim + ring, 0.0, paint),
            turn(rim + ring, 2.0 * ring, paint),
            turn(rim - ring, 2.0 * ring, paint),
            turn(rim - ring, 0.0, paint),
        ],
        SEGMENTS,
    );
    solid.cylinder(
        base,
        base + Vec3::Y * 2.0 * ring,
        HUB_RADIUS * unit,
        SEGMENTS,
        paint,
    );
    for spoke in 0..SPOKES {
        let angle = TAU * spoke as f32 / SPOKES as f32;
        let way = vec3(angle.cos(), 0.0, angle.sin());
        solid.bar(
            wheel.center + way * HUB_RADIUS * unit,
            wheel.center + way * rim,
            Vec3::Y.cross(way),
            (SPOKE_HALF * unit, SPOKE_HALF * unit),
            SPOKE_HALF * unit,
            paint,
        );
    }
    Built {
        vertices: solid.vertices,
        indices: solid.indices,
        wheel_start,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/valves.rs"]
mod tests;
