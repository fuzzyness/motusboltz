//! What stands in the figure's way as it walks on the table, and what it
//! can stand on.
//!
//! Everything on the table that reaches up into the figure's body is kept
//! as a plain shape on the table's plane, standing up from the table to a
//! top: the block, with the burner and the coil round its foot, as one
//! rounded slab; each valve with its stem and wheel; the button boxes, the
//! helium tank and its pipe to the port, the desk clock, and the
//! regenerator's cartridge while it lies on the table. The flywheel, the
//! crank and the shaft are all above the figure's head, over the block.
//!
//! The figure is a round body pushed back out of whatever it walks into
//! and cannot step up onto, so it slides along rather than sticking. What
//! is low enough it steps up onto, and what it is above it stands on: the
//! cartridge it walks onto, the valves' wheels and the button boxes it can
//! jump onto, and the rest is too tall. Past the table's edge there is
//! nothing to stand on (see `first_person`).

use macroquad::prelude::{
    Mat3,
    Vec2,
    Vec3,
    vec2, //
};

use crate::graphics::block::BlockShape;
use crate::graphics::buttons::button_boxes;
use crate::graphics::clock::clock_place;
use crate::graphics::table::{
    TOP_HALF_DEPTH,
    TOP_HALF_LENGTH, //
};
use crate::graphics::tank::tank_place;
use crate::graphics::valves::valve_wheels;
use crate::sim::port::Port;

/// How far the burner and the coil reach out round the block's foot, in
/// bore radii.
const BLOCK_SKIRT: f32 = 0.15;
/// How far each valve reaches round its wheel's middle, its wheel and
/// stem, as a share of the wheel's radius.
const VALVE_REACH: f32 = 1.3;
/// How far each button box reaches round its middle, as a share of its
/// half width: out to its corners.
const BOX_REACH: f32 = 1.45;
/// How far the tank reaches round its bottle's middle, as a share of the
/// bottle's radius: out to its bumpers.
const TANK_REACH: f32 = 1.2;
/// Radius of the room the tank's pipe takes, from the bottle to the port,
/// in bore radii.
const PIPE_REACH: f32 = 0.2;
/// Half the clock's width, between the middles of the rounds at its
/// sides, as a share of its case's radius: out under its bells.
const CLOCK_HALF_WIDTH: f32 = 0.85;
/// The room the clock takes front to back, and round its sides, as a
/// share of its case's radius.
const CLOCK_REACH: f32 = 0.6;
/// How far the cartridge reaches round its middle while it lies on the
/// table, as a share of the opening's radius.
const CARTRIDGE_REACH: f32 = 1.15;
/// Most the figure steps up onto without jumping, in bore radii: onto
/// the cartridge, but not onto a valve's wheel or a button box.
pub const STEP_UP: f32 = 0.3;
/// How much of the body's radius must be over a shape to stand on it: its
/// middle need not be over it, but nearly.
const FOOTING: f32 = 0.5;
/// Times the figure is pushed out of what it walks into each step, for
/// corners where two things meet.
const PUSHES: usize = 8;

/// A shape on the table's plane (x, z).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Footprint {
    /// A round.
    Round {
        /// Its middle.
        center: Vec2,
        /// Its radius.
        radius: f32,
    },
    /// A round drawn along a line: every point within `radius` of the line
    /// from `from` to `to`.
    Slab {
        /// One end of the line.
        from: Vec2,
        /// The other end.
        to: Vec2,
        /// How far round the line it reaches.
        radius: f32,
    },
}

impl Footprint {
    /// How far `point` is outside the shape (less than 0 inside it), and
    /// the way straight out of it from there, a unit vector.
    pub fn distance(self, point: Vec2) -> (f32, Vec2) {
        let (nearest, radius) = match self {
            Self::Round { center, radius } => (center, radius),
            Self::Slab { from, to, radius } => {
                let along = to - from;
                let share = (point - from).dot(along) / along.length_squared().max(f32::EPSILON);
                (from + along * share.clamp(0.0, 1.0), radius)
            }
        };
        let out = point - nearest;
        let length = out.length();
        // From the very middle, any way out will do.
        let way = if length > f32::EPSILON {
            out / length
        } else {
            Vec2::Y
        };
        (length - radius, way)
    }
}

/// Something standing on the table: its shape seen from above, standing
/// up from the table to its top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Obstacle {
    /// Its shape seen from above.
    pub footprint: Footprint,
    /// The height of its top.
    pub top: f32,
}

/// The table and what stands on it, for walking.
#[derive(Clone, Debug, PartialEq)]
pub struct WalkMap {
    /// Everything the figure cannot walk into.
    pub obstacles: Vec<Obstacle>,
    /// Half the table's length and depth: the figure's middle beyond them
    /// stands on nothing.
    half_size: Vec2,
    /// The height of the table's top, where the figure's feet rest.
    pub floor: f32,
    /// Most the figure steps up onto without jumping.
    step: f32,
}

