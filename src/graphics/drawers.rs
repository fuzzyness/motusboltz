//! The desk's pedestal of drawers, a way down to the ground and back.
//!
//! At the desk's right end, under its top, a pedestal stands on the
//! ground from the back to the front, on a base with an arched cut at its
//! foot, holding a stack of three drawers of honey pine, each with a
//! raised panel on its front and a round wooden knob. In first person a
//! drawer is grabbed by its front and pulled out as far as the mouse
//! pulls it, or pushed back in, held in its runners. Pulled out, each is a step: a figure on the
//! ground jumps onto the lowest and climbs from drawer to drawer up to the
//! table's top, each a plain jump above the last, or the other way down,
//! when each is pulled out further than the one above. In the orbit view
//! they are only seen.
//!
//! Drawing only, with the drawers' sizes for walking (see
//! `ui::walking`).

use macroquad::models::Mesh;
use macroquad::prelude::{
    Camera3D,
    Color,
    Mat4,
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
    turn, //
};
use super::table::{
    ALONG_SHAFT,
    OVERHANG,
    TOP_HALF_DEPTH,
    TOP_HALF_LENGTH,
    WOOD_FRAGMENT,
    ground,
    underside, //
};
use super::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY,
    draw_lit_placed,
    load_lit, //
};

/// Drawers in the pedestal, bottom to top.
pub const DRAWER_COUNT: usize = 3;
/// Width of the pedestal, in bore radii, in from the desk's right end.
const PEDESTAL_WIDTH: f32 = 6.0;
/// Thickness of its sides and back, in bore radii.
const PANEL: f32 = 0.25;
/// Height of the base along its foot, in bore radii: low, so the lowest
/// drawer is still a plain jump up from the ground.
const KICK: f32 = 0.3;
/// How far up into the base its arch is cut at the middle, as a share of
/// the base's height.
const ARCH_RISE: f32 = 0.55;
/// How much of the base's width the arch spans.
const ARCH_SPAN: f32 = 0.78;
/// Steps along the arch.
const ARCH_STEPS: usize = 24;
/// How far back from the drawers' faces the base's face stands, in bore
/// radii.
const KICK_SET_BACK: f32 = 0.05;
/// Height of the gap between one drawer and the next, in bore radii.
const GAP: f32 = 0.06;
/// Thickness of a drawer's front, in bore radii.
const FRONT_THICKNESS: f32 = 0.12;
/// Thickness of a drawer's sides, back and bottom, in bore radii.
const BOX_THICKNESS: f32 = 0.06;
/// How far short of the cabinet's back a drawer stops, in bore radii.
const BACK_ROOM: f32 = 0.2;
/// How much of a drawer stays in the cabinet when pulled all the way out,
/// held by its runners, in bore radii.
const HELD_IN: f32 = 0.6;
/// How far in from a drawer front's edges its raised panel starts, in
/// bore radii.
const PANEL_MARGIN: f32 = 0.13;
/// How far in the raised panel's bevel reaches to its flat field, in bore
/// radii.
const PANEL_BEVEL: f32 = 0.12;
/// How far the raised panel's field stands out from the front, in bore
/// radii.
const PANEL_RISE: f32 = 0.05;
/// Radius of a drawer's knob, round its dome, in bore radii.
const KNOB_RADIUS: f32 = 0.2;
/// How far a drawer's knob stands out from its front, in bore radii.
const KNOB_REACH: f32 = 0.3;
/// Radius of a knob's neck, in bore radii.
const KNOB_NECK: f32 = 0.08;
/// Segments round a knob.
const KNOB_SEGMENTS: usize = 16;
/// A drawer's front: honey pine, a little warmer than the desk's top.
const FRONT_WOOD: Color = Color::new(0.95, 0.69, 0.37, ALONG_SHAFT);
/// A drawer's box, inside: plain pale wood.
const BOX_WOOD: Color = Color::new(0.88, 0.72, 0.5, ALONG_SHAFT);
/// The knobs, turned from the same pine.
const KNOB_WOOD: Color = Color::new(0.93, 0.66, 0.34, ALONG_SHAFT);

/// Where the pedestal stands: its sides, front and back, foot and top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CabinetPlace {
    /// Its left side's outside, across.
    pub left: f32,
    /// Its right side's outside.
    pub right: f32,
    /// Its front, the drawers' faces, the top's overhang in from its edge.
    pub front: f32,
    /// Its back's outside.
    pub back: f32,
    /// Its foot, on the ground.
    pub bottom: f32,
    /// Its top, against the table's top.
    pub top: f32,
    /// The bore's radius, the unit of its parts' sizes.
    pub unit: f32,
}

