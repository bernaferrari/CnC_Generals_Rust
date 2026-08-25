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
//! Live AttackAreaDecal + TargetingReticleDecal: `HostRadiusDecal` enqueue
//! (C++ `createRadiusDecal` / `setPosition` / `update` / `cleanUp`).

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::game_logic::ObjectId;
use crate::game_logic::host_radius_decal_update::{
    HostRadiusDecal, HostRadiusDecalTemplate, radius_decal_ms_to_frames,
};
use crate::game_logic::host_spectre_gunship_deployment::{
    SPECTRE_PREFERRED_ELEVATION, default_map_extents,
};
use crate::game_logic::special_power_strikes::{
    HostSpectreOrbitField, SPECTRE_ATTACK_AREA_DECAL_TEXTURE, SPECTRE_ATTACK_AREA_DECAL_THROB_MS,
    SPECTRE_DECAL_COLOR, SPECTRE_GATTLING_STRAFE_FX, SPECTRE_GUNSHIP_ORBIT_RADIUS,
    SPECTRE_ORBIT_DURATION_FRAMES, SPECTRE_ORBIT_INSERTION_SLOPE, SPECTRE_ORBIT_RADIUS,
    SPECTRE_TARGETING_RETICLE_DECAL_TEXTURE, SPECTRE_TARGETING_RETICLE_DECAL_THROB_MS,
    SPECTRE_TARGETING_RETICLE_RADIUS, clamp_spectre_override_destination,
};

/// C++ `ORBIT_INSERTION_SLOPE_MAX`.
pub const ORBIT_INSERTION_SLOPE_MAX: f32 = 0.8;
/// C++ `ORBIT_INSERTION_SLOPE_MIN`.
pub const ORBIT_INSERTION_SLOPE_MIN: f32 = 0.5;
/// Residual insertion / departure step (panic afterburner flight).
pub const SPECTRE_INSERTION_SPEED: f32 = 22.0;
/// C++ `disengageAndDepartAO` facing scale.
pub const SPECTRE_DEPART_MAP_SIZE: f32 = 99_999.0;
/// C++ `GameClientRandomValueReal(-5, 5)` jitter on the gattling impact XY.
pub const SPECTRE_GATTLING_STRAFE_SMOKE_JITTER: f32 = 5.0;

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
    /// C++ `m_attackAreaDecal` — owner-only AttackArea ring (SCCSpecTarg).
    #[serde(default)]
    pub attack_area_decal: HostRadiusDecal,
    /// C++ `m_targetingReticleDecal` — owner-only reticle (SCCSpecRet).
    #[serde(default)]
    pub targeting_reticle_decal: HostRadiusDecal,
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
            attack_area_decal: HostRadiusDecal::default(),
            targeting_reticle_decal: HostRadiusDecal::default(),
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

