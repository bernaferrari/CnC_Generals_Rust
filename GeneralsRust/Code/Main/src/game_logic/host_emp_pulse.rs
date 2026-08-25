//! Host China EMP Pulse special-power residual (DISABLED_EMP disable field).
//!
//! Residual slice (playability):
//! - `DoSpecialPower(EmpPulse)` at a world location temporarily disables
//!   vehicles and faction structures in radius (retail SuperweaponEMPPulse →
//!   EMPPulseBomb → EMPPulseEffectSpheroid EMPUpdate path).
//! - C++ EMPUpdate::doDisableAttack: setDisabledUntil(DISABLED_EMP, now +
//!   DisabledDuration) for VEHICLE / faction STRUCTURE / SPAWNS_ARE_THE_WEAPONS;
//!   airborne aircraft (non EMP_HARDENED) are killed residual.
//! - Honesty counters/flags for residual gates and tests.
//!
//! Wave 51 residual pack (retail INI honesty):
//! - SuperweaponEMPPulse RadiusCursorRadius **200**, ReloadTime **360000**ms → **10800**f
//! - SUPERWEAPON_EMPPulse OCL: ChinaJetCargoPlane + EMPPulseBomb,
//!   DropVariance **X:20 Y:20 Z:0**, DeliveryDistance **150**, DeliveryDecalRadius **200**
//! - EMPPulseEffectSpheroid EMPUpdate: DisabledDuration **30000**ms → **900**f,
//!   Lifetime **3000**ms → **90**f, StartFadeTime **300**ms → **9**f,
//!   StartScale **0.01**, TargetScaleMin/Max **3.0**/**4.0**,
//!   StartColor **R32 G64 B255**, EndColor **R0 G0 B0**, EMPSparks FX
//! - EMP_HARDENED residual name markers (cargo plane / bomber / A10 / Spectre path)
//!
//! Fail-closed honesty:
//! - EMPPulseEffectSpheroid object spawn residual closed (GPU scale/tint fail-closed)
//! - Cargo plane + EMPPulseBomb flight residual closed; not full projectile physics
//! - Not full EMPSparks particle volume / GPU tint path
//! - Not full subdual / reject-mask ally matrix beyond residual kindof filters
//! - Not multiplayer shared-synced timer / academy / shortcut UI parity
//! - Not network EMP replication (network deferred)

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const EMP_PULSE_LOGIC_FPS: f32 = 30.0;

/// Retail SuperweaponEMPPulse RadiusCursorRadius residual (= 200).
/// Also matches EMPUpdateModuleData default EffectRadius when INI omits it
/// (EMPPulseEffectSpheroid does not set EffectRadius).
pub const HOST_EMP_PULSE_RADIUS: f32 = 200.0;

/// Retail SuperweaponEMPPulse RadiusCursorRadius residual (alias).
pub const SUPERWEAPON_EMP_PULSE_RADIUS_CURSOR: f32 = 200.0;

/// Retail SuperweaponEMPPulse ReloadTime residual (msec).
pub const SUPERWEAPON_EMP_PULSE_RELOAD_MS: u32 = 360_000;
/// ReloadTime 360000ms → 10800 frames @ 30 FPS.
pub const SUPERWEAPON_EMP_PULSE_RELOAD_FRAMES: u32 = 10_800;

/// Retail EMPPulseEffectSpheroid DisabledDuration = 30000 ms.
pub const EMP_PULSE_DISABLED_DURATION_MS: u32 = 30_000;

/// Logic-frame residual of DisabledDuration (ms * 30 / 1000) = 900 frames.
pub const EMP_PULSE_DISABLED_DURATION_FRAMES: u32 = (EMP_PULSE_DISABLED_DURATION_MS * 30) / 1000;

/// Activate / impact audio residual (SoundEffects.ini EMPPulseWhoosh / FXList).
pub const EMP_PULSE_ACTIVATE_AUDIO: &str = "EMPPulseWhoosh";

// --- SUPERWEAPON_EMPPulse OCL residual ---

/// Retail SUPERWEAPON_EMPPulse Transport residual.
pub const EMP_PULSE_OCL_TRANSPORT: &str = "ChinaJetCargoPlane";
/// Retail payload bomb residual.
pub const EMP_PULSE_BOMB_TEMPLATE: &str = "EMPPulseBomb";
/// Retail OCL effect-spheroid create list residual.
pub const EMP_PULSE_OCL_EFFECT_SPHEROIDS: &str = "OCL_EMPPulseEffectSpheroids";
/// Retail EMPPulseEffectSpheroid object residual.
pub const EMP_PULSE_EFFECT_SPHEROID: &str = "EMPPulseEffectSpheroid";
/// Retail Superweapon / OCL names.
pub const SUPERWEAPON_EMP_PULSE_NAME: &str = "SuperweaponEMPPulse";
pub const SUPERWEAPON_EMP_PULSE_OCL: &str = "SUPERWEAPON_EMPPulse";

