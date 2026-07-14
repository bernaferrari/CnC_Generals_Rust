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
//! Residual floating cash text (Player::doBountyForKill):
//! - Host `+$N` at killer pos + Z **10**, yellow RGBA (255,255,0,255), key `GUI:AddCash`.
//! - Killer ObjectId residual prefers victim `last_damage_source` (BodyModule residual)
//!   when available; falls back to nearest living same-team unit.
//!
//! Wave 66 residual pack (retail SpecialPower.ini / Science.ini / FactionBuilding.ini):
//! - SpecialAbilityCashBounty1/2/3 Enum SPECIAL_CASH_BOUNTY + RequiredScience tiers.
//! - CashBountyPower Bounty residual **5%** / **10%** / **20%**.
//! - SciencePurchasePointCost **1** each; prereq SCIENCE_GLA + Rank3, then chain
//!   CashBounty1→2→3.
//! - Floating text residual: GUI:AddCash, Z lift **10**, yellow RGBA.
//!
//! Fail-closed honesty:
//! - Not full CashBountyPower module-on-palace science gate matrix
//! - Not full InGameUI GPU draw / Unicode GameText localization
//! - Not calcCostToBuild faction handicap matrix (uses template build_cost)
//! - Network deferred

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// C++ doBountyForKill floating text Z lift (pos.z += 10.0f). Host Y-up → Y + 10.
pub const CASH_BOUNTY_FLOATING_TEXT_Z_OFFSET: f32 = 10.0;

/// Residual GameText key honesty for bounty cash caption.
pub const CASH_BOUNTY_FLOATING_TEXT_ADD_CASH_KEY: &str = "GUI:AddCash";

/// Residual floating cash text color (yellow, retail GameMakeColor(255,255,0,255)).
pub const CASH_BOUNTY_FLOATING_TEXT_COLOR_RGBA: (u8, u8, u8, u8) = (255, 255, 0, 255);

/// Retail residual bounty percents from GLA CashBountyPower modules.
/// ChemicalGeneral.ini: Bounty = 5% / 10% / 20%.
pub const CASH_BOUNTY1_PERCENT: f32 = 0.05;
pub const CASH_BOUNTY2_PERCENT: f32 = 0.10;
pub const CASH_BOUNTY3_PERCENT: f32 = 0.20;

/// Retail CashBountyPower Bounty percent strings residual.
pub const CASH_BOUNTY1_PERCENT_STR: &str = "5%";
pub const CASH_BOUNTY2_PERCENT_STR: &str = "10%";
pub const CASH_BOUNTY3_PERCENT_STR: &str = "20%";

/// Science names that unlock cash bounty tiers.
pub const SCIENCE_CASH_BOUNTY1: &str = "SCIENCE_CashBounty1";
pub const SCIENCE_CASH_BOUNTY2: &str = "SCIENCE_CashBounty2";
pub const SCIENCE_CASH_BOUNTY3: &str = "SCIENCE_CashBounty3";

/// Retail SpecialAbility template residual names.
pub const SPECIAL_ABILITY_CASH_BOUNTY1: &str = "SpecialAbilityCashBounty1";
pub const SPECIAL_ABILITY_CASH_BOUNTY2: &str = "SpecialAbilityCashBounty2";
pub const SPECIAL_ABILITY_CASH_BOUNTY3: &str = "SpecialAbilityCashBounty3";
/// Retail SpecialPower enum residual (shared by all tiers).
pub const CASH_BOUNTY_ENUM: &str = "SPECIAL_CASH_BOUNTY";
/// Retail SciencePurchasePointCost residual (all tiers).
pub const CASH_BOUNTY_SCIENCE_POINT_COST: u32 = 1;
/// Retail SCIENCE_CashBounty1 PrerequisiteSciences residual tokens.
pub const CASH_BOUNTY1_PREREQ_SCIENCES: [&str; 2] = ["SCIENCE_GLA", "SCIENCE_Rank3"];
/// Retail SCIENCE_CashBounty2 PrerequisiteSciences residual tokens.
pub const CASH_BOUNTY2_PREREQ_SCIENCES: [&str; 2] = ["SCIENCE_CashBounty1", "SCIENCE_Rank3"];
/// Retail SCIENCE_CashBounty3 PrerequisiteSciences residual tokens.
pub const CASH_BOUNTY3_PREREQ_SCIENCES: [&str; 2] = ["SCIENCE_CashBounty2", "SCIENCE_Rank3"];

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

