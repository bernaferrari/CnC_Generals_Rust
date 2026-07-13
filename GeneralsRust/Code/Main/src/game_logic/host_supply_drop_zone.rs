//! Host America Supply Drop Zone residual cash (OCLUpdate).
//!
//! Residual slice (playability):
//! - Constructed `AmericaSupplyDropZone` / `*SupplyDropZone*` buildings credit cash
//!   on a fixed interval matching retail FactionBuilding.ini OCLUpdate delays.
//! - MinDelay/MaxDelay residual **120000 ms → 3600 frames** at 30 FPS logic.
//! - OCL_AmericaSupplyDropZoneCrateDrop residual: **6 × SupplyDropZoneCrate**
//!   at MoneyProvided **250** each → base drop cash **$1500**.
//! - Optional SupplyLines residual: +25 per crate (**$1650** total) when the
//!   controlling player has Upgrade_AmericaSupplyLines.
//!
//! Fail-closed honesty:
//! - Not full cargo-plane DeliverPayload / CreateAtEdge path / parachute crates
//! - Not full OCL ObjectCreationList spawn / MoneyCrateCollide pickup matrix
//! - Not full ControlBar OCL timer UI / sabotage timer-reset path
//! - Under-construction / neutral / dead residual-skip (C++ OCLUpdate::shouldCreate)

use super::ObjectId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Logic frames per second (host fixed step).
pub const SUPPLY_DROP_ZONE_LOGIC_FPS: f32 = 30.0;

/// Retail AmericaSupplyDropZone OCLUpdate MinDelay/MaxDelay (ms).
pub const SUPPLY_DROP_ZONE_DELAY_MS: u32 = 120_000;

/// Retail delay → frames at 30 FPS (parseDurationUnsignedInt).
/// 120000 ms / (1000/30) = 3600 frames.
pub const SUPPLY_DROP_ZONE_INTERVAL_FRAMES: u32 = 3600;

/// Retail OCL_AmericaSupplyDropZoneCrateDrop Payload count.
pub const SUPPLY_DROP_ZONE_CRATE_COUNT: u32 = 6;

/// Retail SupplyDropZoneCrate MoneyCrateCollide MoneyProvided.
pub const SUPPLY_DROP_ZONE_MONEY_PER_CRATE: u32 = 250;

/// Retail SupplyDropZoneCrate UpgradedBoost for Upgrade_AmericaSupplyLines.
pub const SUPPLY_DROP_ZONE_SUPPLY_LINES_BOOST_PER_CRATE: u32 = 25;

/// Residual base cash per drop (6 × 250).
pub const SUPPLY_DROP_ZONE_DROP_CASH: u32 =
    SUPPLY_DROP_ZONE_CRATE_COUNT * SUPPLY_DROP_ZONE_MONEY_PER_CRATE;

/// Residual cash per drop with Supply Lines (6 × (250 + 25)).
pub const SUPPLY_DROP_ZONE_DROP_CASH_WITH_SUPPLY_LINES: u32 = SUPPLY_DROP_ZONE_CRATE_COUNT
    * (SUPPLY_DROP_ZONE_MONEY_PER_CRATE + SUPPLY_DROP_ZONE_SUPPLY_LINES_BOOST_PER_CRATE);

/// Audio residual when supply drop zone credits cash (fail-closed host cue).
pub const SUPPLY_DROP_ZONE_DROP_AUDIO: &str = "SupplyDropZoneDrop";

/// Convert delay milliseconds to logic frames (30 FPS residual).
pub fn drop_interval_frames_from_ms(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / SUPPLY_DROP_ZONE_LOGIC_FPS)).round() as u32
}

/// Cash amount for one residual drop (base or Supply Lines boosted).
pub fn drop_cash_amount(has_supply_lines: bool) -> u32 {
    if has_supply_lines {
        SUPPLY_DROP_ZONE_DROP_CASH_WITH_SUPPLY_LINES
    } else {
        SUPPLY_DROP_ZONE_DROP_CASH
    }
}

