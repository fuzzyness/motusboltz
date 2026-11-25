//! The regenerator's slot and its sliding door, in first person.
//!
//! In a two-cylinder Stirling engine the regenerator sits in the passage
//! between the cylinders, and here the cartridge seats in the opening
//! through the shared wall. With the block drawn whole in first person,
//! it goes in through a slot in the front face, straight back into the
//! opening: as wide as the cartridge is thick and as tall as the opening,
//! lined with steel, so a disc of screens slides in edgewise across the
//! gas's path. The block's shader cuts the slot (see `block.rs`); this
//! draws its lining, the rails on the front face and the door between
//! them, which slides up as the cartridge arrives and stays up while it is
//! in, and slides down to seal the slot once it is out. Drawing only.

use macroquad::models::Mesh;
use macroquad::prelude::{
    Camera3D,
    Color,
    Mat4,
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
    Solid,
    finish,
    load_metal, //
};
use super::sprites::smoothstep;
use super::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY,
    draw_lit_placed, //
};

/// How far the door reaches past the slot each way, in bore radii.
const DOOR_LIP: f32 = 0.05;
/// The door's thickness, in bore radii.
const DOOR_THICKNESS: f32 = 0.035;
/// The gap between the door and the front face, in bore radii.
const DOOR_GAP: f32 = 0.004;
/// Width of each rail beside the door, in bore radii.
const RAIL_WIDTH: f32 = 0.045;
/// How far the rails stand out past the door's face, in bore radii.
const RAIL_PAST_DOOR: f32 = 0.015;
/// Height of the stop across the rails' tops, in bore radii.
const STOP_HEIGHT: f32 = 0.045;
/// Height of the grip on the door above its bottom, in bore radii.
const GRIP_HEIGHT: f32 = 0.22;
/// Half the grip's width, as a share of half the door's.
const GRIP_SHARE: f32 = 0.6;
/// How far the grip stands off the door, in bore radii.
const GRIP_STANDOFF: f32 = 0.045;
/// Radius of the grip's bar and its posts, in bore radii.
const GRIP_RADIUS: f32 = 0.012;
/// Shares of the door's height where grooves run across it.
const GROOVES: [f32; 2] = [0.45, 0.6];
/// Half the height of each groove, in bore radii.
const GROOVE_HALF: f32 = 0.012;
/// How far a groove's ridge stands off the door, in bore radii.
const GROOVE_PROUD: f32 = 0.006;
/// Steps up the lining's sides, round the opening's curve.
const LINING_STEPS: usize = 24;
/// Segments round the grip's bar and posts.
const GRIP_SEGMENTS: usize = 10;
/// Seconds the door takes to open or close.
pub const DOOR_SECONDS: f32 = 0.35;
/// The door: the teal of the opening it seals.
const DOOR_STEEL: Color = finish(52, 196, 168, 0.45);
/// The grooves' ridges.
const GROOVE_STEEL: Color = finish(36, 150, 128, 0.35);
/// The rails and their stop.
const RAIL_STEEL: Color = finish(132, 140, 156, 0.6);
/// The slot's lining, inside the block.
const LINING_STEEL: Color = finish(84, 92, 108, 0.15);
/// The grip.
const GRIP_CHROME: Color = finish(214, 220, 230, 1.0);

/// The slot through the front face into the opening.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlotShape {
    /// Half its width: half the shared wall's thickness, the cartridge's.
    pub half_width: f32,
    /// Height of its bottom, the bores' floors'.
    pub bottom: f32,
    /// Height of its top, the opening's.
    pub top: f32,
    /// Where the front face is, along z.
    pub front: f32,
}

/// The slot in a block of `shape`: from the front face straight back into
/// the opening, as wide as the shared wall is thick and as tall as the
/// opening.
pub fn slot(shape: BlockShape) -> SlotShape {
    SlotShape {
        half_width: shape.offset - shape.bore,
        bottom: shape.opening_y - shape.opening,
        top: shape.opening_y + shape.opening,
        front: shape.outer,
    }
}

/// How far the door and its rails reach either side of the slot's middle.
pub fn hatch_reach(shape: BlockShape) -> f32 {
    slot(shape).half_width + (DOOR_LIP + RAIL_WIDTH) * shape.bore
}

/// How far the door rises to open, clear of the slot: its own height.
pub fn door_travel(shape: BlockShape) -> f32 {
    let slot = slot(shape);
    slot.top - slot.bottom + 2.0 * DOOR_LIP * shape.bore
}

/// How far in front of the opening's middle the opening's round edge is
/// at height `y`, along z: 0 above and below it.
fn opening_edge(shape: BlockShape, y: f32) -> f32 {
    let from_middle = y - shape.opening_y;
    (shape.opening * shape.opening - from_middle * from_middle)
        .max(0.0)
        .sqrt()
}

