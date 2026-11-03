//! Labels on the scene: tags beside the hot and cold cylinders, the
//! opening, the flywheel and the gas, each with its part in the system,
//! and beside the hand crank, the valves, the Reset and Tracers buttons and
//! the regenerator's cartridge, each with what it does.
//!
//! With Labels on, every tag shows; otherwise a part's tag shows while the
//! pointer is over the part (see `hovered`), one tag at a time.
//!
//! The helium is the system; the hot and cold walls and the flywheel are
//! its surroundings. Each tag stands clear of its part, past a point set in
//! the engine's own space, with a thin leader back to a dot on the part, so
//! the tags follow the parts as the camera turns.

use macroquad::prelude::{
    Color,
    Mat4,
    Rect,
    Vec2,
    Vec3,
    vec2,
    vec3, //
};

use crate::graphics::assembly::flywheel_radius;
use crate::graphics::assembly::hand_crank;
use crate::graphics::block::{
    BASE_THICKNESS,
    BlockShape,
    WALL_THICKNESS, //
};
use crate::graphics::buttons::{
    ButtonBox,
    button_boxes, //
};
use crate::graphics::cartridge;
use crate::graphics::valves::{
    ValveWheel,
    valve_wheels, //
};
use crate::sim::engine::ConnectedChambers;
use crate::ui::controls::{
    BACKING,
    BACKING_RADIUS,
    SceneArea,
    TOGGLES_BOTTOM,
    row_top,
    slider_column, //
};
use crate::ui::motion::{
    faded,
    smooth, //
};
use crate::ui::panel::{
    QUIET,
    TEXT, //
};
use crate::ui::pen::{
    Pen,
    Weight,
    grown, //
};

/// Length of a leader past the point that clears its part, in points.
const LEADER: f32 = 40.0;
/// Width of a leader.
const LEADER_WIDTH: f32 = 1.5;
/// Radius of the dot on a part.
const DOT_RADIUS: f32 = 3.0;
/// Color of the leaders and dots.
const LEADER_COLOR: Color = Color::new(1.0, 1.0, 1.0, 0.75);
/// Space between a tag's edge and its text.
const PADDING: f32 = 7.0;
/// Text size of a part's name.
const NAME_SIZE: f32 = 14.0;
/// Text size of a part's role.
const ROLE_SIZE: f32 = 12.0;
/// Distance between the name's and the role's baselines.
const LINE: f32 = 15.0;
/// Least room kept between a tag and the scene's edges.
const EDGE_ROOM: f32 = 8.0;
/// Shortest step on screen that still gives a leader its own direction.
const MIN_STEP: f32 = 1.0;
/// How far a tag has grown before its words start to show, as a share of
/// its full size.
const WORDS_FROM: f32 = 0.6;
/// Least room kept between two tags.
const TAG_GAP: f32 = 6.0;

/// Where on a part the pointer counts as over it, in the engine's space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Spot {
    /// Anywhere within `radius` of `center`.
    Ball {
        /// The middle of the part.
        center: Vec3,
        /// How far round it the part reaches.
        radius: f32,
    },
    /// The wall of an upright cylinder near its outline: where the line of
    /// sight passes no nearer its axis than `inner`, and no farther than
    /// `outer`, between the heights `low` and `high`, and not on the side
    /// facing the other cylinder.
    Rim {
        /// Where the axis crosses the floor's plane (x, z).
        axis: Vec2,
        /// Nearest the line of sight may pass the axis.
        inner: f32,
        /// Farthest it may pass the axis: the wall's outside.
        outer: f32,
        /// Bottom of the wall.
        low: f32,
        /// Top of the wall.
        high: f32,
        /// The way along x that faces away from the other cylinder.
        side: f32,
    },
    /// Inside an upright cylinder, within `radius` of its axis, between the
    /// heights `low` and `high`.
    Bore {
        /// Where the axis crosses the floor's plane (x, z).
        axis: Vec2,
        /// Radius of the inside.
        radius: f32,
        /// Bottom of the inside.
        low: f32,
        /// Top of the inside.
        high: f32,
    },
}

