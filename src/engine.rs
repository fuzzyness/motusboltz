//! Stirling chamber geometry, coupled pistons, and fixed-timestep simulation.

use std::f32::consts::{
    FRAC_PI_2,
    PI,
    TAU, //
};

use macroquad::prelude::{
    Vec3,
    vec3, //
};
use macroquad::rand::gen_range;

use crate::gas as physics;
use crate::gas::{
    COLLISION_FLASH_DURATION,
    GasMeasurements,
    HeatTransfer,
    HeliumCalibration,
    INITIAL_SPEED_LIMIT,
    KineticMoments,
    MAX_INITIAL_CONCENTRATION_MOL_PER_L,
    MAX_SIMULATED_MARKERS,
    METERS_PER_UNIT,
    MIN_CONNECTED_MARKERS,
    MIN_INITIAL_CONCENTRATION_MOL_PER_L, //
    PARTICLE_MASS,
    Particle,
    ReservoirSide,
    SpatialGrid,
    ThermalReservoirs,
};

/// Linear scale of the default chamber geometry.
pub const DEFAULT_CHAMBER_SCALE: f32 = 1.8;
/// How far the connector extends past the facing cylinder walls into the bores.
const CONNECTOR_OVERLAP_RATIO: f32 = 0.4;

/// Dimensions of two matching cylinders and their lower connecting passage.
#[derive(Clone, Copy, Debug)]
pub struct ChamberGeometry {
    pub cylinder_offset: f32,
    pub cylinder_radius: f32,
    pub cylinder_height: f32,
    pub connector_center_y: f32,
    pub connector_radius: f32,
}

impl ChamberGeometry {
    /// Constructs uniformly scaled geometry; the public fields allow later
    /// independent cylinder and connector dimensions.
    pub const fn scaled(scale: f32) -> Self {
        Self {
            cylinder_offset: 2.1 * scale,
            cylinder_radius: 1.4 * scale,
            cylinder_height: 3.6 * scale,
            connector_center_y: -scale,
            connector_radius: 0.42 * scale,
        }
    }

    /// Validates chamber dimensions and a particle-accessible connecting passage.
    ///
    /// # Panics
    ///
    /// Panics if dimensions are invalid or the passage does not meet each bore.
    pub fn validate(self, particle_radius: f32) {
        let Self {
            cylinder_offset: offset,
            cylinder_radius: radius,
            cylinder_height: height,
            connector_center_y: center_y,
            connector_radius: tube_radius,
        } = self;
        assert!(particle_radius.is_finite() && particle_radius > 0.0);
        assert!(offset.is_finite() && radius.is_finite() && height.is_finite());
        assert!(center_y.is_finite() && tube_radius.is_finite());
        assert!(radius > particle_radius && height > 2.0 * particle_radius);
        assert!(offset > radius);
        assert!(tube_radius > particle_radius && tube_radius < radius);
        // The shortened ends must open fully into the sphere-accessible bores.
        let end_dx = radius - tube_radius * CONNECTOR_OVERLAP_RATIO;
        let opening_radius = tube_radius - particle_radius;
        let bore_radius = radius - particle_radius;
        assert!(end_dx.powi(2) + opening_radius.powi(2) < bore_radius.powi(2));
        // Keep the circular tube below the pistons and inside the bores.
        assert!(center_y - tube_radius > -height / 2.0);
        assert!(center_y + tube_radius < 0.0);
    }

    /// The enclosing cubic grid must cover either unusually tall or wide bores.
    pub fn grid_side(self) -> f32 {
        (2.0 * (self.cylinder_offset + self.cylinder_radius)).max(self.cylinder_height)
    }

    /// Tube end measured from the midpoint; each end sits just inside a bore.
    /// The small overlap leaves an open passage without reaching cylinder centers.
    pub fn connector_half_length(self) -> f32 {
        self.cylinder_offset - self.cylinder_radius
            + self.connector_radius * CONNECTOR_OVERLAP_RATIO
    }

    /// Approximate exposed tube volume between the two cylindrical bores.
    pub fn connector_volume_m3(self) -> f64 {
        let gap = 2.0 * (self.cylinder_offset - self.cylinder_radius) as f64;
        let radius = self.connector_radius as f64;
        gap * std::f64::consts::PI * radius * radius * METERS_PER_UNIT.powi(3)
    }
}

/// Default engine dimensions.
pub const DEFAULT_GEOMETRY: ChamberGeometry = ChamberGeometry::scaled(DEFAULT_CHAMBER_SCALE);

