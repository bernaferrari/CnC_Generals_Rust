//! Host Tech Oil Derrick residual income (AutoDepositUpdate).
//!
//! Residual slice (playability):
//! - Captured `TechOilDerrick` / `*OilDerrick*` deposits cash on a fixed interval
//!   (retail CivilianBuilding.ini TechOilDerrick AutoDepositUpdate).
//! - DepositAmount residual **200**, DepositTiming residual **12000 ms → 360 frames**
//!   at 30 FPS logic.
//! - InitialCaptureBonus residual **1000** once when a neutral derrick first becomes
//!   non-neutral owned (Player::gainObject → awardInitialCaptureBonus residual).
//! - UpgradedBoost residual: `Upgrade_AmericaSupplyLines` **+20** (C++ getUpgradedSupplyBoost).
//! - AutoDeposit floating cash text residual: host `+$N` at building pos + Z **10**,
//!   player color RGB + alpha **230** (presentation state, not full InGameUI draw).
//!
//! Residual STEALTHED local-player display gate (AutoDepositUpdate):
//! - If STEALTHED && !isLocallyControlled && !DETECTED → hide floating cash text.
//! - Cash still deposits; only the floating text presentation is gated.
//!
//! Residual structure geometry scatter (AutoDepositUpdate for KINDOF_STRUCTURE):
//! - Floating cash text offset by ±0.3 × major/minor radius (deterministic residual).
//!
//! Fail-closed honesty:
//! - Not full InGameUI::addFloatingText GPU draw / Unicode GameText localization
//! - Not full GeometryInfo major/minor matrix / GameClientRandomValue stream
//! - Not full capture flow module wiring beyond residual team-change detect
//! - Neutral / under-construction residual-skip (C++ isNeutralControlled +
//!   construction-complete gates)
//! - Network deferred

use super::ObjectId;
use glam::Vec3;
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

/// Retail UpgradedBoost UpgradeType:Upgrade_AmericaSupplyLines Boost:**20**.
pub const OIL_DERRICK_SUPPLY_LINES_BOOST: u32 = 20;

/// Retail Upgrade_AmericaSupplyLines name honesty for oil derrick boost.
pub const OIL_DERRICK_SUPPLY_LINES_UPGRADE: &str = "Upgrade_AmericaSupplyLines";

/// Audio residual when oil derrick deposits (fail-closed host cue name).
pub const OIL_DERRICK_DEPOSIT_AUDIO: &str = "OilDerrickDeposit";

/// Audio residual when capture bonus is awarded.
pub const OIL_DERRICK_CAPTURE_BONUS_AUDIO: &str = "OilDerrickCaptureBonus";

/// C++ AutoDepositUpdate floating text Z lift (pos.z += 10.0f). Host Y-up → Y + 10.
pub const OIL_DERRICK_FLOATING_TEXT_Z_OFFSET: f32 = 10.0;

/// Residual GameText key honesty for cash gain caption (C++ "GUI:AddCash").
pub const OIL_DERRICK_FLOATING_TEXT_ADD_CASH_KEY: &str = "GUI:AddCash";

/// Residual floating text alpha (C++ GameMakeColor(0,0,0,230) OR'd onto player color).
pub const OIL_DERRICK_FLOATING_TEXT_ALPHA: u8 = 230;

/// C++ AutoDepositUpdate structure floating-text scatter scale
/// (`getMajorRadius() * 0.3f` / `getMinorRadius() * 0.3f`).
pub const OIL_DERRICK_FLOATING_TEXT_SCATTER_SCALE: f32 = 0.3;

/// Default residual structure major/minor radius when GeometryInfo is unavailable
/// (fail-closed host residual; TechOilDerrick footprint proxy).
pub const OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS: f32 = 25.0;