/// Least share of a wall's outward way that may face the other cylinder
/// for a point on its outline to count as the wall's: below this, the
/// point is on the side the cylinders share.
const RIM_SIDE: f32 = -0.5;

/// How far in from its outside a cylinder's outline counts as its wall,
/// in bore radii.
const RIM_BAND: f32 = 0.3;

/// A line of sight: where it starts and which way it runs, unit length.
#[derive(Clone, Copy, Debug)]
struct Sight {
    /// Where it starts, on the near plane.
    from: Vec3,
    /// Its way, unit length.
    way: Vec3,
}

/// Where the line of sight `sight` passes an upright axis at `axis`: how
/// far along the sight it comes nearest, how near it comes, and from
/// where to where along the sight it is within `radius` of the axis, or
/// `None` if it never is.
fn past_axis(sight: Sight, axis: Vec2, radius: f32) -> Option<(f32, f32, Vec2, f32, f32)> {
    let start = vec2(sight.from.x - axis.x, sight.from.z - axis.y);
    let way = vec2(sight.way.x, sight.way.z);
    let flat = way.length_squared();
    let nearest = if flat > f32::EPSILON {
        -start.dot(way) / flat
    } else {
        0.0
    };
    let offset = start + way * nearest;
    let distance = offset.length();
    if distance > radius {
        return None;
    }
    if flat <= f32::EPSILON {
        // Looking straight along the axis: all of it is within reach.
        return Some((nearest, distance, offset, f32::MIN, f32::MAX));
    }
    let half = ((radius * radius - distance * distance) / flat).sqrt();
    Some((nearest, distance, offset, nearest - half, nearest + half))
}

/// Whether the line of sight `sight` between `enter` and `leave` along it
/// runs between the heights `low` and `high`.
fn within_heights(sight: Sight, enter: f32, leave: f32, low: f32, high: f32) -> bool {
    let (a, b) = (
        sight.from.y + sight.way.y * enter,
        sight.from.y + sight.way.y * leave,
    );
    a.min(b) <= high && a.max(b) >= low
}

impl Spot {
    /// How far along `sight` the spot is met, ahead of the start, or
    /// `None` if the sight misses it.
    fn met(self, sight: Sight) -> Option<f32> {
        let depth = match self {
            Spot::Ball { center, radius } => {
                let along = (center - sight.from).dot(sight.way);
                let nearest = sight.from + sight.way * along;
                let miss = nearest.distance_squared(center);
                let depth = along - (radius * radius - miss).sqrt();
                // A part behind the eye is not under the pointer.
                return (miss <= radius * radius && along >= 0.0).then_some(depth);
            }
            Spot::Rim {
                axis,
                inner,
                outer,
                low,
                high,
                side,
            } => {
                let (_, distance, offset, enter, leave) = past_axis(sight, axis, outer)?;
                let facing = offset.normalize_or_zero().x * side;
                (distance >= inner
                    && facing >= RIM_SIDE
                    && within_heights(sight, enter, leave, low, high))
                .then_some(enter)?
            }
            Spot::Bore {
                axis,
                radius,
                low,
                high,
            } => {
                let (_, _, _, enter, leave) = past_axis(sight, axis, radius)?;
                within_heights(sight, enter, leave, low, high).then_some(enter)?
            }
        };
        Some(depth)
    }

    /// Which spots win when several are under the pointer, least first: a
    /// small part before a wall, a wall before the gas within it.
    fn rank(self) -> u8 {
        match self {
            Spot::Ball { .. } => 0,
            Spot::Rim { .. } => 1,
            Spot::Bore { .. } => 2,
        }
    }
}

