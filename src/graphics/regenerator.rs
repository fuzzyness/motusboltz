//! The regenerator's wire screens across the opening, each tinted by its
//! own temperature in the block's own paint: the hot side's copper toward
//! the hot wall's temperature, violet between, and the cold side's cobalt
//! toward the cold wall's.
//!
//! Sixteen screens side by side through the thin wall would read as one
//! solid plug, so every four are drawn as one, at their middle and their
//! mean temperature. Each is a ring of wire around the opening's edge with
//! a grid of straight wires across it, built once as thin shaded tubes;
//! each frame only their colors change. From most views the gas in the cold cylinder
//! stands in front of the opening, so after the gas is drawn the screens
//! are drawn again faintly over it, farthest first, like a ghost seen
//! through it. Drawing only; the physics uses `Regenerator`.

use std::f32::consts::TAU;

use macroquad::miniquad::{
    BlendFactor,
    BlendState,
    BlendValue,
    Equation, //
};
use macroquad::models::{
    Mesh,
    draw_mesh, //
};
use macroquad::prelude::{
    Camera3D,
    Color,
    Comparison,
    Material,
    MaterialParams,
    PipelineParams,
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
    COLD_STEEL,
    HOT_STEEL, //
};
use super::chamber::append_tube;
use super::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY,
    mix_color,
    shade, //
};
use crate::sim::geometry::ChamberGeometry;
use crate::sim::regenerator::{
    Regenerator,
    SCREEN_COUNT,
    Screen, //
};

/// Radius of each wire.
const WIRE_RADIUS: f32 = 0.028;
/// Sections around each screen's ring.
const RING_SECTIONS: usize = 48;
/// Straight wires each way across a screen.
const WIRES_ACROSS: usize = 7;
/// How far the ring stands in from the opening's wall, so it does not sink
/// into it.
const RING_INSET: f32 = 0.03;
/// Screens drawn; each stands for as many of the stack's as divide evenly.
const DRAWN_SCREENS: usize = 4;
/// Screens of the stack each drawn screen stands for.
const SCREENS_PER_DRAWN: usize = SCREEN_COUNT / DRAWN_SCREENS;
/// Wire at the cold wall's temperature: the cold side's cobalt.
const COLD_WIRE: Color = COLD_STEEL;
/// Wire halfway between the walls' temperatures: a violet as strong as
/// both, where a straight mix of them would be a dull gray.
const MIDDLE_WIRE: Color = Color::from_rgba(168, 86, 200, 255);
/// Wire at the hot wall's temperature: the hot side's copper.
const HOT_WIRE: Color = HOT_STEEL;
/// Least light on a wire, so the thin wires stay bright on their dark
/// side.
const MIN_WIRE_LIGHT: f32 = 0.8;
/// Smallest gap between the walls' temperatures used for the tint, so
/// equal walls do not divide by zero.
const MIN_TINT_SPAN_KELVIN: f32 = 1.0;
/// Opacity of each screen drawn over the gas; where they line up, the
/// nearest show most.
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

/// Whether the ghost's material is ready.
///
/// It is built on the first draw, since its shader needs the window; if it
/// cannot be built, the screens are drawn only once, behind the gas.
enum Ghost {
    /// Not built yet.
    Untried,
    /// Built.
    Ready(Material),
    /// Its shader could not be built.
    Unavailable,
}

/// The screens' mesh, and what is needed to recolor it.
pub struct RegeneratorScreens {
    /// Every screen's wires.
    mesh: Mesh,
    /// The first vertex of each screen, then the end of the last.
    starts: Vec<usize>,
    /// The light on each vertex, from 0 to 1.
    light: Vec<f32>,
    /// The mesh's triangles screen by screen from the cold side to the
    /// hot side, the reverse of the mesh's own order, for the ghost seen
    /// from the hot side.
    reversed: Vec<u16>,
    /// The material that draws the screens over the gas, once tried.
    ghost: Ghost,
}

