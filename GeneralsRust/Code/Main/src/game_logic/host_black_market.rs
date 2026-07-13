//! Host GLA Black Market residual cash (AutoDepositUpdate).
//!
//! Residual slice (playability):
//! - Constructed `FSBlackMarket` / `*BlackMarket*` buildings deposit cash on a
//!   fixed interval (retail FactionBuilding.ini GLABlackMarket AutoDepositUpdate).
//! - DepositAmount residual **20**, DepositTiming residual **2000 ms → 60 frames**
//!   at 30 FPS logic.
//! - Fake black markets (`*Fake*BlackMarket*`) residual-skip (ActualMoney=No).
//!
//! Fail-closed honesty:
//! - Not full AutoDepositUpdate floating text / InitialCaptureBonus / UpgradedBoost
//! - Oil derrick / hacker residuals live in host_oil_derrick / host_hacker_income
//!   (this module is black-market only)
//! - Not disabled / underpowered / neutral-owner gates beyond is_alive + constructed

use super::ObjectId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Logic frames per second (host fixed step).
pub const BLACK_MARKET_LOGIC_FPS: f32 = 30.0;

/// Retail GLABlackMarket AutoDepositUpdate DepositAmount.
pub const BLACK_MARKET_DEPOSIT_AMOUNT: u32 = 20;

/// Retail DepositTiming = 2000 ms.
pub const BLACK_MARKET_DEPOSIT_TIMING_MS: u32 = 2000;

/// Retail DepositTiming = 2000 ms → frames at 30 FPS (parseDurationUnsignedInt).
pub const BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES: u32 = 60;

/// Audio residual when black market deposits (fail-closed host cue name).
pub const BLACK_MARKET_DEPOSIT_AUDIO: &str = "BlackMarketDeposit";

/// Convert deposit timing milliseconds to logic frames (30 FPS residual).
pub fn deposit_interval_frames_from_ms(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / BLACK_MARKET_LOGIC_FPS)).round() as u32
}

/// True when a template is a residual real black market (not fake/display-only).
pub fn is_black_market_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    if n.contains("fake") {
        return false;
    }
    n.contains("blackmarket") || n.contains("black_market") || n == "testblackmarket"
}

/// Alias for template detection (name residual).
pub fn is_black_market_structure(name: &str) -> bool {
    is_black_market_template(name)
}

/// Whether residual Black Market can award cash this frame.
///
/// Matches C++ AutoDepositUpdate::update gates (subset):
/// alive, construction complete, not neutral-controlled.
pub fn is_legal_black_market_income_source(
    is_alive: bool,
    is_constructed: bool,
    is_neutral: bool,
) -> bool {
    is_alive && is_constructed && !is_neutral
}

/// Host residual honesty + per-market deposit schedule.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostBlackMarketRegistry {
    /// Number of successful residual deposits.
    pub deposits: u32,
    /// Total cash deposited via residual black market path.
    pub cash_total: u32,
    /// Next absolute logic frame each market may deposit.
    next_deposit_frame: HashMap<ObjectId, u32>,
}

impl HostBlackMarketRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn deposits(&self) -> u32 {
        self.deposits
    }

    pub fn cash_total(&self) -> u32 {
        self.cash_total
    }

    /// Ensure market is tracked; returns the next deposit frame for this market.
    /// Matches C++ AutoDepositUpdate ctor: depositOnFrame = now + depositFrame.
    pub fn ensure_scheduled(&mut self, market_id: ObjectId, current_frame: u32) -> u32 {
        *self
            .next_deposit_frame
            .entry(market_id)
            .or_insert_with(|| {
                current_frame.saturating_add(BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES.max(1))
            })
    }

    /// When due, schedule next interval and record a deposit of `amount`.
    /// Returns deposited amount (0 if not yet due).
    pub fn try_deposit(&mut self, market_id: ObjectId, current_frame: u32, amount: u32) -> u32 {
        if amount == 0 {
            return 0;
        }
        let next = self.ensure_scheduled(market_id, current_frame);
        if current_frame < next {
            return 0;
        }
        self.next_deposit_frame.insert(
            market_id,
            current_frame.saturating_add(BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES.max(1)),
        );
        self.deposits = self.deposits.saturating_add(1);
        self.cash_total = self.cash_total.saturating_add(amount);
        amount
    }

    /// Drop schedule when a market is destroyed / gone.
    pub fn forget(&mut self, market_id: ObjectId) {
        self.next_deposit_frame.remove(&market_id);
    }

    /// Snapshot of currently tracked market object ids (for stale cleanup).
    pub fn next_deposit_keys(&self) -> Vec<ObjectId> {
        self.next_deposit_frame.keys().copied().collect()
    }

    /// Residual honesty: at least one deposit completed.
    pub fn honesty_deposit_ok(&self) -> bool {
        self.deposits > 0 && self.cash_total > 0
    }

    /// Combined residual honesty alias (deposit path completed).
    pub fn honesty_ok(&self) -> bool {
        self.honesty_deposit_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_detects_real_black_market() {
        assert!(is_black_market_template("GLABlackMarket"));
        assert!(is_black_market_template("Demo_GLABlackMarket"));
        assert!(is_black_market_template("Slth_GLABlackMarket"));
        assert!(is_black_market_template("TestBlackMarket"));
        assert!(is_black_market_structure("GLABlackMarket"));
        assert!(!is_black_market_template("FakeGLABlackMarket"));
        assert!(!is_black_market_template("Demo_FakeGLABlackMarket"));
        assert!(!is_black_market_template("GLASupplyStash"));
        assert!(!is_black_market_template("AmericaSupplyCenter"));
    }

    #[test]
    fn legal_income_source_matrix() {
        assert!(is_legal_black_market_income_source(true, true, false));
        assert!(!is_legal_black_market_income_source(false, true, false));
        assert!(!is_legal_black_market_income_source(true, false, false));
        assert!(!is_legal_black_market_income_source(true, true, true));
    }

    #[test]
    fn deposit_interval_matches_retail() {
        assert_eq!(BLACK_MARKET_DEPOSIT_AMOUNT, 20);
        assert_eq!(BLACK_MARKET_DEPOSIT_TIMING_MS, 2000);
        assert_eq!(BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES, 60);
        assert_eq!(deposit_interval_frames_from_ms(2000), 60);
        assert_eq!(deposit_interval_frames_from_ms(1000), 30);
        let mut reg = HostBlackMarketRegistry::new();
        let id = ObjectId(1);
        assert_eq!(reg.try_deposit(id, 0, BLACK_MARKET_DEPOSIT_AMOUNT), 0);
        // First schedule is current + interval when first seen at frame 0.
        assert_eq!(reg.try_deposit(id, 60, BLACK_MARKET_DEPOSIT_AMOUNT), 20);
        assert_eq!(reg.try_deposit(id, 60, BLACK_MARKET_DEPOSIT_AMOUNT), 0);
        assert_eq!(reg.try_deposit(id, 120, BLACK_MARKET_DEPOSIT_AMOUNT), 20);
        assert!(reg.honesty_ok());
        assert_eq!(reg.deposits(), 2);
        assert_eq!(reg.cash_total(), 40);
    }
}
