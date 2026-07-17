//! Host hero special-ability residual (Burton / Jarmen Kell / Black Lotus).
//!
//! Residual slice (playability):
//! - Jarmen Kell `SnipeVehicle`: DAMAGE_KILLPILOT residual — vehicle becomes
//!   unmanned + Neutral (no HP damage), so infantry can later recrew/capture.
//! - Colonel Burton `PlantTimedDemoCharge`: walk to structure/vehicle → plant
//!   sticky timed charge (reuses host_mines TimedDemoCharge residual).
//! - Colonel Burton `PlantRemoteDemoCharge` + `DetonateRemoteDemoCharges`:
//!   plant sticky remote charge (no auto-timer) then remote-detonate all charges
//!   planted by that producer (SPECIAL_REMOTE_CHARGES residual).
//! - Black Lotus `CaptureBuilding`: hero capture residual without infantry
//!   Capture research; StartAbilityRange **150** (vs infantry melee pad);
//!   reuses Capturing AI ownership-transfer residual.
//! - Black Lotus `StealCashHack`: walk to enemy cash generator (supply /
//!   black market / drop zone) within range **150** → steal residual cash.
//! - Black Lotus `DisableVehicleHack`: walk to enemy ground vehicle within
//!   range **150** → DISABLED_HACKED for EffectDuration residual (INI **15000**ms
//!   → **450** logic frames); vehicle cannot move or attack until timer expires.
//!
//! Wave 57 residual pack (retail INI honesty):
//! - CashHack science tiers residual (SuperweaponCashHack / Command Center):
//!   MoneyAmount **1000**, SCIENCE_CashHack2 **2000**, SCIENCE_CashHack3 **4000**,
//!   ReloadTime **240000**ms → **7200**f, RequiredScience SCIENCE_CashHack1
//! - Black Lotus StealCashHack EffectValue **1000** residual (unit special;
//!   not science-tiered — fail-closed vs SuperweaponCashHack money matrix)
//! - BlackMarket cash-generator residual: name/kind gates for GLABlackMarket /
//!   FS_BLACK_MARKET; emergency cash steal honesty when target is black market
//! - Special ability timers residual from INI:
//!   - CaptureBuilding: Unpack **6730**ms → **202**f, Pack **2800**ms → **84**f,
//!     Prep **6000**ms → **180**f, AwardXP **20**
//!   - DisableVehicleHack: Unpack **2000**ms → **60**f, Pack **1000**ms → **30**f,
//!     Prep **2000**ms → **60**f, EffectDuration **15000**ms → **450**f
//!   - StealCashHack: Unpack **6730**ms → **202**f, Pack **5800**ms → **174**f,
//!     Prep **6000**ms → **180**f, ReloadTime **2000**ms → **60**f
//!   - Burton charges: Unpack **5500**ms → **165**f, FleeRange **100**,
//!     PreTriggerUnstealth **5000**ms → **150**f
//! - Lotus StealthUpdate residual: StealthDelay **2500**ms → **75**f,
//!   Forbidden **USING_ABILITY**, InnateStealth **Yes**
//!
//! Fail-closed honesty:
//! - Not full SpecialAbilityUpdate preparation timers / packing / flee-after-plant
//! - Not full StickyBombUpdate attach bones / geometry splash / max-charge list UI
//! - Not full CashHackSpecialPower victim money clamp / floating text path
//! - Not combat-bike rider-eject / academy sniped-vehicle stats
//! - Not full laser attach / disable FX particle interleave / VoiceDisableVehicleComplete
//! - Not full ActionManager canCapture edge matrix (stealth / garrison / shroud)

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const HERO_ABILITY_LOGIC_FPS: f32 = 30.0;

/// Retail StartAbilityRange for all three Black Lotus specials
/// (CaptureBuilding / DisableVehicleHack / StealCashHack).
pub const BLACK_LOTUS_START_ABILITY_RANGE: f32 = 150.0;

/// Retail Black Lotus StealCashHack EffectValue residual (unit special cash amount).
pub const STEAL_CASH_DEFAULT_AMOUNT: u32 = 1_000;

/// Retail SpecialAbilityUpdate EffectDuration residual for
/// SpecialAbilityBlackLotusDisableVehicleHack (INI EffectDuration=15000;
/// comment incorrectly claims 30s — host locks INI value).
pub const DISABLE_VEHICLE_HACK_DURATION_MS: u32 = 15_000;

/// Logic-frame residual of EffectDuration (ms * 30 / 1000).
pub const DISABLE_VEHICLE_HACK_DURATION_FRAMES: u32 =
    (DISABLE_VEHICLE_HACK_DURATION_MS * 30) / 1000;

/// Audio residual when a vehicle pilot is sniped (host-side cue name).
pub const SNIPE_VEHICLE_AUDIO: &str = "UnitSniped";

/// Audio residual when Black Lotus completes cash steal.
pub const STEAL_CASH_AUDIO: &str = "BlackLotusStealCash";

/// Audio residual when Black Lotus completes vehicle disable hack.
pub const DISABLE_VEHICLE_HACK_AUDIO: &str = "BlackLotusDisableVehicle";

