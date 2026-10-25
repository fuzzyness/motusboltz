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
    INITIAL_TEMPERATURE,
    PARTICLE_MASS,
    Particle, //
};
use crate::sim::geometry::{
    ChamberGeometry,
    WALL_PROJECTION_MARGIN, //
};
use crate::sim::grid::SpatialGrid;
use crate::sim::pistons::{
    KickDirection,
    PistonSide,
    PistonState,
    PowerPistons,
    starting_energy, //
};
use crate::sim::reservoir::{
    HeatTransfer,
    ReservoirSide,
    ThermalReservoirs, //
};
use crate::sim::units::{
    GasMeasurements,
    HeliumCalibration,
    KineticMoments,
    MAX_INITIAL_CONCENTRATION_MOL_PER_L,
    MAX_SIMULATED_MARKERS,
    METERS_PER_UNIT,
    MIN_CONNECTED_MARKERS,
    MIN_INITIAL_CONCENTRATION_MOL_PER_L,
    volume_cm3, //
};

/// Collision and rendering radius of every particle, at every fill level.
pub const PARTICLE_RADIUS: f32 = 0.13;
/// Initial fill lattice spacing must be at least radius / 0.4, so
/// neighboring particles start with a gap.
const PARTICLE_RADIUS_TO_SPACING: f32 = 0.4;
/// How long, in model time, a particle stays highlighted after a collision.
/// The highlight is drawn only; it changes no energy.
pub const COLLISION_FLASH_DURATION: f32 = 0.01;
const FIXED_STEPS_PER_SECOND: u32 = 120;
const FIXED_DT: f32 = 1.0 / FIXED_STEPS_PER_SECOND as f32;
/// Most fixed steps run in one frame, bounding catch-up work so a slow
/// frame cannot create an ever-growing backlog. Six steps cover a 20 FPS
/// frame, so a display held at 30 FPS (for example Safari in Low Power
/// Mode) keeps real time even when its frames arrive unevenly.
const MAX_FIXED_STEPS_PER_FRAME: usize = 6;
/// Longest frame time accepted per update, exactly the step budget; longer
/// frames are clipped and reported as limited.
const MAX_FRAME_TIME: f32 = MAX_FIXED_STEPS_PER_FRAME as f32 * FIXED_DT;
/// Substep limits: no particle moves more than this many radii relative to
/// a piston, and the crank turns at most this far.
const MAX_TRAVEL_RADII: f32 = 0.5;
const MAX_CRANK_STEP_RADIANS: f32 = 0.02;

/// Which part of the connected gas volume contains a particle center.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    /// The heated cylinder, on the left (negative x).
    Hot,
    /// Below both cylinder floors, in the U passage.
    Passage,
    /// The cooled cylinder, on the right (positive x).
    Cold,
}

/// SI readings for each cylinder, and the passage's particle count.
#[derive(Clone, Copy, Debug)]
pub struct ChamberMeasurements {
    pub hot: GasMeasurements,
    pub passage_markers: usize,
    pub cold: GasMeasurements,
}

/// The complete simulation: gas particles in two cylinders joined by the U
/// passage, thermal cylinder walls, and pistons coupled to the flywheel.
/// Pistons and the passage are insulated; only cylinder sides and floors
/// exchange heat.
pub struct ConnectedChambers {
    pub particles: Vec<Particle>,
    flashes: Vec<f32>,
    grid: SpatialGrid,
    geometry: ChamberGeometry,
    pistons: PowerPistons,
    #[cfg(test)]
    initial_volume_m3: f64,
    piston_work_by_gas: f64,
    accumulator: f32,
    thermal: ThermalReservoirs,
    gas: HeliumCalibration,
    initial_energy: f32,
    cycle: CycleMonitor,
}

impl ConnectedChambers {
    /// Fills the default geometry with `count` particles using a random seed
    /// drawn from Macroquad's global generator.
    pub fn new(count: usize) -> Self {
        Self::with_geometry(count, ChamberGeometry::default())
    }

    /// Fills the default geometry at an initial concentration in mol/L.
    /// 1 mol/L corresponds to the maximum particle count.
    pub fn with_initial_concentration(concentration: f32) -> Self {
        Self::new(Self::markers_for_concentration(concentration))
    }

