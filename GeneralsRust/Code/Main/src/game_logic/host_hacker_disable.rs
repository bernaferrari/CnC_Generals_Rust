//! Host China Hacker DisableBuilding residual combat polish.
//!
//! Residual slice (playability):
//! - `ChinaInfantryHacker` / Tank_/Nuke_ / Test variants can issue
//!   `SpecialAbilityHackerDisableBuilding` residual:
//!   walk to enemy structure within StartAbilityRange **150** → apply
//!   DISABLED_HACKED for EffectDuration **2000**ms → **60** logic frames.
//! - Disabled structures count as `is_disabled()` so residual production stops
//!   (same path as microwave subdued / EMP).
//! - Internet cash residual remains in `host_hacker_income` (not re-opened).
//!
//! Wave 54 residual pack (retail INI honesty):
//! - SpecialAbilityUpdate: StartAbilityRange **150**, EffectDuration **2000**ms → **60**f,
//!   UnpackTime **7300**ms → **219**f, PackTime **5133**ms → **154**f,
//!   PreparationTime **3000**ms → **90**f, PersistentPrepTime **333**ms → **10**f
//! - SpecialAbilityHackerDisableBuilding ReloadTime **500**ms → **15**f
//! - SpecialObject BinaryDataStream / DisableFX DisabledEffectBinaryShower0
//! - Weapon HackerDisableBuildingHack AttackRange **75**, DamageType HACK
//! - SuperweaponCashHack residual (SCIENCE_CashHack tiers on Command Center):
//!   MoneyAmount **1000**, SCIENCE_CashHack2 **2000**, SCIENCE_CashHack3 **4000**,
//!   ReloadTime **240000**ms → **7200**f, RequiredScience SCIENCE_CashHack1
//! - Target filters: enemy structure, not under construction, not already hacked
//!
//! Fail-closed honesty:
//! - Not full SpecialAbilityUpdate continuous BinaryDataStream attach matrix
//! - Not full DisableFX particle interleave / PrepSoundLoop audio stream
//! - Not full CashHackSpecialPower victim money clamp / floating text path
//! - Not network disable-building replication (network deferred)

use crate::game_logic::host_hacker_income::is_hacker_template;

// Re-export template matcher for integration call sites.
pub use crate::game_logic::host_hacker_income::is_hacker_template as is_hacker_disable_unit;

/// Logic frames per second (host fixed step).
pub const HACKER_DISABLE_LOGIC_FPS: f32 = 30.0;

/// Retail special power template.
pub const SPECIAL_ABILITY_HACKER_DISABLE_BUILDING: &str = "SpecialAbilityHackerDisableBuilding";

/// SpecialAbilityUpdate StartAbilityRange residual.
pub const HACKER_DISABLE_START_ABILITY_RANGE: f32 = 150.0;

/// C++ SpecialAbilityUpdate EffectDuration = 2000 ms for
/// SpecialAbilityHackerDisableBuilding (2 seconds at 30 FPS logic).
pub const HACKER_DISABLE_EFFECT_DURATION_MS: u32 = 2_000;

/// Logic-frame residual of EffectDuration (ms * 30 / 1000).
pub const HACKER_DISABLE_EFFECT_DURATION_FRAMES: u32 =
    (HACKER_DISABLE_EFFECT_DURATION_MS * 30) / 1000;

/// Retail SpecialAbilityHackerDisableBuilding ReloadTime residual (msec).
pub const HACKER_DISABLE_RELOAD_MS: u32 = 500;
/// ReloadTime 500ms → 15 frames @ 30 FPS.
pub const HACKER_DISABLE_RELOAD_FRAMES: u32 = 15;

/// Retail SpecialAbilityUpdate UnpackTime residual (msec).
pub const HACKER_DISABLE_UNPACK_TIME_MS: u32 = 7_300;
/// UnpackTime 7300ms → 219 frames @ 30 FPS.
pub const HACKER_DISABLE_UNPACK_TIME_FRAMES: u32 = 219;

/// Retail SpecialAbilityUpdate PackTime residual (msec).
pub const HACKER_DISABLE_PACK_TIME_MS: u32 = 5_133;
/// PackTime 5133ms → 154 frames @ 30 FPS (round).
pub const HACKER_DISABLE_PACK_TIME_FRAMES: u32 = 154;

/// Retail SpecialAbilityUpdate PreparationTime residual (msec).
pub const HACKER_DISABLE_PREPARATION_TIME_MS: u32 = 3_000;
/// PreparationTime 3000ms → 90 frames @ 30 FPS.
pub const HACKER_DISABLE_PREPARATION_TIME_FRAMES: u32 = 90;