/// The slot's lining, the rails and their stop, which stay put.
fn frame_solid(shape: BlockShape) -> Solid {
    let unit = shape.bore;
    let slot = slot(shape);
    let mut solid = Solid::new();
    // The lining's sides, facing into the slot, from the opening's edge
    // out to the front face.
    for side in [-1.0_f32, 1.0] {
        let x = side * slot.half_width;
        let inward = Vec3::X * -side;
        for step in 0..LINING_STEPS {
            let height = |step: usize| {
                let share = step as f32 / LINING_STEPS as f32;
                slot.bottom + (slot.top - slot.bottom) * share
            };
            let (low, high) = (height(step), height(step + 1));
            let corners = [
                vec3(x, low, opening_edge(shape, low)),
                vec3(x, low, slot.front),
                vec3(x, high, slot.front),
                vec3(x, high, opening_edge(shape, high)),
            ];
            solid.quad(corners, [inward; 4], [LINING_STEEL; 4]);
        }
    }
    // Its top and bottom, facing into the slot.
    for (y, inward) in [(slot.top, -Vec3::Y), (slot.bottom, Vec3::Y)] {
        let back = opening_edge(shape, y);
        let corners = [
            vec3(-slot.half_width, y, back),
            vec3(slot.half_width, y, back),
            vec3(slot.half_width, y, slot.front),
            vec3(-slot.half_width, y, slot.front),
        ];
        solid.quad(corners, [inward; 4], [LINING_STEEL; 4]);
    }
    // The rails, either side of the door, from its foot to above where it
    // stands open, and the stop across their tops.
    let door_half = slot.half_width + DOOR_LIP * unit;
    let rail_half = RAIL_WIDTH * unit / 2.0;
    let depth = (DOOR_GAP + DOOR_THICKNESS + RAIL_PAST_DOOR) * unit;
    let middle_z = slot.front + depth / 2.0;
    let foot = slot.bottom - DOOR_LIP * unit;
    let head = slot.top + DOOR_LIP * unit + door_travel(shape) + STOP_HEIGHT * unit;
    for side in [-1.0_f32, 1.0] {
        let x = side * (door_half + rail_half);
        solid.bar(
            vec3(x, foot, middle_z),
            vec3(x, head, middle_z),
            Vec3::X,
            (rail_half, rail_half),
            depth / 2.0,
            RAIL_STEEL,
        );
    }
    let stop_half = STOP_HEIGHT * unit / 2.0;
    let reach = hatch_reach(shape);
    solid.bar(
        vec3(-reach, head - stop_half, middle_z),
        vec3(reach, head - stop_half, middle_z),
        Vec3::Y,
        (stop_half, stop_half),
        depth / 2.0,
        RAIL_STEEL,
    );
    solid
}

/// The door, shut, with its grooves and grip.
fn door_solid(shape: BlockShape) -> Solid {
    let unit = shape.bore;
    let slot = slot(shape);
    let mut solid = Solid::new();
    let half = slot.half_width + DOOR_LIP * unit;
    let thickness = DOOR_THICKNESS * unit;
    let middle_z = slot.front + DOOR_GAP * unit + thickness / 2.0;
    let face = middle_z + thickness / 2.0;
    let foot = slot.bottom - DOOR_LIP * unit;
    let height = door_travel(shape);
    solid.bar(
        vec3(0.0, foot, middle_z),
        vec3(0.0, foot + height, middle_z),
        Vec3::X,
        (half, half),
        thickness / 2.0,
        DOOR_STEEL,
    );
    let groove = GROOVE_HALF * unit;
    let proud = GROOVE_PROUD * unit;
    for share in GROOVES {
        let y = foot + height * share;
        solid.bar(
            vec3(-half, y, face + proud / 2.0),
            vec3(half, y, face + proud / 2.0),
            Vec3::Y,
            (groove, groove),
            proud / 2.0,
            GROOVE_STEEL,
        );
    }
    let grip_y = foot + GRIP_HEIGHT * unit;
    let grip_x = GRIP_SHARE * half;
    let out = face + GRIP_STANDOFF * unit;
    let radius = GRIP_RADIUS * unit;
    solid.cylinder(
        vec3(-grip_x, grip_y, out),
        vec3(grip_x, grip_y, out),
        radius,
        GRIP_SEGMENTS,
        GRIP_CHROME,
    );
    for side in [-1.0_f32, 1.0] {
        solid.cylinder(
            vec3(side * grip_x, grip_y, face),
            vec3(side * grip_x, grip_y, out),
            radius,
            GRIP_SEGMENTS,
            GRIP_CHROME,
        );
    }
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

/// The slot's lining, rails and door, and how far the door stands open.
pub struct Hatch {
    /// The lining, rails and stop.
    frame: GpuMesh,
    /// The door, shut; it is moved up as it opens.
    door: GpuMesh,
    /// How far the door rises to open.
    travel: f32,
    /// How far open the door is, from 0 to 1, before easing.
    open: f32,
    /// The clock at the last `follow`, if there was one.
    last_seconds: Option<f64>,
    /// The lit-metal material, if it can be built.
    material: Option<GpuMaterial>,
}

impl Hatch {
    /// The hatch of a block of `shape`, its door shut.
    pub fn new(shape: BlockShape) -> Self {
        let material = load_metal();
        let plain = material.is_none();
        Self {
            frame: upload(frame_solid(shape), plain),
            door: upload(door_solid(shape), plain),
            travel: door_travel(shape),
            open: 0.0,
            last_seconds: None,
            material,
        }
    }

    /// Slides the door toward open while `open` and shut otherwise, at
    /// `seconds` on the clock.
    pub fn follow(&mut self, open: bool, seconds: f64) {
        let dt = self
            .last_seconds
            .map_or(0.0, |last| (seconds - last).max(0.0) as f32);
        self.last_seconds = Some(seconds);
        let step = dt / DOOR_SECONDS;
        let target = if open { 1.0 } else { 0.0 };
        self.open += (target - self.open).clamp(-step, step);
    }

    /// How far the door has risen, eased in and out.
    pub fn lift(&self) -> f32 {
        smoothstep(0.0, 1.0, self.open) * self.travel
    }

    /// Draws the lining, the rails and the door seen from `camera`.
    pub fn draw(&self, camera: &Camera3D) {
        let light = key_light(camera.position);
        let material = self.material.as_ref();
        draw_lit_placed([&self.frame], material, camera, light, Mat4::IDENTITY);
        let raised = Mat4::from_translation(Vec3::Y * self.lift());
        draw_lit_placed([&self.door], material, camera, light, raised);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/hatch.rs"]
mod tests;
