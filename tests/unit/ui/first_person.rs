//! Tests for first-person mode, `src/ui/first_person.rs`.

use super::*;
use crate::graphics::table::{TOP_HALF_DEPTH, TOP_HALF_LENGTH, top_height};
use crate::sim::geometry::DEFAULT_GEOMETRY;

/// The figure starts on the table, outside the block, its eyes about as
/// high as the engine, looking at the engine.
#[test]
fn the_figure_starts_on_the_table_facing_the_engine() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let walker = Walker::standing(shape);
    let eye = walker.eye;
    assert!(eye.x.abs() < TOP_HALF_LENGTH * shape.bore);
    assert!(eye.z.abs() < TOP_HALF_DEPTH * shape.bore);
    assert!(eye.z > shape.outer + shape.bore, "in front of the block");
    assert!(eye.y > top_height(shape) + shape.bore);
    let toward = (vec3(0.0, eye.y, 0.0) - eye).normalize();
    let level = vec3(walker.forward().x, 0.0, walker.forward().z).normalize();
    assert!(level.dot(toward) > 0.99, "it faces the engine");
    assert!(walker.forward().y < 0.0, "looking a little down");
    assert!((walker.forward().length() - 1.0).abs() < 1e-5);
}

/// The mouse turns the head from where it was first read, and the head
/// never tips past straight up or down.
#[test]
fn the_mouse_turns_the_head() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let mut walker = Walker::standing(shape);
    let start = walker;
    // Still at first, then the capture's jumps: none of it turns the head.
    walker.look(vec2(500.0, 500.0), 1.0);
    walker.look(vec2(500.0, 500.0), 1.0);
    walker.look(vec2(-3000.0, -2000.0), 1.0);
    walker.look(vec2(720.0, 450.0), 1.0);
    assert_eq!((walker.yaw, walker.pitch), (start.yaw, start.pitch));
    walker.look(vec2(820.0, 450.0), 1.0);
    assert!(walker.yaw < start.yaw, "right turns right");
    let before = walker.yaw;
    walker.look(vec2(-100_000.0, 450.0), 1.0);
    let most = MAX_LOOK_STEP * LOOK_SENSITIVITY;
    assert!(
        (walker.yaw - before - most).abs() < 1e-4,
        "a stray jump is held back"
    );
    for step in 1..=100 {
        walker.look(vec2(-100_000.0, -100.0 * step as f32), 1.0);
    }
    assert!(walker.pitch < FRAC_PI_2 && walker.pitch > 1.0);
    walker.release();
    let held = walker;
    walker.look(vec2(0.0, 0.0), 1.0);
    walker.look(vec2(-5000.0, -5000.0), 1.0);
    assert_eq!(walker.yaw, held.yaw, "a fresh capture's jump is ignored");
    let camera = walker.camera();
    assert_eq!(camera.position, walker.eye);
}

/// Both hints can be drawn in the app's font.
#[test]
fn the_hints_can_be_drawn() {
    assert!(crate::ui::fonts::tests::drawable(HINT));
    assert!(crate::ui::fonts::tests::drawable(WAITING_HINT));
}

/// The table under the default engine, the cartridge seated.
fn table_map(shape: BlockShape) -> WalkMap {
    use crate::sim::port::Port;
    WalkMap::new(shape, Port::new(DEFAULT_GEOMETRY), None)
}

/// The keys held to walk toward `way` and nothing else.
fn walking(way: Vec2) -> FirstPersonKeys {
    FirstPersonKeys {
        way,
        ..Default::default()
    }
}

/// A frame's seconds at 60 frames a second.
const FRAME: f32 = 1.0 / 60.0;

/// The figure is as tall as the cylinders: its eyes are below their top.
#[test]
fn the_figure_is_as_tall_as_the_cylinders() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let walker = Walker::standing(shape);
    assert!(walker.eye.y < shape.top);
    assert!(walker.eye.y > shape.base + 0.7 * (shape.top - shape.base));
}

