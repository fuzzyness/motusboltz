//! Heat sparks: a brief twinkle where a particle leaves a thermal wall,
//! orange where heat flows into the gas and blue where it flows out.
//!
//! Every wall impact exchanges heat one way or the other, and even with no
//! temperature difference about as many speed particles up as slow them
//! down. So each wall shows only the sparks that go its way on balance, and
//! only enough of them that the heat they carry adds up to its net flow:
//! plenty at a hot burner or deep ice, almost none at equal temperatures. A
//! spark's size grows with the heat it carries. The balance is smoothed over
//! a few seconds so the sparks do not flicker with each swing of the
//! pistons.

use macroquad::prelude::{
    Camera3D,
    Color,
    vec2, //
};

use super::block::cut_normal;
use super::sprites::{
    Facing,
    SpriteMaterials,
    Sprites,
    unit_hash,
    with_alpha, //
};
use crate::sim::engine::{
    ConnectedChambers,
    PARTICLE_RADIUS, //
};
use crate::sim::geometry::ChamberGeometry;
use crate::sim::sparks::SPARK_SECONDS;

/// Color of heat into the gas.
const HEAT_IN: Color = Color::from_rgba(255, 190, 90, 255);
/// Color of heat out of the gas.
const HEAT_OUT: Color = Color::from_rgba(150, 215, 255, 255);
/// Half the size of a fresh spark carrying one reduced unit of heat (the
/// mean energy of motion along one axis at 600 K), in particle radii.
const HALF_SIZE: f32 = 1.8;
/// Smallest and largest size of a spark against `HALF_SIZE`.
const SIZE_RANGE: (f32, f32) = (0.6, 1.6);
/// How much a spark grows as it fades.
const GROWTH: f32 = 0.45;
/// Opacity of a fresh spark.
const OPACITY: f32 = 0.95;
/// Seconds of wall time over which each wall's balance is smoothed.
const BALANCE_SECONDS: f32 = 2.0;
/// Longest wall-time step the balance takes in one frame.
const MAX_STEP_SECONDS: f32 = 0.1;
/// Salt for each spark's draw.
const SALT_SHOW: u32 = 31;

/// The smoothed heat a wall exchanged: its net flow into the gas and all
/// the heat it moved either way.
#[derive(Clone, Copy, Debug, Default)]
struct Balance {
    /// Smoothed net heat into the gas.
    net: f32,
    /// Smoothed heat moved either way.
    gross: f32,
}

impl Balance {
    /// The net flow as a share of all the heat moved, from -1 to 1.
    fn share(self) -> f32 {
        if self.gross > 0.0 {
            (self.net / self.gross).clamp(-1.0, 1.0)
        } else {
            0.0
        }
    }

    /// Share of the sparks going the net flow's way whose heat adds up to
    /// the net flow.
    ///
    /// Those sparks carry half of all the heat moved plus half the net.
    fn shown(self) -> f32 {
        let share = self.share().abs();
        2.0 * share / (1.0 + share)
    }

    /// Moves toward `net` and `gross` by `weight`.
    fn follow(&mut self, net: f32, gross: f32, weight: f32) {
        self.net += (net - self.net) * weight;
        self.gross += (gross - self.gross) * weight;
    }
}

/// The heat sparks.
pub struct HeatSparks {
    /// The chamber, for which wall a spark is on.
    geometry: ChamberGeometry,
    /// The balance at the hot and the cold wall.
    balance: [Balance; 2],
    /// The clock at the last build.
    seconds: Option<f64>,
    /// The sparks shown.
    sprites: Sprites,
}

impl HeatSparks {
    /// No sparks yet, in the chamber of `geometry`.
    pub fn new(geometry: ChamberGeometry) -> Self {
        Self {
            geometry,
            balance: [Balance::default(); 2],
            seconds: None,
            sprites: Sprites::new(),
        }
    }

    /// Lays out the sparks of `simulation` at `seconds` on the clock, seen
    /// from `camera`.
    pub fn build(&mut self, simulation: &ConnectedChambers, camera: &Camera3D, seconds: f64) {
        self.sprites.clear();
        let sparks = simulation.heat_sparks();
        let mut totals = [(0.0, 0.0); 2];
        for spark in sparks.recent() {
            let side = &mut totals[usize::from(spark.position.x > 0.0)];
            side.0 += spark.heat;
            side.1 += spark.heat.abs();
        }
        let step = self.seconds.map_or(1.0, |last| {
            ((seconds - last) as f32).clamp(0.0, MAX_STEP_SECONDS)
        });
        let weight = if self.seconds.is_some() {
            1.0 - (-step / BALANCE_SECONDS).exp()
        } else {
            1.0
        };
        self.seconds = Some(seconds);
        for (balance, (net, gross)) in self.balance.iter_mut().zip(totals) {
            balance.follow(net, gross, weight);
        }

        let facing = Facing::toward(camera);
        let toward_camera = cut_normal(camera.position);
        let faces = [
            simulation.power_pistons().hot().face_y,
            simulation.power_pistons().cold().face_y,
        ];
        let now = sparks.time();
        let floor = -self.geometry.cylinder_height / 2.0 + PARTICLE_RADIUS;
        for spark in sparks.recent() {
            let cold = spark.position.x > 0.0;
            let balance = self.balance[usize::from(cold)];
            if balance.net * spark.heat <= 0.0 {
                continue;
            }
            // The same draw for a spark every frame, so it does not blink.
            let key = spark.position.x.to_bits() ^ spark.position.z.to_bits().rotate_left(16);
            if unit_hash(key, SALT_SHOW) > balance.shown() {
                continue;
            }
            // The near half of each bore's wall is cut away for the window.
            let axis = if cold {
                self.geometry.cold_center_x()
            } else {
                self.geometry.hot_center_x()
            };
            let out = vec2(spark.position.x - axis, spark.position.z);
            let on_floor = spark.position.y <= floor + PARTICLE_RADIUS;
            if !on_floor && out.dot(vec2(toward_camera.x, toward_camera.z)) > 0.0 {
                continue;
            }
            // Seen from above, the piston's crown hides the wall under it.
            let eye = camera.position;
            let face = faces[usize::from(cold)];
            let along = (face - eye.y) / (spark.position.y - eye.y);
            if (0.0..1.0).contains(&along) {
                let crossing = eye + (spark.position - eye) * along;
                let from_axis = vec2(crossing.x - axis, crossing.z);
                if from_axis.length() < self.geometry.cylinder_radius {
                    continue;
                }
            }
            let life = ((now - spark.time) / SPARK_SECONDS) as f32;
            let size = spark.heat.abs().sqrt().clamp(SIZE_RANGE.0, SIZE_RANGE.1);
            let half = HALF_SIZE * size * PARTICLE_RADIUS * (1.0 + GROWTH * life);
            let color = if spark.heat > 0.0 { HEAT_IN } else { HEAT_OUT };
            let alpha = OPACITY * (1.0 - life).max(0.0).powf(1.5);
            let color = with_alpha(color, alpha);
            self.sprites
                .facing(facing, spark.position, half, half, color);
        }
        self.sprites.finish(facing.forward);
    }

    /// Draws the sparks, which add their light to the scene.
    ///
    /// They are drawn over everything: the gas would hide those at a bore's
    /// far wall, and the near wall's are left out.
    pub fn draw(&self, materials: &SpriteMaterials) {
        self.sprites.draw(&materials.overlay);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/heat_sparks.rs"]
mod tests;
