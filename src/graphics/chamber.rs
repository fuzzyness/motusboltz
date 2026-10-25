//! The cylinders, pistons and U passage: tube outlines and solid piston
//! discs, built once and moved each frame rather than rebuilt.

use std::f32::consts::{
    FRAC_PI_2,
    TAU, //
};

use macroquad::models::{
    Mesh,
    Vertex,
    draw_mesh, //
};
use macroquad::prelude::{
    Color,
    Vec2,
    Vec3,
    draw_line_3d,
    vec3, //
};

use super::{
    CylinderDimensions,
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY,
    shade, //
};
use crate::sim::geometry::ChamberGeometry;

// Tubular outlines give the cylinder visible depth without solid walls.
const CYLINDER_RING_SEGMENTS: usize = 96;
const CYLINDER_VERTICAL_GUIDES: usize = 8;
const FRAME_TUBE_SIDES: usize = 8;
const FRAME_TUBE_RADIUS: f32 = 0.035;
const FRAME_AMBIENT_LIGHT: f32 = 0.65;
const FRAME_DIFFUSE_LIGHT: f32 = 0.35;
const FRAME_LIGHT_DIRECTION: Vec3 = Vec3::new(0.4, 0.7, 0.6);

/// Appends a tubular outline to a mesh, using a supplied centerline and basis.
///
/// The basis vectors describe a perpendicular cross-section at each point.
/// The triangles face outward for both circular rims and vertical guides.
fn append_tube(
    mesh: &mut Mesh,
    sections: usize,
    closed: bool,
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
            let position = center + normal * FRAME_TUBE_RADIUS;
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
/// vertical guides. Built once and drawn as one mesh each frame. Visual
/// only; collisions use `ChamberGeometry`.
pub struct CylinderFrame {
    mesh: Mesh,
}

impl CylinderFrame {
    /// Builds two circular rims and evenly spaced vertical supports.
    pub fn new(chamber: CylinderDimensions, color: Color) -> Self {
        assert!(chamber.radius.is_finite() && chamber.radius > 0.0);
        assert!(chamber.height.is_finite() && chamber.height > 0.0);

        let mut mesh = Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture: None,
        };
        let half_height = chamber.height / 2.0;

        for y in [-half_height, half_height] {
            append_tube(&mut mesh, CYLINDER_RING_SEGMENTS, true, color, |section| {
                let angle = TAU * section as f32 / CYLINDER_RING_SEGMENTS as f32;
                let (sin_angle, cos_angle) = angle.sin_cos();
                let radial = vec3(cos_angle, 0.0, sin_angle);

                (radial * chamber.radius + Vec3::Y * y, radial, Vec3::Y)
            });
        }

        for guide in 0..CYLINDER_VERTICAL_GUIDES {
            let angle = TAU * guide as f32 / CYLINDER_VERTICAL_GUIDES as f32;
            let (sin_angle, cos_angle) = angle.sin_cos();
            let radial = vec3(cos_angle, 0.0, sin_angle);
            let lateral = vec3(sin_angle, 0.0, -cos_angle);

            append_tube(&mut mesh, 2, false, color, |section| {
                let y = if section == 0 {
                    -half_height
                } else {
                    half_height
                };
                (radial * chamber.radius + Vec3::Y * y, radial, lateral)
            });
        }

        assert!(mesh.vertices.len() <= u16::MAX as usize);
        assert!(mesh.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
        assert!(mesh.indices.len() < DRAW_CALL_INDEX_CAPACITY);

        Self { mesh }
    }

    /// Places an otherwise identical frame around a horizontally offset chamber.
    pub fn at_x(chamber: CylinderDimensions, color: Color, center_x: f32) -> Self {
        let mut frame = Self::new(chamber, color);
        for vertex in &mut frame.mesh.vertices {
            vertex.position.x += center_x;
        }
        frame
    }

    /// Submits the existing frame geometry without regenerating it.
    pub fn draw(&self) {
        draw_mesh(&self.mesh);
    }
}

const HOT_FRAME_COLOR: Color = Color::from_rgba(241, 145, 92, 255);
const COLD_FRAME_COLOR: Color = Color::from_rgba(110, 189, 255, 255);
const CONNECTOR_COLOR: Color = Color::from_rgba(180, 224, 209, 255);
const PISTON_COLOR: Color = Color::from_rgba(184, 193, 203, 255);
const PISTON_BOTTOM_COLOR: Color = Color::from_rgba(108, 132, 158, 255);
const PISTON_THICKNESS: f32 = 0.10;
const PISTON_EDGE_CLEARANCE: f32 = 0.04;

/// Piston disc built once; each frame shifts its vertices to the face
/// height instead of rebuilding the mesh.
struct PistonDisc {
    mesh: Mesh,
    y: f32,
}

impl PistonDisc {
    fn new(center_x: f32, chamber_radius: f32) -> Self {
        let mut mesh = Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture: None,
        };
        let radius = chamber_radius - PISTON_EDGE_CLEARANCE;
        mesh.vertices.push(Vertex::new2(
            vec3(center_x, PISTON_THICKNESS, 0.0),
            Vec2::ZERO,
            PISTON_COLOR,
        ));
        mesh.vertices.push(Vertex::new2(
            vec3(center_x, 0.0, 0.0),
            Vec2::ZERO,
            PISTON_BOTTOM_COLOR,
        ));
        for index in 0..CYLINDER_RING_SEGMENTS {
            let angle = TAU * index as f32 / CYLINDER_RING_SEGMENTS as f32;
            let (sin, cos) = angle.sin_cos();
            for (y, color) in [(PISTON_THICKNESS, PISTON_COLOR), (0.0, PISTON_BOTTOM_COLOR)] {
                mesh.vertices.push(Vertex::new2(
                    vec3(center_x + radius * cos, y, radius * sin),
                    Vec2::ZERO,
                    color,
                ));
            }
        }
        for index in 0..CYLINDER_RING_SEGMENTS {
            let next = (index + 1) % CYLINDER_RING_SEGMENTS;
            let upper = (2 + 2 * index) as u16;
            let lower = upper + 1;
            let next_upper = (2 + 2 * next) as u16;
            let next_lower = next_upper + 1;
            mesh.indices.extend([0, next_upper, upper]);
            mesh.indices.extend([1, lower, next_lower]);
            mesh.indices.extend([upper, next_upper, lower]);
            mesh.indices.extend([next_upper, next_lower, lower]);
        }
        Self { mesh, y: 0.0 }
    }

    fn draw(&mut self, face_y: f32) {
        let delta = face_y - self.y;
        for vertex in &mut self.mesh.vertices {
            vertex.position.y += delta;
        }
        self.y = face_y;
        draw_mesh(&self.mesh);
    }
}

