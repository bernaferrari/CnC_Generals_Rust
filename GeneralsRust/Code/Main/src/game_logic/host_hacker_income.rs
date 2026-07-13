//! Host China Hacker / Internet Center residual cash (HackInternetAIUpdate).
//!
//! Residual slice (playability):
//! - Living `*Hacker*` units generate cash while residual-hacking.
//! - Internet Center residual: hackers contained in `FSInternetCenter` /
//!   `*InternetCenter*` auto-start hacking and use CashUpdateDelayFast.
//! - Field residual: explicit `start_hacking` (HackInternet command residual).
//! - RegularCashAmount residual **5**, CashUpdateDelay **2000 ms → 60 frames**,
//!   CashUpdateDelayFast **1800 ms → 54 frames** (inside Internet Center).
//! - Veterancy residual: Regular/Veteran/Elite/Heroic = 5/6/8/10.
//!
//! Fail-closed honesty:
//! - Not full Unpack/Pack state machine / variation factor / model conditions
//! - Not full floating text / stealth display gates
//! - Not full DISABLED_HACKED microwave interrupt resume matrix beyond skip-while-disabled
//! - XpPerCashUpdate residual applied as +1 XP when experience tracker present

use super::{ObjectId, VeterancyLevel};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Logic frames per second (host fixed step).
pub const HACKER_LOGIC_FPS: f32 = 30.0;

/// Retail CashUpdateDelay = 2000 ms (field).
pub const HACKER_CASH_UPDATE_DELAY_MS: u32 = 2000;

/// Retail CashUpdateDelayFast = 1800 ms (inside Internet Center).
pub const HACKER_CASH_UPDATE_DELAY_FAST_MS: u32 = 1800;

/// Field cash interval frames (parseDurationUnsignedInt @ 30 FPS).
pub const HACKER_CASH_INTERVAL_FRAMES: u32 = 60;

/// Internet Center cash interval frames.
pub const HACKER_CASH_INTERVAL_FAST_FRAMES: u32 = 54;

/// Retail RegularCashAmount.
pub const HACKER_CASH_REGULAR: u32 = 5;
/// Retail VeteranCashAmount.
pub const HACKER_CASH_VETERAN: u32 = 6;
/// Retail EliteCashAmount.
pub const HACKER_CASH_ELITE: u32 = 8;
/// Retail HeroicCashAmount.
pub const HACKER_CASH_HEROIC: u32 = 10;

/// Retail XpPerCashUpdate.
pub const HACKER_XP_PER_CASH_UPDATE: f32 = 1.0;

/// Audio residual when hacker deposits (UnitCashPing residual cue).
pub const HACKER_CASH_PING_AUDIO: &str = "HackerCashPing";

/// Convert ms duration to logic frames (30 FPS residual).
pub fn cash_interval_frames_from_ms(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / HACKER_LOGIC_FPS)).round() as u32
}

/// True when a template is a residual China Hacker infantry unit.
pub fn is_hacker_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    // Exclude BlackLotus cash-hack hero and non-hacker names.
    if n.contains("blacklotus") || n.contains("black_lotus") {
        return false;
    }
    n.contains("hacker") || n == "testhacker"
}

/// True when a template / kind is residual Internet Center structure.
pub fn is_internet_center_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("internetcenter") || n.contains("internet_center") || n == "testinternetcenter"
}

/// Cash amount residual by veterancy level (C++ HackInternetState fall-through).
pub fn cash_amount_for_level(level: VeterancyLevel) -> u32 {
    match level {
        VeterancyLevel::Heroic => HACKER_CASH_HEROIC,
        VeterancyLevel::Elite => HACKER_CASH_ELITE,
        VeterancyLevel::Veteran => HACKER_CASH_VETERAN,
        VeterancyLevel::Rookie => HACKER_CASH_REGULAR,
    }
}

/// Interval frames: fast when contained in Internet Center.
pub fn cash_interval_frames(in_internet_center: bool) -> u32 {
    if in_internet_center {
        HACKER_CASH_INTERVAL_FAST_FRAMES
    } else {
        HACKER_CASH_INTERVAL_FRAMES
    }
}

