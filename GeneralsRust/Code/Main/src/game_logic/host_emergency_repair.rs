//! Host Emergency Repair special-power residual — single-burst ally vehicle heal.
//!
//! Residual slice (playability):
//! - `DoSpecialPower(EmergencyRepair)` at a world location heals damaged same-team
//!   **VEHICLE** units in radius (retail SuperweaponEmergencyRepair →
//!   SUPERWEAPON_RepairVehicles* → RepairVehiclesInArea_InvisibleMarker_Level*
//!   AutoHealBehavior SingleBurst).
//! - HealingAmount residual 100 / 200 / 300 by science tier (Level1/2/3).
//! - Radius residual 100 (RadiusCursorRadius / AutoHealBehavior Radius).
//! - Honesty counters/flags for residual gates and tests.
//!
//! Wave 52 residual pack (retail System.ini / Science.ini / SpecialPower.ini):
//! - SCIENCE_EmergencyRepair1/2/3 science tier gate residual
//! - OCL RepairVehiclesInArea_InvisibleMarker_Level1/2/3 templates
//! - HealingAmount 100/200/300 + Radius 100 + KindOf=VEHICLE + SingleBurst
//! - Superweapon reload 240000 ms residual
//! - DeletionUpdate Min/MaxLifetime = 0 (immediate one-pulse)
//! - ParticleSysBone RepairCloud residual name
//!
//! Fail-closed honesty:
//! - RepairVehiclesInArea_InvisibleMarker spawn residual closed (RepairCloud GPU fail-closed)
//! - Not full ally relationship filter (uses same-team residual)
//! - Not full player science ownership matrix beyond residual name tier gate
//! - Not KindOf aircraft-as-vehicle edge cases beyond residual Vehicle KindOf
//! - Not network EmergencyRepair replication (network deferred)

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const EMERGENCY_REPAIR_LOGIC_FPS: f32 = 30.0;

/// Retail SuperweaponEmergencyRepair RadiusCursorRadius residual (= 100).
/// Also matches RepairVehiclesInArea_InvisibleMarker AutoHealBehavior Radius.
pub const HOST_EMERGENCY_REPAIR_RADIUS: f32 = 100.0;

/// Retail RepairVehiclesInArea_InvisibleMarker_Level1 HealingAmount.
pub const EMERGENCY_REPAIR_LEVEL1_HEAL: f32 = 100.0;
/// Retail RepairVehiclesInArea_InvisibleMarker_Level2 HealingAmount.
pub const EMERGENCY_REPAIR_LEVEL2_HEAL: f32 = 200.0;
/// Retail RepairVehiclesInArea_InvisibleMarker_Level3 HealingAmount.
pub const EMERGENCY_REPAIR_LEVEL3_HEAL: f32 = 300.0;

/// Activate audio residual (SpecialPower.ini InitiateAtLocationSound).
pub const EMERGENCY_REPAIR_ACTIVATE_AUDIO: &str = "EmergencyRepairActivate";

/// Retail SuperweaponEmergencyRepair ReloadTime residual (msec).
pub const EMERGENCY_REPAIR_RELOAD_TIME_MS: u32 = 240_000;
/// ReloadTime 240000 ms → 7200 frames @ 30 FPS.
pub const EMERGENCY_REPAIR_RELOAD_TIME_FRAMES: u32 = 7_200;

/// Retail science tier residual names (Science.ini).
pub const SCIENCE_EMERGENCY_REPAIR1: &str = "SCIENCE_EmergencyRepair1";
pub const SCIENCE_EMERGENCY_REPAIR2: &str = "SCIENCE_EmergencyRepair2";
pub const SCIENCE_EMERGENCY_REPAIR3: &str = "SCIENCE_EmergencyRepair3";

/// Retail SpecialPower template residual.
pub const SUPERWEAPON_EMERGENCY_REPAIR: &str = "SuperweaponEmergencyRepair";

/// Retail OCL / System.ini invisible-marker templates.
pub const EMERGENCY_REPAIR_MARKER_LEVEL1: &str = "RepairVehiclesInArea_InvisibleMarker_Level1";
pub const EMERGENCY_REPAIR_MARKER_LEVEL2: &str = "RepairVehiclesInArea_InvisibleMarker_Level2";
pub const EMERGENCY_REPAIR_MARKER_LEVEL3: &str = "RepairVehiclesInArea_InvisibleMarker_Level3";

