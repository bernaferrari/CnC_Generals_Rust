//! Host upgrade queue/complete residual.
//!
//! Residual slice: `QueueUpgrade` → research complete → unlocks something
//! **observable** on host GameLogic:
//! - Capture building research unlocks infantry capture ability
//! - FlashBang research equips Ranger secondary grenade + upgrade tag
//! - TOW Missile research equips Humvee secondary + upgrade tag
//! - Supply Lines research tags supply centers (flag residual)
//!
//! Fail-closed: not full science tree, ProductionUpdate build-time parity,
//! WeaponSetUpgrade module matrix, or multiplayer upgrade replication.

use super::{Team, ObjectId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Retail / host upgrade name constants used by residual effects.
pub const UPGRADE_INFANTRY_CAPTURE: &str = "Upgrade_InfantryCaptureBuilding";
pub const UPGRADE_AMERICA_RANGER_CAPTURE: &str = "Upgrade_AmericaRangerCaptureBuilding";
pub const UPGRADE_CHINA_REDGUARD_CAPTURE: &str = "Upgrade_ChinaRedguardCaptureBuilding";
pub const UPGRADE_GLA_REBEL_CAPTURE: &str = "Upgrade_GLARebelCaptureBuilding";
pub const UPGRADE_AMERICA_FLASHBANG: &str = "Upgrade_AmericaRangerFlashBangGrenade";
pub const UPGRADE_AMERICA_TOW: &str = "Upgrade_AmericaTOWMissile";
pub const UPGRADE_AMERICA_SUPPLY_LINES: &str = "Upgrade_AmericaSupplyLines";

/// Normalize upgrade identity the same way Player does (alphanumeric lower).
pub fn normalize_upgrade_identity(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Host residual upgrade kinds with known observable unlocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostUpgradeKind {
    /// Unlocks CaptureBuilding special ability for infantry.
    CaptureBuilding,
    /// Equips Ranger SECONDARY flashbang grenade weapon set.
    FlashBangGrenade,
    /// Equips Humvee SECONDARY TOW missile weapon set.
    TowMissile,
    /// Supply-lines research (tags supply centers; economy bonus residual deferred).
    SupplyLines,
    /// Other / unknown upgrades (unlock flag only).
    Other,
}

impl HostUpgradeKind {
    /// Classify an upgrade name into a residual kind.
    pub fn from_name(name: &str) -> Self {
        let n = normalize_upgrade_identity(name);
        if n.contains("capturebuilding") || n.contains("infantrycapture") {
            HostUpgradeKind::CaptureBuilding
        } else if n.contains("flashbang") {
            HostUpgradeKind::FlashBangGrenade
        } else if n.contains("towmissile") || n == "upgradeamericatowmissile" {
            HostUpgradeKind::TowMissile
        } else if n.contains("supplylines") {
            HostUpgradeKind::SupplyLines
        } else {
            HostUpgradeKind::Other
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            HostUpgradeKind::CaptureBuilding => "CaptureBuilding",
            HostUpgradeKind::FlashBangGrenade => "FlashBangGrenade",
            HostUpgradeKind::TowMissile => "TowMissile",
            HostUpgradeKind::SupplyLines => "SupplyLines",
            HostUpgradeKind::Other => "Other",
        }
    }

    /// Residual research frames before complete (host path).
    /// `0` / `1` ⇒ complete on the next `GameLogic::update` (queue still observable).
    /// Fail-closed: not retail Upgrade.ini BuildTime (30s).
    pub fn residual_research_frames(self) -> u32 {
        match self {
            // One logic update of research so QueueUpgrade is observably pending.
            HostUpgradeKind::CaptureBuilding
            | HostUpgradeKind::FlashBangGrenade
            | HostUpgradeKind::TowMissile
            | HostUpgradeKind::SupplyLines
            | HostUpgradeKind::Other => 1,
        }
    }
}

/// Lifecycle of a host residual upgrade research entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostUpgradePhase {
    /// Queued after QueueUpgrade; waiting for research complete.
    Queued,
    /// Research finished; unlock effects applied.
    Completed,
    /// Cancelled / refunded before complete.
    Cancelled,
}

/// One queued or completed host upgrade research record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostUpgradeResearch {
    pub id: u32,
    pub name: String,
    pub kind: HostUpgradeKind,
    pub team: Team,
    pub player_id: u32,
    pub queue_frame: u32,
    pub complete_frame: u32,
    pub phase: HostUpgradePhase,
    /// Units that received an observable effect (weapon equip / upgrade tag).
    pub units_affected: u32,
    /// Producer building (optional residual).
    pub source_object: Option<ObjectId>,
}