/// Whether residual Hacker can award cash this frame.
///
/// C++ HackInternetState: skip while DISABLED_HACKED; must be alive / non-neutral.
pub fn is_legal_hacker_income_source(
    is_alive: bool,
    is_neutral: bool,
    is_disabled_hacked: bool,
) -> bool {
    is_alive && !is_neutral && !is_disabled_hacked
}

/// Host residual honesty + active hacking schedule.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostHackerIncomeRegistry {
    /// Number of successful residual cash pings.
    pub deposits: u32,
    /// Total cash deposited via residual hacker path.
    pub cash_total: u32,
    /// Deposits that used Internet Center fast interval.
    pub internet_center_deposits: u32,
    /// Explicit field start_hacking activations.
    pub field_starts: u32,
    /// Auto-starts when entering / contained in Internet Center.
    pub internet_center_auto_starts: u32,
    /// Hackers currently residual-hacking (field command or IC).
    active_hackers: HashSet<ObjectId>,
    /// Next absolute logic frame each hacker may deposit.
    next_deposit_frame: HashMap<ObjectId, u32>,
}

impl HostHackerIncomeRegistry {
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

    pub fn internet_center_deposits(&self) -> u32 {
        self.internet_center_deposits
    }

    pub fn is_hacking(&self, hacker_id: ObjectId) -> bool {
        self.active_hackers.contains(&hacker_id)
    }

    /// Explicit field HackInternet residual start.
    /// Schedules first cash after field interval (C++ HackInternetState::onEnter).
    pub fn start_hacking(&mut self, hacker_id: ObjectId, current_frame: u32) {
        self.active_hackers.insert(hacker_id);
        self.field_starts = self.field_starts.saturating_add(1);
        self.next_deposit_frame.insert(
            hacker_id,
            current_frame.saturating_add(HACKER_CASH_INTERVAL_FRAMES.max(1)),
        );
    }

    /// Auto-start when contained in Internet Center (InternetHackContain residual).
    /// Returns true when newly started.
    pub fn ensure_internet_center_hacking(
        &mut self,
        hacker_id: ObjectId,
        current_frame: u32,
    ) -> bool {
        if self.active_hackers.contains(&hacker_id) {
            return false;
        }
        self.active_hackers.insert(hacker_id);
        self.internet_center_auto_starts = self.internet_center_auto_starts.saturating_add(1);
        // Inside IC: no pack/unpack residual; first cash after fast delay.
        self.next_deposit_frame.insert(
            hacker_id,
            current_frame.saturating_add(HACKER_CASH_INTERVAL_FAST_FRAMES.max(1)),
        );
        true
    }

    /// Stop residual hacking (move order / death residual).
    pub fn stop_hacking(&mut self, hacker_id: ObjectId) {
        self.active_hackers.remove(&hacker_id);
        self.next_deposit_frame.remove(&hacker_id);
    }

    /// When due, deposit and reschedule with the given interval.
    /// Returns deposited amount (0 if not hacking / not due / amount 0).
    pub fn try_deposit(
        &mut self,
        hacker_id: ObjectId,
        current_frame: u32,
        amount: u32,
        interval_frames: u32,
        in_internet_center: bool,
    ) -> u32 {
        if amount == 0 || !self.active_hackers.contains(&hacker_id) {
            return 0;
        }
        let next = *self.next_deposit_frame.entry(hacker_id).or_insert_with(|| {
            current_frame.saturating_add(interval_frames.max(1))
        });
        if current_frame < next {
            return 0;
        }
        self.next_deposit_frame.insert(
            hacker_id,
            current_frame.saturating_add(interval_frames.max(1)),
        );
        self.deposits = self.deposits.saturating_add(1);
        self.cash_total = self.cash_total.saturating_add(amount);
        if in_internet_center {
            self.internet_center_deposits = self.internet_center_deposits.saturating_add(1);
        }
        amount
    }

    /// Drop state when hacker is destroyed / gone.
    pub fn forget(&mut self, hacker_id: ObjectId) {
        self.active_hackers.remove(&hacker_id);
        self.next_deposit_frame.remove(&hacker_id);
    }