/// Held ahead, the keys walk it the way it looks at full speed within a
/// tenth of a second or so; let go, it stops within a few millimeters.
#[test]
fn it_walks_ahead_and_stops_short() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let map = table_map(shape);
    let mut walker = Walker::standing(shape);
    walker.yaw = 0.0;
    walker.pitch = 0.0;
    let start = walker.feet();
    for _ in 0..12 {
        walker.walk(walking(vec2(0.0, 1.0)), FRAME, &map);
    }
    let speed = walker.velocity.length() / shape.bore;
    assert!((speed - WALK_SPEED).abs() < 1e-3, "{speed}");
    let walked = walker.feet() - start;
    assert!(
        walked.z < 0.0 && walked.x.abs() < 1e-5,
        "ahead is -z at yaw 0"
    );
    let stopping = walker.feet();
    for _ in 0..30 {
        walker.walk(walking(Vec2::ZERO), FRAME, &map);
    }
    assert_eq!(walker.velocity, Vec3::ZERO);
    let slid = walker.feet().distance(stopping) / shape.bore;
    assert!(slid < 0.4, "stopped within {slid} bore radii");
}

/// Two keys together walk it no faster than one.
#[test]
fn the_diagonal_is_no_faster() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let map = table_map(shape);
    let mut walker = Walker::standing(shape);
    walker.yaw = 0.3;
    for _ in 0..30 {
        walker.walk(walking(vec2(1.0, 1.0)), FRAME, &map);
    }
    let speed = walker.velocity.length() / shape.bore;
    assert!((speed - WALK_SPEED).abs() < 1e-3, "{speed}");
}

/// Walking into the block, it is held clear of it and slides along it
/// rather than stopping dead.
#[test]
fn it_slides_along_what_it_walks_into() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let map = table_map(shape);
    let mut walker = Walker::standing(shape);
    // Straight at the back of the block, a little to its side.
    walker.feet = vec3(0.5 * shape.bore, shape.base, -4.0 * shape.bore);
    walker.yaw = std::f32::consts::PI - 0.4;
    let radius = BODY_RADIUS * shape.bore;
    let mut last = walker.feet();
    for _ in 0..240 {
        walker.walk(walking(vec2(0.0, 1.0)), FRAME, &map);
        let at = vec2(walker.feet().x, walker.feet().z);
        assert!(map.clearance(at, radius, shape.base) > -0.01 * shape.bore);
        last = walker.feet();
    }
    assert!(
        last.x.abs() > shape.offset,
        "it slid round toward the block's end"
    );
}

/// Off the edge it falls, and once far enough below the table is to be
/// put back.
#[test]
fn walking_off_the_edge_falls() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let map = table_map(shape);
    let mut walker = Walker::standing(shape);
    walker.yaw = std::f32::consts::PI;
    let mut fell = false;
    for _ in 0..(10.0 / FRAME) as usize {
        if walker.walk(walking(vec2(0.0, 1.0)), FRAME, &map) == Walked::Fell {
            fell = true;
            break;
        }
    }
    assert!(fell);
    assert!(walker.airborne());
    assert!(walker.feet().y < shape.base - FALL_DEPTH * shape.bore);
}

/// The head bobs while walking, and is still while standing.
#[test]
fn the_head_bobs_only_while_walking() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let map = table_map(shape);
    let mut walker = Walker::standing(shape);
    let standing = walker.eye.y - walker.feet().y;
    walker.yaw = 0.0;
    let mut lowest = standing;
    for _ in 0..30 {
        walker.walk(walking(vec2(0.0, 1.0)), FRAME, &map);
        lowest = lowest.min(walker.eye.y - walker.feet().y);
    }
    assert!(lowest < standing - 0.5 * BOB_DEPTH * shape.bore);
    for _ in 0..30 {
        walker.walk(walking(Vec2::ZERO), FRAME, &map);
    }
    assert!((walker.eye.y - walker.feet().y - standing).abs() < 1e-5);
}