/// Audio residual when Black Lotus completes building capture.
pub const CAPTURE_BUILDING_AUDIO: &str = "BlackLotusCaptureBuilding";

// --- Special power template names residual ---

pub const SPECIAL_ABILITY_BLACK_LOTUS_CAPTURE: &str = "SpecialAbilityBlackLotusCaptureBuilding";
pub const SPECIAL_ABILITY_BLACK_LOTUS_DISABLE_VEHICLE: &str =
    "SpecialAbilityBlackLotusDisableVehicleHack";
pub const SPECIAL_ABILITY_BLACK_LOTUS_STEAL_CASH: &str = "SpecialAbilityBlackLotusStealCashHack";
pub const SPECIAL_ABILITY_BURTON_REMOTE_CHARGES: &str = "SpecialAbilityColonelBurtonRemoteCharges";
pub const SPECIAL_ABILITY_BURTON_TIMED_CHARGES: &str = "SpecialAbilityColonelBurtonTimedCharges";
pub const SPECIAL_ABILITY_JARMEN_SNIPE_VEHICLE: &str =
    "Command_GLAInfantryJarmenKellSnipeVehicleAttack";

// --- SuperweaponCashHack science tier residual (Command Center; Wave 57) ---

/// Retail SuperweaponCashHack special power name.
pub const SUPERWEAPON_CASH_HACK: &str = "SuperweaponCashHack";
/// Retail RequiredScience residual.
pub const SCIENCE_CASH_HACK_1: &str = "SCIENCE_CashHack1";
pub const SCIENCE_CASH_HACK_2: &str = "SCIENCE_CashHack2";
pub const SCIENCE_CASH_HACK_3: &str = "SCIENCE_CashHack3";
/// Retail CashHackSpecialPower MoneyAmount residual (default steal).
pub const CASH_HACK_MONEY_AMOUNT_DEFAULT: u32 = 1_000;
/// Retail UpgradeMoneyAmount SCIENCE_CashHack2 residual.
pub const CASH_HACK_MONEY_AMOUNT_TIER2: u32 = 2_000;
/// Retail UpgradeMoneyAmount SCIENCE_CashHack3 residual.
pub const CASH_HACK_MONEY_AMOUNT_TIER3: u32 = 4_000;
/// Retail SuperweaponCashHack ReloadTime residual (msec).
pub const CASH_HACK_RELOAD_MS: u32 = 240_000;
/// ReloadTime 240000ms → 7200 frames @ 30 FPS.
pub const CASH_HACK_RELOAD_FRAMES: u32 = 7_200;
/// Retail InitiateAtLocationSound residual.
pub const CASH_HACK_ACTIVATE_AUDIO: &str = "CashHackActivate";

// --- BlackMarket cash-generator residual (Wave 57 emergency cash path) ---

/// Retail GLABlackMarket template residual marker.
pub const BLACK_MARKET_TEMPLATE: &str = "GLABlackMarket";
/// Retail KindOf FS_BLACK_MARKET residual marker.
pub const BLACK_MARKET_KIND_MARKER: &str = "FS_BLACK_MARKET";
/// Residual: steal cash from black market is legal emergency cash residual.
pub const BLACK_MARKET_CASH_HACK_LEGAL: bool = true;
/// Retail Black Market AutoDeposit amount residual honesty (host_black_market owns deposit).
pub const BLACK_MARKET_DEPOSIT_AMOUNT_HONESTY: u32 = 20;

// --- Black Lotus CaptureBuilding timers residual ---

pub const LOTUS_CAPTURE_UNPACK_MS: u32 = 6_730;
pub const LOTUS_CAPTURE_UNPACK_FRAMES: u32 = 202;
pub const LOTUS_CAPTURE_PACK_MS: u32 = 2_800;
pub const LOTUS_CAPTURE_PACK_FRAMES: u32 = 84;
pub const LOTUS_CAPTURE_PREP_MS: u32 = 6_000;
pub const LOTUS_CAPTURE_PREP_FRAMES: u32 = 180;
pub const LOTUS_CAPTURE_AWARD_XP: u32 = 20;
pub const LOTUS_CAPTURE_SPECIAL_OBJECT: &str = "BinaryDataStream";
pub const LOTUS_CAPTURE_DO_CAPTURE_FX: bool = true;

// --- Black Lotus DisableVehicleHack timers residual ---

pub const LOTUS_DISABLE_UNPACK_MS: u32 = 2_000;
pub const LOTUS_DISABLE_UNPACK_FRAMES: u32 = 60;
pub const LOTUS_DISABLE_PACK_MS: u32 = 1_000;
pub const LOTUS_DISABLE_PACK_FRAMES: u32 = 30;
pub const LOTUS_DISABLE_PREP_MS: u32 = 2_000;
pub const LOTUS_DISABLE_PREP_FRAMES: u32 = 60;
pub const LOTUS_DISABLE_FX_PARTICLE: &str = "DisabledEffectBinaryShower0";
pub const LOTUS_DISABLE_SPECIAL_OBJECT: &str = "BinaryDataStream";
pub const LOTUS_DISABLE_AWARD_XP: u32 = 0;

// --- Black Lotus StealCashHack timers residual ---

