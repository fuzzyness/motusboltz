//! The regenerator's cartridge: a short steel ring holding a grid of wire
//! mesh, lying on the table beside the hot cylinder while the regenerator
//! is out, and seated in the opening, as thick as the shared wall, while
//! it is in.
//!
//! Put the regenerator in and the cartridge lifts, arcs over the gas valve
//! to the front of the opening, turns to face along the shaft on the way,
//! and slides into the opening. Take it out and it flies back the same
//! way. Seated, it stands behind the gas in the cold cylinder from most
//! views, so after the gas is drawn it is drawn again faintly over it,
//! like a ghost seen through it.
//!
//! In first person it can also be carried: picked up from the table or
//! taken out of the opening into the figure's hands, it floats a little
//! ahead of the eyes, lagging the view. Brought near the front of the
//! opening it flies the rest of the way in from there; let go anywhere
//! else, it falls onto the table and lies where it lands, and flies back
//! there when the regenerator is next taken out. Drawing only: what a
//! press does is the app's.

use std::f32::consts::{
    FRAC_PI_2,
    PI, //
};

use macroquad::miniquad::{
    BlendFactor,
    BlendState,
    BlendValue,
    Equation, //
};
use macroquad::models::Mesh;
use macroquad::prelude::{
    Camera3D,
    Color,
    Comparison,
    Mat4,
    PipelineParams,
    Quat,
    UniformType,
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
use super::sprites::smoothstep;
use super::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY,
    draw_lit_placed, //
};

/// The cartridge's radius against the opening's, so it slides in.
pub const FIT: f32 = 0.97;
/// Thickness of the cartridge, in bore radii: the shared wall's, so it
/// fills the opening's length when seated.
const THICKNESS: f32 = 0.2;
/// Opacity of the seated cartridge drawn again over the gas.
const GHOST_OPACITY: f32 = 0.3;

/// Vertex shader of the ghost: the vertex colors, placed as usual.
const GHOST_VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;

varying lowp vec4 color;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    color = color0 / 255.0;
    gl_Position = Projection * Model * vec4(position, 1.0);
}"#;

/// Fragment shader of the ghost: the vertex color at the ghost's opacity.
const GHOST_FRAGMENT: &str = r#"#version 100
precision mediump float;

varying lowp vec4 color;

uniform float Opacity;

void main() {
    gl_FragColor = vec4(color.rgb, Opacity);
}"#;
/// Width of the steel ring, in bore radii.
const RING_WIDTH: f32 = 0.06;
/// Wires across the cartridge each way.
const WIRES: usize = 7;
/// Half the thickness of a wire, in bore radii.
const WIRE_HALF: f32 = 0.008;
/// Segments around the ring.
const SEGMENTS: usize = 36;
/// How far past the block's hot end the cartridge's middle lies, in bore
/// radii: clear of the burner's flames.
const REST_OUT: f32 = 1.45;
/// How far toward the front the cartridge lies, in bore radii.
const REST_AHEAD: f32 = 0.3;
/// How far in front of the block's face the cartridge lines up with the
/// opening before sliding in, in bore radii.
const LINE_UP_OUT: f32 = 1.0;
/// How high the cartridge's flight arcs above a straight line, in bore
/// radii: well over the gas valve.
const ARC_HEIGHT: f32 = 1.6;
/// Share of the trip spent flying to the front of the opening; the rest
/// slides it in.
const FLIGHT_SHARE: f32 = 0.65;
/// Seconds the whole trip takes.
pub const TRIP_SECONDS: f32 = 1.2;
/// How far its flight arcs above a straight line, as a share of the
/// line's length, for a flight shorter than `ARC_HEIGHT` would suit.
const ARC_SHARE: f32 = 0.5;
/// Seconds for a carried cartridge to catch up most of the way with where
/// it is carried to: it lags the view a little.
const CARRY_LAG_SECONDS: f32 = 0.08;
/// How fast a dropped cartridge falls, in bore radii a second each second.
const DROP_GRAVITY: f32 = 40.0;
/// How far from the front of the opening a carried cartridge flies in by
/// itself, in bore radii.
const SNAP_REACH: f32 = 1.3;
/// How far from the front of the opening a cartridge taken out of it must
/// be carried before bringing it back flies it in, as a share of
/// `SNAP_REACH`: it leaves the opening first rather than flying straight
/// back in.
const SNAP_REARM: f32 = 1.5;
/// How near where it is carried to a cartridge must be to be in the
/// hands, rather than still on its way out of the opening, in bore radii.
const IN_HANDS: f32 = 0.2;
/// The steel ring.
const RING_STEEL: Color = finish(176, 184, 198, 0.75);
/// The wire mesh.
const MESH_STEEL: Color = finish(150, 158, 172, 0.5);

