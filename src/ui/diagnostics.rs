//! Where each frame's time goes, for the diagnostics build only.
//!
//! Built with `--features diagnostics`, the program times the parts of
//! each frame and shows them, with the frame rate, under their own heading
//! at the foot of the readings panel's Advanced view:
//! drawing the scene, and within it the calls that hand the meshes to the
//! graphics card; measuring and drawing the panel, labels and buttons; and
//! the hand-off, from giving Macroquad the frame to the start of the next,
//! which takes in its last drawing calls and the wait for the display.
//! Without the feature, `time` only runs what it is given and nothing is
//! kept, so the program that ships carries none of it.
//!
//! The graphics card cannot be asked how long each part of the scene takes
//! it, so the diagnostics build lets parts be switched off with the number
//! keys, 1 to 6, to see what each saves: the sky, the particles, the
//! flames, mist and sparks, the soft view (the scene drawn straight to the
//! screen instead), the readings panel, and the table. Everywhere else
//! every part is always drawn.

#[cfg(any(test, feature = "diagnostics"))]
use std::cell::{
    Cell,
    RefCell, //
};

/// A part of the picture the diagnostics build can switch off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(any(test, feature = "diagnostics")), allow(dead_code))]
pub enum Piece {
    /// The sky and its clouds.
    Sky,
    /// The particles.
    Particles,
    /// The flames, mist, sparks and the tank's puff.
    Effects,
    /// The soft view: the scene drawn into its own image first.
    SoftView,
    /// The readings panel.
    Panel,
    /// The table.
    Table,
}

/// Each piece that can be switched off and its name, in the order of the
/// number keys that switch them.
#[cfg(any(test, feature = "diagnostics"))]
const PIECES: [(Piece, &str); 6] = [
    (Piece::Sky, "sky"),
    (Piece::Particles, "particles"),
    (Piece::Effects, "effects"),
    (Piece::SoftView, "soft view"),
    (Piece::Panel, "panel"),
    (Piece::Table, "table"),
];

/// A part of a frame that is timed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    /// Drawing the scene, the graphics calls included.
    Scene,
    /// The calls that hand the kept meshes to the graphics card, within
    /// the scene.
    GraphicsCalls,
    /// Measuring and drawing the panel, labels, buttons and menus.
    Overlay,
    /// Measuring every string before drawing, within the overlay.
    Measuring,
    /// Showing the readings panel, painted anew or copied, within the
    /// overlay.
    Panel,
    /// Painting the readings panel anew, within showing it.
    PanelPaint,
    /// Drawing the buttons, sliders and drawers, within the overlay.
    Controls,
    /// Drawing the scene's labels, within the overlay.
    Labels,
    /// Waiting for the graphics card to finish drawing the scene.
    #[cfg_attr(not(any(test, feature = "diagnostics")), allow(dead_code))]
    CardScene,
    /// Waiting for the graphics card to finish the rest of the frame.
    #[cfg_attr(not(any(test, feature = "diagnostics")), allow(dead_code))]
    CardRest,
    /// From giving Macroquad the frame to the start of the next, timed
    /// in the diagnostics build's main loop.
    #[cfg_attr(not(any(test, feature = "diagnostics")), allow(dead_code))]
    HandOff,
}

/// How many sections there are.
#[cfg(any(test, feature = "diagnostics"))]
const SECTIONS: usize = 11;
/// Frames each reading averages over.
#[cfg(any(test, feature = "diagnostics"))]
const WINDOW_FRAMES: u32 = 30;
/// Milliseconds in a second.
#[cfg(any(test, feature = "diagnostics"))]
const MILLISECONDS_PER_SECOND: f64 = 1000.0;

/// The sections' times being added up, and the last reading.
#[cfg(any(test, feature = "diagnostics"))]
#[derive(Default)]
struct Totals {
    /// Each section's time in the current window, in seconds.
    seconds: [f64; SECTIONS],
    /// Frames finished in the current window.
    frames: u32,
    /// Each section's mean milliseconds a frame over the last window.
    reading: Option<[f64; SECTIONS]>,
}

#[cfg(any(test, feature = "diagnostics"))]
thread_local! {
    /// The window's totals, on the window's thread.
    static TOTALS: RefCell<Totals> = RefCell::new(Totals::default());
    /// The pieces switched off, one bit each in `PIECES`' order.
    static OFF: Cell<u8> = const { Cell::new(0) };
}

/// Whether `piece` is drawn: always, but in the diagnostics build where it
/// was switched off.
#[cfg_attr(not(feature = "diagnostics"), inline(always))]
pub fn shown(piece: Piece) -> bool {
    #[cfg(any(test, feature = "diagnostics"))]
    return OFF.with(|off| off.get() & bit(piece) == 0);
    #[cfg(not(any(test, feature = "diagnostics")))]
    {
        let _ = piece;
        true
    }
}

/// `piece`'s bit among those switched off.
#[cfg(any(test, feature = "diagnostics"))]
fn bit(piece: Piece) -> u8 {
    let index = PIECES
        .iter()
        .position(|&(each, _)| each == piece)
        .expect("every piece is listed");
    1 << index
}

/// Switches the `number`th piece, from 1, on or off; other numbers do
/// nothing.
#[cfg(any(test, feature = "diagnostics"))]
pub fn switch(number: usize) {
    if let Some(&(piece, _)) = number.checked_sub(1).and_then(|index| PIECES.get(index)) {
        OFF.with(|off| off.set(off.get() ^ bit(piece)));
    }
}