/// Retail SpecialAbilityUpdate PersistentPrepTime residual (msec).
/// Drives how often the disable effect is re-triggered while packing prep.
pub const HACKER_DISABLE_PERSISTENT_PREP_TIME_MS: u32 = 333;
/// PersistentPrepTime 333ms → 10 frames @ 30 FPS (round).
pub const HACKER_DISABLE_PERSISTENT_PREP_TIME_FRAMES: u32 = 10;

/// Retail SpecialObject residual.
pub const HACKER_DISABLE_SPECIAL_OBJECT: &str = "BinaryDataStream";
/// Retail DisableFXParticleSystem residual.
pub const HACKER_DISABLE_FX_PARTICLE: &str = "DisabledEffectBinaryShower0";

/// Residual audio when hacker disables a building.
pub const HACKER_DISABLE_BUILDING_AUDIO: &str = "HackerDisableBuilding";
/// Retail pack / unpack / prep audio residuals.
pub const HACKER_DISABLE_PACK_SOUND: &str = "HackerPack";
pub const HACKER_DISABLE_UNPACK_SOUND: &str = "HackerUnpack";
pub const HACKER_DISABLE_PREP_SOUND_LOOP: &str = "HackerPrepLoop";

/// Retail Weapon HackerDisableBuildingHack AttackRange residual.
pub const HACKER_DISABLE_WEAPON_ATTACK_RANGE: f32 = 75.0;
/// Retail weapon template name.
pub const HACKER_DISABLE_WEAPON: &str = "HackerDisableBuildingHack";
/// Retail DamageType residual marker.
pub const HACKER_DISABLE_DAMAGE_TYPE: &str = "HACK";

// --- SuperweaponCashHack residual (SCIENCE_CashHack tiers; Command Center) ---

/// Retail SuperweaponCashHack special power name.
pub const SUPERWEAPON_CASH_HACK: &str = "SuperweaponCashHack";
/// Retail RequiredScience residual.
pub const SCIENCE_CASH_HACK_1: &str = "SCIENCE_CashHack1";
pub const SCIENCE_CASH_HACK_2: &str = "SCIENCE_CashHack2";
pub const SCIENCE_CASH_HACK_3: &str = "SCIENCE_CashHack3";
/// Retail CashHackSpecialPower MoneyAmount residual (default steal).
pub const CASH_HACK_MONEY_AMOUNT_DEFAULT: i32 = 1_000;
/// Retail UpgradeMoneyAmount SCIENCE_CashHack2 residual.
pub const CASH_HACK_MONEY_AMOUNT_TIER2: i32 = 2_000;
/// Retail UpgradeMoneyAmount SCIENCE_CashHack3 residual.
pub const CASH_HACK_MONEY_AMOUNT_TIER3: i32 = 4_000;
/// Retail SuperweaponCashHack ReloadTime residual (msec).
pub const CASH_HACK_RELOAD_MS: u32 = 240_000;
/// ReloadTime 240000ms → 7200 frames @ 30 FPS.
pub const CASH_HACK_RELOAD_FRAMES: u32 = 7_200;
/// Retail InitiateAtLocationSound residual.
pub const CASH_HACK_ACTIVATE_AUDIO: &str = "CashHackActivate";

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn hacker_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / HACKER_DISABLE_LOGIC_FPS)).round() as u32
}

/// Whether residual unit can issue DisableBuilding special.
pub fn can_activate_hacker_disable_building(is_hacker: bool, is_alive: bool) -> bool {
    is_hacker && is_alive
}

/// Whether target is within StartAbilityRange residual.
pub fn hacker_disable_in_start_range(distance: f32) -> bool {
    distance <= HACKER_DISABLE_START_ABILITY_RANGE
}

/// Legal residual DisableBuilding target (enemy structure, not under construction).
pub fn is_legal_hacker_disable_target(
    is_alive: bool,
    is_structure: bool,
    under_construction: bool,
    is_enemy: bool,
    already_hacked: bool,
) -> bool {
    is_alive && is_structure && !under_construction && is_enemy && !already_hacked
}

/// Absolute expiry frame for residual disable (now + EffectDuration frames).
pub fn hacker_disable_until_frame(current_frame: u32) -> u32 {
    current_frame.saturating_add(HACKER_DISABLE_EFFECT_DURATION_FRAMES)
}

