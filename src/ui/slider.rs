//! Horizontal sliders for the hot and cold reservoir temperatures and the
//! helium concentration, and the switch under them that puts the
//! regenerator in the opening.

use macroquad::prelude::{
    Color,
    MouseButton,
    Rect,
    WHITE,
    is_mouse_button_down,
    is_mouse_button_pressed,
    mouse_position,
    screen_width,
    vec2, //
};

use crate::sim::reservoir::{
    MAX_COLD_KELVIN,
    MAX_HOT_KELVIN,
    MIN_COLD_KELVIN,
    MIN_HOT_KELVIN, //
};
use crate::sim::units::{
    MAX_CONCENTRATION,
    MIN_CONCENTRATION, //
};
use crate::ui::button::whiten;
use crate::ui::layout::{
    CONTROL_PADDING_X,
    CONTROL_WIDTH,
    control_left, //
};
use crate::ui::motion::{
    advance,
    approach,
    faded,
    smooth, //
};
use crate::ui::pen::{
    Pen,
    Weight,
    grown, //
};

/// Length of a track.
const SLIDER_WIDTH: f32 = CONTROL_WIDTH;
/// Thickness of the filled part of a lit track.
const LIT_TRACK_HEIGHT: f32 = 11.0;
/// How much a lit track's color moves toward white.
const LIT_WHITENING: f32 = 0.3;
/// How far the halo of a lit track reaches past it.
const HALO: f32 = 4.0;
/// Opacity of the halo of a lit track.
const HALO_OPACITY: f32 = 0.35;
/// How much a lit handle grows, in points.
const LIT_HANDLE_GROWTH: f32 = 2.0;
/// Seconds for the glow to close all but 1/e of its way.
const GLOW_SECONDS: f32 = 0.07;
/// Thickness of a track.
const SLIDER_TRACK_HEIGHT: f32 = 6.0;
/// Radius of the handle.
const SLIDER_HANDLE_RADIUS: f32 = 10.0;
/// Width of the colored ring around the white handle.
const HANDLE_RING: f32 = 3.0;
/// How far above and below a track a press starts a drag.
const SLIDER_HIT_HALF_HEIGHT: f32 = 22.0;
/// How far past a track's ends a press starts a drag.
const SLIDER_HIT_MARGIN_X: f32 = 14.0;
/// Room above a track for the label and value.
///
/// The camera ignores the mouse over it and the range below.
const AREA_ABOVE: f32 = 40.0;
/// Room below a track for the range.
const AREA_BELOW: f32 = 32.0;
/// Text size of the label and value.
const LABEL_SIZE: f32 = 20.0;
/// From a track's middle up to the baseline of the label and value.
const LABEL_OFFSET_Y: f32 = 16.0;
/// Text size of the range.
const RANGE_SIZE: f32 = 15.0;
/// From a track's middle down to the baseline of the range.
const RANGE_OFFSET_Y: f32 = 26.0;
/// Middle of the hot track, near the top of the column.
const HOT_Y: f32 = 68.0;
/// Distance between the middles of neighboring tracks.
const SLIDER_SPACING: f32 = 90.0;
/// Middle of the cold track.
const COLD_Y: f32 = HOT_Y + SLIDER_SPACING;
/// Middle of the helium track.
const CONCENTRATION_Y: f32 = COLD_Y + SLIDER_SPACING;
/// From the helium track's middle to the middle of the switch's row, which
/// has no line above it, so it sits closer than the next slider would.
const SWITCH_SPACING: f32 = 64.0;
/// Middle of the regenerator switch's row.
const SWITCH_Y: f32 = CONCENTRATION_Y + SWITCH_SPACING;
/// Width of the switch's track.
const SWITCH_WIDTH: f32 = 46.0;
/// Height of the switch's track.
const SWITCH_HEIGHT: f32 = 26.0;
/// Gap between the knob and the edge of the switch's track.
const KNOB_INSET: f32 = 3.0;
/// Seconds the knob takes to slide across.
const SWITCH_SECONDS: f32 = 0.18;
/// How far above and below the row's middle a press flips the switch; the
/// row ends there.
const SWITCH_HIT_HALF_HEIGHT: f32 = 18.0;
/// Name beside the switch.
const SWITCH_LABEL: &str = "Regenerator";
/// The switch's key, after its name, as the buttons show theirs.
const SWITCH_KEY: &str = "E";
/// Gap between the name and the key.
const SWITCH_KEY_GAP: f32 = 8.0;
/// Color of the switch's track while on: brass, like the mesh's wire.
const SWITCH_COLOR: Color = Color::from_rgba(226, 188, 122, 255);
/// Color of the empty part of a track.
const TRACK_COLOR: Color = Color::new(0.27, 0.33, 0.40, 0.9);
/// Color of the value.
const VALUE_COLOR: Color = WHITE;
/// Color of the label.
const LABEL_COLOR: Color = Color::from_rgba(185, 198, 212, 255);
/// Color of the range.
const RANGE_TEXT_COLOR: Color = Color::from_rgba(120, 138, 158, 255);

