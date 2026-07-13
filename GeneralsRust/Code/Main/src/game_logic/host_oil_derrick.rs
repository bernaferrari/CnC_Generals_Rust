//! Host Tech Oil Derrick residual income (AutoDepositUpdate).
//!
//! Residual slice (playability):
//! - Captured `TechOilDerrick` / `*OilDerrick*` deposits cash on a fixed interval
//!   (retail CivilianBuilding.ini TechOilDerrick AutoDepositUpdate).
//! - DepositAmount residual **200**, DepositTiming residual **12000 ms → 360 frames**
//!   at 30 FPS logic.
//! - InitialCaptureBonus residual **1000** once when a neutral derrick first becomes
//!   non-neutral owned (Player::gainObject → awardInitialCaptureBonus residual).
//!
//! Fail-closed honesty:
//! - Not full AutoDepositUpdate floating text / UpgradedBoost (SupplyLines +20)
//! - Not full capture flow module wiring beyond residual team-change detect
//! - Neutral / under-construction residual-skip (C++ isNeutralControlled +
//!   construction-complete gates)

use super::ObjectId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Logic frames per second (host fixed step).
pub const OIL_DERRICK_LOGIC_FPS: f32 = 30.0;

/// Retail TechOilDerrick AutoDepositUpdate DepositAmount.
pub const OIL_DERRICK_DEPOSIT_AMOUNT: u32 = 200;

/// Retail DepositTiming = 12000 ms.
pub const OIL_DERRICK_DEPOSIT_TIMING_MS: u32 = 12000;

/// Retail DepositTiming = 12000 ms → frames at 30 FPS (parseDurationUnsignedInt).
pub const OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES: u32 = 360;

/// Retail InitialCaptureBonus.
pub const OIL_DERRICK_INITIAL_CAPTURE_BONUS: u32 = 1000;

/// Audio residual when oil derrick deposits (fail-closed host cue name).
pub const OIL_DERRICK_DEPOSIT_AUDIO: &str = "OilDerrickDeposit";

/// Audio residual when capture bonus is awarded.
pub const OIL_DERRICK_CAPTURE_BONUS_AUDIO: &str = "OilDerrickCaptureBonus";

/// Convert deposit timing milliseconds to logic frames (30 FPS residual).
pub fn deposit_interval_frames_from_ms(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / OIL_DERRICK_LOGIC_FPS)).round() as u32
}

/// True when a template is a residual oil derrick tech building.
pub fn is_oil_derrick_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("oilderrick") || n.contains("oil_derrick") || n == "testoilderrick"
}

/// Alias for template detection (name residual).
pub fn is_oil_derrick_structure(name: &str) -> bool {
    is_oil_derrick_template(name)
}

/// Whether residual Oil Derrick can award cash this frame.
///
/// Matches C++ AutoDepositUpdate::update gates (subset):
/// alive, construction complete, not neutral-controlled.
pub fn is_legal_oil_derrick_income_source(
    is_alive: bool,
    is_constructed: bool,
    is_neutral: bool,
) -> bool {
    is_alive && is_constructed && !is_neutral
}

/// Host residual honesty + per-derrick deposit schedule + capture bonus tracking.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostOilDerrickRegistry {
    /// Number of successful residual periodic deposits.
    pub deposits: u32,
    /// Total cash from periodic AutoDeposit residual.
    pub cash_total: u32,
    /// Number of residual capture bonuses awarded.
    pub capture_bonuses: u32,
    /// Total cash from InitialCaptureBonus residual.
    pub capture_bonus_cash_total: u32,
    /// Next absolute logic frame each derrick may deposit.
    next_deposit_frame: HashMap<ObjectId, u32>,
    /// Derricks that have already received InitialCaptureBonus this instance life.
    capture_bonus_awarded: HashSet<ObjectId>,
    /// Last known non-neutral owner team (for re-capture residual; bonus once only).
    last_owner_was_non_neutral: HashSet<ObjectId>,
}

impl HostOilDerrickRegistry {
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

    pub fn capture_bonuses(&self) -> u32 {
        self.capture_bonuses
    }

    pub fn capture_bonus_cash_total(&self) -> u32 {
        self.capture_bonus_cash_total
    }

    /// Combined residual cash (periodic + capture bonus).
    pub fn total_cash_awarded(&self) -> u32 {
        self.cash_total.saturating_add(self.capture_bonus_cash_total)
    }

    /// Ensure derrick is tracked; returns the next deposit frame.
    /// Matches C++ AutoDepositUpdate ctor: depositOnFrame = now + depositFrame.
    pub fn ensure_scheduled(&mut self, derrick_id: ObjectId, current_frame: u32) -> u32 {
        *self.next_deposit_frame.entry(derrick_id).or_insert_with(|| {
            current_frame.saturating_add(OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES.max(1))
        })
    }

    /// When due, schedule next interval and record a deposit of `amount`.
    /// Returns deposited amount (0 if not yet due).
    pub fn try_deposit(&mut self, derrick_id: ObjectId, current_frame: u32, amount: u32) -> u32 {
        if amount == 0 {
            return 0;
        }
        let next = self.ensure_scheduled(derrick_id, current_frame);
        if current_frame < next {
            return 0;
        }
        self.next_deposit_frame.insert(
            derrick_id,
            current_frame.saturating_add(OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES.max(1)),
        );
        self.deposits = self.deposits.saturating_add(1);
        self.cash_total = self.cash_total.saturating_add(amount);
        amount
    }

