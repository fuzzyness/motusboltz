//! Coarse-grained helium markers, hard-sphere collisions, and wall heat.

use macroquad::prelude::Vec3;
use macroquad::rand::gen_range;

/// Marker count corresponding to an initial system-wide concentration of 1 mol/L.
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
pub const INITIAL_SPEED_LIMIT: f32 = 1.5;
pub const COLLISION_FLASH_DURATION: f32 = 0.01;
pub const MAX_HEATER_PERCENT: f32 = 100.0;

/// Common particle mass in reduced simulation units.
pub const PARTICLE_MASS: f32 = 1.0;

/// One rendered hard-sphere marker, representing a fraction of the helium gas.
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

/// Changes thermal kinetic energy while preserving mean particle velocity.
///
/// The reduced-unit temperature excludes the gas's center-of-mass motion.
/// Scaling velocity relative to that mean changes thermal energy without
/// imposing a net momentum impulse. This is an instantaneous temperature
/// adjustment, not a microscopic model of heat transfer through the walls.
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

/// A uniform collision grid for equal-radius hard spheres.
///
/// Every cell is at least one particle diameter wide, so overlapping
/// particles lie in the same cell or in immediately adjacent cells.
pub struct SpatialGrid {
    cells: Vec<Vec<usize>>,
    occupied_cells: Vec<usize>,
    cells_per_axis: usize,
    cell_size: f32,
    chamber_half_size: f32,
    particle_radius: f32,
}

impl SpatialGrid {
    /// Builds a cubic broad-phase grid around the cylindrical chamber.
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
        let cell_count = cells_per_axis.pow(3);

