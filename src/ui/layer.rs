//! A screen area drawn into an offscreen image only when its content
//! changes, and copied to the screen every frame. Text is the slowest thing
//! Macroquad draws (each letter is looked up and drawn on its own), so the
//! readings panel, whose text changes ten times a second, costs far less
//! this way: on the ThinkPad X230 in Firefox, drawing it every frame took
//! about 8 ms of each frame.

use macroquad::prelude::{
    Camera2D,
    DrawTextureParams,
    FilterMode,
    Material,
    MaterialParams,
    Rect,
    RenderTarget,
    ShaderSource,
    WHITE,
    draw_texture_ex,
    gl_use_default_material,
    gl_use_material,
    load_material,
    render_target,
    screen_dpi_scale,
    set_camera,
    set_default_camera,
    vec2, //
};

/// Copies the image as it is, opaque. Macroquad's own shader would blend
/// it, and the image's alpha is lowered wherever something translucent
/// (antialiased letter edges, the P-V loop fill) was drawn into it, so the
/// copy would show the window background through those spots.
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

const COPY_FRAGMENT: &str = r#"#version 100
varying lowp vec2 uv;

uniform sampler2D Texture;

void main() {
    gl_FragColor = vec4(texture2D(Texture, uv).rgb, 1.0);
}"#;

/// Largest image side made, in pixels. Every desktop GPU in use takes
/// textures this large; a taller window is drawn directly instead.
const MAX_IMAGE_SIDE: u32 = 4_096;

/// Whether the copy shader is ready. If it cannot be built, the area is
/// drawn straight to the screen every frame instead, as before.
enum Copier {
    Untried,
    Ready(Material),
    Unavailable,
}

/// The offscreen image and its size in pixels.
struct Image {
    target: RenderTarget,
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

pub struct CachedLayer {
    copier: Copier,
    image: Option<Image>,
}

impl CachedLayer {
    /// Creates nothing yet: the image and shader need a window, and are
    /// made on the first `draw`.
    pub fn new() -> Self {
        Self {
            copier: Copier::Untried,
            image: None,
        }
    }

    /// Shows `area` (in screen points). `paint` draws its content in
    /// screen coordinates, covering the whole area with opaque color; it
    /// runs only when `stale` is true or the image had to be (re)made, as
    /// on the first frame or after the window changes size.
    pub fn draw(&mut self, area: Rect, stale: bool, paint: impl FnOnce()) {
        if let Copier::Untried = self.copier {
            let shader = ShaderSource::Glsl {
                vertex: COPY_VERTEX,
                fragment: COPY_FRAGMENT,
            };
            // Default pipeline settings: blending off, so the copy is opaque.
            self.copier = match load_material(shader, MaterialParams::default()) {
                Ok(material) => Copier::Ready(material),
                Err(_) => Copier::Unavailable,
            };
        }
        let dpi = screen_dpi_scale();
        let pixels = (
            (area.w * dpi).round().max(1.0) as u32,
            (area.h * dpi).round().max(1.0) as u32,
        );
        let current = self.image.as_ref().map(|image| image.pixels);
        let step = step(
            current,
            pixels,
            stale,
            matches!(self.copier, Copier::Ready(_)),
        );
        let (Copier::Ready(material), false) = (&self.copier, step == Step::Direct) else {
            // Drop any image, so it cannot come back out of date later.
            self.image = None;
            paint();
            return;
        };
        let (image, made) = match (step, self.image.take()) {
            (Step::Repaint | Step::Copy, Some(image)) => (image, false),
            _ => {
                let target = render_target(pixels.0, pixels.1);
                target.texture.set_filter(FilterMode::Linear);
                (Image { target, pixels }, true)
            }
        };
        if made || step != Step::Copy {
            // One point of the area maps to `dpi` pixels of the image, as
            // on the screen, so text is rasterized at full sharpness.
            let mut camera = Camera2D::from_display_rect(area);
            camera.render_target = Some(image.target.clone());
            set_camera(&camera);
            paint();
            set_default_camera();
        }

        gl_use_material(material);
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
