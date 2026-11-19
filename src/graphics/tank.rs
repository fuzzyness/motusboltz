//! The helium tank: a small gas bottle standing on the table beyond the
//! cold cylinder, with a knob on its valve, a regulator with a pressure
//! gauge, and a pipe from the regulator to the port low on the cold
//! cylinder's wall (see `sim::port`).
//!
//! It is drawn as a toy, like the rest of the scene: a deep satin silver
//! capsule with chunky red bumper rings trimmed in chrome, a chrome valve
//! with a big red knob, and a gauge in a fat chrome bezel. Turning the
//! knob pumps gas in or out (see `ui::handles::knob_concentration`); the
//! gauge's needle shows how much the bottle holds, falling across most of
//! the dial as the pump fills the engine and rising as it takes gas back.
//! While gas moves, a faint glow shows at the port inside the cylinder.
//! Drawing only.

use std::f32::consts::{
    FRAC_PI_2,
    PI,
    TAU, //
};

use macroquad::models::Mesh;
use macroquad::prelude::{
    Camera3D,
    Color,
    Vec3,
    vec3, //
};

use super::block::{
    BlockShape,
    key_light, //
};
use super::gpu::{
    GpuMaterial,
    GpuMesh, //
};
use super::solid::{
    Frame,
    Solid,
    Turn,
    finish,
    load_metal,
    turn, //
};
use super::sprites::{
    Facing,
    SpriteMaterials,
    Sprites,
    with_alpha, //
};
use super::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY,
    draw_lit, //
};
use crate::sim::port::{
    FLOW_PER_SECOND,
    Port, //
};
use crate::sim::units::{
    MAX_PARTICLES,
    MIN_PARTICLES, //
};

