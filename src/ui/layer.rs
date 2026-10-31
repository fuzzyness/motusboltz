//! A screen area drawn into an offscreen image only when its content
//! changes, and copied to the screen every frame.
//!
//! Text is the slowest thing Macroquad draws (each letter is looked up and drawn on its own), so the
//! readings panel, whose text changes ten times a second, costs far less
//! this way: on the ThinkPad X230 in Firefox, drawing it every frame took
//! about 8 ms of each frame.
//!
//! The image keeps its transparency, so whatever lies behind the area
//! shows through where nothing was painted. It starts clear and is painted
//! with a blend that adds up coverage correctly (a letter's soft edge over
//! clear image leaves a partly covered pixel, not a hole), which stores
//! each pixel premultiplied by its coverage; the copy blends it that way.

use macroquad::miniquad::{
    BlendFactor,
    BlendState,
    BlendValue,
    Equation, //
};
use macroquad::prelude::{
    Camera2D,
    Color,
    DrawTextureParams,
    FilterMode,
    Material,
    MaterialParams,
    PipelineParams,
    Rect,
    RenderTarget,
    ShaderSource,
    WHITE,
    clear_background,
    draw_texture_ex,
    gl_use_default_material,
    gl_use_material,
    load_material,
    render_target,
    screen_dpi_scale,
    vec2, //
};

use super::draw_target::{
    draw_to,
    draw_to_screen, //
};

/// Macroquad's own vertex and fragment shaders, used for painting into the
/// image with a different blend (see `painting_blend`).
const PAINT_VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;

varying lowp vec2 uv;
varying lowp vec4 color;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    color = color0 / 255.0;
    uv = texcoord;
}"#;

/// Macroquad's own fragment shader (see `PAINT_VERTEX`).
const PAINT_FRAGMENT: &str = r#"#version 100
varying lowp vec4 color;
varying lowp vec2 uv;

uniform sampler2D Texture;

void main() {
    gl_FragColor = color * texture2D(Texture, uv);
}"#;

/// Vertex shader that copies the image as it is.
///
/// The pipeline blends it as premultiplied.
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

/// Fragment shader that copies the image as it is.
const COPY_FRAGMENT: &str = r#"#version 100
varying lowp vec2 uv;

uniform sampler2D Texture;

void main() {
    gl_FragColor = texture2D(Texture, uv);
}"#;

/// The clear color the image starts from: nothing painted.
const CLEAR: Color = Color::new(0.0, 0.0, 0.0, 0.0);

/// The blend for painting into the image.
///
/// Color blends as usual (source over), and coverage adds up as
/// `a + a_behind (1 - a)`, so the image ends up premultiplied by coverage.
fn painting_blend() -> PipelineParams {
    PipelineParams {
        color_blend: Some(BlendState::new(
            Equation::Add,
            BlendFactor::Value(BlendValue::SourceAlpha),
            BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
        )),
        alpha_blend: Some(BlendState::new(
            Equation::Add,
            BlendFactor::One,
            BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
        )),
        ..Default::default()
    }
}

/// Copying blend for a premultiplied image: `color + behind (1 - a)`.
fn copying_blend() -> PipelineParams {
    PipelineParams {
        color_blend: Some(BlendState::new(
            Equation::Add,
            BlendFactor::One,
            BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
        )),
        ..Default::default()
    }
}

/// Largest image side made, in pixels.
///
/// Every desktop GPU in use takes textures this large; a taller window is
/// drawn directly instead.
const MAX_IMAGE_SIDE: u32 = 4_096;

/// The painting and copying materials.
struct Materials {
    /// Paints into the image.
    paint: Material,
    /// Copies the image to the screen.
    copy: Material,
}

/// Whether the materials are ready.
///
/// If they cannot be built, the area is drawn straight to the screen every
/// frame instead.
enum Copier {
    /// Not built yet, as they need a window.
    Untried,
    /// Built.
    Ready(Materials),
    /// Could not be built.
    Unavailable,
}

/// Builds the painting and copying materials, if the GPU takes them.
fn load_materials() -> Option<Materials> {
    let material = |vertex, fragment, pipeline_params| {
        load_material(
            ShaderSource::Glsl { vertex, fragment },
            MaterialParams {
                pipeline_params,
                ..Default::default()
            },
        )
        .ok()
    };
    Some(Materials {
        paint: material(PAINT_VERTEX, PAINT_FRAGMENT, painting_blend())?,
        copy: material(COPY_VERTEX, COPY_FRAGMENT, copying_blend())?,
    })
}

