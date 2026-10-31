//! A faint grid on the floor under the engine, built once.
//!
//! The grid keeps the engine from floating in a void and makes turning the
//! camera easier to follow.

use macroquad::models::{
    Mesh,
    Vertex,
    draw_mesh, //
};
use macroquad::prelude::{
    Color,
    Vec2,
    vec3, //
};

use super::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY, //
};

/// Half the grid's width, in model units.
const GRID_HALF_WIDTH: f32 = 24.0;
/// Distance between lines.
const GRID_SPACING: f32 = 1.0;
/// Width of a line.
const LINE_WIDTH: f32 = 0.035;
/// Length of the pieces each line is built from; the fade is computed at
/// their ends, so the grid fits one draw call.
const PIECE_LENGTH: f32 = 2.0;
/// Just below the block's underside, so the two never fight for depth.
const BELOW_BLOCK: f32 = 0.005;
/// Color of the lines, a quiet blue-gray.
const GRID_COLOR: Color = Color::from_rgba(96, 120, 150, 255);
/// Opacity of the lines in the middle, fading to nothing at the edge.
const GRID_OPACITY: f32 = 0.45;

/// The floor grid.
pub struct FloorGrid {
    /// The lines, as thin quads.
    mesh: Mesh,
}

impl FloorGrid {
    /// A grid centered under the engine, at height `floor`.
    pub fn new(floor: f32) -> Self {
        let y = floor - BELOW_BLOCK;
        let lines = (2.0 * GRID_HALF_WIDTH / GRID_SPACING).round() as usize;
        let pieces = (2.0 * GRID_HALF_WIDTH / PIECE_LENGTH).round() as usize;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let opacity = |x: f32, z: f32| {
            let reach = (x * x + z * z).sqrt() / GRID_HALF_WIDTH;
            let fade = (1.0 - reach * reach).max(0.0);
            Color {
                a: GRID_OPACITY * fade * fade,
                ..GRID_COLOR
            }
        };
        for line in 0..=lines {
            let across = -GRID_HALF_WIDTH + line as f32 * GRID_SPACING;
            // One line along x and one along z, in short pieces so the fade
            // follows the distance from the middle.
            for along_x in [true, false] {
                for piece in 0..pieces {
                    let start = -GRID_HALF_WIDTH + piece as f32 * PIECE_LENGTH;
                    let end = start + PIECE_LENGTH;
                    let point = |along: f32, offset: f32| {
                        if along_x {
                            vec3(along, y, across + offset)
                        } else {
                            vec3(across + offset, y, along)
                        }
                    };
                    let half = LINE_WIDTH / 2.0;
                    let first = vertices.len() as u16;
                    let corners = [(start, -half), (start, half), (end, half), (end, -half)];
                    for (along, offset) in corners {
                        let position = point(along, offset);
                        let color = opacity(position.x, position.z);
                        vertices.push(Vertex::new2(position, Vec2::ZERO, color));
                    }
                    indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
                }
            }
        }
        assert!(vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
        assert!(indices.len() < DRAW_CALL_INDEX_CAPACITY);
        Self {
            mesh: Mesh {
                vertices,
                indices,
                texture: None,
            },
        }
    }

    /// Draws the grid.
    pub fn draw(&self) {
        draw_mesh(&self.mesh);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/floor.rs"]
mod tests;
