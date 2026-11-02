//! The complete simulation: the gas fill, fixed-timestep stepping, particle
//! impacts on the thermal walls and the pistons, and the readings taken
//! from it.

use macroquad::prelude::{
    Vec3,
    vec3, //
};
use macroquad::rand::{
    RandGenerator,
    gen_range, //
};

use crate::sim::cycle::{
    CycleMonitor,
    CycleTotals,
    Substep, //
};
use crate::sim::gas::{
    self,
    Contact,
    INITIAL_TEMPERATURE,
    PARTICLE_MASS,
    Particle, //
};
use crate::sim::gauge::PressureGauge;
use crate::sim::geometry::{
    ChamberGeometry,
    DEFAULT_GEOMETRY,
    WALL_PROJECTION_MARGIN, //
};
use crate::sim::grid::SpatialGrid;
use crate::sim::pistons::{
    PistonSide,
    PistonState,
    PowerPistons,
    starting_energy, //
};
use crate::sim::regenerator::Regenerator;
use crate::sim::reservoir::{
    PerReservoir,
    ReservoirSide,
    ThermalReservoirs, //
};
use crate::sim::sparks::HeatSparks;
use crate::sim::tracers::Tracers;
use crate::sim::units::{
    GasMeasurements,
    HeliumCalibration,
    KELVIN_PER_UNIT,
    KineticMoments,
    MAX_CONCENTRATION,
    MAX_PARTICLES,
    METERS_PER_UNIT,
    MIN_CONCENTRATION,
    MIN_PARTICLES,
    volume_cm3, //
};

/// Collision and drawing radius of every particle, at every fill.
pub const PARTICLE_RADIUS: f32 = 0.13;
/// Least fill lattice spacing over the particle radius, so neighboring
/// particles start with a gap.
const SPACING_PER_RADIUS: f32 = 2.5;
/// Model time a particle stays highlighted after a collision; drawing only.
pub const COLLISION_FLASH_DURATION: f32 = 0.01;
/// Fixed steps per second of model time.
const FIXED_STEPS_PER_SECOND: u32 = 120;
/// Length of one fixed step.
const FIXED_DT: f32 = 1.0 / FIXED_STEPS_PER_SECOND as f32;
/// Most fixed steps run in one frame, so a slow frame cannot build an
/// ever-growing backlog.
///
/// Six steps cover a 20 FPS frame, so a display held at 30 FPS (for example
/// Safari in Low Power Mode) keeps real time even when its frames arrive
/// unevenly.
const MAX_FIXED_STEPS_PER_FRAME: usize = 6;
/// Longest frame time taken per update, exactly the step budget; longer
/// frames are clipped and reported as limited.
const MAX_FRAME_TIME: f32 = MAX_FIXED_STEPS_PER_FRAME as f32 * FIXED_DT;
/// Most a particle moves relative to a piston in one substep, in radii.
///
/// Collisions are found as overlaps at the end of each substep, so longer
/// substeps miss a few grazing ones: at 0.75, 2,048 particles collide 1.5%
/// less often than at 0.5 (2.9% less than at 0.25), while turns, heat and
/// work stay within the scatter between seeds. Every substep costs a full
/// collision pass, so this limit sets most of the physics time.
const MAX_TRAVEL_RADII: f32 = 0.75;
/// Most the crank turns in one substep.
const MAX_CRANK_STEP_RADIANS: f32 = 0.02;

/// Which part of the connected gas volume holds a particle center.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    /// The heated cylinder, on the left (negative x).
    Hot,
    /// In the opening between the cylinders, outside both bores.
    Passage,
    /// The cooled cylinder, on the right (positive x).
    Cold,
}

/// SI readings for each cylinder, and the passage's particle count.
#[derive(Clone, Copy, Debug)]
pub struct ChamberMeasurements {
    /// The hot cylinder.
    pub hot: GasMeasurements,
    /// Particles in the opening between the bores.
    pub passage_count: usize,
    /// The cold cylinder.
    pub cold: GasMeasurements,
}

