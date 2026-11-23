//! First-person mode: the view from a toy-sized figure standing on the
//! table.
//!
//! The figure is dropped onto the table from its button (see `figure`) at
//! a spot at the front, facing the engine. It is as tall as the
//! cylinders. The mouse, captured by the window, turns its head, and the
//! arrow keys or WASD walk it: slowly, for looking closely in a small
//! space, at full speed almost at once and stopped within a few
//! millimeters, never faster on the diagonal, its head bobbing gently with
//! each step. It slides along whatever it walks into (see `walking`).
//!
//! Space jumps: a quick, light hop high enough to land on a button box,
//! coming down a little faster than it went up, with some steering in the
//! air. A jump pressed a moment before landing is kept until it lands, and
//! one pressed a moment after walking off something still counts. Ctrl (or
//! C) crouches, lowering the eyes on a spring and slowing the walk;
//! crouching in the air tucks the feet up, so a crouched jump clears a
//! little more. Whenever the feet move up or down at once, on a step up or
//! a tuck, the eyes glide after them. Nothing stops it walking off an edge,
//! crouched or not, and off the table's edge it falls, and is put back at
//! the start. Esc lets the mouse go and opens the menu (see `menu`), with
//! a way back out.

use std::f32::consts::{
    FRAC_PI_2,
    PI, //
};

use macroquad::prelude::{
    Camera3D,
    Color,
    Rect,
    Vec2,
    Vec3,
    vec2,
    vec3, //
};

use crate::graphics::block::BlockShape;
use crate::ui::controls::BACKING;
use crate::ui::input::FirstPersonKeys;
use crate::ui::panel::QUIET;
use crate::ui::pen::Pen;
use crate::ui::walking::{
    STEP_UP,
    WalkMap, //
};

