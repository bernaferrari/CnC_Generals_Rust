//! Host GLA Saboteur residual (structure sabotage crate-collide path).
//!
//! Residual slice (playability):
//! - `GLAInfantrySaboteur` / Chem_/Demo_/Slth_ / TestSaboteur walks to enemy
//!   structure and applies type-specific residual (C++ Sabotage*CrateCollide):
//!   - **Power plant** (`FS_POWER` / powerplant name): player power brownout for
//!     `SabotagePowerDuration = 30000` ms → **900** frames
//!   - **Supply center** (`FS_SUPPLY_CENTER` / supply name): steal **1000** cash
//!   - **Military factory** (barracks / warfactory / airfield): `DISABLED_HACKED`
//!     for **30000** ms → **900** frames
//!   - **Superweapon / Strategy Center / Command Center**: reset special-power
//!     recharge residual (`startPowerRecharge` honesty)
//!   - **Internet Center**: `DISABLED_HACKED` for **15000** ms → **450** frames
//!   - **Fake building** (`FS_FAKE`): kill structure (unresistable residual)
//! - On success saboteur is consumed (C++ CrateCollide destroyObject).
//!
//! Wave 70 residual pack (retail GLAInfantry.ini sabotage crate modules):
//! - Effect residual: Power/Military SabotageDuration **30000**ms → **900**f,
//!   Internet **15000**ms → **450**f, StealCashAmount **1000**.
//! - Body residual: MaxHealth **120**, Vision **150**/Shroud **300**, BuildCost **800**,
//!   BuildTime **15**s → **450**f, slots **1**, Geometry CYLINDER **10**/**12**,
//!   Speed **30**/Damaged **20**, IsTrainable **No**, StealthDelay **2500**ms → **75**f.
//! - Honesty: `honesty_saboteur_residual_pack_ok` + layer honesty tests.
//!
//! Fail-closed honesty:
//! - Not full BuildingPickup CrateCollide goal-object gate edge matrix
//! - Not full EVA / floating-text / radar infiltration FX matrix
//! - Not full internet-center spy-vision / contained-hacker disable iterate
//! - Not network saboteur replication (network deferred)

use super::ObjectId;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const SABOTEUR_LOGIC_FPS: f32 = 30.0;

/// Retail SabotagePowerDuration 30000 ms → 900 frames @ 30 FPS.
pub const SABOTEUR_POWER_DURATION_FRAMES: u32 = 900;
/// Retail military factory SabotageDuration 30000 ms → 900 frames.
pub const SABOTEUR_MILITARY_DURATION_FRAMES: u32 = 900;
/// Retail Internet Center SabotageDuration 15000 ms → 450 frames.
pub const SABOTEUR_INTERNET_DURATION_FRAMES: u32 = 450;
/// Retail Supply Center StealCashAmount.
pub const SABOTEUR_STEAL_CASH_AMOUNT: u32 = 1000;
/// Retail SabotagePowerDuration residual (msec).
pub const SABOTEUR_POWER_DURATION_MS: u32 = 30_000;
/// Retail military factory SabotageDuration residual (msec).
pub const SABOTEUR_MILITARY_DURATION_MS: u32 = 30_000;
/// Retail Internet Center SabotageDuration residual (msec).
pub const SABOTEUR_INTERNET_DURATION_MS: u32 = 15_000;
/// Retail MaxHealth residual.
pub const SABOTEUR_MAX_HEALTH: f32 = 120.0;
/// Retail VisionRange residual.
pub const SABOTEUR_VISION_RANGE: f32 = 150.0;
/// Retail ShroudClearingRange residual.
pub const SABOTEUR_SHROUD_CLEARING_RANGE: f32 = 300.0;
/// Retail BuildCost residual.
pub const SABOTEUR_BUILD_COST: u32 = 800;
/// Retail BuildTime residual (seconds).
pub const SABOTEUR_BUILD_TIME_SEC: f32 = 15.0;
/// BuildTime 15s → 450 frames @ 30 FPS.
pub const SABOTEUR_BUILD_TIME_FRAMES: u32 = 450;
/// Retail TransportSlotCount residual.
pub const SABOTEUR_TRANSPORT_SLOT_COUNT: u32 = 1;
/// Retail Geometry CYLINDER MajorRadius residual.
pub const SABOTEUR_GEOMETRY_RADIUS: f32 = 10.0;
/// Retail GeometryHeight residual.
pub const SABOTEUR_GEOMETRY_HEIGHT: f32 = 12.0;
/// Retail SaboteurGroundLocomotor Speed residual.
pub const SABOTEUR_LOCOMOTOR_SPEED: f32 = 30.0;
/// Retail SaboteurGroundLocomotor SpeedDamaged residual.
pub const SABOTEUR_LOCOMOTOR_SPEED_DAMAGED: f32 = 20.0;
/// Retail ExperienceValue residual.
pub const SABOTEUR_EXPERIENCE_VALUE: [u32; 4] = [15, 15, 30, 40];
/// Retail IsTrainable residual (Saboteur cannot gain XP).
pub const SABOTEUR_IS_TRAINABLE: bool = false;
/// Retail StealthUpdate StealthDelay residual (msec) — innate stealth residual.
pub const SABOTEUR_STEALTH_DELAY_MS: u32 = 2_500;
/// StealthDelay 2500ms → 75 frames @ 30 FPS.
pub const SABOTEUR_STEALTH_DELAY_FRAMES: u32 = 75;

