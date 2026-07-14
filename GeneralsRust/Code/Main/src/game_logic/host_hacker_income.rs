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
//! Residual floating cash text (HackInternetAIUpdate):
//! - Host `+$N` at unit pos + Z **20**, green RGBA (0,255,0,255), key `GUI:AddCash`.
//! - STEALTHED local-player display gate residual (owner + containedBy Internet Center).
//! - Internet Center geometry scatter residual (±0.3 major/minor radius).
//!
//! Fail-closed honesty:
//! - Not full Unpack/Pack state machine / variation factor / model conditions
//! - Not full InGameUI GPU draw / Unicode GameText localization
//! - Not full DISABLED_HACKED microwave interrupt resume matrix beyond skip-while-disabled
//! - XpPerCashUpdate residual applied as +1 XP when experience tracker present
//! - Network deferred

use super::ObjectId;
use super::VeterancyLevel;
use glam::Vec3;
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

/// C++ HackInternet floating text Z lift (pos.z += 20.0f). Host Y-up → Y + 20.
pub const HACKER_FLOATING_TEXT_Z_OFFSET: f32 = 20.0;

/// Residual GameText key honesty for cash gain caption.
pub const HACKER_FLOATING_TEXT_ADD_CASH_KEY: &str = "GUI:AddCash";

/// Residual floating cash text color (green, retail GameMakeColor(0,255,0,255)).
pub const HACKER_FLOATING_TEXT_COLOR_RGBA: (u8, u8, u8, u8) = (0, 255, 0, 255);

/// C++ Internet Center geometry scatter scale for floating text inside container
/// (`getMajorRadius() * 0.3f` / `getMinorRadius() * 0.3f`).
pub const HACKER_IC_FLOATING_TEXT_SCATTER_SCALE: f32 = 0.3;

/// C++ HackInternetAIUpdate floating-text local display gate (owner + container).
///
/// ```text
/// if owner STEALTHED && !local && !DETECTED → hide
/// if containedBy STEALTHED && !container local && !container DETECTED → hide
/// ```
pub fn should_display_hacker_floating_cash(
    owner_stealthed: bool,
    owner_detected: bool,
    owner_local: bool,
    has_container: bool,
    container_stealthed: bool,
    container_detected: bool,
    container_local: bool,
) -> bool {
    use crate::game_logic::host_oil_derrick::should_display_stealthed_floating_cash;
    if !should_display_stealthed_floating_cash(owner_stealthed, owner_detected, owner_local) {
        return false;
    }
    if has_container
        && !should_display_stealthed_floating_cash(
            container_stealthed,
            container_detected,
            container_local,
        )
    {
        return false;
    }
    true
}

/// Residual Internet Center floating-text scatter (C++ GameClientRandomValue
/// ± width/depth). Returns host XZ offset.
///
/// Uses pure ADC RandomValue algorithm seeded by `seed` (hacker_id + frame) so
/// re-query is stable and matches C++ integer client stream math.
/// Fail-closed vs full GeometryInfo major/minor matrix / live DisplayString GPU.
pub fn internet_center_floating_text_scatter(
    seed: u32,
    major_radius: f32,
    minor_radius: f32,
) -> (f32, f32) {
    super::host_rng_residual::pure_client_structure_scatter(
        seed,
        major_radius,
        minor_radius,
        HACKER_IC_FLOATING_TEXT_SCATTER_SCALE,
    )
}

/// Host residual HackInternet floating cash text presentation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostHackerFloatingText {
    pub text: String,
    pub text_key: String,
    pub position: Vec3,
    pub color_rgba: (u8, u8, u8, u8),
    pub amount: u32,
    pub spawn_frame: u32,
    pub hacker_id: ObjectId,
    pub in_internet_center: bool,
}