/// The complete simulation: gas particles in two cylinders joined by an
/// opening through the wall between them, thermal cylinder walls, and
/// pistons coupled to the flywheel.
///
/// The pistons and the opening's wall are insulated; only the cylinders'
/// sides and floors exchange heat with the reservoirs, and the regenerator's
/// screens, when they are in the opening, trade heat with the gas passing
/// through.
pub struct ConnectedChambers {
    /// The gas.
    pub particles: Vec<Particle>,
    /// Model time left on each particle's collision highlight.
    flashes: Vec<f32>,
    /// Finds nearby pairs for collisions.
    grid: SpatialGrid,
    /// Particles moved by overlap correction in the current substep, the
    /// only ones that can have crossed a wall since the wall check.
    nudged: Vec<usize>,
    /// Shape of the chamber.
    geometry: ChamberGeometry,
    /// Crankshaft, flywheel and pistons.
    pistons: PowerPistons,
    /// Work done by the gas on the pistons since the fill.
    piston_work_by_gas: f64,
    /// Frame time not yet run as fixed steps.
    accumulator: f32,
    /// Hot and cold walls.
    thermal: ThermalReservoirs,
    /// Screens that can be put in the opening.
    regenerator: Regenerator,
    /// Conversions to SI units.
    units: HeliumCalibration,
    /// Gas kinetic energy at the fill.
    initial_energy: f32,
    /// Per-turn records.
    cycle: CycleMonitor,
    /// The particles being followed, if tracers are on.
    tracers: Option<Tracers>,
    /// Recent heat exchanges at the walls, for drawing.
    sparks: HeatSparks,
    /// A pressure gauge on each piston, indexed by `HOT` and `COLD`.
    gauges: [PressureGauge; 2],
}

impl ConnectedChambers {
    /// Fills the default chamber at `concentration` (mol/L) with a random
    /// seed from Macroquad's global generator.
    ///
    /// # Panics
    ///
    /// Panics if `concentration` is outside the fill range.
    pub fn with_concentration(concentration: f32) -> Self {
        let count = Self::particles_for_concentration(concentration);
        Self::with_geometry_seed(count, DEFAULT_GEOMETRY, gen_range(0, u64::MAX))
    }

    /// Particles for an initial concentration in mol/L.
    ///
    /// # Panics
    ///
    /// Panics if `concentration` is outside the fill range.
    fn particles_for_concentration(concentration: f32) -> usize {
        assert!((MIN_CONCENTRATION..=MAX_CONCENTRATION).contains(&concentration));
        ((concentration as f64 / MAX_CONCENTRATION as f64 * MAX_PARTICLES as f64).round() as usize)
            .clamp(MIN_PARTICLES, MAX_PARTICLES)
    }

