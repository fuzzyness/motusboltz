//! The cold reservoir as a coil of clear glass tube wound around the cold
//! half of the engine block, fed with liquid nitrogen by the blue valve.
//!
//! The tube runs back and forth around the cold half in a few passes near
//! the floor, close to the block, from its inlet on the valve's pipe. Once
//! the valve opens at all, below 300 K, liquid nitrogen fills and runs
//! through the whole coil; the colder the Cold side setting, the faster it
//! flows, carrying bubbles with it, and the more the glass frosts. Wisps
//! of cold smoke curl off the coil and the feed pipe, more, bigger, thicker
//! and longer lasting the colder it is; like the mist off dry ice, most of
//! it is heavier than the air, so it sinks a little as it fades. The
//! boiled-off gas leaves through an outlet at the coil's end that turns
//! down toward the floor, so its smoke sinks gently there, below the
//! engine, instead of rising in front of it.
//!
//! The coil is thin and see-through, so the gas in the cold cylinder stays
//! in view behind it. A real coil would be copper; glass shows the flow.
//! Drawing only: the physics knows nothing of it, and the coil stands for
//! the surroundings, not the gas.

use std::f32::consts::PI;

use macroquad::models::Mesh;
use macroquad::prelude::{
    Camera3D,
    Color,
    Material,
    Vec3, //
};

use super::block::{
    BlockShape,
    key_light, //
};
use super::chamber::append_tube;
use super::outline::{
    Half,
    HalfOutline, //
};
use super::sprites::{
    Facing,
    SpriteMaterials,
    Sprites,
    smoothstep,
    unit_hash,
    with_alpha, //
};
use super::valves::{
    PIPE_HEIGHT,
    valve_wheels, //
};
use super::{
    DRAW_CALL_INDEX_CAPACITY,
    DRAW_CALL_VERTEX_CAPACITY,
    draw_lit,
    load_clear, //
};
use crate::sim::reservoir::{
    MAX_COLD_KELVIN,
    MIN_COLD_KELVIN, //
};

/// How far the coil's runs along the block's front and back reach from
/// the cold cylinder's axis toward the middle, as a share of the distance
/// between them; the inlet is at the front run's inner end.
const COIL_REACH: f32 = 0.5;
/// Passes of the coil around the cold half, bottom to top.
const PASSES: usize = 4;
/// Height between passes, in bore radii.
const PASS_SPACING: f32 = 0.16;
/// Steps along each pass.
const PASS_STEPS: usize = 40;
/// From the block's side out to the tube's axis, in bore radii.
const COIL_OUT: f32 = 0.06;
/// Outer radius of the glass tube, in bore radii.
const TUBE_RADIUS: f32 = 0.032;
/// How far the outlet steps out from the coil's end before it turns
/// down, in bore radii.
const OUTLET_OUT: f32 = 0.08;
/// Height of the outlet's mouth above the floor, in bore radii.
const OUTLET_MOUTH: f32 = 0.05;
/// Radius of the liquid inside the tube, as a share of the tube's.
const LIQUID_SHARE: f32 = 0.7;
/// The glass: a faint blue-white, mostly clear (alpha is opacity).
const GLASS_TINT: Color = Color::new(0.82, 0.92, 1.0, 0.18);
/// Frosted glass (alpha is opacity).
const FROSTED: Color = Color::new(0.93, 0.97, 1.0, 0.4);
/// The liquid nitrogen: pale blue and partly clear (alpha is opacity).
const LIQUID: Color = Color::new(0.45, 0.78, 1.0, 0.8);
/// Bubbles in the coil when it is full.
const BUBBLES: u32 = 60;
/// Coil lengths a bubble travels per second at the first sign of cold.
const BUBBLE_PACE_SLOWEST: f32 = 0.03;
/// Coil lengths a bubble travels per second at full flow, at 77 K.
const BUBBLE_PACE_FASTEST: f32 = 0.15;
/// Longest step of the clock taken at once, in seconds, so a stall does
/// not fling the bubbles and wisps forward.
const LONGEST_STEP: f64 = 0.25;
/// Half-size of a bubble, in bore radii.
const BUBBLE_HALF_SIZE: f32 = 0.026;
/// Color of a bubble.
const BUBBLE_COLOR: Color = Color::from_rgba(240, 250, 255, 255);
/// Opacity of a bubble.
const BUBBLE_OPACITY: f32 = 0.9;
/// Salt for each bubble's place.
const SALT_BUBBLE: u32 = 21;
/// Fragment shader for the glass and the liquid: lit like the block, with
/// a glint, and more opaque toward their edges, where the eye looks
/// through more of them, as glass and water are.
const CLEAR_FRAGMENT: &str = r#"#version 100
precision mediump float;