/// Where the cartridge's middle lies on the table, beside the hot end of a
/// block of `shape`.
pub fn rest(shape: BlockShape) -> Vec3 {
    vec3(
        -(shape.offset + shape.outer + REST_OUT * shape.bore),
        shape.base + THICKNESS * shape.bore / 2.0,
        REST_AHEAD * shape.bore,
    )
}

/// Where the cartridge's middle sits in the opening of a block of `shape`.
pub fn seat(shape: BlockShape) -> Vec3 {
    vec3(0.0, shape.opening_y, 0.0)
}

/// Where the cartridge lines up with the opening, in front of the block's
/// face.
pub fn line_up(shape: BlockShape) -> Vec3 {
    seat(shape) + Vec3::Z * (shape.outer + LINE_UP_OUT * shape.bore)
}

/// Where the cartridge's middle is and how it is turned `progress` of the
/// way (0 on the table, 1 seated) along its trip into a block of `shape`
/// from where it first lies.
#[cfg(test)]
pub fn pose(shape: BlockShape, progress: f32) -> (Vec3, Quat) {
    trip_pose(shape, (rest(shape), Quat::IDENTITY), progress)
}

/// Where the cartridge's middle is and how it is turned `progress` of the
/// way along its trip into a block of `shape` from `from`, its middle and
/// turn where the trip starts: lying on the table, or carried.
pub fn trip_pose(shape: BlockShape, from: (Vec3, Quat), progress: f32) -> (Vec3, Quat) {
    let progress = progress.clamp(0.0, 1.0);
    // Lying flat on the table its axis is upright; in the opening it runs
    // along the shaft.
    let seated = Quat::from_rotation_z(-FRAC_PI_2);
    let (start, turned) = from;
    if progress < FLIGHT_SHARE {
        let share = smoothstep(0.0, 1.0, progress / FLIGHT_SHARE);
        let height = (ARC_HEIGHT * shape.bore).min(ARC_SHARE * start.distance(line_up(shape)));
        let arc = (PI * share).sin() * height;
        let center = start.lerp(line_up(shape), share) + Vec3::Y * arc;
        (center, turned.slerp(seated, share))
    } else {
        let share = smoothstep(0.0, 1.0, (progress - FLIGHT_SHARE) / (1.0 - FLIGHT_SHARE));
        (line_up(shape).lerp(seat(shape), share), seated)
    }
}

/// Builds the cartridge for an opening of `shape`, its middle at the
/// origin and its axis upright.
fn build(shape: BlockShape) -> Solid {
    let unit = shape.bore;
    let radius = FIT * shape.opening;
    let inner = radius - RING_WIDTH * unit;
    let half = THICKNESS * unit / 2.0;
    let mut solid = Solid::new();
    solid.lathe(
        Frame::along(Vec3::Y * -half, Vec3::Y),
        &[
            turn(inner, 0.0, RING_STEEL),
            turn(radius, 0.0, RING_STEEL),
            turn(radius, 2.0 * half, RING_STEEL),
            turn(inner, 2.0 * half, RING_STEEL),
            turn(inner, 0.0, RING_STEEL),
        ],
        SEGMENTS,
    );
    // The mesh: wires each way across the ring, through its middle.
    let spacing = 2.0 * inner / (WIRES + 1) as f32;
    let wire = WIRE_HALF * unit;
    for index in 1..=WIRES {
        let offset = -inner + index as f32 * spacing;
        let reach = (inner * inner - offset * offset).sqrt();
        for (along, across) in [(Vec3::X, Vec3::Z), (Vec3::Z, Vec3::X)] {
            solid.bar(
                across * offset - along * reach,
                across * offset + along * reach,
                Vec3::Y,
                (wire, wire),
                wire,
                MESH_STEEL,
            );
        }
    }
    solid
}