/// Gap between the block's side and the bottle, along the port's line, in
/// bore radii.
const TANK_GAP: f32 = 1.0;
/// Radius of the bottle, in bore radii.
const TANK_RADIUS: f32 = 0.55;
/// Height of the bottle's side, from the table to the shoulder, in bore
/// radii.
const BODY_HEIGHT: f32 = 3.0;
/// Radius of the rounding where the bottle meets the table, in bore radii.
const FOOT_ROUND: f32 = 0.14;
/// How far the bumper rings stand out from the side, in bore radii.
const BUMPER_OUT: f32 = 0.07;
/// Height of each bumper ring, in bore radii.
const BUMPER_HEIGHT: f32 = 0.32;
/// Where the bumper rings start up the side, in bore radii.
const BUMPER_HEIGHTS: [f32; 2] = [0.22, 2.45];
/// Height of the chrome trim bands below and above each bumper ring, in
/// bore radii.
const TRIM_HEIGHT: f32 = 0.06;
/// How far the trim bands stand out from the side, in bore radii.
const TRIM_OUT: f32 = 0.015;
/// Height of the rounded shoulder above the side, in bore radii.
const SHOULDER_HEIGHT: f32 = 0.5;
/// Radius of the neck, in bore radii.
const NECK_RADIUS: f32 = 0.16;
/// Height of the neck, in bore radii.
const NECK_HEIGHT: f32 = 0.12;
/// Radius of the valve's body, in bore radii.
const VALVE_RADIUS: f32 = 0.15;
/// Height of the valve's body, in bore radii.
const VALVE_HEIGHT: f32 = 0.3;
/// Radius of the knob's rim, in bore radii: big enough to grab.
pub const KNOB_RADIUS: f32 = 0.24;
/// Half the thickness of the knob's rim, in bore radii.
const KNOB_RIM_HALF: f32 = 0.045;
/// Radius of the knob's hub, in bore radii.
const KNOB_HUB_RADIUS: f32 = 0.08;
/// Height of the knob's hub, in bore radii.
const KNOB_HUB_HEIGHT: f32 = 0.11;
/// Spokes in the knob, so its turn shows.
const KNOB_SPOKES: usize = 4;
/// Half the width of a spoke, in bore radii.
const KNOB_SPOKE_HALF: f32 = 0.03;
/// How far the knob's middle stands above the valve's top, in bore radii.
const KNOB_RISE: f32 = 0.06;
/// Length of the regulator's body, out from the valve toward the engine,
/// in bore radii.
const REGULATOR_LENGTH: f32 = 0.8;
/// Radius of the regulator's body, in bore radii.
const REGULATOR_RADIUS: f32 = 0.1;
/// Length of the chrome cap on the regulator's end, in bore radii.
const REGULATOR_CAP: f32 = 0.08;
/// How much wider than the regulator its caps are, in bore radii.
const REGULATOR_CAP_OUT: f32 = 0.025;
/// Radius of the gauge's dial, in bore radii.
const DIAL_RADIUS: f32 = 0.3;
/// Thickness of the gauge's case, in bore radii.
const DIAL_DEPTH: f32 = 0.08;
/// Half the thickness of the gauge's bezel ring, in bore radii.
const BEZEL_HALF: f32 = 0.045;
/// How far the gauge stands above the regulator's axis, in bore radii.
const DIAL_RISE: f32 = 0.36;
/// The way the gauge's face looks, roughly toward the usual view.
const DIAL_FACING: Vec3 = vec3(0.55, 0.25, 1.0);
/// Marks round the dial, from empty to full.
const DIAL_TICKS: usize = 9;
/// Where the marks start and end, as shares of the dial's radius.
const TICK_SPAN: (f32, f32) = (0.68, 0.84);
/// Half the width of a mark, in bore radii.
const TICK_HALF: f32 = 0.014;
/// Length of the needle, as a share of the dial's radius.
const NEEDLE_REACH: f32 = 0.78;
/// Half the width of the needle at its pivot, in bore radii.
const NEEDLE_HALF: f32 = 0.02;
/// Radius of the cap over the needle's pivot, in bore radii.
const PIVOT_RADIUS: f32 = 0.04;
/// Sweep of the needle from empty to full, in radians, centered on
/// straight up.
const NEEDLE_SWEEP: f32 = 4.7;
/// Share of the dial the needle crosses as the engine goes from its least
/// fill to its most: most of it, so every pump shows.
const NEEDLE_SWING: f32 = 0.85;
/// Radius of the pipe, in bore radii.
const PIPE_RADIUS: f32 = 0.05;
/// Radius of the flange where the pipe meets the block, in bore radii.
const FLANGE_RADIUS: f32 = 0.14;
/// Thickness of the flange, in bore radii.
const FLANGE_THICKNESS: f32 = 0.06;
/// How much the ring round the port inside the cylinder overlaps it, in
/// bore radii.
const RIM_WIDTH: f32 = 0.035;
/// Segments round turned parts.
const SEGMENTS: usize = 32;
/// Steps round each rounded edge of the bottle.
const ROUND_STEPS: usize = 6;
/// Steps round a ball at a bend in the pipe.
const BALL_STEPS: usize = 8;
/// How far the dial's face and what lies on it stand off the part behind,
/// in bore radii, so neither fights the other for depth.
const STANDOFF: f32 = 0.006;
/// Largest glow at the port, in bore radii.
const PUFF_SIZE: f32 = 0.5;
/// Opacity of the glow at full flow.
const PUFF_OPACITY: f32 = 0.55;
/// The glow's color.
const PUFF_COLOR: Color = Color::new(0.85, 0.93, 1.0, 1.0);

/// The bottle, its shoulder, and the regulator: a deep silver with a
/// satin finish, so the mirror-bright chrome trims stand out against it.
const TANK_PAINT: Color = finish(130, 136, 148, 0.6);
/// The bumper rings, red to match the knob.
const BUMPER_PAINT: Color = finish(226, 38, 50, 0.85);
/// The knob and the needle.
const TOY_RED: Color = finish(234, 34, 48, 0.8);
/// The neck, the trims, the valve, the regulator's cap, the bezel, the
/// pipe, the flange and the knob's hub.
const CHROME: Color = finish(214, 224, 240, 0.95);
/// The dial's face.
const DIAL_FACE: Color = finish(250, 250, 246, 0.3);
/// Marks for a half-empty bottle, then a nearly empty one; a fuller one
/// has none.
const TICK_COLORS: [Color; 2] = [finish(255, 196, 30, 0.4), finish(230, 40, 50, 0.4)];
/// Share of the dial, from empty, that carries marks.
const MARKED_SHARE: f32 = 2.0 / 3.0;
/// How far along the regulator, out from the valve, the gauge stands, in
/// bore radii: clear of the knob's rim.
const DIAL_ALONG: f32 = 0.72;