/// Height of the figure's eyes above the table, as a share of the
/// cylinders' height: the figure is as tall as the cylinders, its eyes a
/// little below the top of its big toy's head.
const EYE_SHARE: f32 = 0.85;
/// Radius of the figure's body, seen from above, in bore radii.
pub const BODY_RADIUS: f32 = 0.6;
/// Walking speed, in bore radii a second: about a third of the table's
/// length.
pub const WALK_SPEED: f32 = 5.3;
/// Seconds from standing to full speed.
const SPEED_UP_SECONDS: f32 = 0.1;
/// Seconds from full speed to standing.
const SLOW_DOWN_SECONDS: f32 = 0.08;
/// How far into something the figure may be pushed out to before it is
/// taken as wedged in a nook too narrow for it, in bore radii.
const WEDGE_TOLERANCE: f32 = 0.01;
/// Length of a step, in bore radii: the head bobs once a step.
const STEP_LENGTH: f32 = 2.2;
/// How far the head bounces down at each footfall, in bore radii: a
/// plain, plodding bounce, as in LSD: Dream Emulator.
const BOB_DEPTH: f32 = 0.22;
/// How far the head sways to each side, once over two steps, in bore
/// radii.
const BOB_SWAY: f32 = 0.08;
/// Seconds for the bob to grow to its full size on setting off, or die
/// away on stopping.
const BOB_FADE_SECONDS: f32 = 0.2;
/// How fast a jump slows on its way up, in bore radii a second each
/// second: a quick, light hop.
const GRAVITY: f32 = 40.0;
/// How much faster falling speeds up than rising slows, so the figure
/// comes down crisply rather than drifting.
const FALL_PULL: f32 = 1.5;
/// Height of a jump, in bore radii: well over a button box.
pub const JUMP_HEIGHT: f32 = 1.8;
/// How much of its usual pull toward the way the keys point, or to a stop,
/// the figure has in the air: a little steering, enough to land on a box
/// by letting go.
const AIR_CONTROL: f32 = 0.5;
/// Seconds after walking off something a jump still counts.
const COYOTE_SECONDS: f32 = 0.1;
/// Seconds before landing a jump pressed is kept, to jump on landing.
const JUMP_BUFFER_SECONDS: f32 = 0.12;
/// Height of the eyes crouched, as a share of their height standing.
const CROUCH_EYES: f32 = 0.6;
/// How quickly the eyes settle into a crouch or back up, in radians a
/// second: a spring with no overshoot, settled in about a quarter of a
/// second, whose speed never changes at once, even when the crouch key is
/// let go halfway down.
const CROUCH_SPRING: f32 = 18.0;
/// Walking speed crouched, as a share of the speed standing.
const CROUCH_SPEED: f32 = 0.5;
/// How far crouching in the air tucks the feet up, in bore radii: enough
/// that a crouched jump easily reaches the top of the desk clock.
const TUCK: f32 = 0.7;
/// Seconds for the eyes to catch up most of the way when the feet jump
/// up or down at once, as on a step up or a tuck: the view glides
/// instead of jolting.
const EYE_GLIDE_SECONDS: f32 = 0.07;
/// How far the view moves freely from where it was while the figure grips
/// a valve, the knob or the crank, in radians each way: about 15 degrees.
/// Within it the view stays where the mouse leaves it.
const GRIP_LIMIT: f32 = 0.26;
/// How much further the view gives, pushed past `GRIP_LIMIT`, in radians:
/// about 5 degrees, harder the further it goes.
const GRIP_GIVE: f32 = 0.09;
/// Seconds for a gripped view pushed past `GRIP_LIMIT` to spring most of
/// the way back to it.
const GRIP_SPRING_SECONDS: f32 = 0.25;
/// Seconds for a view let go to spring most of the way back to where it
/// was; the mouse turns it again at once meanwhile.
const GRIP_RETURN_SECONDS: f32 = 0.06;
/// Share of `GRIP_LIMIT` a gripped view pushed sideways must reach to be at
/// its edge, where it leans on what it grips (see `Walker::grip_lean`).
const GRIP_EDGE_SHARE: f32 = 0.97;
/// Most push kept past `GRIP_LIMIT`, as a share of `GRIP_GIVE`: the give
/// is nearly used up there.
const GRIP_PUSH_MOST: f32 = 2.0;
/// Least push left, in radians, below which a view let go is back.
const GRIP_SETTLED: f32 = 1e-3;
/// How far below the table's top a figure that has stepped off its edge
/// falls before it is put back at the start, in bore radii.
const FALL_DEPTH: f32 = 5.0;
/// Where the figure stands, along the shaft toward the cold side, in bore
/// radii.
const START_ACROSS: f32 = 3.6;
/// Where the figure stands, toward the front, in bore radii.
const START_AHEAD: f32 = 4.4;
/// The point the figure faces at first, above the block's middle, in bore
/// radii above the table.
const FIRST_LOOK_HEIGHT: f32 = 2.5;
/// Turn per point of mouse movement, in radians.
const LOOK_SENSITIVITY: f32 = 0.0025;
/// Moves of the mouse ignored after it is captured: the first move the
/// captured mouse reports can carry a jump (the pointer sent to the
/// window's middle, or all the motion saved up before the capture), which
/// would whip the head round however the mouse is moved.
const SETTLE_MOVES: u8 = 2;
/// Most the head follows the mouse in one frame, in points, so a stray
/// jump in the pointer cannot whip it round either.
const MAX_LOOK_STEP: f32 = 120.0;
/// How far short of straight up or down the head stops, in radians.
const LOOK_MARGIN: f32 = 0.05;
/// Field of view, top to bottom, in radians: wider than the orbit view's,
/// as a game's first-person view is.
const FIELD_OF_VIEW: f32 = 1.05;
/// Nearest distance drawn, in model units (centimeters): close enough for
/// parts within reach.
const NEAR: f32 = 0.2;
/// Farthest distance drawn.
const FAR: f32 = 400.0;