varying lowp vec4 color;
varying lowp vec2 uv;
varying vec3 world;
varying vec3 surface_normal;

uniform sampler2D Texture;
uniform vec3 Eye;
uniform vec3 Light;

void main() {
    vec3 toward_eye = normalize(Eye - world);
    vec3 n = normalize(surface_normal);
    n = dot(n, toward_eye) < 0.0 ? -n : n;
    float facing = max(dot(n, toward_eye), 0.0);
    float diffuse = max(dot(n, Light), 0.0);
    float shine = pow(max(dot(n, normalize(Light + toward_eye)), 0.0), 40.0);
    float edge = pow(1.0 - facing, 2.0);
    vec3 lit = color.rgb * (0.6 + 0.4 * diffuse) + vec3(0.7 * shine);
    float alpha = clamp(color.a * (0.7 + 0.6 * edge) + 0.5 * shine, 0.0, 1.0);
    gl_FragColor = vec4(lit, alpha) * texture2D(Texture, uv);
}"#;
/// Reach of the frost on the floor around the coil, in radii of the
/// coil's path.
const FROST_REACH: f32 = 1.7;
/// Color of the frost on the floor.
const FLOOR_FROST_COLOR: Color = Color::from_rgba(205, 230, 255, 255);
/// Opacity of the frost on the floor at its coldest.
const FLOOR_FROST_OPACITY: f32 = 0.3;
/// Height of the floor frost above the floor.
const ABOVE_FLOOR: f32 = 0.012;
/// Wisps of smoke.
const PUFFS: u32 = 420;
/// Share of the wisps that show at the first sign of cold.
const FEWEST_PUFFS: f32 = 0.35;
/// Lives of each wisp per second at the first sign of cold: thin smoke
/// fades fast.
const PUFF_RATE_WARM: f32 = 0.7;
/// Lives of each wisp per second at 77 K: thick smoke lingers.
const PUFF_RATE_COLD: f32 = 0.28;
/// Share of its full size a wisp has at the first sign of cold.
const PUFF_SCALE_WARM: f32 = 0.45;
/// How far a wisp drifts out from its pipe over its life, in bore radii.
const PUFF_SPREAD: f32 = 0.12;
/// Half-size of a new wisp, in bore radii.
const PUFF_SIZE_NEW: f32 = 0.04;
/// Half-size of a wisp at the end of its life, in bore radii.
const PUFF_SIZE_OLD: f32 = 0.25;
/// Opacity of a wisp just below 300 K.
const PUFF_OPACITY_LOW: f32 = 0.4;
/// Opacity of a wisp at 77 K.
const PUFF_OPACITY_HIGH: f32 = 0.95;
/// Share of the wisps that rise a little instead of sinking.
const RISING_SHARE: f32 = 0.3;
/// Share of the wisps that come off the feed pipe rather than the coil.
const PIPE_SHARE: f32 = 0.2;
/// Share of the wisps that pour from the outlet's mouth.
const OUTLET_SHARE: f32 = 0.25;
/// How far smoke from the outlet sinks over its life, in bore radii.
const OUTLET_SINK: f32 = 0.2;
/// How far smoke from the outlet spreads over the floor over its life, in
/// bore radii: a slow pool, not a jet.
const OUTLET_SPREAD: f32 = 0.1;
/// Coldness over which each wisp fades in.
const REVEAL_SPAN: f32 = 0.08;
/// Coldness by which the first wisps have fully faded in.
const FIRST_SHARE: f32 = 0.08;
/// How far a wisp rises or sinks over its life, in bore radii.
const DRIFT: f32 = 0.12;
/// How high a sunken wisp rests, in its own half-sizes.
const SETTLE: f32 = 0.55;
/// Color of the smoke: white with a faint cool tint, the color of the
/// fog liquid nitrogen makes.
const MIST_COLOR: Color = Color::from_rgba(232, 240, 252, 255);
/// Salt for each puff's place along the coil.
const SALT_PLACE: u32 = 11;
/// Salt for each puff's spread.
const SALT_WIDTH: u32 = 12;
/// Salt for each puff's starting height.
const SALT_HEIGHT: u32 = 13;
/// Salt for each puff's phase.
const SALT_PUFF: u32 = 16;
/// Salt for whether a wisp pours from the outlet.
const SALT_KIND: u32 = 17;
/// Salt for how far out a puff starts.
const SALT_DEPTH: u32 = 18;
/// Salt for whether a wisp off the pipes rises.
const SALT_RISE: u32 = 19;

