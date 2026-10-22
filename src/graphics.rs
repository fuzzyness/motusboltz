//! 3D drawing of the particles, cylinders, passage, pistons, crank and
//! flywheel. Drawing only; nothing here affects the physics.

use std::f32::consts::{
    FRAC_PI_2,
    PI,
    TAU, //
};

use macroquad::models::{
    Mesh,
    Vertex,
    draw_mesh, //
};
use macroquad::prelude::{
    Camera3D,
    Color,
    ORANGE,
    Vec2,
    Vec3,
    draw_line_3d,
    vec3, //
};

use crate::engine::{
    ChamberGeometry,
    PowerPistons, //
};
use crate::gas as physics;
use crate::gas::{
    COLLISION_FLASH_DURATION,
    INITIAL_TEMPERATURE,
    Particle, //
};

// Keep each batch within Macroquad's vertex and index buffer limits.
const PARTICLES_PER_BATCH: usize = 256;
const SPHERE_LATITUDE_SEGMENTS: usize = 4;
const SPHERE_LONGITUDE_SEGMENTS: usize = 6;
// At high fill, use coarser spheres to reduce vertex work; every particle is
// still drawn.
const LOW_DETAIL_PARTICLE_THRESHOLD: usize = 1_536;
const LOW_DETAIL_LATITUDE_SEGMENTS: usize = 3;
const LOW_DETAIL_LONGITUDE_SEGMENTS: usize = 4;

// Colors encode each particle's kinetic energy. A single particle has no
// temperature; temperature is a property of many particles.
const COLD_COLOR: Color = Color::from_rgba(60, 151, 255, 255);
const NORMAL_COLOR: Color = ORANGE;
const HOT_COLOR: Color = Color::from_rgba(255, 67, 62, 255);

// Mean kinetic energy per particle at 300 K: 3 kT / 2 in reduced units.
const NORMAL_PARTICLE_ENERGY: f32 = 1.5 * INITIAL_TEMPERATURE;
const HOT_ENERGY_MULTIPLIER: f32 = 3.0;

// Camera-space light components: right, up, and toward the viewer.
const LIGHT_DIRECTION: Vec3 = Vec3::new(0.4, 0.7, 0.6);
const AMBIENT_LIGHT: f32 = 0.35;
const DIFFUSE_LIGHT: f32 = 0.65;
const DEPTH_FADE_STRENGTH: f32 = 0.8;

// Brief collision highlight; purely visual, it changes no energy.
const COLLISION_FLASH_COLOR: Color = Color::from_rgba(255, 252, 226, 255);
const COLLISION_FLASH_STRENGTH: f32 = 0.3;

pub const DRAW_CALL_VERTEX_CAPACITY: usize = 16_384;
pub const DRAW_CALL_INDEX_CAPACITY: usize = 60_000;

/// Radius and height, in model units, of a vertical cylinder centered on
/// the origin. Used for cylinder frames and for the depth-fade extent.
#[derive(Clone, Copy)]
pub struct CylinderDimensions {
    pub radius: f32,
    pub height: f32,
}

/// Mixes two opaque colors, clamping the interpolation fraction to [0, 1].
fn mix_color(first: Color, second: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);

    Color {
        r: first.r + (second.r - first.r) * amount,
        g: first.g + (second.g - first.g) * amount,
        b: first.b + (second.b - first.b) * amount,
        a: first.a,
    }
}

/// Maps a particle's kinetic energy (reduced units) to blue, orange or red on
/// a fixed scale: blue below the 300 K average, orange at it, red at 3x.
fn kinetic_energy_color(energy: f32) -> Color {
    if energy <= NORMAL_PARTICLE_ENERGY {
        mix_color(COLD_COLOR, NORMAL_COLOR, energy / NORMAL_PARTICLE_ENERGY)
    } else {
        let hot_energy = NORMAL_PARTICLE_ENERGY * HOT_ENERGY_MULTIPLIER;
        let progress = (energy - NORMAL_PARTICLE_ENERGY) / (hot_energy - NORMAL_PARTICLE_ENERGY);

        mix_color(NORMAL_COLOR, HOT_COLOR, progress)
    }
}