impl WalkMap {
    /// The table under a block of `shape` with the tank's `port`, with the
    /// cartridge lying on it with its middle at `cartridge`, if it does.
    pub fn new(shape: BlockShape, port: Port, cartridge: Option<Vec3>) -> Self {
        let unit = shape.bore;
        let level = |point: Vec3| vec2(point.x, point.z);
        let stand = |footprint, top| Obstacle { footprint, top };
        let mut obstacles = vec![stand(
            Footprint::Slab {
                from: vec2(-shape.offset, 0.0),
                to: vec2(shape.offset, 0.0),
                radius: shape.outer + BLOCK_SKIRT * unit,
            },
            shape.top,
        )];
        for wheel in valve_wheels(shape) {
            obstacles.push(stand(
                Footprint::Slab {
                    from: vec2(wheel.center.x, shape.outer),
                    to: level(wheel.center),
                    radius: VALVE_REACH * wheel.radius,
                },
                wheel.center.y,
            ));
        }
        for button in button_boxes(shape) {
            // Its dome is low and round, and stood on with the box.
            obstacles.push(stand(
                Footprint::Round {
                    center: level(button.base),
                    radius: BOX_REACH * button.half_width(),
                },
                button.dome_top().y,
            ));
        }
        let tank = tank_place(shape, port);
        let tank_top = tank.foot.y + tank.height;
        obstacles.push(stand(
            Footprint::Round {
                center: level(tank.foot),
                radius: TANK_REACH * tank.radius,
            },
            tank_top,
        ));
        obstacles.push(stand(
            Footprint::Slab {
                from: level(tank.foot),
                to: vec2(port.axis.0, port.axis.1) + level(port.outward) * shape.outer,
                radius: PIPE_REACH * unit,
            },
            tank_top,
        ));
        let clock = clock_place(shape);
        let across = Mat3::from_rotation_y(clock.yaw) * Vec3::X;
        let half = level(across) * CLOCK_HALF_WIDTH * clock.radius();
        obstacles.push(stand(
            Footprint::Slab {
                from: level(clock.base) - half,
                to: level(clock.base) + half,
                radius: CLOCK_REACH * clock.radius(),
            },
            clock.base.y + clock.height(),
        ));
        if let Some(rest) = cartridge {
            // It lies flat, its middle half its thickness up.
            obstacles.push(stand(
                Footprint::Round {
                    center: level(rest),
                    radius: CARTRIDGE_REACH * shape.opening,
                },
                shape.base + 2.0 * (rest.y - shape.base),
            ));
        }
        Self {
            obstacles,
            half_size: vec2(TOP_HALF_LENGTH, TOP_HALF_DEPTH) * unit,
            floor: shape.base,
            step: STEP_UP * unit,
        }
    }

    /// `point` brought onto the table, with room round it for `radius`.
    pub fn onto_table(&self, point: Vec2, radius: f32) -> Vec2 {
        let room = (self.half_size - Vec2::splat(radius)).max(Vec2::ZERO);
        point.clamp(-room, room)
    }

    /// Whether a figure whose middle is at `point` stands on the table.
    pub fn on_table(&self, point: Vec2) -> bool {
        point.x.abs() <= self.half_size.x && point.y.abs() <= self.half_size.y
    }

    /// Whether `obstacle` stands in the way of feet at `feet`, too tall to
    /// step up onto.
    fn blocks(&self, obstacle: &Obstacle, feet: f32) -> bool {
        obstacle.top > feet + self.step
    }

    /// How far a body of `radius` at `point`, its feet at `feet`, is from
    /// the nearest thing in its way, less than 0 where it overlaps one.
    pub fn clearance(&self, point: Vec2, radius: f32, feet: f32) -> f32 {
        self.obstacles
            .iter()
            .filter(|obstacle| self.blocks(obstacle, feet))
            .map(|obstacle| obstacle.footprint.distance(point).0 - radius)
            .fold(f32::INFINITY, f32::min)
    }

    /// `point` moved out of everything in the way of a body of `radius`
    /// there, its feet at `feet`, and the ways it was pushed, to stop it
    /// moving into them.
    pub fn push_out(&self, point: Vec2, radius: f32, feet: f32) -> (Vec2, Vec<Vec2>) {
        let mut point = point;
        let mut pushed = Vec::new();
        for _ in 0..PUSHES {
            let mut moved = false;
            for obstacle in self.obstacles.iter().filter(|o| self.blocks(o, feet)) {
                let (distance, way) = obstacle.footprint.distance(point);
                if distance < radius {
                    point += way * (radius - distance);
                    pushed.push(way);
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        (point, pushed)
    }

    /// The height a body of `radius` at `point`, its feet at `feet`, would
    /// stand at: the highest top under it it is not below by more than a
    /// step, the table's own if it is over the table, or none past the
    /// table's edge with nothing under it.
    pub fn ground(&self, point: Vec2, radius: f32, feet: f32) -> Option<f32> {
        let table = self.on_table(point).then_some(self.floor);
        self.obstacles
            .iter()
            .filter(|obstacle| !self.blocks(obstacle, feet))
            .filter(|obstacle| obstacle.footprint.distance(point).0 < (1.0 - FOOTING) * radius)
            .map(|obstacle| obstacle.top)
            .fold(table, |highest, top| {
                Some(highest.map_or(top, |h| h.max(top)))
            })
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/walking.rs"]
mod tests;
