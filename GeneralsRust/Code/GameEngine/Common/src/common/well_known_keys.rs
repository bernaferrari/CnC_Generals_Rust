//! Common Dict names resolve through the current NameKeyGenerator namespace.
//! Numeric keys are namespace-relative: caching one process-wide would alias
//! another thread's allocation order or an explicitly reset name table.

use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};

pub fn key_player_name() -> NameKeyType {
    NameKeyGenerator::name_to_key("playerName")
}

pub fn key_player_is_human() -> NameKeyType {
    NameKeyGenerator::name_to_key("playerIsHuman")
}

pub fn key_player_display_name() -> NameKeyType {
    NameKeyGenerator::name_to_key("playerDisplayName")
}

pub fn key_player_faction() -> NameKeyType {
    NameKeyGenerator::name_to_key("playerFaction")
}

pub fn key_player_allies() -> NameKeyType {
    NameKeyGenerator::name_to_key("playerAllies")
}

pub fn key_player_enemies() -> NameKeyType {
    NameKeyGenerator::name_to_key("playerEnemies")
}

pub fn key_player_is_skirmish() -> NameKeyType {
    NameKeyGenerator::name_to_key("playerIsSkirmish")
}

pub fn key_player_is_preorder() -> NameKeyType {
    NameKeyGenerator::name_to_key("playerIsPreorder")
}

pub fn key_multiplayer_start_index() -> NameKeyType {
    NameKeyGenerator::name_to_key("multiplayerStartIndex")
}

pub fn key_skirmish_difficulty() -> NameKeyType {
    NameKeyGenerator::name_to_key("skirmishDifficulty")
}

pub fn key_player_color() -> NameKeyType {
    NameKeyGenerator::name_to_key("playerColor")
}

pub fn key_player_night_color() -> NameKeyType {
    NameKeyGenerator::name_to_key("playerNightColor")
}

pub fn key_player_start_money() -> NameKeyType {
    NameKeyGenerator::name_to_key("playerStartMoney")
}

pub fn key_object_name() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectName")
}

pub fn key_object_script_attachment() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectScriptAttachment")
}

pub fn key_object_max_hps() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectMaxHPs")
}

pub fn key_object_initial_health() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectInitialHealth")
}

pub fn key_object_veterancy() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectVeterancy")
}

pub fn key_object_aggressiveness() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectAggressiveness")
}

pub fn key_object_recruitable_ai() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectRecruitableAI")
}

pub fn key_object_selectable() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectSelectable")
}

pub fn key_object_stopping_distance() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectStoppingDistance")
}

pub fn key_object_enabled() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectEnabled")
}

pub fn key_object_powered() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectPowered")
}

pub fn key_object_indestructible() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectIndestructible")
}

pub fn key_object_unsellable() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectUnsellable")
}

pub fn key_object_targetable() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectTargetable")
}

pub fn key_object_visual_range() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectVisualRange")
}

pub fn key_object_shroud_clearing_distance() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectShroudClearingDistance")
}

pub fn key_object_grant_upgrade() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectGrantUpgrade")
}

pub fn key_object_time() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectTime")
}

pub fn key_object_weather() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectWeather")
}

pub fn key_object_sound_ambient_enabled() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectSoundAmbientEnabled")
}

pub fn key_object_sound_ambient() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectSoundAmbient")
}

pub fn key_object_sound_ambient_customized() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectSoundAmbientCustomized")
}

pub fn key_object_sound_ambient_looping() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectSoundAmbientLooping")
}

pub fn key_object_sound_ambient_loop_count() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectSoundAmbientLoopCount")
}

pub fn key_object_sound_ambient_min_volume() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectSoundAmbientMinVolume")
}

pub fn key_object_sound_ambient_volume() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectSoundAmbientVolume")
}

pub fn key_object_sound_ambient_min_range() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectSoundAmbientMinRange")
}

pub fn key_object_sound_ambient_max_range() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectSoundAmbientMaxRange")
}

pub fn key_object_sound_ambient_priority() -> NameKeyType {
    NameKeyGenerator::name_to_key("objectSoundAmbientPriority")
}

pub fn key_team_name() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamName")
}

pub fn key_team_owner() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamOwner")
}

pub fn key_team_is_singleton() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamIsSingleton")
}

pub fn key_team_home() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamHome")
}

pub fn key_team_is_ai_recruitable() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamIsAIRecruitable")
}

pub fn key_team_is_base_defense() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamIsBaseDefense")
}

pub fn key_team_is_perimeter_defense() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamIsPerimeterDefense")
}

pub fn key_team_auto_reinforce() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamAutoReinforce")
}

pub fn key_team_aggressiveness() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamAggressiveness")
}

pub fn key_team_transports_return() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamTransportsReturn")
}