/// Runs `frames` frames of `keys` on `map`, and returns the highest the
/// feet got.
fn run(walker: &mut Walker, keys: FirstPersonKeys, frames: usize, map: &WalkMap) -> f32 {
    let mut highest = walker.feet().y;
    for _ in 0..frames {
        walker.walk(keys, FRAME, map);
        highest = highest.max(walker.feet().y);
    }
    highest
}

/// Space jumps about `JUMP_HEIGHT` and floats down again onto the table.
#[test]
fn it_jumps_and_lands() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let map = table_map(shape);
    let mut walker = Walker::standing(shape);
    let jump = FirstPersonKeys {
        jump: true,
        ..Default::default()
    };
    walker.walk(jump, FRAME, &map);
    assert!(walker.airborne());
    let highest = run(&mut walker, FirstPersonKeys::default(), 120, &map);
    let height = (highest - shape.base) / shape.bore;
    assert!((height - JUMP_HEIGHT).abs() < 0.05, "{height}");
    assert!(!walker.airborne());
    assert_eq!(walker.feet().y, shape.base);
    // Holding Space down does not bounce it again.
    let held = run(&mut walker, FirstPersonKeys::default(), 30, &map);
    assert_eq!(held, shape.base);
}

/// A jump pressed just before landing happens on landing, and one pressed
/// a moment after walking off the table's edge still counts.
#[test]
fn a_jump_a_moment_early_or_late_still_counts() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let map = table_map(shape);
    let mut walker = Walker::standing(shape);
    let jump = FirstPersonKeys {
        jump: true,
        ..Default::default()
    };
    walker.walk(jump, FRAME, &map);
    // On the way down, a couple of frames above the table.
    let mut frames = 0;
    while walker.velocity.y >= 0.0 || walker.feet().y > shape.base + 0.4 * shape.bore {
        walker.walk(FirstPersonKeys::default(), FRAME, &map);
        frames += 1;
        assert!(frames < 120 && walker.airborne(), "missed the moment");
    }
    walker.walk(jump, FRAME, &map);
    run(&mut walker, FirstPersonKeys::default(), 4, &map);
    assert!(
        walker.airborne() && walker.velocity.y > 0.0,
        "jumped on landing"
    );
    // Off the edge: walk off, then jump two frames later.
    let mut walker = Walker::standing(shape);
    walker.yaw = std::f32::consts::PI;
    while !walker.airborne() {
        walker.walk(walking(vec2(0.0, 1.0)), FRAME, &map);
    }
    walker.walk(walking(vec2(0.0, 1.0)), FRAME, &map);
    walker.walk(jump, FRAME, &map);
    assert!(walker.velocity.y > 0.0, "the late jump counted");
}

/// Ctrl lowers the eyes and slows the walk.
#[test]
fn crouching_lowers_the_eyes_and_slows_the_walk() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let map = table_map(shape);
    let mut walker = Walker::standing(shape);
    let standing = walker.eye.y;
    // Along the front of the table, clear of everything.
    walker.yaw = std::f32::consts::FRAC_PI_2;
    let crouched = FirstPersonKeys {
        way: vec2(0.0, 1.0),
        crouch: true,
        ..Default::default()
    };
    run(&mut walker, crouched, 60, &map);
    let speed = walker.velocity.length() / shape.bore;
    assert!((speed - CROUCH_SPEED * WALK_SPEED).abs() < 1e-3, "{speed}");
    let eyes = walker.eye.y - walker.feet().y;
    assert!(eyes < 0.7 * (standing - shape.base), "{eyes}");
    run(&mut walker, FirstPersonKeys::default(), 60, &map);
    assert!(
        (walker.eye.y - standing).abs() < 1e-3 * shape.bore,
        "stood up again"
    );
}

