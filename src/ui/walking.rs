//! What stands in the figure's way as it walks on the table and the
//! ground under it, and what it can stand on.
//!
//! Everything on the table that reaches up into the figure's body is kept
//! as a plain shape on the table's plane, standing up from the table to a
//! top: the block, with the burner and the coil round its foot, as one
//! rounded slab; each valve with its stem and wheel; the button boxes, the
//! helium tank and its pipe to the port, the desk clock, the mixing box,
//! and the regenerator's cartridge while it lies on the table. The flywheel, the
//! crank and the shaft are all above the figure's head, over the block.
//!
//! The figure is a round body pushed back out of whatever it walks into
//! and cannot step up onto, so it slides along rather than sticking. What
//! is low enough it steps up onto, and what it is above it stands on: the
//! cartridge it walks onto, the valves' wheels and the button boxes it can
//! jump onto, and the rest is too tall.
//!
//! Each thing stands between a bottom and a top, so what is above the
//! figure's head is no hindrance: on the ground it walks into the desk's
//! knee space from the front, under the pencil drawer, between the end
//! panel and the pedestal, as far back as the rail. The desk's top is a
//! thing like any other, as are its end panel, the pencil drawer and the
//! rail, the pedestal at its right, and each of the pedestal's drawers,
//! pulled out or not, a step on the way between the ground and the top. The ground is the cloud's flat top, or as much of
//! the grid; past its edge there is nothing to stand on (see
//! `first_person`).

use macroquad::prelude::{
    Mat3,
    Vec2,
    Vec3,
    vec2, //
};

use crate::graphics::block::BlockShape;
use crate::graphics::buttons::button_boxes;
use crate::graphics::clock::clock_place;
use crate::graphics::cloud::ground_reach;
use crate::graphics::drawers::{
    DRAWER_COUNT,
    cabinet_place,
    drawer_places, //
};
use crate::graphics::mixing_box::mixing_box_place;
use crate::graphics::table::{
    Board,
    TOP_HALF_DEPTH,
    TOP_HALF_LENGTH,
    back_rail,
    end_panel,
    ground,
    pencil_drawer,
    underside, //
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
/// Room the head keeps under what is overhead when it stops there, in
/// bore radii, so meeting a ceiling never counts as walking into it.
const HEAD_GAP: f32 = 0.01;
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
    /// A rectangle square to the axes.
    Rect {
        /// Its middle.
        center: Vec2,
        /// Half its size each way.
        half: Vec2,
    },
}

