//! Tests for the ice heap and its mist, `src/graphics/ice.rs`.

use super::*;
use crate::graphics::outline::outside_block;
use crate::sim::geometry::DEFAULT_GEOMETRY;

const TIMES: [f64; 5] = [0.0, 1.1, 4.5, 9.0, 86_400.25];
const LIGHT: Vec3 = Vec3::new(0.4, 0.8, 0.4);

fn ice() -> (BlockShape, Ice) {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    (shape, Ice::new(shape))
}

fn facing() -> Facing {
    Facing::along(Vec3::new(-1.0, -0.66, -1.0).normalize())
}

fn build(ice: &mut Ice, cold: f32, seconds: f64) {
    ice.build(cold, seconds, LIGHT.normalize(), facing());
}

#[test]
fn cold_level_runs_from_none_at_room_temperature_to_full_at_the_bottom() {
    assert_eq!(cold_level(MAX_COLD_KELVIN), 0.0);
    assert_eq!(cold_level(MIN_COLD_KELVIN), 1.0);
    assert!((cold_level(200.0) - 0.5).abs() < 1e-6);
    assert_eq!(cold_level(400.0), 0.0);
    assert_eq!(cold_level(50.0), 1.0);
}

#[test]
fn no_ice_or_mist_at_300_k() {
    let (_, mut ice) = ice();
    build(&mut ice, cold_level(MAX_COLD_KELVIN), 3.0);
    assert!(ice.solid().indices.is_empty());
    assert_eq!(ice.sprites().len(), 0);
}

#[test]
fn heap_grows_block_by_block_as_it_gets_colder() {
    let (shape, mut ice) = ice();
    let mut previous = (0, 0.0);
    for kelvin in [290.0, 260.0, 230.0, 200.0, 170.0, 140.0, 100.0] {
        build(&mut ice, cold_level(kelvin), 0.0);
        let vertices = ice.solid().vertices.len();
        let top = ice
            .solid()
            .vertices
            .iter()
            .map(|vertex| vertex.position.y - shape.base)
            .fold(0.0, f32::max);
        assert!(vertices >= previous.0, "fewer blocks at {kelvin} K");
        assert!(top >= previous.1, "lower heap at {kelvin} K");
        previous = (vertices, top);
    }
    assert_eq!(
        previous.0,
        ice.blocks.len() * 4 * FACES_PER_BLOCK,
        "every block shows at 100 K"
    );
    build(&mut ice, cold_level(290.0), 0.0);
    assert!(
        ice.solid().vertices.len() < previous.0 / 4,
        "a slight chill shows a little ice"
    );
}

#[test]
fn heap_rests_on_the_floor_beside_the_cold_half_and_stays_low() {
    let (shape, mut ice) = ice();
    build(&mut ice, 1.0, 0.0);
    for vertex in &ice.solid().vertices {
        let position = vertex.position;
        assert!(position.y >= shape.base - 1e-5);
        // Low enough to leave most of the cold cylinder's window clear.
        assert!(position.y - shape.base <= 0.6 * shape.bore, "{position}");
        assert!(position.x > 0.0, "{position} is on the hot half");
        let point = Vec2::new(position.x, position.z);
        assert!(
            outside_block(shape.offset, shape.outer, point) > -1e-3,
            "{position} is inside the block"
        );
    }
    assert!(ice.solid().vertices.len() < crate::graphics::DRAW_CALL_VERTEX_CAPACITY);
}

#[test]
fn mist_thickens_with_cold_and_never_sinks_through_the_floor() {
    let (shape, mut ice) = ice();
    let mist = |ice: &mut Ice, cold| {
        let mut count = 0;
        let mut cover = 0.0;
        for seconds in TIMES {
            build(ice, cold, seconds);
            for (center, color) in ice.sprites().centers() {
                assert!(center.y > shape.base, "{center} is under the floor");
                count += 1;
                cover += color.a;
            }
        }
        (count, cover)
    };
    let (thin_count, thin_cover) = mist(&mut ice, cold_level(250.0));
    let (thick_count, thick_cover) = mist(&mut ice, cold_level(100.0));
    assert!(thin_count > 0);
    assert!(thick_count > thin_count && thick_cover > 2.0 * thin_cover);
}