/// One labeled part.
#[derive(Clone, Debug, PartialEq)]
pub struct Tag {
    /// Which part it is, the same whatever its name says, so a tag that
    /// changes its name keeps its place and growth, and another that takes
    /// the name does not.
    pub key: &'static str,
    /// The part's name.
    pub name: &'static str,
    /// Its part in the system.
    pub role: &'static str,
    /// The point the leader runs to.
    pub anchor: Vec3,
    /// A second point a leader runs to, for a part in two places.
    pub also: Option<Vec3>,
    /// The step from the anchor to the part's edge, the way the tag stands
    /// out, in the engine's space.
    pub outward: Vec3,
    /// Where the pointer counts as over the part.
    pub spots: Vec<Spot>,
}

/// The key of the cartridge's tag, which shows only while it lies on the
/// table.
pub const CARTRIDGE_KEY: &str = "cartridge";

/// How far a button's or the cartridge's tag stands out from it, in bore
/// radii.
const PART_OUT: f32 = 0.4;
/// How far round its middle a button box counts as under the pointer, in
/// bore radii.
const BOX_REACH: f32 = 0.7;
/// How far round its middle the cartridge counts as under the pointer, in
/// bore radii.
const CARTRIDGE_REACH: f32 = 0.8;
/// How far round the opening's middle counts as over it, in the opening's
/// radii.
const OPENING_REACH: f32 = 1.3;
/// How far round the middle of the hand crank's sweep counts as over it,
/// in the arm's lengths.
const CRANK_REACH: f32 = 1.2;
/// How far round a valve's handwheel counts as over the valve, in the
/// wheel's radii.
const VALVE_REACH: f32 = 1.3;

