// well_known_keys.rs - central store for common Dict keys.

use once_cell::sync::OnceCell;

use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};

fn key_for(name: &'static str, slot: &OnceCell<NameKeyType>) -> NameKeyType {
    *slot.get_or_init(|| NameKeyGenerator::name_to_key(name))
}

static PLAYER_NAME: OnceCell<NameKeyType> = OnceCell::new();
static PLAYER_IS_HUMAN: OnceCell<NameKeyType> = OnceCell::new();
static PLAYER_DISPLAY_NAME: OnceCell<NameKeyType> = OnceCell::new();
static PLAYER_FACTION: OnceCell<NameKeyType> = OnceCell::new();
static PLAYER_ALLIES: OnceCell<NameKeyType> = OnceCell::new();
static PLAYER_ENEMIES: OnceCell<NameKeyType> = OnceCell::new();
static PLAYER_IS_SKIRMISH: OnceCell<NameKeyType> = OnceCell::new();
static PLAYER_IS_PREORDER: OnceCell<NameKeyType> = OnceCell::new();
static MULTIPLAYER_START_INDEX: OnceCell<NameKeyType> = OnceCell::new();
static SKIRMISH_DIFFICULTY: OnceCell<NameKeyType> = OnceCell::new();
static PLAYER_COLOR: OnceCell<NameKeyType> = OnceCell::new();
static PLAYER_NIGHT_COLOR: OnceCell<NameKeyType> = OnceCell::new();
static PLAYER_START_MONEY: OnceCell<NameKeyType> = OnceCell::new();

static OBJECT_NAME: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_SCRIPT_ATTACHMENT: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_MAX_HPS: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_INITIAL_HEALTH: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_VETERANCY: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_AGGRESSIVENESS: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_RECRUITABLE_AI: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_SELECTABLE: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_STOPPING_DISTANCE: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_ENABLED: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_POWERED: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_INDESTRUCTIBLE: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_UNSELLABLE: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_TARGETABLE: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_VISUAL_RANGE: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_SHROUD_CLEARING_DISTANCE: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_GRANT_UPGRADE: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_TIME: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_WEATHER: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_SOUND_AMBIENT_ENABLED: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_SOUND_AMBIENT: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_SOUND_AMBIENT_CUSTOMIZED: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_SOUND_AMBIENT_LOOPING: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_SOUND_AMBIENT_LOOP_COUNT: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_SOUND_AMBIENT_MIN_VOLUME: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_SOUND_AMBIENT_VOLUME: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_SOUND_AMBIENT_MIN_RANGE: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_SOUND_AMBIENT_MAX_RANGE: OnceCell<NameKeyType> = OnceCell::new();
static OBJECT_SOUND_AMBIENT_PRIORITY: OnceCell<NameKeyType> = OnceCell::new();

static TEAM_NAME: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_OWNER: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_IS_SINGLETON: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_IS_AI_RECRUITABLE: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_IS_BASE_DEFENSE: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_IS_PERIMETER_DEFENSE: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_AUTO_REINFORCE: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_AGGRESSIVENESS: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_TRANSPORTS_RETURN: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_AVOID_THREATS: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_ATTACK_COMMON_TARGET: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_EXECUTES_ACTIONS_ON_CREATE: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_ON_CREATE_SCRIPT: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_ON_IDLE_SCRIPT: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_ON_UNIT_DESTROYED_SCRIPT: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_ON_DESTROYED_SCRIPT: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_ENEMY_SIGHTED_SCRIPT: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_ALL_CLEAR_SCRIPT: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_INITIAL_IDLE_FRAMES: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_DESTROYED_THRESHOLD: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_PRODUCTION_PRIORITY: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_PRODUCTION_PRIORITY_SUCCESS_INCREASE: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_PRODUCTION_PRIORITY_FAILURE_DECREASE: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_PRODUCTION_CONDITION: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_GENERIC_SCRIPT_HOOK: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_TYPE1: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_TYPE2: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_TYPE3: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_TYPE4: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_TYPE5: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_TYPE6: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_TYPE7: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_MIN_COUNT1: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_MIN_COUNT2: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_MIN_COUNT3: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_MIN_COUNT4: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_MIN_COUNT5: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_MIN_COUNT6: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_MIN_COUNT7: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_MAX_COUNT1: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_MAX_COUNT2: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_MAX_COUNT3: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_MAX_COUNT4: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_MAX_COUNT5: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_MAX_COUNT6: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_UNIT_MAX_COUNT7: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_MAX_INSTANCES: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_TRANSPORT: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_REINFORCEMENT_ORIGIN: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_STARTS_FULL: OnceCell<NameKeyType> = OnceCell::new();
static TEAM_TRANSPORTS_EXIT: OnceCell<NameKeyType> = OnceCell::new();

