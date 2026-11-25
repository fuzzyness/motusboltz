//! The gas in the mixing box on the table, for the second law.
//!
//! A small glass box holds two gases side by side behind a divider: hot,
//! fast balls on the left, cold, slow ones on the right. Lift the divider
//! and they mix, sharing their energy in collisions until the whole box is
//! one temperature; lower it again and nothing sorts them back out. The
//! box's entropy rises as they mix and never falls back: heat flows from
//! hot to cold on its own, never the other way.
//!
//! The box is apart from the engine: its balls are hard spheres in a box
//! of their own, in bore radii and seconds, and nothing they do enters the
//! engine's ledgers. Each ball's temperature is its kinetic energy
//! averaged over a little time, so the colors settle as they mix rather
//! than flickering with each collision.

use macroquad::prelude::{
    Vec3,
    vec3, //
};
use macroquad::rand::RandGenerator;

/// Half the inside's width, along the divider's normal, in bore radii.
pub const HALF_WIDTH: f32 = 1.1;
/// Half the inside's depth, front to back, in bore radii.
pub const HALF_DEPTH: f32 = 0.5;
/// Height of the inside, in bore radii.
pub const HEIGHT: f32 = 1.0;
/// Radius of a ball, in bore radii.
pub const BALL_RADIUS: f32 = 0.06;
/// Half the divider's thickness, in bore radii.
pub const DIVIDER_HALF: f32 = 0.02;
/// Balls on each side.
pub const BALLS_PER_SIDE: usize = 60;
/// Speed of a cold ball, in bore radii a second.
const COLD_SPEED: f32 = 0.8;
/// How much hotter the hot side starts than the cold side: its balls move
/// twice as fast, as at 1200 K against 300 K.
pub const HOT_OVER_COLD: f32 = 4.0;
/// Seconds over which a ball's temperature is its averaged kinetic energy.
const TEMPERATURE_SECONDS: f32 = 4.0;
/// Seconds of the longest step, split finer if a frame runs longer.
const MOST_STEP_SECONDS: f32 = 1.0 / 120.0;
/// Most steps in one frame, so a long stall does not run away.
const MOST_STEPS: usize = 8;
/// Seconds the divider takes to rise out of the box or drop back in.
const DIVIDER_SECONDS: f32 = 0.6;
/// The seed the balls are laid out from, the same every time.
const SEED: u64 = 0x5eed_b0c5;

/// One ball: where it is, how it moves, and its temperature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ball {
    /// Its middle, from the middle of the box's floor.
    pub position: Vec3,
    /// Its velocity.
    pub velocity: Vec3,
    /// Its kinetic energy averaged over a little time, per unit of mass
    /// (half its speed squared, averaged).
    pub warmth: f32,
}

/// The two gases in the mixing box and its divider.
pub struct MixingGas {
    /// Every ball.
    balls: Vec<Ball>,
    /// How far the divider has risen, from 0 (down) to 1 (all the way up).
    lift: f32,
    /// Whether the divider is to be up.
    raised: bool,
    /// The box's entropy as last sorted, against which the rise is shown.
    sorted_entropy: f32,
    /// Seconds not yet stepped, short of a step.
    owed: f32,
}

/// Kinetic energy per unit of mass of a ball moving at `speed`.
fn energy(speed: f32) -> f32 {
    speed * speed / 2.0
}

impl MixingGas {
    /// The gases sorted, the divider down.
    pub fn new() -> Self {
        let mut gas = Self {
            balls: Vec::new(),
            lift: 0.0,
            raised: false,
            sorted_entropy: 0.0,
            owed: 0.0,
        };
        gas.sort();
        gas
    }

    /// Sorts the gases back out, hot on the left and cold on the right, at
    /// their first temperatures, and drops the divider.
    pub fn sort(&mut self) {
        let random = RandGenerator::new();
        random.srand(SEED);
        let hot_speed = COLD_SPEED * HOT_OVER_COLD.sqrt();
        let room = |half: f32| half - BALL_RADIUS;
        self.balls = [(-1.0_f32, hot_speed), (1.0, COLD_SPEED)]
            .into_iter()
            .flat_map(|(side, speed)| {
                let random = &random;
                (0..BALLS_PER_SIDE).map(move |_| {
                    let low = DIVIDER_HALF + BALL_RADIUS;
                    let across = random.gen_range(low, room(HALF_WIDTH));
                    let position = vec3(
                        side * across,
                        random.gen_range(BALL_RADIUS, room(HEIGHT)),
                        random.gen_range(-room(HALF_DEPTH), room(HALF_DEPTH)),
                    );
                    let way = vec3(
                        random.gen_range(-1.0, 1.0),
                        random.gen_range(-1.0, 1.0),
                        random.gen_range(-1.0, 1.0),
                    )
                    .normalize_or(Vec3::X);
                    Ball {
                        position,
                        velocity: way * speed,
                        warmth: energy(speed),
                    }
                })
            })
            .collect();
        self.lift = 0.0;
        self.raised = false;
        self.owed = 0.0;
        self.sorted_entropy = self.entropy();
    }

    /// Every ball.
    pub fn balls(&self) -> &[Ball] {
        &self.balls
    }

    /// The temperature a ball starts at on the hot side, and on the cold,
    /// as kinetic energy per unit of mass.
    pub fn first_warmths() -> (f32, f32) {
        let cold = energy(COLD_SPEED);
        (cold * HOT_OVER_COLD, cold)
    }

    /// Lifts the divider if it is down, or drops it if it is up.
    pub fn toggle(&mut self) {
        self.raised = !self.raised;
    }

    /// Whether the divider is up, or on its way up.
    pub fn raised(&self) -> bool {
        self.raised
    }