/// Retail ParticleSysBone residual on RepairVehicles marker draw.
pub const EMERGENCY_REPAIR_CLOUD_PARTICLE: &str = "RepairCloud";

/// Retail AutoHealBehavior KindOf residual name.
pub const EMERGENCY_REPAIR_AFFECT_KINDOF: &str = "VEHICLE";
/// Retail AutoHealBehavior SingleBurst residual.
pub const EMERGENCY_REPAIR_SINGLE_BURST: bool = true;
/// Retail AutoHealBehavior StartsActive residual.
pub const EMERGENCY_REPAIR_STARTS_ACTIVE: bool = true;
/// Retail AutoHealBehavior HealingDelay residual (msec; effectively sleep forever).
pub const EMERGENCY_REPAIR_HEALING_DELAY_MS: u32 = 1;

/// Retail DeletionUpdate Min/MaxLifetime residual (msec = 0 → immediate one-pulse).
pub const EMERGENCY_REPAIR_MARKER_DELETION_LIFETIME_MS: u32 = 0;
/// DeletionUpdate lifetime residual frames (0 msec → 0 frames).
pub const EMERGENCY_REPAIR_MARKER_DELETION_LIFETIME_FRAMES: u32 = 0;

/// Convert msec residual → logic frames @ 30 FPS (C++ parseDurationUnsignedInt ceil).
pub fn emergency_repair_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * (EMERGENCY_REPAIR_LOGIC_FPS / 1000.0)).ceil() as u32
}

/// Residual Emergency Repair science tier → HealingAmount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostEmergencyRepairLevel {
    /// SCIENCE_EmergencyRepair1 → Level1 (100 HP).
    One = 1,
    /// SCIENCE_EmergencyRepair2 → Level2 (200 HP).
    Two = 2,
    /// SCIENCE_EmergencyRepair3 → Level3 (300 HP).
    Three = 3,
}

impl HostEmergencyRepairLevel {
    /// Parse residual level from 1..=3 (fail-closed: unknown → One).
    pub fn from_u8(level: u8) -> Self {
        match level {
            2 => HostEmergencyRepairLevel::Two,
            3 => HostEmergencyRepairLevel::Three,
            _ => HostEmergencyRepairLevel::One,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Retail science residual name for this tier.
    pub fn science_name(self) -> &'static str {
        match self {
            HostEmergencyRepairLevel::One => SCIENCE_EMERGENCY_REPAIR1,
            HostEmergencyRepairLevel::Two => SCIENCE_EMERGENCY_REPAIR2,
            HostEmergencyRepairLevel::Three => SCIENCE_EMERGENCY_REPAIR3,
        }
    }

    /// Retail OCL / System.ini invisible-marker template for this tier.
    pub fn marker_template(self) -> &'static str {
        match self {
            HostEmergencyRepairLevel::One => EMERGENCY_REPAIR_MARKER_LEVEL1,
            HostEmergencyRepairLevel::Two => EMERGENCY_REPAIR_MARKER_LEVEL2,
            HostEmergencyRepairLevel::Three => EMERGENCY_REPAIR_MARKER_LEVEL3,
        }
    }

    /// Retail AutoHealBehavior HealingAmount for this tier.
    pub fn heal_amount(self) -> f32 {
        match self {
            HostEmergencyRepairLevel::One => EMERGENCY_REPAIR_LEVEL1_HEAL,
            HostEmergencyRepairLevel::Two => EMERGENCY_REPAIR_LEVEL2_HEAL,
            HostEmergencyRepairLevel::Three => EMERGENCY_REPAIR_LEVEL3_HEAL,
        }
    }

    /// Residual radius (all levels share RadiusCursorRadius / AutoHeal Radius = 100).
    pub fn radius(self) -> f32 {
        HOST_EMERGENCY_REPAIR_RADIUS
    }
}

/// Map science residual name → Emergency Repair level (fail-closed: unknown → One).
pub fn emergency_repair_level_from_science(science: &str) -> HostEmergencyRepairLevel {
    match science {
        SCIENCE_EMERGENCY_REPAIR2 | "Early_SCIENCE_EmergencyRepair2" => {
            HostEmergencyRepairLevel::Two
        }
        SCIENCE_EMERGENCY_REPAIR3 | "Early_SCIENCE_EmergencyRepair3" => {
            HostEmergencyRepairLevel::Three
        }
        SCIENCE_EMERGENCY_REPAIR1 | "Early_SCIENCE_EmergencyRepair1" | _ => {
            HostEmergencyRepairLevel::One
        }
    }
}