    /// Fills `geometry` with `count` particles at 300 K, the shaft at rest.
    ///
    /// The same `seed` gives the same velocities and wall-impact draws, and
    /// each simulation has its own generator.
    ///
    /// # Panics
    ///
    /// Panics if `count` is zero or above `MAX_PARTICLES`, or if `geometry`
    /// cannot hold the particles.
    pub fn with_geometry_seed(count: usize, geometry: ChamberGeometry, seed: u64) -> Self {
        assert!(
            (1..=MAX_PARTICLES).contains(&count),
            "particle count must be between 1 and {MAX_PARTICLES}"
        );
        geometry.validate(PARTICLE_RADIUS);
        let random = RandGenerator::new();
        random.srand(seed);
        let mut pistons = PowerPistons::new(geometry, PARTICLE_RADIUS);
        // The back pressure equals the gas's initial pressure, P = N kT / V
        // with kT = 1 in reduced units: a mechanical boundary condition, not
        // a heat source.
        let volume_m3 = pistons.volume_m3(geometry);
        pistons.set_reference_pressure(count as f32 / (volume_m3 / METERS_PER_UNIT.powi(3)) as f32);
        pistons.set_starting_energy(starting_energy(count));

        // Sort the particles by region, then bring each region to rest on
        // average and to exactly 300 K, so no region starts with a flow.
        let sites = fill_sites(geometry, &pistons, count);
        let mut regions: [Vec<Particle>; 3] = Default::default();
        for index in 0..count {
            let position = sites[((2 * index + 1) * sites.len()) / (2 * count)];
            let particle = Particle {
                position,
                velocity: gas::sample_thermal_velocity(INITIAL_TEMPERATURE, &random),
            };
            regions[region_of(geometry, &pistons, position) as usize].push(particle);
        }
        for region in &mut regions {
            gas::thermalize(region, INITIAL_TEMPERATURE);
        }
        let particles = regions.concat();

        let (lower, upper) = geometry.gas_bounds();
        let thermal_seed = random.gen_range(0, u64::MAX);
        let regenerator_seed = random.gen_range(0, u64::MAX);
        let start = CycleTotals {
            gas_energy: gas_energy(&particles),
            ..CycleTotals::default()
        };
        Self {
            initial_energy: gas::kinetic_energy(&particles),
            flashes: vec![0.0; count],
            particles,
            grid: SpatialGrid::covering(geometry.grid_side(), PARTICLE_RADIUS, lower, upper),
            nudged: Vec::new(),
            geometry,
            cycle: CycleMonitor::new(pistons.face_area() as f64, start),
            pistons,
            piston_work_by_gas: 0.0,
            accumulator: 0.0,
            thermal: ThermalReservoirs::with_seed(thermal_seed),
            regenerator: Regenerator::new(
                geometry,
                count,
                INITIAL_TEMPERATURE as f64,
                regenerator_seed,
            ),
            units: HeliumCalibration::new(volume_m3),
            tracers: None,
            sparks: HeatSparks::new(),
            gauges: [PressureGauge::new(); 2],
        }
    }

    /// Rebuilds the simulation with `count` particles and a fresh random
    /// seed, keeping the reservoir temperatures and whether the regenerator
    /// is in.
    ///
    /// The shaft starts at rest, the regenerator's screens start between the
    /// wall temperatures, and heat, work, starter work and the per-turn
    /// records start over.
    fn refill(&mut self, count: usize) {
        let (hot_kelvin, cold_kelvin) = (self.hot_kelvin(), self.cold_kelvin());
        let regenerating = self.regenerator.is_enabled();
        *self = Self::with_geometry_seed(count, self.geometry, gen_range(0, u64::MAX));
        self.thermal.set_hot_kelvin(hot_kelvin);
        self.thermal.set_cold_kelvin(cold_kelvin);
        let (hot, cold) = self.reservoir_temperatures();
        self.regenerator.set_enabled(regenerating, hot, cold);
    }

    /// Refills with the same amount of helium at 300 K.
    pub fn reset(&mut self) {
        self.refill(self.particles.len());
    }

    /// Refills at a new concentration in mol/L, if it changes the particle
    /// count; each particle stands for a fixed amount of helium.
    ///
    /// # Panics
    ///
    /// Panics if `concentration` is outside the fill range.
    pub fn set_concentration(&mut self, concentration: f32) {
        let count = Self::particles_for_concentration(concentration);
        if count != self.particles.len() {
            self.refill(count);
        }
    }

    /// Sets the hot wall temperature; the gas changes only as it hits it.
    ///
    /// A change discards the per-turn history (see `CycleMonitor`).
    ///
    /// # Panics
    ///
    /// Panics if `kelvin` is outside the hot range.
    pub fn set_hot_kelvin(&mut self, kelvin: f32) {
        if kelvin != self.hot_kelvin() {
            self.cycle.mark_settings_changed();
        }
        self.thermal.set_hot_kelvin(kelvin);
    }

    /// Sets the cold wall temperature; the gas changes only as it hits it.
    ///
    /// A change discards the per-turn history (see `CycleMonitor`).
    ///
    /// # Panics
    ///
    /// Panics if `kelvin` is outside the cold range.
    pub fn set_cold_kelvin(&mut self, kelvin: f32) {
        if kelvin != self.cold_kelvin() {
            self.cycle.mark_settings_changed();
        }
        self.thermal.set_cold_kelvin(kelvin);
    }

