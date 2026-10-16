//! Rolling measurements of frame rate and physics workload.

use crate::simulation::PhysicsUpdateStats;

const SAMPLE_WINDOW_SECONDS: f32 = 1.0;
const MILLISECONDS_PER_SECOND: f64 = 1_000.0;

/// A completed sampling window of averaged performance measurements.
#[derive(Clone, Copy, Debug)]
pub struct PerformanceSample {
    pub fps: f32,
    pub physics_ms_per_frame: f32,
    pub physics_steps_per_second: f32,
    pub candidate_checks_per_step: f32,
}

/// Accumulates per-frame performance counters into one-second samples.
#[derive(Default)]
pub struct PerformanceMonitor {
    elapsed_seconds: f32,
    frames: u32,
    physics_seconds: f64,
    physics_steps: u32,
    candidate_checks: u64,
    sample: Option<PerformanceSample>,
}

impl PerformanceMonitor {
    /// Starts a fresh monitor with no completed sample.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one frame and publishes a sample when its window ends.
    pub fn record_frame(
        &mut self,
        frame_time: f32,
        physics_seconds: f64,
        update: PhysicsUpdateStats,
    ) {
        self.elapsed_seconds += frame_time;
        self.frames += 1;
        self.physics_seconds += physics_seconds;
        self.physics_steps += update.steps;
        self.candidate_checks += update.candidate_checks as u64;

        if self.elapsed_seconds < SAMPLE_WINDOW_SECONDS {
            return;
        }

        let steps = self.physics_steps;

        self.sample = Some(PerformanceSample {
            fps: self.frames as f32 / self.elapsed_seconds,
            physics_ms_per_frame: (self.physics_seconds * MILLISECONDS_PER_SECOND
                / self.frames as f64) as f32,
            physics_steps_per_second: steps as f32 / self.elapsed_seconds,
            candidate_checks_per_step: if steps == 0 {
                0.0
            } else {
                self.candidate_checks as f32 / steps as f32
            },
        });

        self.elapsed_seconds = 0.0;
        self.frames = 0;
        self.physics_seconds = 0.0;
        self.physics_steps = 0;
        self.candidate_checks = 0;
    }

    /// Returns the most recent sample, if a full window has elapsed.
    pub fn sample(&self) -> Option<PerformanceSample> {
        self.sample
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_FRAME_TIME: f32 = 0.5;
    const SAMPLE_PHYSICS_TIME: f64 = 0.004;
    const SAMPLE_STEPS: u32 = 2;
    const SAMPLE_CHECKS: usize = 20;

    const EXPECTED_FPS: f32 = 2.0;
    const EXPECTED_PHYSICS_MS: f32 = 4.0;
    const EXPECTED_STEPS_PER_SECOND: f32 = 4.0;
    const EXPECTED_CHECKS_PER_STEP: f32 = 10.0;

    const TOLERANCE: f32 = 1e-5;

    #[test]
    fn reports_averages_after_one_window() {
        let mut monitor = PerformanceMonitor::new();

        let update = PhysicsUpdateStats {
            steps: SAMPLE_STEPS,
            candidate_checks: SAMPLE_CHECKS,
        };

        monitor.record_frame(SAMPLE_FRAME_TIME, SAMPLE_PHYSICS_TIME, update);

        assert!(monitor.sample().is_none());

        monitor.record_frame(SAMPLE_FRAME_TIME, SAMPLE_PHYSICS_TIME, update);

        let sample = monitor.sample().expect("sample should be ready");

        assert!((sample.fps - EXPECTED_FPS).abs() < TOLERANCE);
        assert!((sample.physics_ms_per_frame - EXPECTED_PHYSICS_MS).abs() < TOLERANCE);
        assert!((sample.physics_steps_per_second - EXPECTED_STEPS_PER_SECOND).abs() < TOLERANCE);
        assert!((sample.candidate_checks_per_step - EXPECTED_CHECKS_PER_STEP).abs() < TOLERANCE);
    }
}