/// A jump onto a button box lands on its top, and a crouched jump tucks
/// the feet up above where a plain one reaches.
#[test]
fn it_can_jump_onto_a_button_box() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let map = table_map(shape);
    let button = crate::graphics::buttons::button_boxes(shape)[1];
    let mut walker = Walker::standing(shape);
    // Standing back from the box, facing it.
    let toward = (button.base - walker.feet()).normalize();
    walker.feet = button.base - toward * 1.9 * shape.bore;
    walker.feet.y = shape.base;
    walker.yaw = (-toward.x).atan2(-toward.z);
    let jump = FirstPersonKeys {
        way: vec2(0.0, 1.0),
        jump: true,
        ..Default::default()
    };
    // Walking at it, it jumps short of the box, and lets go of the keys
    // at the top of the jump to come down on it.
    walker.walk(jump, FRAME, &map);
    for _ in 0..120 {
        if walker.velocity.y <= 0.0 {
            break;
        }
        walker.walk(walking(vec2(0.0, 1.0)), FRAME, &map);
    }
    for _ in 0..120 {
        if !walker.airborne() {
            break;
        }
        walker.walk(FirstPersonKeys::default(), FRAME, &map);
    }
    run(&mut walker, FirstPersonKeys::default(), 30, &map);
    assert!(!walker.airborne());
    assert!(
        (walker.feet().y - button.dome_top().y).abs() < 1e-4,
        "on the box"
    );
    // A crouched jump from the table reaches higher.
    let mut plain = Walker::standing(shape);
    plain.walk(jump, FRAME, &map);
    let plain_top = run(&mut plain, FirstPersonKeys::default(), 60, &map);
    let mut tucked = Walker::standing(shape);
    let crouched = FirstPersonKeys {
        crouch: true,
        ..Default::default()
    };
    tucked.walk(
        FirstPersonKeys {
            jump: true,
            ..crouched
        },
        FRAME,
        &map,
    );
    let tucked_top = run(&mut tucked, crouched, 60, &map);
    assert!(tucked_top > plain_top + 0.5 * TUCK * shape.bore);
}

/// Through a crouched jump, tucking up, landing and standing again, the
/// view moves smoothly: its speed never changes in a frame by more than
/// landing changes it, as a sudden jump of the feet up or down would.
#[test]
fn a_crouched_jump_never_jolts_the_view() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let map = table_map(shape);
    let mut walker = Walker::standing(shape);
    // Landing stops a fall from the top of a tucked jump in one frame; on
    // top of that, only the crouch's own easing.
    let fall = (2.0 * FALL_PULL * GRAVITY * (JUMP_HEIGHT + TUCK)).sqrt() * FRAME;
    let most = (fall + 0.1) * shape.bore;
    let mut heights = vec![walker.eye.y];
    let mut play = |walker: &mut Walker, keys: FirstPersonKeys, frames: usize| {
        for _ in 0..frames {
            walker.walk(keys, FRAME, &map);
            heights.push(walker.eye.y);
        }
    };
    let jump = FirstPersonKeys {
        jump: true,
        ..Default::default()
    };
    let crouch = FirstPersonKeys {
        crouch: true,
        ..Default::default()
    };
    play(&mut walker, jump, 1);
    play(&mut walker, FirstPersonKeys::default(), 6);
    // Tucking up in the air, then letting go again before landing.
    play(&mut walker, crouch, 8);
    play(&mut walker, FirstPersonKeys::default(), 4);
    play(&mut walker, crouch, 40);
    play(&mut walker, FirstPersonKeys::default(), 30);
    assert!(!walker.airborne());
    for (frame, three) in heights.windows(3).enumerate() {
        let jolt = (three[2] - 2.0 * three[1] + three[0]).abs();
        assert!(jolt < most, "frame {frame}: {}", jolt / shape.bore);
    }
}

