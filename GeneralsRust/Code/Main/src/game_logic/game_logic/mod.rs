#![allow(non_snake_case)]

/*
** Command & Conquer Generals Zero Hour(tm) - Game Logic System
** Copyright 2025 Electronic Arts Inc.
**
** Main GameLogic singleton - manages all objects, simulation, and game state
** Ported from GeneralsMD/Code/GameEngine/Include/GameLogic/GameLogic.h
*/

mod prelude;
pub(self) use prelude::*;

mod authority;
mod construct;
mod crate_tick;
mod host;
mod player;
mod script_camera;

pub(self) use authority::DestructionEvent;
pub(crate) use authority::{AcceptedGatherCommand, SupplyDropoffEvent};
pub use authority::{
    CommandPipelineOp, DirectPlayerOrder, HordeGrantCounter, HostObjectIdOp, HostObjectIdResult,
    HostResidualMutationOp, HostSupportOp, HostSupportResult, HostWritebackOp, ObjectIdentityClear,
    ObjectLifecycleOp, ObjectLifecycleResult, PostWritebackCompleteOp, ProductionAuthorityOp,
    ProductionAuthorityResult, ReadyLogDrainOp, SessionControlOp, SpawnedPayloadKind,
};
pub use crate_tick::{
    AICommand, AudioEventRequest, FixedStepDiagnostics, GameMode, PendingSpecialAbility,
    PlayerStatistics, SimTimingSnapshot, crate_empty_noop_tick_count, tick_gamelogic_crate,
};
pub(self) use crate_tick::{CRATE_EMPTY_NOOP_TICKS, GAME_LOGIC, note_crate_empty_noop_if_any};
pub(crate) use host::PathfindingHeightSamples;
pub use host::{GameLogic, RuntimeWeatherState};
pub(self) use player::{
    AirfieldHealingInfo, AirfieldParkingSpace, FRAMES_TO_ALLOW_SCAFFOLD_RESIDUAL,
    HostHeliTakeoffOrLanding, ObjectSellInfo, REBUILD_HOLE_HEALTH_REGEN_PERCENT_PER_SEC,
    REBUILD_HOLE_MAX_HEALTH_RESIDUAL, REBUILD_HOLE_WORKER_RESPAWN_FRAMES,
    REBUILD_HOLE_WORKER_TEMPLATE, SELL_CONSTRUCTION_DECREMENT_RESIDUAL,
    SELL_FINISH_CONSTRUCTION_PERCENT_RESIDUAL, TOTAL_FRAMES_TO_SELL_OBJECT_RESIDUAL,
    capture_upgrade_names_for_team, normalize_upgrade_name,
};
pub use player::{
    HostAuthoredBuild, HostObjectStore, Player, PlayerMapSideState, PlayerTemplateIdentity,
    SkirmishRulesState,
};
pub use script_camera::{
    ATTACK_PRIORITY_DISTANCE_MODIFIER, AbleToAttackType, AttackAimResult, AttackFireResult,
    AttackMachineResult, AttackPriorityInfo, CanAttackResult, MoodMatrixAction, find_enemy_flags,
    mood_action_adjust,
};
pub(self) use script_camera::{
    ParabolicEase, ScriptBroadcast, ScriptCameraMoveTo, ScriptCameraPathMove,
    derive_objective_status, localized_objective_string, mission_objective_to_display,
};

// Split `impl GameLogic` chunks as real child modules (type-checked separately).
// Combat lives in world_combat/*.rs (#[path] because this file is a directory).
// Sibling private methods use pub(super) / pub(in super::super).
#[path = "../world_combat/mod.rs"]
mod world_combat;
pub(crate) use world_combat::weapon_visual_capture::{
    PendingWeaponVisualDispatchCapture, source_is_locally_controlled,
};
#[path = "../world_objects/mod.rs"]
mod world_objects;
#[path = "../world_save.rs"]
mod world_save;
#[path = "../world_scripts/mod.rs"]
mod world_scripts;
#[path = "../world_tick/mod.rs"]
mod world_tick;

impl GameLogic {
    fn update_player_alive_state(&mut self) {
        let mut events = Vec::new();
        for player in self.players.values_mut() {
            let alive = self
                .objects
                .values()
                .any(|obj| obj.team == player.team && obj.is_alive());
            if player.is_alive != alive {
                player.is_alive = alive;
                events.push((player.id, alive));
            } else {
                player.is_alive = alive;
            }
        }
        for (id, alive) in events {
            crate::game_logic::host_player_meta_log::record_alive(id, alive);
        }
    }

    pub fn evaluate_victory_condition(&mut self) -> Option<VictoryCondition> {
        // Wave 816: under coupled shadow, player is_alive owned by GW expire + writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_player_alive_state();
        }
        let outcome = self.victory_conditions.evaluate_with_templates(
            &self.players,
            &self.objects,
            self.frame,
            self.game_mode,
            &self.player_template_bindings,
        );
        // C++ VictoryConditions.cpp:196 p->killPlayer() on first defeat frame.
        let pending = self.victory_conditions.take_pending_kills();
        for player_id in pending {
            self.kill_player_for_victory(player_id);
        }
        outcome
    }

    pub fn victory_type(&self) -> VictoryType {
        self.victory_conditions.victory_type()
    }

    pub fn peek_defeat_events(&self) -> &[u32] {
        self.victory_conditions.peek_defeat_events()
    }

    pub fn take_defeat_events(&mut self) -> Vec<u32> {
        self.victory_conditions.take_defeat_events()
    }

    pub fn peek_alliance_events(&self) -> &[AllianceNotification] {
        self.victory_conditions.peek_alliance_events()
    }

    pub fn take_alliance_events(&mut self) -> Vec<AllianceNotification> {
        self.victory_conditions.take_alliance_events()
    }
}

/// Detailed object information for UI display
#[derive(Debug, Clone)]
pub struct ObjectInfo {
    pub id: ObjectId,
    pub name: String,
    pub team: Team,
    pub object_type: ObjectType,
    pub health: Health,
    pub max_health: f32,
    pub position: Vec3,
    pub is_selected: bool,
    pub is_moving: bool,
    pub is_attacking: bool,
    pub under_construction: bool,
    pub construction_percent: f32,
    pub experience_level: VeterancyLevel,
    pub ai_state: AIState,
    pub can_attack: bool,
    pub can_move: bool,
}

#[derive(Clone)]
struct ShroudVisibilitySnapshot {
    visible_objects: HashSet<u32>,
    explored_objects: HashSet<u32>,
}

#[cfg(test)]
#[path = "../world_tests/mod.rs"]
mod tests;

#[cfg(test)]
#[path = "../world_skirmish_tests.rs"]
mod skirmish_starting_unit_residual_tests;

/// Concatenated facade source in original `game_logic.rs` order.
/// Source-scan tests that previously `include_str!` the monolith use this.
#[cfg(test)]
pub const GAME_LOGIC_FACADE_SRC: &str = concat!(
    include_str!("../world_scripts/unit_commands.rs"),
    include_str!("crate_tick.rs"),
    include_str!("player.rs"),
    include_str!("host.rs"),
    include_str!("script_camera.rs"),
    include_str!("authority.rs"),
    include_str!("construct.rs"),
    include_str!("mod.rs"),
);
