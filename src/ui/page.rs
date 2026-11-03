//! What the web page hosting the simulation needs to know about first
//! person.
//!
//! A browser captures the mouse only when asked from inside the handler
//! of a click, and the simulation sees a click a frame later, too late. So
//! the simulation tells the page, a frame ahead, when the next press or
//! release of the button should capture the mouse: while the figure is
//! dragged over the table, or the pointer is over the menu's Resume. The
//! page then asks from its own handler. It is also told when first person
//! is on, so a mouse the browser lets go opens the menu instead of the
//! page's hint about clicking the simulation (see `web/engine.html`).
//!
//! Natively there is no page, and these do nothing.

// Imported from the page, as miniquad's own calls into `gl.js` are; without
// the module named, the linker looks for them in the program and fails.
#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    /// The page's side of `arm_capture` (see `web/engine.html`).
    fn motusboltz_arm_capture(armed: i32);
    /// The page's side of `set_first_person`.
    fn motusboltz_first_person(on: i32);
}

/// Tells the page whether the next press or release of the mouse button
/// on the simulation should capture the mouse.
pub fn arm_capture(armed: bool) {
    #[cfg(target_arch = "wasm32")]
    // SAFETY: the page provides the function (or `gl.js` a stand-in that
    // does nothing); it takes a plain integer and returns nothing.
    unsafe {
        motusboltz_arm_capture(i32::from(armed));
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = armed;
}

/// Tells the page whether first person is on.
pub fn set_first_person(on: bool) {
    #[cfg(target_arch = "wasm32")]
    // SAFETY: as for `arm_capture`.
    unsafe {
        motusboltz_first_person(i32::from(on));
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = on;
}