    fn markers_for_concentration(concentration: f32) -> usize {
        assert!(concentration.is_finite());
        assert!(
            (MIN_INITIAL_CONCENTRATION_MOL_PER_L..=MAX_INITIAL_CONCENTRATION_MOL_PER_L)
                .contains(&concentration)
        );
        ((concentration as f64 / MAX_INITIAL_CONCENTRATION_MOL_PER_L as f64
            * MAX_SIMULATED_MARKERS as f64)
            .round() as usize)
            .clamp(MIN_CONNECTED_MARKERS, MAX_SIMULATED_MARKERS)
    }

    /// Refills the whole system at a new concentration (mol/L) with a fresh
    /// random fill; reservoir temperatures and the running state are kept.
    pub fn set_initial_concentration(&mut self, concentration: f32) {
        self.set_particle_count(Self::markers_for_concentration(concentration));
    }

    /// Fills the given geometry with `count` particles using a random seed
    /// drawn from Macroquad's global generator.
    pub fn with_geometry(count: usize, geometry: ChamberGeometry) -> Self {
        Self::with_geometry_seed(count, geometry, gen_range(0, u64::MAX))
    }

    /// Fills the given geometry with `count` particles. The same seed gives
    /// the same initial velocities and wall-heating random stream, and each
    /// simulation has its own generator, independent of any other.
    pub fn with_geometry_seed(count: usize, geometry: ChamberGeometry, seed: u64) -> Self {
        let random = RandGenerator::new();
        random.srand(seed);
        assert!(
            count > 0 && count <= MAX_SIMULATED_MARKERS,
            "marker count must be between 1 and {MAX_SIMULATED_MARKERS}"
        );
        geometry.validate(PARTICLE_RADIUS);
        let mut pistons = PowerPistons::new(geometry, PARTICLE_RADIUS);
        // Piston-back pressure equals the initial gas pressure at 300 K:
        // P = N kT / V with kT = 1 in reduced units and V in model units.
        // This is a mechanical boundary condition, not a heat source.
        let volume_units = pistons.volume_m3(geometry) / METERS_PER_UNIT.powi(3);
        pistons.set_reference_pressure(count as f32 / volume_units as f32);
        pistons.set_starting_energy(starting_energy(count));

        let sites = Self::fill_sites(geometry, &pistons, count);
        // Sample velocities at 300 K, then remove each region's mean velocity
        // and rescale, so every region starts at exactly 300 K with no flow.
        let mut hot = Vec::new();
        let mut passage = Vec::new();
        let mut cold = Vec::new();
        for index in 0..count {
            let site = ((2 * index + 1) * sites.len()) / (2 * count);
            let position = sites[site];
            let particle = Particle {
                position,
                velocity: gas::sample_thermal_velocity(INITIAL_TEMPERATURE, &random),
            };
            if geometry.within_cylinder(
                position,
                geometry.hot_center_x(),
                PARTICLE_RADIUS,
                pistons.hot().face_y,
            ) {
                hot.push(particle);
            } else if geometry.within_cylinder(
                position,
                geometry.cold_center_x(),
                PARTICLE_RADIUS,
                pistons.cold().face_y,
            ) {
                cold.push(particle);
            } else {
                passage.push(particle);
            }
        }

        for region in [&mut hot, &mut passage, &mut cold] {
            if region.len() >= 2 {
                let mean = gas::mean_velocity(region);
                for particle in region.iter_mut() {
                    particle.velocity -= mean;
                }
                gas::rescale_thermal_velocities(region, INITIAL_TEMPERATURE);
            }
        }
        let mut particles = Vec::with_capacity(count);
        particles.extend(hot);
        particles.extend(passage);
        particles.extend(cold);

        let initial_energy = gas::kinetic_energy(&particles);
        let initial_volume_m3 = pistons.volume_m3(geometry);
        let cycle = CycleMonitor::new(
            pistons.face_area() as f64,
            CycleTotals {
                gas_energy: gas_energy(&particles),
                ..CycleTotals::default()
            },
        );
        Self {
            particles,
            flashes: vec![0.0; count],
            grid: SpatialGrid::new(geometry.grid_side(), PARTICLE_RADIUS),
            geometry,
            pistons,
            #[cfg(test)]
            initial_volume_m3,
            piston_work_by_gas: 0.0,
            accumulator: 0.0,
            thermal: ThermalReservoirs::with_seed(random.gen_range(0, u64::MAX)),
            gas: HeliumCalibration::new(
                initial_volume_m3,
                MAX_SIMULATED_MARKERS,
                MAX_INITIAL_CONCENTRATION_MOL_PER_L as f64,
            ),
            initial_energy,
            cycle,
        }
    }