/// The tags for the engine of `engine`, the system's first and the
/// buttons' and cartridge's last: where two would overlap, the later one
/// gives way. The cartridge has one only while it lies on the table.
pub fn tags(engine: &ConnectedChambers) -> Vec<Tag> {
    let geometry = engine.geometry();
    let outer = geometry.cylinder_radius + WALL_THICKNESS;
    let shaft = engine.power_pistons().shaft_height();
    // The walls' tags stand level with the upper half of the bores, the
    // gas's lower down, under the cold wall's. The opening's clears the
    // block's base at the front.
    let (upper, lower) = (
        geometry.cylinder_height / 4.0,
        -geometry.cylinder_height / 4.0,
    );
    let under_opening = vec3(0.0, -geometry.passage_radius - BASE_THICKNESS, outer);
    // With the regenerator in, the opening's tag names its screens.
    let opening = if engine.regenerator().is_enabled() {
        ("Regenerator", "stores heat, gives it back")
    } else {
        ("Opening", "insulated: no heat")
    };
    let shape = BlockShape::new(geometry);
    let out = PART_OUT * shape.bore;
    let [reset, tracers] = button_boxes(shape);
    let pistons = engine.power_pistons();
    let crank = hand_crank(geometry, pistons);
    let handle = crank.handle(pistons.angle());
    let wheel = flywheel_radius(geometry);
    let button = |key, name, role, box_: ButtonBox| Tag {
        key,
        name,
        role,
        anchor: box_.dome_top(),
        also: None,
        outward: away(box_.dome_top(), out),
        spots: vec![Spot::Ball {
            center: box_.base,
            radius: BOX_REACH * shape.bore,
        }],
    };
    let [hot_valve, cold_valve] = valve_wheels(shape);
    let valve = |key, name, role, wheel: ValveWheel| {
        let front = wheel.center + Vec3::Z * wheel.radius;
        Tag {
            key,
            name,
            role,
            anchor: front,
            also: None,
            outward: away(front, out) + Vec3::Z * out,
            spots: vec![Spot::Ball {
                center: wheel.center,
                radius: VALVE_REACH * wheel.radius,
            }],
        }
    };
    // The walls count as under the pointer near their outlines, the gas
    // inside them.
    let hot_axis = vec2(geometry.hot_center_x(), 0.0);
    let cold_axis = vec2(geometry.cold_center_x(), 0.0);
    let rim = |axis: Vec2, side: f32| Spot::Rim {
        axis,
        inner: shape.bore - RIM_BAND * shape.bore,
        outer,
        low: shape.base,
        high: shape.top,
        side,
    };
    let bore = |axis: Vec2| Spot::Bore {
        axis,
        radius: shape.bore,
        low: shape.floor,
        high: shape.top,
    };
    let mut tags = vec![
        Tag {
            key: "gas",
            name: "Helium gas",
            role: "the system",
            anchor: vec3(geometry.cold_center_x(), lower, 0.0),
            also: Some(vec3(geometry.hot_center_x(), lower, 0.0)),
            outward: Vec3::X * outer,
            spots: vec![bore(hot_axis), bore(cold_axis)],
        },
        Tag {
            key: "hot",
            name: "Hot cylinder",
            role: "hot wall: surroundings",
            anchor: vec3(geometry.hot_center_x() - outer, upper, 0.0),
            also: None,
            outward: -Vec3::X * WALL_THICKNESS,
            spots: vec![rim(hot_axis, -1.0)],
        },
        Tag {
            key: "cold",
            name: "Cold cylinder",
            role: "cold wall: surroundings",
            anchor: vec3(geometry.cold_center_x() + outer, upper, 0.0),
            also: None,
            outward: Vec3::X * WALL_THICKNESS,
            spots: vec![rim(cold_axis, 1.0)],
        },
        Tag {
            key: "flywheel",
            name: "Flywheel",
            role: "surroundings: takes the work",
            anchor: vec3(0.0, shaft + wheel, 0.0),
            also: None,
            outward: -Vec3::X * WALL_THICKNESS,
            spots: vec![Spot::Ball {
                center: vec3(0.0, shaft, 0.0),
                radius: wheel,
            }],
        },
        Tag {
            key: "opening",
            name: opening.0,
            role: opening.1,
            anchor: vec3(0.0, geometry.passage_center_y(), 0.0),
            also: None,
            outward: under_opening,
            spots: vec![Spot::Ball {
                center: vec3(0.0, geometry.passage_center_y(), 0.0),
                radius: OPENING_REACH * geometry.passage_radius,
            }],
        },
        Tag {
            key: "crank",
            name: "Hand crank",
            role: "turn it to start the engine",
            anchor: handle,
            also: None,
            outward: Vec3::X * out + Vec3::Y * out,
            spots: vec![Spot::Ball {
                center: crank.center + Vec3::X * crank.handle_length / 2.0,
                radius: CRANK_REACH * crank.arm,
            }],
        },
        valve(
            "gas valve",
            "Gas valve",
            "turn it to heat the hot side",
            hot_valve,
        ),
        valve(
            "nitrogen valve",
            "Nitrogen valve",
            "turn it to cool the cold side",
            cold_valve,
        ),
        button("reset", "Reset", "press for fresh gas", reset),
        button("tracers", "Tracers", "press to follow a few atoms", tracers),
    ];
    if !engine.regenerator().is_enabled() {
        let rest = cartridge::rest(shape);
        tags.push(Tag {
            key: CARTRIDGE_KEY,
            name: "Regenerator",
            role: "press to slide it in",
            anchor: rest,
            also: None,
            outward: away(rest, out) + Vec3::Y * out,
            spots: vec![Spot::Ball {
                center: rest,
                radius: CARTRIDGE_REACH * shape.bore,
            }],
        });
    }
    tags
}

/// The line of sight through `pointer` on a screen of `size` seen
/// through `matrix`, or `None` if the matrix cannot be turned around.
fn sight(matrix: Mat4, size: Vec2, pointer: Vec2) -> Option<Sight> {
    let back = matrix.inverse();
    let ndc = vec2(
        2.0 * pointer.x / size.x - 1.0,
        1.0 - 2.0 * pointer.y / size.y,
    );
    let near = back.project_point3(ndc.extend(-1.0));
    let far = back.project_point3(ndc.extend(1.0));
    let way = (far - near).try_normalize()?;
    near.is_finite().then_some(Sight { from: near, way })
}

