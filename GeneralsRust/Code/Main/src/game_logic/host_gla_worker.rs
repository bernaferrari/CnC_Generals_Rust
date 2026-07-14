//! Host GLA Worker residual (WorkerShoes upgrade speed + supply boost).
//!
//! Residual slice (playability):
//! - `GLAInfantryWorker` / Chem_/Slth_/GC_* / GLA_Worker / TestWorker:
//!   - Construction / repair / mine-clear already residual via host_repair /
//!     host_mines / Gathering (not re-opened here).
//! - `Upgrade_GLAWorkerShoes` PLAYER_UPGRADE residual:
//!   - LocomotorSetUpgrade → WorkerShoesLocomotor Speed **30** (from FastHuman **25**)
//!   - WorkerAIUpdate UpgradedSupplyBoost **8** cash per drop-off when shoes unlocked
//!
//! Wave 63 residual pack (retail INI honesty):
//! - Shoes residual: FastHuman **25** → WorkerShoes **30**, UpgradedSupplyBoost **8**.
//! - Body residual: MaxHealth **100**, Vision **100**, Shroud **200**, BuildCost **200**,
//!   BuildTime **3**s → **90**f, TransportSlotCount **1**.
//! - Supply residual: MaxBoxes **1**, SupplyCenterActionDelay **150**ms → **5**f,
//!   mine-disarm weapon residual name **WorkerMineDisarmingWeapon**.
//!
//! Fail-closed honesty:
//! - Not full WorkerAIUpdate BoredTime/Range auto-task matrix
//! - Not full SupplyWarehouseActionDelay / SupplyCenterActionDelay timing matrix
//! - Not full fake-building CommandSetUpgrade residual
//! - Not network WorkerShoes / supply-boost replication (network deferred)

use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const WORKER_LOGIC_FPS: f32 = 30.0;

/// Retail upgrade name.
pub const UPGRADE_GLA_WORKER_SHOES: &str = "Upgrade_GLAWorkerShoes";

/// Retail FastHumanLocomotor Speed residual for workers (dist/sec host units).
pub const WORKER_BASE_SPEED: f32 = 25.0;

/// Retail WorkerShoesLocomotor Speed residual (dist/sec host units).
pub const WORKER_SHOES_SPEED: f32 = 30.0;

/// Retail WorkerAIUpdate UpgradedSupplyBoost residual (standard GLA worker = 8).
pub const WORKER_SHOES_SUPPLY_BOOST: u32 = 8;

/// Residual audio when WorkerShoes unlock applies.
pub const WORKER_SHOES_AUDIO: &str = "WorkerVoiceUpgradeShoes";

/// Retail FastHumanLocomotor name residual.
pub const WORKER_BASE_LOCOMOTOR: &str = "FastHumanLocomotor";
/// Retail WorkerShoesLocomotor name residual.
pub const WORKER_SHOES_LOCOMOTOR: &str = "WorkerShoesLocomotor";
/// Retail mine-disarm weapon residual name.
pub const WORKER_MINE_DISARM_WEAPON: &str = "WorkerMineDisarmingWeapon";

// --- Body residual (GLAInfantryWorker) ---

/// Retail MaxHealth residual.
pub const WORKER_MAX_HEALTH: f32 = 100.0;
/// Retail VisionRange residual.
pub const WORKER_VISION_RANGE: f32 = 100.0;
/// Retail ShroudClearingRange residual.
pub const WORKER_SHROUD_CLEARING_RANGE: f32 = 200.0;
/// Retail BuildCost residual.
pub const WORKER_BUILD_COST: u32 = 200;
/// Retail BuildTime residual (seconds).
pub const WORKER_BUILD_TIME_SEC: f32 = 3.0;
/// Retail BuildTime → frames @ 30 FPS.
pub const WORKER_BUILD_TIME_FRAMES: u32 = 90;
/// Retail TransportSlotCount residual.
pub const WORKER_TRANSPORT_SLOT_COUNT: u32 = 1;
/// Retail WorkerAIUpdate MaxBoxes residual.
pub const WORKER_MAX_BOXES: u32 = 1;
/// Retail SupplyCenterActionDelay residual (msec).
pub const WORKER_SUPPLY_CENTER_ACTION_DELAY_MS: u32 = 150;
/// Retail SupplyCenterActionDelay → frames @ 30 FPS.
pub const WORKER_SUPPLY_CENTER_ACTION_DELAY_FRAMES: u32 = 5;

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn worker_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * WORKER_LOGIC_FPS / 1000.0).round() as u32
}