    /// Snapshot of tracked / active hacker ids (for stale cleanup).
    pub fn tracked_keys(&self) -> Vec<ObjectId> {
        self.active_hackers
            .iter()
            .chain(self.next_deposit_frame.keys())
            .copied()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    }

    /// Residual honesty: at least one cash deposit completed.
    pub fn honesty_deposit_ok(&self) -> bool {
        self.deposits > 0 && self.cash_total > 0
    }

    /// Residual honesty: at least one Internet Center deposit.
    pub fn honesty_internet_center_ok(&self) -> bool {
        self.internet_center_deposits > 0
    }

    /// Combined residual honesty.
    pub fn honesty_ok(&self) -> bool {
        self.honesty_deposit_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_detects_hacker_and_internet_center() {
        assert!(is_hacker_template("ChinaInfantryHacker"));
        assert!(is_hacker_template("Tank_ChinaInfantryHacker"));
        assert!(is_hacker_template("Nuke_ChinaInfantryHacker"));
        assert!(is_hacker_template("TestHacker"));
        assert!(!is_hacker_template("ChinaInfantryBlackLotus"));
        assert!(!is_hacker_template("ChinaTankBattleMaster"));
        assert!(is_internet_center_template("ChinaInternetCenter"));
        assert!(is_internet_center_template("Tank_ChinaInternetCenter"));
        assert!(is_internet_center_template("TestInternetCenter"));
        assert!(!is_internet_center_template("ChinaPropagandaCenter"));
    }

    #[test]
    fn cash_amounts_match_retail_by_level() {
        assert_eq!(cash_amount_for_level(VeterancyLevel::Rookie), 5);
        assert_eq!(cash_amount_for_level(VeterancyLevel::Veteran), 6);
        assert_eq!(cash_amount_for_level(VeterancyLevel::Elite), 8);
        assert_eq!(cash_amount_for_level(VeterancyLevel::Heroic), 10);
        assert_eq!(HACKER_CASH_INTERVAL_FRAMES, 60);
        assert_eq!(HACKER_CASH_INTERVAL_FAST_FRAMES, 54);
        assert_eq!(cash_interval_frames_from_ms(2000), 60);
        assert_eq!(cash_interval_frames_from_ms(1800), 54);
        assert_eq!(cash_interval_frames(true), 54);
        assert_eq!(cash_interval_frames(false), 60);
    }

    #[test]
    fn legal_income_source_matrix() {
        assert!(is_legal_hacker_income_source(true, false, false));
        assert!(!is_legal_hacker_income_source(false, false, false));
        assert!(!is_legal_hacker_income_source(true, true, false));
        assert!(!is_legal_hacker_income_source(true, false, true));
    }

    #[test]
    fn field_hacking_deposits_on_interval() {
        let mut reg = HostHackerIncomeRegistry::new();
        let id = ObjectId(1);
        reg.start_hacking(id, 0);
        assert!(reg.is_hacking(id));
        assert_eq!(
            reg.try_deposit(id, 0, HACKER_CASH_REGULAR, HACKER_CASH_INTERVAL_FRAMES, false),
            0
        );
        assert_eq!(
            reg.try_deposit(id, 60, HACKER_CASH_REGULAR, HACKER_CASH_INTERVAL_FRAMES, false),
            5
        );
        assert_eq!(
            reg.try_deposit(id, 120, HACKER_CASH_REGULAR, HACKER_CASH_INTERVAL_FRAMES, false),
            5
        );
        assert!(reg.honesty_deposit_ok());
        assert_eq!(reg.deposits(), 2);
        assert_eq!(reg.cash_total(), 10);
        assert_eq!(reg.internet_center_deposits(), 0);
    }

    #[test]
    fn internet_center_auto_start_uses_fast_interval() {
        let mut reg = HostHackerIncomeRegistry::new();
        let id = ObjectId(2);
        assert!(reg.ensure_internet_center_hacking(id, 0));
        assert!(!reg.ensure_internet_center_hacking(id, 0)); // already active
        assert_eq!(
            reg.try_deposit(id, 54, HACKER_CASH_REGULAR, HACKER_CASH_INTERVAL_FAST_FRAMES, true),
            5
        );
        assert!(reg.honesty_internet_center_ok());
        assert_eq!(reg.internet_center_deposits(), 1);
    }
}
