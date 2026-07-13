//! Host Cleanup Area residual (Ambulance detox + dozer/minefield clear).
//!
//! Residual slice (playability):
//! - `DoSpecialPower(CleanupArea)` at a world location clears residual hazards
//!   and mines around the target (retail `CleanupAreaPower` →
//!   `CleanupHazardUpdate::setCleanupAreaParameters` + HAZARD_CLEANUP weapon path).
//! - Hazard clear residual: remove host radiation / toxin fields whose epicenters
//!   fall within cleanup radius of the target (AmbulanceCleanHazardWeapon residual:
//!   PrimaryDamageRadius 50, ScanRange 100, MaxMoveDistanceFromLocation 300).
//! - Minefield clear residual: disarm enemy/neutral residual mines within the
//!   same cleanup radius without detonation (dozer/worker mine-clear residual
//!   when caster is a clearer; ambulance also clears mines in ordered area).
//! - Caster residual: ambulance / medic / dozer / worker name gates.
//!
//! Fail-closed honesty:
//! - Not full CleanupHazardUpdate scan/shot/clip / CleanupStreamProjectile path
//! - Not full HazardousMaterialArmor object stack / CLEANUP_HAZARD KindOf matrix
//! - Not full rubble geometry / pathfind ground-rubble zone clear
//! - Not full MaxMoveDistance idle-patrol cleanup loop (instant residual clear)
//! - Network CleanupArea replication (network deferred)

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const CLEANUP_AREA_LOGIC_FPS: f32 = 30.0;

/// Retail AmbulanceCleanHazardWeapon PrimaryDamageRadius residual (= 50).
/// Also used as the hazard/mine clear radius around the ordered location.
pub const HOST_CLEANUP_AREA_RADIUS: f32 = 50.0;

/// Retail CleanupHazardUpdate ScanRange residual (= 100).
/// Fail-closed: host uses PrimaryDamageRadius for the ordered clear; scan is
/// deferred to auto-path residual elsewhere.
pub const HOST_CLEANUP_SCAN_RANGE: f32 = 100.0;

/// Retail CleanupAreaPower MaxMoveDistanceFromLocation residual (= 300).
/// Host residual: caster may order clear if within this distance of target
/// (or target itself is the order point for remote residual).
pub const HOST_CLEANUP_MAX_MOVE_DISTANCE: f32 = 300.0;

/// Activate audio residual (AmbulanceVoiceDetox / InitiateSound).
pub const CLEANUP_AREA_ACTIVATE_AUDIO: &str = "AmbulanceVoiceDetox";

/// Hazard-clear audio residual (WeaponFX_CleanupToxinDetonation cue).
pub const CLEANUP_AREA_HAZARD_AUDIO: &str = "CleanupHazardDetox";

/// Mine-clear audio residual (shared with dozer mine clear).
pub const CLEANUP_AREA_MINE_AUDIO: &str = "MineCleared";

/// Whether template can issue CleanupArea residual (ambulance detox or dozer clear).
pub fn is_cleanup_area_caster(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("ambulance")
        || n.contains("vehiclemedic")
        || n.ends_with("medic")
        || n.contains("dozer")
        || n.contains("worker")
        || n == "testcleanupunit"
}

/// 2D distance check residual (C++ FROM_CENTER_2D).
pub fn in_cleanup_radius_2d(center: (f32, f32), target: (f32, f32), radius: f32) -> bool {
    let dx = center.0 - target.0;
    let dz = center.1 - target.1;
    dx * dx + dz * dz <= radius * radius
}

/// One residual CleanupArea activation bookkeeping entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostCleanupArea {
    pub id: u32,
    pub player_id: u32,
    pub location: Vec3,
    pub radius: f32,
    pub activate_frame: u32,
    pub caster_id: Option<ObjectId>,
    /// Residual radiation fields cleared this activation.
    pub radiation_cleared: u32,
    /// Residual toxin fields cleared this activation.
    pub toxin_cleared: u32,
    /// Residual mines disarmed this activation.
    pub mines_cleared: u32,
}

impl HostCleanupArea {
    pub fn total_cleared(&self) -> u32 {
        self.radiation_cleared
            .saturating_add(self.toxin_cleared)
            .saturating_add(self.mines_cleared)
    }
}