/// Residual audio when sabotage succeeds (building sabotaged cue).
pub const SABOTEUR_SUCCESS_AUDIO: &str = "BuildingSabotaged";
/// Residual cash-steal audio honesty.
pub const SABOTEUR_CASH_STEAL_AUDIO: &str = "MoneyWithdrawSound";
/// Residual superweapon timer-reset audio honesty.
pub const SABOTEUR_RESET_TIMER_AUDIO: &str = "SabotageResetTimerBuilding";
/// C++ MiscAudio m_sabotageShutDownBuilding residual.
pub const SABOTEUR_SHUTDOWN_AUDIO: &str = "SabotageShutDownBuilding";
/// C++ Drawable::flashAsSelected envelope decay residual (play color,0,4).
pub const SABOTEUR_FLASH_DECAY_FRAMES: u32 = 4;

/// Linear residual intensity for selection flash envelope (1.0 → 0.0).
#[inline]
pub fn selection_flash_intensity(remaining: u32) -> f32 {
    if remaining == 0 || SABOTEUR_FLASH_DECAY_FRAMES == 0 {
        return 0.0;
    }
    (remaining as f32 / SABOTEUR_FLASH_DECAY_FRAMES as f32).clamp(0.0, 1.0)
}

/// C++ GUI:AddCash floating text over saboteur (pos.z + 20).
pub const SABOTEUR_ADD_CASH_Z_OFFSET: f32 = 20.0;
/// C++ GUI:LoseCash floating text over victim (pos.z + 30).
pub const SABOTEUR_LOSE_CASH_Z_OFFSET: f32 = 30.0;
/// C++ GameMakeColor(0,255,0,255) residual for AddCash.
pub const SABOTEUR_ADD_CASH_COLOR_RGBA: (u8, u8, u8, u8) = (0, 255, 0, 255);
/// C++ GameMakeColor(255,0,0,255) residual for LoseCash.
pub const SABOTEUR_LOSE_CASH_COLOR_RGBA: (u8, u8, u8, u8) = (255, 0, 0, 255);
pub const SABOTEUR_ADD_CASH_TEXT_KEY: &str = "GUI:AddCash";
pub const SABOTEUR_LOSE_CASH_TEXT_KEY: &str = "GUI:LoseCash";

