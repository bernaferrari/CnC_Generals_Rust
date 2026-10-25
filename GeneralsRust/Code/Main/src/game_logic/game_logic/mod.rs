#![allow(non_snake_case)]

/*
** Command & Conquer Generals Zero Hour(tm) - Game Logic System
** Copyright 2025 Electronic Arts Inc.
**
** Main GameLogic singleton - manages all objects, simulation, and game state
** Ported from GeneralsMD/Code/GameEngine/Include/GameLogic/GameLogic.h
*/

pub(crate) mod named_script_cache;
mod prelude;
pub(self) use prelude::*;

mod ambient_sound_retries;
mod authority;
mod beacon_client_state;
mod camera_continuation;
pub(crate) mod command_overrides;
mod construct;
mod crate_tick;
pub(crate) use crate_tick::score_kind_of;
mod host;
mod host_object_store;
mod map_loading;
pub(crate) use map_loading::MapAttemptKind;
#[path = "../kill_credit.rs"]
pub(crate) mod kill_credit;
mod player;
#[path = "../score_academy.rs"]
mod score_academy;
mod script_camera;
pub(crate) use camera_continuation::LogicCameraContinuation;

pub(self) use ambient_sound_retries::AmbientSoundRetries;
pub(self) use authority::DestructionEvent;
pub(crate) use authority::{AcceptedGatherCommand, SupplyDropoffEvent};
pub use authority::{
    CommandPipelineOp, DirectPlayerOrder, HordeGrantCounter, HostObjectIdOp, HostObjectIdResult,
    HostResidualMutationOp, HostSupportOp, HostSupportResult, ObjectIdentityClear,
    ObjectLifecycleOp, ObjectLifecycleResult, ProductionAuthorityOp, ProductionAuthorityResult,
    SessionControlOp, SpawnedPayloadKind,
};
pub use crate_tick::{
    AICommand, AudioEventRequest, FixedStepDiagnostics, GameMode, PendingSpecialAbility,
    PlayerStatistics, SimTimingSnapshot,
};
pub(crate) use host::PathfindingHeightSamples;
pub use host::{CampaignSessionKind, GameLogic, RuntimeWeatherState};
pub use host_object_store::{HostObjectRead, HostObjectStore};
pub(crate) use player::PlayerSideRole;
pub(self) use player::{
    AirfieldHealingInfo, AirfieldParkingSpace, FRAMES_TO_ALLOW_SCAFFOLD_RESIDUAL,
    HostHeliTakeoffOrLanding, ObjectSellInfo, REBUILD_HOLE_HEALTH_REGEN_PERCENT_PER_SEC,
    REBUILD_HOLE_MAX_HEALTH_RESIDUAL, REBUILD_HOLE_WORKER_RESPAWN_FRAMES,
    REBUILD_HOLE_WORKER_TEMPLATE, SELL_CONSTRUCTION_DECREMENT_RESIDUAL,
    SELL_FINISH_CONSTRUCTION_PERCENT_RESIDUAL, TOTAL_FRAMES_TO_SELL_OBJECT_RESIDUAL,
    capture_upgrade_names_for_team, normalize_upgrade_name,
};
pub use player::{
    HostAuthoredBuild, Player, PlayerMapSideState, PlayerTemplateIdentity, SkirmishRulesState,
};
pub use script_camera::{
    ATTACK_PRIORITY_DISTANCE_MODIFIER, AbleToAttackType, AttackAimResult, AttackFireResult,
    AttackMachineResult, AttackPriorityInfo, CanAttackResult, MoodMatrixAction, find_enemy_flags,
    mood_action_adjust,
};
pub(self) use script_camera::{
    ScriptBroadcast, derive_objective_status, localized_objective_string,
    mission_objective_to_display,
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
pub(crate) mod world_objects;
#[path = "../world_save.rs"]
mod world_save;
#[path = "../world_scripts/mod.rs"]
mod world_scripts;
#[path = "../world_tick/mod.rs"]
mod world_tick;
pub(crate) use world_tick::FlightTerrainView;
pub(super) use world_tick::host_object_footprint;
pub(crate) use world_tick::{
    NativeCommandAction, NativeCommandCompletion, NativeTickContinuation, NativeTickProgress,
};

impl GameLogic {
    pub(crate) fn is_scoring_enabled(&self) -> bool {
        self.is_scoring_enabled
    }

    pub(crate) fn set_scoring_enabled(&mut self, enabled: bool) {
        self.is_scoring_enabled = enabled;
    }

    fn update_player_alive_state(&mut self) {
        for player in self.players.values_mut() {
            player.is_alive = self
                .objects
                .registered_values()
                .any(|obj| obj.team == player.team && obj.is_alive());
        }
    }

    /// Unique W3D names for objects already spawned on this world.
    pub fn live_drawable_model_names(&self) -> Vec<String> {
        let mut names = std::collections::BTreeSet::new();
        for obj in self.objects.registered_values() {
            let Some(name) = obj.thing().template.model_name.as_deref() else {
                continue;
            };
            let trimmed = name.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
                continue;
            }
            names.insert(trimmed.to_string());
        }
        names.into_iter().collect()
    }

    pub fn evaluate_victory_condition(&mut self) -> Option<VictoryCondition> {
        self.evaluate_victory_condition_with_effects(&mut Default::default())
    }

    pub(crate) fn evaluate_victory_condition_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<VictoryCondition> {
        // Player life is derived from this session's canonical objects at the
        // original victory phase, regardless of any comparison context.
        self.update_player_alive_state();
        let outcome = self.victory_conditions.evaluate_with_world_factory(
            &self.players,
            self.objects.read(),
            self.frame,
            self.game_mode,
            &self.player_template_bindings,
            &self.team_factory,
        );
        // C++ VictoryConditions.cpp:196 p->killPlayer() on first defeat frame.
        let pending = self.victory_conditions.take_pending_kills();
        for player_id in pending {
            // C++ VictoryConditions.cpp:172-196 reveals the defeated player's
            // map before killPlayer callbacks. This is gameplay state owned by
            // this session, independent of script loading or UI notification.
            if self.frame > 1 {
                self.partition_manager.reveal_map_for_player_permanently(
                    &mut self
                        .world_services
                        .shroud()
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()),
                    player_id,
                );
            }
            self.kill_player_for_victory_with_effects(player_id, effects);
        }
        // Publish only after defeat callbacks complete. C++ GameLogic.cpp:3769
        // owns the update; observers must not rerun its player census or kills.
        self.completed_victory_observation = Some((
            self.game_mode,
            VictoryObservation {
                match_over: super::victory_conditions::is_multiplayer_or_skirmish_victory(
                    self.game_mode,
                ) && self.victory_conditions.end_frame().is_some(),
                outcome,
            },
        ));
        outcome
    }

    /// Read the last completed update, including a later update returning None.
    /// This is a presentation projection, not C++'s live per-player victory query.
    pub(crate) fn current_victory_condition(&self) -> Option<VictoryCondition> {
        self.current_victory_observation().outcome
    }

    /// Match completion and the current outcome are distinct in C++.
    pub(crate) fn current_victory_observation(&self) -> VictoryObservation {
        self.completed_victory_observation
            .filter(|(mode, _)| *mode == self.game_mode)
            .map(|(_, result)| result)
            .unwrap_or_default()
    }

    /// Reset/load must not display a result from the previously observed world.
    pub(crate) fn clear_victory_observation(&mut self) {
        self.completed_victory_observation = None;
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
mod historic_bonus_owner_tests;

#[cfg(test)]
mod materialization_frame_tests;

#[cfg(test)]
#[path = "../builder_catalog_tests.rs"]
mod builder_catalog_tests;

#[cfg(test)]
#[path = "../world_skirmish_tests.rs"]
mod skirmish_starting_unit_residual_tests;

/// Concatenated facade source in original `game_logic.rs` order.
/// Source-scan tests that previously `include_str!` the monolith use this.

#[cfg(test)]
mod accepted_projectile_owner_tests;

#[cfg(test)]
mod radius_decal_clock_owner_tests;

#[cfg(test)]
pub(crate) mod pose_owner_tests;

#[cfg(test)]
mod world_read_owner_tests;

#[cfg(test)]
#[path = "../victory_owner_tests.rs"]
mod victory_owner_tests;

#[path = "../radar_owner.rs"]
mod radar_owner;

#[cfg(test)]
#[path = "../containment_audio_owner_tests.rs"]
pub(crate) mod containment_audio_owner_tests;