pub fn key_player_name() -> NameKeyType {
    key_for("playerName", &PLAYER_NAME)
}

pub fn key_player_is_human() -> NameKeyType {
    key_for("playerIsHuman", &PLAYER_IS_HUMAN)
}

pub fn key_player_display_name() -> NameKeyType {
    key_for("playerDisplayName", &PLAYER_DISPLAY_NAME)
}

pub fn key_player_faction() -> NameKeyType {
    key_for("playerFaction", &PLAYER_FACTION)
}

pub fn key_player_allies() -> NameKeyType {
    key_for("playerAllies", &PLAYER_ALLIES)
}

pub fn key_player_enemies() -> NameKeyType {
    key_for("playerEnemies", &PLAYER_ENEMIES)
}

pub fn key_player_is_skirmish() -> NameKeyType {
    key_for("playerIsSkirmish", &PLAYER_IS_SKIRMISH)
}

pub fn key_player_is_preorder() -> NameKeyType {
    key_for("playerIsPreorder", &PLAYER_IS_PREORDER)
}

pub fn key_multiplayer_start_index() -> NameKeyType {
    key_for("multiplayerStartIndex", &MULTIPLAYER_START_INDEX)
}

pub fn key_skirmish_difficulty() -> NameKeyType {
    key_for("skirmishDifficulty", &SKIRMISH_DIFFICULTY)
}

pub fn key_player_color() -> NameKeyType {
    key_for("playerColor", &PLAYER_COLOR)
}

pub fn key_player_night_color() -> NameKeyType {
    key_for("playerNightColor", &PLAYER_NIGHT_COLOR)
}

pub fn key_player_start_money() -> NameKeyType {
    key_for("playerStartMoney", &PLAYER_START_MONEY)
}

pub fn key_object_name() -> NameKeyType {
    key_for("objectName", &OBJECT_NAME)
}

pub fn key_object_script_attachment() -> NameKeyType {
    key_for("objectScriptAttachment", &OBJECT_SCRIPT_ATTACHMENT)
}

pub fn key_object_max_hps() -> NameKeyType {
    key_for("objectMaxHPs", &OBJECT_MAX_HPS)
}

pub fn key_object_initial_health() -> NameKeyType {
    key_for("objectInitialHealth", &OBJECT_INITIAL_HEALTH)
}

pub fn key_object_veterancy() -> NameKeyType {
    key_for("objectVeterancy", &OBJECT_VETERANCY)
}

pub fn key_object_aggressiveness() -> NameKeyType {
    key_for("objectAggressiveness", &OBJECT_AGGRESSIVENESS)
}

pub fn key_object_recruitable_ai() -> NameKeyType {
    key_for("objectRecruitableAI", &OBJECT_RECRUITABLE_AI)
}

pub fn key_object_selectable() -> NameKeyType {
    key_for("objectSelectable", &OBJECT_SELECTABLE)
}

pub fn key_object_stopping_distance() -> NameKeyType {
    key_for("objectStoppingDistance", &OBJECT_STOPPING_DISTANCE)
}