/// The hot-side temperature.
const HOT: Setting = Setting {
    label: "Hot side",
    unit: "K",
    color: Color::from_rgba(255, 160, 90, 255),
    min: MIN_HOT_KELVIN,
    max: MAX_HOT_KELVIN,
    step: 1.0,
    precision: 0,
    y: HOT_Y,
};
/// The cold-side temperature.
const COLD: Setting = Setting {
    label: "Cold side",
    unit: "K",
    color: Color::from_rgba(95, 190, 255, 255),
    min: MIN_COLD_KELVIN,
    max: MAX_COLD_KELVIN,
    step: 1.0,
    precision: 0,
    y: COLD_Y,
};
/// The helium concentration.
const CONCENTRATION: Setting = Setting {
    label: "Helium",
    unit: "mol/L",
    color: Color::from_rgba(180, 224, 209, 255),
    min: MIN_CONCENTRATION,
    max: MAX_CONCENTRATION,
    step: 0.005,
    precision: 3,
    y: CONCENTRATION_Y,
};

/// The top of the first slider's label and the bottom of the switch's
/// note, in screen points.
pub fn column_span() -> (f32, f32) {
    (HOT_Y - AREA_ABOVE, SWITCH_Y + SWITCH_HIT_HALF_HEIGHT)
}

/// What a slider sets and where it sits.
struct Setting {
    /// Name above the track.
    label: &'static str,
    /// Unit of the values.
    unit: &'static str,
    /// Color of the filled track and the handle's ring.
    color: Color,
    /// Value at the left end.
    min: f32,
    /// Value at the right end.
    max: f32,
    /// Values snap to multiples of this above `min`.
    step: f32,
    /// Decimal places shown.
    precision: usize,
    /// Middle of the track, in screen points from the top.
    y: f32,
}

/// What happened to a slider this frame.
#[derive(Default)]
pub struct SliderUpdate {
    /// The value changed.
    pub changed: bool,
    /// A drag ended.
    pub released: bool,
}

/// Horizontal slider dragged with the left mouse button.
pub struct Slider {
    /// What it sets.
    setting: &'static Setting,
    /// The value, a multiple of the step above the minimum.
    value: f32,
    /// Whether a drag that started on the track is held.
    dragging: bool,
    /// How lit the slider is, from 0 at rest to 1 under the pointer or
    /// while dragged.
    glow: f32,
}

impl Slider {
    /// The hot-side temperature, at `value` kelvin.
    pub fn hot_temperature(value: f32) -> Self {
        Self::new(&HOT, value)
    }

    /// The cold-side temperature, at `value` kelvin.
    pub fn cold_temperature(value: f32) -> Self {
        Self::new(&COLD, value)
    }

    /// The helium concentration, at `value` mol/L.
    pub fn concentration(value: f32) -> Self {
        Self::new(&CONCENTRATION, value)
    }

    /// A slider for `setting` at `value`.
    fn new(setting: &'static Setting, value: f32) -> Self {
        let mut slider = Self {
            setting,
            value: setting.min,
            dragging: false,
            glow: 0.0,
        };
        slider.set_value(value);
        slider
    }

    /// `value` within the range, snapped to a step.
    fn quantized(&self, value: f32) -> f32 {
        let Setting { min, max, step, .. } = *self.setting;
        (min + ((value.clamp(min, max) - min) / step).round() * step).clamp(min, max)
    }

    /// The value.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Sets the value, snapped to a step, and drops any drag.
    pub fn set_value(&mut self, value: f32) {
        self.value = self.quantized(value);
        self.dragging = false;
    }

    /// Left end of the track.
    fn origin_x(&self) -> f32 {
        control_left(screen_width())
    }

    /// Whether the pointer is within `margin` of the track's ends and
    /// between `above` and `below` its middle.
    fn pointer_within(&self, margin: f32, above: f32, below: f32) -> bool {
        let (x, y) = mouse_position();
        let origin = self.origin_x();
        (origin - margin..=origin + SLIDER_WIDTH + margin).contains(&x)
            && (self.setting.y - above..=self.setting.y + below).contains(&y)
    }