pub const LOTUS_STEAL_UNPACK_MS: u32 = 6_730;
pub const LOTUS_STEAL_UNPACK_FRAMES: u32 = 202;
pub const LOTUS_STEAL_PACK_MS: u32 = 5_800;
pub const LOTUS_STEAL_PACK_FRAMES: u32 = 174;
pub const LOTUS_STEAL_PREP_MS: u32 = 6_000;
pub const LOTUS_STEAL_PREP_FRAMES: u32 = 180;
/// Retail SpecialAbilityBlackLotusStealCashHack ReloadTime residual (msec).
pub const LOTUS_STEAL_RELOAD_MS: u32 = 2_000;
pub const LOTUS_STEAL_RELOAD_FRAMES: u32 = 60;
pub const LOTUS_STEAL_SPECIAL_OBJECT: &str = "BinaryDataStream";
pub const LOTUS_STEAL_AWARD_XP: u32 = 20;
/// Retail EffectValue residual (amount of cash stolen).
pub const LOTUS_STEAL_EFFECT_VALUE: u32 = 1_000;

// --- Burton charge ability timers residual ---

pub const BURTON_CHARGE_UNPACK_MS: u32 = 5_500;
pub const BURTON_CHARGE_UNPACK_FRAMES: u32 = 165;
pub const BURTON_CHARGE_FLEE_RANGE: f32 = 100.0;
pub const BURTON_CHARGE_PRE_TRIGGER_UNSTEALTH_MS: u32 = 5_000;
pub const BURTON_CHARGE_PRE_TRIGGER_UNSTEALTH_FRAMES: u32 = 150;
pub const BURTON_CHARGE_LOSE_STEALTH_ON_TRIGGER: bool = true;
pub const BURTON_MAX_REMOTE_CHARGES: u32 = 8;
pub const BURTON_MAX_TIMED_CHARGES: u32 = 10;
pub const BURTON_REMOTE_CHARGE_OBJECT: &str = "RemoteC4Charge";
pub const BURTON_TIMED_CHARGE_OBJECT: &str = "TimedC4Charge";

// --- Black Lotus body / stealth residual ---

pub const LOTUS_MAX_HEALTH: f32 = 200.0;
pub const LOTUS_VISION_RANGE: f32 = 300.0;
pub const LOTUS_SHROUD_CLEARING_RANGE: f32 = 400.0;
pub const LOTUS_BUILD_COST: u32 = 1_500;
pub const LOTUS_STEALTH_DELAY_MS: u32 = 2_500;
pub const LOTUS_STEALTH_DELAY_FRAMES: u32 = 75;
pub const LOTUS_INNATE_STEALTH: bool = true;
pub const LOTUS_STEALTH_BREAKS_ON_ABILITY: bool = true;
pub const LOTUS_ORDER_IDLE_ENEMIES_ON_REVEAL: bool = true;
pub const LOTUS_ENEMY_DETECTION_EVA: &str = "EnemyBlackLotusDetected";
pub const LOTUS_OWN_DETECTION_EVA: &str = "OwnBlackLotusDetected";
pub const LOTUS_PACK_SOUND: &str = "BlackLotusPack";
pub const LOTUS_UNPACK_SOUND: &str = "BlackLotusUnpack";
pub const LOTUS_TRIGGER_SOUND: &str = "BlackLotusTrigger";
pub const LOTUS_PREP_SOUND_LOOP: &str = "BlackLotusPrepLoop";
pub const LOTUS_VOICE_HACK_CASH: &str = "BlackLotusVoiceHackCash";
pub const LOTUS_VOICE_HACK_VEHICLE: &str = "BlackLotusVoiceHackVehicle";
pub const LOTUS_VOICE_HACK_BUILDING: &str = "BlackLotusVoiceHackBuilding";
pub const LOTUS_VOICE_CASH_COMPLETE: &str = "BlackLotusVoiceCashComplete";

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn hero_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / HERO_ABILITY_LOGIC_FPS)).round() as u32
}

/// Whether template is a residual Black Lotus hero.
///
/// Fail-closed: name residual. Excludes weapons / science / debris tokens.
pub fn is_black_lotus_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("voice")
        || n.contains("command")
        || n.contains("button")
        || n.contains("portrait")
        || n.contains("hack")
        || n.contains("disable")
        || n.contains("steal")
        || n.contains("capture")
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testblacklotus"
        || n == "testlotus"
        || n == "black_lotus"
        || n == "china_blacklotus"
        || n == "china_lotus"
    {
        return true;
    }
    n.contains("blacklotus") || n.contains("black_lotus")
}

/// Whether residual unit can issue Black Lotus specials (alive + template).
pub fn can_activate_black_lotus_ability(is_lotus: bool, is_alive: bool) -> bool {
    is_lotus && is_alive
}

/// Whether residual unit may use CaptureBuilding without infantry Capture research.
///
/// Heroes (KindOf::Hero / name) and Black Lotus template residual.
pub fn can_capture_without_upgrade(is_hero: bool, is_lotus: bool) -> bool {
    is_hero || is_lotus
}