pub fn key_object_enabled() -> NameKeyType {
    key_for("objectEnabled", &OBJECT_ENABLED)
}

pub fn key_object_powered() -> NameKeyType {
    key_for("objectPowered", &OBJECT_POWERED)
}

pub fn key_object_indestructible() -> NameKeyType {
    key_for("objectIndestructible", &OBJECT_INDESTRUCTIBLE)
}

pub fn key_object_unsellable() -> NameKeyType {
    key_for("objectUnsellable", &OBJECT_UNSELLABLE)
}

pub fn key_object_targetable() -> NameKeyType {
    key_for("objectTargetable", &OBJECT_TARGETABLE)
}

pub fn key_object_visual_range() -> NameKeyType {
    key_for("objectVisualRange", &OBJECT_VISUAL_RANGE)
}

pub fn key_object_shroud_clearing_distance() -> NameKeyType {
    key_for(
        "objectShroudClearingDistance",
        &OBJECT_SHROUD_CLEARING_DISTANCE,
    )
}

pub fn key_object_grant_upgrade() -> NameKeyType {
    key_for("objectGrantUpgrade", &OBJECT_GRANT_UPGRADE)
}

pub fn key_object_time() -> NameKeyType {
    key_for("objectTime", &OBJECT_TIME)
}

pub fn key_object_weather() -> NameKeyType {
    key_for("objectWeather", &OBJECT_WEATHER)
}

pub fn key_object_sound_ambient_enabled() -> NameKeyType {
    key_for("objectSoundAmbientEnabled", &OBJECT_SOUND_AMBIENT_ENABLED)
}

pub fn key_object_sound_ambient() -> NameKeyType {
    key_for("objectSoundAmbient", &OBJECT_SOUND_AMBIENT)
}

pub fn key_object_sound_ambient_customized() -> NameKeyType {
    key_for(
        "objectSoundAmbientCustomized",
        &OBJECT_SOUND_AMBIENT_CUSTOMIZED,
    )
}

pub fn key_object_sound_ambient_looping() -> NameKeyType {
    key_for("objectSoundAmbientLooping", &OBJECT_SOUND_AMBIENT_LOOPING)
}

pub fn key_object_sound_ambient_loop_count() -> NameKeyType {
    key_for(
        "objectSoundAmbientLoopCount",
        &OBJECT_SOUND_AMBIENT_LOOP_COUNT,
    )
}

pub fn key_object_sound_ambient_min_volume() -> NameKeyType {
    key_for(
        "objectSoundAmbientMinVolume",
        &OBJECT_SOUND_AMBIENT_MIN_VOLUME,
    )
}

pub fn key_object_sound_ambient_volume() -> NameKeyType {
    key_for("objectSoundAmbientVolume", &OBJECT_SOUND_AMBIENT_VOLUME)
}

pub fn key_object_sound_ambient_min_range() -> NameKeyType {
    key_for(
        "objectSoundAmbientMinRange",
        &OBJECT_SOUND_AMBIENT_MIN_RANGE,
    )
}

pub fn key_object_sound_ambient_max_range() -> NameKeyType {
    key_for(
        "objectSoundAmbientMaxRange",
        &OBJECT_SOUND_AMBIENT_MAX_RANGE,
    )
}

pub fn key_object_sound_ambient_priority() -> NameKeyType {
    key_for("objectSoundAmbientPriority", &OBJECT_SOUND_AMBIENT_PRIORITY)
}

pub fn key_team_name() -> NameKeyType {
    key_for("teamName", &TEAM_NAME)
}

pub fn key_team_owner() -> NameKeyType {
    key_for("teamOwner", &TEAM_OWNER)
}

pub fn key_team_is_singleton() -> NameKeyType {
    key_for("teamIsSingleton", &TEAM_IS_SINGLETON)
}

pub fn key_team_is_ai_recruitable() -> NameKeyType {
    key_for("teamIsAIRecruitable", &TEAM_IS_AI_RECRUITABLE)
}

