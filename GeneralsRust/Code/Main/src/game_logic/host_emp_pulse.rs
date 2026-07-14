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
//! - Not full OCL cargo plane flight / EMPPulseBomb projectile physics /
//!   EMPPulseEffectSpheroid drawable GPU scale/tint / EMPSparks particle volume
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
/// - Not EMP_HARDENED (aircraft kill path skips hardened; residual skip disable)
/// - Not under construction residual
pub fn is_legal_emp_disable_target(
    is_vehicle: bool,
    is_faction_structure: bool,
    is_spawns_are_weapons: bool,
    is_alive: bool,
    under_construction: bool,
    is_emp_hardened: bool,
) -> bool {
    if !is_alive || under_construction || is_emp_hardened {
        return false;
    }
    is_vehicle || is_faction_structure || is_spawns_are_weapons
}

/// True when residual EMP should kill instead of disable (airborne aircraft).
///
/// C++: KINDOF_AIRCRAFT && isAirborneTarget && !KINDOF_EMP_HARDENED → kill.
pub fn should_emp_kill_airborne(
    is_aircraft: bool,
    is_airborne: bool,
    is_emp_hardened: bool,
) -> bool {
    is_aircraft && is_airborne && !is_emp_hardened
}

/// 2D distance check residual (ground plane x/z; host gameplay convention).
pub fn in_emp_pulse_radius_2d(center: (f32, f32), target: (f32, f32), radius: f32) -> bool {
    let dx = center.0 - target.0;
    let dz = center.1 - target.1;
    dx * dx + dz * dz <= radius * radius
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

    /// Residual honesty: at least one unit/structure received DISABLED_EMP.
    pub fn honesty_disable_ok(&self) -> bool {
        self.disable_count > 0
    }

    /// Combined host path: activated and applied at least one disable.
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_activate_ok() && self.honesty_disable_ok()
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
        assert!(!is_legal_emp_disable_target(
            true, false, false, true, true, false
        ));
        assert!(!is_legal_emp_disable_target(
            true, false, false, true, false, true
        ));
        assert!(!is_legal_emp_disable_target(
            false, false, false, true, false, false
        )); // non-faction structure path uses is_faction_structure=false
    }

    #[test]
    fn airborne_kill_and_radius_filters() {
        assert!(should_emp_kill_airborne(true, true, false));
        assert!(!should_emp_kill_airborne(true, false, false));
        assert!(!should_emp_kill_airborne(true, true, true));
        assert!(!should_emp_kill_airborne(false, true, false));
        assert!(in_emp_pulse_radius_2d((0.0, 0.0), (100.0, 0.0), 200.0));
        assert!(!in_emp_pulse_radius_2d((0.0, 0.0), (250.0, 0.0), 200.0));
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
        assert_eq!(EMP_SPHEROID_START_COLOR, (32, 64, 255));
        assert_eq!(EMP_SPHEROID_END_COLOR, (0, 0, 0));
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
}
