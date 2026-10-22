//! Gas particles (each standing for an amount of helium), hard-sphere
//! collisions, SI unit conversion, and heat exchange at thermal walls.

use macroquad::prelude::Vec3;
use macroquad::rand::RandGenerator;
#[cfg(test)]
use macroquad::rand::gen_range;

/// Particle count corresponding to an initial system-wide concentration of
/// 1 mol/L; also the largest fill offered.
pub const MAX_SIMULATED_MARKERS: usize = 2_048;
pub const MIN_CONNECTED_MARKERS: usize = 256;
pub const DEFAULT_CONNECTED_MARKERS: usize = 1_024;
pub const MAX_INITIAL_CONCENTRATION_MOL_PER_L: f32 = 1.0;
pub const MIN_INITIAL_CONCENTRATION_MOL_PER_L: f32 = MAX_INITIAL_CONCENTRATION_MOL_PER_L
    * MIN_CONNECTED_MARKERS as f32
    / MAX_SIMULATED_MARKERS as f32;
pub const DEFAULT_INITIAL_CONCENTRATION_MOL_PER_L: f32 = MAX_INITIAL_CONCENTRATION_MOL_PER_L
    * DEFAULT_CONNECTED_MARKERS as f32
    / MAX_SIMULATED_MARKERS as f32;
/// Initial gas temperature in reduced units (1 = 300 K).
pub const INITIAL_TEMPERATURE: f32 = 1.0;
/// How long, in model time, a particle stays highlighted after a collision.
pub const COLLISION_FLASH_DURATION: f32 = 0.01;

/// Slows all motion by this factor so particles are easy to follow. Particle
/// mass and flywheel inertia scale with its square and drag scales to match
/// (engine.rs), so temperatures, energies and the ratio of gas speed to shaft
/// speed are unchanged; only the pace of the animation changes.
pub const DYNAMICS_TIME_SCALE: f32 = 6.0;
const REFERENCE_PARTICLE_MASS: f32 = 0.001;
pub const PARTICLE_MASS: f32 = REFERENCE_PARTICLE_MASS * DYNAMICS_TIME_SCALE * DYNAMICS_TIME_SCALE;

/// One simulated hard sphere, standing for a fixed amount of helium.
pub struct Particle {
    pub position: Vec3,
    pub velocity: Vec3,
}

const COLLISION_DISTANCE_EPSILON: f32 = 1e-6;
const BOLTZMANN_CONSTANT: f32 = 1.0;
const TRANSLATIONAL_DEGREES_OF_FREEDOM: f32 = 3.0;

/// Resolves an elastic collision between two equal-mass hard spheres.
///
/// Corrects overlap, then exchanges the normal velocity component of
/// approaching particles. Returns `true` only when velocities change due
/// to an actual collision; overlap correction alone is not a collision event.
pub fn resolve_pair(a: &mut Particle, b: &mut Particle, radius: f32) -> bool {
    let minimum_distance = 2.0 * radius;
    let displacement = b.position - a.position;
    let distance_squared = displacement.length_squared();

    if distance_squared >= minimum_distance * minimum_distance {
        return false;
    }

    let distance = distance_squared.sqrt();
    // A fixed fallback axis avoids division by zero for coincident centers.
    let normal = if distance > COLLISION_DISTANCE_EPSILON {
        displacement / distance
    } else {
        Vec3::X
    };

    let overlap = minimum_distance - distance;
    a.position -= normal * overlap * 0.5;
    b.position += normal * overlap * 0.5;

    let normal_speed = (b.velocity - a.velocity).dot(normal);
    if normal_speed >= 0.0 {
        return false;
    }

    a.velocity += normal * normal_speed;
    b.velocity -= normal * normal_speed;
    true
}

/// Returns translational kinetic energy in reduced simulation units.
pub(crate) fn particle_kinetic_energy(velocity: Vec3) -> f32 {
    0.5 * PARTICLE_MASS * velocity.length_squared()
}