impl Default for ChamberGeometry {
    fn default() -> Self {
        DEFAULT_GEOMETRY
    }
}

/// Maximum requested travel; geometry may impose a smaller safe stroke.
const STROKE_METERS: f64 = 0.038;
const DRIVE_ANGULAR_SPEED: f32 = 0.30;
/// Minimum piston-to-connector clearance measured in sphere radii.
const PASSAGE_CLEARANCE_RADII: f32 = 2.0;
/// Flywheel inertia in reduced marker-mass x distance-squared units.
const FLYWHEEL_INERTIA: f32 = 400.0;
/// A finite connecting rod avoids an impossible telescoping linkage.
const ROD_TO_CRANK_RATIO: f32 = 3.0;
const BEARING_DRAG: f32 = 0.15;
const LOAD_DRAG: f32 = 0.08;
const MANUAL_KICK_SPEED: f32 = 0.8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionMode {
    Free,
    Controlled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PistonSide {
    Hot,
    Cold,
}

/// Direction of an optional, externally supplied angular impulse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KickDirection {
    Positive,
    Negative,
}

impl KickDirection {
    fn sign(self) -> f32 {
        match self {
            Self::Positive => 1.0,
            Self::Negative => -1.0,
        }
    }
}

/// Vertical face position and velocity in simulation units.
#[derive(Clone, Copy, Debug)]
pub struct PistonState {
    pub face_y: f32,
    pub velocity: f32,
}

/// Two pistons share an angle, angular velocity, and flywheel kinetic energy.
pub struct PowerPistons {
    stroke: f32,
    rod_length: f32,
    shaft_y: f32,
    area: f32,
    angle: f32,
    angular_velocity: f32,
    total_angle: f32,
    reference_pressure: f32,
    running: bool,
    mode: MotionMode,
    left: PistonState,
    right: PistonState,
    starter_work: f32,
    back_pressure_work: f32,
    load_energy: f32,
}

impl PowerPistons {
    pub fn new(geometry: ChamberGeometry, particle_radius: f32) -> Self {
        let top = geometry.cylinder_height / 2.0;
        let tube_top = geometry.connector_center_y + geometry.connector_radius;
        let safe_stroke = top - tube_top - PASSAGE_CLEARANCE_RADII * particle_radius;
        assert!(
            safe_stroke > 0.0,
            "connecting passage leaves no piston clearance"
        );
        let stroke = ((STROKE_METERS / METERS_PER_UNIT) as f32).min(safe_stroke);
        let crank_radius = stroke / 2.0;
        let rod_length = ROD_TO_CRANK_RATIO * crank_radius;
        let shaft_y = top + rod_length - crank_radius;
        let right_y = shaft_y - (rod_length * rod_length - crank_radius * crank_radius).sqrt();
        Self {
            stroke,
            rod_length,
            shaft_y,
            area: PI * geometry.cylinder_radius.powi(2),
            angle: 0.0,
            angular_velocity: 0.0,
            total_angle: 0.0,
            reference_pressure: 0.0,
            running: true,
            mode: MotionMode::Free,
            left: PistonState {
                face_y: top,
                velocity: 0.0,
            },
            right: PistonState {
                face_y: right_y,
                velocity: 0.0,
            },
            starter_work: 0.0,
            back_pressure_work: 0.0,
            load_energy: 0.0,
        }
    }

    /// Reference pressure acting on both piston backs in reduced units.
    /// At 300 K, the initial gas has approximately N / V reduced pressure.
    /// The opposing pressure prevents a pressurized but unheated gas from
    /// driving the crank merely because the upper faces are unconstrained.
    pub fn set_reference_pressure(&mut self, pressure: f32) {
        assert!(pressure.is_finite() && pressure >= 0.0);
        self.reference_pressure = pressure;
    }

    fn position_at(&self, angle: f32) -> f32 {
        let (sin, cos) = angle.sin_cos();
        let crank = self.stroke / 2.0;
        self.shaft_y + crank * cos - (self.rod_length.powi(2) - crank.powi(2) * sin * sin).sqrt()
    }

    fn derivative_at(&self, angle: f32) -> f32 {
        let (sin, cos) = angle.sin_cos();
        let crank = self.stroke / 2.0;
        let rod_vertical = (self.rod_length.powi(2) - crank.powi(2) * sin * sin).sqrt();
        -crank * sin + crank.powi(2) * sin * cos / rod_vertical
    }