/// Kind of sabotage residual applied to a structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SaboteurEffectKind {
    /// Player power brownout residual.
    PowerPlant,
    /// Steal cash residual.
    SupplyCenter,
    /// Structure DISABLED_HACKED residual (production pause).
    MilitaryFactory,
    /// Superweapon / strategy / command special-power recharge reset.
    SuperweaponOrCommand,
    /// Internet Center DISABLED_HACKED residual (shorter).
    InternetCenter,
    /// Fake building destroy residual.
    FakeBuilding,
}

impl SaboteurEffectKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::PowerPlant => "PowerPlant",
            Self::SupplyCenter => "SupplyCenter",
            Self::MilitaryFactory => "MilitaryFactory",
            Self::SuperweaponOrCommand => "SuperweaponOrCommand",
            Self::InternetCenter => "InternetCenter",
            Self::FakeBuilding => "FakeBuilding",
        }
    }

    /// Absolute until-frame for DISABLED_HACKED residual kinds (None otherwise).
    pub fn disabled_hacked_until(self, current_frame: u32) -> Option<u32> {
        match self {
            Self::MilitaryFactory => {
                Some(current_frame.saturating_add(SABOTEUR_MILITARY_DURATION_FRAMES))
            }
            Self::InternetCenter => {
                Some(current_frame.saturating_add(SABOTEUR_INTERNET_DURATION_FRAMES))
            }
            _ => None,
        }
    }

    pub fn power_sabotage_until(self, current_frame: u32) -> Option<u32> {
        match self {
            Self::PowerPlant => Some(current_frame.saturating_add(SABOTEUR_POWER_DURATION_FRAMES)),
            _ => None,
        }
    }

    pub fn steals_cash(self) -> bool {
        matches!(self, Self::SupplyCenter)
    }

    /// C++ CrateCollide::doSabotageFeedbackFX sound selection residual.
    /// FakeBuilding returns None (no additional feedback).
    pub fn feedback_audio(self) -> Option<&'static str> {
        match self {
            Self::FakeBuilding => None,
            Self::SuperweaponOrCommand => Some(SABOTEUR_RESET_TIMER_AUDIO),
            Self::SupplyCenter => Some(SABOTEUR_CASH_STEAL_AUDIO),
            Self::PowerPlant | Self::MilitaryFactory | Self::InternetCenter => {
                Some(SABOTEUR_SHUTDOWN_AUDIO)
            }
        }
    }

    /// C++ SabotageVictimType residual label for honesty/tests.
    pub fn victim_type_label(self) -> &'static str {
        match self {
            Self::PowerPlant => "SAB_VICTIM_POWER_PLANT",
            Self::SupplyCenter => "SAB_VICTIM_SUPPLY_CENTER",
            Self::MilitaryFactory => "SAB_VICTIM_MILITARY_FACTORY",
            Self::SuperweaponOrCommand => "SAB_VICTIM_SUPERWEAPON",
            Self::InternetCenter => "SAB_VICTIM_INTERNET_CENTER",
            Self::FakeBuilding => "SAB_VICTIM_FAKE_BUILDING",
        }
    }

    pub fn destroys_target(self) -> bool {
        matches!(self, Self::FakeBuilding)
    }

    pub fn resets_special_power(self) -> bool {
        matches!(self, Self::SuperweaponOrCommand)
    }
}

