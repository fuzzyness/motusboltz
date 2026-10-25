//! The physics: helium particles in two cylinders joined by a U passage,
//! hot and cold walls, and pistons that turn a flywheel. Nothing here draws
//! or reads input; `engine::ConnectedChambers` ties the parts together.

pub mod cycle;
pub mod engine;
pub mod gas;
pub mod geometry;
pub mod grid;
pub mod pistons;
pub mod reservoir;
pub mod units;