/// Retail SUPERWEAPON_EMPPulse DropVariance residual (X/Y/Z).
pub const EMP_PULSE_DROP_VARIANCE: (f32, f32, f32) = (20.0, 20.0, 0.0);
/// Retail SUPERWEAPON_EMPPulse DeliveryDistance residual.
pub const EMP_PULSE_DELIVERY_DISTANCE: f32 = 150.0;
/// Retail SUPERWEAPON_EMPPulse DeliveryDecalRadius residual.
pub const EMP_PULSE_DELIVERY_DECAL_RADIUS: f32 = 200.0;

// --- EMPPulseEffectSpheroid EMPUpdate residual ---

/// Retail EMPUpdate Lifetime residual (msec).
pub const EMP_SPHEROID_LIFETIME_MS: u32 = 3_000;
/// Lifetime 3000ms → 90 frames @ 30 FPS.
pub const EMP_SPHEROID_LIFETIME_FRAMES: u32 = 90;
/// Retail EMPUpdate StartFadeTime residual (msec).
pub const EMP_SPHEROID_START_FADE_MS: u32 = 300;
/// StartFadeTime 300ms → 9 frames @ 30 FPS.
pub const EMP_SPHEROID_START_FADE_FRAMES: u32 = 9;
/// Retail EMPUpdate StartScale residual.
pub const EMP_SPHEROID_START_SCALE: f32 = 0.01;
/// Retail EMPUpdate TargetScaleMin residual.
pub const EMP_SPHEROID_TARGET_SCALE_MIN: f32 = 3.0;
/// Retail EMPUpdate TargetScaleMax residual.
pub const EMP_SPHEROID_TARGET_SCALE_MAX: f32 = 4.0;
/// Retail EMPUpdate StartColor residual (RGB).
pub const EMP_SPHEROID_START_COLOR: (u8, u8, u8) = (32, 64, 255);
/// Retail EMPUpdate EndColor residual (RGB).
pub const EMP_SPHEROID_END_COLOR: (u8, u8, u8) = (0, 0, 0);
/// Retail DisableFXParticleSystem residual.
pub const EMP_SPHEROID_DISABLE_FX: &str = "EMPSparks";
/// Retail EMPPulseEffectSpheroid GeometryMajorRadius residual.
pub const EMP_SPHEROID_GEOMETRY_RADIUS: f32 = 30.0;
/// Retail DoesNotAffectMyOwnBuildings residual.
pub const EMP_SPHEROID_DOES_NOT_AFFECT_OWN_BUILDINGS: bool = false;

/// Retail KindOf EMP_HARDENED residual name markers (cargo / bomber / A10 / Spectre).
/// Fail-closed vs full KindOf mask matrix — name residual only.
pub const EMP_HARDENED_NAME_MARKERS: &[&str] = &[
    "emphardened",
    "emp_hardened",
    "empresistant",
    "cargoplane",
    "jetb52",
    "jetb3",
    "a10thunderbolt",
    "spectregunship",
    "carpetbomber",
    "mignapalmstriker",
    "chinaartillerycannon",
    "supw_americapatriotbattery",
];

/// Whether residual target is a legal EMP disable victim.
///
/// Retail EMPUpdate::doDisableAttack:
/// - VEHICLE, STRUCTURE (faction only), SPAWNS_ARE_THE_WEAPONS
/// - Not infantry (unless SPAWNS_ARE_THE_WEAPONS)
/// - KINDOF_EMP_HARDENED is consulted only on the airborne-aircraft branch
///   (`should_emp_kill_airborne` / `should_emp_skip_hardened_airborne`).
///   Ground hardened vehicles/structures
/// - C++ has no under-construction skip.
pub fn is_legal_emp_disable_target(
    is_vehicle: bool,
    is_faction_structure: bool,
    is_spawns_are_weapons: bool,
    is_alive: bool,
    _under_construction: bool,
    _is_emp_hardened: bool,
) -> bool {
    if !is_alive {
        return false;
    }
    is_vehicle || is_faction_structure || is_spawns_are_weapons
}

/// True when residual EMP should kill instead of disable (airborne aircraft).
///
/// C++ EMPUpdate.cpp:231-249: KINDOF_AIRCRAFT && isAirborneTarget &&
/// !KINDOF_EMP_HARDENED → kill. Hardened airborne `continue` (no disable).
pub fn should_emp_kill_airborne(
    is_aircraft: bool,
    is_airborne: bool,
    is_emp_hardened: bool,
) -> bool {
    is_aircraft && is_airborne && !is_emp_hardened
}

/// C++ EMPUpdate.cpp:240-241 — airborne EMP_HARDENED aircraft `continue`
/// (neither kill nor `setDisabledUntil(DISABLED_EMP)`).
pub fn should_emp_skip_hardened_airborne(
    is_aircraft: bool,
    is_airborne: bool,
    is_emp_hardened: bool,
) -> bool {
    is_aircraft && is_airborne && is_emp_hardened
}

/// C++ EMPUpdate.cpp Patch 1.01 (`onlyEffectAirborne`): when the producer's
/// current AI victim is airborne, skip every non-airborne victim.
pub fn emp_skip_ground_when_airborne_only(
    intended_victim_is_airborne: bool,
    victim_is_airborne: bool,
) -> bool {
    intended_victim_is_airborne && !victim_is_airborne
}