/// Radius of the crosshair's dot, in points.
const CROSSHAIR_RADIUS: f32 = 2.5;
/// Radius of the crosshair's ring, in points.
const CROSSHAIR_RING: f32 = 4.5;
/// The crosshair's dot.
const CROSSHAIR: Color = Color::new(1.0, 1.0, 1.0, 0.85);
/// The ring around it, so it shows on light surfaces too.
const CROSSHAIR_SHADE: Color = Color::new(0.0, 0.0, 0.0, 0.35);
/// Radius of the ring the crosshair grows over something within reach, in
/// points.
const AIM_RING: f32 = 10.0;
/// Thickness of that ring, in points.
const AIM_RING_THICKNESS: f32 = 2.0;
/// Pieces the ring is drawn in.
const AIM_RING_PIECES: usize = 24;
/// Size of the caption under the crosshair, in points.
const CAPTION_SIZE: f32 = 17.0;
/// How far under the crosshair the caption's baseline is, in points.
const CAPTION_BELOW: f32 = 40.0;
/// The hint at the bottom of the view.
const HINT: &str =
    "WASD: walk \u{b7} Space: jump \u{b7} Ctrl: crouch \u{b7} E or click: use \u{b7} Esc: menu";
/// The hint while the mouse is let go in the browser, with the menu
/// closed: a click on the view captures it again.
const WAITING_HINT: &str = "Click to look around \u{b7} Esc: menu";
/// Text size of the hint.
const HINT_SIZE: f32 = 14.0;
/// Room under the hint, in points.
const HINT_BOTTOM: f32 = 22.0;
/// Room round the hint's text, in points.
const HINT_PAD: f32 = 7.0;

/// A view held while the figure grips a valve, the knob or the crank (see
/// `Walker::grip`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Grip {
    /// The yaw the view was at, and springs back to.
    yaw: f32,
    /// The pitch the view was at, and springs back to.
    pitch: f32,
    /// How far the mouse has pushed the view since, in yaw and pitch, before
    /// the push past `GRIP_LIMIT` is softened. The mouse pushes it the
    /// other way to its moves, as it turns the view.
    push: Vec2,
    /// Whether the figure still grips.
    holding: bool,
}

/// What a step of walking came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Walked {
    /// The figure is on its feet, in the air, or still falling.
    OnFoot,
    /// It fell off the table far enough to be put back at the start.
    Fell,
}

/// The figure standing on the table: where it stands, how it moves, where
/// its eyes are and which way it looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Walker {
    /// Its eyes.
    pub eye: Vec3,
    /// While it grips something, or a gripped view springs back: the
    /// angles the view was at, how far the mouse has pushed it since in
    /// yaw and pitch, and whether it still grips.
    grip: Option<Grip>,
    /// The middle of its feet.
    feet: Vec3,
    /// How fast and which way it moves.
    velocity: Vec3,
    /// Height of its eyes above its feet, standing.
    eye_height: f32,
    /// Size of a bore radius, which its sizes and speeds are given in.
    unit: f32,
    /// How far it has walked, for its steps.
    walked: f32,
    /// How much of its bob the head has, from 0 standing still to 1
    /// walking at full speed.
    bobbing: f32,
    /// Whether it is in the air.
    airborne: bool,
    /// Seconds since it last stood on something.
    since_ground: f32,
    /// Whether it has jumped since it last stood on something.
    jumped: bool,
    /// Seconds since a jump was pressed that has not happened yet.
    jump_wanted: Option<f32>,
    /// How far it is crouched, from 0 standing to 1.
    crouch: f32,
    /// How fast it is crouching, in crouches a second.
    crouch_speed: f32,
    /// Whether its feet are tucked up from crouching in the air.
    tucked: bool,
    /// How far its eyes lag behind where they belong, after the feet
    /// jumped up or down at once; it fades away.
    eye_lag: f32,
    /// Its turn about the vertical, in radians; 0 looks toward -z (from the
    /// front toward the back).
    pub yaw: f32,
    /// Its look up (positive) or down, in radians.
    pub pitch: f32,
    /// The captured mouse's last position, once it has been read.
    last: Option<Vec2>,
    /// Moves of the mouse still to be ignored while the capture settles.
    settling: u8,
}

impl Walker {
    /// The figure at its starting spot on the table of a block of `shape`,
    /// facing the engine.
    pub fn standing(shape: BlockShape) -> Self {
        let unit = shape.bore;
        let feet = vec3(START_ACROSS * unit, shape.base, START_AHEAD * unit);
        let eye_height = EYE_SHARE * (shape.top - shape.base);
        let eye = feet + Vec3::Y * eye_height;
        let target = vec3(0.0, shape.base + FIRST_LOOK_HEIGHT * unit, 0.0);
        let look = (target - eye).normalize();
        Self {
            eye,
            feet,
            velocity: Vec3::ZERO,
            eye_height,
            unit,
            walked: 0.0,
            bobbing: 0.0,
            airborne: false,
            since_ground: 0.0,
            jumped: false,
            jump_wanted: None,
            crouch: 0.0,
            crouch_speed: 0.0,
            tucked: false,
            eye_lag: 0.0,
            yaw: (-look.x).atan2(-look.z),
            pitch: look.y.asin(),
            last: None,
            settling: SETTLE_MOVES,
            grip: None,
        }
    }

