//! Everything on screen around the 3D scene, and the input that drives it:
//! the readings panel (kept as an image between changes) and the sliders,
//! each in a drawer that can be closed, the buttons, the keys list, the
//! camera, the key bindings, and the frame-rate readout.

pub mod button;
pub mod camera;
pub mod controls;
pub mod draw_target;
pub mod drawer;
pub mod fonts;
pub mod frame_stats;
pub mod input;
pub mod layer;
pub mod layout;
pub mod panel;
pub mod pen;
pub mod shortcuts;
pub mod slider;
pub mod start_prompt;