pub fn key_team_is_base_defense() -> NameKeyType {
    key_for("teamIsBaseDefense", &TEAM_IS_BASE_DEFENSE)
}

pub fn key_team_is_perimeter_defense() -> NameKeyType {
    key_for("teamIsPerimeterDefense", &TEAM_IS_PERIMETER_DEFENSE)
}

pub fn key_team_auto_reinforce() -> NameKeyType {
    key_for("teamAutoReinforce", &TEAM_AUTO_REINFORCE)
}

pub fn key_team_aggressiveness() -> NameKeyType {
    key_for("teamAggressiveness", &TEAM_AGGRESSIVENESS)
}

pub fn key_team_transports_return() -> NameKeyType {
    key_for("teamTransportsReturn", &TEAM_TRANSPORTS_RETURN)
}

pub fn key_team_avoid_threats() -> NameKeyType {
    key_for("teamAvoidThreats", &TEAM_AVOID_THREATS)
}

pub fn key_team_attack_common_target() -> NameKeyType {
    key_for("teamAttackCommonTarget", &TEAM_ATTACK_COMMON_TARGET)
}

pub fn key_team_executes_actions_on_create() -> NameKeyType {
    key_for(
        "teamExecutesActionsOnCreate",
        &TEAM_EXECUTES_ACTIONS_ON_CREATE,
    )
}

pub fn key_team_on_create_script() -> NameKeyType {
    key_for("teamOnCreateScript", &TEAM_ON_CREATE_SCRIPT)
}

pub fn key_team_on_idle_script() -> NameKeyType {
    key_for("teamOnIdleScript", &TEAM_ON_IDLE_SCRIPT)
}

pub fn key_team_on_unit_destroyed_script() -> NameKeyType {
    key_for("teamOnUnitDestroyedScript", &TEAM_ON_UNIT_DESTROYED_SCRIPT)
}

pub fn key_team_on_destroyed_script() -> NameKeyType {
    key_for("teamOnDestroyedScript", &TEAM_ON_DESTROYED_SCRIPT)
}

pub fn key_team_enemy_sighted_script() -> NameKeyType {
    key_for("teamEnemySightedScript", &TEAM_ENEMY_SIGHTED_SCRIPT)
}

pub fn key_team_all_clear_script() -> NameKeyType {
    key_for("teamAllClearScript", &TEAM_ALL_CLEAR_SCRIPT)
}

pub fn key_team_initial_idle_frames() -> NameKeyType {
    key_for("teamInitialIdleFrames", &TEAM_INITIAL_IDLE_FRAMES)
}

pub fn key_team_destroyed_threshold() -> NameKeyType {
    key_for("teamDestroyedThreshold", &TEAM_DESTROYED_THRESHOLD)
}

pub fn key_team_production_priority() -> NameKeyType {
    key_for("teamProductionPriority", &TEAM_PRODUCTION_PRIORITY)
}

pub fn key_team_production_priority_success_increase() -> NameKeyType {
    key_for(
        "teamProductionPrioritySuccessIncrease",
        &TEAM_PRODUCTION_PRIORITY_SUCCESS_INCREASE,
    )
}

pub fn key_team_production_priority_failure_decrease() -> NameKeyType {
    key_for(
        "teamProductionPriorityFailureDecrease",
        &TEAM_PRODUCTION_PRIORITY_FAILURE_DECREASE,
    )
}

pub fn key_team_production_condition() -> NameKeyType {
    key_for("teamProductionCondition", &TEAM_PRODUCTION_CONDITION)
}

pub fn key_team_generic_script_hook() -> NameKeyType {
    key_for("teamGenericScriptHook", &TEAM_GENERIC_SCRIPT_HOOK)
}

pub fn key_team_unit_type1() -> NameKeyType {
    key_for("teamUnitType1", &TEAM_UNIT_TYPE1)
}

