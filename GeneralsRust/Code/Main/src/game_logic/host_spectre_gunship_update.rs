//! Host SpectreGunshipUpdate residual (insertion flight / doors / afterburner / departure).
//!
//! C++: `SpectreGunshipUpdate::initiateIntentToDoSpecialPower` + `update`
//! - `chooseLocomotorSet(LOCOMOTORSET_PANIC)` + invalid-pos + ultra-accurate
//! - `clearAndSetModelConditionState(DOOR_1_OPENING, DOOR_1_CLOSING)`
//! - `friend_enableAfterburners(TRUE)` + `GUNSHIP_STATUS_INSERTING`
//! - Each tick: move toward orbit-insertion satellite; when dist < orbitR
//!   → ORBITING, doors OPENING, afterburners off, NORMAL loco
//! - After OrbitTime → DEPARTING: panic + afterburners + doors CLOSING,
//!   fly facing * 99999, destroy when off-map
//!
//! Leftover `spectre_gunship_update.rs` already matches C++ but Wave 325
//! empty-gates on the live host (`OBJECT_REGISTRY` empty). This residual
//! is the live spawn-plan path.
//!
//! Fail-closed: not full gattling contain / howitzer projectile / decal pair.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::game_logic::host_spectre_gunship_deployment::{
    default_map_extents, SPECTRE_PREFERRED_ELEVATION,
};
use crate::game_logic::special_power_strikes::{
    SPECTRE_GUNSHIP_ORBIT_RADIUS, SPECTRE_ORBIT_DURATION_FRAMES, SPECTRE_ORBIT_INSERTION_SLOPE,
    SPECTRE_ORBIT_RADIUS, SPECTRE_TARGETING_RETICLE_RADIUS,
};

/// C++ `ORBIT_INSERTION_SLOPE_MAX`.
pub const ORBIT_INSERTION_SLOPE_MAX: f32 = 0.8;
/// C++ `ORBIT_INSERTION_SLOPE_MIN`.
pub const ORBIT_INSERTION_SLOPE_MIN: f32 = 0.5;
/// Residual insertion / departure step (panic afterburner flight).
pub const SPECTRE_INSERTION_SPEED: f32 = 22.0;
/// C++ `disengageAndDepartAO` facing scale.
pub const SPECTRE_DEPART_MAP_SIZE: f32 = 99_999.0;

/// C++ `GunshipStatus` (Inserting < Orbiting < Departing < Idle).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum HostGunshipStatus {
    Inserting = 0,
    Orbiting = 1,
    Departing = 2,
    #[default]
    Idle = 3,
}

impl HostGunshipStatus {
    pub fn overridable_destination_active(self) -> bool {
        (self as u8) < (HostGunshipStatus::Departing as u8)
    }
}

/// Per-gunship SpectreGunshipUpdate residual.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostSpectreGunshipUpdateData {
    pub status: HostGunshipStatus,
    pub initial_target: Vec3,
    pub override_target: Vec3,
    pub satellite_position: Vec3,
    pub exit_point: Vec3,
    pub orbit_escape_frame: u32,
    pub orbit_radius: f32,
    pub orbit_frames: u32,
    pub orbit_insertion_slope: f32,
    pub attack_area_radius: f32,
    pub targeting_reticle_radius: f32,
    pub preferred_elevation: f32,
    pub afterburners_on: bool,
    /// True → `DOOR_1_OPENING`; false → `DOOR_1_CLOSING`.
    pub door_opening: bool,
}

impl Default for HostSpectreGunshipUpdateData {
    fn default() -> Self {
        Self {
            status: HostGunshipStatus::Idle,
            initial_target: Vec3::ZERO,
            override_target: Vec3::ZERO,
            satellite_position: Vec3::ZERO,
            exit_point: Vec3::ZERO,
            orbit_escape_frame: 0,
            orbit_radius: SPECTRE_GUNSHIP_ORBIT_RADIUS,
            orbit_frames: SPECTRE_ORBIT_DURATION_FRAMES,
            orbit_insertion_slope: SPECTRE_ORBIT_INSERTION_SLOPE,
            attack_area_radius: SPECTRE_ORBIT_RADIUS,
            targeting_reticle_radius: SPECTRE_TARGETING_RETICLE_RADIUS,
            preferred_elevation: SPECTRE_PREFERRED_ELEVATION,
            afterburners_on: false,
            door_opening: false,
        }
    }
}