/// The cartridge and where it is on its trip.
pub struct Cartridge {
    /// Its shape as built, middle at the origin, axis upright, on the
    /// graphics card; it is moved into place as it is drawn.
    mesh: GpuMesh,
    /// Where the last `draw` placed it.
    placed: Mat4,
    /// How far along its trip into the opening it is, from 0 to 1.
    progress: f32,
    /// Where its middle lies on the table when it is out.
    lying: Vec3,
    /// Its middle and turn where its trip starts: lying on the table, or
    /// where it was let go of to fly in.
    from: (Vec3, Quat),
    /// Its middle and turn while carried.
    carried: Option<(Vec3, Quat)>,
    /// Whether, carried, it flies in once brought near the opening: not
    /// until it has been carried away from it after being taken out.
    snaps: bool,
    /// A fall onto the table under way: its middle and turn when let go,
    /// and seconds since.
    falling: Option<((Vec3, Quat), f32)>,
    /// The clock at the last pose, if there was one.
    last_seconds: Option<f64>,
    /// The block it goes into.
    shape: BlockShape,
    /// The lit-metal material, if it can be built.
    material: Option<GpuMaterial>,
    /// The ghost's material, if it can be built.
    ghost: Option<GpuMaterial>,
}

impl Cartridge {
    /// The cartridge for a block of `shape`, on the table, or seated if
    /// `seated`.
    pub fn new(shape: BlockShape, seated: bool) -> Self {
        let mut cartridge = Self::build(shape, seated);
        cartridge.material = load_metal();
        cartridge.ghost = load_ghost();
        if cartridge.material.is_none() {
            // Drawn in plain colors, alpha means transparency: make it solid.
            for vertex in &mut cartridge.mesh.edit().vertices {
                vertex.color[3] = u8::MAX;
            }
        }
        cartridge
    }