    /// The value under `x`, on a track starting at `origin`.
    fn value_at_x(&self, x: f32, origin: f32) -> f32 {
        let Setting { min, max, .. } = *self.setting;
        self.quantized(min + ((x - origin) / SLIDER_WIDTH).clamp(0.0, 1.0) * (max - min))
    }

    /// Handles mouse input.
    ///
    /// A drag starts only on the track; the value follows the pointer until
    /// the button is released.
    pub fn update(&mut self) -> SliderUpdate {
        if !is_mouse_button_down(MouseButton::Left) {
            let released = self.dragging;
            self.dragging = false;
            return SliderUpdate {
                released,
                ..Default::default()
            };
        }
        let hit = SLIDER_HIT_HALF_HEIGHT;
        if is_mouse_button_pressed(MouseButton::Left)
            && self.pointer_within(SLIDER_HIT_MARGIN_X, hit, hit)
        {
            self.dragging = true;
        }
        if !self.dragging {
            return SliderUpdate::default();
        }
        let next = self.value_at_x(mouse_position().0, self.origin_x());
        let changed = next != self.value;
        self.value = next;
        SliderUpdate {
            changed,
            released: false,
        }
    }

    /// Moves the glow on by `dt` seconds: up while the slider is `shown`
    /// and dragged or under the pointer, down otherwise.
    pub fn animate(&mut self, shown: bool, dt: f32) {
        let hit = SLIDER_HIT_HALF_HEIGHT;
        let lit = shown && (self.dragging || self.pointer_within(SLIDER_HIT_MARGIN_X, hit, hit));
        let target = if lit { 1.0 } else { 0.0 };
        self.glow = approach(self.glow, target, dt, GLOW_SECONDS);
    }

    /// Whether the camera should ignore the mouse, while dragging or with
    /// the pointer over this slider.
    pub fn blocks_camera(&self) -> bool {
        self.dragging || self.pointer_within(CONTROL_PADDING_X, AREA_ABOVE, AREA_BELOW)
    }

    /// The value with its unit, as shown beside the label.
    fn value_text(&self, value: f32) -> String {
        format!("{:.*} {}", self.setting.precision, value, self.setting.unit)
    }

    /// Draws the label and value above the track and the range below it.
    ///
    /// There is no background, so the scene shows around it.
    pub fn draw(&self, pen: &mut Pen) {
        let Setting {
            label,
            color,
            min,
            max,
            y,
            ..
        } = *self.setting;
        let x = self.origin_x();
        let right = x + SLIDER_WIDTH;
        let handle_x = x + (self.value - min) / (max - min) * SLIDER_WIDTH;
        let label_y = y - LABEL_OFFSET_Y;
        pen.heading(label, x, label_y, LABEL_SIZE, LABEL_COLOR);
        let value = self.value_text(self.value);
        let value_width = pen.measure_weight(&value, LABEL_SIZE, Weight::Semibold);
        pen.heading(
            &value,
            right - value_width,
            label_y,
            LABEL_SIZE,
            VALUE_COLOR,
        );
        let (top, thickness) = (y - SLIDER_TRACK_HEIGHT / 2.0, SLIDER_TRACK_HEIGHT);
        let track = Rect::new(x, top, SLIDER_WIDTH, thickness);
        pen.round_rect(track, thickness / 2.0, TRACK_COLOR);
        // Lit, the filled part thickens and brightens inside a soft halo.
        let glow = self.glow;
        let lit = whiten(color, LIT_WHITENING * glow);
        let fill_thickness = thickness + (LIT_TRACK_HEIGHT - thickness) * glow;
        let fill = Rect::new(x, y - fill_thickness / 2.0, handle_x - x, fill_thickness);
        let halo = grown(fill, HALO * glow);
        pen.round_rect(halo, halo.h / 2.0, faded(color, HALO_OPACITY * glow));
        pen.round_rect(fill, fill_thickness / 2.0, lit);
        let handle = vec2(handle_x, y);
        let radius = SLIDER_HANDLE_RADIUS + LIT_HANDLE_GROWTH * glow;
        pen.circle(handle, radius, lit);
        pen.circle(handle, radius - HANDLE_RING, WHITE);
        let range_y = y + RANGE_OFFSET_Y;
        let (low, high) = (self.value_text(min), self.value_text(max));
        pen.text(&low, x, range_y, RANGE_SIZE, RANGE_TEXT_COLOR);
        pen.text_right(&high, right, range_y, RANGE_SIZE, RANGE_TEXT_COLOR);
    }
}