/// C++ EMPUpdate.cpp:321-339 / leftover `do_disable_attack` Patch 1.01.
/// When the intended victim was not processed in the radius scan, still
/// disable an un-hardened aircraft whose 3D `dist_sqr` is `<= radius * 2`
/// or `<= 40*40` (near-miss).
pub fn emp_intended_victim_near_miss_disables(
    intended_processed: bool,
    is_aircraft: bool,
    is_emp_hardened: bool,
    dist_sqr: f32,
    radius: f32,
) -> bool {
    if intended_processed || !is_aircraft || is_emp_hardened {
        return false;
    }
    dist_sqr <= radius * 2.0 || dist_sqr <= 40.0 * 40.0
}

/// 2D distance check residual (ground plane x/z; host gameplay convention).
/// Kept for honesty / older call sites; playable EMP uses the 3D helper.
pub fn in_emp_pulse_radius_2d(center: (f32, f32), target: (f32, f32), radius: f32) -> bool {
    let dx = center.0 - target.0;
    let dz = center.1 - target.1;
    dx * dx + dz * dz <= radius * radius
}

/// C++ PartitionManager FROM_BOUNDINGSPHERE_3D: 3D center-to-center minus
/// the victim bounding-sphere radius (clamped at 0).
pub fn in_emp_pulse_radius_from_bounding_sphere_3d(
    center: Vec3,
    target: Vec3,
    target_sphere_radius: f32,
    radius: f32,
) -> bool {
    let dx = target.x - center.x;
    let dy = target.y - center.y;
    let dz = target.z - center.z;
    let center_dist = (dx * dx + dy * dy + dz * dz).sqrt();
    let sphere = target_sphere_radius.max(0.0);
    let boundary = if center_dist <= sphere {
        0.0
    } else {
        center_dist - sphere
    };
    boundary <= radius
}

/// Leftover GeometryInfo bounding-sphere residual (AABB half-extents + radius).
pub fn leftover_emp_bounding_sphere_radius(
    radius: f32,
    bounds_min: Vec3,
    bounds_max: Vec3,
    selection_radius: f32,
) -> f32 {
    let hx = (bounds_max.x - bounds_min.x).abs() * 0.5;
    let hy = (bounds_max.y - bounds_min.y).abs() * 0.5;
    let hz = (bounds_max.z - bounds_min.z).abs() * 0.5;
    let from_bounds = (hx * hx + hy * hy + hz * hz).sqrt();
    radius.max(selection_radius).max(from_bounds)
}

/// C++ EMPUpdate.cpp saturateRGB — 0-1 operate, pack back to 8-bit.
pub fn saturate_emp_rgb(color: (u8, u8, u8), factor: f32) -> (u8, u8, u8) {
    let half = factor * 0.5;
    let sat = |channel: u8| -> u8 {
        let v = (channel as f32 / 255.0) * factor - half;
        (v.max(0.0) * 255.0).round().clamp(0.0, 255.0) as u8
    };
    (sat(color.0), sat(color.1), sat(color.2))
}

/// C++ `m_currentScale += (m_targetScale - m_currentScale) * 0.05f`.
pub fn tick_leftover_emp_spheroid_scale(current: f32, target: f32) -> f32 {
    current + (target - current) * 0.05
}

/// Deterministic TargetScaleMin/Max pick (leftover GameLogicRandomValueReal).
pub fn leftover_emp_target_scale(seed: u32) -> f32 {
    let t = (seed.wrapping_mul(2_654_435_761) >> 16) as f32 / 65_535.0;
    EMP_SPHEROID_TARGET_SCALE_MIN
        + t * (EMP_SPHEROID_TARGET_SCALE_MAX - EMP_SPHEROID_TARGET_SCALE_MIN)
}

/// StartColor×2 tint before fade; EndColor×5 flash on/after play frame.
pub fn leftover_emp_spheroid_tint(now: u32, tint_play_frame: u32) -> (u8, u8, u8) {
    if now < tint_play_frame {
        saturate_emp_rgb(EMP_SPHEROID_START_COLOR, 2.0)
    } else {
        saturate_emp_rgb(EMP_SPHEROID_END_COLOR, 5.0)
    }
}

/// C++ `now == m_tintEnvPlayFrame` is the exact disable frame.
pub fn leftover_emp_should_disable(now: u32, tint_play_frame: u32, already: bool) -> bool {
    !already && now == tint_play_frame
}

/// C++ EMPUpdate.cpp default SparksPerCubicFoot.
pub const EMP_SPARKS_PER_CUBIC_FOOT: f32 = 0.001;
/// C++ `MAX(15, ceil(sparksPerCubicFoot * volume))`.
pub const EMP_SPARKS_MIN_EMITTERS: u32 = 15;
/// C++ `setSystemLifetime(MAX(0, DisabledDuration - 30))`.
pub const EMP_SPARKS_LIFETIME_SLACK_FRAMES: u32 = 30;

/// C++ EMPUpdate.cpp:280-284 spark emitter count.
pub fn leftover_emp_spark_emitter_count(footprint_area: f32, height: f32) -> u32 {
    let volume = footprint_area.max(0.0) * height.max(0.0).min(10.0);
    let computed = (EMP_SPARKS_PER_CUBIC_FOOT * volume).ceil() as i32;
    computed.max(EMP_SPARKS_MIN_EMITTERS as i32) as u32
}