/// Stepping up onto the cartridge glides the view up rather than jolting
/// it.
#[test]
fn stepping_up_glides_the_view() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let map = WalkMap::new(
        shape,
        crate::sim::port::Port::new(DEFAULT_GEOMETRY),
        Some(crate::graphics::cartridge::rest(shape)),
    );
    let rest = crate::graphics::cartridge::rest(shape);
    let mut walker = Walker::standing(shape);
    let toward = (rest - walker.feet()).with_y(0.0).normalize();
    walker.feet = vec3(rest.x, shape.base, rest.z) - toward * 2.0 * shape.bore;
    walker.yaw = (-toward.x).atan2(-toward.z);
    let mut biggest: f32 = 0.0;
    let mut stood_up = false;
    for _ in 0..40 {
        let before = walker.eye.y;
        walker.walk(walking(vec2(0.0, 1.0)), FRAME, &map);
        biggest = biggest.max((walker.eye.y - before).abs());
        stood_up |= walker.feet().y > shape.base;
    }
    assert!(stood_up, "it stepped up onto the cartridge");
    let step = map
        .ground(vec2(rest.x, rest.z), 0.0, shape.base + 1.0)
        .unwrap()
        - shape.base;
    assert!(biggest < 0.6 * step, "{biggest} against a step of {step}");
}

/// A crouched jump reaches the top of the desk clock with room to spare,
/// and a plain one does not.
#[test]
fn a_crouched_jump_clears_the_clock() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let map = table_map(shape);
    let clock = crate::graphics::clock::clock_place(shape).height();
    let reach = |crouch: bool| {
        let mut walker = Walker::standing(shape);
        let keys = FirstPersonKeys {
            crouch,
            ..Default::default()
        };
        walker.walk(FirstPersonKeys { jump: true, ..keys }, FRAME, &map);
        run(&mut walker, keys, 60, &map) - shape.base + STEP_UP * shape.bore
    };
    assert!(reach(true) > clock + 0.3 * shape.bore, "crouched");
    assert!(reach(false) < clock, "plain");
}

/// Gripping something, the mouse pushes the view against a spring past
/// `GRIP_LIMIT`, never more than `GRIP_GIVE` further however far it
/// moves; let go, the view springs back to where it was.
#[test]
fn a_grip_holds_the_view_near_where_it_was() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let map = table_map(shape);
    let mut walker = Walker::standing(shape);
    let (yaw, pitch) = (walker.yaw, walker.pitch);
    walker.look(vec2(100.0, 100.0), 1.0);
    walker.settling = 0;
    walker.grip();
    assert!(walker.gripping());
    let mut x = 100.0;
    for _ in 0..30 {
        x += 100.0;
        let moved = walker.look(vec2(x, 100.0), 1.0);
        assert!(moved.x > 0.0, "the mouse's move is told");
        walker.walk(FirstPersonKeys::default(), FRAME, &map);
        assert!((walker.yaw - yaw).abs() <= GRIP_LIMIT + GRIP_GIVE + 1e-5);
    }
    assert!(
        (walker.yaw - yaw).abs() > GRIP_LIMIT,
        "pushed against the spring"
    );
    for _ in 0..120 {
        walker.walk(FirstPersonKeys::default(), FRAME, &map);
    }
    assert!(
        ((walker.yaw - yaw).abs() - GRIP_LIMIT).abs() < 1e-3,
        "back to the limit, still held there"
    );
    walker.let_go();
    assert!(!walker.gripping());
    for _ in 0..120 {
        walker.walk(FirstPersonKeys::default(), FRAME, &map);
    }
    assert_eq!(
        (walker.yaw, walker.pitch),
        (yaw, pitch),
        "back where it was"
    );
}

/// Gripping something, the view moved within `GRIP_LIMIT` stays where the
/// mouse leaves it rather than drifting back.
#[test]
fn a_grip_keeps_the_view_where_it_is_left_within_the_limit() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let map = table_map(shape);
    let mut walker = Walker::standing(shape);
    let yaw = walker.yaw;
    walker.look(vec2(100.0, 100.0), 1.0);
    walker.settling = 0;
    walker.grip();
    // Half the limit's worth of mouse.
    let points = 0.5 * GRIP_LIMIT / LOOK_SENSITIVITY;
    walker.look(vec2(100.0 + points, 100.0), 1.0);
    walker.walk(FirstPersonKeys::default(), FRAME, &map);
    let pushed = walker.yaw - yaw;
    assert!((pushed.abs() - 0.5 * GRIP_LIMIT).abs() < 1e-3);
    for _ in 0..120 {
        walker.walk(FirstPersonKeys::default(), FRAME, &map);
    }
    assert!((walker.yaw - yaw - pushed).abs() < 1e-5, "still there");
}

