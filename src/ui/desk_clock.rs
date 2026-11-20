//! The desk clock's time, and what pressing it does.
//!
//! The clock shows the computer's time, in its own time zone, but keeps
//! time with the engine: in slow motion its hands slow with it and it
//! falls behind, and while the engine is paused it stops. Back at full
//! speed it catches up, its hands running fast until it shows the
//! computer's time again, faster the further behind it is.
//!
//! A click on the clock turns slow motion on or off; held down for
//! `LONG_PRESS_SECONDS`, it pauses or resumes the engine instead, and
//! letting go then does nothing more.
//!
//! The time zone comes from the computer natively, and from the page in
//! the browser (see `web/bridge.js`).

use std::f64::consts::TAU;

/// Seconds the clock must be held down for a long press.
pub const LONG_PRESS_SECONDS: f32 = 0.6;
/// Seconds the clock takes to make up most of its lag at full speed: the
/// further behind, the faster it catches up.
const CATCH_UP_SECONDS: f64 = 1.5;
/// Least lag the clock makes up a second at full speed, in seconds: its
/// hands run at least this much faster than the time.
const MIN_CATCH_UP_RATE: f64 = 2.0;
/// Seconds between asks for the time zone, which changes with daylight
/// saving time.
const ZONE_REFRESH_SECONDS: f64 = 60.0;
/// Seconds in a minute.
const SECONDS_PER_MINUTE: f64 = 60.0;
/// Seconds in an hour.
const SECONDS_PER_HOUR: f64 = 3600.0;
/// Seconds once round the dial for the hour hand: twelve hours.
const SECONDS_PER_DIAL: f64 = 12.0 * SECONDS_PER_HOUR;
/// Seconds in a day.
const SECONDS_PER_DAY: f64 = 24.0 * SECONDS_PER_HOUR;

/// Where the hands point, in radians clockwise from 12.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HandAngles {
    /// The hour hand.
    pub hour: f32,
    /// The minute hand.
    pub minute: f32,
    /// The second hand, which ticks once a second, as a quartz clock's
    /// does.
    pub second: f32,
}

/// Where the hands point at `seconds` past midnight.
pub fn hand_angles(seconds: f64) -> HandAngles {
    let seconds = seconds.rem_euclid(SECONDS_PER_DAY);
    let share = |period: f64, of: f64| (of.rem_euclid(period) / period * TAU) as f32;
    HandAngles {
        hour: share(SECONDS_PER_DIAL, seconds),
        minute: share(SECONDS_PER_HOUR, seconds),
        second: share(SECONDS_PER_MINUTE, seconds.floor()),
    }
}

/// What a press on the clock came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PressEnd {
    /// Let go before it was a long press: a click.
    Click,
    /// Let go after a long press, which already did what it does.
    AfterLong,
}

/// The clock: how far behind the computer's time it runs, the time zone,
/// and a press under way.
#[derive(Clone, Debug, Default)]
pub struct DeskClock {
    /// Seconds behind the computer's time.
    lag: f64,
    /// The time zone's offset from UTC in seconds, and the computer's time
    /// when it was asked.
    zone: Option<(f64, f64)>,
    /// Seconds the clock has been held down, and whether that already came
    /// to a long press, while it is.
    press: Option<(f32, bool)>,
}

impl DeskClock {
    /// A clock showing the computer's time, not pressed.
    pub fn new() -> Self {
        Self::default()
    }

    /// Moves the clock on by `seconds` of the computer's time while the
    /// engine runs at `pace` of full speed (0 while paused): it falls
    /// behind below full speed and catches up at it.
    pub fn advance(&mut self, seconds: f64, pace: f64) {
        let seconds = seconds.max(0.0);
        if pace < 1.0 {
            self.lag += (1.0 - pace.max(0.0)) * seconds;
        } else {
            let rate = (self.lag / CATCH_UP_SECONDS).max(MIN_CATCH_UP_RATE);
            self.lag = (self.lag - rate * seconds).max(0.0);
        }
    }

    /// Seconds behind the computer's time.
    #[cfg(test)]
    pub fn lag(&self) -> f64 {
        self.lag
    }

    /// Where the hands point at `utc`, the computer's time in seconds since
    /// 1970 in UTC, in the computer's time zone.
    pub fn hands(&mut self, utc: f64) -> HandAngles {
        let stale = self
            .zone
            .is_none_or(|(_, asked)| (utc - asked).abs() >= ZONE_REFRESH_SECONDS);
        if stale {
            self.zone = Some((zone_offset(utc), utc));
        }
        let offset = self.zone.map_or(0.0, |(offset, _)| offset);
        hand_angles(utc + offset - self.lag)
    }

    /// Starts a press on the clock.
    pub fn press(&mut self) {
        self.press = Some((0.0, false));
    }

    /// Whether the clock is held down.
    #[cfg(test)]
    pub fn pressed(&self) -> bool {
        self.press.is_some()
    }