    fn phase(side: PistonSide) -> f32 {
        match side {
            PistonSide::Hot => 0.0,
            PistonSide::Cold => FRAC_PI_2,
        }
    }

    pub fn displacement_derivative(&self, side: PistonSide) -> f32 {
        self.derivative_at(self.angle + Self::phase(side))
    }

    fn update_faces(&mut self, dt: f32) {
        let left_y = self.position_at(self.angle);
        let right_y = self.position_at(self.angle + FRAC_PI_2);
        self.left.velocity = (left_y - self.left.face_y) / dt;
        self.right.velocity = (right_y - self.right.face_y) / dt;
        self.left.face_y = left_y;
        self.right.face_y = right_y;
    }

    /// Evolves the single crank angle under load and piston-back pressure.
    /// Tracks drag losses and work from the prescribed piston-back pressure
    /// using changes in flywheel kinetic energy.
    pub fn advance(&mut self, dt: f32) {
        assert!(dt > 0.0);
        if !self.running {
            self.left.velocity = 0.0;
            self.right.velocity = 0.0;
            return;
        }
        match self.mode {
            MotionMode::Controlled => {
                let rotation = DRIVE_ANGULAR_SPEED * dt;
                self.angle = (self.angle + rotation).rem_euclid(TAU);
                self.total_angle += rotation;
            }
            MotionMode::Free => {
                let before = self.kinetic_energy();
                let speed = self.angular_velocity;
                let drag = BEARING_DRAG * speed + LOAD_DRAG * speed * speed.abs();
                let next = speed - drag * dt / FLYWHEEL_INERTIA;
                self.angular_velocity = if speed.signum() == next.signum() {
                    next
                } else {
                    0.0
                };
                let loss = (before - self.kinetic_energy()).max(0.0);
                self.load_energy += loss;

                let before = self.kinetic_energy();
                let volume_derivative = self.area
                    * (self.displacement_derivative(PistonSide::Hot)
                        + self.displacement_derivative(PistonSide::Cold));
                let opposing_torque = -self.reference_pressure * volume_derivative;
                self.angular_velocity += opposing_torque * dt / FLYWHEEL_INERTIA;
                let back_work = self.kinetic_energy() - before;
                self.back_pressure_work += back_work;
                let rotation = self.angular_velocity * dt;
                self.angle = (self.angle + rotation).rem_euclid(TAU);
                self.total_angle += rotation;
            }
        }
        self.update_faces(dt);
    }

    /// Reflects an upward-moving particle and applies its reaction impulse
    /// to the free flywheel. In controlled mode the wall is prescribed.
    pub fn collide(&mut self, side: PistonSide, upward_speed: f32, particle_mass: f32) -> f32 {
        let wall_speed = self.face_velocity(side);
        let relative_speed = upward_speed - wall_speed;
        if relative_speed <= 0.0 {
            return upward_speed;
        }
        let inverse_mass = if self.running && self.mode == MotionMode::Free {
            self.displacement_derivative(side).powi(2) / FLYWHEEL_INERTIA
        } else {
            0.0
        };
        let impulse = 2.0 * relative_speed / (1.0 / particle_mass + inverse_mass);
        let new_speed = upward_speed - impulse / particle_mass;
        if inverse_mass > 0.0 {
            self.angular_velocity +=
                impulse * self.displacement_derivative(side) / FLYWHEEL_INERTIA;
        }
        new_speed
    }

    fn face_velocity(&self, side: PistonSide) -> f32 {
        if !self.running {
            0.0
        } else if self.mode == MotionMode::Free {
            self.displacement_derivative(side) * self.angular_velocity
        } else {
            match side {
                PistonSide::Hot => self.left.velocity,
                PistonSide::Cold => self.right.velocity,
            }
        }
    }

    /// Manually applies a signed angular impulse, not a sustained motor.
    /// Its signed energy contribution is kept separate from heat-driven work.
    pub fn kick(&mut self, direction: KickDirection) {
        if self.mode != MotionMode::Free {
            return;
        }
        let before = self.kinetic_energy();
        self.angular_velocity += direction.sign() * MANUAL_KICK_SPEED;
        let work = self.kinetic_energy() - before;
        self.starter_work += work;
    }

    pub fn set_mode(&mut self, mode: MotionMode) {
        if self.mode != mode {
            self.mode = mode;
            self.angular_velocity = 0.0;
            self.total_angle = 0.0;
            self.left.velocity = 0.0;
            self.right.velocity = 0.0;
        }
    }

