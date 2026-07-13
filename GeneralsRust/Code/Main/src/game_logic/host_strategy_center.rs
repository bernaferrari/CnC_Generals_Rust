//! Host USA Strategy Center battle-plan residual
//! (Bombardment / HoldTheLine / SearchAndDestroy).
//!
//! Residual slice (playability):
//! - `AmericaStrategyCenter` / *StrategyCenter selects a battle plan via
//!   `SpecialAbilityChangeBattlePlans` residual (`BattlePlanUpdate`).
//! - Selecting a plan applies army-wide residual bonuses to same-team
//!   legal members (ValidMemberKindOf = INFANTRY | CAN_ATTACK | VEHICLE;
//!   InvalidMemberKindOf = DOZER | STRUCTURE | AIRCRAFT | DRONE):
//!   - **Bombardment**: WeaponBonus BATTLEPLAN_BOMBARDMENT DAMAGE **120%**
//!   - **HoldTheLine**: HoldTheLinePlanArmorDamageScalar **0.9** (take 90% damage)
//!   - **SearchAndDestroy**: WeaponBonus RANGE **120%** + SightRangeScalar **1.2**
//! - Strategy Center building residuals (when plan active on the center):
//!   - HoldTheLine max-health scalar **2.0** (PRESERVE_RATIO residual)
//!   - SearchAndDestroy building sight scalar **2.0** + stealth detect residual
//!
//! - **BattlePlanChangeParalyzeTime residual**: on plan change (including first
//!   residual select), legal army members receive DISABLED_PARALYZED for
//!   **5000 ms → 150 frames** (retail BattlePlanChangeParalyzeTime).
//!
//! Fail-closed honesty:
//! - Not full BattlePlanUpdate pack/unpack animation / door model-condition matrix
//! - Not full pack→NONE→unpack transition ordering (host paralyzes on activate)
//! - Not full turret enable/recenter for Bombardment residual path
//! - Not full StealthDetectorUpdate enable/disable module stack beyond residual flag
//! - Not full vision object (VisionObjectName) spawn residual
//! - Not network battle-plan replication (network deferred)

use super::ObjectId;
use serde::{Deserialize, Serialize};

/// Retail HoldTheLinePlanArmorDamageScalar (LESS is better — damage taken mult).
pub const HOLD_THE_LINE_ARMOR_DAMAGE_SCALAR: f32 = 0.9;

/// Retail SearchAndDestroyPlanSightRangeScalar.
pub const SEARCH_AND_DESTROY_SIGHT_RANGE_SCALAR: f32 = 1.2;

/// Retail GameData.ini WeaponBonus BATTLEPLAN_BOMBARDMENT DAMAGE 120%.
pub const BOMBARDMENT_DAMAGE_MULT: f32 = 1.20;

/// Retail GameData.ini WeaponBonus BATTLEPLAN_SEARCHANDDESTROY RANGE 120%.
pub const SEARCH_AND_DESTROY_RANGE_MULT: f32 = 1.20;

/// Retail StrategyCenterHoldTheLineMaxHealthScalar.
pub const STRATEGY_CENTER_HOLD_THE_LINE_MAX_HEALTH_SCALAR: f32 = 2.0;

/// Retail StrategyCenterSearchAndDestroySightRangeScalar.
pub const STRATEGY_CENTER_SEARCH_AND_DESTROY_SIGHT_SCALAR: f32 = 2.0;

/// Retail BattlePlanChangeParalyzeTime (ms).
pub const BATTLE_PLAN_PARALYZE_TIME_MS: u32 = 5000;

/// Logic frames per second (host fixed step).
pub const BATTLE_PLAN_LOGIC_FPS: f32 = 30.0;

/// Retail BattlePlanChangeParalyzeTime → frames at 30 FPS (5000 / (1000/30) = 150).
pub const BATTLE_PLAN_PARALYZE_FRAMES: u32 = 150;

/// Residual announcement audio (pack/unpack sounds fail-closed).
pub const BATTLE_PLAN_BOMBARDMENT_AUDIO: &str = "StrategyCenter_BombardmentPlanAnnouncement";
pub const BATTLE_PLAN_HOLD_THE_LINE_AUDIO: &str = "StrategyCenter_HoldTheLineAnnouncement";
pub const BATTLE_PLAN_SEARCH_AND_DESTROY_AUDIO: &str =
    "StrategyCenter_SearchAndDestroyAnnouncement";

/// Convert BattlePlanChangeParalyzeTime ms → logic frames (30 FPS residual).
pub fn battle_plan_paralyze_frames_from_ms(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / BATTLE_PLAN_LOGIC_FPS)).round() as u32
}