/// Both cylinder frames (orange hot, blue cold), the pistons and the
/// passage outline.
pub struct ConnectedChamberVisual {
    hot: CylinderFrame,
    cold: CylinderFrame,
    hot_piston: PistonDisc,
    cold_piston: PistonDisc,
    passage_lines: Vec<(Vec3, Vec3)>,
}

impl ConnectedChamberVisual {
    pub fn new(geometry: ChamberGeometry) -> Self {
        let dimensions = CylinderDimensions {
            radius: geometry.cylinder_radius,
            height: geometry.cylinder_height,
        };
        Self {
            hot: CylinderFrame::at_x(dimensions, HOT_FRAME_COLOR, geometry.hot_center_x()),
            cold: CylinderFrame::at_x(dimensions, COLD_FRAME_COLOR, geometry.cold_center_x()),
            hot_piston: PistonDisc::new(geometry.hot_center_x(), geometry.cylinder_radius),
            cold_piston: PistonDisc::new(geometry.cold_center_x(), geometry.cylinder_radius),
            passage_lines: passage_wireframe(geometry),
        }
    }

    /// Draws the frames and passage, with the pistons at the given heights.
    pub fn draw(&mut self, hot_face_y: f32, cold_face_y: f32) {
        self.hot.draw();
        self.cold.draw();
        self.hot_piston.draw(hot_face_y);
        self.cold_piston.draw(cold_face_y);

        for &(a, b) in &self.passage_lines {
            draw_line_3d(a, b, CONNECTOR_COLOR);
        }
    }
}

/// Line segments outlining the passage capsules, computed once. Edges inside
/// another capsule or above the cylinder floors are left out, so elbows and
/// floor openings do not look like closed internal walls.
fn passage_wireframe(geometry: ChamberGeometry) -> Vec<(Vec3, Vec3)> {
    const RING_SEGMENTS: usize = 24;
    const CAP_STEPS: usize = 6;
    const LONGITUDINAL_STRIDE: usize = 3;
    const SURFACE_TOLERANCE: f32 = 0.999;
    let sections = geometry.connector_segments();
    let radius = geometry.connector_radius;
    let mut lines = Vec::new();
    for (section, &(start, end)) in sections.iter().enumerate() {
        let direction = (end - start).normalize();
        let across = direction.cross(Vec3::Z);
        let mut add = |a: Vec3, b: Vec3| {
            let middle = (a + b) / 2.0;
            if middle.y > -geometry.cylinder_height / 2.0 {
                return;
            }
            let hidden = sections.iter().enumerate().any(|(other, &(c, d))| {
                if other == section {
                    return false;
                }
                let axis = d - c;
                let nearest =
                    c + axis * ((middle - c).dot(axis) / axis.length_squared()).clamp(0.0, 1.0);
                middle.distance_squared(nearest) < radius * radius * SURFACE_TOLERANCE
            });
            if !hidden {
                lines.push((a, b));
            }
        };
        for segment in 0..RING_SEGMENTS {
            let angle = TAU * segment as f32 / RING_SEGMENTS as f32;
            let next = TAU * (segment + 1) as f32 / RING_SEGMENTS as f32;
            let radial = |angle: f32| across * angle.cos() + Vec3::Z * angle.sin();
            if segment % LONGITUDINAL_STRIDE == 0 {
                add(start + radial(angle) * radius, end + radial(angle) * radius);
            }
            for (center, sign) in [(start, -1.0), (end, 1.0)] {
                for step in 0..CAP_STEPS {
                    let latitude = FRAC_PI_2 * step as f32 / CAP_STEPS as f32;
                    let next_latitude = FRAC_PI_2 * (step + 1) as f32 / CAP_STEPS as f32;
                    let point = |angle: f32, latitude: f32| {
                        center
                            + radius
                                * (radial(angle) * latitude.cos()
                                    + direction * sign * latitude.sin())
                    };
                    add(point(angle, latitude), point(next, latitude));
                    if segment % LONGITUDINAL_STRIDE == 0 {
                        add(point(angle, latitude), point(angle, next_latitude));
                    }
                }
            }
        }
    }
    lines
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/chamber.rs"]
mod tests;
