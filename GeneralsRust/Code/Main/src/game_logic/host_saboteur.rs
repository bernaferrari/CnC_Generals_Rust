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

/// Residual audio when sabotage succeeds (building sabotaged cue).
pub const SABOTEUR_SUCCESS_AUDIO: &str = "BuildingSabotaged";
/// Residual cash-steal audio honesty.
pub const SABOTEUR_CASH_STEAL_AUDIO: &str = "MoneyWithdrawSound";
/// Residual superweapon timer-reset audio honesty.
pub const SABOTEUR_RESET_TIMER_AUDIO: &str = "SabotageResetTimerBuilding";

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
            Self::MilitaryFactory => Some(current_frame.saturating_add(SABOTEUR_MILITARY_DURATION_FRAMES)),
            Self::InternetCenter => Some(current_frame.saturating_add(SABOTEUR_INTERNET_DURATION_FRAMES)),
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
    if is_fs_internet_center
        || n.contains("internetcenter")
        || n.contains("internet_center")
    {
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
        assert!(!is_saboteur_template("Command_ConstructGLAInfantrySaboteur"));
    }

    #[test]
    fn classify_sabotage_target_matrix() {
        assert_eq!(
            classify_sabotage_target(
                "AmericaPowerPlant", true, true, false, false, false, false, false, false, false,
                false, false, false
            ),
            Some(SaboteurEffectKind::PowerPlant)
        );
        assert_eq!(
            classify_sabotage_target(
                "AmericaSupplyCenter", false, false, true, true, false, false, false, false, false,
                false, false, false
            ),
            Some(SaboteurEffectKind::SupplyCenter)
        );
        assert_eq!(
            classify_sabotage_target(
                "AmericaWarFactory", false, false, false, false, false, true, false, false, false,
                false, false, false
            ),
            Some(SaboteurEffectKind::MilitaryFactory)
        );
        assert_eq!(
            classify_sabotage_target(
                "AmericaCommandCenter", false, false, false, false, false, false, false, false,
                false, true, false, false
            ),
            Some(SaboteurEffectKind::SuperweaponOrCommand)
        );
        assert_eq!(
            classify_sabotage_target(
                "ChinaInternetCenter", false, false, false, false, false, false, false, false,
                false, false, true, false
            ),
            Some(SaboteurEffectKind::InternetCenter)
        );
        assert_eq!(
            classify_sabotage_target(
                "GLAFakeBarracks", false, false, false, false, false, false, false, false, false,
                false, false, true
            ),
            Some(SaboteurEffectKind::FakeBuilding)
        );
        assert_eq!(
            classify_sabotage_target(
                "AmericaBunker", false, false, false, false, false, false, false, false, false,
                false, false, false
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
}