    /// Puts the regenerator's screens in the opening or takes them out.
    ///
    /// Screens put in start in a straight line between the hot and cold
    /// wall temperatures (see `Regenerator::set_enabled`). A change
    /// discards the per-turn history, as a temperature change does.
    pub fn set_regenerator(&mut self, enabled: bool) {
        if enabled == self.regenerator.is_enabled() {
            return;
        }
        self.cycle.mark_settings_changed();
        let (hot, cold) = self.reservoir_temperatures();
        self.regenerator.set_enabled(enabled, hot, cold);
    }

    /// The hot and cold wall temperatures in reduced units.
    fn reservoir_temperatures(&self) -> (f64, f64) {
        let reduced = |kelvin: f32| kelvin as f64 / KELVIN_PER_UNIT;
        (reduced(self.hot_kelvin()), reduced(self.cold_kelvin()))
    }

    /// The regenerator's screens, in the opening or not.
    pub fn regenerator(&self) -> &Regenerator {
        &self.regenerator
    }

    /// The hot wall temperature.
    pub fn hot_kelvin(&self) -> f32 {
        self.thermal.hot_kelvin()
    }

    /// The cold wall temperature.
    pub fn cold_kelvin(&self) -> f32 {
        self.thermal.cold_kelvin()
    }

    /// Per-turn heat, work, entropy and pressure-volume records.
    pub fn cycle(&self) -> &CycleMonitor {
        &self.cycle
    }

    /// Heat each reservoir has given the gas since the fill.
    pub fn heat(&self) -> PerReservoir {
        self.thermal.heat()
    }

    /// Gas first-law residual, ideally zero: the change in gas energy, minus
    /// heat into the gas from the walls and the regenerator, plus work done
    /// by the gas on the pistons.
    pub fn energy_balance_error(&self) -> f64 {
        self.kinetic_energy() as f64
            - self.initial_energy as f64
            - self.heat().total()
            - self.regenerator.heat()
            + self.piston_work_by_gas
    }

    /// Flywheel energy-balance residual, ideally zero: flywheel energy minus
    /// gas work, starter work and back-pressure work, plus drag losses.
    pub fn mechanical_balance_error(&self) -> f64 {
        let pistons = &self.pistons;
        pistons.kinetic_energy()
            - self.piston_work_by_gas
            - pistons.starter_work()
            - pistons.back_pressure_work()
            + pistons.drag_loss()
    }

    /// Work done by the gas on both pistons since the fill; negative when the
    /// pistons have done net work on the gas.
    pub fn piston_work_by_gas(&self) -> f64 {
        self.piston_work_by_gas
    }

    /// Crankshaft, flywheel and pistons.
    pub fn power_pistons(&self) -> &PowerPistons {
        &self.pistons
    }

    /// Brings the flywheel up to the starting speed for this fill (see
    /// `PowerPistons::start`); returns whether the speed changed.
    pub fn start_engine(&mut self) -> bool {
        self.pistons.start()
    }

    /// Turns the shaft by hand at `speed` radians per second of model time
    /// (see `PowerPistons::crank`); returns whether the speed changed.
    pub fn crank_engine(&mut self, speed: f64) -> bool {
        self.pistons.crank(speed)
    }

    /// Kicks the flywheel once (see `PowerPistons::kick`).
    pub fn kick_flywheel(&mut self) {
        self.pistons.kick();
    }

    /// Shape of the chamber.
    pub fn geometry(&self) -> ChamberGeometry {
        self.geometry
    }

    /// Conversions from reduced units to SI.
    pub fn units(&self) -> HeliumCalibration {
        self.units
    }

    /// Helium amount represented by all the particles.
    pub fn total_moles(&self) -> f64 {
        self.units.total_moles(self.particles.len())
    }

    /// Smallest and largest gas volume of one cylinder over a turn, in cm3.
    pub fn cylinder_volume_range_cm3(&self) -> (f64, f64) {
        let area = self.pistons.face_area() as f64;
        let height = self.geometry.cylinder_height as f64;
        let stroke = self.pistons.stroke() as f64;
        (
            volume_cm3(area * (height - stroke)),
            volume_cm3(area * height),
        )
    }