    /// Starting positions for at least `count` particles: one lattice
    /// spanning both bores and the passage, refined until there are enough
    /// sites. A single lattice cannot overlap itself at the junctions.
    fn fill_sites(geometry: ChamberGeometry, pistons: &PowerPistons, count: usize) -> Vec<Vec3> {
        let mut divisions = 0_usize;
        loop {
            divisions += 1;
            let spacing = 2.0 * geometry.cylinder_radius / divisions as f32;
            let half_width = geometry.cylinder_offset + geometry.cylinder_radius;
            let x_divisions = (2.0 * half_width / spacing).ceil() as usize;
            let full_height = geometry.cylinder_height / 2.0 - geometry.bottom();
            let y_divisions = (full_height / spacing).ceil() as usize;
            let x_spacing = 2.0 * half_width / x_divisions as f32;
            let y_spacing = full_height / y_divisions as f32;
            assert!(
                spacing.min(x_spacing).min(y_spacing) * PARTICLE_RADIUS_TO_SPACING
                    >= PARTICLE_RADIUS,
                "chamber is too small for {count} particles of radius {PARTICLE_RADIUS}; enlarge the geometry"
            );
            let mut sites = Vec::new();

            for iy in 0..y_divisions {
                let y = geometry.bottom() + (iy as f32 + 0.5) * y_spacing;
                for iz in 0..divisions {
                    let z = -geometry.cylinder_radius + (iz as f32 + 0.5) * spacing;
                    for ix in 0..x_divisions {
                        let x = -half_width + (ix as f32 + 0.5) * x_spacing;
                        let position = vec3(x, y, z);
                        if geometry.contains(
                            position,
                            PARTICLE_RADIUS,
                            pistons.hot().face_y,
                            pistons.cold().face_y,
                        ) {
                            sites.push(position);
                        }
                    }
                }
            }

            if sites.len() >= count {
                return sites;
            }
        }
    }

    /// Rebuilds the simulation with a fresh random seed and `count`
    /// particles, keeping reservoir temperatures and the running state.
    /// Heat, work, starter work and the per-turn records restart from zero.
    fn refill(&mut self, count: usize) {
        let hot_kelvin = self.thermal.hot_kelvin();
        let running = self.pistons.running();
        let cold_kelvin = self.thermal.cold_kelvin();
        *self = Self::with_geometry(count, self.geometry);
        self.thermal.set_hot_kelvin(hot_kelvin);
        self.thermal.set_cold_kelvin(cold_kelvin);
        self.pistons.set_running(running);
    }

    /// Refills with the same particle count (same helium amount) at 300 K
    /// and a new random seed.
    pub fn reset(&mut self) {
        self.refill(self.particles.len());
    }

    /// Refills with a different particle count. Each particle represents a
    /// fixed amount of helium, so this changes the amount of gas.
    pub fn set_particle_count(&mut self, count: usize) {
        if count != self.particles.len() {
            self.refill(count);
        }
    }

    /// Sets the hot wall temperature; particles change only when they hit
    /// it. A change discards the per-turn history (see `CycleMonitor`).
    pub fn set_hot_kelvin(&mut self, kelvin: f32) {
        if kelvin != self.thermal.hot_kelvin() {
            self.cycle.mark_settings_changed();
        }
        self.thermal.set_hot_kelvin(kelvin);
    }

    /// Sets the cold wall temperature; particles change only when they hit
    /// it. A change discards the per-turn history (see `CycleMonitor`).
    pub fn set_cold_kelvin(&mut self, kelvin: f32) {
        if kelvin != self.thermal.cold_kelvin() {
            self.cycle.mark_settings_changed();
        }
        self.thermal.set_cold_kelvin(kelvin);
    }

