//! Script Evaluation System
//!
//! This module provides script evaluation logic that matches the C++ ScriptEngine evaluation,
//! including condition evaluation, action execution, and script state management.

use super::core::*;
use super::engine::{get_area_tracker, get_named_object_tracker, ScriptActionHandler, *};
use super::executor::{
    ScriptActionDispatcher, ScriptActionResult, ScriptConditionEvaluator, ScriptConditionResult,
    ScriptContext,
};
use crate::commands::get_selection_manager;
use crate::common::{
    AsciiString, DisabledType, KindOf, ObjectID, ObjectShroudStatus, PlayerMaskType, UnsignedInt,
    LOGICFRAMES_PER_SECOND,
};
use crate::helpers::TheGameLogic;
use crate::modules::ContainModuleInterface;
use crate::object::object_types::ObjectTypes;
use crate::player::player_list;
use crate::polygon_trigger::PolygonTrigger;
use crate::team::get_team_factory;
use crate::terrain::get_terrain_logic;
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::{get_science_store, SCIENCE_INVALID};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Script Evaluator matching C++ ScriptEngine evaluation logic
pub struct ScriptEvaluator {
    engine: Arc<RwLock<Option<ScriptEngine>>>,
}

static TRANSPORT_STATUSES: Lazy<RwLock<HashMap<ObjectID, (UnsignedInt, usize)>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

impl ScriptEvaluator {
    pub fn new(engine: Arc<RwLock<Option<ScriptEngine>>>) -> Self {
        Self { engine }
    }

    /// Evaluate a complete script matching C++ EvaluateScripts
    pub fn evaluate_script(&self, script: &mut Script) -> GameLogicResult<bool> {
        log::debug!("Evaluating script: {}", script.get_name());

        // Check if script is active
        if !script.is_active() {
            return Ok(false);
        }

        // Evaluate conditions
        let condition_result = if let Some(or_condition) = script.condition.as_deref_mut() {
            self.evaluate_or_condition(or_condition)?
        } else {
            true // No conditions means always true
        };

        log::debug!(
            "Script '{}' condition result: {}",
            script.get_name(),
            condition_result
        );

        // Execute actions based on condition result
        if condition_result {
            if let Some(action) = script.get_action() {
                self.execute_action_sequence(action)?;
            }
        } else {
            if let Some(false_action) = script.get_false_action() {
                self.execute_action_sequence(false_action)?;
            }
        }

        Ok(condition_result)
    }

    /// Evaluate OR condition matching C++ EvaluateConditions
    pub fn evaluate_or_condition(&self, or_condition: &mut OrCondition) -> GameLogicResult<bool> {
        let mut current_or = Some(or_condition);

        while let Some(or_cond) = current_or {
            // Evaluate all AND conditions in this OR clause
            if let Some(and_condition) = or_cond.first_and.as_deref_mut() {
                if self.evaluate_and_condition(and_condition)? {
                    return Ok(true); // Short-circuit on first true OR
                }
            }

            current_or = or_cond.next_or.as_deref_mut();
        }

        Ok(false) // All OR conditions were false
    }

    /// Evaluate AND condition chain
    pub fn evaluate_and_condition(&self, and_condition: &mut Condition) -> GameLogicResult<bool> {
        let mut current_and = Some(and_condition);

        while let Some(and_cond) = current_and {
            if !self.evaluate_condition(and_cond)? {
                return Ok(false); // Short-circuit on first false AND
            }

            current_and = and_cond.next_and_condition.as_deref_mut();
        }

        Ok(true) // All AND conditions were true
    }

    /// Evaluate a single condition matching C++ EvaluateCondition
    pub fn evaluate_condition(&self, condition: &mut Condition) -> GameLogicResult<bool> {
        const SLOW_SCRIPT_CONDITION_WARN_MS: u64 = 40;
        let condition_type = condition.get_condition_type();
        let eval_started = Instant::now();
        let result = match condition_type {
            ConditionType::ConditionFalse => Ok(false),
            ConditionType::ConditionTrue => Ok(true),
            ConditionType::Counter => self.evaluate_counter_condition(condition),
            ConditionType::Flag => self.evaluate_flag_condition(condition),
            ConditionType::TimerExpired => self.evaluate_timer_expired_condition(condition),
            ConditionType::PlayerAllDestroyed => {
                self.evaluate_player_all_destroyed_condition(condition)
            }
            ConditionType::PlayerAllBuildfacilitiesDestroyed => {
                self.evaluate_player_all_buildfacilities_destroyed_condition(condition)
            }
            ConditionType::TeamInsideAreaPartially => {
                self.evaluate_team_inside_area_partially_condition(condition)
            }
            ConditionType::TeamDestroyed => self.evaluate_team_destroyed_condition(condition),
            ConditionType::TeamHasUnits => self.evaluate_team_has_units_condition(condition),
            ConditionType::TeamStateIs => self.evaluate_team_state_is_condition(condition),
            ConditionType::TeamStateIsNot => self.evaluate_team_state_is_not_condition(condition),
            ConditionType::NamedInsideArea => self.evaluate_named_inside_area_condition(condition),
            ConditionType::NamedOutsideArea => {
                self.evaluate_named_outside_area_condition(condition)
            }
            ConditionType::NamedDestroyed => self.evaluate_named_destroyed_condition(condition),
            ConditionType::NamedNotDestroyed => {
                self.evaluate_named_not_destroyed_condition(condition)
            }
            ConditionType::NamedAttackedByObjecttype => {
                self.evaluate_named_attacked_by_object_type_condition(condition)
            }
            ConditionType::TeamAttackedByObjecttype => {
                self.evaluate_team_attacked_by_object_type_condition(condition)
            }
            ConditionType::NamedAttackedByPlayer => {
                self.evaluate_named_attacked_by_player_condition(condition)
            }
            ConditionType::TeamAttackedByPlayer => {
                self.evaluate_team_attacked_by_player_condition(condition)
            }
            ConditionType::NamedCreated => self.evaluate_named_created_condition(condition),
            ConditionType::TeamCreated => self.evaluate_team_created_condition(condition),
            ConditionType::NamedDiscovered => self.evaluate_named_discovered_condition(condition),
            ConditionType::TeamDiscovered => self.evaluate_team_discovered_condition(condition),
            ConditionType::TeamInsideAreaEntirely => {
                self.evaluate_team_inside_area_entirely_condition(condition)
            }
            ConditionType::TeamOutsideAreaEntirely => {
                self.evaluate_team_outside_area_entirely_condition(condition)
            }
            ConditionType::PlayerHasCredits => {
                self.evaluate_player_has_credits_condition(condition)
            }
            ConditionType::PlayerHasPower => self.evaluate_player_has_power_condition(condition),
            ConditionType::PlayerHasNoPower => {
                self.evaluate_player_has_no_power_condition(condition)
            }
            ConditionType::NamedOwnedByPlayer => {
                self.evaluate_named_owned_by_player_condition(condition)
            }
            ConditionType::TeamOwnedByPlayer => {
                self.evaluate_team_owned_by_player_condition(condition)
            }
            ConditionType::PlayerHasNOrFewerBuildings => {
                self.evaluate_player_has_n_or_fewer_buildings_condition(condition)
            }
            ConditionType::BuildingEnteredByPlayer => {
                self.evaluate_building_entered_by_player_condition(condition)
            }
            ConditionType::HasFinishedVideo => {
                self.evaluate_has_finished_video_condition(condition)
            }
            ConditionType::HasFinishedSpeech => {
                self.evaluate_has_finished_speech_condition(condition)
            }
            ConditionType::HasFinishedAudio => {
                self.evaluate_has_finished_audio_condition(condition)
            }
            ConditionType::UnitHealth => self.evaluate_unit_health_condition(condition),
            ConditionType::NamedEnteredArea => {
                self.evaluate_named_entered_area_condition(condition)
            }
            ConditionType::NamedExitedArea => self.evaluate_named_exited_area_condition(condition),
            ConditionType::NamedDying => self.evaluate_named_dying_condition(condition),
            ConditionType::NamedTotallyDead => {
                self.evaluate_named_totally_dead_condition(condition)
            }
            ConditionType::NamedSelected => self.evaluate_named_selected_condition(condition),
            ConditionType::TeamEnteredAreaEntirely => {
                self.evaluate_team_entered_area_entirely_condition(condition)
            }
            ConditionType::TeamEnteredAreaPartially => {
                self.evaluate_team_entered_area_partially_condition(condition)
            }
            ConditionType::TeamExitedAreaEntirely => {
                self.evaluate_team_exited_area_entirely_condition(condition)
            }
            ConditionType::TeamExitedAreaPartially => {
                self.evaluate_team_exited_area_partially_condition(condition)
            }
            ConditionType::PlayerHasNOrFewerFactionBuildings => {
                self.evaluate_player_has_n_or_fewer_faction_buildings_condition(condition)
            }
            ConditionType::BuiltByPlayer => self.evaluate_built_by_player_condition(condition),
            ConditionType::NamedBuildingIsEmpty => {
                self.evaluate_named_building_is_empty_condition(condition)
            }
            ConditionType::PlayerPowerComparePercent => {
                self.evaluate_player_power_compare_percent_condition(condition)
            }
            ConditionType::PlayerExcessPowerCompareValue => {
                self.evaluate_player_excess_power_compare_value_condition(condition)
            }
            ConditionType::UnitHasObjectStatus => {
                self.evaluate_unit_has_object_status_condition(condition)
            }
            ConditionType::TeamAllHasObjectStatus => {
                self.evaluate_team_has_object_status_condition(condition, true)
            }
            ConditionType::TeamSomeHaveObjectStatus => {
                self.evaluate_team_has_object_status_condition(condition, false)
            }
            ConditionType::PlayerAcquiredScience => {
                self.evaluate_player_acquired_science_condition(condition)
            }
            ConditionType::PlayerHasSciencepurchasepoints => {
                self.evaluate_player_has_science_purchase_points_condition(condition)
            }
            ConditionType::PlayerCanPurchaseScience => {
                self.evaluate_player_can_purchase_science_condition(condition)
            }
            ConditionType::NamedHasFreeContainerSlots => {
                self.evaluate_named_has_free_container_slots_condition(condition)
            }
            ConditionType::UnitEmptied => self.evaluate_unit_emptied_condition(condition),
            _ => {
                let ctx = self.make_script_context();
                let mut evaluator = ScriptConditionEvaluator::new(ctx);
                match evaluator.evaluate_condition(condition) {
                    Ok(ScriptConditionResult::True) => Ok(true),
                    Ok(ScriptConditionResult::False) => Ok(false),
                    Ok(ScriptConditionResult::Error(msg)) => Err(GameLogicError::Configuration(
                        format!("Script condition evaluation error: {}", msg),
                    )),
                    Err(err) => Err(GameLogicError::Configuration(format!(
                        "Script condition evaluation failed: {}",
                        err
                    ))),
                }
            }
        };
        let elapsed = eval_started.elapsed();
        if elapsed >= std::time::Duration::from_millis(SLOW_SCRIPT_CONDITION_WARN_MS) {
            log::warn!(
                "Slow script condition evaluate: {:?} took {:?}",
                condition_type,
                elapsed
            );
        }
        result
    }