/// Whether template is a residual GLA Worker.
///
/// Fail-closed: name residual. Excludes weapons / science / rebel / dozer USA.
pub fn is_gla_worker_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("voice")
        || n.contains("commandset")
        || n.contains("rebel")
        || n.contains("hijacker")
        || n.contains("terrorist")
        || n.contains("saboteur")
        || n.contains("dozer")
        || n.contains("china")
        || n.contains("america")
        || n.contains("usa")
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testworker" || n == "gla_worker" || n == "glainfantryworker" {
        return true;
    }
    // GLAInfantryWorker / Chem_GLAInfantryWorker / Slth_ / GC_Chem_ / GC_Slth_
    n.contains("infantryworker") || (n.contains("worker") && n.contains("gla"))
}

/// Residual cash added on supply-center deposit when WorkerShoes is active for
/// a residual GLA worker carrier.
///
/// Fail-closed: 0 when shoes not unlocked or unit is not a worker.
pub fn residual_worker_shoes_drop_off_boost(is_worker: bool, has_worker_shoes: bool) -> u32 {
    if is_worker && has_worker_shoes {
        WORKER_SHOES_SUPPLY_BOOST
    } else {
        0
    }
}

/// Residual speed to apply when WorkerShoes unlocks (or base without shoes).
pub fn worker_residual_speed(has_worker_shoes: bool) -> f32 {
    if has_worker_shoes {
        WORKER_SHOES_SPEED
    } else {
        WORKER_BASE_SPEED
    }
}

/// Whether WorkerShoes upgrade identity matches residual kind.
pub fn is_worker_shoes_upgrade_name(name: &str) -> bool {
    let n = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect::<String>();
    n.contains("workershoes") || n.contains("glaworkershoes")
}

/// Host residual honesty counters for GLA Worker residual.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostGlaWorkerRegistry {
    /// Workers that received WorkerShoes speed residual.
    pub shoes_units_affected: u32,
    /// Total extra cash from WorkerShoes supply boost on drop-off.
    pub shoes_bonus_cash_total: u32,
    /// Number of drop-offs that applied WorkerShoes boost.
    pub shoes_boost_drop_offs: u32,
}

impl HostGlaWorkerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_shoes_applied(&mut self, units: u32) {
        self.shoes_units_affected = self.shoes_units_affected.saturating_add(units);
    }

    pub fn record_shoes_drop_off_boost(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }
        self.shoes_boost_drop_offs = self.shoes_boost_drop_offs.saturating_add(1);
        self.shoes_bonus_cash_total = self.shoes_bonus_cash_total.saturating_add(amount);
    }

    /// Residual honesty: shoes upgrade affected at least one worker.
    pub fn honesty_shoes_apply_ok(&self) -> bool {
        self.shoes_units_affected > 0
    }

    /// Residual honesty: shoes supply boost observed on drop-off.
    pub fn honesty_shoes_boost_ok(&self) -> bool {
        self.shoes_boost_drop_offs > 0 && self.shoes_bonus_cash_total > 0
    }

    /// Combined worker residual honesty.
    pub fn honesty_worker_ok(&self) -> bool {
        self.honesty_shoes_apply_ok() || self.honesty_shoes_boost_ok()
    }
}

// --- Wave 63 residual honesty packs ---

/// Wave 63 residual honesty: WorkerShoes speed + supply boost residual.
pub fn honesty_worker_shoes_residual_ok() -> bool {
    UPGRADE_GLA_WORKER_SHOES == "Upgrade_GLAWorkerShoes"
        && (WORKER_BASE_SPEED - 25.0).abs() < 0.01
        && (WORKER_SHOES_SPEED - 30.0).abs() < 0.01
        && WORKER_SHOES_SUPPLY_BOOST == 8
        && WORKER_SHOES_AUDIO == "WorkerVoiceUpgradeShoes"
        && WORKER_BASE_LOCOMOTOR == "FastHumanLocomotor"
        && WORKER_SHOES_LOCOMOTOR == "WorkerShoesLocomotor"
        && (worker_residual_speed(false) - 25.0).abs() < 0.01
        && (worker_residual_speed(true) - 30.0).abs() < 0.01
        && residual_worker_shoes_drop_off_boost(true, true) == 8
        && residual_worker_shoes_drop_off_boost(true, false) == 0
        && is_worker_shoes_upgrade_name("Upgrade_GLAWorkerShoes")
}