/// Returns the arithmetic mean velocity of the particle sample.
pub(crate) fn mean_velocity(particles: &[Particle]) -> Vec3 {
    if particles.is_empty() {
        return Vec3::ZERO;
    }

    particles
        .iter()
        .map(|particle| particle.velocity)
        .sum::<Vec3>()
        / particles.len() as f32
}

/// Returns the sum of particle translational kinetic energies.
pub fn kinetic_energy(particles: &[Particle]) -> f32 {
    particles
        .iter()
        .map(|particle| particle_kinetic_energy(particle.velocity))
        .sum()
}

// The mean is supplied so velocity rescaling can reuse its existing value.
fn temperature_about_mean(particles: &[Particle], mean_velocity: Vec3) -> f32 {
    let thermal_energy: f32 = particles
        .iter()
        .map(|particle| particle_kinetic_energy(particle.velocity - mean_velocity))
        .sum();

    2.0 * thermal_energy
        / (TRANSLATIONAL_DEGREES_OF_FREEDOM * (particles.len() - 1) as f32 * BOLTZMANN_CONSTANT)
}

/// Scales velocities about their mean so the sample has exactly
/// `target_temperature` (reduced units), keeping total momentum unchanged.
///
/// Used only to set the initial 300 K fill. Heating while the engine runs
/// happens solely through wall impacts (`ThermalReservoirs::reflect`).
pub fn rescale_thermal_velocities(particles: &mut [Particle], target_temperature: f32) {
    assert!(target_temperature.is_finite() && target_temperature >= 0.0);

    let count = particles.len();
    if count < 2 {
        return;
    }

    let mean_velocity = mean_velocity(particles);

    let initial_temperature = temperature_about_mean(particles, mean_velocity);
    if initial_temperature > 0.0 {
        let scale = (target_temperature / initial_temperature).sqrt();

        for particle in particles {
            particle.velocity = mean_velocity + (particle.velocity - mean_velocity) * scale;
        }
    } else if target_temperature > 0.0 {
        // A gas with no thermal motion cannot be heated by multiplication.
        // Seed a balanced pair to generate the requested thermal energy.
        let thermal_speed = (TRANSLATIONAL_DEGREES_OF_FREEDOM
            * (count - 1) as f32
            * BOLTZMANN_CONSTANT
            * target_temperature
            / (2.0 * PARTICLE_MASS))
            .sqrt();

        particles[0].velocity = mean_velocity + Vec3::X * thermal_speed;
        particles[1].velocity = mean_velocity - Vec3::X * thermal_speed;
    }
}

const EMPTY_CELL: usize = usize::MAX;

/// Uniform grid that finds nearby particle pairs for collision checks.
///
/// Every cell is at least one particle diameter wide, so overlapping
/// particles lie in the same cell or in neighboring cells. Positions outside
/// the grid are clamped into its edge cells.
pub struct SpatialGrid {
    heads: Vec<usize>,
    next: Vec<usize>,
    occupied_cells: Vec<usize>,
    cells_per_axis: usize,
    cell_size: f32,
    padded_side: usize,
    neighbor_offsets: [isize; 13],
    chamber_half_size: f32,
    particle_radius: f32,
}