/// Places the light in camera space so orbiting does not darken every sphere.
fn camera_light_direction(camera: &Camera3D) -> Vec3 {
    let forward = (camera.target - camera.position).normalize();
    let right = forward.cross(camera.up).normalize();
    let up = right.cross(forward).normalize();

    (right * LIGHT_DIRECTION.x + up * LIGHT_DIRECTION.y - forward * LIGHT_DIRECTION.z).normalize()
}

/// Calculates Lambert brightness from the current camera-relative light.
fn lambert_brightness(normal: Vec3, light_direction: Vec3) -> f32 {
    let diffuse = normal.dot(light_direction).max(0.0);
    (AMBIENT_LIGHT + DIFFUSE_LIGHT * diffuse).clamp(0.0, 1.0)
}

/// Returns a brief pale highlight that fades as the simulation advances.
fn collision_color(base: Color, remaining_seconds: f32) -> Color {
    let fraction = remaining_seconds / COLLISION_FLASH_DURATION;
    mix_color(
        base,
        COLLISION_FLASH_COLOR,
        fraction * COLLISION_FLASH_STRENGTH,
    )
}

/// Smooth depth fade for a vertical cylinder centered at the origin.
///
/// Using the chamber's projected extent keeps the fade stable as particles
/// move, and makes camera panning independent of particle brightness.
struct DepthFade {
    forward: Vec3,
    half_extent: f32,
}

impl DepthFade {
    fn new(camera: &Camera3D, radius: f32, height: f32) -> Self {
        assert!(radius.is_finite() && radius > 0.0);
        assert!(height.is_finite() && height > 0.0);

        let forward = (camera.target - camera.position).normalize();
        // The largest projected radius of a cylinder is the projected
        // circular cross-section plus its half-height contribution.
        let horizontal = (forward.x * forward.x + forward.z * forward.z).sqrt();
        let half_extent = radius * horizontal + height / 2.0 * forward.y.abs();

        Self {
            forward,
            half_extent,
        }
    }

    fn amount(&self, position: Vec3) -> f32 {
        let depth = position.dot(self.forward);
        let t = ((depth + self.half_extent) / (2.0 * self.half_extent)).clamp(0.0, 1.0);

        // Cubic smoothstep avoids sharp brightness changes at either end.
        let smooth_depth = t * t * (3.0 - 2.0 * t);
        DEPTH_FADE_STRENGTH * smooth_depth
    }
}

/// Unit sphere topology and reusable lighting coefficients for all batches.
struct SphereTemplate {
    positions: Vec<Vec3>,
    brightness: Vec<f32>,
    indices: Vec<u16>,
}

impl SphereTemplate {
    fn new() -> Self {
        Self::with_segments(SPHERE_LATITUDE_SEGMENTS, SPHERE_LONGITUDE_SEGMENTS)
    }

    fn with_segments(latitude_segments: usize, longitude_segments: usize) -> Self {
        let mut positions = Vec::new();
        let mut indices = Vec::new();

        // Share one vertex at each pole.
        let north = 0;
        positions.push(Vec3::Y);

        for latitude in 1..latitude_segments {
            let theta = PI * latitude as f32 / latitude_segments as f32;
            let (sin_theta, cos_theta) = theta.sin_cos();

            for longitude in 0..longitude_segments {
                let phi = TAU * longitude as f32 / longitude_segments as f32;
                let (sin_phi, cos_phi) = phi.sin_cos();

                positions.push(vec3(sin_theta * cos_phi, cos_theta, sin_theta * sin_phi));
            }
        }

        let south = positions.len();
        positions.push(Vec3::NEG_Y);

        // Ring indices exclude the two pole vertices.
        let ring_vertex = |latitude: usize, longitude: usize| -> usize {
            1 + (latitude - 1) * longitude_segments + longitude
        };

        for longitude in 0..longitude_segments {
            let next = (longitude + 1) % longitude_segments;

            indices.extend([
                north as u16,
                ring_vertex(1, next) as u16,
                ring_vertex(1, longitude) as u16,
            ]);
        }

        for latitude in 1..latitude_segments - 1 {
            for longitude in 0..longitude_segments {
                let next = (longitude + 1) % longitude_segments;
                let upper = ring_vertex(latitude, longitude) as u16;
                let upper_next = ring_vertex(latitude, next) as u16;
                let lower = ring_vertex(latitude + 1, longitude) as u16;
                let lower_next = ring_vertex(latitude + 1, next) as u16;

                indices.extend([upper, upper_next, lower]);
                indices.extend([upper_next, lower_next, lower]);
            }
        }

        for longitude in 0..longitude_segments {
            let next = (longitude + 1) % longitude_segments;

            indices.extend([
                south as u16,
                ring_vertex(latitude_segments - 1, longitude) as u16,
                ring_vertex(latitude_segments - 1, next) as u16,
            ]);
        }

        // Initialize the lighting buffer; update it for the current camera
        // once per frame, rather than once per particle or vertex copy.
        let brightness = vec![AMBIENT_LIGHT; positions.len()];

        Self {
            positions,
            brightness,
            indices,
        }
    }

