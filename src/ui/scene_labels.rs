//! Labels on the scene: tags beside the hot and cold cylinders, the
//! opening, the flywheel and the gas, each with its part in the system.
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
    WALL_THICKNESS, //
};
use crate::sim::engine::ConnectedChambers;
use crate::ui::controls::{
    BACKING,
    BACKING_RADIUS,
    SceneArea,
    TOGGLES_BOTTOM,
    row_top, //
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

/// The tags for the engine of `engine`, the system's first: where two
/// would overlap, the later one gives way.
pub fn tags(engine: &ConnectedChambers) -> [Tag; 5] {
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
    [
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
            name: "Opening",
            role: "insulated: no heat",
            anchor: vec3(0.0, geometry.passage_center_y(), 0.0),
            outward: under_opening,
        },
    ]
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

/// `rect` moved up clear of every box in `taken`, or `None` if that would
/// lift its top above `top`.
fn clear_of(mut rect: Rect, taken: &[Rect], top: f32) -> Option<Rect> {
    // Each move lifts the box above one in its way, so there are at most
    // as many moves as boxes.
    for _ in 0..=taken.len() {
        let Some(other) = taken
            .iter()
            .find(|other| grown(**other, TAG_GAP).overlaps(&rect))
        else {
            return Some(rect);
        };
        rect.y = other.y - TAG_GAP - rect.h;
        if rect.y < top {
            return None;
        }
    }
    None
}

/// Where each tag goes, for `tags` seen through `matrix` in `area` clear of
/// `cards`: its dot and its box, or `None` for a part behind the camera or
/// a tag with no room left.
///
/// The boxes stay between the panel and the scene's span's right, and
/// between the toggles and the bottom row. A box in the way of one of
/// `cards` or of an earlier tag moves up past it.
pub fn place(
    pen: &Pen,
    tags: &[Tag],
    matrix: Mat4,
    area: SceneArea,
    cards: &[Rect],
) -> Vec<Option<(Vec2, Rect)>> {
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
            let high = vec2(area.right(), floor) - half - EDGE_ROOM;
            let center = center.clamp(low, high.max(low));
            let rect = Rect::new(center.x - half.x, center.y - half.y, width, height);
            let rect = clear_of(rect, &taken, low.y - half.y)?;
            taken.push(rect);
            Some((dot, rect))
        })
        .collect()
}

/// Draws the tags of `engine` seen through `matrix` in `area`, clear of
/// `cards`.
pub fn draw(
    pen: &mut Pen,
    engine: &ConnectedChambers,
    matrix: Mat4,
    area: SceneArea,
    cards: &[Rect],
) {
    let tags = tags(engine);
    for (tag, placed) in tags.iter().zip(place(pen, &tags, matrix, area, cards)) {
        let Some((dot, rect)) = placed else {
            continue;
        };
        let end = vec2(
            dot.x.clamp(rect.x, rect.right()),
            dot.y.clamp(rect.y, rect.bottom()),
        );
        pen.line(dot, end, LEADER_WIDTH, LEADER_COLOR);
        pen.circle(dot, DOT_RADIUS, LEADER_COLOR);
        pen.round_rect(rect, BACKING_RADIUS, BACKING);
        let left = rect.x + PADDING;
        let name_y = rect.y + PADDING + pen.text_height("H", NAME_SIZE);
        pen.strong(tag.name, left, name_y, NAME_SIZE, TEXT);
        pen.text(tag.role, left, name_y + LINE, ROLE_SIZE, QUIET);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/scene_labels.rs"]
mod tests;