impl HostHackerFloatingText {
    pub fn new(
        hacker_id: ObjectId,
        position: Vec3,
        amount: u32,
        spawn_frame: u32,
        in_internet_center: bool,
    ) -> Self {
        Self {
            text: format!("+${amount}"),
            text_key: HACKER_FLOATING_TEXT_ADD_CASH_KEY.to_string(),
            position: Vec3::new(
                position.x,
                position.y + HACKER_FLOATING_TEXT_Z_OFFSET,
                position.z,
            ),
            color_rgba: HACKER_FLOATING_TEXT_COLOR_RGBA,
            amount,
            spawn_frame,
            hacker_id,
            in_internet_center,
        }
    }
}

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
    /// Floating cash text residual descriptors spawned this session.
    #[serde(default)]
    pub floating_texts: Vec<HostHackerFloatingText>,
    /// Floating cash text residual spawn count (honesty).
    #[serde(default)]
    pub floating_texts_total: u32,
    /// Floating cash text suppressed by STEALTHED local display gate residual.
    #[serde(default)]
    pub floating_texts_suppressed: u32,
    /// Internet Center geometry scatter residual applications.
    #[serde(default)]
    pub ic_scatter_applications: u32,
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

    /// Record residual HackInternet floating cash text presentation.
    pub fn record_floating_text(&mut self, text: HostHackerFloatingText) {
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

    /// Record Internet Center floating-text geometry scatter residual application.
    pub fn record_ic_scatter(&mut self) {
        self.ic_scatter_applications = self.ic_scatter_applications.saturating_add(1);
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

    /// Residual honesty: floating cash text presentation spawned.
    pub fn honesty_floating_text_ok(&self) -> bool {
        self.floating_texts_total > 0
            && self.floating_texts.iter().any(|t| {
                t.amount > 0
                    && t.text_key == HACKER_FLOATING_TEXT_ADD_CASH_KEY
                    && t.color_rgba == HACKER_FLOATING_TEXT_COLOR_RGBA
            })
    }

    pub fn honesty_floating_text_constants_ok() -> bool {
        HACKER_FLOATING_TEXT_ADD_CASH_KEY == "GUI:AddCash"
            && (HACKER_FLOATING_TEXT_Z_OFFSET - 20.0).abs() < 0.01
            && HACKER_FLOATING_TEXT_COLOR_RGBA == (0, 255, 0, 255)
            && (HACKER_IC_FLOATING_TEXT_SCATTER_SCALE - 0.3).abs() < 0.001
    }

    /// Residual honesty: STEALTHED local display gate suppressed at least one text.
    pub fn honesty_floating_text_stealth_gate_ok(&self) -> bool {
        self.floating_texts_suppressed > 0
    }

    /// Residual honesty: Internet Center scatter residual applied.
    pub fn honesty_ic_scatter_ok(&self) -> bool {
        self.ic_scatter_applications > 0
    }

    /// Combined residual honesty.
    pub fn honesty_ok(&self) -> bool {
        self.honesty_deposit_ok()
    }
}


// --- Wave 69 residual honesty peels (retail HackInternet / body / floating text) ---

/// Retail unpack residual (msec) — honesty peel only (host skips anim matrix).
pub const HACKER_UNPACK_TIME_MS: u32 = 7_300;
/// Retail pack residual (msec).
pub const HACKER_PACK_TIME_MS: u32 = 5_133;
/// Retail PackUnpackVariationFactor residual.
pub const HACKER_PACK_UNPACK_VARIATION: f32 = 0.5;

/// Retail ChinaInfantryHacker body residual.
pub const HACKER_MAX_HEALTH: f32 = 100.0;
pub const HACKER_BUILD_COST: u32 = 625;
pub const HACKER_BUILD_TIME_SEC: f32 = 20.0;
pub const HACKER_BUILD_TIME_FRAMES: u32 = 600;
pub const HACKER_VISION_RANGE: f32 = 150.0;
pub const HACKER_SHROUD_CLEARING_RANGE: f32 = 300.0;
pub const HACKER_TRANSPORT_SLOT_COUNT: u32 = 1;

/// Wave 69 residual honesty: HackInternet cash residual peel.
pub fn honesty_hacker_income_cash_residual_ok() -> bool {
    HACKER_CASH_UPDATE_DELAY_MS == 2_000
        && HACKER_CASH_UPDATE_DELAY_FAST_MS == 1_800
        && HACKER_CASH_INTERVAL_FRAMES
            == cash_interval_frames_from_ms(HACKER_CASH_UPDATE_DELAY_MS)
        && HACKER_CASH_INTERVAL_FRAMES == 60
        && HACKER_CASH_INTERVAL_FAST_FRAMES
            == cash_interval_frames_from_ms(HACKER_CASH_UPDATE_DELAY_FAST_MS)
        && HACKER_CASH_INTERVAL_FAST_FRAMES == 54
        && HACKER_CASH_REGULAR == 5
        && HACKER_CASH_VETERAN == 6
        && HACKER_CASH_ELITE == 8
        && HACKER_CASH_HEROIC == 10
        && (HACKER_XP_PER_CASH_UPDATE - 1.0).abs() < 0.01
        && cash_amount_for_level(VeterancyLevel::Rookie) == 5
        && cash_amount_for_level(VeterancyLevel::Heroic) == 10
        && cash_interval_frames(false) == 60
        && cash_interval_frames(true) == 54
        && HACKER_CASH_PING_AUDIO == "HackerCashPing"
        && HACKER_UNPACK_TIME_MS == 7_300
        && HACKER_PACK_TIME_MS == 5_133
        && (HACKER_PACK_UNPACK_VARIATION - 0.5).abs() < 0.01
}

/// Wave 69 residual honesty: floating cash text residual peel.
pub fn honesty_hacker_income_floating_text_residual_ok() -> bool {
    HostHackerIncomeRegistry::honesty_floating_text_constants_ok()
        && HACKER_FLOATING_TEXT_ADD_CASH_KEY == "GUI:AddCash"
        && (HACKER_FLOATING_TEXT_Z_OFFSET - 20.0).abs() < 0.01
        && HACKER_FLOATING_TEXT_COLOR_RGBA == (0, 255, 0, 255)
        && (HACKER_IC_FLOATING_TEXT_SCATTER_SCALE - 0.3).abs() < 0.001
        && {
            let ft = HostHackerFloatingText::new(
                ObjectId(1),
                glam::Vec3::ZERO,
                5,
                0,
                false,
            );
            ft.text == "+$5"
                && ft.text_key == "GUI:AddCash"
                && (ft.position.y - 20.0).abs() < 0.01
        }
        && should_display_hacker_floating_cash(true, false, true, false, false, false, false)
        && !should_display_hacker_floating_cash(true, false, false, false, false, false, false)
}

/// Wave 69 residual honesty: hacker body residual peel.
pub fn honesty_hacker_income_body_residual_ok() -> bool {
    (HACKER_MAX_HEALTH - 100.0).abs() < 0.01
        && HACKER_BUILD_COST == 625
        && (HACKER_BUILD_TIME_SEC - 20.0).abs() < 0.01
        && HACKER_BUILD_TIME_FRAMES
            == ((HACKER_BUILD_TIME_SEC * HACKER_LOGIC_FPS).round() as u32)
        && HACKER_BUILD_TIME_FRAMES == 600
        && (HACKER_VISION_RANGE - 150.0).abs() < 0.01
        && (HACKER_SHROUD_CLEARING_RANGE - 300.0).abs() < 0.01
        && HACKER_TRANSPORT_SLOT_COUNT == 1
        && is_hacker_template("ChinaInfantryHacker")
        && !is_hacker_template("ChinaInfantryBlackLotus")
        && is_internet_center_template("ChinaInternetCenter")
        && is_legal_hacker_income_source(true, false, false)
        && !is_legal_hacker_income_source(true, false, true)
}

/// Combined Wave 69 Hacker income residual honesty pack.
pub fn honesty_hacker_income_residual_pack_ok() -> bool {
    honesty_hacker_income_cash_residual_ok()
        && honesty_hacker_income_floating_text_residual_ok()
        && honesty_hacker_income_body_residual_ok()
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

    #[test]
    fn floating_text_residual_green_z20() {
        assert!(HostHackerIncomeRegistry::honesty_floating_text_constants_ok());
        let mut reg = HostHackerIncomeRegistry::new();
        let ft = HostHackerFloatingText::new(ObjectId(3), Vec3::new(1.0, 0.0, 2.0), 5, 60, false);
        assert_eq!(ft.text, "+$5");
        assert_eq!(ft.color_rgba, (0, 255, 0, 255));
        assert!((ft.position.y - 20.0).abs() < 0.01);
        reg.record_floating_text(ft);
        assert!(reg.honesty_floating_text_ok());
    }

    #[test]
    fn stealthed_local_display_gate_and_ic_scatter_residual() {
        // Field: stealthed non-local undetected → hide.
        assert!(!should_display_hacker_floating_cash(
            true, false, false, false, false, false, false
        ));
        // Field: stealthed local → show.
        assert!(should_display_hacker_floating_cash(
            true, false, true, false, false, false, false
        ));
        // Field: stealthed detected → show.
        assert!(should_display_hacker_floating_cash(
            true, true, false, false, false, false, false
        ));
        // IC: owner visible but container stealthed non-local → hide.
        assert!(!should_display_hacker_floating_cash(
            false, false, false, true, true, false, false
        ));
        // IC: container stealthed but local → show.
        assert!(should_display_hacker_floating_cash(
            false, false, false, true, true, false, true
        ));

        let (dx, dz) = internet_center_floating_text_scatter(0, 50.0, 40.0);
        assert!(dx.abs() <= 50.0 * 0.3 + 0.001);
        assert!(dz.abs() <= 40.0 * 0.3 + 0.001);
        let zero = internet_center_floating_text_scatter(1, 0.0, 0.0);
        assert!((zero.0).abs() < 0.001 && (zero.1).abs() < 0.001);

        let mut reg = HostHackerIncomeRegistry::new();
        reg.record_floating_text_suppressed();
        reg.record_ic_scatter();
        assert!(reg.honesty_floating_text_stealth_gate_ok());
        assert!(reg.honesty_ic_scatter_ok());
    }

    #[test]
    fn hacker_income_residual_pack_honesty_wave69() {
        assert_eq!(cash_interval_frames_from_ms(2000), 60);
        assert_eq!(cash_interval_frames_from_ms(1800), 54);
        assert!(honesty_hacker_income_cash_residual_ok());
        assert!(honesty_hacker_income_floating_text_residual_ok());
        assert!(honesty_hacker_income_body_residual_ok());
        assert!(honesty_hacker_income_residual_pack_ok());
        assert_eq!(HACKER_BUILD_TIME_FRAMES, 600);
        assert_eq!(HACKER_CASH_REGULAR, 5);
        assert_eq!(HACKER_FLOATING_TEXT_ADD_CASH_KEY, "GUI:AddCash");
    }
}