/// Whether each of `tags` is under `pointer` on a screen of `size` seen
/// through `matrix`: at most one is, the one whose part the line of sight
/// meets first among the smallest kind of spot it meets (see
/// `Spot::rank`), so parts that overlap on the screen never show their
/// tags together.
pub fn hovered(tags: &[Tag], matrix: Mat4, size: Vec2, pointer: Vec2) -> Vec<bool> {
    let mut over = vec![false; tags.len()];
    let Some(sight) = sight(matrix, size, pointer) else {
        return over;
    };
    let best = tags
        .iter()
        .enumerate()
        .flat_map(|(index, tag)| {
            tag.spots
                .iter()
                .filter_map(move |spot| spot.met(sight).map(|depth| (spot.rank(), depth, index)))
        })
        .min_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    if let Some((_, _, index)) = best {
        over[index] = true;
    }
    over
}

/// A step of `length` from `point` straight away from the engine's
/// middle, level, so a part's tag stands off on the side away from the
/// engine and does not hide it.
fn away(point: Vec3, length: f32) -> Vec3 {
    vec3(point.x, 0.0, point.z).normalize_or_zero() * length
}

/// Seconds a tag crowded aside takes to settle most of the way into its
/// new place, so it glides there instead of jumping.
const SHIFT_SECONDS: f32 = 0.08;

/// How far each tag has been moved aside from its own place to clear the
/// others, eased over time, by key.
#[derive(Debug, Default)]
pub struct Shifts {
    /// Each tag's key and its eased move.
    moves: Vec<(&'static str, Vec2)>,
    /// Seconds since the last frame, which the moves ease over.
    dt: f32,
}

impl Shifts {
    /// Starts a frame `dt` seconds after the last.
    pub fn tick(&mut self, dt: f32) {
        self.dt = dt.max(0.0);
    }

    /// Eases the move of the tag keyed `key` toward `target` over this
    /// frame and returns it; a tag not seen before takes its move at once.
    fn ease(&mut self, key: &'static str, target: Vec2) -> Vec2 {
        let dt = self.dt;
        let Some((_, shift)) = self.moves.iter_mut().find(|(seen, _)| *seen == key) else {
            self.moves.push((key, target));
            return target;
        };
        let share = 1.0 - (-dt / SHIFT_SECONDS).exp();
        *shift += (target - *shift) * share;
        *shift
    }