/// Select highest unlocked EmergencyRepair science tier (fail-closed → Level1).
pub fn highest_emergency_repair_level_from_sciences<'a, I>(sciences: I) -> HostEmergencyRepairLevel
where
    I: IntoIterator<Item = &'a str>,
{
    let mut best = HostEmergencyRepairLevel::One;
    for s in sciences {
        let n = s.to_ascii_lowercase().replace('_', "").replace('-', "");
        if n.contains("emergencyrepair3") {
            return HostEmergencyRepairLevel::Three;
        }
        if n.contains("emergencyrepair2") {
            best = HostEmergencyRepairLevel::Two;
        }
    }
    best
}

/// Whether residual target can receive Emergency Repair heal burst.
///
/// Retail AutoHealBehavior KindOf = VEHICLE, SingleBurst, StartsActive:
/// - same-team residual (allies)
/// - alive
/// - VEHICLE KindOf
/// - not under construction residual
/// - damaged (current < max) so heal is observable
pub fn is_legal_emergency_repair_target(
    is_vehicle: bool,
    is_alive: bool,
    same_team: bool,
    under_construction: bool,
    is_damaged: bool,
) -> bool {
    is_vehicle && is_alive && same_team && !under_construction && is_damaged
}

/// 2D distance check residual (C++ FROM_CENTER_2D / AutoHeal Radius).
pub fn in_emergency_repair_radius_2d(center: (f32, f32), target: (f32, f32), radius: f32) -> bool {
    let dx = center.0 - target.0;
    let dz = center.1 - target.1;
    dx * dx + dz * dz <= radius * radius
}

/// Wave 52 residual honesty: science tier / amount / radius / marker lifetime pack.
pub fn honesty_emergency_repair_residual_ok() -> bool {
    (HOST_EMERGENCY_REPAIR_RADIUS - 100.0).abs() < 0.01
        && (HostEmergencyRepairLevel::One.heal_amount() - 100.0).abs() < 0.01
        && (HostEmergencyRepairLevel::Two.heal_amount() - 200.0).abs() < 0.01
        && (HostEmergencyRepairLevel::Three.heal_amount() - 300.0).abs() < 0.01
        && HostEmergencyRepairLevel::One.science_name() == SCIENCE_EMERGENCY_REPAIR1
        && HostEmergencyRepairLevel::Two.science_name() == SCIENCE_EMERGENCY_REPAIR2
        && HostEmergencyRepairLevel::Three.science_name() == SCIENCE_EMERGENCY_REPAIR3
        && HostEmergencyRepairLevel::One.marker_template() == EMERGENCY_REPAIR_MARKER_LEVEL1
        && HostEmergencyRepairLevel::Two.marker_template() == EMERGENCY_REPAIR_MARKER_LEVEL2
        && HostEmergencyRepairLevel::Three.marker_template() == EMERGENCY_REPAIR_MARKER_LEVEL3
        && EMERGENCY_REPAIR_RELOAD_TIME_MS == 240_000
        && EMERGENCY_REPAIR_RELOAD_TIME_FRAMES
            == emergency_repair_ms_to_frames(EMERGENCY_REPAIR_RELOAD_TIME_MS)
        && EMERGENCY_REPAIR_MARKER_DELETION_LIFETIME_MS == 0
        && EMERGENCY_REPAIR_MARKER_DELETION_LIFETIME_FRAMES == 0
        && EMERGENCY_REPAIR_AFFECT_KINDOF == "VEHICLE"
        && EMERGENCY_REPAIR_SINGLE_BURST
        && EMERGENCY_REPAIR_STARTS_ACTIVE
        && EMERGENCY_REPAIR_HEALING_DELAY_MS == 1
        && EMERGENCY_REPAIR_CLOUD_PARTICLE == "RepairCloud"
        && SUPERWEAPON_EMERGENCY_REPAIR == "SuperweaponEmergencyRepair"
        && !EMERGENCY_REPAIR_ACTIVATE_AUDIO.is_empty()
}
/// Combined residual honesty pack (Wave 71).
pub fn honesty_emergency_repair_residual_pack_ok() -> bool {
    honesty_emergency_repair_residual_ok()
}

