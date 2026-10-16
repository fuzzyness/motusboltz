//! Reusable 3D meshes for the cylindrical frame, piston, and crank linkage.

use std::f32::consts::TAU;

use macroquad::models::{Mesh, Vertex, draw_mesh};
use macroquad::prelude::{Color, Vec2, Vec3, vec3};

use crate::crankshaft::{CRANK_RADIUS, Crankshaft};
use crate::render::{CylinderDimensions, DRAW_CALL_INDEX_CAPACITY, DRAW_CALL_VERTEX_CAPACITY};

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
            let shaded_color = Color {
                r: color.r * brightness,
                g: color.g * brightness,
                b: color.b * brightness,
                a: color.a,
            };

            mesh.vertices
                .push(Vertex::new2(position, Vec2::ZERO, shaded_color));
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

/// Static cylindrical wireframe with small, shaded, three-dimensional tubes.
///
/// Constructed once and submitted as one mesh each frame. The frame is only
/// visual; the collision boundary still uses the dimensions of the cylinder.
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

    /// Submits the existing frame geometry without regenerating it.
    pub fn draw(&self) {
        draw_mesh(&self.mesh);
    }
}

// Opaque piston head with a short boss for the crank linkage.
const PISTON_PLATE_THICKNESS: f32 = 0.10;
const PISTON_RADIAL_CLEARANCE: f32 = 2.0 * FRAME_TUBE_RADIUS;
const PISTON_ROD_LENGTH: f32 = 0.80;
const PISTON_METAL_COLOR: Color = Color::from_rgba(144, 169, 197, 255);
const PISTON_UNDERSIDE_COLOR: Color = Color::from_rgba(95, 120, 151, 255);

/// A reusable opaque piston plate with a short vertical rod.
///
/// The face at local Y = 0 is the gas-facing surface. Translating vertex
/// positions keeps the plate synchronized with the physical collision wall.
pub struct PistonVisual {
    mesh: Mesh,
    position: f32,
}

impl PistonVisual {
    /// Builds the disk once; the piston movement does not rebuild topology.
    pub fn new(chamber_radius: f32) -> Self {
        let radius = chamber_radius - PISTON_RADIAL_CLEARANCE;
        assert!(radius > 0.0);

        let mut mesh = Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture: None,
        };

        let top_center = 0_u16;
        let bottom_center = 1_u16;
        mesh.vertices.push(Vertex::new2(
            Vec3::Y * PISTON_PLATE_THICKNESS,
            Vec2::ZERO,
            PISTON_METAL_COLOR,
        ));
        mesh.vertices
            .push(Vertex::new2(Vec3::ZERO, Vec2::ZERO, PISTON_UNDERSIDE_COLOR));

        // Paired vertices form the upper and lower circular faces.
        for segment in 0..CYLINDER_RING_SEGMENTS {
            let angle = TAU * segment as f32 / CYLINDER_RING_SEGMENTS as f32;
            let (sin_angle, cos_angle) = angle.sin_cos();
            let radial = vec3(cos_angle, 0.0, sin_angle) * radius;
            mesh.vertices.push(Vertex::new2(
                radial + Vec3::Y * PISTON_PLATE_THICKNESS,
                Vec2::ZERO,
                PISTON_METAL_COLOR,
            ));
            mesh.vertices
                .push(Vertex::new2(radial, Vec2::ZERO, PISTON_UNDERSIDE_COLOR));
        }

        for segment in 0..CYLINDER_RING_SEGMENTS {
            let next = (segment + 1) % CYLINDER_RING_SEGMENTS;
            let top = (2 + 2 * segment) as u16;
            let bottom = top + 1;
            let next_top = (2 + 2 * next) as u16;
            let next_bottom = next_top + 1;

            // Top and bottom use opposite windings; the rim closes the plate.
            mesh.indices.extend([top_center, next_top, top]);
            mesh.indices.extend([bottom_center, bottom, next_bottom]);
            mesh.indices.extend([top, next_top, bottom]);
            mesh.indices.extend([next_top, next_bottom, bottom]);
        }

        // A short boss visually joins the head to the movable connecting rod.
        append_tube(&mut mesh, 2, false, PISTON_METAL_COLOR, |section| {
            let y = PISTON_PLATE_THICKNESS + if section == 0 { 0.0 } else { PISTON_ROD_LENGTH };
            (Vec3::Y * y, Vec3::X, Vec3::Z)
        });

        assert!(mesh.vertices.len() <= u16::MAX as usize);
        assert!(mesh.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
        assert!(mesh.indices.len() < DRAW_CALL_INDEX_CAPACITY);

        Self {
            mesh,
            position: 0.0,
        }
    }

    /// Translates the piston to the physical height without reallocating.
    fn set_position(&mut self, position: f32) {
        let displacement = position - self.position;
        if displacement != 0.0 {
            for vertex in &mut self.mesh.vertices {
                vertex.position.y += displacement;
            }
            self.position = position;
        }
    }

    /// Draws the piston at its current physical height.
    pub fn draw(&mut self, position: f32) {
        self.set_position(position);
        draw_mesh(&self.mesh);
    }
}

// The flywheel lies in the crank's XY plane, above the chamber.
const FLYWHEEL_RING_SEGMENTS: usize = 48;
const FLYWHEEL_COLOR: Color = Color::from_rgba(212, 179, 109, 255);
const CONNECTING_ROD_COLOR: Color = Color::from_rgba(179, 196, 218, 255);
const CRANK_ARM_COLOR: Color = Color::from_rgba(218, 184, 119, 255);
const CRANK_TUBE_SECTIONS: usize = 2;

