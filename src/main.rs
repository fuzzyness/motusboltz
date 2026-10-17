//! MotusBoltz: coarse-grained alpha-type Stirling engine.

mod camera;
mod engine;
mod gas;
mod graphics;
mod slider;
mod ui;

use crate::graphics::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY, //
};

fn window_conf() -> macroquad::conf::Conf {
    let mut config = macroquad::conf::Conf::default();
    config.miniquad_conf.window_title = "MotusBoltz".to_owned();
    config.draw_call_vertex_capacity = DRAW_CALL_VERTEX_CAPACITY;
    config.draw_call_index_capacity = DRAW_CALL_INDEX_CAPACITY;
    config
}

#[macroquad::main(window_conf)]
async fn main() {
    ui::run().await;
}