    /// Temperature, concentration and measured pressure for each cylinder.
    ///
    /// Each region's mean flow is left out of its temperature; the passage
    /// reports only its particle count.
    pub fn measurements(&self) -> ChamberMeasurements {
        let mut moments = [KineticMoments::default(); 3];
        for particle in &self.particles {
            moments[self.region(particle.position) as usize].record(particle.velocity);
        }
        let face_area = self.pistons.face_area() as f64;
        let measure = |region: Region, piston: PistonState, gauge: PressureGauge| {
            let volume = self.pistons.cylinder_volume_m3(self.geometry, piston);
            let pressure = gauge.pressure(face_area);
            self.units
                .measure(moments[region as usize], volume, pressure)
        };
        let [hot, cold] = self.gauges;
        ChamberMeasurements {
            hot: measure(Region::Hot, self.pistons.hot(), hot),
            passage_count: moments[Region::Passage as usize].count(),
            cold: measure(Region::Cold, self.pistons.cold(), cold),
        }
    }

    /// Starts following up to `count` particles chosen with `seed`; drawing
    /// only.
    ///
    /// They are picked alternately from the hot and the cold cylinder, so
    /// both sides have one to watch.
    pub fn follow_particles(&mut self, count: usize, seed: u64) {
        let random = RandGenerator::new();
        random.srand(seed);
        let mut sides: [Vec<usize>; 2] = Default::default();
        for (index, particle) in self.particles.iter().enumerate() {
            match self.region(particle.position) {
                Region::Hot => sides[0].push(index),
                Region::Cold => sides[1].push(index),
                Region::Passage => {}
            }
        }
        let mut chosen = Vec::with_capacity(count);
        for pick in 0..count.min(self.particles.len()) {
            let side = if sides[pick % 2].is_empty() {
                (pick + 1) % 2
            } else {
                pick % 2
            };
            let side = &mut sides[side];
            if side.is_empty() {
                break;
            }
            let at = random.gen_range(0, side.len() as u64) as usize;
            chosen.push(side.swap_remove(at));
        }
        self.tracers = Some(Tracers::new(&chosen, &self.particles));
    }

    /// Stops following particles.
    pub fn stop_following(&mut self) {
        self.tracers = None;
    }

    /// The followed particles and their recent paths, if tracers are on.
    pub fn tracers(&self) -> Option<&Tracers> {
        self.tracers.as_ref()
    }

    /// Recent heat exchanges at the walls.
    pub fn heat_sparks(&self) -> &HeatSparks {
        &self.sparks
    }

    /// Model time left on each particle's collision highlight.
    pub fn collision_flashes(&self) -> &[f32] {
        &self.flashes
    }

    /// Kinetic (internal) energy of the gas.
    pub fn kinetic_energy(&self) -> f32 {
        gas::kinetic_energy(&self.particles)
    }

    /// Which region holds a particle centered at `position`.
    pub fn region(&self, position: Vec3) -> Region {
        region_of(self.geometry, &self.pistons, position)
    }

    /// Advances the simulation by `frame_time` of fixed steps, at most
    /// `MAX_FIXED_STEPS_PER_FRAME`.
    ///
    /// Returns true when frame time had to be dropped to keep the window
    /// responsive; the step size never grows to catch up.
    ///
    /// # Panics
    ///
    /// Panics if `frame_time` is negative or not finite.
    pub fn update(&mut self, frame_time: f32) -> bool {
        assert!(frame_time.is_finite() && frame_time >= 0.0);
        let mut limited = frame_time > MAX_FRAME_TIME;
        self.accumulator += frame_time.min(MAX_FRAME_TIME);
        for _ in 0..MAX_FIXED_STEPS_PER_FRAME {
            if self.accumulator < FIXED_DT {
                break;
            }
            self.step(FIXED_DT);
            self.accumulator -= FIXED_DT;
        }
        if self.accumulator >= FIXED_DT {
            self.accumulator %= FIXED_DT;
            limited = true;
        }
        limited
    }