/// C++ `setSystemLifetime(MAX(0, DisabledDuration - 30))`.
pub fn leftover_emp_spark_lifetime(disabled_duration_frames: u32) -> u32 {
    disabled_duration_frames.saturating_sub(EMP_SPARKS_LIFETIME_SLACK_FRAMES)
}

/// C++ `GameLogicRandomValue(3, victimHeight)` (host Y-up height).
pub fn leftover_emp_spark_z(victim_height: f32) -> f32 {
    let hi = victim_height.max(3.0) as i32;
    gamelogic::helpers::get_game_logic_random_value(3, hi).max(3) as f32
}

/// C++ `GameLogicRandomValue(1, 100)` spark `setInitialDelay`.
pub fn leftover_emp_spark_initial_delay() -> u32 {
    gamelogic::helpers::get_game_logic_random_value(1, 100).max(1) as u32
}

/// C++ EMPUpdate.cpp:297-307 quadrahemicycloid clamp. Host Y-up: y is height.
pub fn leftover_emp_spark_dome_clamp(offset: glam::Vec3, victim_height: f32) -> glam::Vec3 {
    let len = offset.length();
    if len > victim_height && len > 0.0 {
        let n = offset / len;
        glam::Vec3::new(offset.x, n.y * victim_height, offset.z)
    } else {
        offset
    }
}

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn emp_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / EMP_PULSE_LOGIC_FPS)).round() as u32
}

/// Name-based EMP_HARDENED residual (fail-closed vs full KindOf mask matrix).
///
/// Wave 51 expands retail markers for cargo plane / bomber / A10 / Spectre /
/// carpet bomber / napalm MIG / SUPW Patriot paths.
pub fn is_emp_hardened_name(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    EMP_HARDENED_NAME_MARKERS.iter().any(|m| n.contains(m))
}

/// Apply SUPERWEAPON_EMPPulse DropVariance residual to a delivery center.
pub fn apply_emp_pulse_drop_variance(center: Vec3, unit_x: f32, unit_y: f32) -> Vec3 {
    let (vx, vy, _vz) = EMP_PULSE_DROP_VARIANCE;
    let ux = unit_x.clamp(0.0, 1.0);
    let uy = unit_y.clamp(0.0, 1.0);
    Vec3::new(
        center.x + (ux * 2.0 - 1.0) * vx,
        center.y,
        center.z + (uy * 2.0 - 1.0) * vy,
    )
}

/// Wave 51 residual honesty: DisabledDuration / radius / reload residual.
pub fn honesty_emp_pulse_duration_radius_residual_ok() -> bool {
    (HOST_EMP_PULSE_RADIUS - 200.0).abs() < 0.01
        && (SUPERWEAPON_EMP_PULSE_RADIUS_CURSOR - 200.0).abs() < 0.01
        && EMP_PULSE_DISABLED_DURATION_MS == 30_000
        && EMP_PULSE_DISABLED_DURATION_FRAMES == emp_ms_to_frames(EMP_PULSE_DISABLED_DURATION_MS)
        && EMP_PULSE_DISABLED_DURATION_FRAMES == 900
        && SUPERWEAPON_EMP_PULSE_RELOAD_MS == 360_000
        && SUPERWEAPON_EMP_PULSE_RELOAD_FRAMES == emp_ms_to_frames(SUPERWEAPON_EMP_PULSE_RELOAD_MS)
}

/// Wave 51 residual honesty: EffectSpheroid scale / tint / lifetime residual.
pub fn honesty_emp_spheroid_scale_tint_residual_ok() -> bool {
    EMP_SPHEROID_LIFETIME_MS == 3_000
        && EMP_SPHEROID_LIFETIME_FRAMES == emp_ms_to_frames(EMP_SPHEROID_LIFETIME_MS)
        && EMP_SPHEROID_START_FADE_MS == 300
        && EMP_SPHEROID_START_FADE_FRAMES == emp_ms_to_frames(EMP_SPHEROID_START_FADE_MS)
        && (EMP_SPHEROID_START_SCALE - 0.01).abs() < 0.0001
        && (EMP_SPHEROID_TARGET_SCALE_MIN - 3.0).abs() < 0.01
        && (EMP_SPHEROID_TARGET_SCALE_MAX - 4.0).abs() < 0.01
        && EMP_SPHEROID_TARGET_SCALE_MAX > EMP_SPHEROID_TARGET_SCALE_MIN
        && EMP_SPHEROID_START_COLOR == (32, 64, 255)
        && EMP_SPHEROID_END_COLOR == (0, 0, 0)
        && EMP_SPHEROID_DISABLE_FX == "EMPSparks"
        && (EMP_SPHEROID_GEOMETRY_RADIUS - 30.0).abs() < 0.01
        && !EMP_SPHEROID_DOES_NOT_AFFECT_OWN_BUILDINGS
}

