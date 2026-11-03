//! Labels on the scene: tags beside the hot and cold cylinders, the
//! opening, the flywheel and the gas, each with its part in the system,
//! and beside the Pause and Tracers buttons and the regenerator's
//! cartridge, each with what pressing it does.
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
use crate::graphics::block::{
    BASE_THICKNESS,
    BlockShape,
    WALL_THICKNESS, //
};
use crate::graphics::buttons::button_boxes;
use crate::graphics::cartridge;
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

/// One labeled part.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tag {
    /// The part's name.
    pub name: &'static str,
    /// Its part in the system.
    pub role: &'static str,
    /// The point the leader runs to.
    pub anchor: Vec3,
    /// The step from the anchor to the part's edge, the way the tag stands
    /// out, in the engine's space.
    pub outward: Vec3,
}

/// How far a button's or the cartridge's tag stands out from it, in bore
/// radii.
const PART_OUT: f32 = 0.4;

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
    let [pause, tracers] = button_boxes(shape);
    let mut tags = vec![
        Tag {
            name: "Helium gas",
            role: "the system",
            anchor: vec3(geometry.cold_center_x(), lower, 0.0),
            outward: Vec3::X * outer,
        },
        Tag {
            name: "Hot cylinder",
            role: "hot wall: surroundings",
            anchor: vec3(geometry.hot_center_x() - outer, upper, 0.0),
            outward: -Vec3::X * WALL_THICKNESS,
        },
        Tag {
            name: "Cold cylinder",
            role: "cold wall: surroundings",
            anchor: vec3(geometry.cold_center_x() + outer, upper, 0.0),
            outward: Vec3::X * WALL_THICKNESS,
        },
        Tag {
            name: "Flywheel",
            role: "surroundings: takes the work",
            anchor: vec3(0.0, shaft + flywheel_radius(geometry), 0.0),
            outward: -Vec3::X * WALL_THICKNESS,
        },
        Tag {
            name: opening.0,
            role: opening.1,
            anchor: vec3(0.0, geometry.passage_center_y(), 0.0),
            outward: under_opening,
        },
        Tag {
            name: "Pause",
            role: "press to pause or go on",
            anchor: pause.dome_top(),
            outward: away(pause.dome_top(), out),
        },
        Tag {
            name: "Tracers",
            role: "press to follow a few atoms",
            anchor: tracers.dome_top(),
            outward: away(tracers.dome_top(), out),
        },
    ];
    if !engine.regenerator().is_enabled() {
        tags.push(Tag {
            name: "Regenerator",
            role: "press to slide it in",
            anchor: cartridge::rest(shape),
            outward: away(cartridge::rest(shape), out) + Vec3::Y * out,
        });
    }
    tags
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
/// others, eased over time, by name.
#[derive(Debug, Default)]
pub struct Shifts {
    /// Each tag's name and its eased move.
    moves: Vec<(&'static str, Vec2)>,
    /// Seconds since the last frame, which the moves ease over.
    dt: f32,
}

impl Shifts {
    /// Starts a frame `dt` seconds after the last.
    pub fn tick(&mut self, dt: f32) {
        self.dt = dt.max(0.0);
    }

    /// Eases `name`'s move toward `target` over this frame and returns it;
    /// a tag not seen before takes its move at once.
    fn ease(&mut self, name: &'static str, target: Vec2) -> Vec2 {
        let dt = self.dt;
        let Some((_, shift)) = self.moves.iter_mut().find(|(seen, _)| *seen == name) else {
            self.moves.push((name, target));
            return target;
        };
        let share = 1.0 - (-dt / SHIFT_SECONDS).exp();
        *shift += (target - *shift) * share;
        *shift
    }

    /// `name`'s move as it stands, if it has one.
    fn current(&self, name: &str) -> Option<Vec2> {
        self.moves
            .iter()
            .find(|(seen, _)| *seen == name)
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
/// `cards`, each `grown` out of its dot from 0 (gone) to 1 (full size).
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
    grown: f32,
    shifts: &mut Shifts,
) {
    let tags = tags(engine);
    let words = smooth((grown - WORDS_FROM) / (1.0 - WORDS_FROM));
    for (tag, placed) in tags.iter().zip(place(pen, &tags, matrix, area, cards)) {
        let Some((dot, rect, moved)) = placed else {
            continue;
        };
        // A tag crowded aside glides to its new place; the pass that only
        // measures leaves the moves as they stand.
        let eased = if pen.is_warming() {
            shifts.current(tag.name).unwrap_or(moved)
        } else {
            shifts.ease(tag.name, moved)
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
