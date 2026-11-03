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
//! like a ghost seen through it. Drawing only: what a press does is the
//! app's.

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
use macroquad::models::{
    Mesh,
    Vertex,
    draw_mesh, //
};
use macroquad::prelude::{
    Camera3D,
    Color,
    Comparison,
    Material,
    MaterialParams,
    PipelineParams,
    Quat,
    ShaderSource,
    UniformDesc,
    UniformType,
    Vec3,
    gl_use_default_material,
    gl_use_material,
    load_material,
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
use super::sprites::smoothstep;
use super::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY,
    draw_lit, //
};

/// The cartridge's radius against the opening's, so it slides in.
const FIT: f32 = 0.97;
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
fn line_up(shape: BlockShape) -> Vec3 {
    seat(shape) + Vec3::Z * (shape.outer + LINE_UP_OUT * shape.bore)
}

/// Where the cartridge's middle is and how it is turned `progress` of the
/// way (0 on the table, 1 seated) along its trip into a block of `shape`.
pub fn pose(shape: BlockShape, progress: f32) -> (Vec3, Quat) {
    let progress = progress.clamp(0.0, 1.0);
    // Lying flat on the table its axis is upright; in the opening it runs
    // along the shaft.
    let seated = Quat::from_rotation_z(-FRAC_PI_2);
    if progress < FLIGHT_SHARE {
        let share = smoothstep(0.0, 1.0, progress / FLIGHT_SHARE);
        let arc = (PI * share).sin() * ARC_HEIGHT * shape.bore;
        let center = rest(shape).lerp(line_up(shape), share) + Vec3::Y * arc;
        (center, Quat::IDENTITY.slerp(seated, share))
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
    /// Its shape as built, middle at the origin, axis upright.
    built: Vec<Vertex>,
    /// As posed this frame.
    mesh: Mesh,
    /// How far along its trip into the opening it is, from 0 to 1.
    progress: f32,
    /// The clock at the last pose, if there was one.
    last_seconds: Option<f64>,
    /// The block it goes into.
    shape: BlockShape,
    /// The lit-metal material, if it can be built.
    material: Option<Material>,
    /// The ghost's material, if it can be built.
    ghost: Option<Material>,
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
            for vertex in cartridge
                .built
                .iter_mut()
                .chain(&mut cartridge.mesh.vertices)
            {
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
            built: solid.vertices.clone(),
            mesh: Mesh {
                vertices: solid.vertices,
                indices: solid.indices,
                texture: None,
            },
            progress: if seated { 1.0 } else { 0.0 },
            last_seconds: None,
            shape,
            material: None,
            ghost: None,
        }
    }

    /// Whether it is all the way into the opening.
    pub fn seated(&self) -> bool {
        self.progress >= 1.0
    }

    /// Moves it along its trip toward the opening while `inside` and back
    /// to the table otherwise, at `seconds` on the clock.
    pub fn follow(&mut self, inside: bool, seconds: f64) {
        let step = self
            .last_seconds
            .map_or(0.0, |last| (seconds - last).max(0.0) as f32 / TRIP_SECONDS);
        self.last_seconds = Some(seconds);
        // Turned back halfway, a trip starts back from where it is.
        let target = if inside { 1.0 } else { 0.0 };
        self.progress += (target - self.progress).clamp(-step, step);
    }

    /// Draws it seen from `camera`, on the table, on its way or seated.
    pub fn draw(&mut self, camera: &Camera3D) {
        let (center, turn) = pose(self.shape, self.progress);
        for (vertex, built) in self.mesh.vertices.iter_mut().zip(&self.built) {
            vertex.position = center + turn * built.position;
            vertex.normal = (turn * built.normal.truncate()).extend(0.0);
        }
        let light = key_light(camera.position);
        draw_lit([&self.mesh], self.material.as_ref(), camera, light);
    }

    /// Draws it again, faintly, over everything drawn so far, as posed by
    /// the last `draw`, if it is seated in the opening.
    pub fn draw_ghost(&self) {
        let Some(material) = self.ghost.as_ref().filter(|_| self.seated()) else {
            return;
        };
        material.set_uniform("Opacity", GHOST_OPACITY);
        gl_use_material(material);
        draw_mesh(&self.mesh);
        gl_use_default_material();
    }
}

/// The ghost's material, blended over everything and hidden by nothing, or
/// `None` if its shader cannot be built.
fn load_ghost() -> Option<Material> {
    let source = ShaderSource::Glsl {
        vertex: GHOST_VERTEX,
        fragment: GHOST_FRAGMENT,
    };
    let blend = BlendState::new(
        Equation::Add,
        BlendFactor::Value(BlendValue::SourceAlpha),
        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
    );
    let params = MaterialParams {
        pipeline_params: PipelineParams {
            depth_test: Comparison::Always,
            depth_write: false,
            color_blend: Some(blend),
            ..Default::default()
        },
        uniforms: vec![UniformDesc::new("Opacity", UniformType::Float1)],
        ..Default::default()
    };
    load_material(source, params).ok()
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/cartridge.rs"]
mod tests;
