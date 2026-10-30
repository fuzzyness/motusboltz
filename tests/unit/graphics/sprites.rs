//! Tests for the soft sprites and their helpers, `src/graphics/sprites.rs`.

use super::*;

const SAMPLES: u32 = 10_000;

#[test]
fn hash_is_repeatable_in_range_and_spread_out() {
    let mut sum = 0.0;
    let mut buckets = [0_u32; 10];
    for index in 0..SAMPLES {
        let value = unit_hash(index, 7);
        assert_eq!(value, unit_hash(index, 7));
        assert!((0.0..1.0).contains(&value));
        sum += value;
        buckets[(value * 10.0) as usize] += 1;
    }
    assert!((sum / SAMPLES as f32 - 0.5).abs() < 0.02);
    for count in buckets {
        assert!(count > SAMPLES / 15, "uneven buckets {buckets:?}");
    }
    assert_ne!(unit_hash(3, 1), unit_hash(3, 2), "salts keep values apart");
}

#[test]
fn age_wraps_and_smoothstep_eases() {
    assert_eq!(age(0.0, 1.0, 0.25), 0.25);
    assert!((age(1.5, 1.0, 0.25) - 0.75).abs() < 1e-6);
    assert!((age(1_000_000.75, 2.0, 0.0) - 0.5).abs() < 1e-3);
    assert!((0.0..1.0).contains(&age(3.0, 0.5, 0.9)));
    assert_eq!(smoothstep(0.2, 0.4, 0.1), 0.0);
    assert_eq!(smoothstep(0.2, 0.4, 0.5), 1.0);
    assert_eq!(smoothstep(0.0, 1.0, 0.5), 0.5);
}

#[test]
fn facing_is_square_to_the_view() {
    for forward in [
        Vec3::new(-1.0, -0.6, -1.0).normalize(),
        Vec3::new(0.0, -1.0, 0.0),
    ] {
        let facing = Facing::along(forward);
        assert!((facing.right.length() - 1.0).abs() < 1e-5);
        assert!((facing.up.length() - 1.0).abs() < 1e-5);
        assert!(facing.right.dot(facing.up).abs() < 1e-5);
        assert!(facing.right.dot(forward).abs() < 1e-5);
        assert!(facing.up.dot(forward).abs() < 1e-5);
    }
}

#[test]
fn finish_draws_floor_sprites_first_then_far_to_near() {
    let facing = Facing::along(Vec3::new(0.0, 0.0, -1.0));
    let mut sprites = Sprites::new();
    let color = with_alpha(Color::from_rgba(255, 255, 255, 255), 0.5);
    for z in [-3.0, 2.0, -8.0, 0.0] {
        sprites.facing(facing, Vec3::new(0.0, 1.0, z), 0.5, 0.5, color);
    }
    sprites.flat(Vec3::new(0.0, 0.0, 5.0), 2.0, color);
    sprites.facing(facing, Vec3::ZERO, 1.0, 1.0, with_alpha(color, 0.0));
    assert_eq!(sprites.len(), 5, "a clear sprite is left out");
    sprites.finish(facing.forward);

    let mesh = sprites.mesh();
    assert_eq!(mesh.vertices.len(), 5 * VERTICES_PER_SPRITE);
    assert_eq!(mesh.indices.len(), 5 * INDICES_PER_SPRITE);
    let centers: Vec<Vec3> = mesh
        .vertices
        .as_chunks::<VERTICES_PER_SPRITE>()
        .0
        .iter()
        .map(|corners| corners.iter().map(|vertex| vertex.position).sum::<Vec3>() / 4.0)
        .collect();
    assert_eq!(centers[0].y, 0.0, "the floor sprite comes first");
    let depths: Vec<f32> = centers[1..].iter().map(|center| center.z).collect();
    assert_eq!(depths, [-8.0, -3.0, 0.0, 2.0]);
}

#[test]
fn a_full_batch_stays_within_one_draw_call() {
    let facing = Facing::along(Vec3::NEG_Z);
    let mut sprites = Sprites::new();
    let color = with_alpha(Color::from_rgba(255, 255, 255, 255), 1.0);
    for index in 0..MAX_SPRITES + 10 {
        sprites.facing(facing, Vec3::X * index as f32, 0.1, 0.1, color);
    }
    assert_eq!(sprites.len(), MAX_SPRITES);
    sprites.finish(facing.forward);
    assert!(sprites.mesh().vertices.len() <= DRAW_CALL_VERTEX_CAPACITY);
    assert!(sprites.mesh().indices.len() <= DRAW_CALL_INDEX_CAPACITY);
}

#[test]
fn shaders_are_glsl_100() {
    assert!(SPRITE_VERTEX.starts_with("#version 100\n"));
    for shader in [SPRITE_FRAGMENT, FLAME_FRAGMENT, SPARKLE_FRAGMENT] {
        assert!(shader.starts_with("#version 100\n"));
        assert!(
            shader.contains("discard"),
            "clear corners must not hide what is behind"
        );
    }
}