/// Wave 51 residual honesty: OCL cargo plane / bomb / spheroid residual names.
pub fn honesty_emp_pulse_ocl_residual_ok() -> bool {
    EMP_PULSE_OCL_TRANSPORT == "ChinaJetCargoPlane"
        && EMP_PULSE_BOMB_TEMPLATE == "EMPPulseBomb"
        && EMP_PULSE_EFFECT_SPHEROID == "EMPPulseEffectSpheroid"
        && EMP_PULSE_OCL_EFFECT_SPHEROIDS == "OCL_EMPPulseEffectSpheroids"
        && SUPERWEAPON_EMP_PULSE_NAME == "SuperweaponEMPPulse"
        && SUPERWEAPON_EMP_PULSE_OCL == "SUPERWEAPON_EMPPulse"
        && EMP_PULSE_DROP_VARIANCE == (20.0, 20.0, 0.0)
        && (EMP_PULSE_DELIVERY_DISTANCE - 150.0).abs() < 0.01
        && (EMP_PULSE_DELIVERY_DECAL_RADIUS - 200.0).abs() < 0.01
        && !EMP_PULSE_ACTIVATE_AUDIO.is_empty()
}

/// Wave 51 residual honesty: expanded EMP_HARDENED name residual list.
pub fn honesty_emp_hardened_name_list_residual_ok() -> bool {
    EMP_HARDENED_NAME_MARKERS.len() >= 8
        && is_emp_hardened_name("ChinaJetCargoPlane")
        && is_emp_hardened_name("AmericaJetCargoPlane")
        && is_emp_hardened_name("AmericaJetB52")
        && is_emp_hardened_name("AmericaJetA10Thunderbolt")
        && is_emp_hardened_name("AmericaJetSpectreGunship")
        && is_emp_hardened_name("SupW_AmericaPatriotBattery")
        && is_emp_hardened_name("AmericaJetAuroraEMPHardened")
        && is_emp_hardened_name("Test_EMP_Hardened")
        && !is_emp_hardened_name("ChinaTankBattleMaster")
        && !is_emp_hardened_name("AmericaJetRaptor")
        && !is_emp_hardened_name("AmericaVehicleChinook")
}

/// Combined Wave 51 EMP residual honesty pack.
pub fn honesty_emp_pulse_residual_pack_ok() -> bool {
    honesty_emp_pulse_duration_radius_residual_ok()
        && honesty_emp_spheroid_scale_tint_residual_ok()
        && honesty_emp_pulse_ocl_residual_ok()
        && honesty_emp_hardened_name_list_residual_ok()
}

/// One active residual EMP pulse bookkeeping entry (honesty / debug).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostEmpPulse {
    pub id: u32,
    pub player_id: u32,
    pub location: Vec3,
    pub radius: f32,
    pub activate_frame: u32,
    pub disable_until_frame: u32,
    pub caster_id: Option<ObjectId>,
    /// Units/structures that received DISABLED_EMP this pulse.
    pub disables: u32,
    /// Airborne aircraft killed residual this pulse.
    pub airborne_kills: u32,
}

/// Leftover EMPPulseEffectSpheroid EMPUpdate residual (scale / tint / StartFadeTime).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostEmpPulseSpheroid {
    pub id: ObjectId,
    pub player_id: u32,
    pub location: Vec3,
    pub caster_id: Option<ObjectId>,
    pub tint_env_play_frame: u32,
    pub die_frame: u32,
    pub current_scale: f32,
    pub target_scale: f32,
    pub tint: (u8, u8, u8),
    pub disable_applied: bool,
    pub flashed: bool,
}

/// Host residual registry for EmpPulse special power activations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostEmpPulseRegistry {
    next_id: u32,
    /// Recent pulse activations (bookkeeping; disable timers live on objects).
    activations: Vec<HostEmpPulse>,
    /// Total activations (honesty).
    pub activation_count: u32,
    /// Total DISABLED_EMP grants applied.
    pub disable_count: u32,
    /// Total airborne EMP kills residual.
    pub airborne_kill_count: u32,
    /// Honesty: EMPPulseEffectSpheroid objects spawned.
    pub spheroids_spawned: u32,
    /// Live leftover spheroids waiting StartFadeTime / Lifetime.
    spheroids: Vec<HostEmpPulseSpheroid>,
    last_visual_tick_frame: Option<u32>,
}