/// One drawer's room in the cabinet, shut.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawerPlace {
    /// Its sides, across, outside.
    pub left: f32,
    /// Its right side.
    pub right: f32,
    /// Its front's face, shut.
    pub front: f32,
    /// Its back's outside, shut.
    pub back: f32,
    /// Its bottom's underside.
    pub bottom: f32,
    /// The top of its front and sides: what stands on it stands here.
    pub top: f32,
    /// Farthest it pulls out.
    pub most: f32,
}

impl DrawerPlace {
    /// Where the middle of its front's top edge is, pulled out by `pull`:
    /// what is aimed at to grab it.
    pub fn grip(self, pull: f32) -> Vec3 {
        vec3((self.left + self.right) / 2.0, self.top, self.front + pull)
    }
}

/// Where the pedestal stands under the desk of a block of `shape`: at its
/// right end, from its back to its front.
pub fn cabinet_place(shape: BlockShape) -> CabinetPlace {
    let unit = shape.bore;
    let right = (TOP_HALF_LENGTH - OVERHANG) * unit;
    CabinetPlace {
        left: right - PEDESTAL_WIDTH * unit,
        right,
        front: (TOP_HALF_DEPTH - OVERHANG) * unit,
        back: -TOP_HALF_DEPTH * unit,
        bottom: ground(shape),
        top: underside(shape),
        unit,
    }
}

/// Where each drawer sits in the cabinet under the table of a block of
/// `shape`, bottom to top: each the same height, stacked from the base
/// up to the desk's top.
pub fn drawer_places(shape: BlockShape) -> [DrawerPlace; DRAWER_COUNT] {
    let cabinet = cabinet_place(shape);
    let unit = cabinet.unit;
    let low = cabinet.bottom + KICK * unit;
    let band = (cabinet.top - low) / DRAWER_COUNT as f32;
    let length = cabinet.front - cabinet.back - (PANEL + BACK_ROOM) * unit;
    std::array::from_fn(|index| {
        let bottom = low + band * index as f32 + GAP * unit / 2.0;
        DrawerPlace {
            left: cabinet.left + PANEL * unit,
            right: cabinet.right - PANEL * unit,
            front: cabinet.front,
            back: cabinet.front - length,
            bottom,
            top: bottom + band - GAP * unit,
            most: length - HELD_IN * unit,
        }
    })
}

/// Adds the pedestal's sides, back and arched base to `solid` in `wood`;
/// the desk's top closes it above, and the drawers fill its front.
pub fn cabinet_solid(solid: &mut Solid, cabinet: CabinetPlace, wood: Color) {
    let unit = cabinet.unit;
    let panel = PANEL * unit / 2.0;
    let (low, high) = (cabinet.bottom, cabinet.top);
    let depth = (cabinet.front - cabinet.back) / 2.0;
    let middle_z = (cabinet.front + cabinet.back) / 2.0;
    for x in [cabinet.left + panel, cabinet.right - panel] {
        solid.bar(
            vec3(x, low, middle_z),
            vec3(x, high, middle_z),
            Vec3::Z,
            (depth, depth),
            panel,
            wood,
        );
    }
    let width = (cabinet.right - cabinet.left) / 2.0;
    let middle_x = (cabinet.left + cabinet.right) / 2.0;
    solid.bar(
        vec3(middle_x, low, cabinet.back + panel),
        vec3(middle_x, high, cabinet.back + panel),
        Vec3::X,
        (width, width),
        panel,
        wood,
    );
    let face = cabinet.front - KICK_SET_BACK * unit;
    arched_base(
        solid,
        (cabinet.left + 2.0 * panel, cabinet.right - 2.0 * panel),
        (low, low + KICK * unit),
        (face - 2.0 * panel, face),
        Color {
            a: ALONG_SHAFT,
            ..wood
        },
    );
}