/// One-tick result applied by the host.
#[derive(Debug, Clone, Copy)]
pub struct SpectreGunshipTick {
    pub pos: Vec3,
    pub vel: Vec3,
    pub destroy: bool,
    pub afterburners_on: bool,
    pub door_opening: bool,
    pub panic_loco: bool,
}

impl HostSpectreGunshipUpdateData {
    pub fn for_template(template_name: &str) -> Option<Self> {
        let n = template_name.to_ascii_lowercase();
        if n.contains("spectregunship") || n.contains("spectre_gunship") {
            Some(Self::default())
        } else {
            None
        }
    }

    /// C++ `initiateIntentToDoSpecialPower` insertion latch.
    pub fn initiate(&mut self, target: Vec3) {
        self.initial_target = target;
        self.override_target = target;
        self.satellite_position = target;
        self.exit_point = Vec3::ZERO;
        self.orbit_escape_frame = 0;
        self.status = HostGunshipStatus::Inserting;
        self.afterburners_on = true;
        self.door_opening = false;
    }

    pub fn initiate_at(target: Vec3) -> Self {
        let mut data = Self::default();
        data.initiate(target);
        data
    }

    /// C++ orbit-insertion satellite (XZ ground plane; leftover uses XY).
    pub fn compute_satellite(&self, pos: Vec3) -> Vec3 {
        let mut px = pos.x - self.initial_target.x;
        let mut pz = pos.z - self.initial_target.z;
        let dist = (px * px + pz * pz).sqrt();
        if dist > 1.0e-4 {
            px /= dist;
            pz /= dist;
        } else {
            px = 1.0;
            pz = 0.0;
        }
        let apx = -pz;
        let apz = px;
        let slope = self
            .orbit_insertion_slope
            .clamp(ORBIT_INSERTION_SLOPE_MIN, ORBIT_INSERTION_SLOPE_MAX);
        let n2 = 1.0 - slope;
        let dx = px * slope + apx * n2;
        let dz = pz * slope + apz * n2;
        Vec3::new(
            self.initial_target.x + dx * self.orbit_radius,
            self.preferred_elevation,
            self.initial_target.z + dz * self.orbit_radius,
        )
    }

    pub fn distance_to_target(&self, pos: Vec3) -> f32 {
        let dx = pos.x - self.initial_target.x;
        let dz = pos.z - self.initial_target.z;
        (dx * dx + dz * dz).sqrt()
    }

    pub fn is_point_off_map(pos: Vec3, min_x: f32, min_z: f32, max_x: f32, max_z: f32) -> bool {
        pos.x < min_x || pos.x > max_x || pos.z < min_z || pos.z > max_z
    }

    fn step_toward(pos: Vec3, dest: Vec3, elevation: f32) -> (Vec3, Vec3) {
        let dx = dest.x - pos.x;
        let dz = dest.z - pos.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let mut new_pos = pos;
        new_pos.y = elevation;
        if dist < 1.0 {
            return (new_pos, Vec3::ZERO);
        }
        let step = SPECTRE_INSERTION_SPEED.min(dist);
        new_pos.x += dx / dist * step;
        new_pos.z += dz / dist * step;
        (new_pos, new_pos - pos)
    }

    fn facing_exit(pos: Vec3, facing: f32, elevation: f32) -> Vec3 {
        Vec3::new(
            pos.x + facing.cos() * SPECTRE_DEPART_MAP_SIZE,
            elevation,
            pos.z + facing.sin() * SPECTRE_DEPART_MAP_SIZE,
        )
    }

    fn constrain_override(&mut self) {
        let constraint = (self.attack_area_radius - self.targeting_reticle_radius).max(0.0);
        let mut dx = self.initial_target.x - self.override_target.x;
        let mut dz = self.initial_target.z - self.override_target.z;
        let dist = (dx * dx + dz * dz).sqrt();
        if dist > constraint && dist > 1.0e-4 {
            dx /= dist;
            dz /= dist;
            self.override_target.x = self.initial_target.x - dx * constraint;
            self.override_target.z = self.initial_target.z - dz * constraint;
        }
    }