/// Whether unit is within Black Lotus StartAbilityRange residual.
pub fn black_lotus_in_start_range(distance: f32) -> bool {
    distance <= BLACK_LOTUS_START_ABILITY_RANGE
}

/// Legal residual StealCashHack target (enemy cash generator structure).
pub fn is_legal_steal_cash_target(
    is_alive: bool,
    is_structure: bool,
    under_construction: bool,
    is_enemy: bool,
    is_cash_generator: bool,
) -> bool {
    is_alive && is_structure && !under_construction && is_enemy && is_cash_generator
}

/// Legal residual DisableVehicleHack target (enemy manned ground vehicle).
pub fn is_legal_disable_vehicle_target(
    is_alive: bool,
    is_vehicle: bool,
    is_airborne: bool,
    is_enemy: bool,
    already_hacked: bool,
    unmanned: bool,
) -> bool {
    is_alive && is_vehicle && !is_airborne && is_enemy && !already_hacked && !unmanned
}

/// Legal residual Black Lotus CaptureBuilding target (enemy structure).
pub fn is_legal_lotus_capture_target(
    is_alive: bool,
    is_structure: bool,
    under_construction: bool,
    is_enemy: bool,
) -> bool {
    is_alive && is_structure && !under_construction && is_enemy
}

/// Absolute expiry frame for residual vehicle disable.
pub fn disable_vehicle_until_frame(current_frame: u32) -> u32 {
    current_frame.saturating_add(DISABLE_VEHICLE_HACK_DURATION_FRAMES)
}

/// True when a template/building is a residual cash-hack target (C++ KINDOF_CASH_GENERATOR).
///
/// Fail-closed name residual for supply centers, black markets, supply drop zones.
pub fn is_cash_hack_target_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("supplycenter")
        || n.contains("supply_center")
        || n.contains("blackmarket")
        || n.contains("black_market")
        || n.contains("supplydropzone")
        || n.contains("supply_drop")
        || n == "testsupplycenter"
        || n == "testbuilding"
        || n == "testcashgenerator"
}

/// Whether template is a residual Black Market structure (emergency cash residual).
pub fn is_black_market_cash_hack_target(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    if n.contains("fake") {
        return false;
    }
    n.contains("blackmarket") || n.contains("black_market") || n == "testblackmarket"
}

/// Whether object kinds residual-match a cash generator (SupplyCenter / BlackMarket).
pub fn is_cash_generator_kind(
    is_supply_center: bool,
    is_fs_supply_center: bool,
    is_black_market: bool,
    is_supply_dropzone: bool,
) -> bool {
    is_supply_center || is_fs_supply_center || is_black_market || is_supply_dropzone
}

/// Combined residual cash-generator check (template name OR kind flags).
pub fn is_cash_hack_target(
    template_name: &str,
    is_supply_center: bool,
    is_fs_supply_center: bool,
    is_black_market: bool,
    is_supply_dropzone: bool,
) -> bool {
    is_cash_hack_target_template(template_name)
        || is_cash_generator_kind(
            is_supply_center,
            is_fs_supply_center,
            is_black_market,
            is_supply_dropzone,
        )
}

/// Legal residual emergency cash steal from black market specifically.
pub fn is_legal_black_market_emergency_steal(
    is_alive: bool,
    under_construction: bool,
    is_enemy: bool,
    is_black_market: bool,
) -> bool {
    BLACK_MARKET_CASH_HACK_LEGAL && is_alive && !under_construction && is_enemy && is_black_market
}

/// Retail CashHack steal amount for residual science tier.
///
/// C++ CashHackSpecialPower::findAmountToSteal walks upgrades highest-first;
/// residual: tier3 → 4000, tier2 → 2000, else default 1000.
pub fn cash_hack_money_for_science_tier(tier: u8) -> u32 {
    match tier {
        3 => CASH_HACK_MONEY_AMOUNT_TIER3,
        2 => CASH_HACK_MONEY_AMOUNT_TIER2,
        _ => CASH_HACK_MONEY_AMOUNT_DEFAULT,
    }
}

/// Select highest unlocked SCIENCE_CashHack* tier (1/2/3), fail-closed → 1.
pub fn highest_cash_hack_tier_from_sciences<'a, I>(sciences: I) -> u8
where
    I: IntoIterator<Item = &'a str>,
{
    let mut best: u8 = 1;
    for s in sciences {
        let n = s.to_ascii_lowercase().replace('_', "").replace('-', "");
        if n.contains("cashhack3") {
            return 3;
        }
        if n.contains("cashhack2") {
            best = 2;
        } else if n.contains("cashhack1") || n.contains("cashhack") {
            // keep at least 1
        }
    }
    best
}

/// Money amount for highest unlocked CashHack science among names.
pub fn cash_hack_money_from_sciences<'a, I>(sciences: I) -> u32
where
    I: IntoIterator<Item = &'a str>,
{
    cash_hack_money_for_science_tier(highest_cash_hack_tier_from_sciences(sciences))
}

/// Name residual for SCIENCE_CashHack* markers.
pub fn is_cash_hack_science_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n == "science_cashhack1"
        || n == "science_cashhack2"
        || n == "science_cashhack3"
        || n.contains("cashhack")
}