/// How cold the coil is, from 0 (empty, at 300 K) to 1 (full, at 77 K).
pub fn cold_level(cold_kelvin: f32) -> f32 {
    ((MAX_COLD_KELVIN - cold_kelvin) / (MAX_COLD_KELVIN - MIN_COLD_KELVIN)).clamp(0.0, 1.0)
}

/// Across of the inlet, where the coil's front run starts and the blue
/// valve's pipe joins it, for a block of `shape`.
pub fn coil_inlet_x(shape: BlockShape) -> f32 {
    shape.offset * (1.0 - COIL_REACH)
}

/// Whether the see-through material is ready.
///
/// It is built on the first draw, since its shader needs the window; if it
/// cannot be built, the coil is left out.
enum Clear {
    /// Not built yet.
    Untried,
    /// Built.
    Ready(Material),
    /// Its shader could not be built.
    Unavailable,
}

/// One point along the coil's axis.
#[derive(Clone, Copy, Debug)]
struct Station {
    /// Where it is.
    point: Vec3,
    /// The way straight out from the block there, level.
    outward: Vec3,
    /// Its distance along the coil from the inlet, as a share of the whole.
    along: f32,
}

/// The coil, the liquid and bubbles in it, and the frost and mist.
pub struct Nitrogen {
    /// The bore radius every size is a fraction of.
    unit: f32,
    /// Height of the floor.
    floor: f32,
    /// The cold cylinder's axis, seen from above (x, z).
    axis: (f32, f32),
    /// Radius of the coil around the cold cylinder's rounded end.
    reach: f32,
    /// Where the feed pipe leaves the blue valve, at the inlet's height.
    feed: Vec3,
    /// Points along the coil from the inlet to its end.
    stations: Vec<Station>,
    /// The outlet: from the coil's end, out a little and down to its mouth
    /// near the floor.
    outlet: [Station; 3],
    /// The glass, frosted as it is now.
    glass: Mesh,
    /// The liquid, as far along as it has flowed; empty when there is none.
    liquid: Mesh,
    /// Bubbles in the liquid.
    bubbles: Sprites,
    /// Frost on the floor, and mist.
    mist: Sprites,
    /// The see-through material, once tried.
    clear: Clear,
    /// The clock at the last build, if there was one.
    last_seconds: Option<f64>,
    /// How far the liquid has flowed, in coil lengths; it runs faster the
    /// colder the setting.
    flowed: f64,
    /// How many lives the wisps have lived; they live faster the warmer the
    /// setting.
    smoked: f64,
}

