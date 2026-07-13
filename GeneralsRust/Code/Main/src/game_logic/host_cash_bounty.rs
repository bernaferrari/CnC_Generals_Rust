//! Host cash bounty residual (GLA SCIENCE_CashBounty).
//!
//! Residual slice (playability):
//! - Player holds a cash-bounty percent (from science unlock / direct set).
//! - On enemy unit/structure kill, killer player receives
//!   `ceil(victim_build_cost * cash_bounty_percent)` cash.
//! - SCIENCE_CashBounty1/2/3 map to retail residual 5% / 10% / 20%.
//!
//! Matches C++ Player::doBountyForKill + CashBountyPower on science path:
//! - No bounty when percent is 0
//! - No bounty for under-construction victims
//! - No bounty for same-team / non-enemy kills
//!
//! Fail-closed honesty:
//! - Not full CashBountyPower module-on-palace science gate matrix
//! - Not floating text / InGameUI AddCash feedback
//! - Not calcCostToBuild faction handicap matrix (uses template build_cost)

use serde::{Deserialize, Serialize};

/// Retail residual bounty percents from GLA CashBountyPower modules.
/// ChemicalGeneral.ini: Bounty = 5% / 10% / 20%.
pub const CASH_BOUNTY1_PERCENT: f32 = 0.05;
pub const CASH_BOUNTY2_PERCENT: f32 = 0.10;
pub const CASH_BOUNTY3_PERCENT: f32 = 0.20;

/// Science names that unlock cash bounty tiers.
pub const SCIENCE_CASH_BOUNTY1: &str = "SCIENCE_CashBounty1";
pub const SCIENCE_CASH_BOUNTY2: &str = "SCIENCE_CashBounty2";
pub const SCIENCE_CASH_BOUNTY3: &str = "SCIENCE_CashBounty3";

/// Normalize science/upgrade identity (alphanumeric lower).
pub fn normalize_science_identity(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Map a science / ability name to residual cash-bounty percent.
/// Returns `None` when the name is not a cash-bounty science.
pub fn cash_bounty_percent_for_science(name: &str) -> Option<f32> {
    let n = normalize_science_identity(name);
    // Higher tiers first so "cashbounty3" is not matched as tier 1.
    if n.contains("cashbounty3") {
        Some(CASH_BOUNTY3_PERCENT)
    } else if n.contains("cashbounty2") {
        Some(CASH_BOUNTY2_PERCENT)
    } else if n.contains("cashbounty1") || n == "cashbounty" {
        Some(CASH_BOUNTY1_PERCENT)
    } else {
        None
    }
}

/// Compute bounty award: `ceil(cost * percent)` as C++ REAL_TO_INT_CEIL.
/// Returns 0 when percent ≤ 0, cost ≤ 0, or result would be 0.
pub fn compute_bounty_award(build_cost: u32, cash_bounty_percent: f32) -> u32 {
    if build_cost == 0 || cash_bounty_percent <= 0.0 {
        return 0;
    }
    let raw = (build_cost as f32) * cash_bounty_percent;
    // C++ REAL_TO_INT_CEIL — ceil then cast to int (non-negative here).
    let bounty = raw.ceil() as i32;
    if bounty > 0 {
        bounty as u32
    } else {
        0
    }
}

/// Host residual honesty counters for cash bounty awards.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostCashBountyRegistry {
    /// Number of kills that awarded non-zero bounty.
    pub bounty_kills: u32,
    /// Total cash deposited via residual bounty awards.
    pub bounty_earned_total: u32,
    /// Highest cash-bounty percent applied on a player this session.
    pub max_bounty_percent: f32,
}

impl HostCashBountyRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_bounty_set(&mut self, percent: f32) {
        if percent > self.max_bounty_percent {
            self.max_bounty_percent = percent;
        }
    }

    pub fn record_bounty_award(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }
        self.bounty_kills = self.bounty_kills.saturating_add(1);
        self.bounty_earned_total = self.bounty_earned_total.saturating_add(amount);
    }

    /// Residual honesty: at least one bounty award completed.
    pub fn honesty_bounty_award_ok(&self) -> bool {
        self.bounty_kills > 0 && self.bounty_earned_total > 0
    }

    /// Residual honesty: cash bounty percent was configured.
    pub fn honesty_bounty_configured_ok(&self) -> bool {
        self.max_bounty_percent > 0.0
    }

    /// Combined residual honesty (configured + awarded).
    pub fn honesty_ok(&self) -> bool {
        self.honesty_bounty_configured_ok() && self.honesty_bounty_award_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn science_tiers_map_to_retail_percents() {
        assert!((cash_bounty_percent_for_science(SCIENCE_CASH_BOUNTY1).unwrap() - 0.05).abs() < 1e-6);
        assert!((cash_bounty_percent_for_science(SCIENCE_CASH_BOUNTY2).unwrap() - 0.10).abs() < 1e-6);
        assert!((cash_bounty_percent_for_science(SCIENCE_CASH_BOUNTY3).unwrap() - 0.20).abs() < 1e-6);
        assert!((cash_bounty_percent_for_science("cashbounty1").unwrap() - 0.05).abs() < 1e-6);
        assert!(cash_bounty_percent_for_science("SCIENCE_A10").is_none());
    }

    #[test]
    fn compute_bounty_ceil_matches_cpp() {
        // 600 * 0.20 = 120 exactly
        assert_eq!(compute_bounty_award(600, 0.20), 120);
        // 100 * 0.05 = 5
        assert_eq!(compute_bounty_award(100, 0.05), 5);
        // 101 * 0.05 = 5.05 → ceil 6
        assert_eq!(compute_bounty_award(101, 0.05), 6);
        assert_eq!(compute_bounty_award(600, 0.0), 0);
        assert_eq!(compute_bounty_award(0, 0.20), 0);
    }

    #[test]
    fn honesty_tracks_awards() {
        let mut reg = HostCashBountyRegistry::new();
        assert!(!reg.honesty_ok());
        reg.record_bounty_set(0.20);
        assert!(reg.honesty_bounty_configured_ok());
        assert!(!reg.honesty_bounty_award_ok());
        reg.record_bounty_award(120);
        assert!(reg.honesty_ok());
        assert_eq!(reg.bounty_earned_total, 120);
        assert_eq!(reg.bounty_kills, 1);
    }
}