    /// Advances one fixed step of `dt`, in substeps short enough that no
    /// particle moves more than `MAX_TRAVEL_RADII` relative to a piston and
    /// the crank turns at most `MAX_CRANK_STEP_RADIANS` in each.
    fn step(&mut self, dt: f32) {
        let mut remaining = dt;
        while remaining > 0.0 {
            let max_speed = self
                .particles
                .iter()
                .map(|particle| particle.velocity.length_squared())
                .fold(0.0_f32, f32::max)
                .sqrt();
            let wall_speed = self
                .pistons
                .face_velocity(PistonSide::Hot)
                .abs()
                .max(self.pistons.face_velocity(PistonSide::Cold).abs());
            let travel_dt = MAX_TRAVEL_RADII * PARTICLE_RADIUS / (max_speed + wall_speed);
            let crank_dt = MAX_CRANK_STEP_RADIANS / self.pistons.angular_velocity().abs();
            let sub_dt = remaining.min(travel_dt).min(crank_dt);
            self.integrate(sub_dt);
            remaining -= sub_dt;
        }
    }

    /// One substep of `dt`: moves the shaft, then the particles, bounces
    /// them off the pistons and walls, and resolves particle collisions.
    fn integrate(&mut self, dt: f32) {
        self.pistons.advance(dt);
        let (hot, cold) = (self.pistons.hot(), self.pistons.cold());
        let walls = Walls {
            geometry: self.geometry,
            hot_top: hot.face_y,
            cold_top: cold.face_y,
        };
        self.sparks.advance(dt as f64);
        for particle in &mut self.particles {
            let previous_x = particle.position.x;
            particle.position += particle.velocity * dt;
            self.regenerator.pass(particle, previous_x);
            let (center_x, piston, side) = if particle.position.x < 0.0 {
                (self.geometry.hot_center_x(), hot, PistonSide::Hot)
            } else {
                (self.geometry.cold_center_x(), cold, PistonSide::Cold)
            };
            let work = walls.bounce_piston(particle, center_x, piston, side, &mut self.pistons);
            self.piston_work_by_gas += work as f64;
            if let Some(heat) = walls.constrain(particle, &mut self.thermal, true) {
                self.sparks.record(particle.position, heat);
            }
        }

        for remaining in &mut self.flashes {
            *remaining = (*remaining - dt).max(0.0);
        }
        let (flashes, nudged) = (&mut self.flashes, &mut self.nudged);
        nudged.clear();
        self.grid
            .resolve_collisions(&mut self.particles, |a, b, contact| {
                if contact == Contact::Impact {
                    flashes[a] = COLLISION_FLASH_DURATION;
                    flashes[b] = COLLISION_FLASH_DURATION;
                }
                nudged.extend([a, b]);
            });
        // Overlap correction may have pushed those centers past a wall;
        // every other particle is where the wall check left it.
        nudged.sort_unstable();
        nudged.dedup();
        for &index in nudged.iter() {
            walls.constrain(&mut self.particles[index], &mut self.thermal, false);
        }

        if let Some(tracers) = &mut self.tracers {
            tracers.record(dt as f64, &self.particles);
        }
        self.record_cycle(dt);
    }

    /// Reports the substep of `dt` just taken to the cycle monitor.
    fn record_cycle(&mut self, dt: f32) {
        let pistons = &mut self.pistons;
        let substep = Substep {
            total_angle: pistons.total_angle(),
            crank_angle: pistons.angle(),
            dt: dt as f64,
            gas_impulse: pistons.take_gas_impulse(),
            volume: [pistons.hot(), pistons.cold()].map(|piston| {
                pistons.cylinder_volume_m3(self.geometry, piston) / METERS_PER_UNIT.powi(3)
            }),
        };
        for (gauge, impulse) in self.gauges.iter_mut().zip(substep.gas_impulse) {
            gauge.record(impulse, substep.dt);
        }
        let kelvin = (self.thermal.hot_kelvin(), self.thermal.cold_kelvin());
        let (thermal, regenerator, particles, gas_work) = (
            &self.thermal,
            &self.regenerator,
            &self.particles,
            self.piston_work_by_gas,
        );
        self.cycle.record(substep, kelvin, || CycleTotals {
            hot_heat: thermal.heat().hot,
            cold_heat: thermal.heat().cold,
            regenerator_heat: regenerator.heat(),
            hot_entropy: thermal.entropy().hot,
            cold_entropy: thermal.entropy().cold,
            gas_work,
            gas_energy: gas_energy(particles),
        });
    }
}