    fn update_lighting(&mut self, camera: &Camera3D) {
        let light_direction = camera_light_direction(camera);

        for (&normal, brightness) in self.positions.iter().zip(&mut self.brightness) {
            *brightness = lambert_brightness(normal, light_direction);
        }
    }
}

/// Draws every particle as a small shaded sphere, 256 per mesh.
///
/// Each frame rewrites vertex positions and colors in place: color from the
/// particle's energy relative to the gas's mean motion, plus collision
/// highlight, camera-relative lighting and depth fade. Meshes are rebuilt
/// only when the particle count changes.
pub struct ParticleRenderer {
    template: SphereTemplate,
    batches: Vec<Mesh>,
    particle_count: usize,
    low_detail: bool,
}

impl ParticleRenderer {
    /// Creates the sphere template and checks the batch capacity limits.
    pub fn new() -> Self {
        let template = SphereTemplate::new();

        assert!(
            template.positions.len() * PARTICLES_PER_BATCH <= u16::MAX as usize,
            "particle batch exceeds the 16-bit vertex index limit",
        );
        assert!(
            template.positions.len() * PARTICLES_PER_BATCH < DRAW_CALL_VERTEX_CAPACITY,
            "particle batch exceeds Macroquad's vertex buffer capacity",
        );
        assert!(
            template.indices.len() * PARTICLES_PER_BATCH < DRAW_CALL_INDEX_CAPACITY,
            "particle batch exceeds Macroquad's index buffer capacity",
        );

        Self {
            template,
            batches: Vec::new(),
            particle_count: 0,
            low_detail: false,
        }
    }

    fn resize_batches(&mut self, particle_count: usize) {
        if particle_count == self.particle_count {
            return;
        }

        let low_detail = particle_count >= LOW_DETAIL_PARTICLE_THRESHOLD;
        if low_detail != self.low_detail {
            self.template = if low_detail {
                SphereTemplate::with_segments(
                    LOW_DETAIL_LATITUDE_SEGMENTS,
                    LOW_DETAIL_LONGITUDE_SEGMENTS,
                )
            } else {
                SphereTemplate::new()
            };
            self.low_detail = low_detail;
        }
        self.batches.clear();
        let mut remaining = particle_count;

        while remaining > 0 {
            let count = remaining.min(PARTICLES_PER_BATCH);
            let vertices_per_particle = self.template.positions.len();
            let mut vertices = Vec::with_capacity(count * vertices_per_particle);
            let mut indices = Vec::with_capacity(count * self.template.indices.len());

            for particle_index in 0..count {
                // Colors and positions are overwritten in prepare().
                for &position in &self.template.positions {
                    vertices.push(Vertex::new2(position, Vec2::ZERO, NORMAL_COLOR));
                }

                let base = (particle_index * vertices_per_particle) as u16;
                indices.extend(self.template.indices.iter().map(|&index| base + index));
            }

            self.batches.push(Mesh {
                vertices,
                indices,
                texture: None,
            });

            remaining -= count;
        }

        self.particle_count = particle_count;
    }

    /// Updates vertex coordinates and colors, then submits each mesh batch.
    pub fn draw(
        &mut self,
        particles: &[Particle],
        collision_flashes: &[f32],
        radius: f32,
        camera: &Camera3D,
        chamber: CylinderDimensions,
        background: Color,
    ) {
        self.prepare(
            particles,
            collision_flashes,
            radius,
            camera,
            chamber,
            background,
        );
        for batch in &self.batches {
            draw_mesh(batch);
        }
    }