    /// The cartridge as `new` makes it, without a material.
    ///
    /// # Panics
    ///
    /// Panics if it would not fit Macroquad's buffers.
    fn build(shape: BlockShape, seated: bool) -> Self {
        let solid = build(shape);
        assert!(solid.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
        assert!(solid.indices.len() < DRAW_CALL_INDEX_CAPACITY);
        Self {
            mesh: GpuMesh::new(Mesh {
                vertices: solid.vertices,
                indices: solid.indices,
                texture: None,
            }),
            placed: Mat4::IDENTITY,
            progress: if seated { 1.0 } else { 0.0 },
            lying: rest(shape),
            from: (rest(shape), Quat::IDENTITY),
            carried: None,
            snaps: false,
            falling: None,
            last_seconds: None,
            shape,
            material: None,
            ghost: None,
        }
    }

    /// Whether it is all the way into the opening.
    pub fn seated(&self) -> bool {
        self.progress >= 1.0 && self.carried.is_none()
    }

    /// Whether it lies still on the table, out of the opening.
    pub fn lies_on_table(&self) -> bool {
        self.progress <= 0.0 && self.carried.is_none() && self.falling.is_none()
    }

    /// Where its middle lies on the table when it is out, or will once it
    /// has landed.
    pub fn lying(&self) -> Vec3 {
        self.lying
    }

    /// Where its middle is while it is carried.
    pub fn carried(&self) -> Option<Vec3> {
        self.carried.map(|(center, _)| center)
    }

    /// Where its middle is and how it is turned now.
    fn now(&self) -> (Vec3, Quat) {
        if let Some(carried) = self.carried {
            return carried;
        }
        if let Some(((start, turned), seconds)) = self.falling {
            // Down onto where it will lie, settling flat as it falls.
            let height = (start.y - self.lying.y).max(0.0);
            let fall = (DROP_GRAVITY * self.shape.bore * seconds * seconds / 2.0).min(height);
            let share = if height > 0.0 { fall / height } else { 1.0 };
            let level = start.lerp(self.lying, share);
            let center = vec3(level.x, start.y - fall, level.z);
            return (center, turned.slerp(Quat::IDENTITY, share));
        }
        trip_pose(self.shape, self.from, self.progress)
    }

    /// Takes it into the figure's hands from wherever it is: off the table,
    /// or out of the opening.
    pub fn pick_up(&mut self) {
        // Off the table it may fly in at once; out of the opening, not
        // until it has been carried away (see `carry`).
        self.snaps = self.progress <= 0.0;
        self.carried = Some(self.now());
        self.falling = None;
        self.progress = 0.0;
        self.from = (self.lying, Quat::IDENTITY);
    }

    /// Carries it for `dt` seconds toward `center`, turned `turn`, lagging
    /// a little behind.
    pub fn carry(&mut self, center: Vec3, turn: Quat, dt: f32) {
        let Some((at, turned)) = self.carried else {
            return;
        };
        let share = 1.0 - (-dt.max(0.0) / CARRY_LAG_SECONDS).exp();
        let at = at.lerp(center, share);
        self.carried = Some((at, turned.slerp(turn, share)));
        let in_hands = at.distance(center) < IN_HANDS * self.shape.bore;
        self.snaps |= in_hands && !self.near_opening(at, SNAP_REARM);
    }

    /// Whether `at` is within `share` of `SNAP_REACH` of the front of the
    /// opening.
    fn near_opening(&self, at: Vec3, share: f32) -> bool {
        at.distance(line_up(self.shape)) < share * SNAP_REACH * self.shape.bore
    }

    /// Whether it is carried near enough the opening to fly in: within
    /// `SNAP_REACH` of its front, and carried away from it first if it was
    /// taken out of it.
    pub fn by_opening(&self) -> bool {
        self.snaps && self.carried().is_some_and(|at| self.near_opening(at, 1.0))
    }

    /// Lets go of it to fall onto the table with its middle at `spot`, where
    /// it then lies.
    pub fn drop_onto(&mut self, spot: Vec3) {
        if let Some(carried) = self.carried.take() {
            self.lying = vec3(spot.x, rest(self.shape).y, spot.z);
            self.from = (self.lying, Quat::IDENTITY);
            self.falling = Some((carried, 0.0));
        }
    }

    /// Lets go of it to fly into the opening from where it is carried; the
    /// regenerator goes in with it.
    pub fn fly_in(&mut self) {
        if let Some(carried) = self.carried.take() {
            self.from = carried;
            self.progress = 0.0;
        }
    }

    /// Moves it along its trip toward the opening while `inside` and back
    /// to the table otherwise, at `seconds` on the clock; carried, it
    /// stays in hand, and let go, it falls.
    pub fn follow(&mut self, inside: bool, seconds: f64) {
        let dt = self
            .last_seconds
            .map_or(0.0, |last| (seconds - last).max(0.0) as f32);
        self.last_seconds = Some(seconds);
        if self.carried.is_some() {
            return;
        }
        if let Some((start, fallen)) = self.falling {
            let fallen = fallen + dt;
            let height = (start.0.y - self.lying.y).max(0.0);
            let done = DROP_GRAVITY * self.shape.bore * fallen * fallen / 2.0 >= height;
            self.falling = (!done).then_some((start, fallen));
            return;
        }
        let step = dt / TRIP_SECONDS;
        // Turned back halfway, a trip starts back from where it is.
        let target = if inside { 1.0 } else { 0.0 };
        self.progress += (target - self.progress).clamp(-step, step);
        // Seated, a trip out goes back to where it lies on the table.
        if self.progress >= 1.0 {
            self.from = (self.lying, Quat::IDENTITY);
        }
    }

    /// Draws it seen from `camera`, on the table, on its way, seated,
    /// carried or falling.
    pub fn draw(&mut self, camera: &Camera3D) {
        let (center, turn) = self.now();
        self.placed = Mat4::from_rotation_translation(turn, center);
        let light = key_light(camera.position);
        draw_lit_placed(
            [&self.mesh],
            self.material.as_ref(),
            camera,
            light,
            self.placed,
        );
    }

    /// Draws it again, faintly, over everything drawn so far, as posed by
    /// the last `draw`, if it is seated in the opening.
    pub fn draw_ghost(&self) {
        let Some(material) = self.ghost.as_ref().filter(|_| self.seated()) else {
            return;
        };
        material.set_uniform("Opacity", GHOST_OPACITY);
        material.draw([&self.mesh], self.placed);
    }
}

/// The ghost's material, blended over everything and hidden by nothing, or
/// `None` if its shader cannot be built.
fn load_ghost() -> Option<GpuMaterial> {
    let blend = BlendState::new(
        Equation::Add,
        BlendFactor::Value(BlendValue::SourceAlpha),
        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
    );
    let params = PipelineParams {
        depth_test: Comparison::Always,
        depth_write: false,
        color_blend: Some(blend),
        ..Default::default()
    };
    let uniforms = [("Opacity", UniformType::Float1)];
    GpuMaterial::new(GHOST_VERTEX, GHOST_FRAGMENT, &uniforms, params)
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/cartridge.rs"]
mod tests;