/// Absolute host frame when DISABLED_PARALYZED residual expires.
pub fn battle_plan_paralyze_until_frame(current_frame: u32) -> u32 {
    current_frame.saturating_add(BATTLE_PLAN_PARALYZE_FRAMES.max(1))
}

/// USA Strategy Center battle plan residual kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostBattlePlan {
    /// PLANSTATUS_BOMBARDMENT — army DAMAGE 120% residual.
    Bombardment = 1,
    /// PLANSTATUS_HOLDTHELINE — army armor damage scalar 0.9 residual.
    HoldTheLine = 2,
    /// PLANSTATUS_SEARCHANDDESTROY — army RANGE 120% + sight 1.2 residual.
    SearchAndDestroy = 3,
}

impl HostBattlePlan {
    /// Parse residual plan from 1..=3 (fail-closed: unknown → Bombardment).
    pub fn from_u8(v: u8) -> Self {
        match v {
            2 => HostBattlePlan::HoldTheLine,
            3 => HostBattlePlan::SearchAndDestroy,
            _ => HostBattlePlan::Bombardment,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Retail WeaponBonus DAMAGE multiplier for army members (1.0 if not Bombardment).
    pub fn army_damage_multiplier(self) -> f32 {
        match self {
            HostBattlePlan::Bombardment => BOMBARDMENT_DAMAGE_MULT,
            _ => 1.0,
        }
    }

    /// Retail armor damage scalar for army members (1.0 if not HoldTheLine).
    /// LESS is better — multiplies incoming damage.
    pub fn army_armor_damage_scalar(self) -> f32 {
        match self {
            HostBattlePlan::HoldTheLine => HOLD_THE_LINE_ARMOR_DAMAGE_SCALAR,
            _ => 1.0,
        }
    }

    /// Retail WeaponBonus RANGE multiplier for army members (1.0 if not S&D).
    pub fn army_range_multiplier(self) -> f32 {
        match self {
            HostBattlePlan::SearchAndDestroy => SEARCH_AND_DESTROY_RANGE_MULT,
            _ => 1.0,
        }
    }

    /// Retail sight-range scalar for army members (1.0 if not S&D).
    pub fn army_sight_range_scalar(self) -> f32 {
        match self {
            HostBattlePlan::SearchAndDestroy => SEARCH_AND_DESTROY_SIGHT_RANGE_SCALAR,
            _ => 1.0,
        }
    }

    /// Residual announcement audio event name.
    pub fn activate_audio(self) -> &'static str {
        match self {
            HostBattlePlan::Bombardment => BATTLE_PLAN_BOMBARDMENT_AUDIO,
            HostBattlePlan::HoldTheLine => BATTLE_PLAN_HOLD_THE_LINE_AUDIO,
            HostBattlePlan::SearchAndDestroy => BATTLE_PLAN_SEARCH_AND_DESTROY_AUDIO,
        }
    }
}

/// Whether template is a residual USA Strategy Center building.
///
/// Fail-closed: name residual (not full BattlePlanUpdate module matrix).
pub fn is_strategy_center_template(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("crate")
        || n.contains("commandset")
        || n.contains("gun")
    {
        return false;
    }
    n.contains("strategycenter")
}

fn alnum_lower(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Whether residual unit can receive Strategy Center army battle-plan bonuses.
///
/// Retail BattlePlanUpdate ValidMemberKindOf / InvalidMemberKindOf residual:
/// - Valid: INFANTRY | CAN_ATTACK | VEHICLE
/// - Invalid: DOZER | STRUCTURE | AIRCRAFT | DRONE
/// Host residual also requires same-team + alive + not under construction.
pub fn is_legal_battle_plan_member(
    is_infantry: bool,
    is_vehicle: bool,
    can_attack: bool,
    is_structure: bool,
    is_aircraft: bool,
    is_dozer: bool,
    is_drone: bool,
    is_alive: bool,
    same_team: bool,
    under_construction: bool,
) -> bool {
    if !is_alive || !same_team || under_construction {
        return false;
    }
    if is_structure || is_aircraft || is_dozer || is_drone {
        return false;
    }
    // ValidMember residual: INFANTRY | CAN_ATTACK | VEHICLE
    is_infantry || is_vehicle || can_attack
}

/// Residual dozer name filter (InvalidMemberKindOf DOZER).
pub fn is_dozer_template_name(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("dozer") || n.contains("constructionvehicle")
}

/// Residual drone name filter (InvalidMemberKindOf DRONE).
pub fn is_drone_template_name(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("drone") || n.contains("spyplane")
}

/// One active residual battle-plan selection bookkeeping entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostBattlePlanSelection {
    pub id: u32,
    pub player_id: u32,
    pub plan: HostBattlePlan,
    pub activate_frame: u32,
    pub strategy_center_id: Option<ObjectId>,
    /// Same-team legal army members that received residual bonuses.
    pub buffs: u32,
    /// Strategy Center building residual applied (max-health / detect / sight).
    pub building_bonus: bool,
    /// Same-team legal army members that received DISABLED_PARALYZED residual.
    #[serde(default)]
    pub paralyzed: u32,
}

/// Host residual registry for Strategy Center battle plans.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostBattlePlanRegistry {
    next_id: u32,
    /// Per-player currently active residual plan (player_id → plan).
    active_by_player: Vec<(u32, HostBattlePlan)>,
    /// Recent selections (bookkeeping; unit flags live on objects).
    selections: Vec<HostBattlePlanSelection>,
    /// Total plan selections (honesty).
    pub selection_count: u32,
    /// Total army member residual grants.
    pub buff_count: u32,
    /// Total building residual grants.
    pub building_bonus_count: u32,
    /// Total army members hit by BattlePlanChangeParalyze residual.
    #[serde(default)]
    pub paralyze_count: u32,
}