    /// Award InitialCaptureBonus once when derrick first becomes non-neutral.
    /// Returns bonus amount awarded (0 if already awarded / not eligible).
    ///
    /// Residual of Player::gainObject → AutoDepositUpdate::awardInitialCaptureBonus.
    /// Fail-closed: once per derrick instance life (not every re-capture).
    pub fn try_capture_bonus(&mut self, derrick_id: ObjectId, amount: u32) -> u32 {
        if amount == 0 {
            return 0;
        }
        if self.capture_bonus_awarded.contains(&derrick_id) {
            // Still mark as non-neutral owned so future neutral→owner edges stay quiet.
            self.last_owner_was_non_neutral.insert(derrick_id);
            return 0;
        }
        // First time this instance is non-neutral controlled.
        self.capture_bonus_awarded.insert(derrick_id);
        self.last_owner_was_non_neutral.insert(derrick_id);
        self.capture_bonuses = self.capture_bonuses.saturating_add(1);
        self.capture_bonus_cash_total = self.capture_bonus_cash_total.saturating_add(amount);
        // C++ awardInitialCaptureBonus also resets deposit timer to now + interval.
        // Caller should treat this as a schedule reset when amount > 0.
        amount
    }

    /// Reset deposit schedule after capture bonus (C++ awardInitialCaptureBonus).
    pub fn reschedule_after_capture(&mut self, derrick_id: ObjectId, current_frame: u32) {
        self.next_deposit_frame.insert(
            derrick_id,
            current_frame.saturating_add(OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES.max(1)),
        );
    }

    /// Drop schedule when a derrick is destroyed / gone.
    pub fn forget(&mut self, derrick_id: ObjectId) {
        self.next_deposit_frame.remove(&derrick_id);
        self.capture_bonus_awarded.remove(&derrick_id);
        self.last_owner_was_non_neutral.remove(&derrick_id);
    }

    /// Snapshot of currently tracked derrick object ids (for stale cleanup).
    pub fn next_deposit_keys(&self) -> Vec<ObjectId> {
        self.next_deposit_frame.keys().copied().collect()
    }

    /// Residual honesty: at least one periodic deposit completed.
    pub fn honesty_deposit_ok(&self) -> bool {
        self.deposits > 0 && self.cash_total > 0
    }

    /// Residual honesty: at least one capture bonus awarded.
    pub fn honesty_capture_bonus_ok(&self) -> bool {
        self.capture_bonuses > 0 && self.capture_bonus_cash_total > 0
    }

    /// Combined residual honesty (any cash path completed).
    pub fn honesty_ok(&self) -> bool {
        self.honesty_deposit_ok() || self.honesty_capture_bonus_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_detects_oil_derrick() {
        assert!(is_oil_derrick_template("TechOilDerrick"));
        assert!(is_oil_derrick_template("OilDerrick"));
        assert!(is_oil_derrick_template("TestOilDerrick"));
        assert!(is_oil_derrick_structure("TechOilDerrick"));
        assert!(!is_oil_derrick_template("TechOilRefinery"));
        assert!(!is_oil_derrick_template("GLABlackMarket"));
        assert!(!is_oil_derrick_template("AmericaSupplyCenter"));
    }

    #[test]
    fn legal_income_source_matrix() {
        assert!(is_legal_oil_derrick_income_source(true, true, false));
        assert!(!is_legal_oil_derrick_income_source(false, true, false));
        assert!(!is_legal_oil_derrick_income_source(true, false, false));
        assert!(!is_legal_oil_derrick_income_source(true, true, true));
    }

    #[test]
    fn deposit_interval_matches_retail() {
        assert_eq!(OIL_DERRICK_DEPOSIT_AMOUNT, 200);
        assert_eq!(OIL_DERRICK_DEPOSIT_TIMING_MS, 12000);
        assert_eq!(OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES, 360);
        assert_eq!(deposit_interval_frames_from_ms(12000), 360);
        assert_eq!(OIL_DERRICK_INITIAL_CAPTURE_BONUS, 1000);

        let mut reg = HostOilDerrickRegistry::new();
        let id = ObjectId(1);
        assert_eq!(reg.try_deposit(id, 0, OIL_DERRICK_DEPOSIT_AMOUNT), 0);
        assert_eq!(reg.try_deposit(id, 360, OIL_DERRICK_DEPOSIT_AMOUNT), 200);
        assert_eq!(reg.try_deposit(id, 360, OIL_DERRICK_DEPOSIT_AMOUNT), 0);
        assert_eq!(reg.try_deposit(id, 720, OIL_DERRICK_DEPOSIT_AMOUNT), 200);
        assert!(reg.honesty_deposit_ok());
        assert_eq!(reg.deposits(), 2);
        assert_eq!(reg.cash_total(), 400);
    }

    #[test]
    fn capture_bonus_once_per_instance() {
        let mut reg = HostOilDerrickRegistry::new();
        let id = ObjectId(7);
        assert_eq!(
            reg.try_capture_bonus(id, OIL_DERRICK_INITIAL_CAPTURE_BONUS),
            1000
        );
        assert_eq!(
            reg.try_capture_bonus(id, OIL_DERRICK_INITIAL_CAPTURE_BONUS),
            0,
            "capture bonus once only"
        );
        assert!(reg.honesty_capture_bonus_ok());
        assert_eq!(reg.capture_bonuses(), 1);
        assert_eq!(reg.capture_bonus_cash_total(), 1000);
        assert_eq!(reg.total_cash_awarded(), 1000);
    }
}