    /// Updates reusable CPU buffers separately from GPU submission.
    fn prepare(
        &mut self,
        particles: &[Particle],
        collision_flashes: &[f32],
        radius: f32,
        camera: &Camera3D,
        chamber: CylinderDimensions,
        background: Color,
    ) {
        assert_eq!(particles.len(), collision_flashes.len());
        self.resize_batches(particles.len());
        self.template.update_lighting(camera);

        let depth_fade = DepthFade::new(camera, chamber.radius, chamber.height);
        let mean_velocity = physics::mean_velocity(particles);

        for ((batch, chunk), flash_chunk) in self
            .batches
            .iter_mut()
            .zip(particles.chunks(PARTICLES_PER_BATCH))
            .zip(collision_flashes.chunks(PARTICLES_PER_BATCH))
        {
            for ((particle, vertices), &flash) in chunk
                .iter()
                .zip(
                    batch
                        .vertices
                        .chunks_exact_mut(self.template.positions.len()),
                )
                .zip(flash_chunk)
            {
                // Ignore the gas's bulk drift: only relative kinetic energy
                // should determine its color.
                let thermal_velocity = particle.velocity - mean_velocity;
                let energy = physics::particle_kinetic_energy(thermal_velocity);
                let base_color = kinetic_energy_color(energy);
                let color = collision_color(base_color, flash);
                let fade = depth_fade.amount(particle.position);

                for ((vertex, &offset), &brightness) in vertices
                    .iter_mut()
                    .zip(&self.template.positions)
                    .zip(&self.template.brightness)
                {
                    vertex.position = particle.position + offset * radius;

                    let shaded = Color {
                        r: color.r * brightness,
                        g: color.g * brightness,
                        b: color.b * brightness,
                        a: color.a,
                    };
                    vertex.color = mix_color(shaded, background, fade).into();
                }
            }
        }
    }
}

// Tubular outlines give the cylinder visible depth without solid walls.
pub(crate) const CYLINDER_RING_SEGMENTS: usize = 96;
const CYLINDER_VERTICAL_GUIDES: usize = 8;
pub(crate) const FRAME_TUBE_SIDES: usize = 8;
pub(crate) const FRAME_TUBE_RADIUS: f32 = 0.035;
const FRAME_AMBIENT_LIGHT: f32 = 0.65;
const FRAME_DIFFUSE_LIGHT: f32 = 0.35;
const FRAME_LIGHT_DIRECTION: Vec3 = Vec3::new(0.4, 0.7, 0.6);

/// Appends a tubular outline to a mesh, using a supplied centerline and basis.
///
/// The basis vectors describe a perpendicular cross-section at each point.
/// The triangles face outward for both circular rims and vertical guides.
pub(crate) fn append_tube(
    mesh: &mut Mesh,
    sections: usize,
    closed: bool,
    color: Color,
    mut cross_section: impl FnMut(usize) -> (Vec3, Vec3, Vec3),
) {
    let first_vertex = mesh.vertices.len();
    let light_direction = FRAME_LIGHT_DIRECTION.normalize();

    for section in 0..sections {
        let (center, axis_u, axis_v) = cross_section(section);

        for side in 0..FRAME_TUBE_SIDES {
            let angle = TAU * side as f32 / FRAME_TUBE_SIDES as f32;
            let (sin_angle, cos_angle) = angle.sin_cos();
            let normal = axis_u * cos_angle + axis_v * sin_angle;
            let position = center + normal * FRAME_TUBE_RADIUS;
            let brightness =
                FRAME_AMBIENT_LIGHT + FRAME_DIFFUSE_LIGHT * normal.dot(light_direction).max(0.0);
            let shaded_color = Color {
                r: color.r * brightness,
                g: color.g * brightness,
                b: color.b * brightness,
                a: color.a,
            };

            mesh.vertices
                .push(Vertex::new2(position, Vec2::ZERO, shaded_color));
        }
    }

    let segments = if closed { sections } else { sections - 1 };

    for section in 0..segments {
        let next_section = (section + 1) % sections;

        for side in 0..FRAME_TUBE_SIDES {
            let next_side = (side + 1) % FRAME_TUBE_SIDES;
            let a = (first_vertex + section * FRAME_TUBE_SIDES + side) as u16;
            let b = (first_vertex + next_section * FRAME_TUBE_SIDES + side) as u16;
            let c = (first_vertex + section * FRAME_TUBE_SIDES + next_side) as u16;
            let d = (first_vertex + next_section * FRAME_TUBE_SIDES + next_side) as u16;

            mesh.indices.extend([a, c, b, b, c, d]);
        }
    }
}