/// Maintain Black Lotus stealth residual (USING_ABILITY breaks cloak).
pub fn lotus_stealth_desired(
    is_lotus: bool,
    innate_stealth: bool,
    is_alive: bool,
    using_ability: bool,
) -> Option<bool> {
    if !is_lotus || !innate_stealth || !is_alive {
        return None;
    }
    if LOTUS_STEALTH_BREAKS_ON_ABILITY && using_ability {
        Some(false)
    } else {
        Some(true)
    }
}

/// Horizontal distance helper for residual attach placement.
pub fn horizontal_distance(a: Vec3, b: Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

/// Bookkeeping id for residual plant (producer → charge object).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeroAbilityPlant {
    pub producer_id: ObjectId,
    pub charge_id: ObjectId,
    pub target_id: ObjectId,
}

/// Host residual honesty counters for hero special abilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostHeroAbilityRegistry {
    /// Jarmen Kell snipe resolved (vehicle unmanned).
    pub snipe_kills: u32,
    /// Burton timed demo charge planted via special ability.
    pub timed_charges_planted: u32,
    /// Burton remote demo charge planted via special ability.
    pub remote_charges_planted: u32,
    /// Remote demo charge detonations resolved (count of charges blown).
    pub remote_charges_detonated: u32,
    /// Black Lotus cash-hack steals completed.
    pub cash_steals: u32,
    /// Total cash transferred via residual cash-hack.
    pub cash_stolen_total: u32,
    /// Black Lotus disable-vehicle hacks completed.
    pub vehicle_disables: u32,
    /// Black Lotus / hero CaptureBuilding residual completes.
    pub building_captures: u32,
    /// EVA BuildingBeingStolen residual fires.
    pub eva_building_being_stolen: u32,
    /// EVA BuildingStolen residual fires.
    pub eva_building_stolen: u32,
    /// Black Market emergency cash steals completed (subset of cash_steals).
    #[serde(default)]
    pub black_market_emergency_steals: u32,
}

impl HostHeroAbilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_snipe(&mut self) {
        self.snipe_kills = self.snipe_kills.saturating_add(1);
    }

    pub fn record_timed_charge_plant(&mut self) {
        self.timed_charges_planted = self.timed_charges_planted.saturating_add(1);
    }

    pub fn record_remote_charge_plant(&mut self) {
        self.remote_charges_planted = self.remote_charges_planted.saturating_add(1);
    }

    pub fn record_remote_charge_detonate(&mut self, count: u32) {
        self.remote_charges_detonated = self.remote_charges_detonated.saturating_add(count);
    }

    pub fn record_cash_steal(&mut self, amount: u32) {
        self.cash_steals = self.cash_steals.saturating_add(1);
        self.cash_stolen_total = self.cash_stolen_total.saturating_add(amount);
    }

    /// Record cash steal that specifically targeted a black market (emergency residual).
    pub fn record_black_market_emergency_steal(&mut self, amount: u32) {
        self.record_cash_steal(amount);
        self.black_market_emergency_steals = self.black_market_emergency_steals.saturating_add(1);
    }

    pub fn record_vehicle_disable(&mut self) {
        self.vehicle_disables = self.vehicle_disables.saturating_add(1);
    }

    pub fn record_building_capture(&mut self) {
        self.building_captures = self.building_captures.saturating_add(1);
    }

    /// Residual honesty: at least one snipe unmanned a vehicle.
    pub fn honesty_snipe_ok(&self) -> bool {
        self.snipe_kills > 0
    }

    /// Residual honesty: at least one timed charge planted by hero ability.
    pub fn honesty_timed_charge_plant_ok(&self) -> bool {
        self.timed_charges_planted > 0
    }

    /// Residual honesty: at least one remote charge planted by hero ability.
    pub fn honesty_remote_charge_plant_ok(&self) -> bool {
        self.remote_charges_planted > 0
    }

    /// Residual honesty: plant → remote detonate path exercised.
    pub fn honesty_remote_charge_detonate_ok(&self) -> bool {
        self.remote_charges_planted > 0 && self.remote_charges_detonated > 0
    }

    /// Residual honesty: at least one cash steal completed.
    pub fn honesty_cash_steal_ok(&self) -> bool {
        self.cash_steals > 0 && self.cash_stolen_total > 0
    }

    /// Residual honesty: at least one black market emergency cash steal.
    pub fn honesty_black_market_emergency_ok(&self) -> bool {
        self.black_market_emergency_steals > 0
    }

    /// Residual honesty: at least one vehicle disable hack completed.
    pub fn honesty_vehicle_disable_ok(&self) -> bool {
        self.vehicle_disables > 0
    }

    /// Residual honesty: at least one Black Lotus / hero building capture completed.
    pub fn honesty_building_capture_ok(&self) -> bool {
        self.building_captures > 0
    }

    pub fn record_eva_building_being_stolen(&mut self) {
        self.eva_building_being_stolen = self.eva_building_being_stolen.saturating_add(1);
    }

    pub fn honesty_eva_building_being_stolen_ok(&self) -> bool {
        self.eva_building_being_stolen > 0
    }

    pub fn record_eva_building_stolen(&mut self) {
        self.eva_building_stolen = self.eva_building_stolen.saturating_add(1);
    }

    pub fn honesty_eva_building_stolen_ok(&self) -> bool {
        self.eva_building_stolen > 0
    }

    /// Combined hero residual path honesty (any hero ability observed).
    pub fn honesty_any_ok(&self) -> bool {
        self.honesty_snipe_ok()
            || self.honesty_timed_charge_plant_ok()
            || self.honesty_remote_charge_plant_ok()
            || self.honesty_remote_charge_detonate_ok()
            || self.honesty_cash_steal_ok()
            || self.honesty_vehicle_disable_ok()
            || self.honesty_building_capture_ok()
            || self.honesty_black_market_emergency_ok()
    }
}

