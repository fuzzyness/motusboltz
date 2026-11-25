//! The glass windows in the cylinders' walls, in first person.
//!
//! Up close the orbit view's cutaway, which opens each cylinder toward
//! the camera wherever it stands, makes no sense, so in first person the
//! block is drawn solid with windows in its walls instead. Each cylinder
//! has two broad panes, one toward the front and one toward the back,
//! each wrapping round its curve from beside the regenerator's slot to
//! near its far end, so the gas is in plain view from most places on the
//! table. The block's shader cuts the holes and paints a bolted frame round
//! each (see `block.rs`); this draws the glass in them, after everything
//! inside, which shows through it.
//!
//! Angles round a cylinder are measured from the way toward the shared
//! wall, toward the front; the cold cylinder's panes are the hot one's
//! mirror image. Drawing only.

use std::f32::consts::PI;

use macroquad::models::{
    Mesh,
    Vertex, //
};
use macroquad::prelude::{
    Camera3D,
    Color,
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
    Solid,
    finish,
    load_metal, //
};
use super::{
    draw_lit,
    draw_lit_placed,
    load_clear_with, //
};

/// Where the middle of each cylinder's front pane is round it, in radians
/// from the way toward the shared wall, toward the front: 107.5 degrees,
/// halfway between the slot's side and the far end. The back's is the
/// same toward the back.
pub const PANE_ANGLE: f32 = 43.0 * PI / 72.0;
/// Half the angle each pane spans round its cylinder, in radians: 52.5
/// degrees, from 55 degrees, clear of the opening and the slot's rails,
/// to 160, leaving a pillar of wall at the far end between the front
/// pane and the back.
pub const PANE_HALF_ANGLE: f32 = 7.0 * PI / 24.0;
/// Wall left above and below the panes, in bore radii.
const PANE_MARGIN: f32 = 0.18;
/// Radius of the panes' rounded corners, in bore radii.
const PANE_CORNER: f32 = 0.12;
/// Width of the bolted frame round each pane, in bore radii.
pub const FRAME_BAND: f32 = 0.08;
/// Distance between the bolts round a frame, in bore radii.
pub const BOLT_SPACING: f32 = 0.36;
/// Radius of a bolt's head, in bore radii.
pub const BOLT_RADIUS: f32 = 0.022;
/// How far the glass stands off the wall's outside, in bore radii, so it
/// never fights the wall for the same depth.
const GLASS_LIFT: f32 = 0.004;
/// Columns of the glass across each pane, following the wall's curve.
const GLASS_COLUMNS: usize = 32;
/// Steps round each of a window's rounded corners, for its sill.
const SILL_CORNER_STEPS: usize = 6;
/// The sills lining each window through the wall: the frames' dark steel.
const SILL_STEEL: Color = finish(62, 66, 79, 0.3);
/// Tint of the glass.
const GLASS_TINT: Color = Color::from_rgba(196, 228, 255, 255);

/// Fragment shader of the glass: faintly tinted, clearer face on, with a
/// dark seal round its edge, a bright rim where it turns away and two
/// soft streaks of reflected light across it.
pub(super) const GLASS_FRAGMENT: &str = r#"#version 100
precision mediump float;

varying lowp vec4 color;
varying lowp vec2 uv;
varying vec3 world;
varying vec3 surface_normal;

uniform sampler2D Texture;
uniform vec3 Eye;
uniform vec3 Light;
// The pane: half width round the wall, half height, corner radius, and
// the seal's width; `uv` is the place on it from its middle, as a share
// of its half width and half height.
uniform vec4 Pane;

void main() {
    vec2 place = uv * Pane.xy;
    vec2 corner = abs(place) - (Pane.xy - Pane.z);
    float edge = length(max(corner, 0.0)) + min(max(corner.x, corner.y), 0.0) - Pane.z;
    if (edge > 0.0) {
        discard;
    }
    vec3 n = normalize(surface_normal);
    vec3 toward_eye = normalize(Eye - world);
    float facing = abs(dot(n, toward_eye));
    float rim = pow(1.0 - facing, 3.0);
    float shine = pow(max(dot(n, normalize(Light + toward_eye)), 0.0), 60.0);
    // Two streaks slanting across the pane, a broad faint one and a thin
    // bright one beside it, repeating down tall panes.
    float slant = fract((place.x + 0.3 * place.y) / (4.0 * Pane.x) + 0.25);
    float streaks = 0.5 * smoothstep(0.12, 0.0, abs(slant - 0.35))
        + 0.6 * smoothstep(0.03, 0.0, abs(slant - 0.52));
    float light = clamp(streaks + shine, 0.0, 1.0);
    vec3 lit = mix(color.rgb, vec3(1.0), light);
    float alpha = clamp(0.1 + 0.45 * rim + 0.3 * streaks + shine, 0.0, 0.85);
    // A dark rubber seal where the glass meets its frame.
    if (edge > -Pane.w) {
        lit = vec3(0.08, 0.09, 0.11);
        alpha = 0.95;
    }
    gl_FragColor = vec4(lit, alpha) * texture2D(Texture, uv);
}"#;

