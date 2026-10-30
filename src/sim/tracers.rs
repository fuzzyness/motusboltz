//! The recent paths of a few chosen particles, for drawing tracers. A
//! particle moves in a straight line between bounces, so the points where
//! its velocity changed, followed by where it is now, trace its path
//! exactly. Recording changes nothing in the physics.

use std::collections::VecDeque;

use macroquad::prelude::Vec3;

use crate::sim::gas::Particle;

/// How much model time of each path is kept.
pub const TRAIL_SECONDS: f64 = 0.15;
/// Most points kept per path, so a particle rattling in a corner cannot
/// grow its path without bound.
const MAX_PATH_POINTS: usize = 256;

/// A point on a path: where the particle was when its velocity changed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathPoint {
    /// Model time, in seconds since the tracers were chosen.
    pub time: f64,
    pub position: Vec3,
}

/// One followed particle: its index and its recent bounce points.
struct Path {
    index: usize,
    velocity: Vec3,
    points: VecDeque<PathPoint>,
}

pub struct Tracers {
    paths: Vec<Path>,
    time: f64,
}

impl Tracers {
    /// Starts following the particles at `indices`, from where they are now.
    pub fn new(indices: &[usize], particles: &[Particle]) -> Self {
        let paths = indices
            .iter()
            .map(|&index| {
                let particle = particles[index];
                Path {
                    index,
                    velocity: particle.velocity,
                    points: VecDeque::from([PathPoint {
                        time: 0.0,
                        position: particle.position,
                    }]),
                }
            })
            .collect();
        Self { paths, time: 0.0 }
    }

    /// Advances the clock by `dt` and notes every followed particle whose
    /// velocity changed since the last call, dropping points older than
    /// `TRAIL_SECONDS`. Call once per substep, after all bounces.
    pub fn record(&mut self, dt: f64, particles: &[Particle]) {
        self.time += dt;
        let oldest = self.time - TRAIL_SECONDS;
        for path in &mut self.paths {
            let particle = particles[path.index];
            if particle.velocity != path.velocity {
                path.velocity = particle.velocity;
                if path.points.len() == MAX_PATH_POINTS {
                    path.points.pop_front();
                }
                path.points.push_back(PathPoint {
                    time: self.time,
                    position: particle.position,
                });
            }
            // Keep one point older than the window, so the path still
            // reaches back the full trail length.
            while path.points.len() > 1 && path.points[1].time <= oldest {
                path.points.pop_front();
            }
        }
    }

    /// Model time since the tracers were chosen.
    pub fn time(&self) -> f64 {
        self.time
    }

    /// The followed particles' indices.
    pub fn indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.paths.iter().map(|path| path.index)
    }

    /// Each followed particle's index and recent bounce points, oldest
    /// first. The path ends at the particle's current position.
    pub fn paths(&self) -> impl Iterator<Item = (usize, &VecDeque<PathPoint>)> + '_ {
        self.paths.iter().map(|path| (path.index, &path.points))
    }
}

#[cfg(test)]
#[path = "../../tests/unit/sim/tracers.rs"]
mod tests;