/// Host residual registry for Cleanup Area special power activations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostCleanupAreaRegistry {
    next_id: u32,
    /// Recent activations (bookkeeping).
    activations: Vec<HostCleanupArea>,
    /// Total activations (honesty).
    pub activation_count: u32,
    /// Lifetime radiation fields cleared.
    pub radiation_cleared_total: u32,
    /// Lifetime toxin fields cleared.
    pub toxin_cleared_total: u32,
    /// Lifetime mines disarmed via CleanupArea residual.
    pub mines_cleared_total: u32,
}

impl HostCleanupAreaRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn activation_count(&self) -> u32 {
        self.activation_count
    }

    pub fn radiation_cleared_total(&self) -> u32 {
        self.radiation_cleared_total
    }

    pub fn toxin_cleared_total(&self) -> u32 {
        self.toxin_cleared_total
    }

    pub fn mines_cleared_total(&self) -> u32 {
        self.mines_cleared_total
    }

    pub fn activations(&self) -> &[HostCleanupArea] {
        &self.activations
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Record a successful residual CleanupArea activation.
    pub fn record_activation(&mut self, entry: HostCleanupArea) {
        self.activation_count = self.activation_count.saturating_add(1);
        self.radiation_cleared_total = self
            .radiation_cleared_total
            .saturating_add(entry.radiation_cleared);
        self.toxin_cleared_total = self
            .toxin_cleared_total
            .saturating_add(entry.toxin_cleared);
        self.mines_cleared_total = self
            .mines_cleared_total
            .saturating_add(entry.mines_cleared);
        self.activations.push(entry);
        if self.activations.len() > 32 {
            let drain = self.activations.len() - 32;
            self.activations.drain(0..drain);
        }
    }

    /// Residual honesty: at least one CleanupArea activated.
    pub fn honesty_activate_ok(&self) -> bool {
        self.activation_count > 0
    }

    /// Residual honesty: at least one hazard field or mine cleared.
    pub fn honesty_clear_ok(&self) -> bool {
        self.radiation_cleared_total > 0
            || self.toxin_cleared_total > 0
            || self.mines_cleared_total > 0
    }

    /// Combined host path: activated and cleared at least one residual hazard/mine.
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_activate_ok() && self.honesty_clear_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_area_constants_match_retail_residual() {
        assert!((HOST_CLEANUP_AREA_RADIUS - 50.0).abs() < 0.01);
        assert!((HOST_CLEANUP_SCAN_RANGE - 100.0).abs() < 0.01);
        assert!((HOST_CLEANUP_MAX_MOVE_DISTANCE - 300.0).abs() < 0.01);
        assert!(!CLEANUP_AREA_ACTIVATE_AUDIO.is_empty());
    }

    #[test]
    fn cleanup_area_caster_name_residual() {
        assert!(is_cleanup_area_caster("AmericaVehicleMedic"));
        assert!(is_cleanup_area_caster("USA_Ambulance"));
        assert!(is_cleanup_area_caster("AmericaVehicleDozer"));
        assert!(is_cleanup_area_caster("ChinaVehicleDozer"));
        assert!(is_cleanup_area_caster("GLAWorker"));
        assert!(is_cleanup_area_caster("TestCleanupUnit"));
        assert!(!is_cleanup_area_caster("USA_Ranger"));
        assert!(!is_cleanup_area_caster("TestTank"));
    }

    #[test]
    fn cleanup_area_registry_honesty() {
        let mut reg = HostCleanupAreaRegistry::new();
        assert!(!reg.honesty_activate_ok());
        assert!(!reg.honesty_clear_ok());
        assert!(!reg.honesty_host_path_ok());

        let id = reg.alloc_id();
        reg.record_activation(HostCleanupArea {
            id,
            player_id: 0,
            location: Vec3::ZERO,
            radius: HOST_CLEANUP_AREA_RADIUS,
            activate_frame: 10,
            caster_id: Some(ObjectId(1)),
            radiation_cleared: 1,
            toxin_cleared: 0,
            mines_cleared: 1,
        });
        assert!(reg.honesty_activate_ok());
        assert!(reg.honesty_clear_ok());
        assert!(reg.honesty_host_path_ok());
        assert_eq!(reg.radiation_cleared_total(), 1);
        assert_eq!(reg.mines_cleared_total(), 1);
    }

    #[test]
    fn cleanup_radius_2d() {
        assert!(in_cleanup_radius_2d((0.0, 0.0), (30.0, 0.0), 50.0));
        assert!(!in_cleanup_radius_2d((0.0, 0.0), (80.0, 0.0), 50.0));
    }
}