/// Deterministic residual structure floating-text scatter (C++ GameClientRandomValue
/// ± width/depth for KINDOF_STRUCTURE). Returns host XZ offset.
///
/// Fail-closed vs full GameClientRandomValue stream / GeometryInfo matrix.
pub fn structure_floating_text_scatter(
    seed: u32,
    major_radius: f32,
    minor_radius: f32,
) -> (f32, f32) {
    let width = (major_radius * OIL_DERRICK_FLOATING_TEXT_SCATTER_SCALE).max(0.0);
    let depth = (minor_radius * OIL_DERRICK_FLOATING_TEXT_SCATTER_SCALE).max(0.0);
    if width <= 0.0 && depth <= 0.0 {
        return (0.0, 0.0);
    }
    let phase = (seed as f32 + 1.0) * 0.618_033_988_7;
    let dx = if width > 0.0 {
        (phase.fract() * 2.0 - 1.0) * width
    } else {
        0.0
    };
    let dz = if depth > 0.0 {
        ((phase + 0.37).fract() * 2.0 - 1.0) * depth
    } else {
        0.0
    };
    (dx, dz)
}

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

/// C++ AutoDepositUpdate::getUpgradedSupplyBoost residual for oil derrick.
///
/// Retail: UpgradedBoost UpgradeType:Upgrade_AmericaSupplyLines Boost:20.
pub fn oil_derrick_deposit_amount(has_supply_lines: bool) -> (u32, u32) {
    let boost = if has_supply_lines {
        OIL_DERRICK_SUPPLY_LINES_BOOST
    } else {
        0
    };
    (
        OIL_DERRICK_DEPOSIT_AMOUNT.saturating_add(boost),
        boost,
    )
}

/// C++ AutoDepositUpdate / HackInternetAIUpdate floating-text local display gate.
///
/// When the source is STEALTHED, only the local controlling player (or a DETECTED
/// unit) may see the floating cash text. Cash still deposits either way.
///
/// ```text
/// if STEALTHED && !isLocallyControlled && !DETECTED → displayMoney = FALSE
/// ```
pub fn should_display_stealthed_floating_cash(
    is_stealthed: bool,
    is_detected: bool,
    is_locally_controlled: bool,
) -> bool {
    if is_stealthed && !is_locally_controlled && !is_detected {
        return false;
    }
    true
}

/// Host residual AutoDeposit floating cash text presentation.
///
/// C++ AutoDepositUpdate::update → InGameUI::addFloatingText(GUI:AddCash, pos+Z10,
/// playerColor | A230). Fail-closed: not full InGameUI GPU draw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostAutoDepositFloatingText {
    pub text: String,
    pub text_key: String,
    pub position: Vec3,
    pub color_rgba: (u8, u8, u8, u8),
    pub amount: u32,
    pub spawn_frame: u32,
    pub source_id: ObjectId,
    /// True when this text was from InitialCaptureBonus residual.
    pub is_capture_bonus: bool,
}

impl HostAutoDepositFloatingText {
    pub fn new(
        source_id: ObjectId,
        position: Vec3,
        amount: u32,
        player_color_rgb: (u8, u8, u8),
        spawn_frame: u32,
        is_capture_bonus: bool,
    ) -> Self {
        Self {
            text: format!("+${amount}"),
            text_key: OIL_DERRICK_FLOATING_TEXT_ADD_CASH_KEY.to_string(),
            position: Vec3::new(
                position.x,
                position.y + OIL_DERRICK_FLOATING_TEXT_Z_OFFSET,
                position.z,
            ),
            color_rgba: (
                player_color_rgb.0,
                player_color_rgb.1,
                player_color_rgb.2,
                OIL_DERRICK_FLOATING_TEXT_ALPHA,
            ),
            amount,
            spawn_frame,
            source_id,
            is_capture_bonus,
        }
    }
}

