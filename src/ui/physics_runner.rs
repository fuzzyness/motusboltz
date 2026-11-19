//! Runs each frame's physics, on a thread of its own where there can be
//! one, while the frame is handed over and the next awaited.
//!
//! A frame used to step the physics and then draw, one after the other on
//! one core. Now it draws the engine as it stands, then hands the engine to
//! the physics thread to step while the frame is shown and the display's
//! refresh awaited, and takes it back at the start of the next frame. The
//! settings changed in a frame still come before its step, as before, and
//! the steps are the same, so the physics is the same; what a frame shows
//! is the engine before its own step rather than after it, one frame
//! behind.
//!
//! While the engine is away, the app holds a small stand-in in its place,
//! which nothing reads: only the wait for the next frame comes between.
//!
//! In the browser, which gives the page no threads here, the step runs at
//! once, in the same place in the frame.

use macroquad::miniquad::date;

use crate::sim::engine::ConnectedChambers;
#[cfg(not(target_arch = "wasm32"))]
use crate::sim::geometry::DEFAULT_GEOMETRY;
#[cfg(not(target_arch = "wasm32"))]
use crate::sim::units::MIN_PARTICLES;

/// Seed of the stand-in, which never runs.
#[cfg(not(target_arch = "wasm32"))]
const STAND_IN_SEED: u64 = 0;

/// What a step ran.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ran {
    /// Whether frame time had to be dropped (see `ConnectedChambers::update`).
    pub limited: bool,
    /// Model time run, in seconds.
    pub model: f32,
    /// The computer's time the step took, in seconds.
    pub seconds: f64,
}

/// Steps the engine, on the physics thread where there is one (see the
/// module documentation).
pub struct PhysicsRunner {
    /// The physics thread, where there is one.
    #[cfg(not(target_arch = "wasm32"))]
    worker: Option<worker::Worker>,
    /// A step run at once, waiting to be collected.
    ran: Option<Ran>,
    /// The stand-in, while the engine is with the physics thread.
    #[cfg(not(target_arch = "wasm32"))]
    stand_in: Option<Box<ConnectedChambers>>,
}

impl PhysicsRunner {
    /// A runner with its physics thread started, where there can be one.
    pub fn new() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            worker: worker::Worker::start(),
            ran: None,
            #[cfg(not(target_arch = "wasm32"))]
            stand_in: Some(Box::new(ConnectedChambers::with_geometry_seed(
                MIN_PARTICLES,
                DEFAULT_GEOMETRY,
                STAND_IN_SEED,
            ))),
        }
    }

    /// A runner that steps at once, with no thread, as in the browser.
    #[cfg(test)]
    fn inline() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            worker: None,
            ..Self::new()
        }
    }

    /// Starts `simulation` running `model` seconds of model time: handed
    /// to the physics thread, leaving the stand-in in its place, or run at
    /// once where there is no thread.
    ///
    /// # Panics
    ///
    /// Panics if a step started is not yet finished.
    pub fn start(&mut self, simulation: &mut Box<ConnectedChambers>, model: f32) {
        assert!(self.ran.is_none(), "a step is already running");
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(worker) = &self.worker {
            let stand_in = self.stand_in.take().expect("the stand-in is home");
            let engine = std::mem::replace(simulation, stand_in);
            worker.send(engine, model);
            return;
        }
        self.ran = Some(step(simulation, model));
    }

    /// Waits for the step `start` started, if any, puts the engine back in
    /// `simulation`, and says what ran.
    ///
    /// # Panics
    ///
    /// Panics if the physics thread has stopped, as it does when the
    /// physics panics.
    pub fn finish(&mut self, simulation: &mut Box<ConnectedChambers>) -> Option<Ran> {
        #[cfg(not(target_arch = "wasm32"))]
        if let (Some(worker), true) = (&self.worker, self.stand_in.is_none()) {
            let (engine, ran) = worker.receive();
            self.stand_in = Some(std::mem::replace(simulation, engine));
            return Some(ran);
        }
        let _ = simulation;
        self.ran.take()
    }
}

/// Runs `model` seconds of `simulation` and says what ran.
fn step(simulation: &mut ConnectedChambers, model: f32) -> Ran {
    let started = date::now();
    let limited = simulation.update(model);
    Ran {
        limited,
        model,
        seconds: date::now() - started,
    }
}

/// The physics thread.
#[cfg(not(target_arch = "wasm32"))]
mod worker {
    use std::sync::mpsc::{
        Receiver,
        Sender,
        channel, //
    };

    use super::{
        ConnectedChambers,
        Ran,
        step, //
    };

    /// The engine and the model time it is to run.
    type Job = (Box<ConnectedChambers>, f32);
    /// The engine, stepped, and what ran.
    type Done = (Box<ConnectedChambers>, Ran);

    /// Quality of service of work the user waits on, which macOS keeps on
    /// the processor's fast cores (`QOS_CLASS_USER_INTERACTIVE` in
    /// `<sys/qos.h>`).
    #[cfg(target_os = "macos")]
    const USER_INTERACTIVE: u32 = 0x21;

    #[cfg(target_os = "macos")]
    unsafe extern "C" {
        /// Sets the calling thread's quality of service (`<pthread/qos.h>`).
        fn pthread_set_qos_class_self_np(class: u32, relative_priority: i32) -> i32;
    }

    /// Asks the system to run the calling thread on a fast core: macOS
    /// otherwise runs a plain thread, as background work, on its slower
    /// efficiency cores. Elsewhere the scheduler already treats it as it
    /// treats the program's own.
    fn on_a_fast_core() {
        // SAFETY: the call only sets the calling thread's own class; a
        // refusal leaves it as it was, which is no harm.
        #[cfg(target_os = "macos")]
        unsafe {
            pthread_set_qos_class_self_np(USER_INTERACTIVE, 0);
        }
    }

    /// The thread's two ends.
    pub struct Worker {
        /// Where steps are sent.
        jobs: Sender<Job>,
        /// Where they come back.
        done: Receiver<Done>,
    }

    impl Worker {
        /// Starts the thread, or `None` if it cannot be started; the steps
        /// then run at once.
        pub fn start() -> Option<Self> {
            let (jobs, waiting) = channel::<Job>();
            let (finished, done) = channel::<Done>();
            std::thread::Builder::new()
                .name("physics".to_owned())
                .spawn(move || {
                    on_a_fast_core();
                    for (mut engine, model) in waiting {
                        let ran = step(&mut engine, model);
                        if finished.send((engine, ran)).is_err() {
                            break;
                        }
                    }
                })
                .ok()?;
            Some(Self { jobs, done })
        }

        /// Hands `engine` to the thread to run `model` seconds.
        pub fn send(&self, engine: Box<ConnectedChambers>, model: f32) {
            self.jobs
                .send((engine, model))
                .expect("the physics thread stopped");
        }

        /// Waits for the engine to come back, stepped.
        pub fn receive(&self) -> Done {
            self.done.recv().expect("the physics thread stopped")
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/physics_runner.rs"]
mod tests;