impl HostBattlePlanRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn selection_count(&self) -> u32 {
        self.selection_count
    }

    pub fn buff_count(&self) -> u32 {
        self.buff_count
    }

    pub fn building_bonus_count(&self) -> u32 {
        self.building_bonus_count
    }

    pub fn paralyze_count(&self) -> u32 {
        self.paralyze_count
    }

    pub fn selections(&self) -> &[HostBattlePlanSelection] {
        &self.selections
    }

    pub fn active_plan_for_player(&self, player_id: u32) -> Option<HostBattlePlan> {
        self.active_by_player
            .iter()
            .find(|(pid, _)| *pid == player_id)
            .map(|(_, p)| *p)
    }

    pub fn set_active_plan(&mut self, player_id: u32, plan: HostBattlePlan) {
        if let Some(entry) = self
            .active_by_player
            .iter_mut()
            .find(|(pid, _)| *pid == player_id)
        {
            entry.1 = plan;
        } else {
            self.active_by_player.push((player_id, plan));
        }
    }

    pub fn clear_active_plan(&mut self, player_id: u32) {
        self.active_by_player.retain(|(pid, _)| *pid != player_id);
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Record a successful residual battle-plan selection.
    pub fn record_selection(&mut self, selection: HostBattlePlanSelection) {
        self.selection_count = self.selection_count.saturating_add(1);
        self.buff_count = self.buff_count.saturating_add(selection.buffs);
        if selection.building_bonus {
            self.building_bonus_count = self.building_bonus_count.saturating_add(1);
        }
        self.paralyze_count = self.paralyze_count.saturating_add(selection.paralyzed);
        self.set_active_plan(selection.player_id, selection.plan);
        self.selections.push(selection);
        // Keep bookkeeping bounded (residual, not full history Xfer).
        if self.selections.len() > 32 {
            let drain = self.selections.len() - 32;
            self.selections.drain(0..drain);
        }
    }

    /// Residual honesty: at least one battle plan selected.
    pub fn honesty_select_ok(&self) -> bool {
        self.selection_count > 0
    }

    /// Residual honesty: at least one unit received army residual bonuses.
    pub fn honesty_buff_ok(&self) -> bool {
        self.buff_count > 0
    }

    /// Residual honesty: BattlePlanChangeParalyze applied to at least one unit.
    pub fn honesty_paralyze_ok(&self) -> bool {
        self.paralyze_count > 0
    }

    /// Combined host path: selected and applied at least one army buff.
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_select_ok() && self.honesty_buff_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battle_plan_constants_match_retail_residual() {
        assert!((BOMBARDMENT_DAMAGE_MULT - 1.20).abs() < 0.001);
        assert!((HOLD_THE_LINE_ARMOR_DAMAGE_SCALAR - 0.9).abs() < 0.001);
        assert!((SEARCH_AND_DESTROY_RANGE_MULT - 1.20).abs() < 0.001);
        assert!((SEARCH_AND_DESTROY_SIGHT_RANGE_SCALAR - 1.2).abs() < 0.001);
        assert!((STRATEGY_CENTER_HOLD_THE_LINE_MAX_HEALTH_SCALAR - 2.0).abs() < 0.001);
        assert!((STRATEGY_CENTER_SEARCH_AND_DESTROY_SIGHT_SCALAR - 2.0).abs() < 0.001);
        assert_eq!(BATTLE_PLAN_PARALYZE_TIME_MS, 5000);
        assert_eq!(BATTLE_PLAN_PARALYZE_FRAMES, 150);
        assert_eq!(battle_plan_paralyze_frames_from_ms(5000), 150);
        assert_eq!(battle_plan_paralyze_until_frame(10), 160);
        assert!(!BATTLE_PLAN_BOMBARDMENT_AUDIO.is_empty());
        assert!(!BATTLE_PLAN_HOLD_THE_LINE_AUDIO.is_empty());
        assert!(!BATTLE_PLAN_SEARCH_AND_DESTROY_AUDIO.is_empty());
    }

    #[test]
    fn strategy_center_name_matrix() {
        assert!(is_strategy_center_template("AmericaStrategyCenter"));
        assert!(is_strategy_center_template("USA_StrategyCenter"));
        assert!(is_strategy_center_template("SupW_AmericaStrategyCenter"));
        assert!(is_strategy_center_template("Lazr_AmericaStrategyCenter"));
        assert!(is_strategy_center_template("AirF_AmericaStrategyCenter"));
        assert!(is_strategy_center_template("TestStrategyCenter"));
        assert!(!is_strategy_center_template("AmericaCommandCenter"));
        assert!(!is_strategy_center_template("StrategyCenterGun"));
        assert!(!is_strategy_center_template("USA_Ranger"));
        assert!(!is_strategy_center_template("TestTank"));
    }

    #[test]
    fn legal_battle_plan_member_matrix() {
        // infantry, vehicle, can_attack, structure, aircraft, dozer, drone, alive, same_team, under_construction
        assert!(is_legal_battle_plan_member(
            true, false, true, false, false, false, false, true, true, false
        ));
        assert!(is_legal_battle_plan_member(
            false, true, true, false, false, false, false, true, true, false
        ));
        assert!(is_legal_battle_plan_member(
            false, false, true, false, false, false, false, true, true, false
        ));
        // Invalid: structure
        assert!(!is_legal_battle_plan_member(
            false, false, true, true, false, false, false, true, true, false
        ));
        // Invalid: aircraft
        assert!(!is_legal_battle_plan_member(
            false, false, true, false, true, false, false, true, true, false
        ));
        // Invalid: dozer
        assert!(!is_legal_battle_plan_member(
            false, true, true, false, false, true, false, true, true, false
        ));
        // Invalid: drone
        assert!(!is_legal_battle_plan_member(
            false, true, true, false, false, false, true, true, true, false
        ));
        // Invalid: enemy / dead / construction
        assert!(!is_legal_battle_plan_member(
            true, false, true, false, false, false, false, true, false, false
        ));
        assert!(!is_legal_battle_plan_member(
            true, false, true, false, false, false, false, false, true, false
        ));
        assert!(!is_legal_battle_plan_member(
            true, false, true, false, false, false, false, true, true, true
        ));
    }

    #[test]
    fn plan_multipliers_and_parse() {
        assert_eq!(HostBattlePlan::from_u8(1), HostBattlePlan::Bombardment);
        assert_eq!(HostBattlePlan::from_u8(2), HostBattlePlan::HoldTheLine);
        assert_eq!(HostBattlePlan::from_u8(3), HostBattlePlan::SearchAndDestroy);
        assert_eq!(HostBattlePlan::from_u8(99), HostBattlePlan::Bombardment); // fail-closed
        assert!((HostBattlePlan::Bombardment.army_damage_multiplier() - 1.20).abs() < 0.001);
        assert!((HostBattlePlan::HoldTheLine.army_armor_damage_scalar() - 0.9).abs() < 0.001);
        assert!((HostBattlePlan::SearchAndDestroy.army_range_multiplier() - 1.20).abs() < 0.001);
        assert!((HostBattlePlan::SearchAndDestroy.army_sight_range_scalar() - 1.2).abs() < 0.001);
        assert!((HostBattlePlan::Bombardment.army_armor_damage_scalar() - 1.0).abs() < f32::EPSILON);
        assert!((HostBattlePlan::HoldTheLine.army_damage_multiplier() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn honesty_registry_records_buffs() {
        let mut reg = HostBattlePlanRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        let id = reg.alloc_id();
        reg.record_selection(HostBattlePlanSelection {
            id,
            player_id: 0,
            plan: HostBattlePlan::Bombardment,
            activate_frame: 0,
            strategy_center_id: None,
            buffs: 2,
            building_bonus: true,
            paralyzed: 2,
        });
        assert!(reg.honesty_select_ok());
        assert!(reg.honesty_buff_ok());
        assert!(reg.honesty_paralyze_ok());
        assert!(reg.honesty_host_path_ok());
        assert_eq!(reg.selection_count(), 1);
        assert_eq!(reg.buff_count(), 2);
        assert_eq!(reg.building_bonus_count(), 1);
        assert_eq!(reg.paralyze_count(), 2);
        assert_eq!(
            reg.active_plan_for_player(0),
            Some(HostBattlePlan::Bombardment)
        );
    }
}