/// Where the bottle, its knob and its gauge are, for tags, for grabbing
/// the knob and for finding them under the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TankPlace {
    /// The middle of the bottle's foot on the table.
    pub foot: Vec3,
    /// Radius of the bottle.
    pub radius: f32,
    /// Height of the bottle, valve included.
    pub height: f32,
    /// The middle of the gauge's face.
    pub gauge: Vec3,
    /// The middle of the knob, which turns about the upright.
    pub knob: Vec3,
}

/// The parts' places for a block of `shape` with the port `port`.
struct Layout {
    /// Size of a bore radius.
    unit: f32,
    /// The middle of the bottle's foot.
    foot: Vec3,
    /// The way from the bottle toward the engine, level.
    inward: Vec3,
    /// Top of the valve, where the knob sits.
    valve_top: f32,
    /// The regulator's outlet, where the pipe starts.
    outlet: Vec3,
    /// The middle of the gauge's face, and the way it looks.
    dial: (Vec3, Vec3),
    /// The middle of the knob.
    knob: Vec3,
    /// Where the pipe meets the block's outside.
    flange: Vec3,
}

impl Layout {
    /// The layout for a block of `shape` with the port `port`.
    fn new(shape: BlockShape, port: Port) -> Self {
        let unit = shape.bore;
        let center = port.at(shape.outer + (TANK_GAP + TANK_RADIUS) * unit);
        let foot = vec3(center.x, shape.base, center.z);
        let inward = -port.outward;
        let valve_top =
            shape.base + (BODY_HEIGHT + SHOULDER_HEIGHT + NECK_HEIGHT + VALVE_HEIGHT) * unit;
        let regulator_y = valve_top - VALVE_HEIGHT * unit / 2.0;
        let outlet = vec3(foot.x, regulator_y, foot.z) + inward * REGULATOR_LENGTH * unit;
        let facing = DIAL_FACING.normalize();
        let dial =
            vec3(foot.x, regulator_y + DIAL_RISE * unit, foot.z) + inward * DIAL_ALONG * unit;
        Self {
            unit,
            foot,
            inward,
            valve_top,
            outlet,
            dial: (dial, facing),
            knob: vec3(foot.x, valve_top + KNOB_RISE * unit, foot.z),
            flange: port.at(shape.outer),
        }
    }
}

/// Where the bottle, its knob and its gauge stand for a block of `shape`
/// with the port `port`.
pub fn tank_place(shape: BlockShape, port: Port) -> TankPlace {
    let layout = Layout::new(shape, port);
    TankPlace {
        foot: layout.foot,
        radius: TANK_RADIUS * layout.unit,
        height: layout.knob.y + KNOB_HUB_HEIGHT * layout.unit - shape.base,
        gauge: layout.dial.0,
        knob: layout.knob,
    }
}

/// How much the bottle holds with `particles` in the engine, from 0 (empty)
/// to 1 (full): it holds enough to take the engine from its least fill to
/// its most with a little to spare, so the needle crosses most of the dial
/// over the slider's range.
pub fn tank_share(particles: usize) -> f32 {
    let filled =
        (particles.saturating_sub(MIN_PARTICLES)) as f32 / (MAX_PARTICLES - MIN_PARTICLES) as f32;
    (1.0 - NEEDLE_SWING * filled).clamp(0.0, 1.0)
}