    /// The way it looks, a unit vector.
    pub fn forward(&self) -> Vec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        vec3(-sin_yaw * cos_pitch, sin_pitch, -cos_yaw * cos_pitch)
    }

    /// The middle of its feet.
    #[cfg(test)]
    pub fn feet(&self) -> Vec3 {
        self.feet
    }

    /// Whether it is in the air.
    #[cfg(test)]
    pub fn airborne(&self) -> bool {
        self.airborne
    }

    /// Moves the figure for `dt` seconds with the first-person keys `keys`
    /// on `map`: it speeds up toward the way the walking keys point,
    /// relative to the way it looks, or slows to a stop; slides along
    /// whatever it walks into; jumps, crouches, steps up, lands and falls.
    pub fn walk(&mut self, keys: FirstPersonKeys, dt: f32, map: &WalkMap) -> Walked {
        let dt = dt.max(0.0);
        let unit = self.unit;
        self.spring_view(dt);
        self.eye_lag *= (-dt / EYE_GLIDE_SECONDS).exp();
        self.crouch_for(keys.crouch, dt, map);
        // Along the table.
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let ahead = vec2(-sin_yaw, -cos_yaw);
        let right = vec2(cos_yaw, -sin_yaw);
        let wish = (right * keys.way.x + ahead * keys.way.y).clamp_length_max(1.0);
        let speed = WALK_SPEED * unit * (1.0 + (CROUCH_SPEED - 1.0) * self.crouch);
        let flat = vec2(self.velocity.x, self.velocity.z);
        let seconds = if wish == Vec2::ZERO {
            SLOW_DOWN_SECONDS
        } else {
            SPEED_UP_SECONDS
        };
        let control = if self.airborne { AIR_CONTROL } else { 1.0 };
        let change = (wish * speed - flat).clamp_length_max(control * speed / seconds * dt);
        let mut flat = flat + change;
        let here = vec2(self.feet.x, self.feet.z);
        let radius = BODY_RADIUS * unit;
        let (mut place, pushed) = map.push_out(here + flat * dt, radius, self.feet.y);
        for way in pushed {
            // Nothing more of its speed goes into what it walked into.
            flat -= way * flat.dot(way).min(0.0);
        }
        // Wedged into a nook too narrow for it, it stays where it was.
        if map.clearance(place, radius, self.feet.y) < -WEDGE_TOLERANCE * unit {
            place = here;
            flat = Vec2::ZERO;
        }
        if !self.airborne {
            self.walked += place.distance(here);
        }
        self.feet = vec3(place.x, self.feet.y, place.y);
        self.velocity = vec3(flat.x, self.velocity.y, flat.y);
        // The bob grows and dies away with the walking pace, on its feet.
        let pace = if self.airborne {
            0.0
        } else {
            (flat.length() / speed.max(f32::EPSILON)).min(1.0)
        };
        let fade = dt / BOB_FADE_SECONDS;
        self.bobbing += (pace - self.bobbing).clamp(-fade, fade);
        // Up and down.
        self.jump_wanted = if keys.jump {
            Some(0.0)
        } else {
            self.jump_wanted
                .map(|waited| waited + dt)
                .filter(|&waited| waited <= JUMP_BUFFER_SECONDS)
        };
        let ground = map.ground(place, radius, self.feet.y);
        if !self.airborne {
            match ground {
                Some(height) if self.feet.y - height <= STEP_UP * unit => {
                    self.move_feet_to(height);
                    self.since_ground = 0.0;
                    self.jumped = false;
                }
                // Walked off something, or off the table's edge.
                _ => {
                    self.airborne = true;
                    self.velocity.y = 0.0;
                }
            }
        }
        let can_jump = !self.jumped && (!self.airborne || self.since_ground <= COYOTE_SECONDS);
        if self.jump_wanted.is_some() && can_jump {
            self.velocity.y = (2.0 * GRAVITY * JUMP_HEIGHT).sqrt() * unit;
            self.airborne = true;
            self.jumped = true;
            self.jump_wanted = None;
        }
        if self.airborne {
            self.since_ground += dt;
            let pull = if self.velocity.y < 0.0 {
                FALL_PULL
            } else {
                1.0
            };
            let gravity = pull * GRAVITY * unit;
            // Exactly as a thrown thing moves, so a jump reaches its full
            // height at any frame rate.
            self.feet.y += (self.velocity.y - gravity * dt / 2.0) * dt;
            self.velocity.y -= gravity * dt;
            let ground = map.ground(place, radius, self.feet.y);
            let landing = ground.filter(|&height| self.feet.y <= height && self.velocity.y <= 0.0);
            if let Some(height) = landing {
                self.move_feet_to(height);
                self.velocity.y = 0.0;
                self.airborne = false;
                self.tucked = false;
                self.since_ground = 0.0;
                self.jumped = false;
            }
        }
        self.set_eye();
        if !map.on_table(place) && self.feet.y < map.floor - FALL_DEPTH * unit {
            Walked::Fell
        } else {
            Walked::OnFoot
        }
    }

    /// Crouches or stands for `dt` seconds as `wanted`: the eyes spring
    /// down or up; in the air, crouching tucks the feet up as well, and standing
    /// lets them down again as far as `map` allows.
    fn crouch_for(&mut self, wanted: bool, dt: f32, map: &WalkMap) {
        let target = if wanted { 1.0 } else { 0.0 };
        let pull = CROUCH_SPRING * CROUCH_SPRING * (target - self.crouch)
            - 2.0 * CROUCH_SPRING * self.crouch_speed;
        self.crouch_speed += pull * dt;
        self.crouch = (self.crouch + self.crouch_speed * dt).clamp(0.0, 1.0);
        if !self.airborne || wanted == self.tucked {
            return;
        }
        let tuck = TUCK * self.unit;
        if wanted {
            self.move_feet_to(self.feet.y + tuck);
        } else {
            let here = vec2(self.feet.x, self.feet.z);
            let lowest = map
                .ground(here, BODY_RADIUS * self.unit, self.feet.y)
                .unwrap_or(f32::NEG_INFINITY);
            self.move_feet_to((self.feet.y - tuck).max(lowest));
        }
        self.tucked = wanted;
    }

    /// Moves the feet straight to `height`, leaving the eyes where they
    /// were to glide after them.
    fn move_feet_to(&mut self, height: f32) {
        self.eye_lag -= height - self.feet.y;
        self.feet.y = height;
    }

    /// Where the head is now in its step, from the middle of the eyes:
    /// bounced down at each footfall and swayed to the side, as much as it
    /// is bobbing.
    fn bob(&self) -> Vec3 {
        let step = PI * self.walked / (STEP_LENGTH * self.unit);
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let right = vec3(cos_yaw, 0.0, -sin_yaw);
        let down = BOB_DEPTH * (1.0 - step.sin().abs());
        let side = BOB_SWAY * step.sin();
        (right * side - Vec3::Y * down) * self.unit * self.bobbing
    }

    /// Puts its eyes over its feet, as high as its crouch allows, moved by
    /// the step, and behind by any lag.
    fn set_eye(&mut self) {
        let eyes = self.eye_height * (1.0 + (CROUCH_EYES - 1.0) * self.crouch);
        self.eye = self.feet + Vec3::Y * (eyes + self.eye_lag) + self.bob();
    }

    /// Turns the head by how far the captured mouse moved to `pointer`
    /// since the last call, `sensitivity` times the usual, and returns how
    /// far it moved, in points. The first call after a capture only notes
    /// where it is, the first moves after it are ignored, and a single
    /// frame's move is held to `MAX_LOOK_STEP`. While the figure grips
    /// something, the mouse moves the view freely within `GRIP_LIMIT` of
    /// where it was, and past it only a little, against a spring.
    pub fn look(&mut self, pointer: Vec2, sensitivity: f32) -> Vec2 {
        let moved = self.last.map(|last| pointer - last);
        self.last = Some(pointer);
        let Some(moved) = moved.filter(|moved| *moved != Vec2::ZERO) else {
            return Vec2::ZERO;
        };
        if self.settling > 0 {
            self.settling -= 1;
            return Vec2::ZERO;
        }
        let moved = moved.clamp_length_max(MAX_LOOK_STEP);
        let turn = LOOK_SENSITIVITY * sensitivity;
        let limit = FRAC_PI_2 - LOOK_MARGIN;
        match self.grip.as_mut() {
            Some(grip) if grip.holding => {
                // Past this the give is all but used up anyway, and a push
                // stored beyond it would only keep the view from springing
                // back in time.
                let most = Vec2::splat(GRIP_LIMIT + GRIP_PUSH_MOST * GRIP_GIVE);
                grip.push = (grip.push - moved * turn).clamp(-most, most);
            }
            // Let go and springing back, the view turns with the mouse at
            // once, the spring easing out on top.
            Some(grip) => {
                grip.yaw -= moved.x * turn;
                grip.pitch = (grip.pitch - moved.y * turn).clamp(-limit, limit);
            }
            None => {
                self.yaw -= moved.x * turn;
                self.pitch = (self.pitch - moved.y * turn).clamp(-limit, limit);
            }
        }
        moved
    }

    /// Grips something: from now the mouse pushes the view against a
    /// spring rather than turning it (see `look`).
    pub fn grip(&mut self) {
        let (yaw, pitch) = match self.grip {
            // Gripped again while springing back: from where it was.
            Some(grip) => (grip.yaw, grip.pitch),
            None => (self.yaw, self.pitch),
        };
        let push = self.grip.map_or(Vec2::ZERO, |grip| grip.push);
        self.grip = Some(Grip {
            yaw,
            pitch,
            push,
            holding: true,
        });
    }

    /// Lets go of what it grips; the view springs back to where it was.
    pub fn let_go(&mut self) {
        if let Some(grip) = self.grip.as_mut() {
            grip.holding = false;
        }
    }

    /// Whether it grips something.
    pub fn gripping(&self) -> bool {
        self.grip.is_some_and(|grip| grip.holding)
    }

    /// How hard a gripped view leans on the edge of its room sideways, the
    /// way the mouse went to get there: 0 short of the edge, 1 at it, and up
    /// to `1 + GRIP_PUSH_MOST` pushed into the give past it; positive to
    /// the right. Held at the edge, a valve keeps turning.
    pub fn grip_lean(&self) -> f32 {
        let Some(grip) = self.grip.filter(|grip| grip.holding) else {
            return 0.0;
        };
        let side = grip.push.x.abs();
        if side < GRIP_EDGE_SHARE * GRIP_LIMIT {
            return 0.0;
        }
        let into_give = ((side - GRIP_LIMIT) / GRIP_GIVE).clamp(0.0, GRIP_PUSH_MOST);
        // The push is the other way to the mouse.
        -grip.push.x.signum() * (1.0 + into_give)
    }

    /// Springs a gripped view pushed past `GRIP_LIMIT` back to it, or a
    /// let-go view back to where it was, for `dt` seconds, and turns the
    /// head to it.
    fn spring_view(&mut self, dt: f32) {
        let Some(mut grip) = self.grip else {
            return;
        };
        let seconds = if grip.holding {
            GRIP_SPRING_SECONDS
        } else {
            GRIP_RETURN_SECONDS
        };
        let decay = (-dt.max(0.0) / seconds).exp();
        // Held, the view stays anywhere within the limit; let go, it goes
        // all the way back.
        let rest = if grip.holding {
            grip.push
                .clamp(Vec2::splat(-GRIP_LIMIT), Vec2::splat(GRIP_LIMIT))
        } else {
            Vec2::ZERO
        };
        grip.push = rest + (grip.push - rest) * decay;
        let held = vec2(grip_view(grip.push.x), grip_view(grip.push.y));
        self.yaw = grip.yaw + held.x;
        self.pitch = grip.pitch + held.y;
        let settled = !grip.holding && grip.push.length() < GRIP_SETTLED;
        self.grip = (!settled).then_some(grip);
        if settled {
            self.yaw = grip.yaw;
            self.pitch = grip.pitch;
        }
    }

    /// Forgets the mouse, which is let go; once captured again, its first
    /// moves are ignored while the capture settles.
    pub fn release(&mut self) {
        self.last = None;
        self.settling = SETTLE_MOVES;
    }

    /// The camera at its eyes.
    pub fn camera(&self) -> Camera3D {
        Camera3D {
            position: self.eye,
            target: self.eye + self.forward(),
            up: Vec3::Y,
            fovy: FIELD_OF_VIEW,
            z_near: NEAR,
            z_far: FAR,
            ..Default::default()
        }
    }
}