/// Gripping something, the view leans on the edge of its room only once
/// pushed to it, the way the mouse went, and still does once the push
/// past the edge has sprung back.
#[test]
fn a_grip_leans_on_its_edge_the_way_the_mouse_went() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let map = table_map(shape);
    let mut walker = Walker::standing(shape);
    walker.look(vec2(100.0, 100.0), 1.0);
    walker.settling = 0;
    walker.grip();
    let points = 0.5 * GRIP_LIMIT / LOOK_SENSITIVITY;
    walker.look(vec2(100.0 + points, 100.0), 1.0);
    assert_eq!(walker.grip_lean(), 0.0, "short of the edge");
    // Well past it, to the right.
    walker.look(vec2(100.0 + 4.0 * points, 100.0), 1.0);
    assert!(walker.grip_lean() > 1.0, "pushed into the give");
    for _ in 0..120 {
        walker.walk(FirstPersonKeys::default(), FRAME, &map);
    }
    assert!((walker.grip_lean() - 1.0).abs() < 1e-3, "held at the edge");
    walker.let_go();
    assert_eq!(walker.grip_lean(), 0.0, "let go");
}

/// Let go, the view turns with the mouse at once while it springs back.
#[test]
fn a_view_let_go_turns_with_the_mouse_at_once() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let map = table_map(shape);
    let mut walker = Walker::standing(shape);
    let yaw = walker.yaw;
    walker.look(vec2(100.0, 100.0), 1.0);
    walker.settling = 0;
    walker.grip();
    let points = 0.5 * GRIP_LIMIT / LOOK_SENSITIVITY;
    walker.look(vec2(100.0 + points, 100.0), 1.0);
    walker.walk(FirstPersonKeys::default(), FRAME, &map);
    walker.let_go();
    // The same again the other way, just after letting go.
    walker.look(vec2(100.0, 100.0), 1.0);
    for _ in 0..30 {
        walker.walk(FirstPersonKeys::default(), FRAME, &map);
    }
    let turned = walker.yaw - yaw;
    assert!(
        (turned - 0.5 * GRIP_LIMIT).abs() < 1e-3,
        "turned by the mouse, not lost to the spring: {turned}"
    );
    assert!(!walker.gripping());
}

/// The drawers pulled out as a staircase, each a bore radius further out
/// than the one above, the top one a little past the table's edge.
fn staircase(shape: BlockShape) -> [f32; crate::graphics::drawers::DRAWER_COUNT] {
    use crate::graphics::drawers::DRAWER_COUNT;
    std::array::from_fn(|index| (1.2 + (DRAWER_COUNT - 1 - index) as f32) * shape.bore)
}

