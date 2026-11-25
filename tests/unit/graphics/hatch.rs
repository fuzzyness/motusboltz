//! Tests for the regenerator's slot and door, `src/graphics/hatch.rs`.

use super::*;
use crate::graphics::cartridge::{
    FIT,
    THICKNESS, //
};
use crate::sim::geometry::DEFAULT_GEOMETRY;
use crate::sim::port::Port;
use crate::ui::walking::WalkMap;

/// The cartridge slides edgewise through the slot, which reaches from the
/// front face to the opening and no further round than the shared wall.
#[test]
fn the_slot_fits_the_cartridge() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let slot = slot(shape);
    assert!(2.0 * slot.half_width >= THICKNESS * shape.bore - 1e-5);
    assert!(slot.half_width <= shape.offset - shape.bore + 1e-6);
    assert!(slot.top - slot.bottom >= 2.0 * FIT * shape.opening);
    assert!(slot.bottom >= shape.floor - 1e-6);
    assert_eq!(slot.front, shape.outer);
    assert!(opening_edge(shape, shape.opening_y) == shape.opening);
    assert_eq!(opening_edge(shape, slot.top), 0.0);
}

/// Shut, the door covers the slot; open, it clears it and still stands
/// below the block's top.
#[test]
fn the_door_covers_the_slot_and_clears_it_when_open() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let slot = slot(shape);
    let door = door_solid(shape);
    let (low, high) = door
        .vertices
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(low, high), vertex| {
            (low.min(vertex.position.y), high.max(vertex.position.y))
        });
    assert!(low < slot.bottom && high > slot.top);
    let travel = door_travel(shape);
    assert!(low + travel >= slot.top, "open, it clears the slot");
    let frame = frame_solid(shape);
    let head = frame
        .vertices
        .iter()
        .map(|vertex| vertex.position.y)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(high + travel <= head && head < shape.top);
}

/// The door slides open in its time, eased, and back shut, turning back
/// from wherever it is.
#[test]
fn the_door_slides_open_and_shut() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let mut hatch = Hatch {
        frame: GpuMesh::empty(),
        door: GpuMesh::empty(),
        travel: door_travel(shape),
        open: 0.0,
        last_seconds: None,
        material: None,
    };
    let frame = 1.0 / 60.0;
    let mut seconds = 0.0;
    let mut last = -1.0;
    while hatch.lift() < hatch.travel {
        hatch.follow(true, seconds);
        assert!(hatch.lift() >= last);
        last = hatch.lift();
        seconds += frame;
        assert!(seconds < 2.0 * f64::from(DOOR_SECONDS), "opens in its time");
    }
    hatch.follow(false, seconds + f64::from(DOOR_SECONDS) / 2.0);
    let halfway = hatch.lift();
    assert!(0.0 < halfway && halfway < hatch.travel);
    hatch.follow(false, seconds + 2.0 * f64::from(DOOR_SECONDS));
    assert_eq!(hatch.lift(), 0.0);
}

/// Everything on the front face stands within what the figure walks
/// round in first person.
#[test]
fn the_hatch_stands_within_the_blocks_walking_room() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let map = WalkMap::new(shape, Port::new(DEFAULT_GEOMETRY), None);
    let block = map.obstacles[0].footprint;
    for solid in [frame_solid(shape), door_solid(shape)] {
        for vertex in &solid.vertices {
            let point = vertex.position;
            let (outside, _) = block.distance(macroquad::prelude::vec2(point.x, point.z));
            assert!(outside < 0.0, "{point} sticks out");
        }
    }
}