/// Host residual honesty + per-derrick deposit schedule + capture bonus tracking.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostOilDerrickRegistry {
    /// Number of successful residual periodic deposits.
    pub deposits: u32,
    /// Total cash from periodic AutoDeposit residual (includes SupplyLines boost).
    pub cash_total: u32,
    /// SupplyLines UpgradedBoost cash portion observed.
    #[serde(default)]
    pub supply_lines_boost_cash_total: u32,
    /// Number of residual capture bonuses awarded.
    pub capture_bonuses: u32,
    /// Total cash from InitialCaptureBonus residual.
    pub capture_bonus_cash_total: u32,
    /// Floating cash text residual descriptors spawned this session.
    #[serde(default)]
    pub floating_texts: Vec<HostAutoDepositFloatingText>,
    /// Floating cash text residual spawn count (honesty).
    #[serde(default)]
    pub floating_texts_total: u32,
    /// Floating cash text suppressed by STEALTHED local display gate residual.
    #[serde(default)]
    pub floating_texts_suppressed: u32,
    /// Structure geometry scatter residual applications (honesty).
    #[serde(default)]
    pub geometry_scatter_applications: u32,
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

    pub fn supply_lines_boost_cash_total(&self) -> u32 {
        self.supply_lines_boost_cash_total
    }

    pub fn floating_texts_total(&self) -> u32 {
        self.floating_texts_total
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
    ///
    /// `supply_lines_boost` is the UpgradedBoost portion included in `amount`
    /// (honesty tracking only; amount is already base+boost).
    pub fn try_deposit(
        &mut self,
        derrick_id: ObjectId,
        current_frame: u32,
        amount: u32,
        supply_lines_boost: u32,
    ) -> u32 {
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
        self.supply_lines_boost_cash_total = self
            .supply_lines_boost_cash_total
            .saturating_add(supply_lines_boost.min(amount));
        amount
    }

    /// Record residual AutoDeposit floating cash text presentation.
    pub fn record_floating_text(&mut self, text: HostAutoDepositFloatingText) {
        self.floating_texts_total = self.floating_texts_total.saturating_add(1);
        self.floating_texts.push(text);
        if self.floating_texts.len() > 32 {
            let drain = self.floating_texts.len() - 32;
            self.floating_texts.drain(0..drain);
        }
    }

    /// Record STEALTHED local-player display gate residual (text hidden).
    pub fn record_floating_text_suppressed(&mut self) {
        self.floating_texts_suppressed = self.floating_texts_suppressed.saturating_add(1);
    }

    /// Record structure geometry scatter residual application on floating text.
    pub fn record_geometry_scatter(&mut self) {
        self.geometry_scatter_applications =
            self.geometry_scatter_applications.saturating_add(1);
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

    /// Residual honesty: SupplyLines UpgradedBoost cash observed.
    pub fn honesty_supply_lines_boost_ok(&self) -> bool {
        self.supply_lines_boost_cash_total > 0
            && OIL_DERRICK_SUPPLY_LINES_BOOST == 20
            && OIL_DERRICK_SUPPLY_LINES_UPGRADE == "Upgrade_AmericaSupplyLines"
    }

    /// Residual honesty: floating cash text presentation spawned.
    pub fn honesty_floating_text_ok(&self) -> bool {
        self.floating_texts_total > 0
            && self.floating_texts.iter().any(|t| {
                t.amount > 0
                    && t.text_key == OIL_DERRICK_FLOATING_TEXT_ADD_CASH_KEY
                    && t.color_rgba.3 == OIL_DERRICK_FLOATING_TEXT_ALPHA
            })
    }

    pub fn honesty_floating_text_constants_ok() -> bool {
        OIL_DERRICK_FLOATING_TEXT_ADD_CASH_KEY == "GUI:AddCash"
            && (OIL_DERRICK_FLOATING_TEXT_Z_OFFSET - 10.0).abs() < 0.01
            && OIL_DERRICK_FLOATING_TEXT_ALPHA == 230
            && OIL_DERRICK_SUPPLY_LINES_BOOST == 20
    }

    /// Residual honesty: STEALTHED local display gate suppressed at least one text.
    pub fn honesty_floating_text_stealth_gate_ok(&self) -> bool {
        self.floating_texts_suppressed > 0
    }

    /// Residual honesty: structure geometry scatter residual applied.
    pub fn honesty_geometry_scatter_ok(&self) -> bool {
        self.geometry_scatter_applications > 0
            && (OIL_DERRICK_FLOATING_TEXT_SCATTER_SCALE - 0.3).abs() < 0.001
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
        assert_eq!(reg.try_deposit(id, 0, OIL_DERRICK_DEPOSIT_AMOUNT, 0), 0);
        assert_eq!(reg.try_deposit(id, 360, OIL_DERRICK_DEPOSIT_AMOUNT, 0), 200);
        assert_eq!(reg.try_deposit(id, 360, OIL_DERRICK_DEPOSIT_AMOUNT, 0), 0);
        assert_eq!(reg.try_deposit(id, 720, OIL_DERRICK_DEPOSIT_AMOUNT, 0), 200);
        assert!(reg.honesty_deposit_ok());
        assert_eq!(reg.deposits(), 2);
        assert_eq!(reg.cash_total(), 400);
    }

    #[test]
    fn supply_lines_boost_and_floating_text_residual() {
        assert!(HostOilDerrickRegistry::honesty_floating_text_constants_ok());
        assert_eq!(oil_derrick_deposit_amount(false), (200, 0));
        assert_eq!(oil_derrick_deposit_amount(true), (220, 20));

        let mut reg = HostOilDerrickRegistry::new();
        let id = ObjectId(3);
        let (amount, boost) = oil_derrick_deposit_amount(true);
        // First observe schedules next deposit at frame 360.
        assert_eq!(reg.try_deposit(id, 0, amount, boost), 0);
        assert_eq!(reg.try_deposit(id, 360, amount, boost), 220);
        assert_eq!(reg.supply_lines_boost_cash_total(), 20);
        assert!(reg.honesty_supply_lines_boost_ok());

        let ft = HostAutoDepositFloatingText::new(
            id,
            Vec3::new(10.0, 0.0, 20.0),
            220,
            (0, 128, 255),
            360,
            false,
        );
        assert_eq!(ft.text, "+$220");
        assert_eq!(ft.text_key, "GUI:AddCash");
        assert!((ft.position.y - 10.0).abs() < 0.01);
        assert_eq!(ft.color_rgba, (0, 128, 255, 230));
        reg.record_floating_text(ft);
        assert!(reg.honesty_floating_text_ok());
        assert_eq!(reg.floating_texts_total(), 1);
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

    #[test]
    fn stealthed_local_display_gate_residual() {
        // Visible when not stealthed.
        assert!(should_display_stealthed_floating_cash(false, false, false));
        assert!(should_display_stealthed_floating_cash(false, false, true));
        // Stealthed + local → show (local player sees own cash pings).
        assert!(should_display_stealthed_floating_cash(true, false, true));
        // Stealthed + detected (any viewer) → show.
        assert!(should_display_stealthed_floating_cash(true, true, false));
        // Stealthed + non-local + undetected → hide.
        assert!(!should_display_stealthed_floating_cash(true, false, false));

        let mut reg = HostOilDerrickRegistry::new();
        reg.record_floating_text_suppressed();
        assert!(reg.honesty_floating_text_stealth_gate_ok());
        assert_eq!(reg.floating_texts_suppressed, 1);
    }

    #[test]
    fn structure_geometry_scatter_residual() {
        assert!((OIL_DERRICK_FLOATING_TEXT_SCATTER_SCALE - 0.3).abs() < 0.001);
        let (dx, dz) = structure_floating_text_scatter(0, 50.0, 40.0);
        // ±0.3 * major/minor → within ±15 / ±12.
        assert!(dx.abs() <= 15.0 + 0.001);
        assert!(dz.abs() <= 12.0 + 0.001);
        assert!(dx != 0.0 || dz != 0.0, "non-zero scatter expected for r>0");
        let zero = structure_floating_text_scatter(1, 0.0, 0.0);
        assert_eq!(zero, (0.0, 0.0));
        // Deterministic for same seed.
        let a = structure_floating_text_scatter(7, 25.0, 25.0);
        let b = structure_floating_text_scatter(7, 25.0, 25.0);
        assert_eq!(a, b);

        let mut reg = HostOilDerrickRegistry::new();
        let id = ObjectId(9);
        let (sx, sz) = structure_floating_text_scatter(9, OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS, OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS);
        let base = Vec3::new(100.0, 0.0, 200.0);
        let ft = HostAutoDepositFloatingText::new(
            id,
            Vec3::new(base.x + sx, base.y, base.z + sz),
            200,
            (255, 0, 0),
            360,
            false,
        );
        // Floating text Y still lifts by Z offset residual.
        assert!((ft.position.y - OIL_DERRICK_FLOATING_TEXT_Z_OFFSET).abs() < 0.01);
        assert!((ft.position.x - (100.0 + sx)).abs() < 0.01);
        assert!((ft.position.z - (200.0 + sz)).abs() < 0.01);
        reg.record_geometry_scatter();
        reg.record_floating_text(ft);
        assert!(reg.honesty_geometry_scatter_ok());
        assert!(reg.honesty_floating_text_ok());
        assert_eq!(reg.geometry_scatter_applications, 1);
    }
}