impl SpatialGrid {
    /// Builds a cubic grid of side `chamber_size`, centered on the origin.
    ///
    /// # Panics
    ///
    /// Panics if the chamber size or particle radius is nonpositive or
    /// non-finite.
    pub fn new(chamber_size: f32, particle_radius: f32) -> Self {
        assert!(chamber_size.is_finite() && chamber_size > 0.0);
        assert!(particle_radius.is_finite() && particle_radius > 0.0);

        let diameter = 2.0 * particle_radius;
        let cells_per_axis = ((chamber_size / diameter).floor() as usize).max(1);
        // Empty halo cells eliminate 13 sets of coordinate/boundary checks
        // for every occupied cell, without wrapping across opposite faces.
        const HALO_WIDTH: usize = 1;
        let padded_side = cells_per_axis + 2 * HALO_WIDTH;
        let row = padded_side as isize;
        let plane = row * row;
        let neighbor_offsets = [
            1,
            row - 1,
            row,
            row + 1,
            plane - row - 1,
            plane - row,
            plane - row + 1,
            plane - 1,
            plane,
            plane + 1,
            plane + row - 1,
            plane + row,
            plane + row + 1,
        ];
        let cell_count = padded_side.pow(3);

        Self {
            heads: vec![EMPTY_CELL; cell_count],
            next: Vec::new(),
            occupied_cells: Vec::new(),
            cells_per_axis,
            padded_side,
            neighbor_offsets,
            cell_size: chamber_size / cells_per_axis as f32,
            chamber_half_size: chamber_size / 2.0,
            particle_radius,
        }
    }

    fn flatten(&self, x: usize, y: usize, z: usize) -> usize {
        ((z + 1) * self.padded_side + y + 1) * self.padded_side + x + 1
    }

    fn axis_cell(&self, coordinate: f32) -> usize {
        let index = ((coordinate + self.chamber_half_size) / self.cell_size).floor();
        index.clamp(0.0, (self.cells_per_axis - 1) as f32) as usize
    }

    fn cell_index(&self, position: Vec3) -> usize {
        self.flatten(
            self.axis_cell(position.x),
            self.axis_cell(position.y),
            self.axis_cell(position.z),
        )
    }

    fn rebuild(&mut self, particles: &[Particle]) {
        // Flat linked lists avoid per-cell allocations; clear only occupied cells.
        self.next.resize(particles.len(), EMPTY_CELL);
        for &index in &self.occupied_cells {
            self.heads[index] = EMPTY_CELL;
        }
        self.occupied_cells.clear();

        for (particle_index, particle) in particles.iter().enumerate() {
            let cell_index = self.cell_index(particle.position);
            if self.heads[cell_index] == EMPTY_CELL {
                self.occupied_cells.push(cell_index);
            }
            self.next[particle_index] = self.heads[cell_index];
            self.heads[cell_index] = particle_index;
        }
    }

    /// Resolves nearby pairs, invoking `on_collision` for actual impacts.
    ///
    /// Each pair is checked once. The callback receives particle indices only
    /// if the collision changes their velocities, not for overlap correction.
    /// Returns the total number of candidate pairs checked.
    pub fn resolve_collisions<F>(
        &mut self,
        particles: &mut [Particle],
        mut on_collision: F,
    ) -> usize
    where
        F: FnMut(usize, usize),
    {
        self.rebuild(particles);

        let mut candidate_checks = 0;
        let mut check_pair = |first, second| {
            if Self::resolve_index_pair(particles, first, second, self.particle_radius) {
                on_collision(first, second);
            }
            candidate_checks += 1;
        };

        // Offsets preserve the original forward-neighbor collision order.
        for &cell_index in &self.occupied_cells {
            let mut first = self.heads[cell_index];
            while first != EMPTY_CELL {
                let mut second = self.next[first];
                while second != EMPTY_CELL {
                    check_pair(first, second);
                    second = self.next[second];
                }
                first = self.next[first];
            }
            for &offset in &self.neighbor_offsets {
                let neighbor = (cell_index as isize + offset) as usize;
                if self.heads[neighbor] == EMPTY_CELL {
                    continue;
                }
                let mut first = self.heads[cell_index];
                while first != EMPTY_CELL {
                    let mut second = self.heads[neighbor];
                    while second != EMPTY_CELL {
                        check_pair(first, second);
                        second = self.next[second];
                    }
                    first = self.next[first];
                }
            }
        }

        candidate_checks
    }