/// Host residual honesty counters for Saboteur path.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostSaboteurRegistry {
    /// Successful sabotage completions (any kind).
    pub sabotages: u32,
    /// Power-plant brownouts applied.
    pub power_plants: u32,
    /// Supply cash steals applied.
    pub supply_steals: u32,
    /// Total cash stolen residual.
    pub cash_stolen_total: u32,
    /// Military factory disables applied.
    pub military_disables: u32,
    /// Superweapon / command timer resets.
    pub superweapon_resets: u32,
    /// Internet center disables.
    pub internet_disables: u32,
    /// SpyVisionUpdate setDisabledUntilFrame residual (all team centers).
    pub internet_spy_vision_disables: u32,
    /// Contained hackers DISABLED_HACKED residual count.
    pub internet_hackers_disabled: u32,
    /// Superweapon startPowerRecharge residual applications.
    pub superweapon_power_resets: u32,
    /// Fake building DETONATED residual kills.
    pub fake_detonated: u32,
    /// Radar tryInfiltrationEvent residual fires.
    pub infiltration_events: u32,
    /// EVA BuildingSabotaged residual fires.
    pub eva_building_sabotaged: u32,
    /// EVA CashStolen residual fires.
    pub eva_cash_stolen: u32,
    /// doSabotageFeedbackFX residual applications (audio and/or flash).
    pub feedback_fx: u32,
    /// flashAsSelected residual applications.
    pub flash_as_selected: u32,
    /// GUI:AddCash/LoseCash floating text pairs from cash sabotage.
    pub cash_floating_texts: u32,
    /// EVA UnitLost residual fires.
    pub eva_unit_lost: u32,
    /// EVA BuildingLost residual fires.
    pub eva_building_lost: u32,
    /// Fake buildings destroyed.
    pub fakes_destroyed: u32,
    /// Saboteurs consumed on success.
    pub saboteurs_consumed: u32,
}

impl HostSaboteurRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record(&mut self, kind: SaboteurEffectKind, cash_stolen: u32) {
        self.sabotages = self.sabotages.saturating_add(1);
        match kind {
            SaboteurEffectKind::PowerPlant => {
                self.power_plants = self.power_plants.saturating_add(1);
            }
            SaboteurEffectKind::SupplyCenter => {
                self.supply_steals = self.supply_steals.saturating_add(1);
                self.cash_stolen_total = self.cash_stolen_total.saturating_add(cash_stolen);
            }
            SaboteurEffectKind::MilitaryFactory => {
                self.military_disables = self.military_disables.saturating_add(1);
            }
            SaboteurEffectKind::SuperweaponOrCommand => {
                self.superweapon_resets = self.superweapon_resets.saturating_add(1);
            }
            SaboteurEffectKind::InternetCenter => {
                self.internet_disables = self.internet_disables.saturating_add(1);
            }
            SaboteurEffectKind::FakeBuilding => {
                self.fakes_destroyed = self.fakes_destroyed.saturating_add(1);
            }
        }
    }

    pub fn record_consumed(&mut self) {
        self.saboteurs_consumed = self.saboteurs_consumed.saturating_add(1);
    }
    pub fn record_internet_spy_vision_disable(&mut self, centers: u32, hackers: u32) {
        self.internet_spy_vision_disables =
            self.internet_spy_vision_disables.saturating_add(centers);
        self.internet_hackers_disabled = self.internet_hackers_disabled.saturating_add(hackers);
    }

    pub fn honesty_internet_spy_vision_ok(&self) -> bool {
        self.internet_spy_vision_disables > 0
    }

    pub fn honesty_internet_hackers_disabled_ok(&self) -> bool {
        self.internet_hackers_disabled > 0
    }

    pub fn record_superweapon_power_reset(&mut self) {
        self.superweapon_power_resets = self.superweapon_power_resets.saturating_add(1);
    }

    pub fn record_fake_detonated(&mut self) {
        self.fake_detonated = self.fake_detonated.saturating_add(1);
    }

    pub fn honesty_superweapon_power_reset_ok(&self) -> bool {
        self.superweapon_power_resets > 0
    }

    pub fn honesty_fake_detonated_ok(&self) -> bool {
        self.fake_detonated > 0
    }

    pub fn record_infiltration_event(&mut self) {
        self.infiltration_events = self.infiltration_events.saturating_add(1);
    }

    pub fn honesty_infiltration_event_ok(&self) -> bool {
        self.infiltration_events > 0
    }

    pub fn record_eva_building_sabotaged(&mut self) {
        self.eva_building_sabotaged = self.eva_building_sabotaged.saturating_add(1);
    }

    pub fn honesty_eva_building_sabotaged_ok(&self) -> bool {
        self.eva_building_sabotaged > 0
    }

    pub fn record_eva_cash_stolen(&mut self) {
        self.eva_cash_stolen = self.eva_cash_stolen.saturating_add(1);
    }

    pub fn honesty_eva_cash_stolen_ok(&self) -> bool {
        self.eva_cash_stolen > 0
    }

    pub fn record_feedback_fx(&mut self) {
        self.feedback_fx = self.feedback_fx.saturating_add(1);
    }

    pub fn honesty_feedback_fx_ok(&self) -> bool {
        self.feedback_fx > 0
    }

    pub fn record_flash_as_selected(&mut self) {
        self.flash_as_selected = self.flash_as_selected.saturating_add(1);
    }

    pub fn honesty_flash_as_selected_ok(&self) -> bool {
        self.flash_as_selected > 0
    }

    pub fn record_cash_floating_texts(&mut self) {
        self.cash_floating_texts = self.cash_floating_texts.saturating_add(1);
    }

    pub fn honesty_cash_floating_texts_ok(&self) -> bool {
        self.cash_floating_texts > 0
    }

    pub fn record_eva_unit_lost(&mut self) {
        self.eva_unit_lost = self.eva_unit_lost.saturating_add(1);
    }

    pub fn honesty_eva_unit_lost_ok(&self) -> bool {
        self.eva_unit_lost > 0
    }

    pub fn record_eva_building_lost(&mut self) {
        self.eva_building_lost = self.eva_building_lost.saturating_add(1);
    }

    pub fn honesty_eva_building_lost_ok(&self) -> bool {
        self.eva_building_lost > 0
    }

    pub fn honesty_sabotage_ok(&self) -> bool {
        self.sabotages > 0
    }

    pub fn honesty_power_ok(&self) -> bool {
        self.power_plants > 0
    }

    pub fn honesty_cash_ok(&self) -> bool {
        self.supply_steals > 0 && self.cash_stolen_total > 0
    }

    pub fn honesty_military_ok(&self) -> bool {
        self.military_disables > 0
    }

    pub fn honesty_superweapon_ok(&self) -> bool {
        self.superweapon_resets > 0
    }

    pub fn honesty_any_ok(&self) -> bool {
        self.honesty_sabotage_ok()
    }
}

