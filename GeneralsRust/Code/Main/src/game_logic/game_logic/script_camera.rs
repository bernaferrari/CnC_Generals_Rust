//! Mechanical split from `game_logic/game_logic.rs`. No behavior change.
#![allow(non_snake_case, unused_imports, dead_code)]
use super::authority::*;
use super::construct::*;
use super::crate_tick::*;
use super::host::*;
use super::player::*;
use super::prelude::*;
use super::*;

impl Default for RuntimeWeatherState {
    fn default() -> Self {
        Self {
            current_weather: "clear".to_string(),
            intensity: 0.0,
            duration_remaining: 0.0,
            next_change_time: 0.0,
            visible: true,
        }
    }
}

pub(super) struct ScriptBroadcast {
    pub(super) text: String,
    pub(super) expires_at: f32,
}

pub(super) fn localized_objective_string(id: &str, suffix: &str, fallback: &str) -> String {
    if id.is_empty() {
        return fallback.to_string();
    }
    let normalized = id.replace(' ', "_").to_ascii_lowercase();
    let key = format!("mission.objective.{normalized}.{suffix}");
    localization::localize(&key, fallback)
}

pub(super) fn derive_objective_status(
    obj: &MissionObjective,
) -> (ObjectiveStatus, Option<(u32, u32)>) {
    if let Some(total) = obj.required_count {
        let current = obj.current_count.min(total);
        let status = if current >= total {
            ObjectiveStatus::Completed
        } else {
            ObjectiveStatus::Active
        };
        (status, Some((current, total)))
    } else {
        (ObjectiveStatus::Active, None)
    }
}

pub(super) fn mission_objective_to_display(
    obj: &MissionObjective,
    category: ObjectiveCategory,
) -> ObjectiveDisplay {
    let id = obj.id.clone();
    let fallback_title = if obj.description.is_empty() {
        id.clone()
    } else {
        obj.description.clone()
    };
    let title = localized_objective_string(&id, "title", &fallback_title);
    let description = localized_objective_string(&id, "desc", "");
    let (status, progress) = derive_objective_status(obj);
    ObjectiveDisplay {
        id: if id.is_empty() { None } else { Some(id) },
        title,
        description,
        status,
        progress,
        category,
    }
}

/// C++ AI::findClosestEnemy qualifier flags residual.
/// C++ AttackPriorityInfo residual (ScriptEngine).
#[derive(Debug, Clone)]
pub struct AttackPriorityInfo {
    pub name: String,
    pub default_priority: i32,
    /// Template name → priority (case-insensitive keys stored lowercased).
    pub priorities: std::collections::HashMap<String, i32>,
    /// KindOf name token → priority residual (SetAttackPriorityKindOf).
    pub kind_priorities: std::collections::HashMap<String, i32>,
}

impl Default for AttackPriorityInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            default_priority: 1, // ATTACK_PRIORITY_DEFAULT
            priorities: std::collections::HashMap::new(),
            kind_priorities: std::collections::HashMap::new(),
        }
    }
}

impl AttackPriorityInfo {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn set_priority_template(&mut self, template_name: &str, priority: i32) {
        self.priorities
            .insert(template_name.to_ascii_lowercase(), priority);
    }

    pub fn set_priority_kind(&mut self, kind_name: &str, priority: i32) {
        self.kind_priorities
            .insert(kind_name.to_ascii_lowercase(), priority);
    }

    /// C++ AttackPriorityInfo::getPriority residual.
    pub fn get_priority_for_template(&self, template_name: &str) -> i32 {
        let key = template_name.to_ascii_lowercase();
        self.priorities
            .get(&key)
            .copied()
            .unwrap_or(self.default_priority)
    }
}

/// C++ AIData::m_attackPriorityDistanceModifier residual (world units per priority step).
pub const ATTACK_PRIORITY_DISTANCE_MODIFIER: f32 = 50.0;

pub mod find_enemy_flags {
    pub const CAN_SEE: u32 = 1 << 0;
    pub const CAN_ATTACK: u32 = 1 << 1;
    pub const IGNORE_INSIGNIFICANT_BUILDINGS: u32 = 1 << 2;
    pub const ATTACK_BUILDINGS: u32 = 1 << 3;
    pub const WITHIN_ATTACK_RANGE: u32 = 1 << 4;
    pub const UNFOGGED: u32 = 1 << 5;
}

/// C++ MoodMatrixAction residual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoodMatrixAction {
    Idle,
    Move,
    Attack,
    AttackMove,
}

/// C++ MAA_* residual flags (host simplified).
pub mod mood_action_adjust {
    pub const ACTION_OK: u32 = 0x01;
    pub const ACTION_TO_IDLE: u32 = 0x02;
    pub const ACTION_TO_ATTACK_MOVE: u32 = 0x04;
    pub const AFFECT_RANGE_IGNORE_ALL: u32 = 0x10;
    pub const AFFECT_RANGE_WAIT_FOR_ATTACK: u32 = 0x20;
    pub const AFFECT_RANGE_ALERT: u32 = 0x40;
    pub const AFFECT_RANGE_AGGRESSIVE: u32 = 0x80;
}

/// C++ CanAttackResult residual (WeaponSet.h).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanAttackResult {
    /// C++ ATTACKRESULT_NOT_POSSIBLE
    NotPossible,
    /// C++ ATTACKRESULT_POSSIBLE
    Possible,
    /// C++ ATTACKRESULT_POSSIBLE_AFTER_MOVING
    PossibleAfterMoving,
    /// C++ ATTACKRESULT_INVALID_SHOT
    InvalidShot,
}

/// C++ AbleToAttackType residual (GameCommon.h).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbleToAttackType {
    /// ATTACK_NEW_TARGET
    NewTarget,
    /// ATTACK_NEW_TARGET_FORCED
    NewTargetForced,
    /// ATTACK_CONTINUED_TARGET
    ContinuedTarget,
    /// ATTACK_CONTINUED_TARGET_FORCED
    ContinuedTargetForced,
    /// ATTACK_TUNNEL_NETWORK_GUARD — skip immobile/contained out-of-range abort
    TunnelNetworkGuard,
}
impl AbleToAttackType {
    pub fn is_forced(self) -> bool {
        matches!(
            self,
            AbleToAttackType::NewTargetForced | AbleToAttackType::ContinuedTargetForced
        )
    }

    pub fn is_continued(self) -> bool {
        matches!(
            self,
            AbleToAttackType::ContinuedTarget | AbleToAttackType::ContinuedTargetForced
        )
    }
}

/// C++ AIAttackState outer residual result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackMachineResult {
    /// Keep running nested AttackStateMachine.
    Continue,
    /// Victim dead / exit success.
    Success,
    /// Cannot attack (no weapon, max shots, under construction).
    Failure,
}

/// C++ AIAttackFireWeaponState residual result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackFireResult {
    /// C++ STATE_CONTINUE (PRE_ATTACK wind-up).
    Continue,
    /// C++ STATE_SUCCESS (shot discharged).
    Success,
    /// C++ STATE_FAILURE (dead target / not ready / out of range).
    Failure,
}

/// C++ AIAttackAimAtTargetState residual result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackAimResult {
    /// Still turning / held out-of-range wait.
    Continue,
    /// Within AcceptableAimDelta.
    Success,
    /// Dead victim / no weapon / held out of range.
    Failure,
}

// Wave 960: chained .find_object/.get_object → host_object idiom.
// Wave 959: internal host_object idiom (legacy get_object/find_object aliases only).