impl RegeneratorScreens {
    /// Builds a ring and a grid of wires for each drawn screen standing for
    /// `screens` in the opening of `geometry`.
    ///
    /// # Panics
    ///
    /// Panics if the mesh would not fit Macroquad's buffers.
    pub fn new(geometry: ChamberGeometry, screens: impl Iterator<Item = Screen>) -> Self {
        let mut mesh = Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture: None,
        };
        let radius = geometry.passage_radius - RING_INSET;
        let center_y = geometry.passage_center_y();
        let mut starts = Vec::new();
        let mut index_starts = Vec::new();
        for screen in drawn(screens) {
            starts.push(mesh.vertices.len());
            index_starts.push(mesh.indices.len());
            let center = vec3(screen.x, center_y, 0.0);
            append_tube(
                &mut mesh,
                RING_SECTIONS,
                true,
                WIRE_RADIUS,
                Color::new(1.0, 1.0, 1.0, 1.0),
                |section| {
                    let angle = TAU * section as f32 / RING_SECTIONS as f32;
                    let (sin, cos) = angle.sin_cos();
                    let radial = vec3(0.0, cos, sin);
                    (center + radial * radius, radial, Vec3::X)
                },
            );
            // Wires across, each way, between the ring's sides.
            let spacing = 2.0 * radius / (WIRES_ACROSS + 1) as f32;
            for wire in 1..=WIRES_ACROSS {
                let offset = -radius + wire as f32 * spacing;
                let half = (radius * radius - offset * offset).sqrt();
                for (along, across) in [(Vec3::Z, Vec3::Y), (Vec3::Y, Vec3::Z)] {
                    append_tube(
                        &mut mesh,
                        2,
                        false,
                        WIRE_RADIUS,
                        Color::new(1.0, 1.0, 1.0, 1.0),
                        |end| {
                            let sign = if end == 0 { -1.0 } else { 1.0 };
                            let point = center + across * offset + along * half * sign;
                            (point, across, Vec3::X)
                        },
                    );
                }
            }
        }
        starts.push(mesh.vertices.len());
        index_starts.push(mesh.indices.len());
        let reversed = index_starts
            .windows(2)
            .rev()
            .flat_map(|run| mesh.indices[run[0]..run[1]].iter().copied())
            .collect();
        assert!(mesh.vertices.len() <= u16::MAX as usize);
        assert!(mesh.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
        assert!(mesh.indices.len() < DRAW_CALL_INDEX_CAPACITY);
        let light = mesh
            .vertices
            .iter()
            .map(|vertex| {
                let baked = vertex.color[0] as f32 / u8::MAX as f32;
                MIN_WIRE_LIGHT + (1.0 - MIN_WIRE_LIGHT) * baked
            })
            .collect();
        Self {
            mesh,
            starts,
            light,
            reversed,
            ghost: Ghost::Untried,
        }
    }

    /// Draws `regenerator`'s screens, tinted between walls at `hot_kelvin`
    /// and `cold_kelvin`, if they are in the opening.
    pub fn draw(&mut self, regenerator: &Regenerator, hot_kelvin: f32, cold_kelvin: f32) {
        if !regenerator.is_enabled() {
            return;
        }
        for (index, screen) in drawn(regenerator.screens()).into_iter().enumerate() {
            let color = tint(screen.kelvin, hot_kelvin, cold_kelvin);
            let (start, end) = (self.starts[index], self.starts[index + 1]);
            for (vertex, light) in self.mesh.vertices[start..end]
                .iter_mut()
                .zip(&self.light[start..end])
            {
                vertex.color = shade(color, *light).into();
            }
        }
        draw_mesh(&self.mesh);
    }

    /// Draws `regenerator`'s screens again, faintly, over everything drawn
    /// so far, in the colors of the last `draw`, farthest from `camera`
    /// first, if they are in the opening.
    pub fn draw_ghost(&mut self, regenerator: &Regenerator, camera: &Camera3D) {
        if !regenerator.is_enabled() {
            return;
        }
        if let Ghost::Untried = self.ghost {
            self.ghost = load_ghost().map_or(Ghost::Unavailable, Ghost::Ready);
        }
        let Ghost::Ready(material) = &self.ghost else {
            return;
        };
        // The mesh runs from the hot side (negative x) to the cold side.
        let from_hot_side = camera.position.x < 0.0;
        if from_hot_side {
            std::mem::swap(&mut self.mesh.indices, &mut self.reversed);
        }
        material.set_uniform("Opacity", GHOST_OPACITY);
        gl_use_material(material);
        draw_mesh(&self.mesh);
        gl_use_default_material();
        if from_hot_side {
            std::mem::swap(&mut self.mesh.indices, &mut self.reversed);
        }
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

/// `t` from 0 to 1 eased in and out, flat at both ends.
fn eased(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// The screens drawn for `screens`: each run of `SCREENS_PER_DRAWN`, at its
/// mean position and temperature.
fn drawn(screens: impl Iterator<Item = Screen>) -> Vec<Screen> {
    let screens: Vec<Screen> = screens.collect();
    screens
        .chunks(SCREENS_PER_DRAWN)
        .map(|run| {
            let count = run.len() as f32;
            Screen {
                x: run.iter().map(|screen| screen.x).sum::<f32>() / count,
                kelvin: run.iter().map(|screen| screen.kelvin).sum::<f32>() / count,
            }
        })
        .collect()
}

/// The wire color for a screen at `kelvin` between walls at `hot_kelvin`
/// and `cold_kelvin`: cobalt at the cold wall's temperature or below,
/// violet halfway, and copper at the hot wall's or above.
pub fn tint(kelvin: f32, hot_kelvin: f32, cold_kelvin: f32) -> Color {
    let span = (hot_kelvin - cold_kelvin).max(MIN_TINT_SPAN_KELVIN);
    let share = ((kelvin - cold_kelvin) / span).clamp(0.0, 1.0);
    // Eased twice, so screens a little off the middle already take most of
    // their side's color and the slope from copper to cobalt shows.
    let share = eased(eased(share));
    if share < 0.5 {
        mix_color(COLD_WIRE, MIDDLE_WIRE, 2.0 * share)
    } else {
        mix_color(MIDDLE_WIRE, HOT_WIRE, 2.0 * share - 1.0)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/regenerator.rs"]
mod tests;