pub fn key_team_unit_type2() -> NameKeyType {
    key_for("teamUnitType2", &TEAM_UNIT_TYPE2)
}

pub fn key_team_unit_type3() -> NameKeyType {
    key_for("teamUnitType3", &TEAM_UNIT_TYPE3)
}

pub fn key_team_unit_type4() -> NameKeyType {
    key_for("teamUnitType4", &TEAM_UNIT_TYPE4)
}

pub fn key_team_unit_type5() -> NameKeyType {
    key_for("teamUnitType5", &TEAM_UNIT_TYPE5)
}

pub fn key_team_unit_type6() -> NameKeyType {
    key_for("teamUnitType6", &TEAM_UNIT_TYPE6)
}

pub fn key_team_unit_type7() -> NameKeyType {
    key_for("teamUnitType7", &TEAM_UNIT_TYPE7)
}

pub fn key_team_unit_min_count1() -> NameKeyType {
    key_for("teamUnitMinCount1", &TEAM_UNIT_MIN_COUNT1)
}

pub fn key_team_unit_min_count2() -> NameKeyType {
    key_for("teamUnitMinCount2", &TEAM_UNIT_MIN_COUNT2)
}

pub fn key_team_unit_min_count3() -> NameKeyType {
    key_for("teamUnitMinCount3", &TEAM_UNIT_MIN_COUNT3)
}

pub fn key_team_unit_min_count4() -> NameKeyType {
    key_for("teamUnitMinCount4", &TEAM_UNIT_MIN_COUNT4)
}

pub fn key_team_unit_min_count5() -> NameKeyType {
    key_for("teamUnitMinCount5", &TEAM_UNIT_MIN_COUNT5)
}

pub fn key_team_unit_min_count6() -> NameKeyType {
    key_for("teamUnitMinCount6", &TEAM_UNIT_MIN_COUNT6)
}

pub fn key_team_unit_min_count7() -> NameKeyType {
    key_for("teamUnitMinCount7", &TEAM_UNIT_MIN_COUNT7)
}

pub fn key_team_unit_max_count1() -> NameKeyType {
    key_for("teamUnitMaxCount1", &TEAM_UNIT_MAX_COUNT1)
}

pub fn key_team_unit_max_count2() -> NameKeyType {
    key_for("teamUnitMaxCount2", &TEAM_UNIT_MAX_COUNT2)
}

pub fn key_team_unit_max_count3() -> NameKeyType {
    key_for("teamUnitMaxCount3", &TEAM_UNIT_MAX_COUNT3)
}

pub fn key_team_unit_max_count4() -> NameKeyType {
    key_for("teamUnitMaxCount4", &TEAM_UNIT_MAX_COUNT4)
}

pub fn key_team_unit_max_count5() -> NameKeyType {
    key_for("teamUnitMaxCount5", &TEAM_UNIT_MAX_COUNT5)
}

pub fn key_team_unit_max_count6() -> NameKeyType {
    key_for("teamUnitMaxCount6", &TEAM_UNIT_MAX_COUNT6)
}

pub fn key_team_unit_max_count7() -> NameKeyType {
    key_for("teamUnitMaxCount7", &TEAM_UNIT_MAX_COUNT7)
}

pub fn key_team_max_instances() -> NameKeyType {
    key_for("teamMaxInstances", &TEAM_MAX_INSTANCES)
}

pub fn key_team_transport() -> NameKeyType {
    key_for("teamTransport", &TEAM_TRANSPORT)
}

pub fn key_team_reinforcement_origin() -> NameKeyType {
    key_for("teamReinforcementOrigin", &TEAM_REINFORCEMENT_ORIGIN)
}

pub fn key_team_starts_full() -> NameKeyType {
    key_for("teamStartsFull", &TEAM_STARTS_FULL)
}

pub fn key_team_transports_exit() -> NameKeyType {
    key_for("teamTransportsExit", &TEAM_TRANSPORTS_EXIT)
}