/// One active residual Emergency Repair activation bookkeeping entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostEmergencyRepair {
    pub id: u32,
    pub player_id: u32,
    pub location: Vec3,
    pub radius: f32,
    pub level: HostEmergencyRepairLevel,
    pub activate_frame: u32,
    pub caster_id: Option<ObjectId>,
    /// Ally vehicles that received the SingleBurst heal this activation.
    pub heals: u32,
    /// Total HP restored this activation (honesty / debug).
    pub heal_amount_total: f32,
}

/// Host residual registry for Emergency Repair special power activations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostEmergencyRepairRegistry {
    next_id: u32,
    /// Recent activations (bookkeeping).
    activations: Vec<HostEmergencyRepair>,
    /// Total activations (honesty).
    pub activation_count: u32,
    /// Total SingleBurst heal grants applied.
    pub heal_count: u32,
    /// Cumulative HP restored.
    pub heal_amount_total: f32,
    /// Honesty: OCL invisible markers spawned.
    pub markers_spawned: u32,
}

impl HostEmergencyRepairRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn activation_count(&self) -> u32 {
        self.activation_count
    }

    pub fn record_marker_spawn(&mut self) {
        self.markers_spawned = self.markers_spawned.saturating_add(1);
    }

    pub fn honesty_marker_ok(&self) -> bool {
        self.markers_spawned > 0
    }

    pub fn heal_count(&self) -> u32 {
        self.heal_count
    }

    pub fn heal_amount_total(&self) -> f32 {
        self.heal_amount_total
    }

    pub fn activations(&self) -> &[HostEmergencyRepair] {
        &self.activations
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Record a successful residual Emergency Repair activation.
    pub fn record_activation(&mut self, entry: HostEmergencyRepair) {
        self.activation_count = self.activation_count.saturating_add(1);
        self.heal_count = self.heal_count.saturating_add(entry.heals);
        self.heal_amount_total += entry.heal_amount_total;
        self.activations.push(entry);
        // Keep bookkeeping bounded (residual, not full history Xfer).
        if self.activations.len() > 32 {
            let drain = self.activations.len() - 32;
            self.activations.drain(0..drain);
        }
    }

    /// Residual honesty: at least one Emergency Repair activated.
    pub fn honesty_activate_ok(&self) -> bool {
        self.activation_count > 0
    }

    /// Residual honesty: at least one vehicle received heal.
    pub fn honesty_heal_ok(&self) -> bool {
        self.heal_count > 0
    }

    /// Combined host path: activated and healed at least one vehicle.
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_activate_ok() && self.honesty_heal_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emergency_repair_constants_match_retail_residual() {
        assert!((HOST_EMERGENCY_REPAIR_RADIUS - 100.0).abs() < 0.01);
        assert!((HostEmergencyRepairLevel::One.heal_amount() - 100.0).abs() < 0.01);
        assert!((HostEmergencyRepairLevel::Two.heal_amount() - 200.0).abs() < 0.01);
        assert!((HostEmergencyRepairLevel::Three.heal_amount() - 300.0).abs() < 0.01);
        assert!(!EMERGENCY_REPAIR_ACTIVATE_AUDIO.is_empty());
    }

    #[test]
    fn emergency_repair_residual_pack_honesty() {
        assert!(honesty_emergency_repair_residual_ok());
        // Science tier gate residual.
        assert_eq!(SCIENCE_EMERGENCY_REPAIR1, "SCIENCE_EmergencyRepair1");
        assert_eq!(SCIENCE_EMERGENCY_REPAIR2, "SCIENCE_EmergencyRepair2");
        assert_eq!(SCIENCE_EMERGENCY_REPAIR3, "SCIENCE_EmergencyRepair3");
        assert_eq!(
            emergency_repair_level_from_science(SCIENCE_EMERGENCY_REPAIR1),
            HostEmergencyRepairLevel::One
        );
        assert_eq!(
            emergency_repair_level_from_science(SCIENCE_EMERGENCY_REPAIR2),
            HostEmergencyRepairLevel::Two
        );
        assert_eq!(
            emergency_repair_level_from_science(SCIENCE_EMERGENCY_REPAIR3),
            HostEmergencyRepairLevel::Three
        );
        assert_eq!(
            emergency_repair_level_from_science("Early_SCIENCE_EmergencyRepair3"),
            HostEmergencyRepairLevel::Three
        );
        // Amount residual per level.
        assert_eq!(EMERGENCY_REPAIR_LEVEL1_HEAL, 100.0);
        assert_eq!(EMERGENCY_REPAIR_LEVEL2_HEAL, 200.0);
        assert_eq!(EMERGENCY_REPAIR_LEVEL3_HEAL, 300.0);
        // Superweapon reload residual duration.
        assert_eq!(EMERGENCY_REPAIR_RELOAD_TIME_MS, 240_000);
        assert_eq!(EMERGENCY_REPAIR_RELOAD_TIME_FRAMES, 7_200);
        assert_eq!(emergency_repair_ms_to_frames(240_000), 7_200);
        // Marker lifetime residual (immediate pulse).
        assert_eq!(EMERGENCY_REPAIR_MARKER_DELETION_LIFETIME_MS, 0);
        assert_eq!(EMERGENCY_REPAIR_MARKER_DELETION_LIFETIME_FRAMES, 0);
        // OCL + particle + KindOf residual.
        assert_eq!(
            EMERGENCY_REPAIR_MARKER_LEVEL1,
            "RepairVehiclesInArea_InvisibleMarker_Level1"
        );
        assert_eq!(EMERGENCY_REPAIR_CLOUD_PARTICLE, "RepairCloud");
        assert_eq!(EMERGENCY_REPAIR_AFFECT_KINDOF, "VEHICLE");
        assert!(EMERGENCY_REPAIR_SINGLE_BURST);
        assert_eq!(
            HostEmergencyRepairLevel::Two.marker_template(),
            EMERGENCY_REPAIR_MARKER_LEVEL2
        );
        assert!((HostEmergencyRepairLevel::Three.radius() - 100.0).abs() < 0.01);
    }

    #[test]
    fn legal_emergency_repair_target_matrix() {
        // is_vehicle, alive, same_team, under_construction, is_damaged
        assert!(is_legal_emergency_repair_target(
            true, true, true, false, true
        ));
        assert!(!is_legal_emergency_repair_target(
            false, true, true, false, true
        )); // infantry
        assert!(!is_legal_emergency_repair_target(
            true, false, true, false, true
        )); // dead
        assert!(!is_legal_emergency_repair_target(
            true, true, false, false, true
        )); // enemy
        assert!(!is_legal_emergency_repair_target(
            true, true, true, true, true
        )); // constructing
        assert!(!is_legal_emergency_repair_target(
            true, true, true, false, false
        )); // full HP
    }

    #[test]
    fn emergency_repair_radius_and_level_parse() {
        assert!(in_emergency_repair_radius_2d(
            (0.0, 0.0),
            (50.0, 0.0),
            100.0
        ));
        assert!(!in_emergency_repair_radius_2d(
            (0.0, 0.0),
            (150.0, 0.0),
            100.0
        ));
        assert_eq!(
            HostEmergencyRepairLevel::from_u8(1),
            HostEmergencyRepairLevel::One
        );
        assert_eq!(
            HostEmergencyRepairLevel::from_u8(2),
            HostEmergencyRepairLevel::Two
        );
        assert_eq!(
            HostEmergencyRepairLevel::from_u8(3),
            HostEmergencyRepairLevel::Three
        );
        assert_eq!(
            HostEmergencyRepairLevel::from_u8(99),
            HostEmergencyRepairLevel::One
        ); // fail-closed
    }

    #[test]
    fn honesty_registry_records_heals() {
        let mut reg = HostEmergencyRepairRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        let id = reg.alloc_id();
        reg.record_activation(HostEmergencyRepair {
            id,
            player_id: 1,
            location: Vec3::ZERO,
            radius: HOST_EMERGENCY_REPAIR_RADIUS,
            level: HostEmergencyRepairLevel::One,
            activate_frame: 0,
            caster_id: None,
            heals: 2,
            heal_amount_total: 200.0,
        });
        assert!(reg.honesty_activate_ok());
        assert!(reg.honesty_heal_ok());
        assert!(reg.honesty_host_path_ok());
        assert_eq!(reg.activation_count(), 1);
        assert_eq!(reg.heal_count(), 2);
        assert!((reg.heal_amount_total() - 200.0).abs() < 0.01);
    }
    /// Wave 71 residual pack honesty gate.
    #[test]
    fn emergency_repair_residual_pack_honesty_wave71() {
        assert!(honesty_emergency_repair_residual_pack_ok());
        assert_eq!(EMERGENCY_REPAIR_LEVEL1_HEAL, 100.0);
        assert_eq!(EMERGENCY_REPAIR_LEVEL3_HEAL, 300.0);
        assert_eq!(EMERGENCY_REPAIR_RELOAD_TIME_FRAMES, 7200);
    }
}
