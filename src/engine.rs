//! Stirling chamber geometry, coupled pistons, and fixed-timestep simulation.

use std::f32::consts::{FRAC_PI_2, PI};

#[cfg(test)]
use std::f32::consts::TAU;

use macroquad::prelude::{
    Vec3,
    vec3, //
};
use macroquad::rand::{RandGenerator, gen_range};

use crate::cycle::{
    CycleMonitor,
    CycleTotals,
    Substep, //
};
use crate::gas as physics;
use crate::gas::{
    COLLISION_FLASH_DURATION,
    DYNAMICS_TIME_SCALE,
    GasMeasurements,
    HeatTransfer,
    HeliumCalibration,
    INITIAL_TEMPERATURE,
    KineticMoments,
    MAX_INITIAL_CONCENTRATION_MOL_PER_L,
    MAX_SIMULATED_MARKERS,
    METERS_PER_UNIT,
    MIN_CONNECTED_MARKERS,
    MIN_INITIAL_CONCENTRATION_MOL_PER_L,
    PARTICLE_MASS,
    Particle,
    ReservoirSide,
    SpatialGrid,
    ThermalReservoirs, //
};

/// Linear scale of the default chamber geometry.
pub const DEFAULT_CHAMBER_SCALE: f32 = 1.8;
/// Depth of the passage centerline below the cylinder floors, in passage
/// radii; must be at least one so the whole bottom run sits below the floors.
const CONNECTOR_DROP_RADII: f32 = 1.5;
/// Passage radius per unit of chamber scale; about half the bore radius, so
/// particles can move between cylinders within one engine cycle.
const CONNECTOR_RADIUS_PER_SCALE: f32 = 0.714;

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
            connector_center_y: -(1.8 + CONNECTOR_DROP_RADII * CONNECTOR_RADIUS_PER_SCALE) * scale,
            connector_radius: CONNECTOR_RADIUS_PER_SCALE * scale,
        }
    }

    /// Checks that the dimensions describe a usable two-cylinder chamber.
    ///
    /// # Panics
    ///
    /// Panics if any dimension is non-finite or too small for the particle
    /// radius, if the cylinders overlap, if the passage is not narrower than
    /// the bore, or if its horizontal run is not entirely below the floors.
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
        assert!(center_y + tube_radius <= -height / 2.0);
        assert!(tube_radius + 2.0 * particle_radius < height);
    }

    pub fn bottom(self) -> f32 {
        self.connector_center_y - self.connector_radius
    }

    /// Side of the cubic collision grid, centered on the origin; large enough
    /// to contain both cylinders and the entire lower passage.
    pub fn grid_side(self) -> f32 {
        (2.0 * (self.cylinder_offset + self.cylinder_radius))
            .max(self.cylinder_height)
            .max(-2.0 * self.bottom())
    }

    /// Centerlines of the three capsules forming the U: a leg down from each
    /// cylinder floor and the bottom run between them. Capsules overlap at
    /// the corners, which gives rounded elbows.
    pub fn connector_segments(self) -> [(Vec3, Vec3); 3] {
        let floor = -self.cylinder_height / 2.0;
        let left = vec3(-self.cylinder_offset, self.connector_center_y, 0.0);
        let right = vec3(self.cylinder_offset, self.connector_center_y, 0.0);
        [
            (vec3(left.x, floor, 0.0), left),
            (left, right),
            (right, vec3(right.x, floor, 0.0)),
        ]
    }

    /// Closest point on any passage centerline; shared by containment and
    /// wall projection.
    fn connector_axis_point(self, position: Vec3) -> Vec3 {
        let mut closest = Vec3::ZERO;
        let mut distance = f32::INFINITY;
        for (start, end) in self.connector_segments() {
            let axis = end - start;
            let point = start
                + axis * ((position - start).dot(axis) / axis.length_squared()).clamp(0.0, 1.0);
            let squared = position.distance_squared(point);
            if squared < distance {
                closest = point;
                distance = squared;
            }
        }
        closest
    }

    /// Exact volume of the union of the three capsules below the cylinder
    /// floors, with the elbow overlaps counted once. The capsule ends that
    /// poke above the floors lie inside the cylinders and are excluded.
    /// Assumes each leg is at least one passage radius long, which
    /// `validate` enforces.
    pub fn connector_volume_m3(self) -> f64 {
        let radius = self.connector_radius as f64;
        let leg = (-self.cylinder_height / 2.0 - self.connector_center_y) as f64;
        let pi = std::f64::consts::PI;
        (pi * radius.powi(2) * (2.0 * self.cylinder_offset as f64 + 2.0 * leg)
            + (2.0 * pi - 8.0) / 3.0 * radius.powi(3))
            * METERS_PER_UNIT.powi(3)
    }
}