/// Adds the base board at the pedestal's foot to `solid` in `wood`:
/// across from the first of `sides` to the second, up from the first of
/// `heights` to the second, and from the first of `depths` back to the
/// second at its face, an arch cut up into it from its bottom edge at the
/// middle, leaving a foot at each end.
fn arched_base(
    solid: &mut Solid,
    sides: (f32, f32),
    heights: (f32, f32),
    depths: (f32, f32),
    wood: Color,
) {
    let (left, right) = sides;
    let (low, high) = heights;
    let (back, face) = depths;
    let middle = (left + right) / 2.0;
    let half_span = ARCH_SPAN * (right - left) / 2.0;
    let rise = ARCH_RISE * (high - low);
    // The bottom edge's height at `x`: up into the board under the arch,
    // a half ellipse, its ends square to the ground.
    let bottom = |x: f32| {
        let share = ((x - middle) / half_span).clamp(-1.0, 1.0);
        low + rise * (1.0 - share * share).max(0.0).sqrt()
    };
    let points: Vec<f32> = std::iter::once(left)
        .chain(
            (0..=ARCH_STEPS)
                .map(|step| middle - half_span + 2.0 * half_span * step as f32 / ARCH_STEPS as f32),
        )
        .chain(std::iter::once(right))
        .collect();
    let colors = [wood; 4];
    for pair in points.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let (from_low, to_low) = (bottom(from), bottom(to));
        for (z, normal) in [(face, Vec3::Z), (back, -Vec3::Z)] {
            solid.quad(
                [
                    vec3(from, from_low, z),
                    vec3(to, to_low, z),
                    vec3(to, high, z),
                    vec3(from, high, z),
                ],
                [normal; 4],
                colors,
            );
        }
        // The bottom edge, facing down and out of the arch.
        let along = vec3(to - from, to_low - from_low, 0.0);
        let normal = along.cross(Vec3::Z).normalize_or(-Vec3::Y);
        let normal = if normal.y > 0.0 { -normal } else { normal };
        solid.quad(
            [
                vec3(from, from_low, back),
                vec3(to, to_low, back),
                vec3(to, to_low, face),
                vec3(from, from_low, face),
            ],
            [normal; 4],
            colors,
        );
    }
    solid.quad(
        [
            vec3(left, high, back),
            vec3(right, high, back),
            vec3(right, high, face),
            vec3(left, high, face),
        ],
        [Vec3::Y; 4],
        colors,
    );
    for (x, normal) in [(left, -Vec3::X), (right, Vec3::X)] {
        solid.quad(
            [
                vec3(x, low, back),
                vec3(x, low, face),
                vec3(x, high, face),
                vec3(x, high, back),
            ],
            [normal; 4],
            colors,
        );
    }
}

/// Adds the raised panel on the face of `drawer`'s front to `solid`: a
/// flat field standing out from the front, a bevel sloping down from it
/// on every side.
fn raised_panel(solid: &mut Solid, drawer: DrawerPlace, unit: f32) {
    let margin = PANEL_MARGIN * unit;
    let base = (
        drawer.left + margin,
        drawer.right - margin,
        drawer.bottom + margin,
        drawer.top - margin,
    );
    let bevel = PANEL_BEVEL * unit;
    let field = (
        base.0 + bevel,
        base.1 - bevel,
        base.2 + bevel,
        base.3 - bevel,
    );
    let (face, raised) = (drawer.front, drawer.front + PANEL_RISE * unit);
    let corners = |(left, right, bottom, top): (f32, f32, f32, f32), z: f32| {
        [
            vec3(left, bottom, z),
            vec3(right, bottom, z),
            vec3(right, top, z),
            vec3(left, top, z),
        ]
    };
    let (outer, inner) = (corners(base, face), corners(field, raised));
    let colors = [FRONT_WOOD; 4];
    solid.quad(inner, [Vec3::Z; 4], colors);
    for side in 0..4 {
        let next = (side + 1) % 4;
        let slope = [outer[side], outer[next], inner[next], inner[side]];
        let normal = (slope[1] - slope[0])
            .cross(slope[3] - slope[0])
            .normalize_or(Vec3::Z);
        let normal = if normal.z < 0.0 { -normal } else { normal };
        solid.quad(slope, [normal; 4], colors);
    }
}

/// One drawer, shut, with its front, raised panel, knob, sides, back and
/// bottom in wood.
pub(super) fn drawer_solid(drawer: DrawerPlace, unit: f32) -> Solid {
    let mut solid = Solid::new();
    let half_width = (drawer.right - drawer.left) / 2.0;
    let middle_x = (drawer.left + drawer.right) / 2.0;
    let wall = BOX_THICKNESS * unit / 2.0;
    let front = FRONT_THICKNESS * unit / 2.0;
    // The front, its face where the cabinet's front is.
    solid.bar(
        vec3(middle_x, drawer.bottom, drawer.front - front),
        vec3(middle_x, drawer.top, drawer.front - front),
        Vec3::X,
        (half_width, half_width),
        front,
        FRONT_WOOD,
    );
    let inside_front = drawer.front - 2.0 * front;
    let half_length = (inside_front - drawer.back) / 2.0;
    let middle_z = (inside_front + drawer.back) / 2.0;
    for x in [drawer.left + wall, drawer.right - wall] {
        solid.bar(
            vec3(x, drawer.bottom, middle_z),
            vec3(x, drawer.top, middle_z),
            Vec3::Z,
            (half_length, half_length),
            wall,
            BOX_WOOD,
        );
    }
    solid.bar(
        vec3(middle_x, drawer.bottom, drawer.back + wall),
        vec3(middle_x, drawer.top, drawer.back + wall),
        Vec3::X,
        (half_width, half_width),
        wall,
        BOX_WOOD,
    );
    solid.bar(
        vec3(middle_x, drawer.bottom, middle_z),
        vec3(middle_x, drawer.bottom + 2.0 * wall, middle_z),
        Vec3::X,
        (half_width, half_width),
        half_length,
        BOX_WOOD,
    );
    raised_panel(&mut solid, drawer, unit);
    knob(&mut solid, drawer, unit);
    solid
}