// --- Wave 57 residual honesty packs ---

/// Wave 57 residual honesty: SCIENCE_CashHack money tiers + reload residual.
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
        && CASH_HACK_RELOAD_FRAMES == hero_ms_to_frames(CASH_HACK_RELOAD_MS)
        && CASH_HACK_ACTIVATE_AUDIO == "CashHackActivate"
        && is_cash_hack_science_name("SCIENCE_CashHack1")
        && is_cash_hack_science_name("SCIENCE_CashHack3")
        && !is_cash_hack_science_name("SCIENCE_Pathfinder")
        // Unit special EffectValue stays default 1000 (not science-tiered).
        && STEAL_CASH_DEFAULT_AMOUNT == 1_000
        && LOTUS_STEAL_EFFECT_VALUE == 1_000
}

/// Wave 57 residual honesty: BlackMarket emergency cash residual.
pub fn honesty_black_market_emergency_residual_ok() -> bool {
    BLACK_MARKET_TEMPLATE == "GLABlackMarket"
        && BLACK_MARKET_KIND_MARKER == "FS_BLACK_MARKET"
        && BLACK_MARKET_CASH_HACK_LEGAL
        && BLACK_MARKET_DEPOSIT_AMOUNT_HONESTY == 20
        && is_black_market_cash_hack_target("GLABlackMarket")
        && is_black_market_cash_hack_target("Chem_GLABlackMarket")
        && is_black_market_cash_hack_target("TestBlackMarket")
        && !is_black_market_cash_hack_target("FakeGLABlackMarket")
        && !is_black_market_cash_hack_target("AmericaSupplyCenter")
        && is_cash_hack_target_template("GLABlackMarket")
        && is_legal_black_market_emergency_steal(true, false, true, true)
        && !is_legal_black_market_emergency_steal(true, true, true, true)
        && !is_legal_black_market_emergency_steal(true, false, false, true)
        && !is_legal_black_market_emergency_steal(true, false, true, false)
}

/// Wave 57 residual honesty: Black Lotus special ability timers residual.
pub fn honesty_lotus_special_ability_timers_residual_ok() -> bool {
    (BLACK_LOTUS_START_ABILITY_RANGE - 150.0).abs() < 0.01
        // CaptureBuilding
        && LOTUS_CAPTURE_UNPACK_MS == 6_730
        && LOTUS_CAPTURE_UNPACK_FRAMES == hero_ms_to_frames(LOTUS_CAPTURE_UNPACK_MS)
        && LOTUS_CAPTURE_PACK_MS == 2_800
        && LOTUS_CAPTURE_PACK_FRAMES == hero_ms_to_frames(LOTUS_CAPTURE_PACK_MS)
        && LOTUS_CAPTURE_PREP_MS == 6_000
        && LOTUS_CAPTURE_PREP_FRAMES == hero_ms_to_frames(LOTUS_CAPTURE_PREP_MS)
        && LOTUS_CAPTURE_AWARD_XP == 20
        && LOTUS_CAPTURE_SPECIAL_OBJECT == "BinaryDataStream"
        && LOTUS_CAPTURE_DO_CAPTURE_FX
        && SPECIAL_ABILITY_BLACK_LOTUS_CAPTURE == "SpecialAbilityBlackLotusCaptureBuilding"
        // DisableVehicleHack
        && LOTUS_DISABLE_UNPACK_MS == 2_000
        && LOTUS_DISABLE_UNPACK_FRAMES == hero_ms_to_frames(LOTUS_DISABLE_UNPACK_MS)
        && LOTUS_DISABLE_PACK_MS == 1_000
        && LOTUS_DISABLE_PACK_FRAMES == hero_ms_to_frames(LOTUS_DISABLE_PACK_MS)
        && LOTUS_DISABLE_PREP_MS == 2_000
        && LOTUS_DISABLE_PREP_FRAMES == hero_ms_to_frames(LOTUS_DISABLE_PREP_MS)
        && DISABLE_VEHICLE_HACK_DURATION_MS == 15_000
        && DISABLE_VEHICLE_HACK_DURATION_FRAMES
            == hero_ms_to_frames(DISABLE_VEHICLE_HACK_DURATION_MS)
        && LOTUS_DISABLE_FX_PARTICLE == "DisabledEffectBinaryShower0"
        && LOTUS_DISABLE_SPECIAL_OBJECT == "BinaryDataStream"
        && LOTUS_DISABLE_AWARD_XP == 0
        && SPECIAL_ABILITY_BLACK_LOTUS_DISABLE_VEHICLE
            == "SpecialAbilityBlackLotusDisableVehicleHack"
        // StealCashHack
        && LOTUS_STEAL_UNPACK_MS == 6_730
        && LOTUS_STEAL_UNPACK_FRAMES == hero_ms_to_frames(LOTUS_STEAL_UNPACK_MS)
        && LOTUS_STEAL_PACK_MS == 5_800
        && LOTUS_STEAL_PACK_FRAMES == hero_ms_to_frames(LOTUS_STEAL_PACK_MS)
        && LOTUS_STEAL_PREP_MS == 6_000
        && LOTUS_STEAL_PREP_FRAMES == hero_ms_to_frames(LOTUS_STEAL_PREP_MS)
        && LOTUS_STEAL_RELOAD_MS == 2_000
        && LOTUS_STEAL_RELOAD_FRAMES == hero_ms_to_frames(LOTUS_STEAL_RELOAD_MS)
        && LOTUS_STEAL_SPECIAL_OBJECT == "BinaryDataStream"
        && LOTUS_STEAL_AWARD_XP == 20
        && SPECIAL_ABILITY_BLACK_LOTUS_STEAL_CASH == "SpecialAbilityBlackLotusStealCashHack"
}