/// The chamber's walls with the piston faces where they are, for keeping
/// particles inside during one substep.
#[derive(Clone, Copy)]
struct Walls {
    /// Shape of the chamber.
    geometry: ChamberGeometry,
    /// Height of the hot piston's face.
    hot_top: f32,
    /// Height of the cold piston's face.
    cold_top: f32,
}

impl Walls {
    /// Whether a particle centered at `position` is past the side wall or
    /// floor of the cylinder whose axis is at `center_x`.
    fn beyond_thermal_wall(self, position: Vec3, center_x: f32) -> bool {
        let dx = position.x - center_x;
        let limit = self.geometry.cylinder_radius - PARTICLE_RADIUS;
        dx * dx + position.z * position.z > limit * limit
            || position.y < -self.geometry.cylinder_height / 2.0 + PARTICLE_RADIUS
    }

    /// The nearest place inside the gas volume to an escaped `position`, and
    /// the reservoir owning the wall it crossed: hot or cold for a
    /// cylinder's side or floor, `None` for the insulated opening.
    fn nearest_inside(self, position: Vec3) -> (Vec3, Option<ReservoirSide>) {
        let geometry = self.geometry;
        let cylinder = |center_x: f32, top: f32, side: ReservoirSide| {
            (
                geometry.project_to_cylinder(position, center_x, PARTICLE_RADIUS, top),
                self.beyond_thermal_wall(position, center_x).then_some(side),
            )
        };
        let candidates = [
            cylinder(geometry.hot_center_x(), self.hot_top, ReservoirSide::Hot),
            cylinder(geometry.cold_center_x(), self.cold_top, ReservoirSide::Cold),
            (geometry.project_to_passage(position, PARTICLE_RADIUS), None),
        ];
        let (mut nearest, mut wall) = candidates[0];
        let mut distance_sq = position.distance_squared(nearest);
        for &(candidate, surface) in &candidates[1..] {
            let candidate_distance_sq = position.distance_squared(candidate);
            if candidate_distance_sq < distance_sq {
                (nearest, wall, distance_sq) = (candidate, surface, candidate_distance_sq);
            }
        }
        (nearest, wall)
    }

    /// Pulls an escaped particle back inside.
    ///
    /// With `reflect` it also bounces off the wall it crossed, exchanging
    /// heat if that wall is thermal. After particle collisions, which only
    /// nudge positions, `reflect` is false so the nudge does not count as
    /// another wall impact.
    ///
    /// Returns the heat the wall gave the gas, if it exchanged any.
    fn constrain(
        self,
        particle: &mut Particle,
        thermal: &mut ThermalReservoirs,
        reflect: bool,
    ) -> Option<f32> {
        let (hot_top, cold_top) = (self.hot_top, self.cold_top);
        if self
            .geometry
            .contains(particle.position, PARTICLE_RADIUS, hot_top, cold_top)
        {
            return None;
        }
        let (nearest, reservoir) = self.nearest_inside(particle.position);
        let displacement = particle.position - nearest;
        particle.position = nearest;
        if !reflect || displacement.length_squared() <= f32::EPSILON {
            return None;
        }
        thermal.reflect(particle, displacement.normalize(), reservoir)
    }