/// Default engine dimensions.
pub const DEFAULT_GEOMETRY: ChamberGeometry = ChamberGeometry::scaled(DEFAULT_CHAMBER_SCALE);

impl Default for ChamberGeometry {
    fn default() -> Self {
        DEFAULT_GEOMETRY
    }
}

/// Requested piston travel; `PowerPistons::new` uses less if the geometry
/// cannot fit it with the required passage clearance.
const STROKE_METERS: f64 = 0.048;
/// Minimum gap between a piston at bottom dead center and the top of the
/// passage opening, in particle radii.
const PASSAGE_CLEARANCE_RADII: f32 = 2.0;
/// Flywheel inertia before pacing scaling, in reduced units. Large enough to
/// carry the crank through compression and to keep shaft motion slow compared
/// with gas transport through the passage and heating at the walls.
const REFERENCE_FLYWHEEL_INERTIA: f64 = 16_000.0;
const FLYWHEEL_INERTIA: f64 =
    REFERENCE_FLYWHEEL_INERTIA * (DYNAMICS_TIME_SCALE as f64) * (DYNAMICS_TIME_SCALE as f64);
/// Connecting-rod length divided by crank radius.
const ROD_TO_CRANK_RATIO: f32 = 3.0;
/// Bearing friction torque per unit angular speed, before pacing scaling.
const REFERENCE_BEARING_DRAG: f32 = 0.15;
/// External load (fan-like) torque per unit squared angular speed, before
/// pacing scaling.
const REFERENCE_LOAD_DRAG: f32 = 0.08;
// Scaled so drag torque at a given crank phase is unchanged by the pacing
// scale: the linear term scales once, the quadratic term twice.
const BEARING_DRAG: f32 = REFERENCE_BEARING_DRAG * DYNAMICS_TIME_SCALE;
const LOAD_DRAG: f32 = REFERENCE_LOAD_DRAG * DYNAMICS_TIME_SCALE * DYNAMICS_TIME_SCALE;
/// Flywheel energy one kick gives a shaft at rest, booked as starter work.
/// The 48 mm stroke needs this much to carry the crank past compression.
const STARTER_ENERGY: f64 = 1_600.0;

/// Which piston; the discriminants index per-cylinder arrays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PistonSide {
    Hot = crate::cycle::HOT as isize,
    Cold = crate::cycle::COLD as isize,
}

/// Direction of a manual kick. `Negative` is the heat-engine direction and
/// the only one the UI offers; `Positive` exists for mechanical tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KickDirection {
    #[cfg(test)]
    Positive,
    Negative,
}

impl KickDirection {
    fn sign(self) -> f32 {
        match self {
            #[cfg(test)]
            Self::Positive => 1.0,
            Self::Negative => -1.0,
        }
    }
}

/// Height of a piston face and its vertical velocity, in model units.
#[derive(Clone, Copy, Debug)]
pub struct PistonState {
    pub face_y: f32,
    pub velocity: f32,
}