/// What the crosshair is on, for drawing it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Aim {
    /// The caption under it: what is aimed at and what E does, or what to
    /// do with what is in use; nothing if it is on nothing.
    pub caption: Option<&'static str>,
    /// Whether something is in use, gripped, held or carried.
    pub using: bool,
}

/// Draws the crosshair in the middle of a window `width` by `height`, grown
/// into a ring over something within reach and filled while it is in use,
/// with `aim`'s caption under it, and the hint at the bottom, saying a
/// click captures the mouse while `waiting` for one.
pub fn draw_view(pen: &mut Pen, width: f32, height: f32, waiting: bool, aim: Aim) {
    let middle = vec2(width, height) / 2.0;
    if aim.caption.is_some() || aim.using {
        let ring = |radius: f32, color: Color| {
            for piece in 0..AIM_RING_PIECES {
                let angle = |at: usize| std::f32::consts::TAU * at as f32 / AIM_RING_PIECES as f32;
                let from = middle + Vec2::from_angle(angle(piece)) * radius;
                let to = middle + Vec2::from_angle(angle(piece + 1)) * radius;
                pen.line(from, to, AIM_RING_THICKNESS, color);
            }
        };
        ring(AIM_RING + AIM_RING_THICKNESS / 2.0, CROSSHAIR_SHADE);
        ring(AIM_RING, CROSSHAIR);
        if aim.using {
            pen.circle(middle, AIM_RING / 2.0, CROSSHAIR);
        }
    }
    pen.circle(middle, CROSSHAIR_RING, CROSSHAIR_SHADE);
    pen.circle(middle, CROSSHAIR_RADIUS, CROSSHAIR);
    if let Some(caption) = aim.caption.filter(|caption| !caption.is_empty()) {
        let text = pen.measure(caption, CAPTION_SIZE);
        let baseline = middle.y + CAPTION_BELOW;
        let left = middle.x - text / 2.0;
        let shade = Rect::new(
            left - HINT_PAD,
            baseline - CAPTION_SIZE - HINT_PAD / 2.0,
            text + 2.0 * HINT_PAD,
            CAPTION_SIZE + 2.0 * HINT_PAD,
        );
        pen.round_rect(shade, shade.h / 2.0, BACKING);
        pen.text(caption, left, baseline, CAPTION_SIZE, CROSSHAIR);
    }
    let hint = if waiting { WAITING_HINT } else { HINT };
    let text = pen.measure(hint, HINT_SIZE);
    let baseline = height - HINT_BOTTOM;
    let left = (width - text) / 2.0;
    let shade = Rect::new(
        left - HINT_PAD,
        baseline - HINT_SIZE - HINT_PAD / 2.0,
        text + 2.0 * HINT_PAD,
        HINT_SIZE + 2.0 * HINT_PAD,
    );
    pen.round_rect(shade, shade.h / 2.0, BACKING);
    pen.text(hint, left, baseline, HINT_SIZE, QUIET);
}

/// How far a gripped view turns for a `push` of the mouse, in radians:
/// all of it within `GRIP_LIMIT`, and past it a little more, never past
/// `GRIP_GIVE` further.
fn grip_view(push: f32) -> f32 {
    let past = push.abs() - GRIP_LIMIT;
    if past <= 0.0 {
        push
    } else {
        push.signum() * (GRIP_LIMIT + GRIP_GIVE * (past / GRIP_GIVE).tanh())
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/first_person.rs"]
mod tests;