impl Footprint {
    /// How far `point` is outside the shape (less than 0 inside it), and
    /// the way straight out of it from there, a unit vector.
    pub fn distance(self, point: Vec2) -> (f32, Vec2) {
        if let Self::Rect { center, half } = self {
            let from = point - center;
            let past = from.abs() - half;
            let sign = vec2(from.x.signum(), from.y.signum());
            if past.x > 0.0 || past.y > 0.0 {
                let out = past.max(Vec2::ZERO);
                return (out.length(), (out * sign).normalize_or(Vec2::Y));
            }
            // Inside: out the nearest side.
            return if past.x > past.y {
                (past.x, vec2(sign.x, 0.0))
            } else {
                (past.y, vec2(0.0, sign.y))
            };
        }
        let (nearest, radius) = match self {
            Self::Round { center, radius } => (center, radius),
            Self::Rect { .. } => unreachable!("measured above"),
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

/// Something to walk round or stand on: its shape seen from above,
/// reaching from its bottom up to its top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Obstacle {
    /// Its shape seen from above.
    pub footprint: Footprint,
    /// The height of its bottom: below this it is no hindrance.
    pub bottom: f32,
    /// The height of its top.
    pub top: f32,
}

/// The table, what stands on it, and the ground under it, for walking.
#[derive(Clone, Debug, PartialEq)]
pub struct WalkMap {
    /// Everything the figure cannot walk into, or can stand on.
    pub obstacles: Vec<Obstacle>,
    /// Half the table's length and depth.
    half_size: Vec2,
    /// The height of the table's top.
    pub floor: f32,
    /// The height of the ground under the table.
    pub ground: f32,
    /// How far the ground reaches round the middle, along and across: an
    /// oval.
    ground_reach: Vec2,
    /// Most the figure steps up onto without jumping.
    step: f32,
    /// The figure's height: what is above its head is no hindrance.
    height: f32,
    /// Room the head keeps under a ceiling it stops at.
    head_gap: f32,
}

impl WalkMap {
    /// The table under a block of `shape` with the tank's `port`, with the
    /// cartridge lying on it with its middle at `cartridge`, if it does.
    pub fn new(shape: BlockShape, port: Port, cartridge: Option<Vec3>) -> Self {
        let unit = shape.bore;
        let level = |point: Vec3| vec2(point.x, point.z);
        let floor = shape.base;
        let stand = |footprint, top| Obstacle {
            footprint,
            bottom: floor,
            top,
        };
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
        let mixer = mixing_box_place(shape);
        let (left, right, depth) = mixer.footprint();
        obstacles.push(stand(
            Footprint::Slab {
                from: level(left),
                to: level(right),
                radius: depth,
            },
            mixer.top(),
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
        // The desk's top, its end panel, rail and pencil drawer, and the
        // pedestal, from the ground up.
        let half_size = vec2(TOP_HALF_LENGTH, TOP_HALF_DEPTH) * unit;
        let (low, under) = (ground(shape), underside(shape));
        obstacles.push(Obstacle {
            footprint: Footprint::Rect {
                center: Vec2::ZERO,
                half: half_size,
            },
            bottom: under,
            top: floor,
        });
        let pencil = pencil_drawer(shape);
        let boards = [
            end_panel(shape),
            back_rail(shape),
            Board {
                low: Vec3::new(pencil.left, pencil.bottom, pencil.back),
                high: Vec3::new(pencil.right, under, pencil.front),
            },
        ];
        for board in boards {
            let (low, high) = (level(board.low), level(board.high));
            obstacles.push(Obstacle {
                footprint: Footprint::Rect {
                    center: (low + high) / 2.0,
                    half: (high - low) / 2.0,
                },
                bottom: board.low.y,
                top: board.high.y,
            });
        }
        let cabinet = cabinet_place(shape);
        obstacles.push(Obstacle {
            footprint: Footprint::Rect {
                center: vec2(
                    (cabinet.left + cabinet.right) / 2.0,
                    (cabinet.back + cabinet.front) / 2.0,
                ),
                half: vec2(
                    (cabinet.right - cabinet.left) / 2.0,
                    (cabinet.front - cabinet.back) / 2.0,
                ),
            },
            bottom: low,
            top: under,
        });
        Self {
            obstacles,
            half_size,
            floor,
            ground: low,
            ground_reach: ground_reach(shape),
            step: STEP_UP * unit,
            height: shape.top - shape.base,
            head_gap: HEAD_GAP * unit,
        }
    }

    /// The map with the drawers of the table under a block of `shape`
    /// pulled out by `pulls`, bottom to top: each a step, wherever it is.
    pub fn with_drawers(mut self, shape: BlockShape, pulls: [f32; DRAWER_COUNT]) -> Self {
        for (drawer, pull) in drawer_places(shape).into_iter().zip(pulls) {
            let (back, front) = (drawer.back + pull, drawer.front + pull);
            self.obstacles.push(Obstacle {
                footprint: Footprint::Rect {
                    center: vec2((drawer.left + drawer.right) / 2.0, (back + front) / 2.0),
                    half: vec2((drawer.right - drawer.left) / 2.0, (front - back) / 2.0),
                },
                bottom: drawer.bottom,
                top: drawer.top,
            });
        }
        self
    }

    /// Whether a figure whose middle is at `point` is over the ground: the
    /// cloud's flat top, or as much of the grid.
    pub fn over_ground(&self, point: Vec2) -> bool {
        (point / self.ground_reach).length_squared() <= 1.0
    }

    /// `point` brought onto the table, with room round it for `radius`.
    pub fn onto_table(&self, point: Vec2, radius: f32) -> Vec2 {
        let room = (self.half_size - Vec2::splat(radius)).max(Vec2::ZERO);
        point.clamp(-room, room)
    }

    /// Whether a figure whose middle is at `point` stands on the table.
    #[cfg(test)]
    pub fn on_table(&self, point: Vec2) -> bool {
        point.x.abs() <= self.half_size.x && point.y.abs() <= self.half_size.y
    }

    /// Whether `obstacle` stands in the way of feet at `feet`: too tall to
    /// step up onto, and not above the head.
    fn blocks(&self, obstacle: &Obstacle, feet: f32) -> bool {
        obstacle.top > feet + self.step && obstacle.bottom < feet + self.height
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

    /// Highest the feet of a body of `radius` at `point`, its feet at
    /// `feet`, can rise before its head meets the underside of something
    /// above it, as the table's top is above a figure under it: none if
    /// nothing is overhead.
    pub fn head_room(&self, point: Vec2, radius: f32, feet: f32) -> Option<f32> {
        let head = feet + self.height;
        self.obstacles
            .iter()
            .filter(|obstacle| obstacle.bottom >= head - self.head_gap)
            .filter(|obstacle| obstacle.footprint.distance(point).0 < radius)
            .map(|obstacle| obstacle.bottom - self.height - self.head_gap)
            .reduce(f32::min)
    }

    /// The height a body of `radius` at `point`, its feet at `feet`, would
    /// stand at: the highest top under it it is not below by more than a
    /// step, the ground's if it is over the ground, or none past the
    /// ground's edge with nothing under it.
    pub fn ground(&self, point: Vec2, radius: f32, feet: f32) -> Option<f32> {
        let reach = feet + self.step;
        let ground = (self.over_ground(point) && self.ground <= reach).then_some(self.ground);
        self.obstacles
            .iter()
            .filter(|obstacle| obstacle.top <= reach)
            .filter(|obstacle| obstacle.footprint.distance(point).0 < (1.0 - FOOTING) * radius)
            .map(|obstacle| obstacle.top)
            .fold(ground, |highest, top| {
                Some(highest.map_or(top, |h| h.max(top)))
            })
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/walking.rs"]
mod tests;
