//! Script Conditions System
//!
//! This module provides condition evaluation for script triggers and decision making.

pub mod skirmish_conditions;

mod area;
mod combat;
mod helpers;
mod leftover;
mod logic;
mod multiplayer;
mod named;
mod object;
mod player;
mod registry;
mod team;

pub use super::{ScriptContext, ScriptValue};
pub use registry::ConditionRegistry;

pub use helpers::{
    HostObjectTriggerPersist, HostScriptPlayerCensus, HostScriptQueryObject,
    HostScriptQuerySnapshot, HostTechBuildingCensus, HostTriggerSlotPersist,
    capture_host_object_trigger_persists, clear_host_script_query_snapshot,
    clear_host_trigger_flags, host_bridge_broken, host_bridge_repaired,
    host_building_entered_by_player, host_count_player_kind_in_area,
    host_count_player_type_in_area, host_enemy_sighted, host_eval_skirmish_captured_count,
    host_eval_skirmish_command_button_ready, host_eval_skirmish_garrisoned_count,
    host_eval_skirmish_player_has_discovered_player,
    host_eval_skirmish_player_has_prerequisite_to_build,
    host_eval_skirmish_player_has_units_in_area, host_eval_skirmish_special_power_ready,
    host_eval_skirmish_supplies_value_within_distance,
    host_eval_skirmish_tech_building_within_distance,
    host_eval_skirmish_unowned_faction_unit_count, host_eval_skirmish_value_in_area,
    host_eval_team_has_object_status, host_eval_team_is_contained,
    host_eval_unit_has_object_status, host_object_did_enter, host_object_did_exit,
    host_object_has_kind, host_query_player_census, host_query_player_has_science,
    host_query_player_science_purchase_points, host_query_player_template_count,
    host_query_supply_source_attacked, host_query_supply_source_safe, host_script_area_bounds,
    host_script_area_unit_ids, host_script_lookup_polygon_trigger, host_script_named_unit_alive,
    host_script_named_unit_id, host_script_named_unit_in_area,
    host_script_named_unit_in_named_area, host_script_named_unit_present,
    host_script_named_unit_selected, host_script_query_has_any, host_script_query_object,
    host_script_query_object_by_id, host_script_team_member_ids, host_script_team_unit_ids,
    host_team_all_inside, host_team_did_all_enter, host_team_did_all_exit,
    host_team_did_partial_enter, host_team_did_partial_exit, host_team_has_any_live_objects,
    host_team_has_any_live_units, host_team_sequential_status, host_team_some_inside_some_outside,
    host_team_was_fielded, host_type_sighted, merge_host_script_query_snapshot,
    restore_host_object_trigger_persists, set_host_script_query_snapshot,
    sync_host_trigger_flags_from_snapshot, update_host_object_trigger_flags,
};
pub(crate) use helpers::{
    get_player_arc, get_str_param, lookup_named_object_id, perform_comparison,
};

use crate::GameLogicResult;
use async_trait::async_trait;
use std::collections::HashMap;

/// Script condition trait
#[async_trait]
pub trait ScriptCondition: Send + Sync {
    /// Evaluate the condition
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool>;

    /// Get condition name
    fn name(&self) -> &str;

    /// Get condition description
    fn description(&self) -> &str;

    /// Get required parameters
    fn required_parameters(&self) -> Vec<String>;

    /// Get optional parameters
    fn optional_parameters(&self) -> Vec<String>;
}

#[cfg(test)]
mod tests {
    use super::leftover::{BridgeBrokenCondition, BridgeRepairedCondition, GameTimeCondition};
    use super::logic::{FlagComparisonCondition, VariableEqualsCondition};
    use super::object::ObjectHealthCondition;
    use super::player::{PlayerHasResourceCondition, ResearchCompleteCondition};
    use super::*;
    use crate::player::player_list;
    use crate::scripting::engine::{get_named_object_tracker, get_script_engine};
    use crate::terrain::get_terrain_logic;
    use game_engine::common::rts::SCIENCE_INVALID;
    use std::collections::HashMap;
    use std::sync::{Arc, RwLock};
    use std::time::Duration;

    #[tokio::test]
    async fn test_condition_registry() {
        let registry = ConditionRegistry::new();

        let conditions = registry.list_conditions();
        assert!(conditions.contains(&"player_alive".to_string()));
        assert!(conditions.contains(&"object_exists".to_string()));
        assert!(conditions.contains(&"game_time".to_string()));
    }