    /// How far the divider has risen, from 0 to 1.
    pub fn lift(&self) -> f32 {
        self.lift
    }

    /// The height of the divider's bottom edge above the floor: balls
    /// below it pass under.
    fn divider_bottom(&self) -> f32 {
        self.lift * (HEIGHT + BALL_RADIUS)
    }

    /// Runs the box on for `seconds`.
    pub fn advance(&mut self, seconds: f32) {
        let step = (1.0 / DIVIDER_SECONDS) * seconds.max(0.0);
        let target = if self.raised { 1.0 } else { 0.0 };
        self.lift += (target - self.lift).clamp(-step, step);
        self.owed = (self.owed + seconds.max(0.0)).min(MOST_STEP_SECONDS * MOST_STEPS as f32);
        while self.owed >= MOST_STEP_SECONDS {
            self.owed -= MOST_STEP_SECONDS;
            self.step(MOST_STEP_SECONDS);
        }
    }

    /// Moves every ball on by `dt` seconds: along its way, off the walls
    /// and the divider, and off each other.
    fn step(&mut self, dt: f32) {
        let bottom = self.divider_bottom();
        let side_room = DIVIDER_HALF + BALL_RADIUS;
        for ball in &mut self.balls {
            let before = ball.position.x;
            ball.position += ball.velocity * dt;
            bounce(
                &mut ball.position.x,
                &mut ball.velocity.x,
                -HALF_WIDTH,
                HALF_WIDTH,
            );
            bounce(&mut ball.position.y, &mut ball.velocity.y, 0.0, HEIGHT);
            bounce(
                &mut ball.position.z,
                &mut ball.velocity.z,
                -HALF_DEPTH,
                HALF_DEPTH,
            );
            // The divider stops what reaches above its bottom edge, back to
            // the side it came from.
            let blocked = ball.position.y + BALL_RADIUS > bottom;
            if blocked && ball.position.x.abs() < side_room {
                let side = if before != 0.0 {
                    before.signum()
                } else {
                    -ball.velocity.x.signum()
                };
                ball.position.x = side * side_room;
                ball.velocity.x = side * ball.velocity.x.abs();
            }
        }
        self.collide();
        let keep = (-dt / TEMPERATURE_SECONDS).exp();
        for ball in &mut self.balls {
            let now = energy(ball.velocity.length());
            ball.warmth = now + (ball.warmth - now) * keep;
        }
    }

    /// Bounces every two balls that touch and are closing off each other,
    /// as equal hard spheres do, sharing their energy.
    fn collide(&mut self) {
        let touch = 2.0 * BALL_RADIUS;
        for first in 0..self.balls.len() {
            for second in first + 1..self.balls.len() {
                let (a, b) = (self.balls[first], self.balls[second]);
                let apart = b.position - a.position;
                let distance = apart.length();
                if distance >= touch || distance <= f32::EPSILON {
                    continue;
                }
                let normal = apart / distance;
                let closing = (a.velocity - b.velocity).dot(normal);
                if closing <= 0.0 {
                    continue;
                }
                // Equal masses: they swap their speeds along the line
                // between them.
                let swap = normal * closing;
                let overlap = (touch - distance) / 2.0;
                self.balls[first].velocity -= swap;
                self.balls[second].velocity += swap;
                self.balls[first].position -= normal * overlap;
                self.balls[second].position += normal * overlap;
            }
        }
    }

    /// The balls' kinetic energy per unit of mass, all told.
    pub fn energy(&self) -> f32 {
        self.balls
            .iter()
            .map(|ball| energy(ball.velocity.length()))
            .sum()
    }

    /// The box's entropy, in units of the gas constant per ball over its
    /// degrees of freedom, but for a constant: each half of the box, left
    /// and right of the divider, holds its balls at their mean energy, and
    /// a gas of n balls at temperature T has n ln T of it.
    pub fn entropy(&self) -> f32 {
        [-1.0_f32, 1.0]
            .into_iter()
            .map(|side| {
                let (count, total) = self
                    .balls
                    .iter()
                    .filter(|ball| ball.position.x * side >= 0.0)
                    .fold((0_usize, 0.0_f32), |(count, total), ball| {
                        (count + 1, total + energy(ball.velocity.length()))
                    });
                if count == 0 {
                    0.0
                } else {
                    let temperature = (total / count as f32).max(f32::MIN_POSITIVE);
                    count as f32 * temperature.ln()
                }
            })
            .sum()
    }

    /// How far the entropy has risen from its sorted value toward the most
    /// it can reach, all of the box at one temperature: from 0 to 1.
    pub fn entropy_share(&self) -> f32 {
        let count = self.balls.len().max(1) as f32;
        let most = count * (self.energy() / count).max(f32::MIN_POSITIVE).ln();
        let room = most - self.sorted_entropy;
        if room <= f32::EPSILON {
            return 0.0;
        }
        ((self.entropy() - self.sorted_entropy) / room).clamp(0.0, 1.0)
    }
}

impl Default for MixingGas {
    fn default() -> Self {
        Self::new()
    }
}

/// Bounces `place` and `speed` along one axis off walls at `low` and
/// `high`, the balls' middles kept a radius inside them.
fn bounce(place: &mut f32, speed: &mut f32, low: f32, high: f32) {
    let (low, high) = (low + BALL_RADIUS, high - BALL_RADIUS);
    if *place < low {
        *place = low + (low - *place).min(high - low);
        *speed = speed.abs();
    } else if *place > high {
        *place = high - (*place - high).min(high - low);
        *speed = -speed.abs();
    }
}

#[cfg(test)]
#[path = "../../tests/unit/sim/mixing.rs"]
mod tests;
