//! MotusBoltz: a particle-driven two-cylinder (alpha-type) Stirling engine.

mod app;
mod graphics;
mod sim;
mod ui;

use crate::graphics::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY, //
};

/// Window size and title, vsync, and Macroquad's per-draw-call buffer sizes.
fn window_conf() -> macroquad::conf::Conf {
    let mut config = macroquad::conf::Conf::default();
    const WINDOW_WIDTH: i32 = 1440;
    const WINDOW_HEIGHT: i32 = 900;
    config.miniquad_conf.window_width = WINDOW_WIDTH;
    config.miniquad_conf.window_height = WINDOW_HEIGHT;
    config.miniquad_conf.window_title = "MotusBoltz".to_owned();
    // Render at full resolution on Retina and other high-DPI displays,
    // natively and in the browser, instead of letting the system upscale.
    config.miniquad_conf.high_dpi = true;
    // Request vsync on native builds; in the browser, requestAnimationFrame
    // paces frames instead.
    config.miniquad_conf.platform.swap_interval = Some(1);
    config.draw_call_vertex_capacity = DRAW_CALL_VERTEX_CAPACITY;
    config.draw_call_index_capacity = DRAW_CALL_INDEX_CAPACITY;
    config
}

#[macroquad::main(window_conf)]
async fn main() {
    app::run().await;
}