    fn resolve_team_name_token(&self, raw: &str) -> String {
        match raw {
            THIS_TEAM => get_script_engine()
                .read()
                .ok()
                .and_then(|g| {
                    g.as_ref().and_then(|e| {
                        e.get_condition_team_name()
                            .or_else(|| e.get_calling_team_name())
                            .map(|s| s.to_string())
                    })
                })
                .unwrap_or_else(|| raw.to_string()),
            TEAM_THE_PLAYER => {
                let current_player = get_script_engine().read().ok().and_then(|g| {
                    g.as_ref()
                        .and_then(|e| e.get_current_player_name().map(|s| s.to_string()))
                });
                let Some(player_name) = current_player else {
                    return raw.to_string();
                };

                player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.find_player_by_name(&player_name))
                    .and_then(|p| p.read().ok().and_then(|p| p.get_default_team()))
                    .and_then(|team| team.read().ok().map(|t| t.get_name().to_string()))
                    .unwrap_or_else(|| raw.to_string())
            }
            _ => raw.to_string(),
        }
    }

    fn resolve_team_instances(&self, team_name: &str) -> Vec<Arc<RwLock<crate::team::Team>>> {
        let Ok(mut factory) = get_team_factory().lock() else {
            return Vec::new();
        };

        if let Some(team_arc) = factory.find_team(team_name) {
            return vec![team_arc];
        }

        factory.find_team_instances(team_name)
    }

    fn resolve_object_types(&self, param: &Parameter) -> ObjectTypes {
        let type_name = param.get_string();
        let mut types = ObjectTypes::new();
        if type_name.is_empty() {
            return types;
        }

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                if let Some(found) = engine.get_object_types(type_name) {
                    return found;
                }
            }
        }

        types.add_object_type(AsciiString::from(type_name));
        types
    }

    fn resolve_player_from_param(
        &self,
        param: &Parameter,
    ) -> Option<Arc<RwLock<crate::player::Player>>> {
        let mask_bits = param.get_int() as u32;
        if param.get_parameter_type() == ParameterType::Side && mask_bits != 0 {
            let mask = PlayerMaskType::from_bits_truncate(mask_bits);
            if let Ok(list) = player_list().read() {
                for player_arc in list.iter() {
                    let Ok(player_guard) = player_arc.read() else {
                        continue;
                    };
                    if mask.intersects(player_guard.get_player_mask()) {
                        return Some(Arc::clone(player_arc));
                    }
                }
            }
        }

        let raw = param.get_string();
        let resolved = match raw {
            THE_PLAYER | THIS_PLAYER => get_script_engine()
                .read()
                .ok()
                .and_then(|g| {
                    g.as_ref()
                        .and_then(|e| e.get_current_player_name().map(|s| s.to_string()))
                })
                .unwrap_or_else(|| raw.to_string()),
            LOCAL_PLAYER => player_list()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned())
                .and_then(|p| {
                    p.read()
                        .ok()
                        .and_then(|p| NameKeyGenerator::key_to_name(p.get_player_name_key()))
                })
                .unwrap_or_else(|| raw.to_string()),
            _ => raw.to_string(),
        };

        if resolved.is_empty() {
            return None;
        }
        player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&resolved))
    }

    fn get_trigger_area(&self, area_name: &str) -> Option<PolygonTrigger> {
        let Ok(terrain) = get_terrain_logic().read() else {
            return None;
        };
        terrain.get_trigger_area_by_name(area_name).cloned()
    }

    fn is_object_considerable(obj: &crate::object::Object) -> bool {
        if obj.is_effectively_dead() {
            return false;
        }
        !obj.is_kind_of(KindOf::Inert)
    }

    fn is_object_inside_trigger(obj: &crate::object::Object, trigger: &PolygonTrigger) -> bool {
        let pos = obj.get_position();
        let point = crate::common::ICoord3D::new(pos.x as i32, pos.y as i32, pos.z as i32);
        trigger.point_in_trigger_int(&point)
    }

    fn evaluate_named_inside_area_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedInsideArea condition missing unit parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedInsideArea condition missing trigger area parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let area_name = area_param.get_string();

        let trigger = match self.get_trigger_area(area_name) {
            Some(trigger) => trigger,
            None => return Ok(false),
        };

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };

        if !Self::is_object_considerable(&obj_guard) {
            return Ok(false);
        }

        Ok(Self::is_object_inside_trigger(&obj_guard, &trigger))
    }

    fn evaluate_named_outside_area_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        Ok(!self.evaluate_named_inside_area_condition(condition)?)
    }

    fn evaluate_team_inside_area_partially_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamInsideAreaPartially condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamInsideAreaPartially condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamInsideAreaPartially condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;

        let trigger = match self.get_trigger_area(area_name) {
            Some(trigger) => trigger,
            None => return Ok(false),
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if team_guard.some_inside_some_outside(&trigger, which_to_consider)
                || team_guard.all_inside(&trigger, which_to_consider)
            {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn evaluate_team_inside_area_entirely_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamInsideAreaEntirely condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamInsideAreaEntirely condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamInsideAreaEntirely condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;

        let trigger = match self.get_trigger_area(area_name) {
            Some(trigger) => trigger,
            None => return Ok(false),
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if team_guard.all_inside(&trigger, which_to_consider) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn evaluate_team_outside_area_entirely_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamOutsideAreaEntirely condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamOutsideAreaEntirely condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamOutsideAreaEntirely condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;

        let trigger = match self.get_trigger_area(area_name) {
            Some(trigger) => trigger,
            None => return Ok(false),
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if !(team_guard.all_inside(&trigger, which_to_consider)
                || team_guard.some_inside_some_outside(&trigger, which_to_consider))
            {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn evaluate_named_entered_area_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedEnteredArea condition missing unit parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedEnteredArea condition missing trigger area parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let area_name = area_param.get_string();
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let current_frame = TheGameLogic::get_frame() as u32;
        let area_tracker = get_area_tracker();
        if !area_tracker.has_area(area_name).unwrap_or(false) {
            return Ok(false);
        }

        Ok(area_tracker
            .get_last_enter_frame(area_name, object_id)
            .map(|frame| frame == current_frame)
            .unwrap_or(false))
    }

    fn evaluate_named_exited_area_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedExitedArea condition missing unit parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedExitedArea condition missing trigger area parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let area_name = area_param.get_string();
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let current_frame = TheGameLogic::get_frame() as u32;
        let area_tracker = get_area_tracker();
        if !area_tracker.has_area(area_name).unwrap_or(false) {
            return Ok(false);
        }

        Ok(area_tracker
            .get_last_exit_frame(area_name, object_id)
            .map(|frame| frame == current_frame)
            .unwrap_or(false))
    }

    fn evaluate_team_entered_area_entirely_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamEnteredAreaEntirely condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamEnteredAreaEntirely condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamEnteredAreaEntirely condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;
        let area_tracker = get_area_tracker();
        if !area_tracker.has_area(area_name).unwrap_or(false) {
            return Ok(false);
        }
        let Some(trigger) = self.get_trigger_area(area_name) else {
            return Ok(false);
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if !team_guard.did_enter_or_exit() {
                continue;
            }

            if team_guard.did_all_enter(&trigger, which_to_consider) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn evaluate_team_entered_area_partially_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamEnteredAreaPartially condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamEnteredAreaPartially condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamEnteredAreaPartially condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;
        let area_tracker = get_area_tracker();
        if !area_tracker.has_area(area_name).unwrap_or(false) {
            return Ok(false);
        }
        let Some(trigger) = self.get_trigger_area(area_name) else {
            return Ok(false);
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if team_guard.did_partial_enter(&trigger, which_to_consider) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn evaluate_team_exited_area_entirely_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamExitedAreaEntirely condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamExitedAreaEntirely condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamExitedAreaEntirely condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;
        let area_tracker = get_area_tracker();
        if !area_tracker.has_area(area_name).unwrap_or(false) {
            return Ok(false);
        }
        let Some(trigger) = self.get_trigger_area(area_name) else {
            return Ok(false);
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if !team_guard.did_enter_or_exit() {
                continue;
            }

            if team_guard.did_all_exit(&trigger, which_to_consider) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn evaluate_team_exited_area_partially_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamExitedAreaPartially condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamExitedAreaPartially condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamExitedAreaPartially condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;
        let area_tracker = get_area_tracker();
        if !area_tracker.has_area(area_name).unwrap_or(false) {
            return Ok(false);
        }
        let Some(trigger) = self.get_trigger_area(area_name) else {
            return Ok(false);
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if team_guard.did_partial_exit(&trigger, which_to_consider) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Evaluate counter condition
    fn evaluate_counter_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let counter_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("Counter condition missing counter parameter".to_string())
        })?;
        let comparison_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "Counter condition missing comparison parameter".to_string(),
            )
        })?;
        let value_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration("Counter condition missing value parameter".to_string())
        })?;

        let counter_name = counter_param.get_string();
        let comparison = comparison_param.get_int() as u32;
        let target_value = value_param.get_int();

        let engine = self.engine.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_ref().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        if let Some(counter) = engine.get_counter(counter_name) {
            let current_value = counter.value;
            match comparison {
                0 => Ok(current_value < target_value),  // LessThan
                1 => Ok(current_value <= target_value), // LessEqual
                2 => Ok(current_value == target_value), // Equal
                3 => Ok(current_value >= target_value), // GreaterEqual
                4 => Ok(current_value > target_value),  // Greater
                5 => Ok(current_value != target_value), // NotEqual
                _ => Err(GameLogicError::Configuration(format!(
                    "Invalid comparison type: {}",
                    comparison
                ))),
            }
        } else {
            Ok(false) // Counter doesn't exist
        }
    }

    /// Evaluate flag condition
    fn evaluate_flag_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let flag_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("Flag condition missing flag parameter".to_string())
        })?;
        let value_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration("Flag condition missing value parameter".to_string())
        })?;

        let flag_name = flag_param.get_string();
        let target_value = value_param.get_int() != 0;

        let engine = self.engine.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_ref().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        if let Some(flag) = engine.get_flag(flag_name) {
            Ok(flag.value == target_value)
        } else {
            Ok(false) // Flag doesn't exist
        }
    }

    /// Evaluate timer expired condition
    fn evaluate_timer_expired_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let counter_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("Timer condition missing counter parameter".to_string())
        })?;

        let counter_name = counter_param.get_string();

        let engine = self.engine.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_ref().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        if let Some(counter) = engine.get_counter(counter_name) {
            if !counter.is_countdown_timer {
                return Ok(false);
            }
            Ok(counter.value < 1)
        } else {
            Ok(false) // Timer doesn't exist
        }
    }

    /// Evaluate player all destroyed condition
    fn evaluate_player_all_destroyed_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerAllDestroyed condition missing player parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        log::debug!("Evaluating PlayerAllDestroyed for player: {}", player_name);

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        for object_id in player_guard.get_all_objects() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let alive = if let Ok(obj_guard) = obj_arc.read() {
                !obj_guard.is_destroyed()
            } else {
                false
            };
            if alive {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Evaluate player all build facilities destroyed condition
    fn evaluate_player_all_buildfacilities_destroyed_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerAllBuildfacilitiesDestroyed condition missing player parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        log::debug!(
            "Evaluating PlayerAllBuildfacilitiesDestroyed for player: {}",
            player_name
        );

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        for object_id in player_guard.get_all_objects() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if !obj_guard.is_destroyed() && is_build_facility(&obj_guard) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Evaluate team destroyed condition
    fn evaluate_team_destroyed_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamDestroyed condition missing team parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        log::debug!("Evaluating TeamDestroyed for team: {}", team_name);

        let teams = self.resolve_team_instances(&team_name);
        if teams.is_empty() {
            return Ok(true);
        }

        for team_arc in teams {
            if let Ok(team) = team_arc.read() {
                if team.has_any_objects() {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Evaluate team has units condition
    fn evaluate_team_has_units_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamHasUnits condition missing team parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        log::debug!("Evaluating TeamHasUnits for team: {}", team_name);

        for team_arc in self.resolve_team_instances(&team_name) {
            if let Ok(team) = team_arc.read() {
                if team.has_any_units() {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Evaluate named destroyed condition
    fn evaluate_named_destroyed_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedDestroyed condition missing unit parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        log::debug!("Evaluating NamedDestroyed for unit: {}", unit_name);

        // Look up the named object using the tracker
        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(unit_name) {
            // Check if the object exists and is destroyed
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj) = obj_arc.read() {
                    return Ok(obj.is_destroyed());
                }
            }
            // Object ID exists but object not found - considered destroyed
            return Ok(true);
        }
        // Object not in tracker - considered destroyed
        Ok(true)
    }

    fn evaluate_named_created_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedCreated condition missing unit parameter".to_string(),
            )
        })?;
        let unit_name = unit_param.get_string();

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };
        Ok(TheGameLogic::find_object_by_id(object_id).is_some())
    }

    fn evaluate_team_created_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamCreated condition missing team parameter".to_string(),
            )
        })?;
        let team_name = self.resolve_team_name_token(team_param.get_string());

        let Some(team_arc) = self.resolve_team_instances(&team_name).into_iter().next() else {
            return Ok(false);
        };
        let Ok(team_guard) = team_arc.read() else {
            return Ok(false);
        };
        Ok(team_guard.is_created())
    }

    fn evaluate_team_state_is_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamStateIs condition missing team parameter".to_string(),
            )
        })?;
        let state_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamStateIs condition missing state parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let expected_state = state_param.get_string();

        let Some(team_arc) = self.resolve_team_instances(&team_name).into_iter().next() else {
            return Ok(false);
        };
        let Ok(team_guard) = team_arc.read() else {
            return Ok(false);
        };
        Ok(team_guard.get_state().as_str() == expected_state)
    }

    fn evaluate_team_state_is_not_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamStateIsNot condition missing team parameter".to_string(),
            )
        })?;
        let state_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamStateIsNot condition missing state parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let expected_state = state_param.get_string();

        let Some(team_arc) = self.resolve_team_instances(&team_name).into_iter().next() else {
            return Ok(false);
        };
        let Ok(team_guard) = team_arc.read() else {
            return Ok(false);
        };
        Ok(team_guard.get_state().as_str() != expected_state)
    }

    fn evaluate_named_attacked_by_object_type_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedAttackedByObjecttype condition missing unit parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedAttackedByObjecttype condition missing type parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };
        let Some(body) = obj_guard.get_body_module() else {
            return Ok(false);
        };
        let Ok(body_guard) = body.lock() else {
            return Ok(false);
        };
        let Some(last) = body_guard.get_last_damage_info() else {
            return Ok(false);
        };

        let types = self.resolve_object_types(type_param);
        if let Some(template) = last.input.source_template.as_deref() {
            return Ok(types.contains_template(Some(template)));
        }

        let attacker_id = last.input.source_id;
        let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
            return Ok(false);
        };
        let Ok(attacker_guard) = attacker_arc.read() else {
            return Ok(false);
        };
        Ok(types.contains_template(Some(attacker_guard.get_template())))
    }

    fn evaluate_team_attacked_by_object_type_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamAttackedByObjecttype condition missing team parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamAttackedByObjecttype condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let types = self.resolve_object_types(type_param);

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };
            for member_id in team_guard.get_members() {
                let Some(member_arc) = TheGameLogic::find_object_by_id(*member_id) else {
                    continue;
                };
                let Ok(member_guard) = member_arc.read() else {
                    continue;
                };
                let Some(body) = member_guard.get_body_module() else {
                    continue;
                };
                let Ok(body_guard) = body.lock() else {
                    continue;
                };
                let Some(last) = body_guard.get_last_damage_info() else {
                    continue;
                };

                if let Some(template) = last.input.source_template.as_deref() {
                    if types.contains_template(Some(template)) {
                        return Ok(true);
                    }
                    continue;
                }

                let attacker_id = last.input.source_id;
                let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
                    continue;
                };
                let Ok(attacker_guard) = attacker_arc.read() else {
                    continue;
                };
                if types.contains_template(Some(attacker_guard.get_template())) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn evaluate_named_attacked_by_player_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedAttackedByPlayer condition missing unit parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedAttackedByPlayer condition missing player parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };
        let Some(body) = obj_guard.get_body_module() else {
            return Ok(false);
        };
        let Ok(body_guard) = body.lock() else {
            return Ok(false);
        };
        let Some(last) = body_guard.get_last_damage_info() else {
            return Ok(false);
        };

        let target_player = self.resolve_player_from_param(player_param);
        if target_player.is_none() {
            return Ok(false);
        }

        if last.input.source_player_mask != PlayerMaskType::none() {
            if let Some(target_player) = target_player.as_ref() {
                if let Ok(target_guard) = target_player.read() {
                    if last
                        .input
                        .source_player_mask
                        .intersects(target_guard.get_player_mask())
                    {
                        return Ok(true);
                    }
                }
            }
        }

        let attacker_id = last.input.source_id;
        let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
            return Ok(false);
        };
        let Ok(attacker_guard) = attacker_arc.read() else {
            return Ok(false);
        };
        let Some(attacker_player) = attacker_guard.get_controlling_player() else {
            return Ok(false);
        };
        let Some(target_player) = target_player else {
            return Ok(false);
        };
        Ok(Arc::ptr_eq(&attacker_player, &target_player))
    }

    fn evaluate_team_attacked_by_player_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamAttackedByPlayer condition missing team parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamAttackedByPlayer condition missing player parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let target_player = self.resolve_player_from_param(player_param);
        if target_player.is_none() {
            return Ok(false);
        }

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };
            for member_id in team_guard.get_members() {
                let Some(member_arc) = TheGameLogic::find_object_by_id(*member_id) else {
                    continue;
                };
                let Ok(member_guard) = member_arc.read() else {
                    continue;
                };
                let Some(body) = member_guard.get_body_module() else {
                    continue;
                };
                let Ok(body_guard) = body.lock() else {
                    continue;
                };
                let Some(last) = body_guard.get_last_damage_info() else {
                    continue;
                };
                let attacker_id = last.input.source_id;
                let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
                    continue;
                };
                let Ok(attacker_guard) = attacker_arc.read() else {
                    continue;
                };
                let Some(attacker_player) = attacker_guard.get_controlling_player() else {
                    continue;
                };
                if Arc::ptr_eq(
                    &attacker_player,
                    target_player.as_ref().expect("checked above"),
                ) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn evaluate_named_dying_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("NamedDying condition missing unit parameter".to_string())
        })?;
        let unit_name = unit_param.get_string();

        let tracker = get_named_object_tracker();
        if let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                return Ok(false);
            };
            let Ok(obj_guard) = obj_arc.read() else {
                return Ok(false);
            };
            return Ok(obj_guard.is_effectively_dead());
        }

        Ok(false)
    }

    fn evaluate_named_totally_dead_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedTotallyDead condition missing unit parameter".to_string(),
            )
        })?;
        let unit_name = unit_param.get_string();

        let tracker = get_named_object_tracker();
        if tracker.get_object_id(unit_name).ok().flatten().is_some() {
            return Ok(false);
        }
        Ok(tracker.did_object_exist(unit_name).unwrap_or(false))
    }

    fn evaluate_named_selected_condition(
        &self,
        condition: &mut Condition,
    ) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedSelected condition missing unit parameter".to_string(),
            )
        })?;
        let unit_name = unit_param.get_string();

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let selection_manager = get_selection_manager();
        let Ok(manager_guard) = selection_manager.read() else {
            return Ok(false);
        };

        let frame_changed = manager_guard.get_frame_selection_changed();
        if condition.custom_data != 0 && condition.custom_frame == frame_changed {
            return Ok(condition.custom_data == 1);
        }

        let mut is_selected = false;
        if let Ok(list) = player_list().read() {
            let local_index = list.get_local_player_index();
            if local_index >= 0 {
                if let Some(selection) = manager_guard.get_player_selection_ref(local_index) {
                    is_selected = selection.is_object_selected(object_id);
                }
            }
        }

        if !is_selected {
            is_selected = manager_guard.is_object_selected_by_any_player(object_id);
        }

        condition.custom_data = if is_selected { 1 } else { -1 };
        condition.custom_frame = frame_changed;
        Ok(is_selected)
    }

    fn evaluate_built_by_player_condition(
        &self,
        condition: &mut Condition,
    ) -> GameLogicResult<bool> {
        let type_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "BuiltByPlayer condition missing type parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "BuiltByPlayer condition missing player parameter".to_string(),
            )
        })?;

        if condition.custom_data != 0 {
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(engine) = engine_guard.as_ref() {
                    if engine.get_frame_object_count_changed() == condition.custom_frame {
                        return Ok(condition.custom_data == 1);
                    }
                }
            }
        }

        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };
        let types = self.resolve_object_types(type_param);

        let mut count = 0;
        for obj_arc in player_guard.get_objects() {
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if types.contains_template(Some(obj_guard.get_template())) {
                count += 1;
            }
        }

        let result = count != 0;
        condition.custom_data = if result { 1 } else { -1 };
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                condition.custom_frame = engine.get_frame_object_count_changed();
            }
        }
        Ok(result)
    }

    fn evaluate_named_building_is_empty_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedBuildingIsEmpty condition missing unit parameter".to_string(),
            )
        })?;
        let unit_name = unit_param.get_string();

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };
        let Some(contain) = obj_guard.get_contain() else {
            return Ok(false);
        };
        let Ok(contain_guard) = contain.lock() else {
            return Ok(false);
        };
        Ok(contain_guard.get_contained_count() == 0)
    }

    fn evaluate_building_entered_by_player_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "BuildingEnteredByPlayer condition missing player parameter".to_string(),
            )
        })?;
        let building_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "BuildingEnteredByPlayer condition missing building parameter".to_string(),
            )
        })?;

        let building_name = building_param.get_string();
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(building_name).ok().flatten() else {
            return Ok(false);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };
        let Some(contain) = obj_guard.get_contain() else {
            return Ok(false);
        };
        let Ok(contain_guard) = contain.lock() else {
            return Ok(false);
        };

        let player_mask = contain_guard.get_player_who_entered();
        if player_mask == PlayerMaskType::none() {
            return Ok(false);
        }

        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };
        Ok(player_mask.intersects(player_guard.get_player_mask()))
    }

    /// Evaluate named not destroyed condition
    fn evaluate_named_not_destroyed_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        Ok(!self.evaluate_named_destroyed_condition(condition)?)
    }

    fn evaluate_named_discovered_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedDiscovered condition missing unit parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedDiscovered condition missing player parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let player_name = player_param.get_string();

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let player_index = player_arc.read().ok().map(|p| p.get_player_index());
        let Some(player_index) = player_index else {
            return Ok(false);
        };

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };

        if obj_guard.is_disabled_by_type(DisabledType::Held) {
            return Ok(false);
        }

        if obj_guard.test_status(crate::common::ObjectStatusTypes::Stealthed)
            && !obj_guard.test_status(crate::common::ObjectStatusTypes::Detected)
            && !obj_guard.test_status(crate::common::ObjectStatusTypes::Disguised)
        {
            return Ok(false);
        }

        let shroud = obj_guard.get_shrouded_status(player_index as i32);
        Ok(matches!(
            shroud,
            ObjectShroudStatus::Clear | ObjectShroudStatus::PartialClear
        ))
    }

    fn evaluate_team_discovered_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamDiscovered condition missing team parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamDiscovered condition missing player parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let player_name = player_param.get_string();

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let player_index = player_arc.read().ok().map(|p| p.get_player_index());
        let Some(player_index) = player_index else {
            return Ok(false);
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            for &member_id in team_guard.get_members() {
                let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                    continue;
                };
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };

                if obj_guard.is_disabled_by_type(DisabledType::Held) {
                    continue;
                }
                if obj_guard.test_status(crate::common::ObjectStatusTypes::Stealthed)
                    && !obj_guard.test_status(crate::common::ObjectStatusTypes::Detected)
                    && !obj_guard.test_status(crate::common::ObjectStatusTypes::Disguised)
                {
                    continue;
                }

                let shroud = obj_guard.get_shrouded_status(player_index as i32);
                if matches!(
                    shroud,
                    ObjectShroudStatus::Clear | ObjectShroudStatus::PartialClear
                ) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Evaluate player has credits condition
    fn evaluate_player_has_credits_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let credits_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasCredits condition missing credits parameter".to_string(),
            )
        })?;
        let comparison_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasCredits condition missing comparison parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasCredits condition missing player parameter".to_string(),
            )
        })?;

        let target_credits = credits_param.get_int();
        let comparison = comparison_param.get_int() as u32;
        let player_name = player_param.get_string();

        log::debug!(
            "Evaluating PlayerHasCredits for player: {} target: {} comparison: {}",
            player_name,
            target_credits,
            comparison
        );

        // Look up the player and get their credits
        let current_credits = if let Ok(players) = player_list().read() {
            if let Some(player_arc) = players.find_player_by_name(player_name) {
                if let Ok(player) = player_arc.read() {
                    player.get_money().get_money()
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        };

        match comparison {
            0 => Ok(current_credits < target_credits),  // LessThan
            1 => Ok(current_credits <= target_credits), // LessEqual
            2 => Ok(current_credits == target_credits), // Equal
            3 => Ok(current_credits >= target_credits), // GreaterEqual
            4 => Ok(current_credits > target_credits),  // Greater
            5 => Ok(current_credits != target_credits), // NotEqual
            _ => Err(GameLogicError::Configuration(format!(
                "Invalid comparison type: {}",
                comparison
            ))),
        }
    }

    /// Evaluate player has power condition
    fn evaluate_player_has_power_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasPower condition missing player parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        log::debug!("Evaluating PlayerHasPower for player: {}", player_name);

        // Look up the player and check their power status
        if let Ok(players) = player_list().read() {
            if let Some(player_arc) = players.find_player_by_name(player_name) {
                if let Ok(player) = player_arc.read() {
                    // Player has power if production >= consumption (not low power)
                    return Ok(!player.get_energy().is_low_power());
                }
            }
        }
        // If player doesn't exist, default to no power
        Ok(false)
    }

    /// Evaluate player has no power condition
    fn evaluate_player_has_no_power_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        Ok(!self.evaluate_player_has_power_condition(condition)?)
    }

    fn evaluate_named_owned_by_player_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedOwnedByPlayer condition missing unit parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedOwnedByPlayer condition missing player parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let player_name = player_param.get_string();
        log::debug!(
            "Evaluating NamedOwnedByPlayer for unit: {} player: {}",
            unit_name,
            player_name
        );

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let player_id = player_arc
            .read()
            .ok()
            .map(|p| p.get_player_index() as UnsignedInt);

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };

        Ok(player_id == obj_guard.get_controlling_player_id())
    }

    fn evaluate_team_owned_by_player_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamOwnedByPlayer condition missing team parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamOwnedByPlayer condition missing player parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let player_name = player_param.get_string();
        log::debug!(
            "Evaluating TeamOwnedByPlayer for team: {} player: {}",
            team_name,
            player_name
        );

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let player_id = player_arc
            .read()
            .ok()
            .map(|p| p.get_player_index() as UnsignedInt);

        for team_arc in self.resolve_team_instances(&team_name) {
            if let Ok(team_guard) = team_arc.read() {
                if team_guard.get_controlling_player_id() == player_id {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn evaluate_player_has_n_or_fewer_buildings_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let building_count_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasNOrFewerBuildings condition missing building count parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasNOrFewerBuildings condition missing player parameter".to_string(),
            )
        })?;

        let max_buildings = building_count_param.get_int();
        let player_name = player_param.get_string();
        log::debug!(
            "Evaluating PlayerHasNOrFewerBuildings for player: {}",
            player_name
        );

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        let mut count = 0;
        for object_id in player_guard.get_all_objects() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.is_effectively_dead() || obj_guard.is_destroyed() {
                continue;
            }
            if obj_guard.is_kind_of(KindOf::Structure) {
                count += 1;
            }
        }

        Ok(max_buildings >= count)
    }

    fn evaluate_player_has_n_or_fewer_faction_buildings_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let building_count_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasNOrFewerFactionBuildings condition missing building count parameter"
                    .to_string(),
            )
        })?;
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasNOrFewerFactionBuildings condition missing player parameter".to_string(),
            )
        })?;

        let max_buildings = building_count_param.get_int();
        let player_name = player_param.get_string();
        log::debug!(
            "Evaluating PlayerHasNOrFewerFactionBuildings for player: {}",
            player_name
        );

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        let mut count = 0;
        for object_id in player_guard.get_all_objects() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.is_effectively_dead() || obj_guard.is_destroyed() {
                continue;
            }
            if obj_guard.is_kind_of(KindOf::Structure)
                && obj_guard.is_kind_of(KindOf::CountsForVictory)
            {
                count += 1;
            }
        }

        Ok(max_buildings >= count)
    }

    fn evaluate_player_power_compare_percent_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerPowerComparePercent condition missing player parameter".to_string(),
            )
        })?;
        let comparison_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerPowerComparePercent condition missing comparison parameter".to_string(),
            )
        })?;
        let percent_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerPowerComparePercent condition missing percent parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        let comparison = comparison_param.get_int() as u32;
        let percent = percent_param.get_int() as f64 / 100.0;

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        let ratio = player_guard.get_energy().supply_ratio() as f64;
        match comparison {
            0 => Ok(ratio < percent),  // LessThan
            1 => Ok(ratio <= percent), // LessEqual
            2 => Ok(ratio == percent), // Equal
            3 => Ok(ratio >= percent), // GreaterEqual
            4 => Ok(ratio > percent),  // Greater
            5 => Ok(ratio != percent), // NotEqual
            _ => Err(GameLogicError::Configuration(format!(
                "Invalid comparison type: {}",
                comparison
            ))),
        }
    }

    fn evaluate_player_excess_power_compare_value_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerExcessPowerCompareValue condition missing player parameter".to_string(),
            )
        })?;
        let comparison_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerExcessPowerCompareValue condition missing comparison parameter".to_string(),
            )
        })?;
        let value_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerExcessPowerCompareValue condition missing value parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        let comparison = comparison_param.get_int() as u32;
        let desired_excess = value_param.get_int() as i64;

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        let energy = player_guard.get_energy();
        let actual_excess = (energy.production() - energy.consumption()) as i64;

        match comparison {
            0 => Ok(actual_excess < desired_excess),  // LessThan
            1 => Ok(actual_excess <= desired_excess), // LessEqual
            2 => Ok(actual_excess == desired_excess), // Equal
            3 => Ok(actual_excess >= desired_excess), // GreaterEqual
            4 => Ok(actual_excess > desired_excess),  // Greater
            5 => Ok(actual_excess != desired_excess), // NotEqual
            _ => Err(GameLogicError::Configuration(format!(
                "Invalid comparison type: {}",
                comparison
            ))),
        }
    }

    /// Evaluate has finished video condition
    fn evaluate_has_finished_video_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let video_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "HasFinishedVideo condition missing video parameter".to_string(),
            )
        })?;

        let video_name = video_param.get_string();

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        Ok(engine.is_video_complete(video_name, true))
    }

    /// Evaluate has finished speech condition
    fn evaluate_has_finished_speech_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let speech_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "HasFinishedSpeech condition missing speech parameter".to_string(),
            )
        })?;

        let speech_name = speech_param.get_string();
        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;
        Ok(engine.is_speech_complete(speech_name, true))
    }

    /// Evaluate has finished audio condition
    fn evaluate_has_finished_audio_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let audio_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "HasFinishedAudio condition missing audio parameter".to_string(),
            )
        })?;

        let audio_name = audio_param.get_string();
        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;
        Ok(engine.is_audio_complete(audio_name, true))
    }

    fn evaluate_unit_health_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("UnitHealth condition missing unit parameter".to_string())
        })?;
        let comparison_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "UnitHealth condition missing comparison parameter".to_string(),
            )
        })?;
        let health_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "UnitHealth condition missing health parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let comparison = comparison_param.get_int() as u32;
        let target_percent = health_param.get_int() as i64;

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };

        let max_health = obj_guard.get_max_health();
        if max_health <= f32::EPSILON {
            return Ok(false);
        }
        let cur_health = obj_guard.get_health();
        let cur_percent = ((cur_health * 100.0) + (max_health / 2.0)) / max_health;
        let cur_percent = cur_percent.round() as i64;

        match comparison {
            0 => Ok(cur_percent < target_percent),  // LessThan
            1 => Ok(cur_percent <= target_percent), // LessEqual
            2 => Ok(cur_percent == target_percent), // Equal
            3 => Ok(cur_percent >= target_percent), // GreaterEqual
            4 => Ok(cur_percent > target_percent),  // Greater
            5 => Ok(cur_percent != target_percent), // NotEqual
            _ => Err(GameLogicError::Configuration(format!(
                "Invalid comparison type: {}",
                comparison
            ))),
        }
    }

    fn evaluate_unit_has_object_status_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "UnitHasObjectStatus condition missing unit parameter".to_string(),
            )
        })?;
        let status_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "UnitHasObjectStatus condition missing status parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let status_mask = status_param.get_object_status();

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };

        Ok(obj_guard.get_status_bits().intersects(status_mask))
    }

    fn evaluate_team_has_object_status_condition(
        &self,
        condition: &Condition,
        entire_team: bool,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamHasObjectStatus condition missing team parameter".to_string(),
            )
        })?;
        let status_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamHasObjectStatus condition missing status parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let status_mask = status_param.get_object_status();

        let teams = self.resolve_team_instances(&team_name);
        if teams.is_empty() {
            return Ok(false);
        }

        for team_arc in teams {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            for &member_id in team_guard.get_members() {
                let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                    continue;
                };
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };

                let has_status = obj_guard.get_status_bits().intersects(status_mask);
                if entire_team && !has_status {
                    return Ok(false);
                } else if !entire_team && has_status {
                    return Ok(true);
                }
            }
        }

        Ok(entire_team)
    }

    fn evaluate_player_acquired_science_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerAcquiredScience condition missing player parameter".to_string(),
            )
        })?;
        let science_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerAcquiredScience condition missing science parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        let science_name = science_param.get_string();

        let science = if let Some(science_store) = get_science_store() {
            science_store.get_science_from_internal_name(science_name)
        } else {
            SCIENCE_INVALID
        };
        if science == SCIENCE_INVALID {
            return Ok(false);
        }

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let player_index = player_arc
            .read()
            .ok()
            .map(|p| p.get_player_index() as usize);
        let Some(player_index) = player_index else {
            return Ok(false);
        };

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        Ok(engine.is_science_acquired(player_index, science, true))
    }

    fn evaluate_player_has_science_purchase_points_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasSciencepurchasepoints condition missing player parameter".to_string(),
            )
        })?;
        let points_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasSciencepurchasepoints condition missing points parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        let points_needed = points_param.get_int();

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        Ok(player_guard.get_science_purchase_points() >= points_needed)
    }

    fn evaluate_player_can_purchase_science_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerCanPurchaseScience condition missing player parameter".to_string(),
            )
        })?;
        let science_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerCanPurchaseScience condition missing science parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        let science_name = science_param.get_string();

        let science = if let Some(science_store) = get_science_store() {
            science_store.get_science_from_internal_name(science_name)
        } else {
            SCIENCE_INVALID
        };
        if science == SCIENCE_INVALID {
            return Ok(false);
        }

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        Ok(player_guard.is_capable_of_purchasing_science(science))
    }

    fn evaluate_named_has_free_container_slots_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedHasFreeContainerSlots condition missing unit parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };
        let Some(contain) = obj_guard.get_contain() else {
            return Ok(false);
        };
        let Ok(contain_guard) = contain.lock() else {
            return Ok(false);
        };

        Ok(contain_guard.get_contained_count() < contain_guard.get_max_capacity())
    }

    fn evaluate_unit_emptied_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "UnitEmptied condition missing unit parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };

        let num_peeps = obj_guard
            .get_contain()
            .and_then(|contain| contain.lock().ok().map(|c| c.get_contained_count()))
            .unwrap_or(0);

        let frame = TheGameLogic::get_frame();
        let mut statuses = TRANSPORT_STATUSES.write().map_err(|e| {
            GameLogicError::Threading(format!("Transport status lock error: {}", e))
        })?;

        let entry = statuses.entry(object_id).or_insert((frame, num_peeps));
        if entry.0 == frame.saturating_sub(1) && entry.1 > 0 && num_peeps == 0 {
            return Ok(true);
        }

        *entry = (frame, num_peeps);
        Ok(false)
    }

    /// Execute a sequence of actions
    pub fn execute_action_sequence(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let mut current_action = Some(action);

        while let Some(act) = current_action {
            self.execute_action(act)?;
            current_action = act.get_next();
        }

        Ok(())
    }

    /// Execute a single action matching C++ DoAction
    pub fn execute_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        log::debug!("Executing action: {:?}", action.get_action_type());

        match action.get_action_type() {
            ScriptActionType::NoOp => Ok(()), // Do nothing
            ScriptActionType::Victory => self.execute_victory_action(action),
            ScriptActionType::Defeat => self.execute_defeat_action(action),
            ScriptActionType::SetFlag => self.execute_set_flag_action(action),
            ScriptActionType::SetCounter => self.execute_set_counter_action(action),
            ScriptActionType::IncrementCounter => self.execute_increment_counter_action(action),
            ScriptActionType::DecrementCounter => self.execute_decrement_counter_action(action),
            ScriptActionType::SetTimer => self.execute_set_timer_action(action),
            ScriptActionType::SetMillisecondTimer => {
                self.execute_set_millisecond_timer_action(action)
            }
            ScriptActionType::DisplayText => self.execute_display_text_action(action),
            ScriptActionType::PlaySoundEffect => self.execute_play_sound_effect_action(action),
            ScriptActionType::EnableScript => self.execute_enable_script_action(action),
            ScriptActionType::DisableScript => self.execute_disable_script_action(action),
            ScriptActionType::FreezeTime => self.execute_freeze_time_action(action),
            ScriptActionType::UnfreezeTime => self.execute_unfreeze_time_action(action),
            ScriptActionType::PlayerSetMoney => self.execute_player_set_money_action(action),
            ScriptActionType::PlayerGiveMoney => self.execute_player_give_money_action(action),
            ScriptActionType::Quickvictory => self.execute_quick_victory_action(action),
            _ => {
                let ctx = self.make_script_context();
                let mut dispatcher = ScriptActionDispatcher::new(ctx);
                match dispatcher.execute_action(action) {
                    Ok(ScriptActionResult::Success) => Ok(()),
                    Ok(ScriptActionResult::Pending(_frames)) => Ok(()),
                    Ok(ScriptActionResult::Failed(msg)) => Err(GameLogicError::Configuration(
                        format!("Script action failed: {}", msg),
                    )),
                    Err(err) => Err(GameLogicError::Configuration(format!(
                        "Script action dispatch failed: {}",
                        err
                    ))),
                }
            }
        }
    }

    fn make_script_context(&self) -> Arc<RwLock<ScriptContext>> {
        let mut context = ScriptContext::new();
        context.current_frame = TheGameLogic::get_frame();
        Arc::new(RwLock::new(context))
    }

    fn with_action_handler<F>(&self, f: F) -> GameLogicResult<()>
    where
        F: FnOnce(&dyn ScriptActionHandler) -> GameLogicResult<()>,
    {
        if let Some(handler) = self.get_action_handler()? {
            f(handler.as_ref())
        } else {
            Ok(())
        }
    }

    fn get_action_handler(&self) -> GameLogicResult<Option<Arc<dyn ScriptActionHandler>>> {
        let engine = self.engine.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        Ok(engine.as_ref().and_then(|engine| engine.action_handler()))
    }

    /// Execute victory action
    fn execute_victory_action(&self, _action: &ScriptAction) -> GameLogicResult<()> {
        log::info!("Victory action executed");

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        engine.start_end_game_timer();
        Ok(())
    }

    /// Execute defeat action
    fn execute_defeat_action(&self, _action: &ScriptAction) -> GameLogicResult<()> {
        log::info!("Defeat action executed");

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        engine.start_end_game_timer();
        Ok(())
    }

    /// Execute quick victory action
    fn execute_quick_victory_action(&self, _action: &ScriptAction) -> GameLogicResult<()> {
        log::info!("Quick victory action executed");

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        engine.start_quick_end_game_timer();
        Ok(())
    }

    /// Execute set flag action
    fn execute_set_flag_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let flag_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("SetFlag action missing flag parameter".to_string())
        })?;
        let value_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration("SetFlag action missing value parameter".to_string())
        })?;

        let flag_name = flag_param.get_string();
        let flag_value = value_param.get_int() != 0;

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        engine.set_flag(flag_name, flag_value)?;
        log::debug!("Set flag '{}' to {}", flag_name, flag_value);
        Ok(())
    }

    /// Execute set counter action
    fn execute_set_counter_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let counter_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("SetCounter action missing counter parameter".to_string())
        })?;
        let value_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration("SetCounter action missing value parameter".to_string())
        })?;

        let counter_name = counter_param.get_string();
        let counter_value = value_param.get_int();

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        engine.set_counter(counter_name, counter_value)?;
        log::debug!("Set counter '{}' to {}", counter_name, counter_value);
        Ok(())
    }

    /// Execute increment counter action
    fn execute_increment_counter_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let counter_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "IncrementCounter action missing counter parameter".to_string(),
            )
        })?;
        let value_param = action.get_parameter(1).map(|p| p.get_int()).unwrap_or(1);

        let counter_name = counter_param.get_string();

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        // Get current value and increment
        let current_value = engine
            .get_counter(counter_name)
            .map(|c| c.value)
            .unwrap_or(0);
        engine.set_counter(counter_name, current_value + value_param)?;
        log::debug!(
            "Incremented counter '{}' by {} to {}",
            counter_name,
            value_param,
            current_value + value_param
        );
        Ok(())
    }

    /// Execute decrement counter action
    fn execute_decrement_counter_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let counter_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "DecrementCounter action missing counter parameter".to_string(),
            )
        })?;
        let value_param = action.get_parameter(1).map(|p| p.get_int()).unwrap_or(1);

        let counter_name = counter_param.get_string();

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        // Get current value and decrement
        let current_value = engine
            .get_counter(counter_name)
            .map(|c| c.value)
            .unwrap_or(0);
        engine.set_counter(counter_name, current_value - value_param)?;
        log::debug!(
            "Decremented counter '{}' by {} to {}",
            counter_name,
            value_param,
            current_value - value_param
        );
        Ok(())
    }

    /// Execute set timer action
    fn execute_set_timer_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let counter_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("SetTimer action missing counter parameter".to_string())
        })?;
        let seconds_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration("SetTimer action missing seconds parameter".to_string())
        })?;

        let counter_name = counter_param.get_string();
        let seconds = seconds_param.get_int();
        let frames = seconds * 30; // 30 fps typical game framerate

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        engine.set_counter(counter_name, frames)?;

        // Mark as countdown timer
        let index = engine.allocate_counter(counter_name)?;
        if let Some(counter) = &mut engine.counters[index] {
            counter.is_countdown_timer = true;
        }

        log::debug!(
            "Set timer '{}' to {} seconds ({} frames)",
            counter_name,
            seconds,
            frames
        );
        Ok(())
    }

    /// Execute set millisecond timer action
    fn execute_set_millisecond_timer_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let counter_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "SetMillisecondTimer action missing counter parameter".to_string(),
            )
        })?;
        let milliseconds_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "SetMillisecondTimer action missing milliseconds parameter".to_string(),
            )
        })?;

        let counter_name = counter_param.get_string();
        let seconds = milliseconds_param.get_real();
        let frames = (seconds.max(0.0) * LOGICFRAMES_PER_SECOND as f32).ceil() as i32;

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        engine.set_counter(counter_name, frames)?;

        // Mark as countdown timer
        let index = engine.allocate_counter(counter_name)?;
        if let Some(counter) = &mut engine.counters[index] {
            counter.is_countdown_timer = true;
        }

        log::debug!(
            "Set millisecond timer '{}' to {} script-seconds ({} frames)",
            counter_name,
            seconds,
            frames
        );
        Ok(())
    }

    /// Execute display text action
    fn execute_display_text_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let text_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("DisplayText action missing text parameter".to_string())
        })?;

        let text = text_param.get_string().to_string();
        self.with_action_handler(|handler| handler.display_text(&text))
    }

    /// Execute play sound effect action
    fn execute_play_sound_effect_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let sound_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlaySoundEffect action missing sound parameter".to_string(),
            )
        })?;

        let sound_name = sound_param.get_string().to_string();
        self.with_action_handler(|handler| handler.play_sound_effect(&sound_name))
    }

    /// Execute enable script action
    fn execute_enable_script_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let script_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "EnableScript action missing script parameter".to_string(),
            )
        })?;

        let script_name = script_param.get_string().to_string();
        self.with_action_handler(|handler| handler.enable_script(&script_name, true))
    }

    /// Execute disable script action
    fn execute_disable_script_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let script_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "DisableScript action missing script parameter".to_string(),
            )
        })?;

        let script_name = script_param.get_string().to_string();
        self.with_action_handler(|handler| handler.enable_script(&script_name, false))
    }

    /// Execute freeze time action
    fn execute_freeze_time_action(&self, _action: &ScriptAction) -> GameLogicResult<()> {
        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        engine.do_freeze_time();
        Ok(())
    }

    /// Execute unfreeze time action
    fn execute_unfreeze_time_action(&self, _action: &ScriptAction) -> GameLogicResult<()> {
        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        engine.do_unfreeze_time();
        Ok(())
    }

    /// Execute player set money action
    fn execute_player_set_money_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let player_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerSetMoney action missing player parameter".to_string(),
            )
        })?;
        let amount_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerSetMoney action missing amount parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        let amount = amount_param.get_int();

        log::info!("Set player '{}' money to {}", player_name, amount);

        // In a real implementation, this would set the player's money
        Ok(())
    }

    /// Execute player give money action
    fn execute_player_give_money_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let player_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerGiveMoney action missing player parameter".to_string(),
            )
        })?;
        let amount_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerGiveMoney action missing amount parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        let amount = amount_param.get_int();

        log::info!("Give player '{}' {} money", player_name, amount);

        // In a real implementation, this would add money to the player
        Ok(())
    }
}