/// Whether template is a residual living GLA Saboteur infantry.
///
/// Fail-closed: name residual. Excludes weapons / science / command tokens.
pub fn is_saboteur_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("command")
        || n.contains("button")
        || n.contains("portrait")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("voice")
    {
        return false;
    }
    if n == "testsaboteur" || n == "test_saboteur" || n == "gla_saboteur" {
        return true;
    }
    n.contains("saboteur")
}

/// Classify residual sabotage effect from structure KindOf / template name.
///
/// Order mirrors C++ multi-module CrateCollide first-valid selection residual
/// (name + KindOf gates). Returns None when structure is not a sabotage target.
pub fn classify_sabotage_target(
    template_name: &str,
    is_fs_power: bool,
    is_power_plant: bool,
    is_fs_supply_center: bool,
    is_supply_center: bool,
    is_fs_barracks: bool,
    is_fs_war_factory: bool,
    is_fs_airfield: bool,
    is_fs_superweapon: bool,
    is_fs_strategy_center: bool,
    is_command_center: bool,
    is_fs_internet_center: bool,
    is_fs_fake: bool,
) -> Option<SaboteurEffectKind> {
    let n = template_name.to_ascii_lowercase();

    // Fake buildings first (kill residual).
    if is_fs_fake || n.contains("fake") {
        return Some(SaboteurEffectKind::FakeBuilding);
    }
    // Power plants.
    if is_fs_power
        || is_power_plant
        || n.contains("powerplant")
        || n.contains("power_plant")
        || (n.contains("power") && n.contains("plant"))
    {
        return Some(SaboteurEffectKind::PowerPlant);
    }
    // Supply centers (not drop zones residual — drop zone module commented in retail).
    if is_fs_supply_center
        || is_supply_center
        || n.contains("supplycenter")
        || n.contains("supply_center")
        || n.contains("supplystash")
        || n.contains("supply_stash")
    {
        return Some(SaboteurEffectKind::SupplyCenter);
    }
    // Internet centers before generic factory names.
    if is_fs_internet_center || n.contains("internetcenter") || n.contains("internet_center") {
        return Some(SaboteurEffectKind::InternetCenter);
    }
    // Superweapon / strategy / command.
    if is_fs_superweapon
        || is_fs_strategy_center
        || is_command_center
        || n.contains("superweapon")
        || n.contains("strategycenter")
        || n.contains("strategy_center")
        || n.contains("commandcenter")
        || n.contains("command_center")
        || n.contains("particleuplink")
        || n.contains("nuclearmissile")
        || n.contains("scudstorm")
    {
        return Some(SaboteurEffectKind::SuperweaponOrCommand);
    }
    // Military factories.
    if is_fs_barracks
        || is_fs_war_factory
        || is_fs_airfield
        || n.contains("barracks")
        || n.contains("warfactory")
        || n.contains("war_factory")
        || n.contains("airfield")
        || n.contains("air_field")
        || n.contains("armsdealer")
        || n.contains("arms_dealer")
        || n.contains("warfactory")
        || n.contains("warfact")
    {
        return Some(SaboteurEffectKind::MilitaryFactory);
    }

    None
}