/// Where a cylinder's panes stand: the angles they span round it and the
/// heights of their bottoms and tops, with their corners and frames.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaneLayout {
    /// Half the angle each pane spans, in radians.
    pub half_angle: f32,
    /// Height of the panes' bottoms.
    pub bottom: f32,
    /// Height of their tops.
    pub top: f32,
    /// Radius of their corners.
    pub corner: f32,
    /// Width of the frame round each.
    pub frame: f32,
    /// Radius of the cylinders' bores, which the panes' widths are
    /// measured round.
    pub bore: f32,
}

impl PaneLayout {
    /// The panes in a block of `shape`.
    pub fn new(shape: BlockShape) -> Self {
        let unit = shape.bore;
        Self {
            half_angle: PANE_HALF_ANGLE,
            bottom: shape.floor + PANE_MARGIN * unit,
            top: shape.top - PANE_MARGIN * unit,
            corner: PANE_CORNER * unit,
            frame: FRAME_BAND * unit,
            bore: unit,
        }
    }

    /// Half a pane's width round its bore and half its height.
    pub fn half_size(self) -> (f32, f32) {
        (self.half_angle * self.bore, (self.top - self.bottom) / 2.0)
    }
}

/// Where a line out from the hot bore's axis at `angle` (from the way
/// toward the shared wall, toward the front) meets the outside of the
/// block, level with the axis's foot, and the way straight out of the
/// wall there: on the block's rounded end, or on its flat front or back
/// face. The panes never reach the plane between the halves.
pub fn outer_point(shape: BlockShape, angle: f32) -> (Vec3, Vec3) {
    let axis = vec3(-shape.offset, 0.0, 0.0);
    let (sin, cos) = angle.sin_cos();
    let way = vec3(cos, 0.0, sin);
    if cos <= 0.0 || sin.abs() <= f32::EPSILON {
        (axis + way * shape.outer, way)
    } else {
        let to_face = shape.outer / sin.abs();
        (axis + way * to_face, vec3(0.0, 0.0, sin.signum()))
    }
}

/// Every pane's middle angle round its cylinder: toward the front and
/// toward the back.
pub fn pane_middles() -> impl Iterator<Item = f32> {
    [PANE_ANGLE, -PANE_ANGLE].into_iter()
}

/// The glass of every pane in a block of `shape`, the cold cylinder's
/// mirroring the hot one's. Each vertex's texture coordinates are its
/// place on its pane from the pane's middle, round the wall and up, as a
/// share of the pane's half width and half height: small numbers, which
/// the shaders' low-precision varyings carry exactly enough.
fn glass_mesh(shape: BlockShape) -> Mesh {
    let layout = PaneLayout::new(shape);
    let lift = GLASS_LIFT * shape.bore;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for mirror in [false, true] {
        let flip = |point: Vec3| {
            if mirror {
                vec3(-point.x, point.y, point.z)
            } else {
                point
            }
        };
        for middle in pane_middles() {
            let first = vertices.len() as u16;
            for column in 0..=GLASS_COLUMNS {
                let share = column as f32 / GLASS_COLUMNS as f32;
                let across = (2.0 * share - 1.0) * layout.half_angle;
                let (point, out) = outer_point(shape, middle + across);
                let point = point + out * lift;
                let along = 2.0 * share - 1.0;
                for (height, up) in [(layout.bottom, -1.0), (layout.top, 1.0)] {
                    let place = vec3(point.x, height, point.z);
                    let mut vertex = Vertex::new2(flip(place), vec2(along, up), GLASS_TINT);
                    let normal = flip(out);
                    vertex.normal = Vec4::new(normal.x, normal.y, normal.z, 0.0);
                    vertices.push(vertex);
                }
            }
            for column in 0..GLASS_COLUMNS as u16 {
                let (low, high) = (first + 2 * column, first + 2 * column + 1);
                indices.extend([low, low + 2, high + 2, low, high + 2, high]);
            }
        }
    }
    Mesh {
        vertices,
        indices,
        texture: None,
    }
}