impl Nitrogen {
    /// The coil around the cold half of a block of `shape`, empty.
    pub fn new(shape: BlockShape) -> Self {
        let unit = shape.bore;
        let radius = shape.outer + COIL_OUT * unit;
        let wall = HalfOutline::new(Half::Cold, shape.offset, radius, COIL_REACH);
        let axis = wall.axis();
        let bottom = shape.base + PIPE_HEIGHT * unit;
        let mut points = Vec::new();
        for pass in 0..PASSES {
            let y = bottom + pass as f32 * PASS_SPACING * unit;
            for step in 0..=PASS_STEPS {
                let share = step as f32 / PASS_STEPS as f32;
                // Back and forth: each pass starts where the last ended.
                let share = if pass % 2 == 0 { share } else { 1.0 - share };
                let (spot, out) = wall.at(share);
                points.push((Vec3::new(spot.x, y, spot.y), Vec3::new(out.x, 0.0, out.y)));
            }
        }
        let lengths: Vec<f32> = std::iter::once(0.0)
            .chain(points.windows(2).scan(0.0, |total, pair| {
                *total += pair[0].0.distance(pair[1].0);
                Some(*total)
            }))
            .collect();
        let &(end, end_out) = points.last().expect("the coil has passes");
        let elbow = end + end_out * OUTLET_OUT * unit;
        let mouth = Vec3::new(elbow.x, shape.base + OUTLET_MOUTH * unit, elbow.z);
        let outlet = [end, elbow, mouth].map(|point| Station {
            point,
            outward: end_out,
            along: 1.0,
        });
        let wheel = valve_wheels(shape)[1].center;
        let feed = Vec3::new(wheel.x, bottom, wheel.z);
        let total = lengths.last().copied().unwrap_or(1.0).max(f32::EPSILON);
        let stations = points
            .iter()
            .zip(&lengths)
            .map(|(&(point, outward), length)| Station {
                point,
                outward,
                along: length / total,
            })
            .collect();
        let mut nitrogen = Self {
            unit,
            floor: shape.base,
            axis: (axis.x, axis.y),
            reach: radius,
            feed,
            stations,
            outlet,
            glass: empty_mesh(),
            liquid: empty_mesh(),
            bubbles: Sprites::new(),
            mist: Sprites::new(),
            clear: Clear::Untried,
            last_seconds: None,
            flowed: 0.0,
            smoked: 0.0,
        };
        nitrogen.build_tubes(0.0);
        nitrogen
    }

    /// Draws the liquid, then the glass around it, both see-through, seen
    /// from `camera`; call it after everything solid.
    pub fn draw_coil(&mut self, camera: &Camera3D) {
        if let Clear::Untried = self.clear {
            self.clear = load_clear(CLEAR_FRAGMENT).map_or(Clear::Unavailable, Clear::Ready);
        }
        let Clear::Ready(material) = &self.clear else {
            return;
        };
        let light = key_light(camera.position);
        draw_lit([&self.liquid, &self.glass], Some(material), camera, light);
    }

    /// Draws the bubbles, the frost on the floor and the mist.
    pub fn draw(&self, materials: &SpriteMaterials) {
        self.bubbles.draw(&materials.sparkle);
        self.mist.draw(&materials.haze);
    }

    /// Fills the coil and builds the bubbles and mist for `cold` (see
    /// `cold_level`) at `seconds` on the clock, facing the camera.
    pub fn build(&mut self, cold: f32, seconds: f64, facing: Facing) {
        // The flow and the smoke run on their own clocks, whose pace follows
        // the setting, so a change of setting speeds or slows them without
        // making them jump.
        let step = self
            .last_seconds
            .map_or(0.0, |last| (seconds - last).clamp(0.0, LONGEST_STEP));
        self.last_seconds = Some(seconds);
        if cold > 0.0 {
            self.flowed += step * bubble_pace(cold) as f64;
            self.smoked += step * puff_rate(cold) as f64;
        }
        self.build_tubes(cold);
        self.build_bubbles(cold, facing);
        self.build_mist(cold, facing);
    }

    /// Builds the glass, frosted the more the colder `cold` is, and the
    /// liquid filling the whole coil whenever the valve is open at all.
    fn build_tubes(&mut self, cold: f32) {
        let frosted = blend(GLASS_TINT, FROSTED, cold);
        let mut path = self.stations.clone();
        path.extend_from_slice(&self.outlet[1..]);
        self.glass = self.tube(&path, TUBE_RADIUS, &|_| frosted);
        self.liquid = if cold > 0.0 {
            let liquid = with_alpha(LIQUID, LIQUID.a * smoothstep(0.0, FIRST_SHARE, cold));
            self.tube(&self.stations, LIQUID_SHARE * TUBE_RADIUS, &|_| liquid)
        } else {
            empty_mesh()
        };
    }