    /// Per-turn heat, work, entropy and pressure-volume records.
    pub fn cycle(&self) -> &CycleMonitor {
        &self.cycle
    }

    pub fn cold_kelvin(&self) -> f32 {
        self.thermal.cold_kelvin()
    }

    pub fn hot_kelvin(&self) -> f32 {
        self.thermal.hot_kelvin()
    }

    /// Cumulative heat each reservoir has given the gas since the fill.
    pub fn heat(&self) -> HeatTransfer {
        self.thermal.heat()
    }

    /// Gas first-law residual, ideally zero: change in gas kinetic energy,
    /// minus wall heat into the gas, plus work done by the gas on the pistons.
    pub fn energy_balance_error(&self) -> f64 {
        self.kinetic_energy() as f64 - self.initial_energy as f64 - self.heat().total()
            + self.piston_work_by_gas
    }

    /// Cumulative work done by the gas on both pistons; negative when the
    /// pistons have done net work on the gas.
    pub fn piston_work_by_gas(&self) -> f64 {
        self.piston_work_by_gas
    }

    pub fn power_pistons(&self) -> &PowerPistons {
        &self.pistons
    }

    #[cfg(test)]
    pub fn set_pistons_running(&mut self, running: bool) {
        self.pistons.set_running(running);
    }

    /// Brings the flywheel up to the starting speed for this fill (see
    /// `PowerPistons::start`). Returns whether the speed changed.
    pub fn start_engine(&mut self) -> bool {
        self.pistons.start()
    }

    /// Applies one additive kick to the flywheel (see `PowerPistons::kick`).
    pub fn kick_flywheel(&mut self, direction: KickDirection) {
        self.pistons.kick(direction);
    }

    /// Flywheel energy-balance residual, ideally zero: flywheel energy minus
    /// gas work, starter work and back-pressure work, plus drag losses.
    pub fn mechanical_balance_error(&self) -> f64 {
        self.pistons.kinetic_energy()
            - self.piston_work_by_gas
            - self.pistons.starter_work()
            - self.pistons.back_pressure_work()
            + self.pistons.drag_loss()
    }

    #[cfg(test)]
    pub fn volume_m3(&self) -> f64 {
        self.pistons.volume_m3(self.geometry)
    }

    /// Current chamber dimensions, shared with rendering and diagnostics.
    pub fn geometry(&self) -> ChamberGeometry {
        self.geometry
    }

    /// Converts an energy in reduced simulation units to joules.
    pub fn energy_joules(&self, reduced_energy: impl Into<f64>) -> f64 {
        self.gas.energy_joules(reduced_energy)
    }

    /// Converts a reduced entropy (energy per reduced temperature) to J/K.
    pub fn entropy_joules_per_kelvin(&self, reduced_entropy: f64) -> f64 {
        self.gas.entropy_joules_per_kelvin(reduced_entropy)
    }