    /// Bounces a particle that has reached the face of `piston` on `side`,
    /// in the cylinder whose axis is at `center_x`, and passes the impulse
    /// to the shaft.
    ///
    /// Returns the work done by the particle on the piston, its loss of
    /// kinetic energy.
    fn bounce_piston(
        self,
        particle: &mut Particle,
        center_x: f32,
        piston: PistonState,
        side: PistonSide,
        pistons: &mut PowerPistons,
    ) -> f32 {
        let radial_limit = self.geometry.cylinder_radius - PARTICLE_RADIUS;
        let dx = particle.position.x - center_x;
        if dx * dx + particle.position.z * particle.position.z > radial_limit * radial_limit
            || particle.position.y <= piston.face_y - PARTICLE_RADIUS
        {
            return 0.0;
        }
        particle.position.y =
            piston.face_y - PARTICLE_RADIUS - (self.geometry.grid_side() * WALL_PROJECTION_MARGIN);
        let old_speed = particle.velocity.y;
        let new_speed = pistons.collide(side, old_speed, PARTICLE_MASS);
        particle.velocity.y = new_speed;
        0.5 * PARTICLE_MASS * (old_speed * old_speed - new_speed * new_speed)
    }
}

/// Which region of `geometry`, with the `pistons` where they are, holds a
/// particle centered at `position`.
///
/// A center inside either cylinder counts as that cylinder, so only the part
/// of the opening between the bores counts as passage.
fn region_of(geometry: ChamberGeometry, pistons: &PowerPistons, position: Vec3) -> Region {
    let within = |center_x: f32, piston: PistonState| {
        geometry.within_cylinder(position, center_x, PARTICLE_RADIUS, piston.face_y)
    };
    if within(geometry.hot_center_x(), pistons.hot()) {
        Region::Hot
    } else if within(geometry.cold_center_x(), pistons.cold()) {
        Region::Cold
    } else {
        Region::Passage
    }
}

/// Starting positions for at least `count` particles in `geometry`.
///
/// One lattice spans both bores and the passage, refined until there are
/// enough sites; a single lattice cannot overlap itself where the parts
/// meet.
///
/// # Panics
///
/// Panics if the lattice gets too fine for the particle radius.
fn fill_sites(geometry: ChamberGeometry, pistons: &PowerPistons, count: usize) -> Vec<Vec3> {
    let half_width = geometry.cylinder_offset + geometry.cylinder_radius;
    let height = geometry.cylinder_height;
    let (hot_top, cold_top) = (pistons.hot().face_y, pistons.cold().face_y);
    for divisions in 1.. {
        let spacing = 2.0 * geometry.cylinder_radius / divisions as f32;
        let x_divisions = (2.0 * half_width / spacing).ceil() as usize;
        let y_divisions = (height / spacing).ceil() as usize;
        let x_spacing = 2.0 * half_width / x_divisions as f32;
        let y_spacing = height / y_divisions as f32;
        assert!(
            spacing.min(x_spacing).min(y_spacing) >= SPACING_PER_RADIUS * PARTICLE_RADIUS,
            "chamber is too small for {count} particles of radius {PARTICLE_RADIUS}"
        );
        let mut sites = Vec::new();
        for iy in 0..y_divisions {
            let y = geometry.floor() + (iy as f32 + 0.5) * y_spacing;
            for iz in 0..divisions {
                let z = -geometry.cylinder_radius + (iz as f32 + 0.5) * spacing;
                for ix in 0..x_divisions {
                    let position = vec3(-half_width + (ix as f32 + 0.5) * x_spacing, y, z);
                    if geometry.contains(position, PARTICLE_RADIUS, hot_top, cold_top) {
                        sites.push(position);
                    }
                }
            }
        }
        if sites.len() >= count {
            return sites;
        }
    }
    unreachable!("the lattice refines until it fits or fails the assertion")
}

/// Gas kinetic (internal) energy summed in f64, so per-turn differences of a
/// few units out of thousands stay accurate.
fn gas_energy(particles: &[Particle]) -> f64 {
    particles
        .iter()
        .map(|particle| gas::particle_kinetic_energy(particle.velocity) as f64)
        .sum()
}

#[cfg(test)]
#[path = "../../tests/unit/sim/engine.rs"]
mod tests;