    /// C++ `SpectreGunshipUpdate::update` insertion / orbit / depart slice.
    pub fn tick(&mut self, pos: Vec3, facing: f32, frame: u32) -> SpectreGunshipTick {
        let (min_x, min_z, max_x, max_z) = default_map_extents();
        match self.status {
            HostGunshipStatus::Idle => SpectreGunshipTick {
                pos,
                vel: Vec3::ZERO,
                destroy: false,
                afterburners_on: self.afterburners_on,
                door_opening: self.door_opening,
                panic_loco: false,
            },
            HostGunshipStatus::Inserting | HostGunshipStatus::Orbiting => {
                self.satellite_position = self.compute_satellite(pos);
                self.constrain_override();
                let (new_pos, vel) =
                    Self::step_toward(pos, self.satellite_position, self.preferred_elevation);
                let dist = self.distance_to_target(new_pos);
                if self.status == HostGunshipStatus::Inserting && dist < self.orbit_radius {
                    self.status = HostGunshipStatus::Orbiting;
                    self.orbit_escape_frame = frame.saturating_add(self.orbit_frames);
                    self.afterburners_on = false;
                    self.door_opening = true;
                }
                if self.status == HostGunshipStatus::Orbiting && frame >= self.orbit_escape_frame {
                    self.status = HostGunshipStatus::Departing;
                    self.exit_point = Self::facing_exit(new_pos, facing, self.preferred_elevation);
                    self.afterburners_on = true;
                    self.door_opening = false;
                }
                SpectreGunshipTick {
                    pos: new_pos,
                    vel,
                    destroy: false,
                    afterburners_on: self.afterburners_on,
                    door_opening: self.door_opening,
                    panic_loco: self.status != HostGunshipStatus::Orbiting,
                }
            }
            HostGunshipStatus::Departing => {
                if self.exit_point.length_squared() < 1.0 {
                    self.exit_point = Self::facing_exit(pos, facing, self.preferred_elevation);
                }
                let (new_pos, vel) =
                    Self::step_toward(pos, self.exit_point, self.preferred_elevation);
                let off = Self::is_point_off_map(new_pos, min_x, min_z, max_x, max_z);
                if off {
                    self.status = HostGunshipStatus::Idle;
                    self.afterburners_on = false;
                }
                SpectreGunshipTick {
                    pos: new_pos,
                    vel,
                    destroy: off,
                    afterburners_on: self.afterburners_on,
                    door_opening: self.door_opening,
                    panic_loco: true,
                }
            }
        }
    }
}

pub fn honesty_spectre_gunship_update_residual_ok() -> bool {
    ORBIT_INSERTION_SLOPE_MAX == 0.8
        && ORBIT_INSERTION_SLOPE_MIN == 0.5
        && (SPECTRE_ORBIT_INSERTION_SLOPE - 0.7).abs() < 0.01
        && (SPECTRE_GUNSHIP_ORBIT_RADIUS - 250.0).abs() < 0.01
        && SPECTRE_ORBIT_DURATION_FRAMES == 450
        && {
            let mut d = HostSpectreGunshipUpdateData::initiate_at(Vec3::new(250.0, 0.0, 250.0));
            d.status == HostGunshipStatus::Inserting
                && d.afterburners_on
                && !d.door_opening
                && {
                    let start = Vec3::new(-250.0, 120.0, -250.0);
                    let tick = d.tick(start, 0.785_398_2, 0);
                    let moved = (tick.pos.x - start.x).abs() > 1.0
                        || (tick.pos.z - start.z).abs() > 1.0;
                    moved && tick.afterburners_on && !tick.door_opening && !tick.destroy
                }
        }
}

/// C++ `clearAndSetModelConditionState` door pair + `friend_enableAfterburners`.
pub fn apply_spectre_door_and_afterburner(
    obj: &mut crate::game_logic::object::Object,
    door_opening: bool,
    afterburners: bool,
) {
    let opening = crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
        "DOOR_1_OPENING",
    );
    let closing = crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
        "DOOR_1_CLOSING",
    );
    if let Some(b) = opening {
        if door_opening {
            obj.model_condition_bits |= 1u128 << b;
        } else {
            obj.model_condition_bits &= !(1u128 << b);
        }
    }
    if let Some(b) = closing {
        if door_opening {
            obj.model_condition_bits &= !(1u128 << b);
        } else {
            obj.model_condition_bits |= 1u128 << b;
        }
    }
    let _ = obj.enable_jet_afterburners(afterburners);
}