    fn resolve_index_pair(
        particles: &mut [Particle],
        first: usize,
        second: usize,
        radius: f32,
    ) -> bool {
        // `split_at_mut()` guarantees disjoint mutable particle references.
        let (a, b) = if first < second {
            let (left, right) = particles.split_at_mut(second);
            (&mut left[first], &mut right[0])
        } else {
            let (left, right) = particles.split_at_mut(first);
            (&mut right[0], &mut left[second])
        };

        resolve_pair(a, b, radius)
    }
}

/// Molar gas constant, in joules per mole-kelvin.
pub const GAS_CONSTANT: f64 = 8.314_462_618_153_24;
/// Physical distance corresponding to one rendered simulation distance unit.
pub const METERS_PER_UNIT: f64 = 0.01;
/// One reduced unit of temperature represents 300 kelvin.
pub const KELVIN_PER_UNIT: f64 = 300.0;

const THERMAL_DEGREES_OF_FREEDOM: f64 = 3.0;

/// Running sums of particle velocities, for measuring a region's temperature
/// without collecting its particles. Temperature uses motion relative to the
/// region's mean velocity, so bulk flow does not count as heat.
#[derive(Clone, Copy, Debug, Default)]
pub struct KineticMoments {
    count: usize,
    velocity_sum: [f64; 3],
    speed_squared_sum: f64,
}

impl KineticMoments {
    pub fn record(&mut self, velocity: Vec3) {
        self.count += 1;
        let components = [velocity.x as f64, velocity.y as f64, velocity.z as f64];
        for (total, component) in self.velocity_sum.iter_mut().zip(components) {
            *total += component;
            self.speed_squared_sum += component * component;
        }
    }

    pub fn count(self) -> usize {
        self.count
    }

    /// Kinetic temperature in kelvin; `None` with fewer than two particles.
    pub fn temperature_kelvin(self) -> Option<f64> {
        if self.count < 2 {
            return None;
        }
        let count = self.count as f64;
        let mean_motion = self.velocity_sum.iter().map(|sum| sum * sum).sum::<f64>() / count;
        let thermal_motion = (self.speed_squared_sum - mean_motion).max(0.0);
        let reduced_temperature = thermal_motion / (THERMAL_DEGREES_OF_FREEDOM * (count - 1.0));
        Some(reduced_temperature * PARTICLE_MASS as f64 * KELVIN_PER_UNIT)
    }
}

/// Readings for one region. Pressure is the ideal-gas estimate nRT/V, not a
/// measured force on the walls.
#[derive(Clone, Copy, Debug)]
pub struct GasMeasurements {
    pub markers: usize,
    pub temperature_kelvin: Option<f64>,
    pub concentration_mol_per_l: f64,
    pub pressure_pa: Option<f64>,
}

/// Converts between simulation units and SI using a fixed amount of helium
/// per particle.
///
/// The amount is chosen so the maximum particle count fills the initial
/// volume at the maximum concentration (1 mol/L). More particles therefore
/// means more helium.
#[derive(Clone, Copy, Debug)]
pub struct HeliumCalibration {
    moles_per_marker: f64,
}

impl HeliumCalibration {
    /// Sets moles per particle = max concentration x volume / max particles.
    pub fn new(total_volume_m3: f64, max_markers: usize, max_concentration: f64) -> Self {
        assert!(max_markers > 0);
        assert!(total_volume_m3.is_finite() && total_volume_m3 > 0.0);
        assert!(max_concentration.is_finite() && max_concentration > 0.0);
        let total_volume_liters = total_volume_m3 * 1_000.0;
        Self {
            moles_per_marker: max_concentration * total_volume_liters / max_markers as f64,
        }
    }

    /// Helium amount represented by `markers` particles.
    pub fn total_moles(self, markers: usize) -> f64 {
        self.moles_per_marker * markers as f64
    }