/// The on-off switch for the regenerator, in a row under the helium slider,
/// flipped with a click anywhere on the row.
pub struct Switch {
    /// Whether it is on.
    on: bool,
    /// How far the knob has slid toward on, from 0 to 1.
    slide: f32,
    /// How lit the switch is, from 0 at rest to 1 under the pointer.
    glow: f32,
}

impl Switch {
    /// The regenerator's switch, on if `on`, with its knob already there.
    pub fn regenerator(on: bool) -> Self {
        Self {
            on,
            slide: if on { 1.0 } else { 0.0 },
            glow: 0.0,
        }
    }

    /// Whether it is on.
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Turns it on or off from elsewhere (a key or a demo); the knob slides
    /// over.
    pub fn set_on(&mut self, on: bool) {
        self.on = on;
    }

    /// Whether the pointer is over the row, within `margin` of its ends and
    /// between `above` and `below` its middle.
    fn pointer_within(&self, margin: f32, above: f32, below: f32) -> bool {
        let (x, y) = mouse_position();
        let left = control_left(screen_width());
        (left - margin..=left + CONTROL_WIDTH + margin).contains(&x)
            && (SWITCH_Y - above..=SWITCH_Y + below).contains(&y)
    }

    /// Whether the pointer is where a press flips the switch.
    fn under_pointer(&self) -> bool {
        let hit = SWITCH_HIT_HALF_HEIGHT;
        self.pointer_within(SLIDER_HIT_MARGIN_X, hit, hit)
    }

    /// Handles mouse input; returns whether a click flipped the switch.
    pub fn update(&mut self) -> bool {
        let flipped = is_mouse_button_pressed(MouseButton::Left) && self.under_pointer();
        self.on ^= flipped;
        flipped
    }

    /// Moves the knob and the glow on by `dt` seconds: the knob toward
    /// where the switch is, the glow up while it is `shown` and under the
    /// pointer.
    pub fn animate(&mut self, shown: bool, dt: f32) {
        self.slide = advance(self.slide, self.on, dt, SWITCH_SECONDS);
        let target = if shown && self.under_pointer() {
            1.0
        } else {
            0.0
        };
        self.glow = approach(self.glow, target, dt, GLOW_SECONDS);
    }

    /// Whether the camera should ignore the mouse, with the pointer over
    /// the row.
    pub fn blocks_camera(&self) -> bool {
        let hit = SWITCH_HIT_HALF_HEIGHT;
        self.pointer_within(CONTROL_PADDING_X, hit, hit)
    }

    /// Draws the name with its key, and the switch, on the row.
    pub fn draw(&self, pen: &mut Pen) {
        let x = control_left(screen_width());
        let right = x + SLIDER_WIDTH;
        let y = SWITCH_Y;
        let label_y = y + pen.text_height(SWITCH_LABEL, LABEL_SIZE) / 2.0;
        pen.heading(SWITCH_LABEL, x, label_y, LABEL_SIZE, LABEL_COLOR);
        let key_x = x + pen.measure_weight(SWITCH_LABEL, LABEL_SIZE, Weight::Semibold);
        let key_x = key_x + SWITCH_KEY_GAP;
        pen.text(SWITCH_KEY, key_x, label_y, RANGE_SIZE, RANGE_TEXT_COLOR);
        let track = Rect::new(
            right - SWITCH_WIDTH,
            y - SWITCH_HEIGHT / 2.0,
            SWITCH_WIDTH,
            SWITCH_HEIGHT,
        );
        // Lit, it brightens inside a soft halo, as a slider does.
        let (on, glow) = (smooth(self.slide), self.glow);
        let color = whiten(mixed(TRACK_COLOR, SWITCH_COLOR, on), LIT_WHITENING * glow);
        let halo = grown(track, HALO * glow);
        pen.round_rect(halo, halo.h / 2.0, faded(SWITCH_COLOR, HALO_OPACITY * glow));
        pen.round_rect(track, track.h / 2.0, color);
        let radius = track.h / 2.0 - KNOB_INSET;
        let travel = track.w - track.h;
        let knob = vec2(track.x + track.h / 2.0 + travel * on, y);
        pen.circle(knob, radius, WHITE);
    }
}

/// `from` moved toward `to` by `share`, channel by channel.
fn mixed(from: Color, to: Color, share: f32) -> Color {
    let toward = |a: f32, b: f32| a + (b - a) * share;
    Color::new(
        toward(from.r, to.r),
        toward(from.g, to.g),
        toward(from.b, to.b),
        toward(from.a, to.a),
    )
}

#[cfg(test)]
#[path = "../../tests/unit/ui/slider.rs"]
mod tests;