/// Whether residual path should apply for this template.
pub fn should_apply_hacker_disable(template_name: &str) -> bool {
    is_hacker_template(template_name)
}

/// Retail CashHack steal amount for residual science tier.
///
/// C++ CashHackSpecialPower::findAmountToSteal walks upgrades highest-first;
/// residual: tier3 → 4000, tier2 → 2000, else default 1000.
pub fn cash_hack_money_for_science_tier(tier: u8) -> i32 {
    match tier {
        3 => CASH_HACK_MONEY_AMOUNT_TIER3,
        2 => CASH_HACK_MONEY_AMOUNT_TIER2,
        _ => CASH_HACK_MONEY_AMOUNT_DEFAULT,
    }
}

/// Name residual for SCIENCE_CashHack* markers.
pub fn is_cash_hack_science_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n == "science_cashhack1"
        || n == "science_cashhack2"
        || n == "science_cashhack3"
        || n.contains("cashhack")
}

/// Wave 54 residual honesty: disable duration / range / reload residual.
pub fn honesty_hacker_disable_duration_range_residual_ok() -> bool {
    (HACKER_DISABLE_START_ABILITY_RANGE - 150.0).abs() < 0.01
        && HACKER_DISABLE_EFFECT_DURATION_MS == 2_000
        && HACKER_DISABLE_EFFECT_DURATION_FRAMES == 60
        && HACKER_DISABLE_EFFECT_DURATION_FRAMES == (HACKER_DISABLE_EFFECT_DURATION_MS * 30) / 1000
        && HACKER_DISABLE_RELOAD_MS == 500
        && HACKER_DISABLE_RELOAD_FRAMES == hacker_ms_to_frames(HACKER_DISABLE_RELOAD_MS)
        && SPECIAL_ABILITY_HACKER_DISABLE_BUILDING == "SpecialAbilityHackerDisableBuilding"
}

/// Wave 54 residual honesty: pack / unpack / prep residual.
pub fn honesty_hacker_disable_pack_unpack_residual_ok() -> bool {
    HACKER_DISABLE_UNPACK_TIME_MS == 7_300
        && HACKER_DISABLE_UNPACK_TIME_FRAMES == hacker_ms_to_frames(HACKER_DISABLE_UNPACK_TIME_MS)
        && HACKER_DISABLE_PACK_TIME_MS == 5_133
        && HACKER_DISABLE_PACK_TIME_FRAMES == hacker_ms_to_frames(HACKER_DISABLE_PACK_TIME_MS)
        && HACKER_DISABLE_PREPARATION_TIME_MS == 3_000
        && HACKER_DISABLE_PREPARATION_TIME_FRAMES
            == hacker_ms_to_frames(HACKER_DISABLE_PREPARATION_TIME_MS)
        && HACKER_DISABLE_PERSISTENT_PREP_TIME_MS == 333
        && HACKER_DISABLE_PERSISTENT_PREP_TIME_FRAMES
            == hacker_ms_to_frames(HACKER_DISABLE_PERSISTENT_PREP_TIME_MS)
        && HACKER_DISABLE_SPECIAL_OBJECT == "BinaryDataStream"
        && HACKER_DISABLE_FX_PARTICLE == "DisabledEffectBinaryShower0"
}

/// Wave 54 residual honesty: weapon residual + target filters.
pub fn honesty_hacker_disable_weapon_target_residual_ok() -> bool {
    (HACKER_DISABLE_WEAPON_ATTACK_RANGE - 75.0).abs() < 0.01
        && HACKER_DISABLE_WEAPON == "HackerDisableBuildingHack"
        && HACKER_DISABLE_DAMAGE_TYPE == "HACK"
        && !HACKER_DISABLE_BUILDING_AUDIO.is_empty()
        && HACKER_DISABLE_PACK_SOUND == "HackerPack"
        && HACKER_DISABLE_UNPACK_SOUND == "HackerUnpack"
        && HACKER_DISABLE_PREP_SOUND_LOOP == "HackerPrepLoop"
        && is_legal_hacker_disable_target(true, true, false, true, false)
        && !is_legal_hacker_disable_target(true, true, true, true, false)
        && !is_legal_hacker_disable_target(true, false, false, true, false)
}