/// Absolute power-sabotage expiry frame residual.
pub fn power_sabotage_until_frame(current_frame: u32) -> u32 {
    current_frame.saturating_add(SABOTEUR_POWER_DURATION_FRAMES)
}

/// Absolute military DISABLED_HACKED expiry frame residual.
pub fn military_disable_until_frame(current_frame: u32) -> u32 {
    current_frame.saturating_add(SABOTEUR_MILITARY_DURATION_FRAMES)
}

/// Absolute internet DISABLED_HACKED expiry frame residual.
pub fn internet_disable_until_frame(current_frame: u32) -> u32 {
    current_frame.saturating_add(SABOTEUR_INTERNET_DURATION_FRAMES)
}

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn saboteur_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * SABOTEUR_LOGIC_FPS / 1000.0).round() as u32
}

// --- Wave 70 residual honesty packs ---

/// Wave 70 residual honesty: Saboteur effect durations / cash residual peel.
pub fn honesty_saboteur_effect_residual_ok() -> bool {
    SABOTEUR_POWER_DURATION_MS == 30_000
        && SABOTEUR_POWER_DURATION_FRAMES == saboteur_ms_to_frames(SABOTEUR_POWER_DURATION_MS)
        && SABOTEUR_POWER_DURATION_FRAMES == 900
        && SABOTEUR_MILITARY_DURATION_MS == 30_000
        && SABOTEUR_MILITARY_DURATION_FRAMES == saboteur_ms_to_frames(SABOTEUR_MILITARY_DURATION_MS)
        && SABOTEUR_MILITARY_DURATION_FRAMES == 900
        && SABOTEUR_INTERNET_DURATION_MS == 15_000
        && SABOTEUR_INTERNET_DURATION_FRAMES == saboteur_ms_to_frames(SABOTEUR_INTERNET_DURATION_MS)
        && SABOTEUR_INTERNET_DURATION_FRAMES == 450
        && SABOTEUR_STEAL_CASH_AMOUNT == 1_000
        && SaboteurEffectKind::SupplyCenter.steals_cash()
        && SaboteurEffectKind::FakeBuilding.destroys_target()
        && SaboteurEffectKind::SuperweaponOrCommand.resets_special_power()
        && SaboteurEffectKind::MilitaryFactory.disabled_hacked_until(0) == Some(900)
        && SaboteurEffectKind::InternetCenter.disabled_hacked_until(0) == Some(450)
        && SaboteurEffectKind::PowerPlant.power_sabotage_until(0) == Some(900)
        && SABOTEUR_SUCCESS_AUDIO == "BuildingSabotaged"
        && SABOTEUR_SHUTDOWN_AUDIO == "SabotageShutDownBuilding"
        && SABOTEUR_RESET_TIMER_AUDIO == "SabotageResetTimerBuilding"
        && SABOTEUR_CASH_STEAL_AUDIO == "MoneyWithdrawSound"
        && SABOTEUR_FLASH_DECAY_FRAMES == 4
        && SaboteurEffectKind::FakeBuilding.feedback_audio().is_none()
        && SaboteurEffectKind::MilitaryFactory.feedback_audio() == Some(SABOTEUR_SHUTDOWN_AUDIO)
        && SaboteurEffectKind::SuperweaponOrCommand.feedback_audio()
            == Some(SABOTEUR_RESET_TIMER_AUDIO)
        && SaboteurEffectKind::SupplyCenter.feedback_audio() == Some(SABOTEUR_CASH_STEAL_AUDIO)
        && SABOTEUR_CASH_STEAL_AUDIO == "MoneyWithdrawSound"
        && SABOTEUR_RESET_TIMER_AUDIO == "SabotageResetTimerBuilding"
}