    pub fn mode(&self) -> MotionMode {
        self.mode
    }
    pub fn left(&self) -> PistonState {
        self.left
    }
    pub fn right(&self) -> PistonState {
        self.right
    }
    pub fn angle(&self) -> f32 {
        self.angle
    }
    /// Displayed angular speed (controlled mode uses the prescribed speed).
    pub fn angular_velocity(&self) -> f32 {
        if !self.running {
            0.0
        } else if self.mode == MotionMode::Controlled {
            DRIVE_ANGULAR_SPEED
        } else {
            self.angular_velocity
        }
    }
    pub fn revolutions(&self) -> f32 {
        self.total_angle / TAU
    }
    pub fn stroke(&self) -> f32 {
        self.stroke
    }
    pub fn shaft_height(&self) -> f32 {
        self.shaft_y
    }
    pub fn running(&self) -> bool {
        self.running
    }
    pub fn set_running(&mut self, running: bool) {
        self.running = running;
        if !running {
            self.left.velocity = 0.0;
            self.right.velocity = 0.0;
        }
    }
    pub fn kinetic_energy(&self) -> f32 {
        0.5 * FLYWHEEL_INERTIA * self.angular_velocity.powi(2)
    }
    pub fn starter_work(&self) -> f32 {
        self.starter_work
    }
    pub fn back_pressure_work(&self) -> f32 {
        self.back_pressure_work
    }
    pub fn load_energy(&self) -> f32 {
        self.load_energy
    }

    /// Working volume beneath both faces including the connecting passage.
    pub fn volume_m3(&self, geometry: ChamberGeometry) -> f64 {
        self.cylinder_volume_m3(geometry, self.left)
            + self.cylinder_volume_m3(geometry, self.right)
            + geometry.connector_volume_m3()
    }

    pub fn cylinder_volume_m3(&self, geometry: ChamberGeometry, piston: PistonState) -> f64 {
        let radius_m = geometry.cylinder_radius as f64 * METERS_PER_UNIT;
        let height_m = (piston.face_y + geometry.cylinder_height / 2.0) as f64 * METERS_PER_UNIT;
        std::f64::consts::PI * radius_m * radius_m * height_m
    }
}

/// Every particle preset uses this same collision and rendering radius.
const PARTICLE_RADIUS: f32 = 0.13;
/// Sphere radius is at most 40% of each lattice spacing.
const PARTICLE_RADIUS_TO_SPACING: f32 = 0.4;
const FIXED_STEPS_PER_SECOND: u32 = 120;
const FIXED_DT: f32 = 1.0 / FIXED_STEPS_PER_SECOND as f32;
const MAX_FRAME_TIME: f32 = 0.05;
// Keep projected centers inside walls to avoid floating-point rounding escapes.
const WALL_PROJECTION_MARGIN: f32 = 16.0 * f32::EPSILON;

/// Which part of the connected gas volume contains a particle center.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    Left,
    Connector,
    Right,
}

/// Inferred SI readings for each cylinder and a passage population count.
#[derive(Clone, Copy, Debug)]
pub struct ChamberMeasurements {
    pub left: GasMeasurements,
    pub passage_markers: usize,
    pub right: GasMeasurements,
}

/// Two connected cylinders with moving insulated caps and thermal walls.
pub struct ConnectedChambers {
    pub particles: Vec<Particle>,
    flashes: Vec<f32>,
    grid: SpatialGrid,
    geometry: ChamberGeometry,
    pistons: PowerPistons,
    #[cfg(test)]
    initial_volume_m3: f64,
    piston_work_by_gas: f32,
    accumulator: f32,
    thermal: ThermalReservoirs,
    gas: HeliumCalibration,
    initial_energy: f32,
}

impl ConnectedChambers {
    /// Initializes helium markers throughout both cylinders and the passage.
    pub fn new(count: usize) -> Self {
        Self::with_geometry(count, ChamberGeometry::default())
    }

    /// Creates a gas with the requested system-wide initial concentration.
    /// One molar is represented by the maximum simulated marker count.
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

    /// Refills the full system at the requested molarity, preserving flame level.
    pub fn set_initial_concentration(&mut self, concentration: f32) {
        self.set_particle_count(Self::markers_for_concentration(concentration));
    }