    /// A tube of `radius` (in bore radii) through `stations`, colored at
    /// each by `color`.
    ///
    /// # Panics
    ///
    /// Panics if it would not fit Macroquad's buffers.
    fn tube(&self, stations: &[Station], radius: f32, color: &dyn Fn(&Station) -> Color) -> Mesh {
        let mut mesh = empty_mesh();
        let count = stations.len();
        let last = count - 1;
        let first = mesh.vertices.len();
        append_tube(
            &mut mesh,
            count,
            false,
            radius * self.unit,
            Color::new(1.0, 1.0, 1.0, 1.0),
            |index| {
                let before = stations[index.saturating_sub(1)].point;
                let after = stations[(index + 1).min(last)].point;
                let tangent = (after - before).normalize_or_zero();
                let side = tangent.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);
                (stations[index].point, side, tangent.cross(side))
            },
        );
        // The tube's vertices go round each station in turn; color each
        // ring by its station, keeping the light baked into it.
        let sides = (mesh.vertices.len() - first) / count;
        for (index, vertex) in mesh.vertices[first..].iter_mut().enumerate() {
            let tint = color(&stations[index / sides]);
            let light = vertex.color[0] as f32 / u8::MAX as f32;
            vertex.color =
                Color::new(tint.r * light, tint.g * light, tint.b * light, tint.a).into();
        }
        assert!(mesh.vertices.len() < DRAW_CALL_VERTEX_CAPACITY);
        assert!(mesh.indices.len() < DRAW_CALL_INDEX_CAPACITY);
        mesh
    }

    /// The point `along` the coil (a share of its length from the inlet),
    /// with the way out from the block there.
    fn point_along(&self, along: f32) -> (Vec3, Vec3) {
        let next = self
            .stations
            .iter()
            .position(|station| station.along >= along)
            .unwrap_or(self.stations.len() - 1)
            .max(1);
        let (a, b) = (self.stations[next - 1], self.stations[next]);
        let span = (b.along - a.along).max(f32::EPSILON);
        let share = ((along - a.along) / span).clamp(0.0, 1.0);
        (a.point.lerp(b.point, share), a.outward)
    }

    /// Builds the bubbles riding along the liquid at `cold`.
    fn build_bubbles(&mut self, cold: f32, facing: Facing) {
        self.bubbles.clear();
        if cold <= 0.0 {
            return;
        }
        let half = BUBBLE_HALF_SIZE * self.unit;
        for bubble in 0..BUBBLES {
            // Each rides the whole coil from the inlet to its end and
            // starts again, faster the more nitrogen flows.
            let along =
                (self.flowed + unit_hash(bubble, SALT_BUBBLE) as f64).rem_euclid(1.0) as f32;
            let (point, _) = self.point_along(along);
            let color = with_alpha(
                BUBBLE_COLOR,
                BUBBLE_OPACITY * smoothstep(0.0, FIRST_SHARE, cold),
            );
            self.bubbles.facing(facing, point, half, half, color);
        }
        self.bubbles.finish(facing.forward);
    }

    /// Builds the frost on the floor, the smoke off the coil and the feed
    /// pipe, and the smoke pouring from the outlet for `cold`.
    fn build_mist(&mut self, cold: f32, facing: Facing) {
        self.mist.clear();
        if cold <= 0.0 {
            return;
        }
        let unit = self.unit;
        self.mist.flat(
            Vec3::new(self.axis.0, self.floor + ABOVE_FLOOR, self.axis.1),
            FROST_REACH * self.reach * (0.6 + 0.4 * cold),
            with_alpha(FLOOR_FROST_COLOR, FLOOR_FROST_OPACITY * cold),
        );
        let inlet = self.stations[0].point;
        // The mist fades in from nothing at the first sign of cold.
        let peak = smoothstep(0.0, FIRST_SHARE, cold)
            * (PUFF_OPACITY_LOW + (PUFF_OPACITY_HIGH - PUFF_OPACITY_LOW) * cold);
        for puff in 0..PUFFS {
            // Each wisp keeps its own place: off the feed pipe, between the
            // valve and the inlet, off the coil, or at the outlet's mouth.
            // Only how many show, how big and thick they are and how fast
            // they fade follow the setting, so a change thickens or thins
            // the smoke where it is instead of moving it along the tube.
            // The first share shows at the first sign of cold; each of the
            // rest fades in at its own coldness.
            let appears = (puff as f32 / PUFFS as f32 - FEWEST_PUFFS) / (1.0 - FEWEST_PUFFS);
            let presence = smoothstep(appears, appears + REVEAL_SPAN, cold);
            if presence <= 0.0 {
                break;
            }
            let place = unit_hash(puff, SALT_PLACE);
            let from_outlet = unit_hash(puff, SALT_KIND) < OUTLET_SHARE;
            let on_coil = !from_outlet && place >= PIPE_SHARE;
            let life = (self.smoked + unit_hash(puff, SALT_PUFF) as f64).rem_euclid(1.0) as f32;
            let half = unit
                * (PUFF_SIZE_NEW + (PUFF_SIZE_OLD - PUFF_SIZE_NEW) * life)
                * (PUFF_SCALE_WARM + (1.0 - PUFF_SCALE_WARM) * cold);
            // Each wisp drifts out a little from its place, most sinking
            // and a few rising.
            let alpha = peak * presence * (PI * life).sin();
            if from_outlet {
                // From the outlet's mouth it sinks and pools slowly on
                // the floor, spreading only a little.
                let mouth = self.outlet[2].point;
                let toward = Vec3::new(place - 0.5, 0.0, unit_hash(puff, SALT_WIDTH) - 0.5);
                let spot = mouth + toward.normalize_or_zero() * OUTLET_SPREAD * unit * life;
                let y = (mouth.y - OUTLET_SINK * unit * life).max(self.floor + SETTLE * half);
                let center = Vec3::new(spot.x, y, spot.z);
                self.mist
                    .facing(facing, center, half, half, with_alpha(MIST_COLOR, alpha));
                continue;
            }
            let (start, outward) = if on_coil {
                self.point_along((place - PIPE_SHARE) / (1.0 - PIPE_SHARE))
            } else {
                (inlet.lerp(self.feed, place / PIPE_SHARE), Vec3::Z)
            };
            let start_y = start.y + TUBE_RADIUS * unit * (2.0 * unit_hash(puff, SALT_HEIGHT) - 1.0);
            let out = TUBE_RADIUS * unit * unit_hash(puff, SALT_DEPTH)
                + PUFF_SPREAD * unit * (0.5 + unit_hash(puff, SALT_WIDTH)) * life;
            let rising = unit_hash(puff, SALT_RISE) < RISING_SHARE;
            let drift = if rising { DRIFT } else { -DRIFT } * unit * life;
            let spot = start + outward * out;
            let y = (start_y + drift).max(self.floor + SETTLE * half);
            let center = Vec3::new(spot.x, y, spot.z);
            self.mist
                .facing(facing, center, half, half, with_alpha(MIST_COLOR, alpha));
        }
        self.mist.finish(facing.forward);
    }
}

/// Coil lengths a bubble travels per second at `cold` (see `cold_level`).
fn bubble_pace(cold: f32) -> f32 {
    BUBBLE_PACE_SLOWEST + (BUBBLE_PACE_FASTEST - BUBBLE_PACE_SLOWEST) * cold
}

/// Lives of each wisp per second at `cold` (see `cold_level`).
fn puff_rate(cold: f32) -> f32 {
    PUFF_RATE_WARM + (PUFF_RATE_COLD - PUFF_RATE_WARM) * cold
}

/// `from` moved toward `to` by `share`, opacity too.
fn blend(from: Color, to: Color, share: f32) -> Color {
    let toward = |a: f32, b: f32| a + (b - a) * share.clamp(0.0, 1.0);
    Color::new(
        toward(from.r, to.r),
        toward(from.g, to.g),
        toward(from.b, to.b),
        toward(from.a, to.a),
    )
}

/// A mesh with nothing in it.
fn empty_mesh() -> Mesh {
    Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        texture: None,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/nitrogen.rs"]
mod tests;
