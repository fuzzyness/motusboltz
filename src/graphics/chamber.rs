//! The cylinders and the opening between them.
//!
//! They are drawn as the solid engine block (see `block.rs`), or as tube
//! outlines where its shader cannot be built. The pistons are drawn with the rest of the moving parts (see
//! `assembly.rs`).

use std::f32::consts::TAU;

use macroquad::models::{
    Mesh,
    Vertex,
    draw_mesh, //
};
use macroquad::prelude::{
    Camera3D,
    Color,
    Vec2,
    Vec3,
    draw_line_3d,
    vec3, //
};

use super::block::EngineBlock;
use super::{
    CylinderDimensions,
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY,
    shade, //
};
use crate::sim::geometry::ChamberGeometry;

/// Segments around each rim of a cylinder's outline.
///
/// Tubular outlines give the cylinder visible depth without solid walls.
const CYLINDER_RING_SEGMENTS: usize = 96;
/// Vertical tubes between the rims.
const CYLINDER_VERTICAL_GUIDES: usize = 8;
/// Sides of each tube.
const FRAME_TUBE_SIDES: usize = 8;
/// Radius of each tube.
const FRAME_TUBE_RADIUS: f32 = 0.035;
/// Light that reaches every side of a tube.
const FRAME_AMBIENT_LIGHT: f32 = 0.65;
/// Light that falls on the side facing the light.
const FRAME_DIFFUSE_LIGHT: f32 = 0.35;
/// The light, in world space.
const FRAME_LIGHT_DIRECTION: Vec3 = Vec3::new(0.4, 0.7, 0.6);
/// Color of the hot cylinder's outline.
const HOT_FRAME_COLOR: Color = Color::from_rgba(241, 145, 92, 255);
/// Color of the cold cylinder's outline.
const COLD_FRAME_COLOR: Color = Color::from_rgba(110, 189, 255, 255);
/// Color of the opening's outline.
const PASSAGE_COLOR: Color = Color::from_rgba(180, 224, 209, 255);
/// Segments around each rim of the opening's outline.
const PASSAGE_RIM_SEGMENTS: usize = 32;
/// Rim segments between the lines along the opening.
const PASSAGE_LONGITUDINAL_STRIDE: usize = 4;

/// Appends a tube of `radius` with `sections` cross-sections in `color` to
/// `mesh`, joining the last back to the first if `closed`.
///
/// `cross_section` gives each section's center and the two basis vectors of
/// its plane. The triangles face outward for both circular rims and
/// vertical guides. The light is baked into the vertex colors.
pub(super) fn append_tube(
    mesh: &mut Mesh,
    sections: usize,
    closed: bool,
    radius: f32,
    color: Color,
    mut cross_section: impl FnMut(usize) -> (Vec3, Vec3, Vec3),
) {
    let first_vertex = mesh.vertices.len();
    let light_direction = FRAME_LIGHT_DIRECTION.normalize();

    for section in 0..sections {
        let (center, axis_u, axis_v) = cross_section(section);

        for side in 0..FRAME_TUBE_SIDES {
            let angle = TAU * side as f32 / FRAME_TUBE_SIDES as f32;
            let (sin_angle, cos_angle) = angle.sin_cos();
            let normal = axis_u * cos_angle + axis_v * sin_angle;
            let position = center + normal * radius;
            let brightness =
                FRAME_AMBIENT_LIGHT + FRAME_DIFFUSE_LIGHT * normal.dot(light_direction).max(0.0);
            mesh.vertices
                .push(Vertex::new2(position, Vec2::ZERO, shade(color, brightness)));
        }
    }

    let segments = if closed { sections } else { sections - 1 };

    for section in 0..segments {
        let next_section = (section + 1) % sections;

        for side in 0..FRAME_TUBE_SIDES {
            let next_side = (side + 1) % FRAME_TUBE_SIDES;
            let a = (first_vertex + section * FRAME_TUBE_SIDES + side) as u16;
            let b = (first_vertex + next_section * FRAME_TUBE_SIDES + side) as u16;
            let c = (first_vertex + section * FRAME_TUBE_SIDES + next_side) as u16;
            let d = (first_vertex + next_section * FRAME_TUBE_SIDES + next_side) as u16;

            mesh.indices.extend([a, c, b, b, c, d]);
        }
    }
}

/// Cylinder outline made of thin shaded tubes: top and bottom rims plus
/// vertical guides.
///
/// It is built once and drawn as one mesh each frame. Visual only;
/// collisions use `ChamberGeometry`.
pub struct CylinderFrame {
    /// The tubes.
    mesh: Mesh,
}