/// Crankshaft, flywheel and both pistons. The pistons follow one crank angle
/// a quarter turn apart; only gas impacts, piston-back pressure, drag and
/// manual kicks change the shaft's speed. Field names `left`/`right` are the
/// hot/cold pistons.
pub struct PowerPistons {
    stroke: f32,
    rod_length: f32,
    shaft_y: f32,
    area: f32,
    angle: f32,
    angle_remainder: f64,
    angular_velocity: f64,
    inertia: f64,
    kick_speed: f32,
    total_angle: f64,
    reference_pressure: f32,
    running: bool,
    left: PistonState,
    right: PistonState,
    starter_work: f64,
    back_pressure_work: f64,
    drag_loss: f64,
    gas_impulse: [f64; 2],
}

impl PowerPistons {
    pub fn new(geometry: ChamberGeometry, particle_radius: f32) -> Self {
        let top = geometry.cylinder_height / 2.0;
        let tube_top = -geometry.cylinder_height / 2.0 + geometry.connector_radius;
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
            angle_remainder: 0.0,
            angular_velocity: 0.0,
            inertia: FLYWHEEL_INERTIA,
            kick_speed: (2.0 * STARTER_ENERGY / FLYWHEEL_INERTIA).sqrt() as f32,
            total_angle: 0.0,
            reference_pressure: 0.0,
            running: true,
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
            drag_loss: 0.0,
            gas_impulse: [0.0; 2],
        }
    }

    /// Sets the constant pressure pushing on the back of both pistons, in
    /// reduced units. The simulation uses the gas's initial pressure, so a
    /// pressurized but unheated gas cannot turn the crank just by pushing
    /// the pistons outward.
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

    /// Moves both faces to the current crank angle; velocity is the change
    /// over `dt`.
    fn update_faces(&mut self, dt: f32) {
        let left_y = self.position_at(self.angle);
        let right_y = self.position_at(self.angle + FRAC_PI_2);
        self.left.velocity = (left_y - self.left.face_y) / dt;
        self.right.velocity = (right_y - self.right.face_y) / dt;
        self.left.face_y = left_y;
        self.right.face_y = right_y;
    }

    /// Advances the crank by `dt`: applies bearing and load drag, then the
    /// piston-back pressure torque (half before and half after rotating),
    /// then moves the faces. Drag losses and back-pressure work are booked
    /// from the actual energy and volume changes. A held shaft does nothing.
    pub fn advance(&mut self, dt: f32) {
        assert!(dt.is_finite() && dt > 0.0);
        if !self.running {
            self.left.velocity = 0.0;
            self.right.velocity = 0.0;
            return;
        }
        let old_volume =
            self.area * (self.position_at(self.angle) + self.position_at(self.angle + FRAC_PI_2));
        let before = self.kinetic_energy();
        let speed = self.angular_velocity;
        let drag = BEARING_DRAG as f64 * speed + LOAD_DRAG as f64 * speed * speed.abs();
        let next = speed - drag * dt as f64 / self.inertia;
        self.angular_velocity = if speed.signum() == next.signum() {
            next
        } else {
            0.0
        };
        // Drag stops the shaft rather than reversing it within one step.
        self.drag_loss += (before - self.kinetic_energy()).max(0.0);

        self.angular_velocity +=
            self.back_pressure_torque() as f64 * dt as f64 / (2.0 * self.inertia);
        let rotation = self.angular_velocity * dt as f64;
        let next_angle =
            (self.angle as f64 + self.angle_remainder + rotation).rem_euclid(std::f64::consts::TAU);
        self.angle = next_angle as f32;
        self.angle_remainder = next_angle - self.angle as f64;
        self.total_angle += rotation;
        self.angular_velocity +=
            self.back_pressure_torque() as f64 * dt as f64 / (2.0 * self.inertia);
        self.update_faces(dt);
        let new_volume =
            self.area * (self.position_at(self.angle) + self.position_at(self.angle + FRAC_PI_2));
        // Booked from the displaced volume, independently of the torque above,
        // so the shaft energy balance can check one against the other.
        self.back_pressure_work -=
            self.reference_pressure as f64 * (new_volume as f64 - old_volume as f64);
    }

    fn back_pressure_torque(&self) -> f32 {
        -self.reference_pressure
            * self.area
            * (self.displacement_derivative(PistonSide::Hot)
                + self.displacement_derivative(PistonSide::Cold))
    }

    /// Returns a particle's new upward velocity after hitting a piston face,
    /// and applies the reaction impulse to the shaft. The collision is
    /// elastic between the particle and the shaft's effective mass at the
    /// current crank angle. Particles moving away from the face pass
    /// through unchanged; a held piston acts as a stationary wall.
    pub fn collide(&mut self, side: PistonSide, upward_speed: f32, particle_mass: f32) -> f32 {
        let wall_speed = self.face_velocity(side);
        let relative_speed = upward_speed - wall_speed;
        if relative_speed <= 0.0 {
            return upward_speed;
        }
        let inverse_mass = if self.running {
            (self.displacement_derivative(side) as f64).powi(2) / self.inertia
        } else {
            0.0
        };
        let impulse = 2.0 * relative_speed as f64 / (1.0 / particle_mass as f64 + inverse_mass);
        let new_speed = upward_speed as f64 - impulse / particle_mass as f64;
        if inverse_mass > 0.0 {
            self.angular_velocity +=
                impulse * self.displacement_derivative(side) as f64 / self.inertia;
            self.gas_impulse[side as usize] += impulse;
        }
        new_speed as f32
    }

    fn face_velocity(&self, side: PistonSide) -> f32 {
        if self.running {
            (self.displacement_derivative(side) as f64 * self.angular_velocity) as f32
        } else {
            0.0
        }
    }

    /// Adds a fixed change in angular speed, not a sustained motor. The
    /// resulting change in flywheel energy is booked as starter work, apart
    /// from gas work. From rest this adds `STARTER_ENERGY`.
    pub fn kick(&mut self, direction: KickDirection) {
        let before = self.kinetic_energy();
        self.angular_velocity += (direction.sign() * self.kick_speed) as f64;
        let work = self.kinetic_energy() - before;
        self.starter_work += work;
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
    /// Signed angular speed in radians per model-time unit; zero while held.
    pub fn angular_velocity(&self) -> f32 {
        if !self.running {
            0.0
        } else {
            self.angular_velocity as f32
        }
    }
    pub fn revolutions(&self) -> f64 {
        self.total_angle / std::f64::consts::TAU
    }
    /// Signed crank rotation since the fill, in radians.
    pub fn total_angle(&self) -> f64 {
        self.total_angle
    }
    /// Area of one piston face, in square model units.
    pub fn face_area(&self) -> f32 {
        self.area
    }
    /// Returns the upward impulse the gas has given each moving piston face
    /// (indexed hot, cold) since the last call, and resets it.
    pub fn take_gas_impulse(&mut self) -> [f64; 2] {
        std::mem::take(&mut self.gas_impulse)
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
    pub fn kinetic_energy(&self) -> f64 {
        0.5 * self.inertia * self.angular_velocity.powi(2)
    }
    pub fn starter_work(&self) -> f64 {
        self.starter_work
    }
    pub fn back_pressure_work(&self) -> f64 {
        self.back_pressure_work
    }
    /// Total flywheel energy removed by bearing friction and the external
    /// load since the last refill.
    pub fn drag_loss(&self) -> f64 {
        self.drag_loss
    }

    /// Gas volume below both piston faces plus the passage.
    pub fn volume_m3(&self, geometry: ChamberGeometry) -> f64 {
        self.cylinder_volume_m3(geometry, self.left)
            + self.cylinder_volume_m3(geometry, self.right)
            + geometry.connector_volume_m3()
    }

    /// Gas volume between one cylinder's floor and the given piston face.
    pub fn cylinder_volume_m3(&self, geometry: ChamberGeometry, piston: PistonState) -> f64 {
        let radius_m = geometry.cylinder_radius as f64 * METERS_PER_UNIT;
        let height_m = (piston.face_y + geometry.cylinder_height / 2.0) as f64 * METERS_PER_UNIT;
        std::f64::consts::PI * radius_m * radius_m * height_m
    }
}

const CUBIC_CENTIMETERS_PER_CUBIC_METER: f64 = 1.0e6;
/// Collision and rendering radius of every particle, at every fill level.
const PARTICLE_RADIUS: f32 = 0.13;
/// Initial fill lattice spacing must be at least radius / 0.4, so
/// neighboring particles start with a gap.
const PARTICLE_RADIUS_TO_SPACING: f32 = 0.4;
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
// Keep projected centers inside walls to avoid floating-point rounding escapes.
const WALL_PROJECTION_MARGIN: f32 = 16.0 * f32::EPSILON;

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
        let provisional_volume = pistons.volume_m3(geometry);
        let volume_units = provisional_volume / crate::gas::METERS_PER_UNIT.powi(3);
        pistons.set_reference_pressure(count as f32 / volume_units as f32);

        // Place particles on one lattice spanning both bores and the passage,
        // refining it until there are enough sites. A single lattice cannot
        // overlap itself at the junctions. Particles take evenly spaced sites.
        let mut divisions = 0_usize;
        let sites = loop {
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

        // Sample velocities at 300 K, then remove each region's mean velocity
        // and rescale, so every region starts at exactly 300 K with no flow.
        let mut left = Vec::new();
        let mut passage = Vec::new();
        let mut right = Vec::new();
        for index in 0..count {
            let site = ((2 * index + 1) * sites.len()) / (2 * count);
            let position = sites[site];
            let particle = Particle {
                position,
                velocity: physics::sample_thermal_velocity(INITIAL_TEMPERATURE, &random),
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
                physics::rescale_thermal_velocities(region, INITIAL_TEMPERATURE);
            }
        }
        let mut particles = Vec::with_capacity(count);
        particles.extend(left);
        particles.extend(passage);
        particles.extend(right);

        let initial_energy = physics::kinetic_energy(&particles);
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

    /// Rebuilds the simulation with a fresh random seed and `count`
    /// particles, keeping reservoir temperatures and the running state.
    /// Heat, work, kicks and the per-turn records restart from zero.
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

    /// Applies one manual kick to the flywheel (see `PowerPistons::kick`).
    pub fn kick_flywheel(&mut self, direction: KickDirection) {
        self.pistons.kick(direction);
    }

    /// Flywheel energy-balance residual, ideally zero: flywheel energy minus
    /// gas work, kicks and back-pressure work, plus drag losses.
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

    /// Converts a volume in cubic model units to cubic centimeters.
    pub fn volume_cm3(&self, cubic_units: f64) -> f64 {
        cubic_units * METERS_PER_UNIT.powi(3) * CUBIC_CENTIMETERS_PER_CUBIC_METER
    }

    /// Smallest and largest gas volume of one cylinder over a turn, in cm3.
    pub fn cylinder_volume_range_cm3(&self) -> (f64, f64) {
        let area = self.pistons.face_area() as f64;
        let height = self.geometry.cylinder_height as f64;
        let stroke = self.pistons.stroke() as f64;
        (
            self.volume_cm3(area * (height - stroke)),
            self.volume_cm3(area * height),
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
                    .cylinder_volume_m3(self.geometry, self.pistons.left()),
            ),
            passage_markers,
            cold: self.gas.measure(
                cold,
                self.pistons
                    .cylinder_volume_m3(self.geometry, self.pistons.right()),
            ),
        }
    }

    /// Helium amount represented by all particles.
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

    /// Whether a sphere of `radius` centered at `position` fits inside one
    /// cylinder, between its floor and the piston face at `top`.
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

    /// Whether a sphere of `radius` centered at `position` fits inside the
    /// passage.
    fn within_connector(geometry: ChamberGeometry, position: Vec3, radius: f32) -> bool {
        let limit = geometry.connector_radius - radius;
        position.distance_squared(geometry.connector_axis_point(position)) <= limit * limit
    }

    /// Whether a sphere fits anywhere in the connected gas volume.
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

    /// Classifies a particle center. A position inside either cylinder counts
    /// as that cylinder, so only the part of the passage below the floors
    /// counts as passage.
    pub fn region(&self, position: Vec3) -> Region {
        if Self::within_cylinder(
            self.geometry,
            position,
            -self.geometry.cylinder_offset,
            PARTICLE_RADIUS,
            self.pistons.left().face_y,
        ) {
            Region::Hot
        } else if Self::within_cylinder(
            self.geometry,
            position,
            self.geometry.cylinder_offset,
            PARTICLE_RADIUS,
            self.pistons.right().face_y,
        ) {
            Region::Cold
        } else {
            Region::Passage
        }
    }

    /// Closest position where a sphere fits inside one cylinder, kept a
    /// rounding margin inside the walls.
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

    /// Closest position where a sphere fits inside the passage, measured to
    /// the nearest centerline and kept a rounding margin inside the wall.
    fn project_to_connector(geometry: ChamberGeometry, position: Vec3, radius: f32) -> Vec3 {
        let center = geometry.connector_axis_point(position);
        let delta = position - center;
        let inset = (geometry.grid_side() * WALL_PROJECTION_MARGIN)
            .min((geometry.connector_radius - radius) * 0.5);
        let limit = geometry.connector_radius - radius - inset;
        center + delta * (limit / delta.length().max(limit)).min(1.0)
    }

    /// Whether an escaped position is past one cylinder's side wall or floor,
    /// the thermal surfaces. Piston faces are handled separately and never
    /// exchange heat.
    fn reservoir_at(geometry: ChamberGeometry, position: Vec3, center_x: f32, radius: f32) -> bool {
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

    /// Pulls an escaped particle back inside. With `reflect`, it also bounces
    /// off the wall it crossed, exchanging heat if that wall is thermal. Use
    /// `reflect = false` after particle collisions, which only nudge
    /// positions and must not count as extra wall impacts.
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
    /// particle moves more than half a radius relative to a piston and the
    /// crank turns at most 0.02 rad per substep.
    fn step(&mut self, dt: f32) {
        const MAX_TRAVEL_RADII: f32 = 0.5;
        const MAX_CRANK_STEP_RADIANS: f32 = 0.02;
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
        let left = self.pistons.left();
        let right = self.pistons.right();
        for particle in &mut self.particles {
            particle.position += particle.velocity * dt;
            let (center, face, side) = if particle.position.x < 0.0 {
                (-self.geometry.cylinder_offset, left, PistonSide::Hot)
            } else {
                (self.geometry.cylinder_offset, right, PistonSide::Cold)
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
                left.face_y,
                right.face_y,
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
                left.face_y,
                right.face_y,
                false,
            );
        }

        let substep = Substep {
            total_angle: self.pistons.total_angle(),
            crank_angle: self.pistons.angle(),
            dt: dt as f64,
            gas_impulse: self.pistons.take_gas_impulse(),
            volume: [self.pistons.left(), self.pistons.right()].map(|piston| {
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
        .map(|particle| physics::particle_kinetic_energy(particle.velocity) as f64)
        .sum()
}

#[cfg(test)]
#[path = "../tests/unit/power_pistons.rs"]
mod tests_power_pistons;

#[cfg(test)]
#[path = "../tests/unit/connected_chamber.rs"]
mod tests_connected_chamber;