    /// Creates a gas with configurable dimensions and a fixed sphere radius.
    pub fn with_geometry(count: usize, geometry: ChamberGeometry) -> Self {
        assert!(
            count > 0 && count <= MAX_SIMULATED_MARKERS,
            "marker count must be between 1 and {MAX_SIMULATED_MARKERS}"
        );
        geometry.validate(PARTICLE_RADIUS);
        let mut pistons = PowerPistons::new(geometry, PARTICLE_RADIUS);
        // Opposing piston-back pressure at the initial 300 K mean density.
        // This is a mechanical boundary condition, not additional gas heat.
        // P = N kT / V, with kT = 1 in reduced units.
        let provisional_volume = pistons.volume_m3(geometry);
        let volume_units = provisional_volume / crate::gas::METERS_PER_UNIT.powi(3);
        pistons.set_reference_pressure(count as f32 / volume_units as f32);

        // Fill a single lattice across both bores and the open connecting
        // passage. The global spacing prevents overlaps at their junctions.
        let mut divisions = 0_usize;
        let sites = loop {
            divisions += 1;
            let spacing = 2.0 * geometry.cylinder_radius / divisions as f32;
            let half_width = geometry.cylinder_offset + geometry.cylinder_radius;
            let x_divisions = (2.0 * half_width / spacing).ceil() as usize;
            let y_divisions = (geometry.cylinder_height / spacing).ceil() as usize;
            let x_spacing = 2.0 * half_width / x_divisions as f32;
            let y_spacing = geometry.cylinder_height / y_divisions as f32;
            assert!(
                spacing.min(x_spacing).min(y_spacing) * PARTICLE_RADIUS_TO_SPACING
                    >= PARTICLE_RADIUS,
                "chamber is too small for {count} particles of radius {PARTICLE_RADIUS}; enlarge the geometry"
            );
            let mut sites = Vec::new();

            for iy in 0..y_divisions {
                let y = -geometry.cylinder_height / 2.0 + (iy as f32 + 0.5) * y_spacing;
                for iz in 0..divisions {
                    let z = -geometry.cylinder_radius + (iz as f32 + 0.5) * spacing;
                    for ix in 0..x_divisions {
                        let x = -half_width + (ix as f32 + 0.5) * x_spacing;
                        let position = vec3(x, y, z);
                        if Self::contains_at(
                            geometry,
                            position,
                            PARTICLE_RADIUS,
                            pistons.left().face_y,
                            pistons.right().face_y,
                        ) {
                            sites.push(position);
                        }
                    }
                }
            }

            if sites.len() >= count {
                break sites;
            }
        };

        // Thermalize each region independently so the two cylinders begin
        // at the same kinetic temperature (300 K), without bulk flow.
        let mut left = Vec::new();
        let mut passage = Vec::new();
        let mut right = Vec::new();
        for index in 0..count {
            let site = ((2 * index + 1) * sites.len()) / (2 * count);
            let position = sites[site];
            let particle = Particle {
                position,
                velocity: vec3(
                    gen_range(-INITIAL_SPEED_LIMIT, INITIAL_SPEED_LIMIT),
                    gen_range(-INITIAL_SPEED_LIMIT, INITIAL_SPEED_LIMIT),
                    gen_range(-INITIAL_SPEED_LIMIT, INITIAL_SPEED_LIMIT),
                ),
            };
            if Self::within_cylinder(
                geometry,
                position,
                -geometry.cylinder_offset,
                PARTICLE_RADIUS,
                pistons.left().face_y,
            ) {
                left.push(particle);
            } else if Self::within_cylinder(
                geometry,
                position,
                geometry.cylinder_offset,
                PARTICLE_RADIUS,
                pistons.right().face_y,
            ) {
                right.push(particle);
            } else {
                passage.push(particle);
            }
        }

        for region in [&mut left, &mut passage, &mut right] {
            if region.len() >= 2 {
                let mean = physics::mean_velocity(region);
                for particle in region.iter_mut() {
                    particle.velocity -= mean;
                }
                physics::rescale_thermal_velocities(region, 1.0);
            }
        }
        let mut particles = Vec::with_capacity(count);
        particles.extend(left);
        particles.extend(passage);
        particles.extend(right);

        let initial_energy = physics::kinetic_energy(&particles);
        let initial_volume_m3 = pistons.volume_m3(geometry);
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
            thermal: ThermalReservoirs::new(),
            gas: HeliumCalibration::new(
                initial_volume_m3,
                MAX_SIMULATED_MARKERS,
                MAX_INITIAL_CONCENTRATION_MOL_PER_L as f64,
            ),
            initial_energy,
        }
    }

    /// Refills the connected volume while preserving its user controls.
    fn refill(&mut self, count: usize) {
        let heater = self.thermal.heater_percent();
        let running = self.pistons.running();
        let mode = self.pistons.mode();
        *self = Self::with_geometry(count, self.geometry);
        self.thermal.set_heater_percent(heater);
        self.pistons.set_mode(mode);
        self.pistons.set_running(running);
    }

    /// Refills both cylinders and passage, retaining the helium amount.
    pub fn reset(&mut self) {
        self.refill(self.particles.len());
    }

    /// Refills the connected volume with a different number of markers.
    /// Each marker always represents the same amount for this chamber size.
    pub fn set_particle_count(&mut self, count: usize) {
        if count != self.particles.len() {
            self.refill(count);
        }
    }

    /// Selects the flame intensity without directly changing the gas energy.
    pub fn set_heater_percent(&mut self, percent: f32) {
        self.thermal.set_heater_percent(percent);
    }

    pub fn heater_percent(&self) -> f32 {
        self.thermal.heater_percent()
    }

    pub fn heat(&self) -> HeatTransfer {
        self.thermal.heat()
    }

    /// Returns the gas first-law residual: change in kinetic energy minus
    /// wall heat plus work transferred from the gas to the pistons.
    pub fn energy_balance_error(&self) -> f32 {
        self.kinetic_energy() - self.initial_energy - self.heat().total() + self.piston_work_by_gas
    }

    pub fn piston_work_by_gas(&self) -> f32 {
        self.piston_work_by_gas
    }

    pub fn power_pistons(&self) -> &PowerPistons {
        &self.pistons
    }

    pub fn set_pistons_running(&mut self, running: bool) {
        self.pistons.set_running(running);
    }

    /// Switching between measured free motion and kinematic validation also
    /// refills the gas, so mechanical work ledgers have a clean zero point.
    pub fn set_piston_mode(&mut self, mode: MotionMode) {
        if mode != self.pistons.mode() {
            let heater = self.heater_percent();
            let count = self.particles.len();
            let geometry = self.geometry;
            *self = Self::with_geometry(count, geometry);
            self.pistons.set_mode(mode);
            self.set_heater_percent(heater);
        }
    }

    /// Applies a diagnostic kick without engaging a continuous starter motor.
    pub fn kick_flywheel(&mut self, direction: KickDirection) {
        self.pistons.kick(direction);
    }

    /// Returns the flywheel energy-balance residual from piston work, back
    /// pressure, manual kicks, and drag losses.
    pub fn mechanical_balance_error(&self) -> f32 {
        self.pistons.kinetic_energy()
            - self.piston_work_by_gas
            - self.pistons.starter_work()
            - self.pistons.back_pressure_work()
            + self.pistons.load_energy()
    }

    #[cfg(test)]
    pub fn volume_m3(&self) -> f64 {
        self.pistons.volume_m3(self.geometry)
    }

    /// Current chamber dimensions, shared with rendering and diagnostics.
    pub fn geometry(&self) -> ChamberGeometry {
        self.geometry
    }

    /// Physical-energy scale for one coarse-grained helium marker.
    pub fn energy_joules(&self, reduced_energy: f32) -> f64 {
        self.gas.energy_joules(reduced_energy)
    }

    /// Estimates regional temperature and ideal-gas pressure after subtracting
    /// the mean velocity of each region.
    pub fn measurements(&self) -> ChamberMeasurements {
        let mut left = KineticMoments::default();
        let mut passage_markers = 0;
        let mut right = KineticMoments::default();

        for particle in &self.particles {
            match self.region(particle.position) {
                Region::Left => left.record(particle.velocity),
                Region::Connector => passage_markers += 1,
                Region::Right => right.record(particle.velocity),
            }
        }

        ChamberMeasurements {
            left: self.gas.measure(
                left,
                self.pistons
                    .cylinder_volume_m3(self.geometry, self.pistons.left()),
            ),
            passage_markers,
            right: self.gas.measure(
                right,
                self.pistons
                    .cylinder_volume_m3(self.geometry, self.pistons.right()),
            ),
        }
    }

    pub fn total_moles(&self) -> f64 {
        self.gas.total_moles(self.particles.len())
    }

    pub fn particle_radius(&self) -> f32 {
        PARTICLE_RADIUS
    }

    pub fn collision_flashes(&self) -> &[f32] {
        &self.flashes
    }

    pub fn kinetic_energy(&self) -> f32 {
        physics::kinetic_energy(&self.particles)
    }

    fn within_cylinder(
        geometry: ChamberGeometry,
        position: Vec3,
        center_x: f32,
        radius: f32,
        top: f32,
    ) -> bool {
        let dx = position.x - center_x;
        let radial_limit = geometry.cylinder_radius - radius;
        dx * dx + position.z * position.z <= radial_limit * radial_limit
            && (-geometry.cylinder_height / 2.0 + radius..=top - radius).contains(&position.y)
    }

    fn within_connector(geometry: ChamberGeometry, position: Vec3, radius: f32) -> bool {
        let dy = position.y - geometry.connector_center_y;
        let radial_limit = geometry.connector_radius - radius;
        (-geometry.connector_half_length()..=geometry.connector_half_length()).contains(&position.x)
            && dy * dy + position.z * position.z <= radial_limit * radial_limit
    }

    fn contains_at(
        geometry: ChamberGeometry,
        position: Vec3,
        radius: f32,
        left_top: f32,
        right_top: f32,
    ) -> bool {
        Self::within_cylinder(
            geometry,
            position,
            -geometry.cylinder_offset,
            radius,
            left_top,
        ) || Self::within_cylinder(
            geometry,
            position,
            geometry.cylinder_offset,
            radius,
            right_top,
        ) || Self::within_connector(geometry, position, radius)
    }

    /// Identifies a chamber first, counting only the exposed middle as passage.
    pub fn region(&self, position: Vec3) -> Region {
        if Self::within_cylinder(
            self.geometry,
            position,
            -self.geometry.cylinder_offset,
            PARTICLE_RADIUS,
            self.pistons.left().face_y,
        ) {
            Region::Left
        } else if Self::within_cylinder(
            self.geometry,
            position,
            self.geometry.cylinder_offset,
            PARTICLE_RADIUS,
            self.pistons.right().face_y,
        ) {
            Region::Right
        } else {
            Region::Connector
        }
    }

    /// Closest point in one inset cylinder to a particle center.
    fn project_to_cylinder(
        geometry: ChamberGeometry,
        position: Vec3,
        center_x: f32,
        radius: f32,
        top: f32,
    ) -> Vec3 {
        let dx = position.x - center_x;
        let dz = position.z;
        let radial_distance = (dx * dx + dz * dz).sqrt();
        // Rounding when adding `center_x` can otherwise put the returned
        // center a few ULPs beyond the exact radial containment boundary.
        let inset = (geometry.grid_side() * WALL_PROJECTION_MARGIN)
            .min((geometry.cylinder_radius - radius) * 0.5);
        let radial_limit = geometry.cylinder_radius - radius - inset;
        let scale = if radial_distance > radial_limit {
            radial_limit / radial_distance
        } else {
            1.0
        };

        vec3(
            center_x + dx * scale,
            position.y.clamp(
                -geometry.cylinder_height / 2.0 + radius + inset,
                top - radius - inset,
            ),
            dz * scale,
        )
    }

    /// Closest point inside the inset circular tube, oriented along X.
    fn project_to_connector(geometry: ChamberGeometry, position: Vec3, radius: f32) -> Vec3 {
        let dy = position.y - geometry.connector_center_y;
        let dz = position.z;
        let distance = (dy * dy + dz * dz).sqrt();
        let inset = (geometry.grid_side() * WALL_PROJECTION_MARGIN)
            .min((geometry.connector_radius - radius) * 0.5);
        let limit = geometry.connector_radius - radius - inset;
        let scale = if distance > limit {
            limit / distance
        } else {
            1.0
        };
        vec3(
            position.x.clamp(
                -geometry.connector_half_length(),
                geometry.connector_half_length(),
            ),
            geometry.connector_center_y + dy * scale,
            dz * scale,
        )
    }

    /// Thermalize the fixed cylinder sides and bottoms, never the pistons.
    fn reservoir_at(geometry: ChamberGeometry, position: Vec3, center_x: f32, radius: f32) -> bool {
        let dx = position.x - center_x;
        let limit = geometry.cylinder_radius - radius;
        dx * dx + position.z * position.z > limit * limit
            || position.y < -geometry.cylinder_height / 2.0 + radius
    }

    /// Projects onto the closest admissible region and labels its boundary.
    fn nearest_inside(
        geometry: ChamberGeometry,
        position: Vec3,
        radius: f32,
        left_top: f32,
        right_top: f32,
    ) -> (Vec3, Option<ReservoirSide>) {
        let left = Self::project_to_cylinder(
            geometry,
            position,
            -geometry.cylinder_offset,
            radius,
            left_top,
        );
        let right = Self::project_to_cylinder(
            geometry,
            position,
            geometry.cylinder_offset,
            radius,
            right_top,
        );
        let passage = Self::project_to_connector(geometry, position, radius);

        let mut nearest = left;
        let mut wall = Self::reservoir_at(geometry, position, -geometry.cylinder_offset, radius)
            .then_some(ReservoirSide::Hot);
        let mut distance_sq = position.distance_squared(left);
        for (candidate, surface) in [
            (
                right,
                Self::reservoir_at(geometry, position, geometry.cylinder_offset, radius)
                    .then_some(ReservoirSide::Cold),
            ),
            (passage, None),
        ] {
            let candidate_distance_sq = position.distance_squared(candidate);
            if candidate_distance_sq < distance_sq {
                nearest = candidate;
                wall = surface;
                distance_sq = candidate_distance_sq;
            }
        }
        (nearest, wall)
    }

    /// Repositions an escaped center and optionally reflects from its wall.
    /// Corrections after gas-gas collisions must not cause extra wall bounces.
    fn constrain_particle(
        particle: &mut Particle,
        geometry: ChamberGeometry,
        radius: f32,
        thermal: &mut ThermalReservoirs,
        left_top: f32,
        right_top: f32,
        reflect: bool,
    ) {
        if Self::contains_at(geometry, particle.position, radius, left_top, right_top) {
            return;
        }

        let (nearest, reservoir) =
            Self::nearest_inside(geometry, particle.position, radius, left_top, right_top);
        let displacement = particle.position - nearest;
        particle.position = nearest;
        if reflect && displacement.length_squared() > f32::EPSILON {
            thermal.reflect(particle, displacement.normalize(), reservoir);
        }
    }

    /// Reflects a marker from a piston, coupling its impulse to the free crankshaft.
    ///
    /// A paused piston is stationary and controlled motion is prescribed. The
    /// returned work is the negative of the marker's kinetic-energy change.
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

    /// Advances transport, moving piston bounces, and elastic gas collisions.
    pub fn update(&mut self, frame_time: f32) {
        self.accumulator += frame_time.min(MAX_FRAME_TIME);

        while self.accumulator >= FIXED_DT {
            self.pistons.advance(FIXED_DT);
            let left = self.pistons.left();
            let right = self.pistons.right();
            for particle in &mut self.particles {
                particle.position += particle.velocity * FIXED_DT;
                self.piston_work_by_gas += Self::bounce_piston(
                    particle,
                    self.geometry,
                    -self.geometry.cylinder_offset,
                    PARTICLE_RADIUS,
                    left,
                    PistonSide::Hot,
                    &mut self.pistons,
                );
                self.piston_work_by_gas += Self::bounce_piston(
                    particle,
                    self.geometry,
                    self.geometry.cylinder_offset,
                    PARTICLE_RADIUS,
                    right,
                    PistonSide::Cold,
                    &mut self.pistons,
                );
                Self::constrain_particle(
                    particle,
                    self.geometry,
                    PARTICLE_RADIUS,
                    &mut self.thermal,
                    left.face_y,
                    right.face_y,
                    true,
                );
            }

            for remaining in &mut self.flashes {
                *remaining = (*remaining - FIXED_DT).max(0.0);
            }

            let flashes = &mut self.flashes;
            self.grid.resolve_collisions(&mut self.particles, |a, b| {
                flashes[a] = COLLISION_FLASH_DURATION;
                flashes[b] = COLLISION_FLASH_DURATION;
            });

            // Pair overlap correction may have moved centers across a wall.
            for particle in &mut self.particles {
                Self::constrain_particle(
                    particle,
                    self.geometry,
                    PARTICLE_RADIUS,
                    &mut self.thermal,
                    left.face_y,
                    right.face_y,
                    false,
                );
            }

            self.accumulator -= FIXED_DT;
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/power_pistons.rs"]
mod tests_power_pistons;

#[cfg(test)]
#[path = "../tests/unit/connected_chamber.rs"]
mod tests_connected_chamber;