/// Host registry of upgrade research queue/complete for honesty + residual apply.
#[derive(Debug, Clone, Default)]
pub struct HostUpgradeRegistry {
    next_id: u32,
    /// Active + completed research keyed by id.
    entries: HashMap<u32, HostUpgradeResearch>,
    /// Pending lookup: (player_id, normalized name) → entry id.
    pending_index: HashMap<(u32, String), u32>,
    completed_this_frame: Vec<u32>,
    queued_this_frame: Vec<u32>,
}

impl HostUpgradeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    pub fn clear_frame_events(&mut self) {
        self.completed_this_frame.clear();
        self.queued_this_frame.clear();
    }

    pub fn get(&self, id: u32) -> Option<&HostUpgradeResearch> {
        self.entries.get(&id)
    }

    pub fn entries_snapshot(&self) -> Vec<HostUpgradeResearch> {
        let mut v: Vec<_> = self.entries.values().cloned().collect();
        v.sort_by_key(|e| e.id);
        v
    }

    pub fn pending_of_kind(&self, kind: HostUpgradeKind) -> Vec<&HostUpgradeResearch> {
        self.entries
            .values()
            .filter(|e| e.kind == kind && e.phase == HostUpgradePhase::Queued)
            .collect()
    }

    pub fn completed_of_kind(&self, kind: HostUpgradeKind) -> Vec<&HostUpgradeResearch> {
        self.entries
            .values()
            .filter(|e| e.kind == kind && e.phase == HostUpgradePhase::Completed)
            .collect()
    }

    /// Record a newly queued upgrade research (idempotent per player+name).
    pub fn record_queue(
        &mut self,
        name: &str,
        team: Team,
        player_id: u32,
        frame: u32,
        source_object: Option<ObjectId>,
    ) -> u32 {
        let key = (player_id, normalize_upgrade_identity(name));
        if let Some(&existing) = self.pending_index.get(&key) {
            return existing;
        }
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let kind = HostUpgradeKind::from_name(name);
        let entry = HostUpgradeResearch {
            id,
            name: name.to_string(),
            kind,
            team,
            player_id,
            queue_frame: frame,
            complete_frame: 0,
            phase: HostUpgradePhase::Queued,
            units_affected: 0,
            source_object,
        };
        self.entries.insert(id, entry);
        self.pending_index.insert(key, id);
        self.queued_this_frame.push(id);
        id
    }

    /// Mark research completed and store how many units were affected.
    pub fn record_complete(
        &mut self,
        name: &str,
        player_id: u32,
        frame: u32,
        units_affected: u32,
    ) -> Option<u32> {
        let key = (player_id, normalize_upgrade_identity(name));
        let id = if let Some(&id) = self.pending_index.get(&key) {
            id
        } else {
            // Complete without prior queue record (script grant path) — create completed entry.
            let id = self.next_id;
            self.next_id = self.next_id.saturating_add(1);
            let kind = HostUpgradeKind::from_name(name);
            self.entries.insert(
                id,
                HostUpgradeResearch {
                    id,
                    name: name.to_string(),
                    kind,
                    team: Team::Neutral,
                    player_id,
                    queue_frame: frame,
                    complete_frame: frame,
                    phase: HostUpgradePhase::Completed,
                    units_affected,
                    source_object: None,
                },
            );
            self.completed_this_frame.push(id);
            return Some(id);
        };

        if let Some(entry) = self.entries.get_mut(&id) {
            entry.phase = HostUpgradePhase::Completed;
            entry.complete_frame = frame;
            entry.units_affected = units_affected;
        }
        self.pending_index.remove(&key);
        self.completed_this_frame.push(id);
        Some(id)
    }

    /// Cancel a pending research (refund path).
    pub fn record_cancel(&mut self, name: &str, player_id: u32) -> bool {
        let key = (player_id, normalize_upgrade_identity(name));
        let Some(id) = self.pending_index.remove(&key) else {
            return false;
        };
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.phase = HostUpgradePhase::Cancelled;
        }
        true
    }

    // --- Honesty flags (host residual; do not claim full retail parity) ---

    pub fn honesty_queue_ok(&self, kind: HostUpgradeKind) -> bool {
        !self.pending_of_kind(kind).is_empty()
    }

    pub fn honesty_complete_ok(&self, kind: HostUpgradeKind) -> bool {
        self.completed_of_kind(kind)
            .iter()
            .any(|e| e.phase == HostUpgradePhase::Completed)
    }

    /// Host path honesty: completed research with at least one observable unlock
    /// (units_affected > 0) **or** CaptureBuilding (ability unlock is player-flag only).
    pub fn honesty_host_path_ok(&self, kind: HostUpgradeKind) -> bool {
        self.completed_of_kind(kind).iter().any(|e| {
            e.phase == HostUpgradePhase::Completed
                && (e.units_affected > 0
                    || kind == HostUpgradeKind::CaptureBuilding
                    || kind == HostUpgradeKind::SupplyLines
                    || kind == HostUpgradeKind::Other)
        })
    }

    /// True if any completed FlashBang research equipped at least one unit.
    pub fn honesty_flashbang_equipped_ok(&self) -> bool {
        self.completed_of_kind(HostUpgradeKind::FlashBangGrenade)
            .iter()
            .any(|e| e.units_affected > 0)
    }

    /// True if Capture research completed (player unlock flag is the ability gate).
    pub fn honesty_capture_unlock_ok(&self) -> bool {
        self.honesty_complete_ok(HostUpgradeKind::CaptureBuilding)
    }
}