    /// The move of the tag keyed `key` as it stands, if it has one.
    fn current(&self, key: &str) -> Option<Vec2> {
        self.moves
            .iter()
            .find(|(seen, _)| *seen == key)
            .map(|(_, shift)| *shift)
    }
}

/// Where `point` lands on a screen of `size` through `matrix`, or `None`
/// behind the camera.
pub fn project(matrix: Mat4, size: Vec2, point: Vec3) -> Option<Vec2> {
    let clip = matrix * point.extend(1.0);
    (clip.w > f32::EPSILON).then(|| {
        let ndc = clip.truncate() / clip.w;
        vec2((ndc.x + 1.0) / 2.0 * size.x, (1.0 - ndc.y) / 2.0 * size.y)
    })
}

/// Half-steps up or down a tag may move to clear the others, each half its
/// height and the gap.
const VERTICAL_STEPS: i32 = 14;
/// Half-steps left or right a tag may move, each half its width.
const SIDEWAYS_STEPS: i32 = 3;

/// `rect` moved as little as it can to clear every box in `taken` and
/// `avoid`, staying inside `bounds`, or `None` if there is no such place
/// within reach.
///
/// It looks a half step at a time up, down and to each side, so a tag
/// crowded on one side finds room on another and no tag is lost while
/// another's room is free.
fn clear_of(rect: Rect, taken: &[Rect], bounds: Rect, avoid: Option<Rect>) -> Option<Rect> {
    let step = vec2(rect.w, rect.h + TAG_GAP) / 2.0;
    let fits = |spot: &Rect| {
        spot.x >= bounds.x
            && spot.y >= bounds.y
            && spot.right() <= bounds.right()
            && spot.bottom() <= bounds.bottom()
            && avoid.is_none_or(|card| !grown(card, TAG_GAP).overlaps(spot))
            && taken
                .iter()
                .all(|other| !grown(*other, TAG_GAP).overlaps(spot))
    };
    let mut best: Option<(f32, Rect)> = None;
    for down in -VERTICAL_STEPS..=VERTICAL_STEPS {
        for across in -SIDEWAYS_STEPS..=SIDEWAYS_STEPS {
            let shift = vec2(across as f32 * step.x, down as f32 * step.y);
            let spot = Rect::new(rect.x + shift.x, rect.y + shift.y, rect.w, rect.h);
            let distance = shift.length();
            if fits(&spot) && best.is_none_or(|(least, _)| distance < least) {
                best = Some((distance, spot));
            }
        }
    }
    best.map(|(_, spot)| spot)
}

/// Where each tag goes, for `tags` seen through `matrix` in `area` clear of
/// `cards`: its dot, its box and how far the box was moved aside from its
/// own place to clear the others, or `None` for a part behind the camera
/// or a tag with no room left.
///
/// The boxes stay between the panel and the window's right edge, left of
/// the slider column's card where they are level with it, and between the
/// toggles and the bottom row. A box in the way of one of `cards` or of an
/// earlier tag moves to the nearest free place (see `clear_of`).
///
/// The right side is not held to the centered span, whose inset mirrors
/// the panel's width, or the tag of a part on the right would be pushed
/// in over the engine.
fn place(
    pen: &Pen,
    tags: &[Tag],
    matrix: Mat4,
    area: SceneArea,
    cards: &[Rect],
) -> Vec<Option<(Vec2, Rect, Vec2)>> {
    let size = vec2(area.window_width, area.height);
    let floor = row_top(area.height);
    let mut taken = cards.to_vec();
    tags.iter()
        .map(|tag| {
            let dot = project(matrix, size, tag.anchor)?;
            // A part seen end on, its outward way pointing at the camera,
            // gets its tag above.
            let edge = project(matrix, size, tag.anchor + tag.outward).unwrap_or(dot);
            let direction = (edge - dot)
                .try_normalize()
                .filter(|_| edge.distance(dot) > MIN_STEP)
                .unwrap_or(-Vec2::Y);
            let width = 2.0 * PADDING
                + pen
                    .measure_weight(tag.name, NAME_SIZE, Weight::Semibold)
                    .max(pen.measure(tag.role, ROLE_SIZE));
            let height = 2.0 * PADDING + LINE + pen.text_height("H", NAME_SIZE);
            let half = vec2(width, height) / 2.0;
            let center = edge + direction * (LEADER + half.dot(direction.abs()));
            let low = vec2(area.edge + EDGE_ROOM, TOGGLES_BOTTOM + EDGE_ROOM) + half;
            let high = vec2(area.window_width, floor) - half - EDGE_ROOM;
            let center = center.clamp(low, high.max(low));
            // Level with the slider column's card, stay left of it.
            let column = slider_column(area.window_width);
            let level = center.y - half.y < column.bottom() + EDGE_ROOM;
            let right = column.x - EDGE_ROOM - half.x;
            let center = if area.sliders && level && center.x > right {
                vec2(right.max(low.x), center.y)
            } else {
                center
            };
            let rect = Rect::new(center.x - half.x, center.y - half.y, width, height);
            let bounds = Rect::new(
                low.x - half.x,
                low.y - half.y,
                high.x.max(low.x) - low.x + width,
                high.y.max(low.y) - low.y + height,
            );
            let avoid = area.sliders.then_some(column);
            let cleared = clear_of(rect, &taken, bounds, avoid)?;
            taken.push(cleared);
            Some((dot, cleared, cleared.point() - rect.point()))
        })
        .collect()
}

/// `rect` of a tag `grown` out of its `dot`, from 0 (a point on the dot) to
/// 1 (full size in its place).
pub fn grown_from(dot: Vec2, rect: Rect, grown: f32) -> Rect {
    let center = dot.lerp(rect.center(), grown);
    let size = rect.size() * grown;
    Rect::new(
        center.x - size.x / 2.0,
        center.y - size.y / 2.0,
        size.x,
        size.y,
    )
}

/// Draws the tags of `engine` seen through `matrix` in `area`, clear of
/// `cards`, each grown out of its dot by its share in `grown`, in the
/// order of `tags`, from 0 (gone) to 1 (full size).
///
/// A tag shrinks toward its dot along its leader; its words fade out
/// first, so they never spill out of a shrinking tag.
///
/// A tag crowded aside to clear another glides there over the frame
/// `shifts` was ticked for, its move kept there from frame to frame.
pub fn draw(
    pen: &mut Pen,
    engine: &ConnectedChambers,
    matrix: Mat4,
    area: SceneArea,
    cards: &[Rect],
    grown: &[f32],
    shifts: &mut Shifts,
) {
    // Only the tags that show take room.
    let (tags, grown): (Vec<Tag>, Vec<f32>) = tags(engine)
        .into_iter()
        .zip(grown.iter().copied())
        .filter(|&(_, grown)| grown > 0.0)
        .unzip();
    let placed = place(pen, &tags, matrix, area, cards);
    let size = vec2(area.window_width, area.height);
    for ((tag, placed), grown) in tags.iter().zip(placed).zip(grown) {
        let words = smooth((grown - WORDS_FROM) / (1.0 - WORDS_FROM));
        let Some((dot, rect, moved)) = placed else {
            continue;
        };
        // A tag crowded aside glides to its new place; the pass that only
        // measures leaves the moves as they stand.
        let eased = if pen.is_warming() {
            shifts.current(tag.key).unwrap_or(moved)
        } else {
            shifts.ease(tag.key, moved)
        };
        let rect = Rect {
            x: rect.x - moved.x + eased.x,
            y: rect.y - moved.y + eased.y,
            ..rect
        };
        let rect = grown_from(dot, rect, grown);
        let end = vec2(
            dot.x.clamp(rect.x, rect.right()),
            dot.y.clamp(rect.y, rect.bottom()),
        );
        pen.line(dot, end, LEADER_WIDTH, faded(LEADER_COLOR, grown));
        pen.circle(dot, DOT_RADIUS, faded(LEADER_COLOR, grown));
        // A part in two places has a second leader, to the nearer edge.
        if let Some(also) = tag.also.and_then(|also| project(matrix, size, also)) {
            let end = vec2(
                also.x.clamp(rect.x, rect.right()),
                also.y.clamp(rect.y, rect.bottom()),
            );
            pen.line(also, end, LEADER_WIDTH, faded(LEADER_COLOR, grown));
            pen.circle(also, DOT_RADIUS, faded(LEADER_COLOR, grown));
        }
        pen.round_rect(rect, BACKING_RADIUS * grown, faded(BACKING, grown));
        if words <= 0.0 {
            continue;
        }
        let left = rect.x + PADDING;
        let name_y = rect.y + PADDING + pen.text_height("H", NAME_SIZE);
        pen.strong(tag.name, left, name_y, NAME_SIZE, faded(TEXT, words));
        pen.text(
            tag.role,
            left,
            name_y + LINE,
            ROLE_SIZE,
            faded(QUIET, words),
        );
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/scene_labels.rs"]
mod tests;