/// Wave 63 residual honesty: worker body + supply timing residual.
pub fn honesty_worker_body_supply_residual_ok() -> bool {
    (WORKER_MAX_HEALTH - 100.0).abs() < 0.01
        && (WORKER_VISION_RANGE - 100.0).abs() < 0.01
        && (WORKER_SHROUD_CLEARING_RANGE - 200.0).abs() < 0.01
        && WORKER_BUILD_COST == 200
        && (WORKER_BUILD_TIME_SEC - 3.0).abs() < 0.01
        && WORKER_BUILD_TIME_FRAMES == ((WORKER_BUILD_TIME_SEC * WORKER_LOGIC_FPS).round() as u32)
        && WORKER_BUILD_TIME_FRAMES == 90
        && WORKER_TRANSPORT_SLOT_COUNT == 1
        && WORKER_MAX_BOXES == 1
        && WORKER_SUPPLY_CENTER_ACTION_DELAY_MS == 150
        && WORKER_SUPPLY_CENTER_ACTION_DELAY_FRAMES
            == worker_ms_to_frames(WORKER_SUPPLY_CENTER_ACTION_DELAY_MS)
        && WORKER_SUPPLY_CENTER_ACTION_DELAY_FRAMES == 5
        && WORKER_MINE_DISARM_WEAPON == "WorkerMineDisarmingWeapon"
}

/// Combined Wave 63 GLA Worker residual honesty pack.
pub fn honesty_gla_worker_residual_pack_ok() -> bool {
    honesty_worker_shoes_residual_ok() && honesty_worker_body_supply_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_name_matrix() {
        assert!(is_gla_worker_template("GLAInfantryWorker"));
        assert!(is_gla_worker_template("Chem_GLAInfantryWorker"));
        assert!(is_gla_worker_template("Slth_GLAInfantryWorker"));
        assert!(is_gla_worker_template("GC_Chem_GLAInfantryWorker"));
        assert!(is_gla_worker_template("GLA_Worker"));
        assert!(is_gla_worker_template("TestWorker"));
        assert!(!is_gla_worker_template("GLAInfantryRebel"));
        assert!(!is_gla_worker_template("GLAInfantryHijacker"));
        assert!(!is_gla_worker_template("AmericaDozer"));
        assert!(!is_gla_worker_template("Upgrade_GLAWorkerShoes"));
        assert!(!is_gla_worker_template("ChinaInfantryRedguard"));
    }

    #[test]
    fn shoes_boost_and_speed() {
        assert_eq!(residual_worker_shoes_drop_off_boost(true, true), 8);
        assert_eq!(residual_worker_shoes_drop_off_boost(true, false), 0);
        assert_eq!(residual_worker_shoes_drop_off_boost(false, true), 0);
        assert!((worker_residual_speed(false) - 25.0).abs() < 0.01);
        assert!((worker_residual_speed(true) - 30.0).abs() < 0.01);
        assert!(is_worker_shoes_upgrade_name("Upgrade_GLAWorkerShoes"));
        assert!(is_worker_shoes_upgrade_name("upgrade_gla_worker_shoes"));
        assert!(!is_worker_shoes_upgrade_name("Upgrade_GLACamouflage"));
    }

    #[test]
    fn honesty_flags() {
        let mut reg = HostGlaWorkerRegistry::new();
        assert!(!reg.honesty_worker_ok());
        reg.record_shoes_applied(2);
        assert!(reg.honesty_shoes_apply_ok());
        assert!(reg.honesty_worker_ok());
        reg.record_shoes_drop_off_boost(8);
        assert!(reg.honesty_shoes_boost_ok());
        assert_eq!(reg.shoes_bonus_cash_total, 8);
    }

    #[test]
    fn gla_worker_residual_pack_honesty_wave63() {
        assert!(honesty_worker_shoes_residual_ok());
        assert!(honesty_worker_body_supply_residual_ok());
        assert!(honesty_gla_worker_residual_pack_ok());
        assert_eq!(worker_ms_to_frames(150), 5);
        assert_eq!(worker_ms_to_frames(0), 0);
        assert_eq!(WORKER_BUILD_TIME_FRAMES, 90);
        assert_eq!(WORKER_MAX_BOXES, 1);
        assert_eq!(WORKER_MINE_DISARM_WEAPON, "WorkerMineDisarmingWeapon");
    }
}