/// Template names that receive FlashBang secondary when research completes.
pub fn is_flashbang_unit_template(name: &str) -> bool {
    matches!(
        name,
        "USA_Ranger"
            | "GoldenRanger"
            | "AmericaInfantryRanger"
            | "TestRanger"
            | "TestInfantry"
    ) || {
        let n = name.to_ascii_lowercase();
        n.contains("ranger") && !n.contains("humvee")
    }
}

/// Template names that receive TOW secondary when research completes.
pub fn is_tow_unit_template(name: &str) -> bool {
    matches!(name, "USA_Humvee" | "AmericaVehicleHumvee" | "GoldenHumvee")
        || name.to_ascii_lowercase().contains("humvee")
}

/// Template names that receive Capture upgrade tag (observability residual).
pub fn is_capture_capable_infantry_template(name: &str) -> bool {
    if name.to_ascii_lowercase().contains("worker")
        || name.to_ascii_lowercase().contains("dozer")
        || name.to_ascii_lowercase().contains("pilot")
    {
        return false;
    }
    matches!(
        name,
        "USA_Ranger"
            | "GoldenRanger"
            | "AmericaInfantryRanger"
            | "GLA_Soldier"
            | "GLA_Rebel"
            | "GLAInfantryRebel"
            | "China_RedGuard"
            | "China_Soldier"
            | "ChinaInfantryRedguard"
            | "TestInfantry"
            | "TestRanger"
    ) || {
        let n = name.to_ascii_lowercase();
        n.contains("ranger")
            || n.contains("rebel")
            || n.contains("redguard")
            || n.contains("infantry")
    }
}