/// The needle's turn from straight up, in radians clockwise as seen on
/// the dial, for a bottle holding `share` of what it holds full.
pub fn needle_turn(share: f32) -> f32 {
    (share.clamp(0.0, 1.0) - 0.5) * NEEDLE_SWEEP
}

/// The bottle, regulator, gauge and pipe, and the glow at the port.
pub struct Tank {
    /// Everything that stays still, on the graphics card.
    body: GpuMesh,
    /// The knob and the needle, as posed this frame.
    moving: GpuMesh,
    /// The knob's turn and the needle's the moving parts were last posed
    /// at: they are built again only when one changes.
    posed: Option<(f32, f32)>,
    /// The parts' places.
    layout: Layout,
    /// The glow at the port.
    puff: Sprites,
    /// The port.
    port: Port,
    /// The lit-metal material, if it can be built.
    material: Option<GpuMaterial>,
}

impl Tank {
    /// The tank for a block of `shape` with the port `port`.
    ///
    /// It needs a window, for the material.
    pub fn new(shape: BlockShape, port: Port) -> Self {
        let mut tank = Self::build(shape, port);
        tank.material = load_metal();
        if tank.material.is_none() {
            solidify(tank.body.edit());
        }
        tank
    }

    /// The tank as `new` builds it, without a material.
    ///
    /// # Panics
    ///
    /// Panics if it would not fit Macroquad's buffers.
    fn build(shape: BlockShape, port: Port) -> Self {
        let layout = Layout::new(shape, port);
        let solid = build_body(&layout, port);
        assert!(solid.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
        assert!(solid.indices.len() < DRAW_CALL_INDEX_CAPACITY);
        Self {
            body: GpuMesh::new(mesh(solid)),
            moving: GpuMesh::empty(),
            posed: None,
            layout,
            puff: Sprites::new(),
            port,
            material: None,
        }
    }

    /// Poses the knob turned `knob_turn` radians counterclockwise seen
    /// from above and the needle for `particles` in the engine, and builds
    /// the glow at the port for gas moving at `flow` particles per second,
    /// facing the camera as `facing` says.
    pub fn pose(&mut self, knob_turn: f32, particles: usize, flow: f32, facing: Facing) {
        let turns = (knob_turn, needle_turn(tank_share(particles)));
        if self.posed != Some(turns) {
            self.posed = Some(turns);
            let mut solid = Solid::new();
            add_knob(&mut solid, &self.layout, turns.0);
            add_needle(&mut solid, &self.layout, turns.1);
            let moving = self.moving.edit();
            *moving = mesh(solid);
            if self.material.is_none() {
                solidify(moving);
            }
        }
        self.puff.clear();
        let strength = (flow.abs() / FLOW_PER_SECOND).min(1.0);
        if strength > 0.0 {
            let unit = self.layout.unit;
            let middle = self.port.at(self.port.bore - PUFF_SIZE * unit / 2.0);
            let size = PUFF_SIZE * unit * (0.5 + 0.5 * strength);
            let color = with_alpha(PUFF_COLOR, PUFF_OPACITY * strength);
            self.puff.facing(facing, middle, size, size, color);
        }
        self.puff.finish(facing.forward);
    }

    /// Draws the bottle, regulator, gauge and pipe seen from `camera`.
    pub fn draw(&self, camera: &Camera3D) {
        let light = key_light(camera.position);
        draw_lit(
            [&self.body, &self.moving],
            self.material.as_ref(),
            camera,
            light,
        );
    }

    /// Draws the glow at the port, which adds its light to the scene.
    pub fn draw_puff(&self, materials: &SpriteMaterials) {
        self.puff.draw(&materials.glow);
    }
}

/// A mesh of `solid`.
fn mesh(solid: Solid) -> Mesh {
    Mesh {
        vertices: solid.vertices,
        indices: solid.indices,
        texture: None,
    }
}

/// Makes `mesh` opaque: drawn in plain colors, without the lit-metal
/// material, its colors' alpha would read as transparency.
fn solidify(mesh: &mut Mesh) {
    for vertex in &mut mesh.vertices {
        vertex.color[3] = u8::MAX;
    }
}

/// The outline of a quarter round from (`from`) to (`to`), bulging away
/// from the corner between them, in `color`; the first point left out.
fn rounded(from: (f32, f32), to: (f32, f32), color: Color) -> Vec<Turn> {
    (1..=ROUND_STEPS)
        .map(|step| {
            let angle = FRAC_PI_2 * step as f32 / ROUND_STEPS as f32;
            let reach = to.0 + (from.0 - to.0) * angle.cos();
            let along = from.1 + (to.1 - from.1) * angle.sin();
            turn(reach, along, color)
        })
        .collect()
}

/// The outline of the bottle, foot to neck, as a lathe turns it about its
/// upright axis, `unit` to a bore radius.
fn bottle_outline(unit: f32) -> Vec<Turn> {
    let radius = TANK_RADIUS * unit;
    let round = FOOT_ROUND * unit;
    let side = BODY_HEIGHT * unit;
    let mut outline = vec![
        turn(0.0, 0.0, TANK_PAINT),
        turn(radius - round, 0.0, TANK_PAINT),
    ];
    // The foot rounds up into the side.
    outline.extend(rounded((radius - round, 0.0), (radius, round), TANK_PAINT));
    // Up the side: two bumper rings, each with a thin chrome trim band
    // standing just proud of the side below it and above it. Each change
    // of color or slope is doubled, starting a new edge, so the edges stay
    // crisp.
    let up = |height: f32, reach: f32, color: Color, outline: &mut Vec<Turn>| {
        outline.push(turn(reach, height * unit, color));
    };
    let trim = radius + TRIM_OUT * unit;
    for start in BUMPER_HEIGHTS {
        let end = start + BUMPER_HEIGHT;
        up(start - TRIM_HEIGHT, radius, TANK_PAINT, &mut outline);
        up(start - TRIM_HEIGHT, trim, CHROME, &mut outline);
        up(start, trim, CHROME, &mut outline);
        up(start, radius, BUMPER_PAINT, &mut outline);
        // The bumper bulges out and back in a half round.
        for step in 1..ROUND_STEPS {
            let share = step as f32 / ROUND_STEPS as f32;
            let out = BUMPER_OUT * unit * (PI * share).sin();
            let height = (start + BUMPER_HEIGHT * share) * unit;
            outline.push(turn(radius + out, height, BUMPER_PAINT));
        }
        up(end, radius, BUMPER_PAINT, &mut outline);
        up(end, trim, CHROME, &mut outline);
        up(end + TRIM_HEIGHT, trim, CHROME, &mut outline);
        up(end + TRIM_HEIGHT, radius, TANK_PAINT, &mut outline);
    }
    // The shoulder curves in to the chrome neck.
    outline.push(turn(radius, side, TANK_PAINT));
    let neck = NECK_RADIUS * unit;
    let top = side + SHOULDER_HEIGHT * unit;
    outline.extend(rounded((radius, side), (neck, top), TANK_PAINT));
    let neck_top = top + NECK_HEIGHT * unit;
    outline.extend([
        turn(neck, top, CHROME),
        turn(neck, neck_top, CHROME),
        turn(0.0, neck_top, CHROME),
    ]);
    outline
}

/// Builds the bottle, valve, regulator, gauge (all but its needle) and
/// pipe for `layout`, the pipe joining the block at `port`.
fn build_body(layout: &Layout, port: Port) -> Solid {
    let unit = layout.unit;
    let mut solid = Solid::new();
    let foot = layout.foot;
    solid.lathe(Frame::along(foot, Vec3::Y), &bottle_outline(unit), SEGMENTS);
    // The valve, a chunky chrome drum on the neck.
    let neck_top = foot.y + (BODY_HEIGHT + SHOULDER_HEIGHT + NECK_HEIGHT) * unit;
    let valve_bottom = vec3(foot.x, neck_top, foot.z);
    let valve_top = vec3(foot.x, layout.valve_top, foot.z);
    solid.cylinder(
        valve_bottom,
        valve_top,
        VALVE_RADIUS * unit,
        SEGMENTS,
        CHROME,
    );
    // The regulator, out of the valve's side toward the engine, with a
    // chrome cap.
    let regulator_start = vec3(foot.x, layout.outlet.y, foot.z);
    let regulator = REGULATOR_RADIUS * unit;
    solid.cylinder(
        regulator_start,
        layout.outlet,
        regulator,
        SEGMENTS,
        TANK_PAINT,
    );
    let cap = REGULATOR_CAP * unit;
    let cap_radius = regulator + REGULATOR_CAP_OUT * unit;
    solid.cylinder(
        layout.outlet - layout.inward * cap,
        layout.outlet,
        cap_radius,
        SEGMENTS,
        CHROME,
    );
    // The gauge on its stem, which ends at the case's back so none of it
    // shows in front of the face.
    let (dial, facing) = layout.dial;
    let stem_foot = regulator_start + layout.inward * DIAL_ALONG * unit;
    let back = dial - facing * DIAL_DEPTH * unit;
    solid.cylinder(stem_foot, back, PIPE_RADIUS * unit, SEGMENTS, CHROME);
    add_gauge(&mut solid, layout);
    // The pipe: down from the regulator to the port's height, then along
    // the port's line into the block.
    let pipe = PIPE_RADIUS * unit;
    let elbow = vec3(layout.outlet.x, port.height, layout.outlet.z);
    solid.cylinder(layout.outlet, elbow, pipe, SEGMENTS, CHROME);
    solid.cylinder(elbow, layout.flange, pipe, SEGMENTS, CHROME);
    for corner in [layout.outlet, elbow] {
        ball(&mut solid, corner, pipe, CHROME);
    }
    // A chrome flange on the block's outside, and a rim round the port
    // inside the cylinder.
    let out = port.outward;
    solid.cylinder(
        layout.flange - out * FLANGE_THICKNESS * unit / 2.0,
        layout.flange + out * FLANGE_THICKNESS * unit / 2.0,
        FLANGE_RADIUS * unit,
        SEGMENTS,
        CHROME,
    );
    let inside = port.at(port.bore - STANDOFF * unit);
    solid.lathe(
        Frame::along(inside, -out),
        &[
            turn(port.radius, 0.0, CHROME),
            turn(port.radius + RIM_WIDTH * unit, 0.0, CHROME),
        ],
        SEGMENTS,
    );
    solid
}

/// Straight up on the gauge's face, and to its right as seen from the
/// front, for a face looking along `facing`.
fn dial_axes(facing: Vec3) -> (Vec3, Vec3) {
    let up = (Vec3::Y - facing * Vec3::Y.dot(facing)).normalize();
    (up, up.cross(facing))
}

/// Adds the gauge's case, its fat chrome bezel, its white face and the
/// marks round its emptier part, yellow then red near empty.
fn add_gauge(solid: &mut Solid, layout: &Layout) {
    let unit = layout.unit;
    let (dial, facing) = layout.dial;
    let radius = DIAL_RADIUS * unit;
    let back = dial - facing * DIAL_DEPTH * unit;
    solid.cylinder(back, dial, radius, SEGMENTS, CHROME);
    // The bezel: a rounded ring standing proud of the face.
    let bezel = BEZEL_HALF * unit;
    let ring: Vec<Turn> = (0..=2 * ROUND_STEPS)
        .map(|step| {
            let angle = PI * step as f32 / (2 * ROUND_STEPS) as f32;
            turn(radius - bezel * angle.cos(), bezel * angle.sin(), CHROME)
        })
        .collect();
    solid.lathe(Frame::along(dial, facing), &ring, SEGMENTS);
    let face = dial + facing * STANDOFF * unit;
    solid.lathe(
        Frame::along(face, facing),
        &[
            turn(0.0, 0.0, DIAL_FACE),
            turn(radius - bezel, 0.0, DIAL_FACE),
        ],
        SEGMENTS,
    );
    let (up, right) = dial_axes(facing);
    let marks = dial + facing * 2.0 * STANDOFF * unit;
    for tick in 0..DIAL_TICKS {
        let share = tick as f32 / (DIAL_TICKS - 1) as f32;
        let angle = needle_turn(share);
        let way = up * angle.cos() + right * angle.sin();
        let (inner, outer) = TICK_SPAN;
        // Only the emptier part is marked: yellow, then red near empty.
        if share > MARKED_SHARE + f32::EPSILON {
            continue;
        }
        let color = TICK_COLORS[usize::from(share < MARKED_SHARE / 2.0)];
        let half = TICK_HALF * unit;
        solid.bar(
            marks + way * radius * inner,
            marks + way * radius * outer,
            way.cross(facing),
            (half, half),
            half / 2.0,
            color,
        );
    }
}

/// Adds a ball of `radius` at `center`, to round a bend in the pipe.
fn ball(solid: &mut Solid, center: Vec3, radius: f32, color: Color) {
    let outline: Vec<_> = (0..=BALL_STEPS)
        .map(|step| {
            let angle = PI * step as f32 / BALL_STEPS as f32;
            turn(radius * angle.sin(), radius * (1.0 - angle.cos()), color)
        })
        .collect();
    solid.lathe(
        Frame::along(center - Vec3::Y * radius, Vec3::Y),
        &outline,
        SEGMENTS,
    );
}

/// Adds the knob for `layout`, turned `angle` counterclockwise seen from
/// above: a red rim on four red spokes round a chrome hub.
fn add_knob(solid: &mut Solid, layout: &Layout, angle: f32) {
    let unit = layout.unit;
    let center = layout.knob;
    let (rim, ring) = (KNOB_RADIUS * unit, KNOB_RIM_HALF * unit);
    // The rim's section is round, for a chunky toy look.
    let section: Vec<Turn> = (0..=2 * ROUND_STEPS)
        .map(|step| {
            let around = TAU * step as f32 / (2 * ROUND_STEPS) as f32;
            turn(rim + ring * around.cos(), ring * around.sin(), TOY_RED)
        })
        .collect();
    solid.lathe(Frame::along(center, Vec3::Y), &section, SEGMENTS);
    let hub = KNOB_HUB_HEIGHT * unit / 2.0;
    solid.cylinder(
        center - Vec3::Y * hub,
        center + Vec3::Y * hub,
        KNOB_HUB_RADIUS * unit,
        SEGMENTS,
        CHROME,
    );
    let half = KNOB_SPOKE_HALF * unit;
    for spoke in 0..KNOB_SPOKES {
        // Counterclockwise seen from above is toward -z from +x.
        let turned = angle + TAU * spoke as f32 / KNOB_SPOKES as f32;
        let way = vec3(turned.cos(), 0.0, -turned.sin());
        solid.bar(
            center + way * KNOB_HUB_RADIUS * unit,
            center + way * rim,
            Vec3::Y.cross(way),
            (half, half),
            half,
            TOY_RED,
        );
    }
}

/// Adds the needle for `layout`, turned `angle` from straight up,
/// clockwise as seen on the dial, with a red cap over its pivot.
fn add_needle(solid: &mut Solid, layout: &Layout, angle: f32) {
    let unit = layout.unit;
    let (dial, facing) = layout.dial;
    let (up, right) = dial_axes(facing);
    let way = up * angle.cos() + right * angle.sin();
    let pivot = dial + facing * 3.0 * STANDOFF * unit;
    let tip = pivot + way * DIAL_RADIUS * NEEDLE_REACH * unit;
    let half = NEEDLE_HALF * unit;
    // Wide at the pivot, narrowing to the tip, lying flat on the face.
    solid.bar(
        pivot,
        tip,
        way.cross(facing),
        (half, half / 4.0),
        half / 2.0,
        TOY_RED,
    );
    solid.cylinder(
        pivot,
        pivot + facing * half,
        PIVOT_RADIUS * unit,
        SEGMENTS,
        TOY_RED,
    );
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/tank.rs"]
mod tests;