impl HostEmpPulseRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn activation_count(&self) -> u32 {
        self.activation_count
    }

    pub fn disable_count(&self) -> u32 {
        self.disable_count
    }

    pub fn airborne_kill_count(&self) -> u32 {
        self.airborne_kill_count
    }

    pub fn activations(&self) -> &[HostEmpPulse] {
        &self.activations
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Record a successful residual EMP pulse activation.
    pub fn record_activation(&mut self, pulse: HostEmpPulse) {
        self.activation_count = self.activation_count.saturating_add(1);
        self.disable_count = self.disable_count.saturating_add(pulse.disables);
        self.airborne_kill_count = self
            .airborne_kill_count
            .saturating_add(pulse.airborne_kills);
        self.activations.push(pulse);
        // Keep bookkeeping bounded (residual, not full history Xfer).
        if self.activations.len() > 32 {
            let drain = self.activations.len() - 32;
            self.activations.drain(0..drain);
        }
    }

    /// Residual honesty: at least one EMP pulse activated.
    pub fn honesty_activate_ok(&self) -> bool {
        self.activation_count > 0
    }

    pub fn record_spheroid_spawn(&mut self) {
        self.spheroids_spawned = self.spheroids_spawned.saturating_add(1);
    }

    pub fn honesty_spheroid_ok(&self) -> bool {
        self.spheroids_spawned > 0
    }

    /// Residual honesty: at least one unit/structure received DISABLED_EMP.
    pub fn honesty_disable_ok(&self) -> bool {
        self.disable_count > 0
    }

    /// Combined host path: activated and applied at least one disable.
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_activate_ok() && self.honesty_disable_ok()
    }

    /// Begin leftover EMPUpdate on a spawned EMPPulseEffectSpheroid.
    pub fn begin_spheroid(
        &mut self,
        id: ObjectId,
        player_id: u32,
        location: Vec3,
        caster_id: Option<ObjectId>,
        now: u32,
    ) {
        let tint_env_play_frame = now.saturating_add(EMP_SPHEROID_START_FADE_FRAMES);
        let die_frame = now.saturating_add(EMP_SPHEROID_LIFETIME_FRAMES);
        let target_scale = leftover_emp_target_scale(id.0.wrapping_add(now));
        self.spheroids.retain(|s| s.id != id);
        self.spheroids.push(HostEmpPulseSpheroid {
            id,
            player_id,
            location,
            caster_id,
            tint_env_play_frame,
            die_frame,
            current_scale: EMP_SPHEROID_START_SCALE,
            target_scale,
            tint: leftover_emp_spheroid_tint(now, tint_env_play_frame),
            disable_applied: false,
            flashed: false,
        });
    }

    pub fn spheroid(&self, id: ObjectId) -> Option<&HostEmpPulseSpheroid> {
        self.spheroids.iter().find(|s| s.id == id)
    }

    pub fn spheroids(&self) -> &[HostEmpPulseSpheroid] {
        &self.spheroids
    }

    /// C++ EMPUpdate::update scale lerp + colorTint / colorFlash.
    pub fn tick_spheroids(&mut self, now: u32) {
        if self.last_visual_tick_frame == Some(now) {
            return;
        }
        self.last_visual_tick_frame = Some(now);
        for s in &mut self.spheroids {
            s.current_scale = tick_leftover_emp_spheroid_scale(s.current_scale, s.target_scale);
            s.tint = leftover_emp_spheroid_tint(now, s.tint_env_play_frame);
            if now == s.tint_env_play_frame {
                s.flashed = true;
            }
        }
    }

    /// Spheroids whose StartFadeTime frame is now and disable has not fired.
    pub fn due_disable_spheroids(&self, now: u32) -> Vec<HostEmpPulseSpheroid> {
        self.spheroids
            .iter()
            .filter(|s| leftover_emp_should_disable(now, s.tint_env_play_frame, s.disable_applied))
            .cloned()
            .collect()
    }

    pub fn mark_disable_applied(&mut self, id: ObjectId) {
        if let Some(s) = self.spheroids.iter_mut().find(|s| s.id == id) {
            s.disable_applied = true;
        }
    }

    pub fn remove_spheroid(&mut self, id: ObjectId) {
        self.spheroids.retain(|s| s.id != id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emp_constants_match_retail_residual() {
        assert!((HOST_EMP_PULSE_RADIUS - 200.0).abs() < 0.01);
        assert_eq!(EMP_PULSE_DISABLED_DURATION_FRAMES, 900);
        assert!(!EMP_PULSE_ACTIVATE_AUDIO.is_empty());
        assert!(honesty_emp_pulse_duration_radius_residual_ok());
    }

    #[test]
    fn legal_emp_disable_target_matrix() {
        // vehicle, faction_struct, spawns, alive, under_construction, emp_hardened
        assert!(is_legal_emp_disable_target(
            true, false, false, true, false, false
        ));
        assert!(is_legal_emp_disable_target(
            false, true, false, true, false, false
        ));
        assert!(is_legal_emp_disable_target(
            false, false, true, true, false, false
        ));
        assert!(!is_legal_emp_disable_target(
            false, false, false, true, false, false
        )); // infantry residual
        assert!(!is_legal_emp_disable_target(
            true, false, false, false, false, false
        ));
        // C++ still disables under-construction buildings and ground EMP_HARDENED.
        assert!(is_legal_emp_disable_target(
            true, false, false, true, true, false
        ));
        assert!(is_legal_emp_disable_target(
            true, false, false, true, false, true
        ));
        assert!(!is_legal_emp_disable_target(
            false, false, false, true, false, false
        )); // non-faction structure path uses is_faction_structure=false
    }

    #[test]
    fn emp_intended_victim_near_miss_fallback_matrix() {
        // Outside EffectRadius 10 but within 40: near-miss disables aircraft.
        assert!(emp_intended_victim_near_miss_disables(
            false,
            true,
            false,
            35.0 * 35.0,
            10.0
        ));
        // Already processed in the radius scan → no fallback.
        assert!(!emp_intended_victim_near_miss_disables(
            true,
            true,
            false,
            35.0 * 35.0,
            10.0
        ));
        // Ground vehicle / infantry → no fallback.
        assert!(!emp_intended_victim_near_miss_disables(
            false,
            false,
            false,
            35.0 * 35.0,
            10.0
        ));
        // EMP_HARDENED aircraft → no fallback.
        assert!(!emp_intended_victim_near_miss_disables(
            false,
            true,
            true,
            35.0 * 35.0,
            10.0
        ));
        // Farther than 40 and radius*2 → miss.
        assert!(!emp_intended_victim_near_miss_disables(
            false,
            true,
            false,
            50.0 * 50.0,
            10.0
        ));
        // Leftover/C++: dist_sqr <= radius * 2.0 (not squared).
        assert!(emp_intended_victim_near_miss_disables(
            false, true, false, 19.0, 10.0
        ));
    }

    #[test]
    fn airborne_kill_and_radius_filters() {
        assert!(should_emp_kill_airborne(true, true, false));
        assert!(emp_skip_ground_when_airborne_only(true, false));
        assert!(!emp_skip_ground_when_airborne_only(true, true));
        assert!(!emp_skip_ground_when_airborne_only(false, false));
        assert!(!emp_skip_ground_when_airborne_only(false, true));
        assert!(!should_emp_kill_airborne(true, false, false));
        assert!(!should_emp_kill_airborne(true, true, true));
        assert!(should_emp_skip_hardened_airborne(true, true, true));
        assert!(!should_emp_skip_hardened_airborne(true, true, false));
        assert!(!should_emp_skip_hardened_airborne(true, false, true));
        assert!(!should_emp_skip_hardened_airborne(false, true, true));
        assert!(!should_emp_kill_airborne(false, true, false));
        assert!(in_emp_pulse_radius_2d((0.0, 0.0), (100.0, 0.0), 200.0));
        assert!(!in_emp_pulse_radius_2d((0.0, 0.0), (250.0, 0.0), 200.0));
        // FROM_BOUNDINGSPHERE_3D: airborne jet at same XZ is outside radius 200.
        assert!(!in_emp_pulse_radius_from_bounding_sphere_3d(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 250.0, 0.0),
            1.0,
            200.0,
        ));
        assert!(in_emp_pulse_radius_from_bounding_sphere_3d(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(100.0, 0.0, 0.0),
            1.0,
            200.0,
        ));
        // Large building whose sphere overlaps 200 but whose center is at 205.
        assert!(in_emp_pulse_radius_from_bounding_sphere_3d(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(205.0, 0.0, 0.0),
            10.0,
            200.0,
        ));
        assert!(!in_emp_pulse_radius_from_bounding_sphere_3d(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(205.0, 0.0, 0.0),
            0.0,
            200.0,
        ));
    }

    #[test]
    fn emp_hardened_name_matrix() {
        assert!(is_emp_hardened_name("AmericaJetAuroraEMPHardened"));
        assert!(is_emp_hardened_name("Test_EMP_Hardened"));
        assert!(!is_emp_hardened_name("ChinaTankBattleMaster"));
        assert!(!is_emp_hardened_name("TestTank"));
        // Wave 51 expanded retail KindOf EMP_HARDENED residual markers.
        assert!(is_emp_hardened_name("ChinaJetCargoPlane"));
        assert!(is_emp_hardened_name("AmericaJetB52"));
        assert!(is_emp_hardened_name("AmericaJetA10Thunderbolt"));
        assert!(is_emp_hardened_name("AmericaJetSpectreGunship"));
        assert!(!is_emp_hardened_name("AmericaJetRaptor"));
        assert!(!is_emp_hardened_name("AmericaVehicleChinook"));
        assert!(honesty_emp_hardened_name_list_residual_ok());
    }

    #[test]
    fn emp_spheroid_scale_tint_residual_honesty() {
        assert!(honesty_emp_spheroid_scale_tint_residual_ok());
        assert_eq!(EMP_SPHEROID_LIFETIME_FRAMES, 90);
        assert_eq!(EMP_SPHEROID_START_FADE_FRAMES, 9);
        assert!((EMP_SPHEROID_START_SCALE - 0.01).abs() < 0.0001);
        assert!((EMP_SPHEROID_TARGET_SCALE_MIN - 3.0).abs() < 0.01);
        assert!((EMP_SPHEROID_TARGET_SCALE_MAX - 4.0).abs() < 0.01);
        assert_eq!(EMP_SPHEROID_END_COLOR, (0, 0, 0));
        let mut scale = EMP_SPHEROID_START_SCALE;
        let target = leftover_emp_target_scale(7);
        assert!(target >= EMP_SPHEROID_TARGET_SCALE_MIN - 0.01);
        assert!(target <= EMP_SPHEROID_TARGET_SCALE_MAX + 0.01);
        let next = tick_leftover_emp_spheroid_scale(scale, target);
        assert!(next > scale);
        scale = next;
        assert!(leftover_emp_should_disable(9, 9, false));
        assert!(!leftover_emp_should_disable(8, 9, false));
        assert!(!leftover_emp_should_disable(9, 9, true));
        assert_eq!(
            leftover_emp_spheroid_tint(0, 9),
            saturate_emp_rgb(EMP_SPHEROID_START_COLOR, 2.0)
        );
        assert_eq!(
            leftover_emp_spheroid_tint(9, 9),
            saturate_emp_rgb(EMP_SPHEROID_END_COLOR, 5.0)
        );
        let _ = scale;
    }

    #[test]
    fn emp_pulse_ocl_residual_honesty() {
        assert!(honesty_emp_pulse_ocl_residual_ok());
        let center = Vec3::new(50.0, 0.0, 75.0);
        let mid = apply_emp_pulse_drop_variance(center, 0.5, 0.5);
        assert!((mid.x - 50.0).abs() < 0.01);
        assert!((mid.z - 75.0).abs() < 0.01);
        let hi = apply_emp_pulse_drop_variance(center, 1.0, 0.0);
        assert!((hi.x - 70.0).abs() < 0.01);
        assert!((hi.z - 55.0).abs() < 0.01);
    }

    #[test]
    fn emp_pulse_residual_pack_honesty() {
        assert!(honesty_emp_pulse_residual_pack_ok());
    }

    /// Wave 72 residual pack honesty gate (wrapper residual_pack_ok).
    #[test]
    fn emp_pulse_residual_pack_honesty_wave72() {
        assert!(honesty_emp_pulse_residual_pack_ok());
        assert!(honesty_emp_pulse_duration_radius_residual_ok());
        assert!(honesty_emp_hardened_name_list_residual_ok());
        assert_eq!(EMP_PULSE_DISABLED_DURATION_FRAMES, 900);
        assert_eq!(SUPERWEAPON_EMP_PULSE_RELOAD_FRAMES, 10_800);
        assert!((HOST_EMP_PULSE_RADIUS - 200.0).abs() < 0.01);
    }

    #[test]
    fn honesty_activate_counters_on_record() {
        let mut reg = HostEmpPulseRegistry::new();
        assert_eq!(reg.activation_count(), 0);
        assert!(!reg.honesty_activate_ok());
        reg.record_activation(HostEmpPulse {
            id: 0,
            player_id: 1,
            location: Vec3::new(10.0, 0.0, 20.0),
            radius: HOST_EMP_PULSE_RADIUS,
            activate_frame: 5,
            disable_until_frame: 5 + EMP_PULSE_DISABLED_DURATION_FRAMES,
            caster_id: Some(ObjectId(7)),
            disables: 3,
            airborne_kills: 1,
        });
        assert_eq!(reg.activation_count(), 1);
        assert_eq!(reg.disable_count(), 3);
        assert_eq!(reg.airborne_kill_count(), 1);
        assert!(reg.honesty_activate_ok());
        assert!(reg.honesty_disable_ok());
        assert!(reg.honesty_host_path_ok());
        assert_eq!(reg.activations().len(), 1);
        assert_eq!(
            reg.activations()[0].disable_until_frame,
            5 + EMP_PULSE_DISABLED_DURATION_FRAMES
        );
    }

    #[test]
    fn honesty_registry_records_disables() {
        let mut reg = HostEmpPulseRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        let id = reg.alloc_id();
        reg.record_activation(HostEmpPulse {
            id,
            player_id: 0,
            location: Vec3::ZERO,
            radius: HOST_EMP_PULSE_RADIUS,
            activate_frame: 0,
            disable_until_frame: 900,
            caster_id: None,
            disables: 2,
            airborne_kills: 0,
        });
        assert!(reg.honesty_activate_ok());
        assert!(reg.honesty_disable_ok());
        assert!(reg.honesty_host_path_ok());
        assert_eq!(reg.disable_count(), 2);
    }

    #[test]
    fn leftover_spheroid_waits_start_fade_and_lerps_scale() {
        let mut reg = HostEmpPulseRegistry::new();
        let id = ObjectId(11);
        reg.begin_spheroid(id, 0, Vec3::ZERO, None, 0);
        let sph = reg.spheroid(id).expect("spheroid");
        assert_eq!(sph.tint_env_play_frame, EMP_SPHEROID_START_FADE_FRAMES);
        assert!((sph.current_scale - EMP_SPHEROID_START_SCALE).abs() < 0.0001);
        assert!(reg.due_disable_spheroids(0).is_empty());
        assert!(reg.due_disable_spheroids(8).is_empty());
        assert_eq!(reg.due_disable_spheroids(9).len(), 1);
        reg.tick_spheroids(1);
        let after = reg.spheroid(id).expect("ticked");
        assert!(after.current_scale > EMP_SPHEROID_START_SCALE);
        assert_eq!(after.tint, saturate_emp_rgb(EMP_SPHEROID_START_COLOR, 2.0));
        reg.tick_spheroids(9);
        let faded = reg.spheroid(id).expect("faded");
        assert!(faded.flashed);
        assert_eq!(faded.tint, saturate_emp_rgb(EMP_SPHEROID_END_COLOR, 5.0));
        reg.mark_disable_applied(id);
        assert!(reg.due_disable_spheroids(9).is_empty());
    }
}