    /// Converts reduced energy to joules: one reduced unit per particle is
    /// R x 300 K per mole.
    pub fn energy_joules(self, reduced_energy: impl Into<f64>) -> f64 {
        self.moles_per_marker * GAS_CONSTANT * KELVIN_PER_UNIT * reduced_energy.into()
    }

    /// Readings for a region's particles occupying `volume_m3`.
    pub fn measure(self, sample: KineticMoments, volume_m3: f64) -> GasMeasurements {
        assert!(volume_m3 > 0.0);
        let moles = self.total_moles(sample.count());
        let temperature_kelvin = sample.temperature_kelvin();
        let pressure_pa =
            temperature_kelvin.map(|temperature| moles * GAS_CONSTANT * temperature / volume_m3);
        GasMeasurements {
            markers: sample.count(),
            temperature_kelvin,
            concentration_mol_per_l: moles / (volume_m3 * 1_000.0),
            pressure_pa,
        }
    }
}

/// Lowest hot-reservoir temperature, 300 K, equal to the highest cold one so
/// both reservoirs can be set equal.
pub const MIN_HOT_KELVIN: f32 = KELVIN_PER_UNIT as f32;
/// Lowest cold-reservoir temperature.
pub const MIN_COLD_KELVIN: f32 = 100.0;
/// Highest cold-reservoir temperature, 300 K.
pub const MAX_COLD_KELVIN: f32 = KELVIN_PER_UNIT as f32;
/// Highest hot-reservoir boundary temperature.
pub const MAX_HOT_KELVIN: f32 = 1_200.0;
/// Chance that a cylinder-wall impact exchanges heat with its reservoir.
const ACCOMMODATION_PROBABILITY: f32 = 0.5;

/// Which thermal reservoir supplies or absorbs energy at a wall impact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReservoirSide {
    Hot,
    Cold,
}

/// Cumulative signed heat delivered to the gas, in reduced energy units.
/// Negative values mean energy has left the gas.
#[derive(Clone, Copy, Debug, Default)]
pub struct HeatTransfer {
    pub hot: f64,
    pub cold: f64,
}

impl HeatTransfer {
    pub fn total(self) -> f64 {
        self.hot + self.cold
    }
}

/// Hot and cold wall temperatures, the random stream for wall impacts, and
/// the cumulative heat each reservoir has given the gas. Both start at 300 K.
pub struct ThermalReservoirs {
    hot_kelvin: f32,
    cold_kelvin: f32,
    accommodation: f32,
    heat: HeatTransfer,
    random: RandGenerator,
}

impl ThermalReservoirs {
    #[cfg(test)]
    pub fn new() -> Self {
        Self::with_seed(gen_range(0, u64::MAX))
    }

    /// Reservoirs at 300 K whose wall impacts use their own seeded generator.
    pub fn with_seed(seed: u64) -> Self {
        let random = RandGenerator::new();
        random.srand(seed);
        Self {
            random,
            hot_kelvin: MIN_HOT_KELVIN,
            cold_kelvin: MAX_COLD_KELVIN,
            accommodation: ACCOMMODATION_PROBABILITY,
            heat: HeatTransfer::default(),
        }
    }

    /// Sets the hot wall temperature; particle velocities are not touched.
    pub fn set_hot_kelvin(&mut self, kelvin: f32) {
        assert!(kelvin.is_finite() && (MIN_HOT_KELVIN..=MAX_HOT_KELVIN).contains(&kelvin));
        self.hot_kelvin = kelvin;
    }

    /// Sets the cold wall temperature; particle velocities are not touched.
    pub fn set_cold_kelvin(&mut self, kelvin: f32) {
        assert!(kelvin.is_finite() && (MIN_COLD_KELVIN..=MAX_COLD_KELVIN).contains(&kelvin));
        self.cold_kelvin = kelvin;
    }

    pub fn cold_kelvin(&self) -> f32 {
        self.cold_kelvin
    }

    pub fn hot_kelvin(&self) -> f32 {
        self.hot_kelvin
    }