/// Wave 54 residual honesty: SCIENCE_CashHack money tiers + reload residual.
pub fn honesty_cash_hack_science_tier_residual_ok() -> bool {
    SUPERWEAPON_CASH_HACK == "SuperweaponCashHack"
        && SCIENCE_CASH_HACK_1 == "SCIENCE_CashHack1"
        && SCIENCE_CASH_HACK_2 == "SCIENCE_CashHack2"
        && SCIENCE_CASH_HACK_3 == "SCIENCE_CashHack3"
        && CASH_HACK_MONEY_AMOUNT_DEFAULT == 1_000
        && CASH_HACK_MONEY_AMOUNT_TIER2 == 2_000
        && CASH_HACK_MONEY_AMOUNT_TIER3 == 4_000
        && cash_hack_money_for_science_tier(1) == 1_000
        && cash_hack_money_for_science_tier(2) == 2_000
        && cash_hack_money_for_science_tier(3) == 4_000
        && CASH_HACK_RELOAD_MS == 240_000
        && CASH_HACK_RELOAD_FRAMES == hacker_ms_to_frames(CASH_HACK_RELOAD_MS)
        && CASH_HACK_ACTIVATE_AUDIO == "CashHackActivate"
        && is_cash_hack_science_name("SCIENCE_CashHack1")
        && is_cash_hack_science_name("SCIENCE_CashHack3")
        && !is_cash_hack_science_name("SCIENCE_Pathfinder")
}

/// Combined Wave 54 hacker disable residual honesty pack.
pub fn honesty_hacker_disable_residual_pack_ok() -> bool {
    honesty_hacker_disable_duration_range_residual_ok()
        && honesty_hacker_disable_pack_unpack_residual_ok()
        && honesty_hacker_disable_weapon_target_residual_ok()
        && honesty_cash_hack_science_tier_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_and_range() {
        assert_eq!(HACKER_DISABLE_EFFECT_DURATION_FRAMES, 60);
        assert!(hacker_disable_in_start_range(150.0));
        assert!(hacker_disable_in_start_range(0.0));
        assert!(!hacker_disable_in_start_range(150.1));
        assert_eq!(hacker_disable_until_frame(100), 160);
    }

    #[test]
    fn legal_target_matrix() {
        assert!(is_legal_hacker_disable_target(
            true, true, false, true, false
        ));
        assert!(!is_legal_hacker_disable_target(
            false, true, false, true, false
        ));
        assert!(!is_legal_hacker_disable_target(
            true, false, false, true, false
        ));
        assert!(!is_legal_hacker_disable_target(
            true, true, true, true, false
        ));
        assert!(!is_legal_hacker_disable_target(
            true, true, false, false, false
        ));
        assert!(!is_legal_hacker_disable_target(
            true, true, false, true, true
        ));
    }

    #[test]
    fn unit_names() {
        assert!(should_apply_hacker_disable("ChinaInfantryHacker"));
        assert!(should_apply_hacker_disable("Tank_ChinaInfantryHacker"));
        assert!(should_apply_hacker_disable("Nuke_ChinaInfantryHacker"));
        assert!(should_apply_hacker_disable("TestHacker"));
        assert!(!should_apply_hacker_disable("ChinaInfantryBlackLotus"));
        assert!(!should_apply_hacker_disable("ChinaTankBattleMaster"));
        assert!(can_activate_hacker_disable_building(true, true));
        assert!(!can_activate_hacker_disable_building(true, false));
        assert!(!can_activate_hacker_disable_building(false, true));
    }

    #[test]
    fn hacker_disable_residual_pack_honesty() {
        assert!(honesty_hacker_disable_residual_pack_ok());
        assert_eq!(hacker_ms_to_frames(7_300), 219);
        assert_eq!(hacker_ms_to_frames(5_133), 154);
        assert_eq!(hacker_ms_to_frames(3_000), 90);
        assert_eq!(hacker_ms_to_frames(333), 10);
        assert_eq!(hacker_ms_to_frames(500), 15);
        assert_eq!(hacker_ms_to_frames(240_000), 7_200);
    }

    #[test]
    fn cash_hack_tier_amounts() {
        assert_eq!(cash_hack_money_for_science_tier(0), 1_000);
        assert_eq!(cash_hack_money_for_science_tier(1), 1_000);
        assert_eq!(cash_hack_money_for_science_tier(2), 2_000);
        assert_eq!(cash_hack_money_for_science_tier(3), 4_000);
        assert!(CASH_HACK_MONEY_AMOUNT_TIER3 > CASH_HACK_MONEY_AMOUNT_TIER2);
        assert!(CASH_HACK_MONEY_AMOUNT_TIER2 > CASH_HACK_MONEY_AMOUNT_DEFAULT);
    }
}