/// Holding forward and jump, a figure on the ground in front of the
/// drawers pulled out as a staircase climbs them, drawer by drawer, onto
/// the table's top; and walking off the top over them, it comes down
/// them to the ground, never falling into the sky.
#[test]
fn the_drawers_are_climbed_up_and_down() {
    use crate::graphics::drawers::drawer_places;
    use crate::sim::port::Port;
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let unit = shape.bore;
    let pulls = staircase(shape);
    let map = WalkMap::new(shape, Port::new(DEFAULT_GEOMETRY), None).with_drawers(shape, pulls);
    let places = drawer_places(shape);
    let middle_x = (places[0].left + places[0].right) / 2.0;
    let bottom_front = places[0].front + pulls[0];
    let mut walker = Walker::standing(shape);
    walker.feet = vec3(middle_x, map.ground, bottom_front + unit);
    assert!(
        map.over_ground(vec2(walker.feet.x, walker.feet.z)),
        "starts on the ground"
    );
    walker.yaw = 0.0;
    walker.settling = 0;
    let climbing = FirstPersonKeys {
        way: vec2(0.0, 1.0),
        jump: true,
        ..Default::default()
    };
    let mut highest = walker.feet.y;
    let on_top = |walker: &Walker| {
        (walker.feet.y - shape.base).abs() < 1e-3 && walker.feet.z < places[0].front - unit
    };
    for _ in 0..600 {
        assert_eq!(walker.walk(climbing, FRAME, &map), Walked::OnFoot);
        highest = highest.max(walker.feet.y);
        if on_top(&walker) {
            break;
        }
    }
    assert!(
        on_top(&walker),
        "on the table's top, at {} after reaching {highest}",
        walker.feet / unit
    );
    // Back down, facing out over the drawers.
    walker.yaw = PI;
    walker.feet.x = middle_x;
    for _ in 0..600 {
        assert_eq!(
            walker.walk(walking(vec2(0.0, 1.0)), FRAME, &map),
            Walked::OnFoot
        );
        if walker.feet.z > bottom_front + unit {
            break;
        }
    }
    for _ in 0..120 {
        walker.walk(FirstPersonKeys::default(), FRAME, &map);
    }
    assert!(
        (walker.feet.y - map.ground).abs() < 1e-3,
        "down on the ground, not at {}",
        walker.feet / unit
    );
}

/// On the ground the figure walks into the knee space under the desk's
/// top and pencil drawer, but not through the end panel or the rail, and
/// never up onto the top.
#[test]
fn under_the_table_the_top_is_overhead_and_the_panel_in_the_way() {
    use crate::graphics::table::{
        back_rail,
        end_panel, //
    };
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let map = table_map(shape);
    let radius = BODY_RADIUS * shape.bore;
    let under = vec2(0.0, 0.0);
    assert_eq!(map.ground(under, radius, map.ground), Some(map.ground));
    assert!(
        map.clearance(under, radius, map.ground) > 0.0,
        "the top is overhead"
    );
    for board in [end_panel(shape), back_rail(shape)] {
        let middle = (board.low + board.high) / 2.0;
        assert!(
            map.clearance(vec2(middle.x, middle.z), radius, map.ground) < 0.0,
            "in the way"
        );
    }
    // From the top, the table is underfoot.
    assert_eq!(
        map.ground(under + vec2(0.0, 5.0 * shape.bore), radius, shape.base),
        Some(shape.base)
    );
}

/// Jumping under the table, the head stops at the top's underside rather
/// than passing through it, and the figure comes back down to the ground.
#[test]
fn a_jump_under_the_table_stops_at_its_underside() {
    let shape = BlockShape::new(DEFAULT_GEOMETRY);
    let map = table_map(shape);
    let radius = BODY_RADIUS * shape.bore;
    let under = vec2(0.0, 0.0);
    let most = map
        .head_room(under, radius, map.ground)
        .expect("the top is overhead");
    let mut walker = Walker::standing(shape);
    walker.feet = vec3(under.x, map.ground, under.y);
    walker.settling = 0;
    let jumping = FirstPersonKeys {
        jump: true,
        ..Default::default()
    };
    let mut highest = walker.feet.y;
    for _ in 0..60 {
        assert_eq!(walker.walk(jumping, FRAME, &map), Walked::OnFoot);
        highest = highest.max(walker.feet.y);
        assert_eq!(
            vec2(walker.feet.x, walker.feet.z),
            under,
            "not pushed aside"
        );
    }
    assert!(highest > map.ground, "it jumped");
    assert!(
        highest <= most + 1e-4,
        "the head stays under the top: {highest} against {most}"
    );
    for _ in 0..120 {
        walker.walk(FirstPersonKeys::default(), FRAME, &map);
    }
    assert!(
        (walker.feet.y - map.ground).abs() < 1e-3,
        "back on the ground"
    );
}