fn is_build_facility(obj: &crate::object::Object) -> bool {
    obj.is_kind_of(KindOf::Factory)
        || obj.is_kind_of(KindOf::CommandCenter)
        || obj.is_kind_of(KindOf::FSBarracks)
        || obj.is_kind_of(KindOf::FSWarfactory)
        || obj.is_kind_of(KindOf::FSAirfield)
        || obj.is_kind_of(KindOf::FSInternetCenter)
        || obj.is_kind_of(KindOf::FSPower)
        || obj.is_kind_of(KindOf::FSSupplyDropzone)
        || obj.is_kind_of(KindOf::FSSupplyCenter)
        || obj.is_kind_of(KindOf::FSSuperweapon)
        || obj.is_kind_of(KindOf::FSStrategyCenter)
}

#[cfg(test)]
mod tests {
    use super::super::engine::initialize_script_engine;
    use super::*;

    #[tokio::test]
    async fn test_script_evaluator_creation() {
        initialize_script_engine().unwrap();
        let engine = get_script_engine();
        let evaluator = ScriptEvaluator::new(engine);

        // Create a simple script to test
        let mut script = Script::new();
        script.set_name("test_script".to_string());

        // Should evaluate to true with no conditions
        let result = evaluator.evaluate_script(&mut script).unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_counter_condition() {
        initialize_script_engine().unwrap();
        let engine = get_script_engine();
        let evaluator = ScriptEvaluator::new(engine.clone());

        // Set up a counter
        {
            let mut engine_guard = engine.write().unwrap();
            let engine = engine_guard.as_mut().unwrap();
            engine.set_counter("test_counter", 50).unwrap();
        }

        // Create counter condition: counter >= 40
        let mut condition = Condition::new(ConditionType::Counter);
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::Counter,
                "test_counter".to_string(),
            ))
            .unwrap();
        condition
            .add_parameter(Parameter::with_int(ParameterType::Comparison, 3))
            .unwrap(); // GreaterEqual
        condition
            .add_parameter(Parameter::with_int(ParameterType::Int, 40))
            .unwrap();