/// Wave 70 residual honesty: Saboteur body residual peel.
pub fn honesty_saboteur_body_residual_ok() -> bool {
    (SABOTEUR_MAX_HEALTH - 120.0).abs() < 0.01
        && (SABOTEUR_VISION_RANGE - 150.0).abs() < 0.01
        && (SABOTEUR_SHROUD_CLEARING_RANGE - 300.0).abs() < 0.01
        && SABOTEUR_BUILD_COST == 800
        && (SABOTEUR_BUILD_TIME_SEC - 15.0).abs() < 0.01
        && SABOTEUR_BUILD_TIME_FRAMES
            == (SABOTEUR_BUILD_TIME_SEC * SABOTEUR_LOGIC_FPS).round() as u32
        && SABOTEUR_BUILD_TIME_FRAMES == 450
        && SABOTEUR_TRANSPORT_SLOT_COUNT == 1
        && (SABOTEUR_GEOMETRY_RADIUS - 10.0).abs() < 0.01
        && (SABOTEUR_GEOMETRY_HEIGHT - 12.0).abs() < 0.01
        && (SABOTEUR_LOCOMOTOR_SPEED - 30.0).abs() < 0.01
        && (SABOTEUR_LOCOMOTOR_SPEED_DAMAGED - 20.0).abs() < 0.01
        && SABOTEUR_EXPERIENCE_VALUE == [15, 15, 30, 40]
        && !SABOTEUR_IS_TRAINABLE
        && SABOTEUR_STEALTH_DELAY_MS == 2_500
        && SABOTEUR_STEALTH_DELAY_FRAMES == saboteur_ms_to_frames(SABOTEUR_STEALTH_DELAY_MS)
        && SABOTEUR_STEALTH_DELAY_FRAMES == 75
        && is_saboteur_template("GLAInfantrySaboteur")
}