/// Host residual floating cash text presentation for bounty awards.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostCashBountyFloatingText {
    pub text: String,
    pub text_key: String,
    pub position: Vec3,
    pub color_rgba: (u8, u8, u8, u8),
    pub amount: u32,
    pub spawn_frame: u32,
    pub killer_id: ObjectId,
    pub victim_id: ObjectId,
}

impl HostCashBountyFloatingText {
    pub fn new(
        killer_id: ObjectId,
        victim_id: ObjectId,
        position: Vec3,
        amount: u32,
        spawn_frame: u32,
    ) -> Self {
        Self {
            text: format!("+${amount}"),
            text_key: CASH_BOUNTY_FLOATING_TEXT_ADD_CASH_KEY.to_string(),
            position: Vec3::new(
                position.x,
                position.y + CASH_BOUNTY_FLOATING_TEXT_Z_OFFSET,
                position.z,
            ),
            color_rgba: CASH_BOUNTY_FLOATING_TEXT_COLOR_RGBA,
            amount,
            spawn_frame,
            killer_id,
            victim_id,
        }
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
    /// Floating cash text residual descriptors spawned this session.
    #[serde(default)]
    pub floating_texts: Vec<HostCashBountyFloatingText>,
    /// Floating cash text residual spawn count (honesty).
    #[serde(default)]
    pub floating_texts_total: u32,
    /// Awards that used victim last_damage_source residual for killer ObjectId.
    #[serde(default)]
    pub last_damage_source_kills: u32,
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

    /// Record residual bounty floating cash text presentation.
    pub fn record_floating_text(&mut self, text: HostCashBountyFloatingText) {
        self.floating_texts_total = self.floating_texts_total.saturating_add(1);
        self.floating_texts.push(text);
        if self.floating_texts.len() > 32 {
            let drain = self.floating_texts.len() - 32;
            self.floating_texts.drain(0..drain);
        }
    }

    /// Record that killer ObjectId came from victim last_damage_source residual.
    pub fn record_last_damage_source_kill(&mut self) {
        self.last_damage_source_kills = self.last_damage_source_kills.saturating_add(1);
    }

    /// Residual honesty: at least one bounty award completed.
    pub fn honesty_bounty_award_ok(&self) -> bool {
        self.bounty_kills > 0 && self.bounty_earned_total > 0
    }

    /// Residual honesty: cash bounty percent was configured.
    pub fn honesty_bounty_configured_ok(&self) -> bool {
        self.max_bounty_percent > 0.0
    }

    /// Residual honesty: floating cash text presentation spawned.
    pub fn honesty_floating_text_ok(&self) -> bool {
        self.floating_texts_total > 0
            && self.floating_texts.iter().any(|t| {
                t.amount > 0
                    && t.text_key == CASH_BOUNTY_FLOATING_TEXT_ADD_CASH_KEY
                    && t.color_rgba == CASH_BOUNTY_FLOATING_TEXT_COLOR_RGBA
            })
    }

    pub fn honesty_floating_text_constants_ok() -> bool {
        CASH_BOUNTY_FLOATING_TEXT_ADD_CASH_KEY == "GUI:AddCash"
            && (CASH_BOUNTY_FLOATING_TEXT_Z_OFFSET - 10.0).abs() < 0.01
            && CASH_BOUNTY_FLOATING_TEXT_COLOR_RGBA == (255, 255, 0, 255)
    }

    /// Residual honesty: at least one bounty used last_damage_source killer residual.
    pub fn honesty_last_damage_source_killer_ok(&self) -> bool {
        self.last_damage_source_kills > 0
    }

    /// Combined residual honesty (configured + awarded).
    pub fn honesty_ok(&self) -> bool {
        self.honesty_bounty_configured_ok() && self.honesty_bounty_award_ok()
    }
}

// --- Wave 66 residual honesty packs ---

/// Wave 66 residual honesty: science / percent tier residual peel.
pub fn honesty_cash_bounty_science_residual_ok() -> bool {
    SCIENCE_CASH_BOUNTY1 == "SCIENCE_CashBounty1"
        && SCIENCE_CASH_BOUNTY2 == "SCIENCE_CashBounty2"
        && SCIENCE_CASH_BOUNTY3 == "SCIENCE_CashBounty3"
        && (CASH_BOUNTY1_PERCENT - 0.05).abs() < 0.0001
        && (CASH_BOUNTY2_PERCENT - 0.10).abs() < 0.0001
        && (CASH_BOUNTY3_PERCENT - 0.20).abs() < 0.0001
        && CASH_BOUNTY1_PERCENT_STR == "5%"
        && CASH_BOUNTY2_PERCENT_STR == "10%"
        && CASH_BOUNTY3_PERCENT_STR == "20%"
        && CASH_BOUNTY_SCIENCE_POINT_COST == 1
        && CASH_BOUNTY1_PREREQ_SCIENCES == ["SCIENCE_GLA", "SCIENCE_Rank3"]
        && CASH_BOUNTY2_PREREQ_SCIENCES == ["SCIENCE_CashBounty1", "SCIENCE_Rank3"]
        && CASH_BOUNTY3_PREREQ_SCIENCES == ["SCIENCE_CashBounty2", "SCIENCE_Rank3"]
        && cash_bounty_percent_for_science(SCIENCE_CASH_BOUNTY1) == Some(0.05)
        && cash_bounty_percent_for_science(SCIENCE_CASH_BOUNTY2) == Some(0.10)
        && cash_bounty_percent_for_science(SCIENCE_CASH_BOUNTY3) == Some(0.20)
}

/// Wave 66 residual honesty: special-power residual peel.
pub fn honesty_cash_bounty_special_power_residual_ok() -> bool {
    SPECIAL_ABILITY_CASH_BOUNTY1 == "SpecialAbilityCashBounty1"
        && SPECIAL_ABILITY_CASH_BOUNTY2 == "SpecialAbilityCashBounty2"
        && SPECIAL_ABILITY_CASH_BOUNTY3 == "SpecialAbilityCashBounty3"
        && CASH_BOUNTY_ENUM == "SPECIAL_CASH_BOUNTY"
        && compute_bounty_award(100, CASH_BOUNTY1_PERCENT) == 5
        && compute_bounty_award(100, CASH_BOUNTY2_PERCENT) == 10
        && compute_bounty_award(100, CASH_BOUNTY3_PERCENT) == 20
        && compute_bounty_award(101, CASH_BOUNTY1_PERCENT) == 6
}

/// Wave 66 residual honesty: floating text residual peel.
pub fn honesty_cash_bounty_floating_text_residual_ok() -> bool {
    HostCashBountyRegistry::honesty_floating_text_constants_ok()
        && CASH_BOUNTY_FLOATING_TEXT_ADD_CASH_KEY == "GUI:AddCash"
        && (CASH_BOUNTY_FLOATING_TEXT_Z_OFFSET - 10.0).abs() < 0.01
        && CASH_BOUNTY_FLOATING_TEXT_COLOR_RGBA == (255, 255, 0, 255)
}

/// Combined Wave 66 Cash Bounty residual honesty pack.
pub fn honesty_cash_bounty_residual_pack_ok() -> bool {
    honesty_cash_bounty_science_residual_ok()
        && honesty_cash_bounty_special_power_residual_ok()
        && honesty_cash_bounty_floating_text_residual_ok()
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

    #[test]
    fn floating_text_residual_yellow_z10() {
        assert!(HostCashBountyRegistry::honesty_floating_text_constants_ok());
        let mut reg = HostCashBountyRegistry::new();
        let ft = HostCashBountyFloatingText::new(
            ObjectId(1),
            ObjectId(2),
            Vec3::new(0.0, 0.0, 0.0),
            120,
            10,
        );
        assert_eq!(ft.text, "+$120");
        assert_eq!(ft.color_rgba, (255, 255, 0, 255));
        assert!((ft.position.y - 10.0).abs() < 0.01);
        reg.record_floating_text(ft);
        assert!(reg.honesty_floating_text_ok());
    }

    #[test]
    fn last_damage_source_killer_residual_honesty() {
        let mut reg = HostCashBountyRegistry::new();
        assert!(!reg.honesty_last_damage_source_killer_ok());
        reg.record_last_damage_source_kill();
        assert!(reg.honesty_last_damage_source_killer_ok());
        assert_eq!(reg.last_damage_source_kills, 1);
    }

    #[test]
    fn cash_bounty_residual_pack_honesty_wave66() {
        assert!(honesty_cash_bounty_science_residual_ok());
        assert!(honesty_cash_bounty_special_power_residual_ok());
        assert!(honesty_cash_bounty_floating_text_residual_ok());
        assert!(honesty_cash_bounty_residual_pack_ok());
        assert_eq!(CASH_BOUNTY_ENUM, "SPECIAL_CASH_BOUNTY");
        assert_eq!(CASH_BOUNTY_SCIENCE_POINT_COST, 1);
        assert_eq!(CASH_BOUNTY3_PERCENT_STR, "20%");
    }
}
