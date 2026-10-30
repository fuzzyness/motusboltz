//! Switching where drawing goes: the 3D scene, an offscreen image or the
//! screen. Every switch also works around a texture bug in Miniquad 0.4.11
//! that turned all text into black boxes on high-DPI screens.
//!
//! Macroquad draws everything queued so far whenever the camera changes,
//! which can leave the font's texture bound. Text queued later in the same
//! frame may need letters the font has not drawn yet, and when they no
//! longer fit, Macroquad replaces the font texture with a larger one.
//! OpenGL gives the new texture the old one's number, and Miniquad, which
//! still records that number as bound, skips binding it. The new texture
//! then never receives its pixels, and every letter samples an empty
//! texture, which draws black. Miniquad forgets its bindings at the end of
//! every frame; doing the same after each mid-frame draw makes the next
//! bind real. High-DPI screens draw letters at twice the pixel size, which
//! is what fills the font texture.

use macroquad::miniquad::Backend;
use macroquad::prelude::{
    Camera,
    get_internal_gl, //
};

/// Sends the drawing that follows to `camera`.
pub fn draw_to(camera: &dyn Camera) {
    macroquad::prelude::set_camera(camera);
    forget_texture_bindings();
}

/// Sends the drawing that follows to the screen, in screen points.
pub fn draw_to_screen() {
    macroquad::prelude::set_default_camera();
    forget_texture_bindings();
}

/// Clears Miniquad's record of bound textures (and buffers), exactly as it
/// does at the end of every frame. Only the OpenGL backend keeps such a
/// record; on Metal, `commit_frame` would end the frame instead.
fn forget_texture_bindings() {
    // SAFETY: called on the main thread between Macroquad's own draws, as
    // Macroquad itself calls `commit_frame` at the end of each frame.
    let gl = unsafe { get_internal_gl() };
    if gl.quad_context.info().backend == Backend::OpenGl {
        gl.quad_context.commit_frame();
    }
}