/// Combined Wave 70 Saboteur residual honesty pack.
pub fn honesty_saboteur_residual_pack_ok() -> bool {
    honesty_saboteur_effect_residual_ok() && honesty_saboteur_body_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saboteur_template_matrix() {
        assert!(is_saboteur_template("GLAInfantrySaboteur"));
        assert!(is_saboteur_template("Chem_GLAInfantrySaboteur"));
        assert!(is_saboteur_template("Demo_GLAInfantrySaboteur"));
        assert!(is_saboteur_template("Slth_GLAInfantrySaboteur"));
        assert!(is_saboteur_template("TestSaboteur"));
        assert!(is_saboteur_template("GLA_Saboteur"));
        assert!(!is_saboteur_template("GLAInfantryRebel"));
        assert!(!is_saboteur_template("GLAInfantryHijacker"));
        assert!(!is_saboteur_template("SaboteurWeapon"));
        assert!(!is_saboteur_template(
            "Command_ConstructGLAInfantrySaboteur"
        ));
    }

    #[test]
    fn classify_sabotage_target_matrix() {
        assert_eq!(
            classify_sabotage_target(
                "AmericaPowerPlant",
                true,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false
            ),
            Some(SaboteurEffectKind::PowerPlant)
        );
        assert_eq!(
            classify_sabotage_target(
                "AmericaSupplyCenter",
                false,
                false,
                true,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false
            ),
            Some(SaboteurEffectKind::SupplyCenter)
        );
        assert_eq!(
            classify_sabotage_target(
                "AmericaWarFactory",
                false,
                false,
                false,
                false,
                false,
                true,
                false,
                false,
                false,
                false,
                false,
                false
            ),
            Some(SaboteurEffectKind::MilitaryFactory)
        );
        assert_eq!(
            classify_sabotage_target(
                "AmericaCommandCenter",
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                true,
                false,
                false
            ),
            Some(SaboteurEffectKind::SuperweaponOrCommand)
        );
        assert_eq!(
            classify_sabotage_target(
                "ChinaInternetCenter",
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                true,
                false
            ),
            Some(SaboteurEffectKind::InternetCenter)
        );
        assert_eq!(
            classify_sabotage_target(
                "GLAFakeBarracks",
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                true
            ),
            Some(SaboteurEffectKind::FakeBuilding)
        );
        assert_eq!(
            classify_sabotage_target(
                "AmericaBunker",
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false
            ),
            None
        );
    }

    #[test]
    fn registry_honesty_counters() {
        let mut reg = HostSaboteurRegistry::new();
        assert!(!reg.honesty_any_ok());
        reg.record(SaboteurEffectKind::PowerPlant, 0);
        assert!(reg.honesty_power_ok());
        assert!(reg.honesty_sabotage_ok());
        reg.record(SaboteurEffectKind::SupplyCenter, 1000);
        assert!(reg.honesty_cash_ok());
        assert_eq!(reg.cash_stolen_total, 1000);
        reg.record_consumed();
        assert_eq!(reg.saboteurs_consumed, 1);
    }

    #[test]
    fn duration_frame_constants() {
        assert_eq!(SABOTEUR_POWER_DURATION_FRAMES, 900);
        assert_eq!(SABOTEUR_MILITARY_DURATION_FRAMES, 900);
        assert_eq!(SABOTEUR_INTERNET_DURATION_FRAMES, 450);
        assert_eq!(SABOTEUR_STEAL_CASH_AMOUNT, 1000);
        assert_eq!(power_sabotage_until_frame(10), 910);
        assert_eq!(
            SaboteurEffectKind::MilitaryFactory.disabled_hacked_until(5),
            Some(905)
        );
    }

    #[test]
    fn saboteur_residual_pack_honesty_wave70() {
        assert!(honesty_saboteur_effect_residual_ok());
        assert!(honesty_saboteur_body_residual_ok());
        assert!(honesty_saboteur_residual_pack_ok());
        assert_eq!(saboteur_ms_to_frames(30_000), 900);
        assert_eq!(saboteur_ms_to_frames(15_000), 450);
        assert_eq!(saboteur_ms_to_frames(2_500), 75);
        assert_eq!(SABOTEUR_BUILD_TIME_FRAMES, 450);
        assert_eq!(SABOTEUR_STEAL_CASH_AMOUNT, 1_000);
        assert!(!SABOTEUR_IS_TRAINABLE);
    }
}