/// The offscreen image and its size in pixels.
struct Image {
    /// The image.
    target: RenderTarget,
    /// Its width and height in pixels.
    pixels: (u32, u32),
}

/// What one frame does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    /// Paint straight to the screen, keeping no image.
    Direct,
    /// Make a new image, paint into it and copy it.
    Make,
    /// Paint into the existing image and copy it.
    Repaint,
    /// Copy the existing image as it is.
    Copy,
}

/// Chooses the frame's step from the current image size (if any), the
/// size needed, whether the content changed, and whether copying works.
fn step(image: Option<(u32, u32)>, pixels: (u32, u32), stale: bool, can_copy: bool) -> Step {
    if !can_copy || pixels.0.max(pixels.1) > MAX_IMAGE_SIDE {
        Step::Direct
    } else if image != Some(pixels) {
        Step::Make
    } else if stale {
        Step::Repaint
    } else {
        Step::Copy
    }
}

/// The image size for `area`, in pixels.
fn pixels(area: Rect) -> (u32, u32) {
    let dpi = screen_dpi_scale();
    (
        (area.w * dpi).round().max(1.0) as u32,
        (area.h * dpi).round().max(1.0) as u32,
    )
}

/// A screen area kept as an image between changes.
pub struct CachedLayer {
    /// The materials, once tried.
    copier: Copier,
    /// The image, while it is in use.
    image: Option<Image>,
}

impl CachedLayer {
    /// A layer with nothing made yet.
    ///
    /// The image and shaders need a window, and are made on the first
    /// `draw`.
    pub fn new() -> Self {
        Self {
            copier: Copier::Untried,
            image: None,
        }
    }

    /// What drawing `area` does this frame, given whether its content is
    /// `stale`.
    fn step(&self, area: Rect, stale: bool) -> Step {
        let current = self.image.as_ref().map(|image| image.pixels);
        let can_copy = !matches!(self.copier, Copier::Unavailable);
        step(current, pixels(area), stale, can_copy)
    }

    /// Whether `draw` will run its painter this frame for `area` and
    /// `stale`, so the caller can measure that text first.
    pub fn will_paint(&self, area: Rect, stale: bool) -> bool {
        self.step(area, stale) != Step::Copy
    }

    /// Shows `area`, in screen points, over whatever is drawn behind it.
    ///
    /// `paint` draws its content in screen coordinates. It runs only when
    /// `stale` is true or the image had to be (re)made, as on the first
    /// frame or after the window changes size.
    pub fn draw(&mut self, area: Rect, stale: bool, paint: impl FnOnce()) {
        if let Copier::Untried = self.copier {
            self.copier = match load_materials() {
                Some(materials) => Copier::Ready(materials),
                None => Copier::Unavailable,
            };
        }
        let step = self.step(area, stale);
        let (Copier::Ready(materials), false) = (&self.copier, step == Step::Direct) else {
            // Drop any image, so it cannot come back out of date later.
            self.image = None;
            paint();
            return;
        };
        let image = match self.image.take() {
            Some(image) if step != Step::Make => image,
            _ => {
                let pixels = pixels(area);
                let target = render_target(pixels.0, pixels.1);
                target.texture.set_filter(FilterMode::Linear);
                Image { target, pixels }
            }
        };
        if step != Step::Copy {
            // One point of the area maps to `dpi` pixels of the image, as
            // on the screen, so text is rasterized at full sharpness.
            let mut camera = Camera2D::from_display_rect(area);
            camera.render_target = Some(image.target.clone());
            draw_to(&camera);
            clear_background(CLEAR);
            gl_use_material(&materials.paint);
            paint();
            gl_use_default_material();
            draw_to_screen();
        }

        gl_use_material(&materials.copy);
        draw_texture_ex(
            &image.target.texture,
            area.x,
            area.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(area.w, area.h)),
                // An image rendered through a 2D camera comes out upside
                // down (Macroquad's letterbox example flips it the same way).
                flip_y: true,
                ..Default::default()
            },
        );
        gl_use_default_material();
        self.image = Some(image);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/layer.rs"]
mod tests;