fn spectre_decal_argb() -> u32 {
    let (r, g, b, a) = SPECTRE_DECAL_COLOR;
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

fn spectre_attack_area_decal_template() -> HostRadiusDecalTemplate {
    HostRadiusDecalTemplate {
        name: "SpectreAttackArea".into(),
        texture: SPECTRE_ATTACK_AREA_DECAL_TEXTURE.into(),
        opacity_min: 0.25,
        opacity_max: 0.50,
        throb_frames: radius_decal_ms_to_frames(SPECTRE_ATTACK_AREA_DECAL_THROB_MS),
        only_visible_to_owner: true,
        color: spectre_decal_argb(),
    }
}

fn spectre_targeting_reticle_decal_template() -> HostRadiusDecalTemplate {
    HostRadiusDecalTemplate {
        name: "SpectreTargetingReticle".into(),
        texture: SPECTRE_TARGETING_RETICLE_DECAL_TEXTURE.into(),
        opacity_min: 0.50,
        opacity_max: 1.00,
        throb_frames: radius_decal_ms_to_frames(SPECTRE_TARGETING_RETICLE_DECAL_THROB_MS),
        only_visible_to_owner: true,
        color: spectre_decal_argb(),
    }
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
        self.create_engagement_decals(target, 0);
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

    /// C++ update clamp: AttackAreaRadius - TargetingReticleRadius from initial.
    pub fn constrain_override(&mut self) {
        self.override_target = clamp_spectre_override_destination(
            self.initial_target,
            self.override_target,
            self.attack_area_radius,
            self.targeting_reticle_radius,
        );
    }

    fn create_engagement_decals(&mut self, pos: Vec3, frame: u32) {
        self.attack_area_decal = HostRadiusDecal::create_with_owner(
            spectre_attack_area_decal_template(),
            self.attack_area_radius,
            pos,
            frame,
            None,
        );
        self.targeting_reticle_decal = HostRadiusDecal::create_with_owner(
            spectre_targeting_reticle_decal_template(),
            self.targeting_reticle_radius,
            pos,
            frame,
            None,
        );
    }

    fn ensure_engagement_decals(&mut self, frame: u32) {
        if self.attack_area_decal.is_empty() || self.targeting_reticle_decal.is_empty() {
            self.create_engagement_decals(self.initial_target, frame);
        }
    }

    fn update_engagement_decals(&mut self, frame: u32) {
        self.ensure_engagement_decals(frame);
        self.attack_area_decal.update(frame);
        self.targeting_reticle_decal.update(frame);
    }

    fn place_engagement_decals(&mut self) {
        self.attack_area_decal.set_position(self.initial_target);
        self.targeting_reticle_decal
            .set_position(self.override_target);
    }

    /// C++ `SpectreGunshipUpdate::cleanUp` — clear AttackArea + Reticle.
    pub fn clean_up_decals(&mut self) {
        self.attack_area_decal.clear();
        self.targeting_reticle_decal.clear();
    }

    /// C++ `SpectreGunshipUpdate::update` insertion / orbit / depart slice.
    pub fn tick(&mut self, pos: Vec3, facing: f32, frame: u32) -> SpectreGunshipTick {
        let (min_x, min_z, max_x, max_z) = default_map_extents();
        match self.status {
            HostGunshipStatus::Idle => {
                self.clean_up_decals();
                SpectreGunshipTick {
                    pos,
                    vel: Vec3::ZERO,
                    destroy: false,
                    afterburners_on: self.afterburners_on,
                    door_opening: self.door_opening,
                    panic_loco: false,
                }
            }
            HostGunshipStatus::Inserting | HostGunshipStatus::Orbiting => {
                self.satellite_position = self.compute_satellite(pos);
                self.update_engagement_decals(frame);
                self.constrain_override();
                self.place_engagement_decals();
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
                self.attack_area_decal.update(frame);
                self.targeting_reticle_decal.update(frame);
                if self.exit_point.length_squared() < 1.0 {
                    self.exit_point = Self::facing_exit(pos, facing, self.preferred_elevation);
                }
                let (new_pos, vel) =
                    Self::step_toward(pos, self.exit_point, self.preferred_elevation);
                let off = Self::is_point_off_map(new_pos, min_x, min_z, max_x, max_z);
                if off {
                    self.status = HostGunshipStatus::Idle;
                    self.afterburners_on = false;
                    self.clean_up_decals();
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
        && SPECTRE_GATTLING_STRAFE_FX == "SpectreGattlingArmsSmoke"
        && (SPECTRE_GATTLING_STRAFE_SMOKE_JITTER - 5.0).abs() < 0.01
        && {
            let mut d = HostSpectreGunshipUpdateData::initiate_at(Vec3::new(250.0, 0.0, 250.0));
            let created = !d.attack_area_decal.is_empty()
                && !d.targeting_reticle_decal.is_empty()
                && (d.attack_area_decal.radius - SPECTRE_ORBIT_RADIUS).abs() < 0.01
                && (d.targeting_reticle_decal.radius - SPECTRE_TARGETING_RETICLE_RADIUS).abs()
                    < 0.01;
            #[cfg(feature = "game_client")]
            let created = created
                && d.attack_area_decal.has_projected_shadow()
                && d.targeting_reticle_decal.has_projected_shadow();
            d.status == HostGunshipStatus::Inserting
                && d.afterburners_on
                && !d.door_opening
                && created
                && {
                    let start = Vec3::new(-250.0, 120.0, -250.0);
                    let tick = d.tick(start, 0.785_398_2, 0);
                    let moved =
                        (tick.pos.x - start.x).abs() > 1.0 || (tick.pos.z - start.z).abs() > 1.0;
                    let placed = (d.attack_area_decal.position - d.initial_target).length() < 0.01
                        && (d.targeting_reticle_decal.position - d.override_target).length() < 0.01;
                    moved && tick.afterburners_on && !tick.door_opening && !tick.destroy && placed
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

/// C++ `SpectreGunshipUpdate::update`: `isEffectivelyDead` → `UPDATE_SLEEP_FOREVER`
/// (cease fire). Missing gunship → `cleanUp` ("OH MY GOODNESS... SHOT DOWN").
/// Live `HostSpectreOrbitField` is the residual bombardment — expire it now.
pub fn expire_orbit_fields_on_gunship_dead(
    fields: &mut [HostSpectreOrbitField],
    current_frame: u32,
    gunship_dead_for_source: impl Fn(ObjectId) -> bool,
) {
    for field in fields {
        if field.is_expired(current_frame) {
            continue;
        }
        if gunship_dead_for_source(field.source_object) {
            field.expires_frame = current_frame;
        }
    }
}

/// C++ `getShroudedStatus(local) <= OBJECTSHROUD_PARTIAL_CLEAR`.
/// No local player / no PartitionData → visible (leftover `unwrap_or(true)`).
pub fn spectre_gunship_visible_for_strafe_fx(
    shroud: Option<gamelogic::common::types::ObjectShroudStatus>,
) -> bool {
    match shroud {
        Some(status) => {
            (status as u8) <= (gamelogic::common::types::ObjectShroudStatus::PartialClear as u8)
        }
        None => true,
    }
}

/// C++ gattling `OBJECT_STATUS_IS_FIRING_WEAPON` residual on the live orbit field.
pub fn spectre_orbit_gattling_is_firing(field: &HostSpectreOrbitField) -> bool {
    field.gattling_consecutive > 0
}

/// Host Y-up impact of leftover `m_gattlingTargetPosition ± 5` + `getGroundHeight`.
///
/// C++ XY is the ground plane; host maps that to XZ and writes terrain height to Y.
pub fn spectre_gattling_strafe_smoke_impact(gattling_target: Vec3) -> Vec3 {
    let x = gattling_target.x
        + gamelogic::helpers::game_client_random_value_real(
            -SPECTRE_GATTLING_STRAFE_SMOKE_JITTER,
            SPECTRE_GATTLING_STRAFE_SMOKE_JITTER,
        );
    let z = gattling_target.z
        + gamelogic::helpers::game_client_random_value_real(
            -SPECTRE_GATTLING_STRAFE_SMOKE_JITTER,
            SPECTRE_GATTLING_STRAFE_SMOKE_JITTER,
        );
    let y = gamelogic::helpers::TheTerrainLogic::get()
        .map(|terrain| terrain.get_ground_height(x, z, None))
        .unwrap_or(gattling_target.y);
    Vec3::new(x, y, z)
}

/// Leftover `TheParticleSystemManager->createParticleSystem(SpectreGattlingArmsSmoke)`
/// + `setPosition` at the C++ Z-up impact (host Y-up `(x, height, z)`).
pub fn spawn_spectre_gattling_strafe_smoke(impact: Vec3) -> Option<u32> {
    if SPECTRE_GATTLING_STRAFE_FX.is_empty() {
        return None;
    }
    let manager = gamelogic::helpers::TheParticleSystemManager::get()?;
    let id = manager.create_particle_system(Some(SPECTRE_GATTLING_STRAFE_FX))?;
    let leftover = gamelogic::common::Coord3D::new(impact.x, impact.z, impact.y);
    manager.set_particle_system_position(id, &leftover);
    Some(id)
}
