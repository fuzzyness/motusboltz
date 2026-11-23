//! Tests for the regenerator's cartridge, `src/graphics/cartridge.rs`.

use super::*;
use crate::graphics::outline::tests::outside_block;
use crate::graphics::valves::valve_wheels;
use crate::sim::geometry::DEFAULT_GEOMETRY;

/// It lies flat on the table beyond the hot end, clear of the block, and
/// ends its trip seated in the opening, along the shaft.
#[test]
fn it_lies_beyond_the_hot_end_and_ends_in_the_opening() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let (start, flat) = pose(shape, 0.0);
    assert_eq!(start, rest(shape));
    assert!(flat.abs_diff_eq(Quat::IDENTITY, 1e-6));
    let radius = FIT * shape.opening;
    let seen_from_above = macroquad::prelude::Vec2::new(start.x, start.z);
    assert!(start.x < 0.0);
    assert!(outside_block(shape.offset, shape.outer, seen_from_above) > radius + 0.5 * shape.bore);
    let (end, turned) = pose(shape, 1.0);
    assert!(end.distance(seat(shape)) < 1e-5);
    assert!((turned * Vec3::Y).abs_diff_eq(Vec3::X, 1e-5));
}

/// On its way it passes over the gas valve, not through it.
#[test]
fn it_flies_over_the_gas_valve() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let wheel = valve_wheels(shape)[0];
    let clearance = FIT * shape.opening + wheel.radius;
    for step in 0..=200 {
        let (center, _) = pose(shape, step as f32 / 200.0);
        let level = (center.x - wheel.center.x).hypot(center.z - wheel.center.z);
        if level < clearance {
            assert!(center.y > wheel.center.y + clearance / 2.0, "{center}");
        }
    }
}

/// It moves along its trip at a steady pace, turns back when asked, and
/// counts as seated only all the way in.
#[test]
fn it_follows_the_regenerator_over_time() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let mut cartridge = Cartridge::build(shape, false);
    cartridge.follow(true, 10.0);
    assert_eq!(cartridge.progress, 0.0, "no clock on the first frame");
    cartridge.follow(true, 10.0 + TRIP_SECONDS as f64 / 2.0);
    assert!((cartridge.progress - 0.5).abs() < 1e-4);
    assert!(!cartridge.seated());
    cartridge.follow(false, 10.0 + TRIP_SECONDS as f64 * 0.75);
    assert!((cartridge.progress - 0.25).abs() < 1e-4);
    cartridge.follow(true, 20.0);
    assert!(cartridge.seated());
    assert!(Cartridge::build(shape, true).seated());
}

/// Seated, it fills the opening's length through the shared wall, inside
/// the opening's radius: the same cartridge as on the table.
#[test]
fn seated_it_fills_the_opening() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let wall = DEFAULT_GEOMETRY.shared_wall_thickness();
    assert!((THICKNESS * shape.bore - wall).abs() < 1e-4);
    let (center, turn) = pose(shape, 1.0);
    let built = build(shape);
    for vertex in &built.vertices {
        let placed = center + turn * vertex.position;
        assert!(placed.x.abs() <= wall / 2.0 + 1e-4, "{placed}");
        let from_axis = (placed - center).with_x(0.0).length();
        assert!(from_axis < shape.opening, "{placed}");
    }
}

/// Picked up from the table it is carried, lagging toward where it is
/// carried to; let go, it falls onto the table where it was dropped, lies
/// there, and is then where a trip in starts from and a trip out returns.
#[test]
fn it_is_carried_dropped_and_lies_where_it_lands() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let mut cartridge = Cartridge::build(shape, false);
    assert!(cartridge.lies_on_table());
    cartridge.pick_up();
    assert!(!cartridge.lies_on_table());
    let held = vec3(-2.0, shape.base + 4.0 * shape.bore, 3.0);
    cartridge.carry(held, Quat::IDENTITY, 0.01);
    let first = cartridge.carried().unwrap();
    assert!(
        first.distance(held) < rest(shape).distance(held),
        "on its way"
    );
    assert!(first.distance(held) > 0.0, "lagging");
    for _ in 0..60 {
        cartridge.carry(held, Quat::IDENTITY, 1.0 / 60.0);
    }
    assert!(cartridge.carried().unwrap().distance(held) < 1e-3);
    let spot = vec3(-2.0, shape.base, 3.0);
    cartridge.drop_onto(spot);
    assert_eq!(cartridge.carried(), None);
    cartridge.follow(false, 0.0);
    cartridge.follow(false, 0.1);
    assert!(!cartridge.lies_on_table(), "still falling");
    cartridge.follow(false, 5.0);
    assert!(cartridge.lies_on_table());
    let lying = cartridge.lying();
    assert!((lying.x - spot.x).abs() < 1e-6 && (lying.z - spot.z).abs() < 1e-6);
    assert!((lying.y - rest(shape).y).abs() < 1e-6, "flat on the table");
    // In from there, and back out to there.
    cartridge.follow(true, 10.0);
    assert!(cartridge.seated());
    cartridge.follow(false, 20.0);
    assert!(cartridge.lies_on_table());
    assert!(cartridge.now().0.distance(lying) < 1e-4);
}

/// Let go near the opening, it flies in from where it was carried, and is
/// seated at the end of the trip.
#[test]
fn let_go_near_the_opening_it_flies_in() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let mut cartridge = Cartridge::build(shape, false);
    cartridge.pick_up();
    let near = line_up(shape) + Vec3::Z * 0.5 * shape.bore;
    for _ in 0..60 {
        cartridge.carry(near, Quat::IDENTITY, 1.0 / 60.0);
    }
    assert!(cartridge.by_opening());
    cartridge.fly_in();
    let (start, _) = cartridge.now();
    assert!(start.distance(near) < 1e-2, "starts from the hands");
    cartridge.follow(true, 0.0);
    cartridge.follow(true, TRIP_SECONDS as f64 + 0.1);
    assert!(cartridge.seated());
}

/// Taken out of the opening, it leaves from its seat into the hands.
#[test]
fn taken_out_it_leaves_from_its_seat() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let mut cartridge = Cartridge::build(shape, true);
    cartridge.pick_up();
    let (at, _) = cartridge.now();
    assert!(at.distance(seat(shape)) < 1e-5);
    assert!(!cartridge.seated());
}

/// Taken out of the opening, it does not fly straight back in, but does
/// once carried away and brought back.
#[test]
fn taken_out_it_flies_back_in_only_once_carried_away() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let mut cartridge = Cartridge::build(shape, true);
    cartridge.pick_up();
    let dt = 1.0 / 60.0;
    let carry = |cartridge: &mut Cartridge, to: Vec3| {
        for _ in 0..60 {
            cartridge.carry(to, Quat::IDENTITY, dt);
        }
    };
    carry(&mut cartridge, line_up(shape));
    assert!(!cartridge.by_opening(), "not straight back in");
    let away = line_up(shape) + Vec3::Z * 2.0 * SNAP_REACH * SNAP_REARM * shape.bore;
    carry(&mut cartridge, away);
    assert!(!cartridge.by_opening());
    carry(&mut cartridge, line_up(shape));
    assert!(cartridge.by_opening(), "brought back");
}