    #[tokio::test]
    async fn test_game_time_condition() {
        let condition = GameTimeCondition;
        let mut params = HashMap::new();
        params.insert(
            "comparison".to_string(),
            ScriptValue::String("greater".to_string()),
        );
        params.insert("time".to_string(), ScriptValue::Float(30.0));

        let context = ScriptContext {
            game_time: Duration::from_secs(60),
            active_player: None,
            variables: HashMap::new(),
            game_state: crate::scripting::GameStateContext {
                map_name: "Test".to_string(),
                game_mode: "Test".to_string(),
                players: vec![],
                objectives: vec![],
            },
        };

        let result = condition.evaluate(&params, &context).await.unwrap();
        assert!(result); // 60 > 30
    }

    #[tokio::test]
    async fn test_variable_equals_condition() {
        let condition = VariableEqualsCondition;
        let mut params = HashMap::new();
        params.insert(
            "variable_name".to_string(),
            ScriptValue::String("test_var".to_string()),
        );
        params.insert("value".to_string(), ScriptValue::Int(42));

        let mut variables = HashMap::new();
        variables.insert("test_var".to_string(), ScriptValue::Int(42));

        let context = ScriptContext {
            game_time: Duration::from_secs(0),
            active_player: None,
            variables,
            game_state: crate::scripting::GameStateContext {
                map_name: "Test".to_string(),
                game_mode: "Test".to_string(),
                players: vec![],
                objectives: vec![],
            },
        };

        let result = condition.evaluate(&params, &context).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_object_health_condition() {
        use std::sync::{Arc, RwLock};

        use crate::common::Coord3D;
        use crate::common::DefaultThingTemplate;
        use crate::object_manager::{GameObjectInstance, ObjectCreationFlags, get_object_manager};

        if let Ok(mut manager) = get_object_manager().write() {
            manager.reset();

            let template = Arc::new(DefaultThingTemplate::new("TestObject".to_string()));
            let instance =
                GameObjectInstance::new(123, Some(template), None, ObjectCreationFlags::new())
                    .expect("failed to create object instance");
            manager
                .register_object_instance(instance, Coord3D::new(0.0, 0.0, 0.0))
                .unwrap();
        }

        let condition = ObjectHealthCondition;
        let mut params = HashMap::new();
        params.insert("object_id".to_string(), ScriptValue::Int(123));
        params.insert(
            "comparison".to_string(),
            ScriptValue::String("greater".to_string()),
        );
        params.insert("value".to_string(), ScriptValue::Float(50.0));

        let context = ScriptContext {
            game_time: Duration::from_secs(0),
            active_player: None,
            variables: HashMap::new(),
            game_state: crate::scripting::GameStateContext {
                map_name: "Test".to_string(),
                game_mode: "Test".to_string(),
                players: vec![],
                objectives: vec![],
            },
        };

        let result = condition.evaluate(&params, &context).await.unwrap();
        assert!(result);

        if let Ok(mut manager) = get_object_manager().write() {
            manager.reset();
        }
    }

    #[tokio::test]
    async fn bridge_conditions_use_terrain_bridge_damage_state() {
        use crate::common::{AsciiString, BodyDamageType};
        use crate::terrain::{BridgeInfo, get_terrain_logic};

        let bridge_name = "RegistryBridgeDamageState";
        let bridge_id = 0x00B1_D6E0;
        get_named_object_tracker()
            .register_named_object(bridge_name.to_string(), bridge_id)
            .expect("register bridge name");

        {
            let mut terrain = get_terrain_logic().write().expect("terrain write lock");
            terrain.reset();
            let mut info = BridgeInfo::new();
            info.bridge_object_id = bridge_id;
            info.cur_damage_state = BodyDamageType::Rubble;
            info.damage_state_changed = true;
            terrain.add_bridge_to_logic(info, AsciiString::from("TestBridgeTemplate"));
        }

        let context = ScriptContext {
            game_time: Duration::from_secs(0),
            active_player: None,
            variables: HashMap::new(),
            game_state: crate::scripting::GameStateContext {
                map_name: "Test".to_string(),
                game_mode: "Test".to_string(),
                players: vec![],
                objectives: vec![],
            },
        };
        let mut params = HashMap::new();
        params.insert(
            "bridge_name".to_string(),
            ScriptValue::String(bridge_name.to_string()),
        );

        assert!(
            BridgeBrokenCondition
                .evaluate(&params, &context)
                .await
                .expect("broken condition")
        );
        assert!(
            !BridgeRepairedCondition
                .evaluate(&params, &context)
                .await
                .expect("repaired condition")
        );

        {
            let mut terrain = get_terrain_logic().write().expect("terrain write lock");
            terrain.reset();
            let mut info = BridgeInfo::new();
            info.bridge_object_id = bridge_id;
            info.cur_damage_state = BodyDamageType::Damaged;
            info.damage_state_changed = true;
            terrain.add_bridge_to_logic(info, AsciiString::from("TestBridgeTemplate"));
        }

        assert!(
            !BridgeBrokenCondition
                .evaluate(&params, &context)
                .await
                .expect("broken condition after repair")
        );
        assert!(
            BridgeRepairedCondition
                .evaluate(&params, &context)
                .await
                .expect("repaired condition after repair")
        );

        get_terrain_logic()
            .write()
            .expect("terrain write lock")
            .reset();
    }

    #[tokio::test]
    async fn research_complete_checks_player_science_store() {
        use game_engine::common::rts::science::{
            ScienceInfo, get_science_store_mut, init_science_store,
        };

        init_science_store();
        let science_name = "SCIENCE_RegistryResearchComplete";
        let science = {
            let mut store = get_science_store_mut().expect("science store");
            store.add_science(ScienceInfo::new(SCIENCE_INVALID, science_name));
            store.get_science_from_internal_name(science_name)
        };
        assert_ne!(science, SCIENCE_INVALID);

        player_list().write().unwrap().clear();
        let player = Arc::new(RwLock::new(crate::player::Player::new(0)));
        player.write().unwrap().add_science(science);
        player_list().write().unwrap().add_player(player);

        let context = ScriptContext {
            game_time: Duration::from_secs(0),
            active_player: None,
            variables: HashMap::new(),
            game_state: crate::scripting::GameStateContext {
                map_name: "Test".to_string(),
                game_mode: "Test".to_string(),
                players: vec![],
                objectives: vec![],
            },
        };
        let mut params = HashMap::new();
        params.insert("player".to_string(), ScriptValue::Int(0));
        params.insert(
            "science_name".to_string(),
            ScriptValue::String(science_name.to_string()),
        );

        assert!(
            ResearchCompleteCondition
                .evaluate(&params, &context)
                .await
                .expect("research complete condition")
        );
    }

    #[tokio::test]
    async fn flag_comparison_reads_script_engine_flags() {
        crate::scripting::engine::initialize_script_engine().expect("script engine");

        let flag_name = "registry_flag_comparison_reads_script_engine_flags";
        {
            let engine = get_script_engine();
            let mut engine_guard = engine.write().expect("script engine write lock");
            engine_guard
                .as_mut()
                .expect("script engine initialized")
                .set_flag(flag_name, true)
                .expect("set flag");
        }

        let mut variables = HashMap::new();
        variables.insert(flag_name.to_string(), ScriptValue::Bool(false));
        let context = ScriptContext {
            game_time: Duration::from_secs(0),
            active_player: None,
            variables,
            game_state: crate::scripting::GameStateContext {
                map_name: "Test".to_string(),
                game_mode: "Test".to_string(),
                players: vec![],
                objectives: vec![],
            },
        };
        let mut params = HashMap::new();
        params.insert(
            "flag_name".to_string(),
            ScriptValue::String(flag_name.to_string()),
        );
        params.insert("value".to_string(), ScriptValue::Bool(true));

        assert!(
            FlagComparisonCondition
                .evaluate(&params, &context)
                .await
                .expect("flag comparison condition")
        );
    }

    #[tokio::test]
    async fn player_has_resource_uses_money_aliases_like_resource_actions() {
        player_list().write().unwrap().clear();
        let player = Arc::new(RwLock::new(crate::player::Player::new(0)));
        player.write().unwrap().get_money_mut().set_money(800);
        player_list().write().unwrap().add_player(player);

        let context = ScriptContext {
            game_time: Duration::from_secs(0),
            active_player: None,
            variables: HashMap::new(),
            game_state: crate::scripting::GameStateContext {
                map_name: "Test".to_string(),
                game_mode: "Test".to_string(),
                players: vec![],
                objectives: vec![],
            },
        };
        let mut params = HashMap::new();
        params.insert("player".to_string(), ScriptValue::Int(0));
        params.insert(
            "resource_type".to_string(),
            ScriptValue::String("supplies".to_string()),
        );
        params.insert("amount".to_string(), ScriptValue::Int(800));

        assert!(
            PlayerHasResourceCondition
                .evaluate(&params, &context)
                .await
                .expect("supplies resource condition")
        );

        params.insert(
            "resource_type".to_string(),
            ScriptValue::String("oil".to_string()),
        );
        assert!(
            !PlayerHasResourceCondition
                .evaluate(&params, &context)
                .await
                .expect("unknown resource condition")
        );
    }
}
