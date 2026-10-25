//! Particle-renderer tests for `src/graphics/particles.rs`.

use super::*;
use crate::sim::engine::{
    ConnectedChambers,
    PARTICLE_RADIUS, //
};
use crate::sim::geometry::DEFAULT_GEOMETRY;
use crate::sim::units::{
    DEFAULT_CONNECTED_MARKERS,
    MAX_SIMULATED_MARKERS, //
};

const CHAMBER_HEIGHT: f32 = 4.0;
const CHAMBER_RADIUS: f32 = 2.0;

const VERTICES_PER_TRIANGLE: usize = 3;
const TEST_BACKGROUND: Color = Color::from_rgba(12, 18, 30, 255);
const TEST_CAMERA_POSITION: Vec3 = Vec3::new(0.0, 0.0, 8.0);
const TEST_CAMERA_TRANSLATION: Vec3 = Vec3::new(1.0, -2.0, 3.0);
const FLOAT_TOLERANCE: f32 = 1e-6;

fn test_camera() -> Camera3D {
    Camera3D {
        position: TEST_CAMERA_POSITION,
        target: Vec3::ZERO,
        up: Vec3::Y,
        ..Default::default()
    }
}

#[test]
fn sphere_mesh_indices_are_valid_and_face_outward() {
    let sphere = SphereTemplate::new();
    let (triangles, remainder) = sphere.indices.as_chunks::<VERTICES_PER_TRIANGLE>();

    assert!(remainder.is_empty());

    for triangle in triangles {
        let a = sphere.positions[triangle[0] as usize];
        let b = sphere.positions[triangle[1] as usize];
        let c = sphere.positions[triangle[2] as usize];

        let outward = (b - a).cross(c - a).dot(a + b + c);
        assert!(outward > 0.0, "sphere triangle is inverted or degenerate");
    }
}

#[test]
fn shading_matches_template_vertices() {
    let mut sphere = SphereTemplate::new();
    sphere.update_lighting(&test_camera());
    assert_eq!(sphere.positions.len(), sphere.brightness.len());

    for &brightness in &sphere.brightness {
        assert!((AMBIENT_LIGHT..=1.0).contains(&brightness));
    }
}

#[test]
fn visible_surface_brightness_is_stable_while_orbiting() {
    let front_camera = test_camera();
    let back_camera = Camera3D {
        position: -TEST_CAMERA_POSITION,
        ..test_camera()
    };
    let front_light = camera_light_direction(&front_camera);
    let back_light = camera_light_direction(&back_camera);

    let front_brightness = lambert_brightness(Vec3::Z, front_light);
    let back_brightness = lambert_brightness(Vec3::NEG_Z, back_light);

    assert!((front_brightness - back_brightness).abs() < FLOAT_TOLERANCE);
    assert!(front_brightness > AMBIENT_LIGHT);
}

#[test]
fn kinetic_colors_use_an_absolute_energy_scale() {
    let low = kinetic_energy_color(0.0);
    let middle = kinetic_energy_color(NORMAL_PARTICLE_ENERGY);
    let high = kinetic_energy_color(NORMAL_PARTICLE_ENERGY * HOT_ENERGY_MULTIPLIER);

    assert_eq!(low, COLD_COLOR);
    assert_eq!(middle, NORMAL_COLOR);
    assert_eq!(high, HOT_COLOR);
    assert_eq!(kinetic_energy_color(f32::MAX), HOT_COLOR);

    let between = kinetic_energy_color(NORMAL_PARTICLE_ENERGY / 2.0);
    assert!(between.b > NORMAL_COLOR.b);
    assert!(between.r < NORMAL_COLOR.r);
}

#[test]
fn collision_highlight_fades_without_changing_alpha() {
    let base = kinetic_energy_color(NORMAL_PARTICLE_ENERGY);
    let just_collided = collision_color(base, COLLISION_FLASH_DURATION);
    let expired = collision_color(base, 0.0);

    assert!(just_collided.g > base.g);
    assert!(just_collided.b > base.b);
    assert_eq!(just_collided.a, base.a);
    assert_eq!(expired, base);
}

#[test]
fn far_particles_fade_more_than_near_particles() {
    let fade = DepthFade::new(&test_camera(), CHAMBER_RADIUS, CHAMBER_HEIGHT);
    let near = vec3(0.0, 0.0, CHAMBER_RADIUS);
    let far = vec3(0.0, 0.0, -CHAMBER_RADIUS);
    let near_amount = fade.amount(near);
    let far_amount = fade.amount(far);

    assert!(near_amount.abs() < FLOAT_TOLERANCE);
    assert!((far_amount - DEPTH_FADE_STRENGTH).abs() < FLOAT_TOLERANCE);
    assert!(near_amount < far_amount);

    let near_color = mix_color(NORMAL_COLOR, TEST_BACKGROUND, near_amount);
    let far_color = mix_color(NORMAL_COLOR, TEST_BACKGROUND, far_amount);
    assert!(near_color.r > far_color.r);
    assert_eq!(far_color.a, NORMAL_COLOR.a);
}