/// True when a template is a residual America Supply Drop Zone structure.
pub fn is_supply_drop_zone_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("supplydropzone")
        || n.contains("supply_drop_zone")
        || n.contains("dropzone") && n.contains("supply")
        || n == "testsupplydropzone"
}

/// Alias for template detection (name residual).
pub fn is_supply_drop_zone_structure(name: &str) -> bool {
    is_supply_drop_zone_template(name)
}

/// Whether residual Supply Drop Zone can award cash this frame.
///
/// Matches C++ OCLUpdate::shouldCreate / update gates (subset):
/// alive, construction complete, not neutral-controlled.
pub fn is_legal_supply_drop_zone_income_source(
    is_alive: bool,
    is_constructed: bool,
    is_neutral: bool,
) -> bool {
    is_alive && is_constructed && !is_neutral
}

/// Host residual honesty + per-zone drop schedule.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostSupplyDropZoneRegistry {
    /// Number of successful residual drops.
    pub drops: u32,
    /// Total cash credited via residual supply drop zone path.
    pub cash_total: u32,
    /// Portion of cash_total from SupplyLines crate boost residual.
    pub supply_lines_boost_cash_total: u32,
    /// Next absolute logic frame each zone may drop.
    next_drop_frame: HashMap<ObjectId, u32>,
}