    /// Converts a reduced pressure to pascals.
    pub fn pressure_pa(&self, reduced_pressure: f64) -> f64 {
        self.gas.pressure_pa(reduced_pressure)
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

    /// Temperature, concentration and ideal-gas pressure for each cylinder,
    /// with each region's mean flow removed from its temperature; the
    /// passage reports only its particle count.
    pub fn measurements(&self) -> ChamberMeasurements {
        let mut hot = KineticMoments::default();
        let mut passage_markers = 0;
        let mut cold = KineticMoments::default();

        for particle in &self.particles {
            match self.region(particle.position) {
                Region::Hot => hot.record(particle.velocity),
                Region::Passage => passage_markers += 1,
                Region::Cold => cold.record(particle.velocity),
            }
        }

        ChamberMeasurements {
            hot: self.gas.measure(
                hot,
                self.pistons
                    .cylinder_volume_m3(self.geometry, self.pistons.hot()),
            ),
            passage_markers,
            cold: self.gas.measure(
                cold,
                self.pistons
                    .cylinder_volume_m3(self.geometry, self.pistons.cold()),
            ),
        }
    }

    /// Helium amount represented by all particles.
    pub fn total_moles(&self) -> f64 {
        self.gas.total_moles(self.particles.len())
    }

    /// Model time left on each particle's collision highlight.
    pub fn collision_flashes(&self) -> &[f32] {
        &self.flashes
    }

    /// Total kinetic (internal) energy of the gas.
    pub fn kinetic_energy(&self) -> f32 {
        gas::kinetic_energy(&self.particles)
    }

    /// Classifies a particle center. A position inside either cylinder counts
    /// as that cylinder, so only the part of the passage below the floors
    /// counts as passage.
    pub fn region(&self, position: Vec3) -> Region {
        let geometry = self.geometry;
        if geometry.within_cylinder(
            position,
            geometry.hot_center_x(),
            PARTICLE_RADIUS,
            self.pistons.hot().face_y,
        ) {
            Region::Hot
        } else if geometry.within_cylinder(
            position,
            geometry.cold_center_x(),
            PARTICLE_RADIUS,
            self.pistons.cold().face_y,
        ) {
            Region::Cold
        } else {
            Region::Passage
        }
    }

    /// Whether an escaped position is past one cylinder's side wall or floor,
    /// the thermal surfaces. Piston faces are handled separately and never
    /// exchange heat.
    fn beyond_thermal_wall(
        geometry: ChamberGeometry,
        position: Vec3,
        center_x: f32,
        radius: f32,
    ) -> bool {
        let dx = position.x - center_x;
        let limit = geometry.cylinder_radius - radius;
        dx * dx + position.z * position.z > limit * limit
            || position.y < -geometry.cylinder_height / 2.0 + radius
    }

    /// Moves an escaped position to the nearest point inside the cylinders or
    /// passage. Also returns which reservoir that wall belongs to: hot or
    /// cold for a cylinder's side or floor, `None` for the insulated passage.
    fn nearest_inside(
        geometry: ChamberGeometry,
        position: Vec3,
        radius: f32,
        hot_top: f32,
        cold_top: f32,
    ) -> (Vec3, Option<ReservoirSide>) {
        let candidates = [
            (
                geometry.project_to_cylinder(position, geometry.hot_center_x(), radius, hot_top),
                Self::beyond_thermal_wall(geometry, position, geometry.hot_center_x(), radius)
                    .then_some(ReservoirSide::Hot),
            ),
            (
                geometry.project_to_cylinder(position, geometry.cold_center_x(), radius, cold_top),
                Self::beyond_thermal_wall(geometry, position, geometry.cold_center_x(), radius)
                    .then_some(ReservoirSide::Cold),
            ),
            (geometry.project_to_connector(position, radius), None),
        ];
        let (mut nearest, mut wall) = candidates[0];
        let mut distance_sq = position.distance_squared(nearest);
        for (candidate, surface) in &candidates[1..] {
            let candidate_distance_sq = position.distance_squared(*candidate);
            if candidate_distance_sq < distance_sq {
                nearest = *candidate;
                wall = *surface;
                distance_sq = candidate_distance_sq;
            }
        }
        (nearest, wall)
    }

    /// Pulls an escaped particle back inside. With `reflect`, it also bounces
    /// off the wall it crossed, exchanging heat if that wall is thermal. Use
    /// `reflect = false` after particle collisions, which only nudge
    /// positions and must not count as extra wall impacts.
    fn constrain_particle(
        particle: &mut Particle,
        geometry: ChamberGeometry,
        radius: f32,
        thermal: &mut ThermalReservoirs,
        hot_top: f32,
        cold_top: f32,
        reflect: bool,
    ) {
        if geometry.contains(particle.position, radius, hot_top, cold_top) {
            return;
        }

        let (nearest, reservoir) =
            Self::nearest_inside(geometry, particle.position, radius, hot_top, cold_top);
        let displacement = particle.position - nearest;
        particle.position = nearest;
        if reflect && displacement.length_squared() > f32::EPSILON {
            thermal.reflect(particle, displacement.normalize(), reservoir);
        }
    }

    /// Bounces a particle that has reached a piston face and passes the
    /// impulse to the shaft. Returns the work done by the particle on the
    /// piston, which equals the particle's loss of kinetic energy.
    fn bounce_piston(
        particle: &mut Particle,
        geometry: ChamberGeometry,
        center_x: f32,
        radius: f32,
        piston: PistonState,
        side: PistonSide,
        flywheel: &mut PowerPistons,
    ) -> f32 {
        let radial_limit = geometry.cylinder_radius - radius;
        let dx = particle.position.x - center_x;
        if dx * dx + particle.position.z * particle.position.z > radial_limit * radial_limit
            || particle.position.y <= piston.face_y - radius
        {
            return 0.0;
        }

        particle.position.y =
            piston.face_y - radius - (geometry.grid_side() * WALL_PROJECTION_MARGIN);
        let old_speed = particle.velocity.y;
        let new_speed = flywheel.collide(side, old_speed, PARTICLE_MASS);
        particle.velocity.y = new_speed;
        0.5 * PARTICLE_MASS * (old_speed * old_speed - new_speed * new_speed)
    }

    /// Advances the simulation by one frame's worth of fixed steps, at most
    /// `MAX_FIXED_STEPS_PER_FRAME`. Returns true when frame time had to be
    /// dropped to keep the UI responsive; the step size never grows to catch up.
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

    /// Advances one fixed step, split into substeps short enough that no
    /// particle moves more than `MAX_TRAVEL_RADII` relative to a piston and
    /// the crank turns at most `MAX_CRANK_STEP_RADIANS` per substep.
    fn step(&mut self, dt: f32) {
        let mut remaining = dt;
        while remaining > 0.0 {
            let max_speed = self
                .particles
                .iter()
                .map(|p| p.velocity.length_squared())
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

    /// One substep: move the shaft, then move particles, bounce them off
    /// pistons and walls, then resolve particle-particle collisions.
    fn integrate(&mut self, dt: f32) {
        assert!(dt.is_finite() && dt > 0.0 && dt <= FIXED_DT);
        self.pistons.advance(dt);
        let hot = self.pistons.hot();
        let cold = self.pistons.cold();
        for particle in &mut self.particles {
            particle.position += particle.velocity * dt;
            let (center, face, side) = if particle.position.x < 0.0 {
                (self.geometry.hot_center_x(), hot, PistonSide::Hot)
            } else {
                (self.geometry.cold_center_x(), cold, PistonSide::Cold)
            };
            self.piston_work_by_gas += Self::bounce_piston(
                particle,
                self.geometry,
                center,
                PARTICLE_RADIUS,
                face,
                side,
                &mut self.pistons,
            ) as f64;
            Self::constrain_particle(
                particle,
                self.geometry,
                PARTICLE_RADIUS,
                &mut self.thermal,
                hot.face_y,
                cold.face_y,
                true,
            );
        }

        for remaining in &mut self.flashes {
            *remaining = (*remaining - dt).max(0.0);
        }

        let flashes = &mut self.flashes;
        self.grid.resolve_collisions(&mut self.particles, |a, b| {
            flashes[a] = COLLISION_FLASH_DURATION;
            flashes[b] = COLLISION_FLASH_DURATION;
        });

        // Collision overlap correction may have pushed centers past a wall.
        for particle in &mut self.particles {
            Self::constrain_particle(
                particle,
                self.geometry,
                PARTICLE_RADIUS,
                &mut self.thermal,
                hot.face_y,
                cold.face_y,
                false,
            );
        }

        let substep = Substep {
            total_angle: self.pistons.total_angle(),
            crank_angle: self.pistons.angle(),
            dt: dt as f64,
            gas_impulse: self.pistons.take_gas_impulse(),
            volume: [self.pistons.hot(), self.pistons.cold()].map(|piston| {
                self.pistons.cylinder_volume_m3(self.geometry, piston) / METERS_PER_UNIT.powi(3)
            }),
        };
        let kelvin = (self.thermal.hot_kelvin(), self.thermal.cold_kelvin());
        let (thermal, particles, gas_work) =
            (&self.thermal, &self.particles, self.piston_work_by_gas);
        self.cycle.record(substep, kelvin, || CycleTotals {
            hot_heat: thermal.heat().hot,
            cold_heat: thermal.heat().cold,
            hot_entropy: thermal.entropy().hot,
            cold_entropy: thermal.entropy().cold,
            gas_work,
            gas_energy: gas_energy(particles),
        });
    }
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
