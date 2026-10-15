use macroquad::prelude::vec3;
use macroquad::rand::gen_range;

use crate::particle::Particle;
use crate::physics;

pub const CHAMBER_SIZE: f32 = 4.0;
pub const PARTICLE_RADIUS: f32 = 0.18;
pub const PRESSURE_WINDOW_SECONDS: u32 = 1;

const PARTICLES_PER_AXIS: usize = 4;
const PARTICLE_COUNT: usize = PARTICLES_PER_AXIS * PARTICLES_PER_AXIS * PARTICLES_PER_AXIS;
const PARTICLE_SPACING: f32 = 1.0;
const INITIAL_SPEED_LIMIT: f32 = 1.5;

const PHYSICS_STEPS_PER_SECOND: u32 = 120;
const FIXED_DT: f32 = 1.0 / PHYSICS_STEPS_PER_SECOND as f32;
const PRESSURE_SAMPLE_STEPS: u32 = PHYSICS_STEPS_PER_SECOND * PRESSURE_WINDOW_SECONDS;
const MAX_FRAME_TIME: f32 = 0.05;

pub struct Simulation {
    pub particles: Vec<Particle>,
    accumulator: f32,
    accumulated_wall_impulse: f32,
    pressure_sample_steps: u32,
    pressure: Option<f32>,
}

impl Simulation {
    pub fn new() -> Self {
        let mut particles = Vec::with_capacity(PARTICLE_COUNT);

        let initial_offset = (PARTICLES_PER_AXIS - 1) as f32 * PARTICLE_SPACING / 2.0;

        for x in 0..PARTICLES_PER_AXIS {
            for y in 0..PARTICLES_PER_AXIS {
                for z in 0..PARTICLES_PER_AXIS {
                    particles.push(Particle {
                        position: vec3(
                            x as f32 * PARTICLE_SPACING - initial_offset,
                            y as f32 * PARTICLE_SPACING - initial_offset,
                            z as f32 * PARTICLE_SPACING - initial_offset,
                        ),
                        velocity: vec3(
                            gen_range(-INITIAL_SPEED_LIMIT, INITIAL_SPEED_LIMIT),
                            gen_range(-INITIAL_SPEED_LIMIT, INITIAL_SPEED_LIMIT),
                            gen_range(-INITIAL_SPEED_LIMIT, INITIAL_SPEED_LIMIT),
                        ),
                    });
                }
            }
        }

        Self {
            particles,
            accumulator: 0.0,
            accumulated_wall_impulse: 0.0,
            pressure_sample_steps: 0,
            pressure: None,
        }
    }

    pub fn update(&mut self, frame_time: f32) {
        self.accumulator += frame_time.min(MAX_FRAME_TIME);

        let wall_limit = CHAMBER_SIZE / 2.0 - PARTICLE_RADIUS;

        while self.accumulator >= FIXED_DT {
            for particle in &mut self.particles {
                self.accumulated_wall_impulse += particle.update(FIXED_DT, wall_limit);
            }

            physics::resolve_collisions(&mut self.particles, PARTICLE_RADIUS);

            self.pressure_sample_steps += 1;

            if self.pressure_sample_steps >= PRESSURE_SAMPLE_STEPS {
                let duration = self.pressure_sample_steps as f32 * FIXED_DT;

                self.pressure = Some(physics::pressure_from_wall_impulse(
                    self.accumulated_wall_impulse,
                    CHAMBER_SIZE,
                    duration,
                ));

                self.accumulated_wall_impulse = 0.0;
                self.pressure_sample_steps = 0;
            }

            self.accumulator -= FIXED_DT;
        }
    }

    pub fn kinetic_energy(&self) -> f32 {
        physics::kinetic_energy(&self.particles)
    }

    pub fn temperature(&self) -> f32 {
        physics::temperature(&self.particles)
    }

    pub fn pressure(&self) -> Option<f32> {
        self.pressure
    }
}