        let result = evaluator.evaluate_condition(&mut condition).unwrap();
        assert!(result); // 50 >= 40 should be true
    }

    #[tokio::test]
    async fn test_flag_condition() {
        initialize_script_engine().unwrap();
        let engine = get_script_engine();
        let evaluator = ScriptEvaluator::new(engine.clone());

        // Set up a flag
        {
            let mut engine_guard = engine.write().unwrap();
            let engine = engine_guard.as_mut().unwrap();
            engine.set_flag("test_flag", true).unwrap();
        }

        // Create flag condition: flag == true
        let mut condition = Condition::new(ConditionType::Flag);
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::Flag,
                "test_flag".to_string(),
            ))
            .unwrap();
        condition
            .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
            .unwrap(); // true

        let result = evaluator.evaluate_condition(&mut condition).unwrap();
        assert!(result);
    }

    #[test]
    fn test_victory_action() {
        initialize_script_engine().unwrap();
        let engine = get_script_engine();
        let evaluator = ScriptEvaluator::new(engine.clone());

        let action = ScriptAction::new(ScriptActionType::Victory);
        evaluator.execute_action(&action).unwrap();

        // Check that end game timer was started
        let engine_guard = engine.read().unwrap();
        let engine = engine_guard.as_ref().unwrap();
        assert!(engine.is_game_ending());
    }
}