/// Supply-center templates for Supply Lines residual tag.
pub fn is_supply_center_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("supplycenter") || n.contains("supply_center") || name == "AmericaSupplyCenter"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_capture_flashbang_tow_supply() {
        assert_eq!(
            HostUpgradeKind::from_name(UPGRADE_INFANTRY_CAPTURE),
            HostUpgradeKind::CaptureBuilding
        );
        assert_eq!(
            HostUpgradeKind::from_name(UPGRADE_AMERICA_RANGER_CAPTURE),
            HostUpgradeKind::CaptureBuilding
        );
        assert_eq!(
            HostUpgradeKind::from_name(UPGRADE_AMERICA_FLASHBANG),
            HostUpgradeKind::FlashBangGrenade
        );
        assert_eq!(
            HostUpgradeKind::from_name("upgradeamericarangerflashbanggrenade"),
            HostUpgradeKind::FlashBangGrenade
        );
        assert_eq!(
            HostUpgradeKind::from_name(UPGRADE_AMERICA_TOW),
            HostUpgradeKind::TowMissile
        );
        assert_eq!(
            HostUpgradeKind::from_name(UPGRADE_AMERICA_SUPPLY_LINES),
            HostUpgradeKind::SupplyLines
        );
        assert_eq!(
            HostUpgradeKind::from_name("Upgrade_ChinaNationalism"),
            HostUpgradeKind::Other
        );
    }

    #[test]
    fn registry_queue_complete_honesty() {
        let mut reg = HostUpgradeRegistry::new();
        let id = reg.record_queue(
            UPGRADE_AMERICA_FLASHBANG,
            Team::USA,
            0,
            10,
            Some(ObjectId(5)),
        );
        assert!(reg.honesty_queue_ok(HostUpgradeKind::FlashBangGrenade));
        assert!(!reg.honesty_complete_ok(HostUpgradeKind::FlashBangGrenade));
        assert_eq!(reg.get(id).unwrap().phase, HostUpgradePhase::Queued);

        reg.record_complete(UPGRADE_AMERICA_FLASHBANG, 0, 11, 2);
        assert!(!reg.honesty_queue_ok(HostUpgradeKind::FlashBangGrenade));
        assert!(reg.honesty_complete_ok(HostUpgradeKind::FlashBangGrenade));
        assert!(reg.honesty_flashbang_equipped_ok());
        assert!(reg.honesty_host_path_ok(HostUpgradeKind::FlashBangGrenade));
        assert_eq!(reg.get(id).unwrap().units_affected, 2);
        assert_eq!(reg.get(id).unwrap().phase, HostUpgradePhase::Completed);
    }

    #[test]
    fn capture_complete_honesty_without_unit_tags() {
        let mut reg = HostUpgradeRegistry::new();
        reg.record_queue(UPGRADE_INFANTRY_CAPTURE, Team::USA, 0, 1, None);
        reg.record_complete(UPGRADE_INFANTRY_CAPTURE, 0, 2, 0);
        assert!(reg.honesty_capture_unlock_ok());
        assert!(reg.honesty_host_path_ok(HostUpgradeKind::CaptureBuilding));
    }

    #[test]
    fn cancel_clears_pending() {
        let mut reg = HostUpgradeRegistry::new();
        reg.record_queue(UPGRADE_AMERICA_SUPPLY_LINES, Team::USA, 0, 1, None);
        assert!(reg.honesty_queue_ok(HostUpgradeKind::SupplyLines));
        assert!(reg.record_cancel(UPGRADE_AMERICA_SUPPLY_LINES, 0));
        assert!(!reg.honesty_queue_ok(HostUpgradeKind::SupplyLines));
    }
}
