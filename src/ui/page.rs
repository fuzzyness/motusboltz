//! What the simulation and the web page hosting it tell each other about
//! first person and full screen (see `web/bridge.js`).
//!
//! A browser captures the mouse, or goes full screen, only when asked from
//! inside the handler of a click, and the simulation sees a click a frame
//! later, too late. So the simulation tells the page, a frame ahead, what
//! the next press on it should do (see `arm`), and the page asks from its
//! own handler.
//!
//! The page knows whether the mouse is captured; the simulation asks it
//! each frame (see `capture` and `take_lost`) instead of keeping a guess of
//! its own, which the browser can make wrong at any moment: Esc, a click
//! away, a refused request. It lets the mouse go only when the simulation
//! asks (see `release`): Macroquad repeats its own idea of the capture to
//! the page every frame, a frame behind the page's, and the page ignores
//! it.
//!
//! Natively there is no page: the simulation captures the mouse and goes
//! full screen itself, and these answer as if every capture were held.

/// Whether the simulation runs in a web page, which captures the mouse and
/// goes full screen for it.
pub const HOSTED: bool = cfg!(target_arch = "wasm32");

// Imported from the page, as miniquad's own calls into `gl.js` are; without
// the module named, the linker looks for them in the program and fails.
#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    /// The page's side of `arm`.
    fn motusboltz_arm(what: i32);
    /// The page's side of `set_first_person`.
    fn motusboltz_first_person(on: i32);
    /// The page's side of `capture`.
    fn motusboltz_capture() -> i32;
    /// The page's side of `take_lost`.
    fn motusboltz_take_lost() -> i32;
    /// The page's side of `full_screen`.
    fn motusboltz_full_screen() -> i32;
    /// The page's side of `release`.
    fn motusboltz_release();
}

/// What the next press of the mouse button on the simulation should do,
/// beyond what the simulation does with it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Arm {
    /// Nothing more.
    #[default]
    Nothing = 0,
    /// Capture the mouse, for looking around in first person (the release
    /// too, for the figure, which is dropped by letting go).
    Capture = 1,
    /// Switch full screen on or off.
    FullScreen = 2,
}

/// How the capture of the mouse stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capture {
    /// Not captured, and not asked for.
    Free,
    /// Asked for, and neither granted nor refused yet.
    Pending,
    /// Captured: the mouse turns the head.
    Held,
}

impl Capture {
    /// The capture as the page counts it: 0 free, 1 pending, 2 held.
    fn from_page(code: i32) -> Self {
        match code {
            1 => Self::Pending,
            2 => Self::Held,
            _ => Self::Free,
        }
    }
}

/// Tells the page what the next press on the simulation should do.
pub fn arm(what: Arm) {
    #[cfg(target_arch = "wasm32")]
    // SAFETY: the page provides the function (or `gl.js` a stand-in that
    // does nothing); it takes a plain integer and returns nothing.
    unsafe {
        motusboltz_arm(what as i32);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = what;
}

/// Tells the page whether first person is on: only then does a lost
/// capture count (see `take_lost`).
pub fn set_first_person(on: bool) {
    #[cfg(target_arch = "wasm32")]
    // SAFETY: as for `arm`.
    unsafe {
        motusboltz_first_person(i32::from(on));
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = on;
}

/// Asks the page to let the mouse go, if it holds it.
pub fn release() {
    #[cfg(target_arch = "wasm32")]
    // SAFETY: as for `arm`; it takes and returns nothing.
    unsafe {
        motusboltz_release();
    }
}

/// How the capture of the mouse stands; natively, always held.
pub fn capture() -> Capture {
    #[cfg(target_arch = "wasm32")]
    // SAFETY: as for `arm`; it takes nothing and returns a plain integer
    // (0 from `gl.js`'s stand-in).
    let code = unsafe { motusboltz_capture() };
    #[cfg(not(target_arch = "wasm32"))]
    let code = 2;
    Capture::from_page(code)
}

/// Whether, in first person, the capture was lost or refused since this
/// was last asked: by Esc, a click away from the simulation, or a browser
/// that would not grant it. Asking clears it; natively, never.
pub fn take_lost() -> bool {
    #[cfg(target_arch = "wasm32")]
    // SAFETY: as for `capture`.
    let lost = unsafe { motusboltz_take_lost() } != 0;
    #[cfg(not(target_arch = "wasm32"))]
    let lost = false;
    lost
}

/// Whether the page shows the simulation full screen; natively, `None`,
/// as the simulation keeps track of its own window.
pub fn full_screen() -> Option<bool> {
    #[cfg(target_arch = "wasm32")]
    // SAFETY: as for `capture`.
    let shown = Some(unsafe { motusboltz_full_screen() } != 0);
    #[cfg(not(target_arch = "wasm32"))]
    let shown = None;
    shown
}

#[cfg(test)]
#[path = "../../tests/unit/ui/page.rs"]
mod tests;