        Self {
            cells: vec![Vec::new(); cell_count],
            occupied_cells: Vec::new(),
            cells_per_axis,
            cell_size: chamber_size / cells_per_axis as f32,
            chamber_half_size: chamber_size / 2.0,
            particle_radius,
        }
    }

    fn flatten(&self, x: usize, y: usize, z: usize) -> usize {
        (z * self.cells_per_axis + y) * self.cells_per_axis + x
    }

    fn unflatten(&self, index: usize) -> (usize, usize, usize) {
        let side = self.cells_per_axis;
        let x = index % side;
        let y = (index / side) % side;
        let z = index / (side * side);
        (x, y, z)
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
        // Reuse buckets to avoid reallocating them every physics step.
        for &index in &self.occupied_cells {
            self.cells[index].clear();
        }
        self.occupied_cells.clear();

        for (particle_index, particle) in particles.iter().enumerate() {
            let cell_index = self.cell_index(particle.position);
            if self.cells[cell_index].is_empty() {
                self.occupied_cells.push(cell_index);
            }
            self.cells[cell_index].push(particle_index);
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
        let side = self.cells_per_axis;
        let mut check_pair = |first, second| {
            if Self::resolve_index_pair(particles, first, second, self.particle_radius) {
                on_collision(first, second);
            }
            candidate_checks += 1;
        };

        for &cell_index in &self.occupied_cells {
            let occupants = &self.cells[cell_index];

            // Each pair of occupants in this cell is visited once.
            for (offset, &first) in occupants.iter().enumerate() {
                for &second in &occupants[offset + 1..] {
                    check_pair(first, second);
                }
            }
            let (cx, cy, cz) = self.unflatten(cell_index);
            let min_x = cx.saturating_sub(1);
            let min_y = cy.saturating_sub(1);
            let min_z = cz.saturating_sub(1);
            let max_x = (cx + 1).min(side - 1);
            let max_y = (cy + 1).min(side - 1);
            let max_z = (cz + 1).min(side - 1);

            for z in min_z..=max_z {
                for y in min_y..=max_y {
                    for x in min_x..=max_x {
                        let neighbor_index = self.flatten(x, y, z);

                        // Avoid checking the same pair of cells in reverse order.
                        if neighbor_index <= cell_index {
                            continue;
                        }

                        for &first in occupants {
                            for &second in &self.cells[neighbor_index] {
                                check_pair(first, second);
                            }
                        }
                    }
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

/// Velocity moments for measuring temperature without allocating particle lists.
/// The mean flow velocity is subtracted from thermal motion in each region.
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

    /// Returns `None` when fewer than two simulation markers are sampled.
    pub fn temperature_kelvin(self) -> Option<f64> {
        if self.count < 2 {
            return None;
        }
        let count = self.count as f64;
        let mean_motion = self.velocity_sum.iter().map(|sum| sum * sum).sum::<f64>() / count;
        let thermal_motion = (self.speed_squared_sum - mean_motion).max(0.0);
        let reduced_temperature = thermal_motion / (THERMAL_DEGREES_OF_FREEDOM * (count - 1.0));
        Some(reduced_temperature * KELVIN_PER_UNIT)
    }
}

/// Local kinetic-temperature and ideal-gas pressure estimates for helium.
#[derive(Clone, Copy, Debug)]
pub struct GasMeasurements {
    pub markers: usize,
    pub temperature_kelvin: Option<f64>,
    pub concentration_mol_per_l: f64,
    pub pressure_pa: Option<f64>,
}

/// SI mapping with a fixed molar amount per rendered marker.
///
/// Each marker represents a fixed molar amount set by the initial fill volume.
/// At maximum marker capacity, the initial concentration is 1 mol/L across
/// the whole system. Increasing marker count increases represented helium amount.
#[derive(Clone, Copy, Debug)]
pub struct HeliumCalibration {
    moles_per_marker: f64,
}

impl HeliumCalibration {
    /// Fixes moles per marker using the total enclosed volume.
    pub fn new(total_volume_m3: f64, max_markers: usize, max_concentration: f64) -> Self {
        assert!(max_markers > 0);
        assert!(total_volume_m3.is_finite() && total_volume_m3 > 0.0);
        assert!(max_concentration.is_finite() && max_concentration > 0.0);
        let total_volume_liters = total_volume_m3 * 1_000.0;
        Self {
            moles_per_marker: max_concentration * total_volume_liters / max_markers as f64,
        }
    }

    pub fn total_moles(self, markers: usize) -> f64 {
        self.moles_per_marker * markers as f64
    }

    pub fn energy_joules(self, reduced_energy: f32) -> f64 {
        self.moles_per_marker * GAS_CONSTANT * KELVIN_PER_UNIT * reduced_energy as f64
    }

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

/// Room-temperature cold wall: 1 reduced temperature unit = 300 kelvin.
pub const COLD_TEMPERATURE: f32 = 1.0;
/// Full flame intensity: 4 reduced units = 1,200 kelvin.
pub const MAX_HOT_TEMPERATURE: f32 = 4.0;
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
    pub hot: f32,
    pub cold: f32,
}

impl HeatTransfer {
    pub fn total(self) -> f32 {
        self.hot + self.cold
    }
}

/// Wall temperatures and heat accounting for a sealed two-cylinder gas.
pub struct ThermalReservoirs {
    heater_percent: f32,
    accommodation: f32,
    heat: HeatTransfer,
}

impl ThermalReservoirs {
    pub fn new() -> Self {
        Self {
            heater_percent: 0.0,
            accommodation: ACCOMMODATION_PROBABILITY,
            heat: HeatTransfer::default(),
        }
    }

    /// Changes the hot reservoir temperature, not the particle velocities.
    pub fn set_heater_percent(&mut self, percent: f32) {
        assert!(percent.is_finite() && (0.0..=MAX_HEATER_PERCENT).contains(&percent));
        self.heater_percent = percent;
    }

    pub fn heater_percent(&self) -> f32 {
        self.heater_percent
    }

    pub fn hot_temperature(&self) -> f32 {
        COLD_TEMPERATURE
            + (MAX_HOT_TEMPERATURE - COLD_TEMPERATURE) * self.heater_percent / MAX_HEATER_PERCENT
    }

    pub fn heat(&self) -> HeatTransfer {
        self.heat
    }

    /// Reflects a particle from a stationary wall with an outward unit normal.
    ///
    /// Non-accommodating collisions remain perfectly elastic. Accommodating
    /// collisions sample inward normal speed from a flux-weighted Maxwellian;
    /// tangential velocity is unchanged. Heat is exactly the resulting change
    /// in particle kinetic energy. There is no instantaneous bulk heating.
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

        let outgoing_speed = if let Some(side) = side {
            if gen_range(0.0_f32, 1.0_f32) < self.accommodation {
                let temperature = match side {
                    ReservoirSide::Hot => self.hot_temperature(),
                    ReservoirSide::Cold => COLD_TEMPERATURE,
                };
                let outgoing_speed = sample_normal_speed(temperature);
                let delta_heat = 0.5
                    * PARTICLE_MASS
                    * (outgoing_speed * outgoing_speed - incoming_speed * incoming_speed);
                match side {
                    ReservoirSide::Hot => self.heat.hot += delta_heat,
                    ReservoirSide::Cold => self.heat.cold += delta_heat,
                }
                outgoing_speed
            } else {
                incoming_speed
            }
        } else {
            incoming_speed
        };

        particle.velocity -= outward_normal * (incoming_speed + outgoing_speed);
    }
}

/// Flux-weighted emission: probability density proportional to
/// v * exp(-m * v^2 / (2 * k_B * T)), with k_B = 1.
fn sample_normal_speed(temperature: f32) -> f32 {
    let uniform = gen_range(0.0_f32, 1.0_f32).max(f32::MIN_POSITIVE);
    (-2.0 * temperature * uniform.ln() / PARTICLE_MASS).sqrt()
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