/// Cylinder outline made of thin shaded tubes: top and bottom rims plus
/// vertical guides. Built once and drawn as one mesh each frame. Visual
/// only; collisions use `ChamberGeometry`.
pub struct CylinderFrame {
    mesh: Mesh,
}

impl CylinderFrame {
    /// Builds two circular rims and evenly spaced vertical supports.
    pub fn new(chamber: CylinderDimensions, color: Color) -> Self {
        assert!(chamber.radius.is_finite() && chamber.radius > 0.0);
        assert!(chamber.height.is_finite() && chamber.height > 0.0);

        let mut mesh = Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture: None,
        };
        let half_height = chamber.height / 2.0;

        for y in [-half_height, half_height] {
            append_tube(&mut mesh, CYLINDER_RING_SEGMENTS, true, color, |section| {
                let angle = TAU * section as f32 / CYLINDER_RING_SEGMENTS as f32;
                let (sin_angle, cos_angle) = angle.sin_cos();
                let radial = vec3(cos_angle, 0.0, sin_angle);

                (radial * chamber.radius + Vec3::Y * y, radial, Vec3::Y)
            });
        }

        for guide in 0..CYLINDER_VERTICAL_GUIDES {
            let angle = TAU * guide as f32 / CYLINDER_VERTICAL_GUIDES as f32;
            let (sin_angle, cos_angle) = angle.sin_cos();
            let radial = vec3(cos_angle, 0.0, sin_angle);
            let lateral = vec3(sin_angle, 0.0, -cos_angle);

            append_tube(&mut mesh, 2, false, color, |section| {
                let y = if section == 0 {
                    -half_height
                } else {
                    half_height
                };
                (radial * chamber.radius + Vec3::Y * y, radial, lateral)
            });
        }

        assert!(mesh.vertices.len() <= u16::MAX as usize);
        assert!(mesh.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
        assert!(mesh.indices.len() < DRAW_CALL_INDEX_CAPACITY);

        Self { mesh }
    }

    /// Places an otherwise identical frame around a horizontally offset chamber.
    pub fn at_x(chamber: CylinderDimensions, color: Color, center_x: f32) -> Self {
        let mut frame = Self::new(chamber, color);
        for vertex in &mut frame.mesh.vertices {
            vertex.position.x += center_x;
        }
        frame
    }

    /// Submits the existing frame geometry without regenerating it.
    pub fn draw(&self) {
        draw_mesh(&self.mesh);
    }
}

const LEFT_COLOR: Color = Color::from_rgba(241, 145, 92, 255);
const RIGHT_COLOR: Color = Color::from_rgba(110, 189, 255, 255);
const CONNECTOR_COLOR: Color = Color::from_rgba(180, 224, 209, 255);
const PISTON_COLOR: Color = Color::from_rgba(184, 193, 203, 255);
const PISTON_BOTTOM_COLOR: Color = Color::from_rgba(108, 132, 158, 255);
const PISTON_THICKNESS: f32 = 0.10;
const PISTON_EDGE_CLEARANCE: f32 = 0.04;

/// Piston disc built once; each frame shifts its vertices to the face
/// height instead of rebuilding the mesh.
struct PistonDisc {
    mesh: Mesh,
    y: f32,
}