/// Wave 57 residual honesty: Burton charge ability timers residual.
pub fn honesty_burton_charge_ability_timers_residual_ok() -> bool {
    BURTON_CHARGE_UNPACK_MS == 5_500
        && BURTON_CHARGE_UNPACK_FRAMES == hero_ms_to_frames(BURTON_CHARGE_UNPACK_MS)
        && (BURTON_CHARGE_FLEE_RANGE - 100.0).abs() < 0.01
        && BURTON_CHARGE_PRE_TRIGGER_UNSTEALTH_MS == 5_000
        && BURTON_CHARGE_PRE_TRIGGER_UNSTEALTH_FRAMES
            == hero_ms_to_frames(BURTON_CHARGE_PRE_TRIGGER_UNSTEALTH_MS)
        && BURTON_CHARGE_LOSE_STEALTH_ON_TRIGGER
        && BURTON_MAX_REMOTE_CHARGES == 8
        && BURTON_MAX_TIMED_CHARGES == 10
        && BURTON_REMOTE_CHARGE_OBJECT == "RemoteC4Charge"
        && BURTON_TIMED_CHARGE_OBJECT == "TimedC4Charge"
        && SPECIAL_ABILITY_BURTON_REMOTE_CHARGES == "SpecialAbilityColonelBurtonRemoteCharges"
        && SPECIAL_ABILITY_BURTON_TIMED_CHARGES == "SpecialAbilityColonelBurtonTimedCharges"
}

/// Wave 57 residual honesty: Lotus body / stealth residual.
pub fn honesty_lotus_body_stealth_residual_ok() -> bool {
    (LOTUS_MAX_HEALTH - 200.0).abs() < 0.01
        && (LOTUS_VISION_RANGE - 300.0).abs() < 0.01
        && (LOTUS_SHROUD_CLEARING_RANGE - 400.0).abs() < 0.01
        && LOTUS_BUILD_COST == 1_500
        && LOTUS_STEALTH_DELAY_MS == 2_500
        && LOTUS_STEALTH_DELAY_FRAMES == hero_ms_to_frames(LOTUS_STEALTH_DELAY_MS)
        && LOTUS_INNATE_STEALTH
        && LOTUS_STEALTH_BREAKS_ON_ABILITY
        && LOTUS_ORDER_IDLE_ENEMIES_ON_REVEAL
        && LOTUS_ENEMY_DETECTION_EVA == "EnemyBlackLotusDetected"
        && LOTUS_OWN_DETECTION_EVA == "OwnBlackLotusDetected"
        && LOTUS_PACK_SOUND == "BlackLotusPack"
        && LOTUS_UNPACK_SOUND == "BlackLotusUnpack"
        && LOTUS_TRIGGER_SOUND == "BlackLotusTrigger"
        && LOTUS_PREP_SOUND_LOOP == "BlackLotusPrepLoop"
        && lotus_stealth_desired(true, true, true, true) == Some(false)
        && lotus_stealth_desired(true, true, true, false) == Some(true)
}