/// Adds a drawer's knob to `solid`, on the middle of its raised panel: a
/// round wooden mushroom on a short neck.
fn knob(solid: &mut Solid, drawer: DrawerPlace, unit: f32) {
    let middle = vec3(
        (drawer.left + drawer.right) / 2.0,
        (drawer.bottom + drawer.top) / 2.0,
        drawer.front + PANEL_RISE * unit,
    );
    let (radius, reach, neck) = (KNOB_RADIUS * unit, KNOB_REACH * unit, KNOB_NECK * unit);
    solid.lathe(
        Frame::along(middle, Vec3::Z),
        &[
            turn(0.0, 0.0, KNOB_WOOD),
            turn(1.5 * neck, 0.0, KNOB_WOOD),
            turn(neck, 0.3 * reach, KNOB_WOOD),
            turn(0.85 * radius, 0.5 * reach, KNOB_WOOD),
            turn(radius, 0.68 * reach, KNOB_WOOD),
            turn(0.85 * radius, 0.88 * reach, KNOB_WOOD),
            turn(0.45 * radius, 0.98 * reach, KNOB_WOOD),
            turn(0.0, reach, KNOB_WOOD),
        ],
        KNOB_SEGMENTS,
    );
}

/// `solid` on the graphics card.
///
/// # Panics
///
/// Panics if it would not fit Macroquad's buffers.
fn upload(solid: Solid) -> GpuMesh {
    assert!(solid.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
    assert!(solid.indices.len() < DRAW_CALL_INDEX_CAPACITY);
    GpuMesh::new(Mesh {
        vertices: solid.vertices,
        indices: solid.indices,
        texture: None,
    })
}

/// The drawers: where each sits, how far each is pulled out, and what
/// draws them.
pub struct Drawers {
    /// Where each sits, shut, bottom to top.
    places: [DrawerPlace; DRAWER_COUNT],
    /// How far each is pulled out.
    pulls: [f32; DRAWER_COUNT],
    /// Each one's wooden box, front and knob, shut.
    boxes: Vec<GpuMesh>,
    /// The wood's material: untried, ready, or not to be had.
    wood: Option<Option<GpuMaterial>>,
}

impl Drawers {
    /// The drawers of the table under a block of `shape`, all shut.
    pub fn new(shape: BlockShape) -> Self {
        let places = drawer_places(shape);
        let unit = shape.bore;
        Self {
            places,
            pulls: [0.0; DRAWER_COUNT],
            boxes: places
                .iter()
                .map(|&place| upload(drawer_solid(place, unit)))
                .collect(),
            wood: None,
        }
    }

    /// Where each drawer sits, shut, bottom to top.
    pub fn places(&self) -> [DrawerPlace; DRAWER_COUNT] {
        self.places
    }

    /// How far each drawer is pulled out.
    pub fn pulls(&self) -> [f32; DRAWER_COUNT] {
        self.pulls
    }

    /// Pulls drawer `index` out by `by` more (in if less than 0), held
    /// between shut and as far as its runners let it go.
    pub fn pull(&mut self, index: usize, by: f32) {
        if let (Some(pull), Some(place)) = (self.pulls.get_mut(index), self.places.get(index)) {
            *pull = (*pull + by).clamp(0.0, place.most);
        }
    }

    /// Draws the drawers seen from `camera`, each where it is pulled to.
    pub fn draw(&mut self, camera: &Camera3D) {
        let light = key_light(camera.position);
        let wood = self.wood.get_or_insert_with(|| {
            let material = load_lit(WOOD_FRAGMENT, &[("Shift", UniformType::Float3)]);
            if material.is_none() {
                // Drawn in plain colors, alpha means transparency.
                for mesh in &mut self.boxes {
                    for vertex in &mut mesh.edit().vertices {
                        vertex.color[3] = u8::MAX;
                    }
                }
            }
            material
        });
        for (mesh, pull) in self.boxes.iter().zip(self.pulls) {
            let shift = Vec3::Z * pull;
            let placed = Mat4::from_translation(shift);
            if let Some(wood) = wood {
                wood.set_uniform("Shift", shift);
            }
            draw_lit_placed([mesh], wood.as_ref(), camera, light, placed);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/drawers.rs"]
mod tests;