impl CylinderFrame {
    /// Builds two circular rims and evenly spaced vertical supports in
    /// `color` around a cylinder of `chamber` whose axis is at `center_x`.
    ///
    /// # Panics
    ///
    /// Panics if the cylinder is not positive and finite, or its mesh would
    /// not fit Macroquad's buffers.
    pub fn new(chamber: CylinderDimensions, color: Color, center_x: f32) -> Self {
        assert!(chamber.radius.is_finite() && chamber.radius > 0.0);
        assert!(chamber.height.is_finite() && chamber.height > 0.0);

        let mut mesh = Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture: None,
        };
        let half_height = chamber.height / 2.0;
        let (bottom, top) = (-half_height, half_height);
        for y in [bottom, top] {
            let tube = FRAME_TUBE_RADIUS;
            append_tube(
                &mut mesh,
                CYLINDER_RING_SEGMENTS,
                true,
                tube,
                color,
                |section| {
                    let angle = TAU * section as f32 / CYLINDER_RING_SEGMENTS as f32;
                    let (sin_angle, cos_angle) = angle.sin_cos();
                    let radial = vec3(cos_angle, 0.0, sin_angle);
                    (radial * chamber.radius + Vec3::Y * y, radial, Vec3::Y)
                },
            );
        }
        for guide in 0..CYLINDER_VERTICAL_GUIDES {
            let angle = TAU * guide as f32 / CYLINDER_VERTICAL_GUIDES as f32;
            let (sin_angle, cos_angle) = angle.sin_cos();
            let radial = vec3(cos_angle, 0.0, sin_angle);
            let lateral = vec3(sin_angle, 0.0, -cos_angle);
            append_tube(&mut mesh, 2, false, FRAME_TUBE_RADIUS, color, |section| {
                let y = [bottom, top][section];
                (radial * chamber.radius + Vec3::Y * y, radial, lateral)
            });
        }
        assert!(mesh.vertices.len() <= u16::MAX as usize);
        assert!(mesh.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
        assert!(mesh.indices.len() < DRAW_CALL_INDEX_CAPACITY);
        for vertex in &mut mesh.vertices {
            vertex.position.x += center_x;
        }
        Self { mesh }
    }

    /// Submits the existing frame geometry without regenerating it.
    pub fn draw(&self) {
        draw_mesh(&self.mesh);
    }
}

/// Whether the solid block is ready.
///
/// It is built on the first draw, since its shader needs the window; if
/// the shader cannot be built, the outline frames are drawn instead.
enum Block {
    /// Not built yet.
    Untried,
    /// Built.
    Ready(EngineBlock),
    /// Its shader could not be built.
    Unavailable,
}

/// The engine block, or, failing that, both cylinder frames, orange hot and
/// blue cold, and the outline of the opening.
pub struct ConnectedChamberVisual {
    /// The engine's shape.
    geometry: ChamberGeometry,
    /// The solid block, once tried.
    block: Block,
    /// The hot cylinder's outline.
    hot: CylinderFrame,
    /// The cold cylinder's outline.
    cold: CylinderFrame,
    /// The opening's outline.
    passage_lines: Vec<(Vec3, Vec3)>,
}

impl ConnectedChamberVisual {
    /// The block, or its outlines, for the engine of `geometry`.
    pub fn new(geometry: ChamberGeometry) -> Self {
        let dimensions = CylinderDimensions {
            radius: geometry.cylinder_radius,
            height: geometry.cylinder_height,
        };
        Self {
            geometry,
            block: Block::Untried,
            hot: CylinderFrame::new(dimensions, HOT_FRAME_COLOR, geometry.hot_center_x()),
            cold: CylinderFrame::new(dimensions, COLD_FRAME_COLOR, geometry.cold_center_x()),
            passage_lines: passage_wireframe(geometry),
        }
    }

    /// Draws the block (or the frames and opening) with its window toward
    /// `camera`.
    pub fn draw(&mut self, camera: &Camera3D) {
        if let Block::Untried = self.block {
            self.block = match EngineBlock::new(self.geometry) {
                Some(block) => Block::Ready(block),
                None => Block::Unavailable,
            };
        }
        if let Block::Ready(block) = &self.block {
            block.draw(camera);
        } else {
            self.hot.draw();
            self.cold.draw();
            for &(a, b) in &self.passage_lines {
                draw_line_3d(a, b, PASSAGE_COLOR);
            }
        }
    }
}

/// Line segments outlining the opening, computed once.
///
/// They are the two rims where the round tube meets each bore wall, and
/// lines along the tube between them. The parts of the tube inside the
/// bores are open gas, so they are not drawn.
fn passage_wireframe(geometry: ChamberGeometry) -> Vec<(Vec3, Vec3)> {
    let radius = geometry.passage_radius;
    let center_y = geometry.passage_center_y();
    // A point of the rim on the hot (side -1) or cold (side 1) bore wall.
    let rim = |angle: f32, side: f32| {
        let (sin, cos) = angle.sin_cos();
        let z = radius * sin;
        let wall = (geometry.cylinder_radius.powi(2) - z * z).sqrt();
        vec3(
            side * (geometry.cylinder_offset - wall),
            center_y - radius * cos,
            z,
        )
    };
    let mut lines = Vec::new();
    for segment in 0..PASSAGE_RIM_SEGMENTS {
        let angle = TAU * segment as f32 / PASSAGE_RIM_SEGMENTS as f32;
        let next = TAU * (segment + 1) as f32 / PASSAGE_RIM_SEGMENTS as f32;
        for side in [-1.0, 1.0] {
            lines.push((rim(angle, side), rim(next, side)));
        }
        if segment % PASSAGE_LONGITUDINAL_STRIDE == 0 {
            lines.push((rim(angle, -1.0), rim(angle, 1.0)));
        }
    }
    lines
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/chamber.rs"]
mod tests;