    /// Holds the press for `seconds` more, and says whether that made it a
    /// long press just now, which happens once a press.
    pub fn hold(&mut self, seconds: f32) -> bool {
        let Some((held, long)) = &mut self.press else {
            return false;
        };
        *held += seconds.max(0.0);
        let now_long = !*long && *held >= LONG_PRESS_SECONDS;
        *long |= now_long;
        now_long
    }

    /// Ends the press, if there is one, and says what it came to.
    pub fn release(&mut self) -> Option<PressEnd> {
        self.press.take().map(|(_, long)| {
            if long {
                PressEnd::AfterLong
            } else {
                PressEnd::Click
            }
        })
    }
}

/// The computer's time zone's offset from UTC at `utc`, in seconds east.
#[cfg(target_arch = "wasm32")]
fn zone_offset(_utc: f64) -> f64 {
    // SAFETY: the page's function takes nothing and returns a number.
    f64::from(unsafe { page::motusboltz_zone_offset() })
}

/// What the page tells the simulation about the time (see `web/bridge.js`).
#[cfg(target_arch = "wasm32")]
mod page {
    // Imported from the page, as miniquad's own calls into `gl.js` are.
    #[link(wasm_import_module = "env")]
    unsafe extern "C" {
        /// The time zone's offset from UTC, in seconds east.
        pub fn motusboltz_zone_offset() -> i32;
    }
}

/// The computer's time zone's offset from UTC at `utc`, in seconds east.
#[cfg(unix)]
fn zone_offset(utc: f64) -> f64 {
    use std::ffi::{
        c_char,
        c_int,
        c_long, //
    };

    /// The C library's broken-down time (`struct tm`), as glibc, musl and
    /// macOS lay it out, with its offset from UTC.
    #[repr(C)]
    struct Tm {
        /// Seconds, minutes, hours, day of the month, month, year, day of
        /// the week, day of the year and daylight saving time, in order.
        fields: [c_int; 9],
        /// Seconds east of UTC.
        gmtoff: c_long,
        /// The zone's abbreviation.
        zone: *const c_char,
    }

    unsafe extern "C" {
        /// Reads the time zone from the system (`<time.h>`).
        fn tzset();
        /// Breaks `time` down in the local time zone into `out`
        /// (`<time.h>`; `time_t` is a `long` on these systems).
        fn localtime_r(time: *const c_long, out: *mut Tm) -> *mut Tm;
    }

    let time = utc as c_long;
    let mut out = Tm {
        fields: [0; 9],
        gmtoff: 0,
        zone: std::ptr::null(),
    };
    // SAFETY: both pointers are to live values of the right types, and
    // nothing else in the program changes the time zone.
    let broken = unsafe {
        tzset();
        localtime_r(&time, &mut out)
    };
    if broken.is_null() {
        0.0
    } else {
        out.gmtoff as f64
    }
}

/// The computer's time zone's offset from UTC now, in seconds east.
#[cfg(windows)]
fn zone_offset(_utc: f64) -> f64 {
    /// Windows' `TIME_ZONE_INFORMATION` (`<timezoneapi.h>`).
    #[repr(C)]
    struct ZoneInformation {
        /// Minutes west of UTC.
        bias: i32,
        /// The standard zone's name.
        standard_name: [u16; 32],
        /// When standard time starts (a `SYSTEMTIME`).
        standard_date: [u16; 8],
        /// Minutes added to the bias in standard time.
        standard_bias: i32,
        /// The daylight zone's name.
        daylight_name: [u16; 32],
        /// When daylight saving time starts (a `SYSTEMTIME`).
        daylight_date: [u16; 8],
        /// Minutes added to the bias in daylight saving time.
        daylight_bias: i32,
    }

    /// The zone is in standard time.
    const STANDARD: u32 = 1;
    /// The zone is in daylight saving time.
    const DAYLIGHT: u32 = 2;
    /// What the zone's call returns when it fails.
    const FAILED: u32 = u32::MAX;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        /// Reads the time zone into `out` and says which part of the year
        /// it is in.
        fn GetTimeZoneInformation(out: *mut ZoneInformation) -> u32;
    }

    let mut out = ZoneInformation {
        bias: 0,
        standard_name: [0; 32],
        standard_date: [0; 8],
        standard_bias: 0,
        daylight_name: [0; 32],
        daylight_date: [0; 8],
        daylight_bias: 0,
    };
    // SAFETY: the pointer is to a live value of the right layout.
    let part = unsafe { GetTimeZoneInformation(&mut out) };
    let minutes_west = match part {
        FAILED => 0,
        STANDARD => out.bias + out.standard_bias,
        DAYLIGHT => out.bias + out.daylight_bias,
        _ => out.bias,
    };
    -f64::from(minutes_west) * SECONDS_PER_MINUTE
}

/// Elsewhere, UTC.
#[cfg(not(any(unix, windows, target_arch = "wasm32")))]
fn zone_offset(_utc: f64) -> f64 {
    0.0
}

#[cfg(test)]
#[path = "../../tests/unit/ui/desk_clock.rs"]
mod tests;
