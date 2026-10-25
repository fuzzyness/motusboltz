//! The crankshaft, the flywheel with its spokes, and each cylinder's crank
//! and connecting rod, drawn as lines at the current crank angle.

use std::f32::consts::{
    FRAC_PI_2,
    TAU, //
};

use macroquad::prelude::{
    Color,
    Vec3,
    draw_line_3d,
    vec3, //
};

use crate::sim::geometry::ChamberGeometry;
use crate::sim::pistons::PowerPistons;

const WHEEL_SEGMENTS: usize = 48;
/// Radius of the flywheel rim.
pub const WHEEL_RADIUS: f32 = 2.10;
/// How far the shaft extends past each cylinder's axis.
const SHAFT_OVERHANG: f32 = 0.5;
const SPOKES: usize = 4;
const SHAFT_COLOR: Color = Color::from_rgba(198, 202, 219, 255);
const WHEEL_COLOR: Color = Color::from_rgba(223, 187, 112, 255);
const HOT_ROD_COLOR: Color = Color::from_rgba(251, 167, 114, 255);
const COLD_ROD_COLOR: Color = Color::from_rgba(135, 198, 246, 255);

/// Crankshaft, flywheel rim with spokes, cranks and connecting rods.
pub struct ShaftVisual {
    geometry: ChamberGeometry,
}

impl ShaftVisual {
    pub fn new(geometry: ChamberGeometry) -> Self {
        Self { geometry }
    }

    /// Draws the shaft along x, the flywheel at its center, and each
    /// cylinder's crank and connecting rod at the current angle.
    pub fn draw(&self, pistons: &PowerPistons) {
        let geometry = self.geometry;
        let shaft_y = pistons.shaft_height();
        let radius = pistons.stroke() / 2.0;
        let offset = geometry.cylinder_offset;
        draw_line_3d(
            vec3(-offset - SHAFT_OVERHANG, shaft_y, 0.0),
            vec3(offset + SHAFT_OVERHANG, shaft_y, 0.0),
            SHAFT_COLOR,
        );

        let wheel_center = vec3(0.0, shaft_y, 0.0);
        for segment in 0..WHEEL_SEGMENTS {
            let angle = TAU * segment as f32 / WHEEL_SEGMENTS as f32;
            let next = TAU * (segment + 1) as f32 / WHEEL_SEGMENTS as f32;
            let point = |theta: f32| -> Vec3 {
                wheel_center + vec3(0.0, WHEEL_RADIUS * theta.cos(), WHEEL_RADIUS * theta.sin())
            };
            draw_line_3d(point(angle), point(next), WHEEL_COLOR);
        }
        for spoke in 0..SPOKES {
            let angle = pistons.angle() + TAU * spoke as f32 / SPOKES as f32;
            draw_line_3d(
                wheel_center,
                wheel_center + vec3(0.0, WHEEL_RADIUS * angle.cos(), WHEEL_RADIUS * angle.sin()),
                WHEEL_COLOR,
            );
        }

        for (x, face_y, phase, color) in [
            (-offset, pistons.hot().face_y, 0.0, HOT_ROD_COLOR),
            (offset, pistons.cold().face_y, FRAC_PI_2, COLD_ROD_COLOR),
        ] {
            let crank_angle = pistons.angle() + phase;
            let pin = vec3(
                x,
                shaft_y + radius * crank_angle.cos(),
                radius * crank_angle.sin(),
            );
            let journal = vec3(x, shaft_y, 0.0);
            let piston_joint = vec3(x, face_y, 0.0);
            draw_line_3d(journal, pin, WHEEL_COLOR);
            draw_line_3d(pin, piston_joint, color);
        }
    }
}