impl PistonDisc {
    fn new(center_x: f32, chamber_radius: f32) -> Self {
        let mut mesh = Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture: None,
        };
        let radius = chamber_radius - PISTON_EDGE_CLEARANCE;
        mesh.vertices.push(Vertex::new2(
            vec3(center_x, PISTON_THICKNESS, 0.0),
            Vec2::ZERO,
            PISTON_COLOR,
        ));
        mesh.vertices.push(Vertex::new2(
            vec3(center_x, 0.0, 0.0),
            Vec2::ZERO,
            PISTON_BOTTOM_COLOR,
        ));
        for index in 0..CYLINDER_RING_SEGMENTS {
            let angle = TAU * index as f32 / CYLINDER_RING_SEGMENTS as f32;
            let (sin, cos) = angle.sin_cos();
            for (y, color) in [(PISTON_THICKNESS, PISTON_COLOR), (0.0, PISTON_BOTTOM_COLOR)] {
                mesh.vertices.push(Vertex::new2(
                    vec3(center_x + radius * cos, y, radius * sin),
                    Vec2::ZERO,
                    color,
                ));
            }
        }
        for index in 0..CYLINDER_RING_SEGMENTS {
            let next = (index + 1) % CYLINDER_RING_SEGMENTS;
            let upper = (2 + 2 * index) as u16;
            let lower = upper + 1;
            let next_upper = (2 + 2 * next) as u16;
            let next_lower = next_upper + 1;
            mesh.indices.extend([0, next_upper, upper]);
            mesh.indices.extend([1, lower, next_lower]);
            mesh.indices.extend([upper, next_upper, lower]);
            mesh.indices.extend([next_upper, next_lower, lower]);
        }
        Self { mesh, y: 0.0 }
    }

    fn draw(&mut self, face_y: f32) {
        let delta = face_y - self.y;
        for vertex in &mut self.mesh.vertices {
            vertex.position.y += delta;
        }
        self.y = face_y;
        draw_mesh(&self.mesh);
    }
}

/// Both cylinder frames (orange hot, blue cold), the pistons and the
/// passage outline.
pub struct ConnectedChamberVisual {
    left: CylinderFrame,
    right: CylinderFrame,
    left_piston: PistonDisc,
    right_piston: PistonDisc,
    passage_lines: Vec<(Vec3, Vec3)>,
}

impl ConnectedChamberVisual {
    pub fn new(geometry: ChamberGeometry) -> Self {
        let dimensions = CylinderDimensions {
            radius: geometry.cylinder_radius,
            height: geometry.cylinder_height,
        };
        Self {
            left: CylinderFrame::at_x(dimensions, LEFT_COLOR, -geometry.cylinder_offset),
            right: CylinderFrame::at_x(dimensions, RIGHT_COLOR, geometry.cylinder_offset),
            left_piston: PistonDisc::new(-geometry.cylinder_offset, geometry.cylinder_radius),
            right_piston: PistonDisc::new(geometry.cylinder_offset, geometry.cylinder_radius),
            passage_lines: passage_wireframe(geometry),
        }
    }

    pub fn draw(&mut self, left_face_y: f32, right_face_y: f32) {
        self.left.draw();
        self.right.draw();
        self.left_piston.draw(left_face_y);
        self.right_piston.draw(right_face_y);

        for &(a, b) in &self.passage_lines {
            draw_line_3d(a, b, CONNECTOR_COLOR);
        }
    }
}

/// Line segments outlining the passage capsules, computed once. Edges inside
/// another capsule or above the cylinder floors are left out, so elbows and
/// floor openings do not look like closed internal walls.
fn passage_wireframe(geometry: ChamberGeometry) -> Vec<(Vec3, Vec3)> {
    const RING_SEGMENTS: usize = 24;
    const CAP_STEPS: usize = 6;
    const LONGITUDINAL_STRIDE: usize = 3;
    const SURFACE_TOLERANCE: f32 = 0.999;
    let sections = geometry.connector_segments();
    let radius = geometry.connector_radius;
    let mut lines = Vec::new();
    for (section, &(start, end)) in sections.iter().enumerate() {
        let direction = (end - start).normalize();
        let across = direction.cross(Vec3::Z);
        let mut add = |a: Vec3, b: Vec3| {
            let middle = (a + b) / 2.0;
            if middle.y > -geometry.cylinder_height / 2.0 {
                return;
            }
            let hidden = sections.iter().enumerate().any(|(other, &(c, d))| {
                if other == section {
                    return false;
                }
                let axis = d - c;
                let nearest =
                    c + axis * ((middle - c).dot(axis) / axis.length_squared()).clamp(0.0, 1.0);
                middle.distance_squared(nearest) < radius * radius * SURFACE_TOLERANCE
            });
            if !hidden {
                lines.push((a, b));
            }
        };
        for segment in 0..RING_SEGMENTS {
            let angle = TAU * segment as f32 / RING_SEGMENTS as f32;
            let next = TAU * (segment + 1) as f32 / RING_SEGMENTS as f32;
            let radial = |angle: f32| across * angle.cos() + Vec3::Z * angle.sin();
            if segment % LONGITUDINAL_STRIDE == 0 {
                add(start + radial(angle) * radius, end + radial(angle) * radius);
            }
            for (center, sign) in [(start, -1.0), (end, 1.0)] {
                for step in 0..CAP_STEPS {
                    let latitude = std::f32::consts::FRAC_PI_2 * step as f32 / CAP_STEPS as f32;
                    let next_latitude =
                        std::f32::consts::FRAC_PI_2 * (step + 1) as f32 / CAP_STEPS as f32;
                    let point = |angle: f32, latitude: f32| {
                        center
                            + radius
                                * (radial(angle) * latitude.cos()
                                    + direction * sign * latitude.sin())
                    };
                    add(point(angle, latitude), point(next, latitude));
                    if segment % LONGITUDINAL_STRIDE == 0 {
                        add(point(angle, latitude), point(angle, next_latitude));
                    }
                }
            }
        }
    }
    lines
}