    pub fn heat(&self) -> HeatTransfer {
        self.heat
    }

    /// Bounces a particle off a stationary wall with the given outward unit
    /// normal. Particles already moving away from the wall are unchanged.
    ///
    /// With `side = None` (insulated wall), or when the accommodation draw
    /// fails, the bounce is a mirror reflection with no energy change.
    /// Otherwise the particle leaves with a new velocity drawn at that
    /// reservoir's temperature: a flux-weighted Maxwellian normal speed and
    /// Gaussian tangential components. Its change in kinetic energy is
    /// booked as heat from that reservoir.
    pub fn reflect(
        &mut self,
        particle: &mut Particle,
        outward_normal: Vec3,
        side: Option<ReservoirSide>,
    ) {
        let incoming_speed = particle.velocity.dot(outward_normal);
        if incoming_speed <= 0.0 {
            return;
        }

        if let Some(side) =
            side.filter(|_| self.random.gen_range(0.0_f32, 1.0_f32) < self.accommodation)
        {
            let temperature = match side {
                ReservoirSide::Hot => self.hot_kelvin / KELVIN_PER_UNIT as f32,
                ReservoirSide::Cold => self.cold_kelvin / KELVIN_PER_UNIT as f32,
            };
            let before = particle_kinetic_energy(particle.velocity);
            // Choose a well-conditioned tangent basis for diffuse reflection.
            const BASIS_ALIGNMENT_LIMIT: f32 = 0.9;
            let axis = if outward_normal.x.abs() < BASIS_ALIGNMENT_LIMIT {
                Vec3::X
            } else {
                Vec3::Y
            };
            let tangent = outward_normal.cross(axis).normalize();
            let bitangent = outward_normal.cross(tangent);
            let (u, v) = sample_gaussian_pair(temperature, &self.random);
            particle.velocity = -outward_normal * sample_normal_speed(temperature, &self.random)
                + tangent * u
                + bitangent * v;
            let heat = (particle_kinetic_energy(particle.velocity) - before) as f64;
            match side {
                ReservoirSide::Hot => self.heat.hot += heat,
                ReservoirSide::Cold => self.heat.cold += heat,
            }
            return;
        }
        particle.velocity -= 2.0 * incoming_speed * outward_normal;
    }
}

/// Flux-weighted emission: probability density proportional to
/// v * exp(-m * v^2 / (2 * k_B * T)), with k_B = 1.
fn sample_normal_speed(temperature: f32, random: &RandGenerator) -> f32 {
    let uniform = random.gen_range(0.0_f32, 1.0_f32).max(f32::MIN_POSITIVE);
    (-2.0 * temperature * uniform.ln() / PARTICLE_MASS).sqrt()
}

/// Two independent Gaussian components with variance kT/m (Box-Muller form).
fn sample_gaussian_pair(temperature: f32, random: &RandGenerator) -> (f32, f32) {
    let radius = sample_normal_speed(temperature, random);
    let angle = random.gen_range(0.0_f32, std::f32::consts::TAU);
    let (sin, cos) = angle.sin_cos();
    (radius * cos, radius * sin)
}

/// Random 3D velocity from the Maxwell distribution at `temperature`.
pub(crate) fn sample_thermal_velocity(temperature: f32, random: &RandGenerator) -> Vec3 {
    let (x, y) = sample_gaussian_pair(temperature, random);
    let (z, _) = sample_gaussian_pair(temperature, random);
    Vec3::new(x, y, z)
}

#[cfg(test)]
#[path = "../tests/unit/physics.rs"]
mod tests_physics;

#[cfg(test)]
#[path = "../tests/unit/spatial_grid.rs"]
mod tests_spatial_grid;

#[cfg(test)]
#[path = "../tests/unit/working_gas.rs"]
mod tests_working_gas;

#[cfg(test)]
#[path = "../tests/unit/thermal_reservoir.rs"]
mod tests_thermal_reservoir;
