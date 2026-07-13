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
//! Fail-closed honesty:
//! - Not full OCL cargo plane / EMPPulseBomb projectile / EMPPulseEffectSpheroid
//!   drawable scale/tint / EMPSparks particle volume path
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

/// Retail EMPPulseEffectSpheroid DisabledDuration = 30000 ms.
pub const EMP_PULSE_DISABLED_DURATION_MS: u32 = 30_000;

/// Logic-frame residual of DisabledDuration (ms * 30 / 1000) = 900 frames.
pub const EMP_PULSE_DISABLED_DURATION_FRAMES: u32 =
    (EMP_PULSE_DISABLED_DURATION_MS * 30) / 1000;

/// Activate / impact audio residual (SoundEffects.ini EMPPulseWhoosh / FXList).
pub const EMP_PULSE_ACTIVATE_AUDIO: &str = "EMPPulseWhoosh";

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

/// Name-based EMP_HARDENED residual (fail-closed vs full KindOf mask matrix).
pub fn is_emp_hardened_name(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("emphardened") || n.contains("emp_hardened") || n.contains("empresistant")
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