/// The sills lining every window's hole through the wall, from the bore
/// out to the block's outside, all round its rounded outline: without
/// them, looking in at a slant, the hole showed into the steel between
/// the cylinders. They face into the hole.
fn sill_solid(shape: BlockShape) -> Solid {
    let layout = PaneLayout::new(shape);
    let (half_width, half_height) = layout.half_size();
    let corner = layout.corner;
    let middle_y = (layout.bottom + layout.top) / 2.0;
    // Round the outline: each corner's arc in turn, with the way out of
    // the outline there.
    let mut outline = Vec::new();
    for (quarter, (across, up)) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)]
        .into_iter()
        .enumerate()
    {
        let center = vec2(across * (half_width - corner), up * (half_height - corner));
        for step in 0..=SILL_CORNER_STEPS {
            let share = step as f32 / SILL_CORNER_STEPS as f32;
            let turn = (quarter as f32 + share) * PI / 2.0;
            let out = vec2(turn.cos(), turn.sin());
            outline.push((center + out * corner, out));
        }
    }
    let mut solid = Solid::new();
    for mirror in [false, true] {
        let flip = |point: Vec3| {
            if mirror {
                vec3(-point.x, point.y, point.z)
            } else {
                point
            }
        };
        for middle in pane_middles() {
            let place = |(at, out): (Vec2, Vec2)| {
                let angle = middle + at.x / layout.bore;
                let y = middle_y + at.y;
                let (sin, cos) = angle.sin_cos();
                let inner = vec3(cos * shape.bore - shape.offset, y, sin * shape.bore);
                let (outer, _) = outer_point(shape, angle);
                let round = vec3(-sin, 0.0, cos);
                let inward = -(round * out.x + Vec3::Y * out.y).normalize();
                (flip(inner), flip(vec3(outer.x, y, outer.z)), flip(inward))
            };
            for index in 0..outline.len() {
                let (inner, outer, inward) = place(outline[index]);
                let next = outline[(index + 1) % outline.len()];
                let (next_inner, next_outer, next_inward) = place(next);
                solid.quad(
                    [inner, outer, next_outer, next_inner],
                    [inward, inward, next_inward, next_inward],
                    [SILL_STEEL; 4],
                );
            }
        }
    }
    solid
}

/// The glass in the cylinders' windows.
pub struct Windows {
    /// Where the panes stand.
    layout: PaneLayout,
    /// Every pane's glass, on the graphics card.
    glass: GpuMesh,
    /// The sills lining the windows' holes, on the graphics card.
    sills: GpuMesh,
    /// The lit-metal material for the sills, if it can be built.
    metal: Option<GpuMaterial>,
    /// The glass's material: untried, ready, or not to be had.
    material: Option<Option<GpuMaterial>>,
}

impl Windows {
    /// The windows of a block of `shape`.
    pub fn new(shape: BlockShape) -> Self {
        Self {
            layout: PaneLayout::new(shape),
            glass: GpuMesh::new(glass_mesh(shape)),
            sills: {
                let solid = sill_solid(shape);
                GpuMesh::new(Mesh {
                    vertices: solid.vertices,
                    indices: solid.indices,
                    texture: None,
                })
            },
            metal: load_metal(),
            material: None,
        }
    }

    /// Draws the sills lining the windows' holes seen from `camera`, with
    /// everything solid.
    pub fn draw_sills(&self, camera: &Camera3D) {
        let light = key_light(camera.position);
        draw_lit_placed(
            [&self.sills],
            self.metal.as_ref(),
            camera,
            light,
            Mat4::IDENTITY,
        );
    }

    /// Draws the glass seen from `camera`, over everything behind it;
    /// call it after everything inside the cylinders.
    pub fn draw(&mut self, camera: &Camera3D) {
        let material = self.material.get_or_insert_with(|| {
            load_clear_with(GLASS_FRAGMENT, &[("Pane", UniformType::Float4)])
        });
        let Some(material) = material.as_ref() else {
            return;
        };
        let (half_width, half_height) = self.layout.half_size();
        let seal = self.layout.frame / 4.0;
        let pane = Vec4::new(half_width, half_height, self.layout.corner, seal);
        material.set_uniform("Pane", pane);
        draw_lit(
            [&self.glass],
            Some(material),
            camera,
            key_light(camera.position),
        );
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/windows.rs"]
mod tests;