/// Switches the pieces whose number keys were pressed this frame.
#[cfg(feature = "diagnostics")]
pub fn read_keys() {
    use macroquad::input::{
        KeyCode,
        is_key_pressed, //
    };
    const KEYS: [KeyCode; 6] = [
        KeyCode::Key1,
        KeyCode::Key2,
        KeyCode::Key3,
        KeyCode::Key4,
        KeyCode::Key5,
        KeyCode::Key6,
    ];
    for (index, key) in KEYS.into_iter().enumerate() {
        if is_key_pressed(key) {
            switch(index + 1);
        }
    }
}

/// The names of the pieces switched off, or "none".
#[cfg(any(test, feature = "diagnostics"))]
pub fn switched_off() -> String {
    let names: Vec<&str> = PIECES
        .iter()
        .filter(|&&(piece, _)| !shown(piece))
        .map(|&(_, name)| name)
        .collect();
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.join(", ")
    }
}

/// Runs `part`, timing it under `section` in the diagnostics build.
#[cfg_attr(not(feature = "diagnostics"), inline(always))]
pub fn time<T>(section: Section, part: impl FnOnce() -> T) -> T {
    #[cfg(feature = "diagnostics")]
    {
        let started = macroquad::miniquad::date::now();
        let result = part();
        add(section, macroquad::miniquad::date::now() - started);
        result
    }
    #[cfg(not(feature = "diagnostics"))]
    {
        let _ = section;
        part()
    }
}

/// The time now, to pass to `stop`, in the diagnostics build.
#[cfg_attr(not(feature = "diagnostics"), inline(always))]
pub fn start() -> f64 {
    #[cfg(feature = "diagnostics")]
    return macroquad::miniquad::date::now();
    #[cfg(not(feature = "diagnostics"))]
    0.0
}

/// Adds the time since `started`, from `start`, to `section`, in the
/// diagnostics build.
#[cfg_attr(not(feature = "diagnostics"), inline(always))]
pub fn stop(section: Section, started: f64) {
    #[cfg(feature = "diagnostics")]
    add(section, macroquad::miniquad::date::now() - started);
    #[cfg(not(feature = "diagnostics"))]
    let _ = (section, started);
}

/// In the diagnostics build, hands what Macroquad has batched to the
/// graphics card and waits for the card to finish all it was given,
/// timing the wait under `section`.
///
/// The program otherwise goes on while the card draws, so this costs the
/// diagnostics build some speed; but it shows how long the card takes, and
/// keeps the card's time out of the sections timed after it.
#[cfg_attr(not(feature = "diagnostics"), inline(always))]
pub fn wait_for_card(section: Section) {
    #[cfg(feature = "diagnostics")]
    {
        let started = macroquad::miniquad::date::now();
        // SAFETY: called on the window's thread, between frames' drawing.
        unsafe {
            macroquad::window::get_internal_gl().flush();
            macroquad::miniquad::raw_gl::glFinish();
        }
        add(section, macroquad::miniquad::date::now() - started);
    }
    #[cfg(not(feature = "diagnostics"))]
    let _ = section;
}

/// Adds `seconds` to `section` in the current window.
#[cfg(any(test, feature = "diagnostics"))]
pub fn add(section: Section, seconds: f64) {
    TOTALS.with(|totals| totals.borrow_mut().seconds[section as usize] += seconds);
}

/// Ends a frame, finishing the window after `WINDOW_FRAMES`.
#[cfg(any(test, feature = "diagnostics"))]
pub fn end_frame() {
    TOTALS.with(|totals| {
        let mut totals = totals.borrow_mut();
        totals.frames += 1;
        if totals.frames >= WINDOW_FRAMES {
            let frames = f64::from(totals.frames);
            let mut reading = [0.0; SECTIONS];
            for (mean, total) in reading.iter_mut().zip(&totals.seconds) {
                *mean = total / frames * MILLISECONDS_PER_SECOND;
            }
            totals.reading = Some(reading);
            totals.seconds = [0.0; SECTIONS];
            totals.frames = 0;
        }
    });
}

/// The last window's split as labeled rows, once there is one.
#[cfg(any(test, feature = "diagnostics"))]
pub fn rows() -> Option<[(&'static str, String); SECTIONS]> {
    TOTALS.with(|totals| {
        totals.borrow().reading.map(|ms| {
            let value = |section: Section| format!("{:.1} ms", ms[section as usize]);
            [
                ("Scene", value(Section::Scene)),
                (
                    "Graphics calls, in the scene",
                    value(Section::GraphicsCalls),
                ),
                ("Panel, labels and buttons", value(Section::Overlay)),
                ("Measuring text, in those", value(Section::Measuring)),
                ("Showing the panel, in those", value(Section::Panel)),
                ("Painting it anew, in that", value(Section::PanelPaint)),
                ("Buttons and sliders, in those", value(Section::Controls)),
                ("Scene labels, in those", value(Section::Labels)),
                ("Graphics card, the scene", value(Section::CardScene)),
                ("Graphics card, the rest", value(Section::CardRest)),
                ("Hand-off and wait", value(Section::HandOff)),
            ]
        })
    })
}

#[cfg(test)]
#[path = "../../tests/unit/ui/diagnostics.rs"]
mod tests;