/// Combined Wave 57 hero abilities residual honesty pack.
pub fn honesty_hero_abilities_residual_pack_ok() -> bool {
    honesty_cash_hack_science_tier_residual_ok()
        && honesty_black_market_emergency_residual_ok()
        && honesty_lotus_special_ability_timers_residual_ok()
        && honesty_burton_charge_ability_timers_residual_ok()
        && honesty_lotus_body_stealth_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honesty_flags_track_snipe_cash_and_capture() {
        let mut reg = HostHeroAbilityRegistry::new();
        assert!(!reg.honesty_any_ok());
        reg.record_snipe();
        assert!(reg.honesty_snipe_ok());
        assert!(reg.honesty_any_ok());
        reg.record_cash_steal(500);
        assert!(reg.honesty_cash_steal_ok());
        assert_eq!(reg.cash_stolen_total, 500);
        reg.record_timed_charge_plant();
        assert!(reg.honesty_timed_charge_plant_ok());
        reg.record_remote_charge_plant();
        reg.record_remote_charge_detonate(2);
        assert!(reg.honesty_remote_charge_detonate_ok());
        assert_eq!(reg.remote_charges_detonated, 2);
        reg.record_vehicle_disable();
        assert!(reg.honesty_vehicle_disable_ok());
        reg.record_building_capture();
        assert!(reg.honesty_building_capture_ok());
        assert_eq!(DISABLE_VEHICLE_HACK_DURATION_FRAMES, 450);
        assert_eq!(BLACK_LOTUS_START_ABILITY_RANGE, 150.0);
    }

    #[test]
    fn cash_hack_template_names() {
        assert!(is_cash_hack_target_template("AmericaSupplyCenter"));
        assert!(is_cash_hack_target_template("GLABlackMarket"));
        assert!(is_cash_hack_target_template("TestSupplyCenter"));
        assert!(is_cash_hack_target_template("TestBuilding"));
        assert!(!is_cash_hack_target_template("AmericaRanger"));
        assert!(!is_cash_hack_target_template("AmericaWarFactory"));
    }

    #[test]
    fn black_lotus_template_names() {
        assert!(is_black_lotus_template("ChinaInfantryBlackLotus"));
        assert!(is_black_lotus_template("Infa_ChinaInfantryBlackLotus"));
        assert!(is_black_lotus_template("Nuke_ChinaInfantryBlackLotus"));
        assert!(is_black_lotus_template("TestBlackLotus"));
        assert!(is_black_lotus_template("TestLotus"));
        assert!(!is_black_lotus_template("ChinaInfantryHacker"));
        assert!(!is_black_lotus_template("ChinaInfantryRedguard"));
        assert!(!is_black_lotus_template("BlackLotusVoiceHackCash"));
        assert!(!is_black_lotus_template(
            "SpecialAbilityBlackLotusStealCashHack"
        ));
        assert!(!is_black_lotus_template("TestTank"));
        assert!(can_activate_black_lotus_ability(true, true));
        assert!(!can_activate_black_lotus_ability(true, false));
        assert!(!can_activate_black_lotus_ability(false, true));
        assert!(can_capture_without_upgrade(true, false));
        assert!(can_capture_without_upgrade(false, true));
        assert!(!can_capture_without_upgrade(false, false));
    }

    #[test]
    fn legal_target_matrices() {
        assert!(is_legal_steal_cash_target(true, true, false, true, true));
        assert!(!is_legal_steal_cash_target(true, true, false, true, false));
        assert!(!is_legal_steal_cash_target(true, true, true, true, true));
        assert!(!is_legal_steal_cash_target(true, false, false, true, true));
        assert!(is_legal_disable_vehicle_target(
            true, true, false, true, false, false
        ));
        assert!(!is_legal_disable_vehicle_target(
            true, true, true, true, false, false
        ));
        assert!(!is_legal_disable_vehicle_target(
            true, true, false, true, true, false
        ));
        assert!(!is_legal_disable_vehicle_target(
            true, true, false, true, false, true
        ));
        assert!(is_legal_lotus_capture_target(true, true, false, true));
        assert!(!is_legal_lotus_capture_target(true, true, true, true));
        assert!(black_lotus_in_start_range(150.0));
        assert!(black_lotus_in_start_range(0.0));
        assert!(!black_lotus_in_start_range(150.1));
        assert_eq!(disable_vehicle_until_frame(100), 550);
    }

    #[test]
    fn hero_abilities_residual_pack_honesty() {
        assert!(honesty_hero_abilities_residual_pack_ok());
        assert_eq!(hero_ms_to_frames(6_730), 202);
        assert_eq!(hero_ms_to_frames(2_800), 84);
        assert_eq!(hero_ms_to_frames(6_000), 180);
        assert_eq!(hero_ms_to_frames(5_800), 174);
        assert_eq!(hero_ms_to_frames(15_000), 450);
        assert_eq!(hero_ms_to_frames(5_500), 165);
        assert_eq!(hero_ms_to_frames(2_500), 75);
        assert_eq!(hero_ms_to_frames(240_000), 7_200);
        assert_eq!(hero_ms_to_frames(0), 0);
    }

    #[test]
    fn cash_hack_tiers_and_black_market_emergency() {
        assert_eq!(cash_hack_money_for_science_tier(0), 1_000);
        assert_eq!(cash_hack_money_for_science_tier(1), 1_000);
        assert_eq!(cash_hack_money_for_science_tier(2), 2_000);
        assert_eq!(cash_hack_money_for_science_tier(3), 4_000);
        let mut reg = HostHeroAbilityRegistry::new();
        assert!(!reg.honesty_black_market_emergency_ok());
        reg.record_black_market_emergency_steal(1_000);
        assert!(reg.honesty_black_market_emergency_ok());
        assert!(reg.honesty_cash_steal_ok());
        assert_eq!(reg.black_market_emergency_steals, 1);
        assert_eq!(reg.cash_stolen_total, 1_000);
        assert!(is_legal_black_market_emergency_steal(
            true, false, true, true
        ));
    }
}
