//! The 3D view drawn a little below the window's resolution and scaled up
//! smoothly.
//!
//! The scaling gives the slightly soft look of a Dreamcast game rather
//! than the crisp edges of a modern one. The panel, buttons and text are
//! drawn afterwards at full resolution, so they stay sharp. It also means
//! fewer pixels to fill on slow laptops and phones.
//!
//! A finished view can also be laid over the next one through an ordered
//! dither, which dissolves one scenery into the other in the chunky,
//! patterned way of an old console.

use macroquad::prelude::{
    Camera2D,
    Comparison,
    DrawTextureParams,
    FilterMode,
    Material,
    MaterialParams,
    PipelineParams,
    Rect,
    RenderTarget,
    RenderTargetParams,
    ShaderSource,
    UniformDesc,
    UniformType,
    WHITE,
    draw_texture_ex,
    gl_use_default_material,
    gl_use_material,
    load_material,
    render_target_ex,
    vec2, //
};

/// The view's height as a share of the window's height in screen points.
///
/// The share is of points, not physical pixels, so a high-DPI screen gets
/// the same softness.
const SCALE: f32 = 0.85;
/// Fewest lines drawn, so a small window or phone is not too blurry,
/// unless the screen itself has fewer.
const MIN_LINES: f32 = 600.0;

/// Vertex shader for the copy to the screen.
const COPY_VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;

varying lowp vec2 uv;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    uv = texcoord;
}"#;

/// Fragment shader that copies the view as it is, fully opaque.
///
/// Blending inside the view leaves its alpha below 1 in places, which must
/// not show the screen through.
const COPY_FRAGMENT: &str = r#"#version 100
precision mediump float;

varying lowp vec2 uv;

uniform sampler2D Texture;

void main() {
    gl_FragColor = vec4(texture2D(Texture, uv).rgb, 1.0);
}"#;

/// Side of a dither cell, in screen points.
const DITHER_CELL: f32 = 2.0;

/// Fragment shader that keeps a share of the view, `Shown`, through a 4x4
/// ordered dither of square cells `Cell` pixels wide.
///
/// Each cell's threshold comes from the Bayer matrix, built from its 2x2
/// pattern so it needs no table, and from small numbers only, so it holds
/// at the low precision of phone GPUs; a cell shows while its threshold is below
/// `Shown`, so the view breaks up evenly as `Shown` falls.
const DITHER_FRAGMENT: &str = r#"#version 100
precision mediump float;

varying lowp vec2 uv;

uniform sampler2D Texture;
uniform float Shown;
uniform float Cell;

float bayer2(vec2 cell) {
    vec2 corner = mod(floor(cell), 2.0);
    return fract(corner.x * 0.5 + corner.y * 0.75);
}

void main() {
    vec2 cell = floor(gl_FragCoord.xy / Cell);
    float threshold = bayer2(cell * 0.5) * 0.25 + bayer2(cell);
    if (threshold >= Shown) {
        discard;
    }
    gl_FragColor = vec4(texture2D(Texture, uv).rgb, 1.0);
}"#;

/// The view's size in pixels for a `width` by `height` window in screen
/// points, at `dpi` pixels per point.
pub fn pixels(width: f32, height: f32, dpi: f32) -> (u32, u32) {
    let physical = height * dpi;
    let lines = (height * SCALE).max(MIN_LINES.min(physical)).min(physical);
    let factor = lines / height.max(1.0);
    (
        (width * factor).round().max(1.0) as u32,
        lines.round().max(1.0) as u32,
    )
}

/// The softened view.
pub struct SoftView {
    /// The copy shader, made on first use (it needs a window); `Some(None)`
    /// where it cannot be built.
    copy: Option<Option<Material>>,
    /// The image the view is drawn into, and its size in pixels.
    target: Option<(RenderTarget, (u32, u32))>,
    /// The dither shader, made on first use; `Some(None)` where it cannot
    /// be built.
    dither: Option<Option<Material>>,
}

impl SoftView {
    /// A view with nothing made yet.
    pub fn new() -> Self {
        Self {
            copy: None,
            target: None,
            dither: None,
        }
    }

    /// The image to draw the view into for a `width` by `height` window
    /// at `dpi`, made or remade as needed.
    ///
    /// It is `None` where its shader cannot be built, and the view is drawn
    /// straight to the screen.
    pub fn target(&mut self, width: f32, height: f32, dpi: f32) -> Option<RenderTarget> {
        let copy = self.copy.get_or_insert_with(|| {
            load_material(
                ShaderSource::Glsl {
                    vertex: COPY_VERTEX,
                    fragment: COPY_FRAGMENT,
                },
                MaterialParams {
                    pipeline_params: PipelineParams {
                        depth_test: Comparison::Always,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .ok()
        });
        copy.as_ref()?;
        let size = pixels(width, height, dpi);
        if self.target.as_ref().map(|(_, made)| *made) != Some(size) {
            let target = render_target_ex(
                size.0,
                size.1,
                RenderTargetParams {
                    sample_count: 1,
                    depth: true,
                },
            );
            target.texture.set_filter(FilterMode::Linear);
            self.target = Some((target, size));
        }
        self.target.as_ref().map(|(target, _)| target.clone())
    }

    /// A 2D camera over the view, in screen points, for the background.
    pub fn flat_camera(target: &RenderTarget, width: f32, height: f32) -> Camera2D {
        let mut camera = Camera2D::from_display_rect(Rect::new(0.0, 0.0, width, height));
        camera.render_target = Some(target.clone());
        camera
    }

    /// Scales the view up over the whole `width` by `height` window.
    ///
    /// The screen camera must be set.
    pub fn present(&self, width: f32, height: f32) {
        let (Some(Some(copy)), Some((target, _))) = (&self.copy, &self.target) else {
            return;
        };
        Self::cover(copy, target, width, height);
    }

    /// Lays the view, as last drawn, over a `width` by `height` window at
    /// `dpi` through the dither, keeping a `shown` share of it, from 0 to 1.
    ///
    /// Nothing is drawn where the dither shader cannot be built. The screen
    /// camera must be set.
    pub fn present_dithered(&mut self, width: f32, height: f32, dpi: f32, shown: f32) {
        let dither = self.dither.get_or_insert_with(|| {
            load_material(
                ShaderSource::Glsl {
                    vertex: COPY_VERTEX,
                    fragment: DITHER_FRAGMENT,
                },
                MaterialParams {
                    pipeline_params: PipelineParams {
                        depth_test: Comparison::Always,
                        ..Default::default()
                    },
                    uniforms: vec![
                        UniformDesc::new("Shown", UniformType::Float1),
                        UniformDesc::new("Cell", UniformType::Float1),
                    ],
                    ..Default::default()
                },
            )
            .ok()
        });
        let (Some(dither), Some((target, _))) = (dither.as_ref(), &self.target) else {
            return;
        };
        dither.set_uniform("Shown", shown);
        dither.set_uniform("Cell", DITHER_CELL * dpi);
        Self::cover(dither, target, width, height);
    }

    /// Draws `target` through `material` over the whole `width` by
    /// `height` window.
    fn cover(material: &Material, target: &RenderTarget, width: f32, height: f32) {
        gl_use_material(material);
        draw_texture_ex(
            &target.texture,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(width, height)),
                // Drawn through a camera into an image, the view comes out
                // upside down (as in `layer.rs`).
                flip_y: true,
                ..Default::default()
            },
        );
        gl_use_default_material();
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/soft_view.rs"]
mod tests;