impl HostSupplyDropZoneRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn drops(&self) -> u32 {
        self.drops
    }

    pub fn cash_total(&self) -> u32 {
        self.cash_total
    }

    pub fn supply_lines_boost_cash_total(&self) -> u32 {
        self.supply_lines_boost_cash_total
    }

    /// Ensure zone is tracked; returns the next drop frame for this zone.
    ///
    /// Matches C++ OCLUpdate::update first-try path:
    /// `m_nextCreationFrame == 0` → setNextCreationFrame() without creating,
    /// so the first drop is after one full interval from first observation.
    pub fn ensure_scheduled(&mut self, zone_id: ObjectId, current_frame: u32) -> u32 {
        *self.next_drop_frame.entry(zone_id).or_insert_with(|| {
            current_frame.saturating_add(SUPPLY_DROP_ZONE_INTERVAL_FRAMES.max(1))
        })
    }

    /// When due, schedule next interval and record a drop of `amount`.
    /// Returns deposited amount (0 if not yet due).
    ///
    /// `supply_lines_boost` is the optional Upgrade_AmericaSupplyLines portion
    /// of `amount` (observability only; amount already includes it).
    pub fn try_drop(
        &mut self,
        zone_id: ObjectId,
        current_frame: u32,
        amount: u32,
        supply_lines_boost: u32,
    ) -> u32 {
        if amount == 0 {
            return 0;
        }
        let next = self.ensure_scheduled(zone_id, current_frame);
        if current_frame < next {
            return 0;
        }
        self.next_drop_frame.insert(
            zone_id,
            current_frame.saturating_add(SUPPLY_DROP_ZONE_INTERVAL_FRAMES.max(1)),
        );
        self.drops = self.drops.saturating_add(1);
        self.cash_total = self.cash_total.saturating_add(amount);
        self.supply_lines_boost_cash_total = self
            .supply_lines_boost_cash_total
            .saturating_add(supply_lines_boost.min(amount));
        amount
    }

    /// Drop schedule when a zone is destroyed / gone.
    pub fn forget(&mut self, zone_id: ObjectId) {
        self.next_drop_frame.remove(&zone_id);
    }

    /// Snapshot of currently tracked zone object ids (for stale cleanup).
    pub fn next_drop_keys(&self) -> Vec<ObjectId> {
        self.next_drop_frame.keys().copied().collect()
    }

    /// Residual honesty: at least one drop completed.
    pub fn honesty_drop_ok(&self) -> bool {
        self.drops > 0 && self.cash_total > 0
    }

    /// Residual honesty: Supply Lines crate boost observed at least once.
    pub fn honesty_supply_lines_boost_ok(&self) -> bool {
        self.supply_lines_boost_cash_total > 0
    }

    /// Combined residual honesty alias (drop path completed).
    pub fn honesty_ok(&self) -> bool {
        self.honesty_drop_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_detects_supply_drop_zone() {
        assert!(is_supply_drop_zone_template("AmericaSupplyDropZone"));
        assert!(is_supply_drop_zone_template("AirF_AmericaSupplyDropZone"));
        assert!(is_supply_drop_zone_template("SupW_AmericaSupplyDropZone"));
        assert!(is_supply_drop_zone_template("Lazr_AmericaSupplyDropZone"));
        assert!(is_supply_drop_zone_template("TestSupplyDropZone"));
        assert!(is_supply_drop_zone_structure("AmericaSupplyDropZone"));
        assert!(!is_supply_drop_zone_template("AmericaSupplyCenter"));
        assert!(!is_supply_drop_zone_template("GLABlackMarket"));
        assert!(!is_supply_drop_zone_template("TechOilDerrick"));
        assert!(!is_supply_drop_zone_template("AmericaCommandCenter"));
    }

    #[test]
    fn legal_income_source_matrix() {
        assert!(is_legal_supply_drop_zone_income_source(true, true, false));
        assert!(!is_legal_supply_drop_zone_income_source(false, true, false));
        assert!(!is_legal_supply_drop_zone_income_source(true, false, false));
        assert!(!is_legal_supply_drop_zone_income_source(true, true, true));
    }

    #[test]
    fn drop_interval_and_cash_match_retail() {
        assert_eq!(SUPPLY_DROP_ZONE_DELAY_MS, 120_000);
        assert_eq!(SUPPLY_DROP_ZONE_INTERVAL_FRAMES, 3600);
        assert_eq!(drop_interval_frames_from_ms(120_000), 3600);
        assert_eq!(SUPPLY_DROP_ZONE_CRATE_COUNT, 6);
        assert_eq!(SUPPLY_DROP_ZONE_MONEY_PER_CRATE, 250);
        assert_eq!(SUPPLY_DROP_ZONE_DROP_CASH, 1500);
        assert_eq!(SUPPLY_DROP_ZONE_SUPPLY_LINES_BOOST_PER_CRATE, 25);
        assert_eq!(SUPPLY_DROP_ZONE_DROP_CASH_WITH_SUPPLY_LINES, 1650);
        assert_eq!(drop_cash_amount(false), 1500);
        assert_eq!(drop_cash_amount(true), 1650);

        let mut reg = HostSupplyDropZoneRegistry::new();
        let id = ObjectId(1);
        assert_eq!(
            reg.try_drop(id, 0, SUPPLY_DROP_ZONE_DROP_CASH, 0),
            0,
            "first observation schedules without drop"
        );
        assert_eq!(
            reg.try_drop(id, 3600, SUPPLY_DROP_ZONE_DROP_CASH, 0),
            1500
        );
        assert_eq!(reg.try_drop(id, 3600, SUPPLY_DROP_ZONE_DROP_CASH, 0), 0);
        assert_eq!(
            reg.try_drop(id, 7200, SUPPLY_DROP_ZONE_DROP_CASH, 0),
            1500
        );
        assert!(reg.honesty_ok());
        assert_eq!(reg.drops(), 2);
        assert_eq!(reg.cash_total(), 3000);
    }

    #[test]
    fn supply_lines_boost_tracked() {
        let mut reg = HostSupplyDropZoneRegistry::new();
        let id = ObjectId(2);
        let amount = SUPPLY_DROP_ZONE_DROP_CASH_WITH_SUPPLY_LINES;
        let boost = amount - SUPPLY_DROP_ZONE_DROP_CASH;
        assert_eq!(reg.try_drop(id, 0, amount, boost), 0);
        assert_eq!(reg.try_drop(id, 3600, amount, boost), 1650);
        assert_eq!(reg.supply_lines_boost_cash_total(), 150);
        assert!(reg.honesty_supply_lines_boost_ok());
    }
}