const WHEEL_SEGMENTS: usize = 48;
const WHEEL_RADIUS: f32 = 2.10;
const SHAFT_COLOR: Color = Color::from_rgba(198, 202, 219, 255);
const WHEEL_COLOR: Color = Color::from_rgba(223, 187, 112, 255);
const HOT_ROD_COLOR: Color = Color::from_rgba(251, 167, 114, 255);
const COLD_ROD_COLOR: Color = Color::from_rgba(135, 198, 246, 255);

/// Crankshaft, flywheel rim with spokes, cranks and connecting rods.
pub struct ShaftVisual {
    geometry: ChamberGeometry,
}

impl ShaftVisual {
    pub fn new(geometry: ChamberGeometry) -> Self {
        Self { geometry }
    }

    /// Draws the shaft along x, the flywheel at its center, and each
    /// cylinder's crank and connecting rod at the current angle.
    pub fn draw(&self, pistons: &PowerPistons) {
        let geometry = self.geometry;
        let shaft_y = pistons.shaft_height();
        let radius = pistons.stroke() / 2.0;
        let offset = geometry.cylinder_offset;
        draw_line_3d(
            vec3(-offset - 0.5, shaft_y, 0.0),
            vec3(offset + 0.5, shaft_y, 0.0),
            SHAFT_COLOR,
        );

        let wheel_center = vec3(0.0, shaft_y, 0.0);
        for segment in 0..WHEEL_SEGMENTS {
            let angle = TAU * segment as f32 / WHEEL_SEGMENTS as f32;
            let next = TAU * (segment + 1) as f32 / WHEEL_SEGMENTS as f32;
            let point = |theta: f32| -> Vec3 {
                wheel_center + vec3(0.0, WHEEL_RADIUS * theta.cos(), WHEEL_RADIUS * theta.sin())
            };
            draw_line_3d(point(angle), point(next), WHEEL_COLOR);
        }
        for spoke in 0..4 {
            let angle = pistons.angle() + TAU * spoke as f32 / 4.0;
            draw_line_3d(
                wheel_center,
                wheel_center + vec3(0.0, WHEEL_RADIUS * angle.cos(), WHEEL_RADIUS * angle.sin()),
                WHEEL_COLOR,
            );
        }

        for (x, face_y, phase, color) in [
            (-offset, pistons.left().face_y, 0.0, HOT_ROD_COLOR),
            (offset, pistons.right().face_y, FRAC_PI_2, COLD_ROD_COLOR),
        ] {
            let crank_angle = pistons.angle() + phase;
            let pin = vec3(
                x,
                shaft_y + radius * crank_angle.cos(),
                radius * crank_angle.sin(),
            );
            let journal = vec3(x, shaft_y, 0.0);
            let piston_joint = vec3(x, face_y, 0.0);
            draw_line_3d(journal, pin, WHEEL_COLOR);
            draw_line_3d(pin, piston_joint, color);
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/render.rs"]
mod tests_render;

#[cfg(test)]
#[path = "../tests/unit/cylinder_frame.rs"]
mod tests_cylinder_frame;