#[test]
fn panning_does_not_change_depth_fade() {
    let first = test_camera();
    let second = Camera3D {
        position: first.position + TEST_CAMERA_TRANSLATION,
        target: first.target + TEST_CAMERA_TRANSLATION,
        ..test_camera()
    };
    let near = vec3(0.0, 0.0, CHAMBER_RADIUS);
    let first_fade = DepthFade::new(&first, CHAMBER_RADIUS, CHAMBER_HEIGHT);
    let second_fade = DepthFade::new(&second, CHAMBER_RADIUS, CHAMBER_HEIGHT);

    assert!((first_fade.amount(near) - second_fade.amount(near)).abs() < FLOAT_TOLERANCE);
}

#[test]
fn depth_fade_uses_cylindrical_projection() {
    let camera = Camera3D {
        position: vec3(0.0, TEST_CAMERA_POSITION.y + CHAMBER_HEIGHT, CHAMBER_RADIUS),
        target: Vec3::ZERO,
        ..test_camera()
    };
    let fade = DepthFade::new(&camera, CHAMBER_RADIUS, CHAMBER_HEIGHT);
    let projected_radius = CHAMBER_RADIUS
        * (fade.forward.x * fade.forward.x + fade.forward.z * fade.forward.z).sqrt()
        + CHAMBER_HEIGHT / 2.0 * fade.forward.y.abs();

    assert!((fade.half_extent - projected_radius).abs() < FLOAT_TOLERANCE);
}

#[test]
fn batches_cover_all_particles_without_overflowing_indices() {
    let mut renderer = ParticleRenderer::new();
    let particle_count = MAX_SIMULATED_MARKERS;
    renderer.resize_batches(particle_count);

    let total_particles: usize = renderer
        .batches
        .iter()
        .map(|mesh| mesh.vertices.len() / renderer.template.positions.len())
        .sum();

    assert_eq!(total_particles, particle_count);

    for mesh in &renderer.batches {
        assert!(mesh.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
        assert!(mesh.indices.len() < DRAW_CALL_INDEX_CAPACITY);
        assert!(
            mesh.indices
                .iter()
                .all(|&index| (index as usize) < mesh.vertices.len())
        );
    }
}

#[test]
fn resizing_rebuilds_only_the_batch_layout() {
    let mut renderer = ParticleRenderer::new();

    renderer.resize_batches(PARTICLES_PER_BATCH);
    assert_eq!(renderer.batches.len(), 1);

    renderer.resize_batches(PARTICLES_PER_BATCH + 1);
    assert_eq!(renderer.batches.len(), 2);
    assert_eq!(renderer.particle_count, PARTICLES_PER_BATCH + 1);
}

#[test]
#[ignore = "release-mode maximum-fill CPU mesh benchmark (excludes GPU)"]
fn maximum_fill_mesh_benchmark() {
    const SAMPLE_FRAMES: usize = 1_000;
    const SEED: u64 = 42;
    let simulation =
        ConnectedChambers::with_geometry_seed(MAX_SIMULATED_MARKERS, DEFAULT_GEOMETRY, SEED);
    let mut renderer = ParticleRenderer::new();
    let camera = test_camera();
    let chamber = CylinderDimensions {
        radius: CHAMBER_RADIUS,
        height: CHAMBER_HEIGHT,
    };
    renderer.resize_batches(MAX_SIMULATED_MARKERS);
    let start = std::time::Instant::now();
    for _ in 0..SAMPLE_FRAMES {
        renderer.prepare(
            &simulation.particles,
            simulation.collision_flashes(),
            PARTICLE_RADIUS,
            &camera,
            chamber,
            TEST_BACKGROUND,
        );
        std::hint::black_box(&renderer.batches);
    }
    println!(
        "mesh CPU: {:.3} ms/frame; vertices={}; indices={}",
        start.elapsed().as_secs_f64() * 1000.0 / SAMPLE_FRAMES as f64,
        renderer
            .batches
            .iter()
            .map(|m| m.vertices.len())
            .sum::<usize>(),
        renderer
            .batches
            .iter()
            .map(|m| m.indices.len())
            .sum::<usize>()
    );
}

#[test]
fn high_fill_detail_reduction_preserves_particles_and_valid_geometry() {
    let mut renderer = ParticleRenderer::new();
    renderer.resize_batches(MAX_SIMULATED_MARKERS);
    assert!(renderer.low_detail);
    let low_vertices = renderer.template.positions.len();
    for triangle in renderer
        .template
        .indices
        .as_chunks::<VERTICES_PER_TRIANGLE>()
        .0
    {
        let a = renderer.template.positions[triangle[0] as usize];
        let b = renderer.template.positions[triangle[1] as usize];
        let c = renderer.template.positions[triangle[2] as usize];
        assert!((b - a).cross(c - a).dot(a + b + c) > 0.0);
    }
    renderer.resize_batches(DEFAULT_CONNECTED_MARKERS);
    assert!(!renderer.low_detail);
    assert!(renderer.template.positions.len() > low_vertices);
    let represented: usize = renderer
        .batches
        .iter()
        .map(|mesh| mesh.vertices.len() / renderer.template.positions.len())
        .sum();
    assert_eq!(represented, DEFAULT_CONNECTED_MARKERS);
}