pub fn key_team_avoid_threats() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamAvoidThreats")
}

pub fn key_team_attack_common_target() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamAttackCommonTarget")
}

pub fn key_team_executes_actions_on_create() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamExecutesActionsOnCreate")
}

pub fn key_team_on_create_script() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamOnCreateScript")
}

pub fn key_team_on_idle_script() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamOnIdleScript")
}

pub fn key_team_on_unit_destroyed_script() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamOnUnitDestroyedScript")
}

pub fn key_team_on_destroyed_script() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamOnDestroyedScript")
}

pub fn key_team_enemy_sighted_script() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamEnemySightedScript")
}

pub fn key_team_all_clear_script() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamAllClearScript")
}

pub fn key_team_initial_idle_frames() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamInitialIdleFrames")
}

pub fn key_team_destroyed_threshold() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamDestroyedThreshold")
}

pub fn key_team_production_priority() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamProductionPriority")
}

pub fn key_team_production_priority_success_increase() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamProductionPrioritySuccessIncrease")
}

pub fn key_team_production_priority_failure_decrease() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamProductionPriorityFailureDecrease")
}

pub fn key_team_production_condition() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamProductionCondition")
}

pub fn key_team_generic_script_hook() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamGenericScriptHook")
}

pub fn key_team_unit_type1() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitType1")
}

pub fn key_team_unit_type2() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitType2")
}

pub fn key_team_unit_type3() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitType3")
}

pub fn key_team_unit_type4() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitType4")
}

pub fn key_team_unit_type5() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitType5")
}

pub fn key_team_unit_type6() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitType6")
}

pub fn key_team_unit_type7() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitType7")
}

pub fn key_team_unit_min_count1() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitMinCount1")
}

pub fn key_team_unit_min_count2() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitMinCount2")
}

pub fn key_team_unit_min_count3() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitMinCount3")
}

pub fn key_team_unit_min_count4() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitMinCount4")
}

pub fn key_team_unit_min_count5() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitMinCount5")
}

pub fn key_team_unit_min_count6() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitMinCount6")
}

pub fn key_team_unit_min_count7() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitMinCount7")
}

pub fn key_team_unit_max_count1() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitMaxCount1")
}

pub fn key_team_unit_max_count2() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitMaxCount2")
}

pub fn key_team_unit_max_count3() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitMaxCount3")
}

pub fn key_team_unit_max_count4() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitMaxCount4")
}

pub fn key_team_unit_max_count5() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitMaxCount5")
}

pub fn key_team_unit_max_count6() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitMaxCount6")
}

pub fn key_team_unit_max_count7() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamUnitMaxCount7")
}

pub fn key_team_max_instances() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamMaxInstances")
}

pub fn key_team_transport() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamTransport")
}

pub fn key_team_reinforcement_origin() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamReinforcementOrigin")
}

pub fn key_team_starts_full() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamStartsFull")
}

pub fn key_team_transports_exit() -> NameKeyType {
    NameKeyGenerator::name_to_key("teamTransportsExit")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::dict::Dict;

    #[test]
    fn well_known_keys_follow_each_receiving_name_namespace_and_reset() {
        let observe = |prefix_count: u32| {
            std::thread::spawn(move || {
                NameKeyGenerator::reset();
                for index in 0..prefix_count {
                    NameKeyGenerator::name_to_key(&format!("NamespacePrefix{index}"));
                }
                let mut dict = Dict::new();
                dict.set_ascii_string(key_team_name(), "DefaultTeam");
                dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
                let expected = NameKeyGenerator::name_to_key("teamProductionCondition");
                let key = key_team_production_condition();
                assert_eq!(key, expected);
                assert_eq!(
                    NameKeyGenerator::key_to_name(key).as_deref(),
                    Some("teamProductionCondition")
                );
                assert!(
                    dict.get_type(key).is_none(),
                    "missing production condition must not alias another Dict field"
                );
                assert!(dict.get_ascii_string(key).is_empty());
                NameKeyGenerator::reset();
                for index in 0..prefix_count + 11 {
                    NameKeyGenerator::name_to_key(&format!("AfterReset{index}"));
                }
                let after_reset = key_team_production_condition();
                assert_eq!(
                    NameKeyGenerator::key_to_name(after_reset).as_deref(),
                    Some("teamProductionCondition")
                );
                assert_eq!(
                    after_reset,
                    NameKeyGenerator::name_to_key("teamProductionCondition")
                );
                assert_eq!(key_team_production_condition(), after_reset);
                assert_ne!(
                    NameKeyGenerator::name_to_key("TeamProductionCondition"),
                    after_reset,
                    "case-sensitive original names remain distinct"
                );
                key
            })
            .join()
            .expect("namespace worker")
        };
        let first = observe(0);
        let second = observe(7);
        assert_ne!(
            first, second,
            "allocation order belongs to its own namespace"
        );
    }
}