/// Flywheel rim and animated crank/connecting rod, with reused vertex storage.
///
/// The rim is rotationally symmetric, so only its link geometry needs to
/// change as the flywheel turns.
pub struct CrankVisual {
    wheel: Mesh,
    links: Mesh,
}

impl CrankVisual {
    pub fn new() -> Self {
        let crank = Crankshaft::at_height(crate::simulation::CHAMBER_HEIGHT / 2.0);
        let center = crank.shaft_center();
        let mut wheel = Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture: None,
        };
        append_tube(
            &mut wheel,
            FLYWHEEL_RING_SEGMENTS,
            true,
            FLYWHEEL_COLOR,
            |section| {
                let angle = TAU * section as f32 / FLYWHEEL_RING_SEGMENTS as f32;
                let (sin, cos) = angle.sin_cos();
                let radial = vec3(sin, cos, 0.0);
                (center + radial * CRANK_RADIUS, radial, Vec3::Z)
            },
        );

        let mut links = Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture: None,
        };
        for color in [CRANK_ARM_COLOR, CONNECTING_ROD_COLOR] {
            append_tube(&mut links, CRANK_TUBE_SECTIONS, false, color, |section| {
                (center + Vec3::Y * section as f32, Vec3::X, Vec3::Z)
            });
        }

        Self { wheel, links }
    }

    fn position_link(vertices: &mut [Vertex], from: Vec3, to: Vec3) {
        let direction = (to - from).normalize();
        let side = direction.cross(Vec3::Z).normalize();
        for (section, center) in [from, to].into_iter().enumerate() {
            for side_index in 0..FRAME_TUBE_SIDES {
                let angle = TAU * side_index as f32 / FRAME_TUBE_SIDES as f32;
                let (sin, cos) = angle.sin_cos();
                let offset = (side * cos + Vec3::Z * sin) * FRAME_TUBE_RADIUS;
                vertices[section * FRAME_TUBE_SIDES + side_index].position = center + offset;
            }
        }
    }

    /// Updates both linkage segments and draws the static rim and moving rods.
    pub fn draw(&mut self, crank: &Crankshaft) {
        let first_link_vertices = CRANK_TUBE_SECTIONS * FRAME_TUBE_SIDES;
        let (arm, rod) = self.links.vertices.split_at_mut(first_link_vertices);
        let piston_joint = Vec3::Y * crank.piston_position();
        Self::position_link(arm, crank.shaft_center(), crank.crank_pin());
        Self::position_link(rod, crank.crank_pin(), piston_joint);
        draw_mesh(&self.wheel);
        draw_mesh(&self.links);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::{CHAMBER_HEIGHT, CHAMBER_RADIUS};
    use macroquad::prelude::ORANGE;

    const FLOAT_TOLERANCE: f32 = 1e-6;

    #[test]
    fn crank_link_mesh_has_valid_indices() {
        let crank = CrankVisual::new();
        assert_eq!(
            crank.links.vertices.len(),
            2 * CRANK_TUBE_SECTIONS * FRAME_TUBE_SIDES
        );
        assert!(
            crank
                .links
                .indices
                .iter()
                .all(|&index| (index as usize) < crank.links.vertices.len())
        );
        assert!(
            crank
                .wheel
                .indices
                .iter()
                .all(|&index| (index as usize) < crank.wheel.vertices.len())
        );
    }

    #[test]
    fn piston_geometry_translates_without_rebuilding() {
        const TEST_PISTON_HEIGHT: f32 = 1.25;
        let mut piston = PistonVisual::new(CHAMBER_RADIUS);
        let original_vertex_count = piston.mesh.vertices.len();
        let original_y = piston.mesh.vertices[0].position.y;

        piston.set_position(TEST_PISTON_HEIGHT);

        assert_eq!(piston.mesh.vertices.len(), original_vertex_count);
        assert_eq!(piston.position, TEST_PISTON_HEIGHT);
        assert!(
            (piston.mesh.vertices[0].position.y - original_y - TEST_PISTON_HEIGHT).abs()
                < FLOAT_TOLERANCE
        );
    }

    #[test]
    fn cylinder_frame_is_one_valid_mesh() {
        let frame = CylinderFrame::new(
            CylinderDimensions {
                radius: CHAMBER_RADIUS,
                height: CHAMBER_HEIGHT,
            },
            ORANGE,
        );
        let expected_vertices =
            (2 * CYLINDER_RING_SEGMENTS + 2 * CYLINDER_VERTICAL_GUIDES) * FRAME_TUBE_SIDES;
        let expected_indices =
            (2 * CYLINDER_RING_SEGMENTS + CYLINDER_VERTICAL_GUIDES) * FRAME_TUBE_SIDES * 6;

        assert_eq!(frame.mesh.vertices.len(), expected_vertices);
        assert_eq!(frame.mesh.indices.len(), expected_indices);
        assert!(frame.mesh.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
        assert!(frame.mesh.indices.len() < DRAW_CALL_INDEX_CAPACITY);
        assert!(
            frame
                .mesh
                .indices
                .iter()
                .all(|&index| (index as usize) < frame.mesh.vertices.len())
        );
    }
}
