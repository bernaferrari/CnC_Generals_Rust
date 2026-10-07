//! Main regressions for consumable PLAYER_ACQUIRED_SCIENCE events.
//! C++ authority: ScriptConditions.cpp:1543–1553 (acquired science is consumed)
//! and ScriptActions.cpp PLAYER_GRANT_SCIENCE (grants to ThePlayerList player).

use super::*;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::science::{
    SCIENCE_INVALID, ScienceInfo, ScienceStore, get_science_store, get_science_store_mut,
    init_science_store,
};
use gamelogic::player::{Player as LogicPlayer, PlayerList, player_list};
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::{ScriptEngine, get_script_engine};
use std::sync::{Arc, RwLock};

const SIDE: &str = "Local";
const SCIENCE: &str = "SCIENCE_EdgeOwnershipFixture";
const OBSERVER_A: &str = "science_edge_observer_a";
const OBSERVER_B: &str = "science_edge_observer_b";

fn truth_condition() -> Box<OrCondition> {
    let and = Condition::new(ConditionType::ConditionTrue);
    let mut result = OrCondition::new();
    result.set_first_and_condition(Some(Box::new(and)));
    Box::new(result)
}

fn acquired_condition() -> Box<OrCondition> {
    let mut condition = Condition::new(ConditionType::PlayerAcquiredScience);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Side, SIDE.into()))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Science,
            SCIENCE.into(),
        ))
        .unwrap();
    let mut result = OrCondition::new();
    result.set_first_and_condition(Some(Box::new(condition)));
    Box::new(result)
}

fn flag_action(name: &str) -> Box<ScriptAction> {
    let mut action = ScriptAction::new(ScriptActionType::SetFlag);
    action
        .add_parameter(Parameter::with_string(ParameterType::Flag, name.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
        .unwrap();
    Box::new(action)
}

fn grant_action() -> Box<ScriptAction> {
    // The core action resolves the side by name and the science through the
    // initialized ScienceStore, exactly like the authored script parameters.
    let mut action = ScriptAction::new(ScriptActionType::PlayerGrantScience);
    action
        .add_parameter(Parameter::with_string(ParameterType::Side, SIDE.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Science,
            SCIENCE.into(),
        ))
        .unwrap();
    Box::new(action)
}

fn script(name: &str, condition: Box<OrCondition>, action: Box<ScriptAction>) -> Box<Script> {
    let mut script = Script::new();
    script.script_name = name.into();
    script.is_one_shot = true;
    script.condition = Some(condition);
    script.action = Some(action);
    Box::new(script)
}

fn set_named_indexed_legacy_player() {
    let mut player = LogicPlayer::new(0);
    player.set_player_name_key(NameKeyGenerator::name_to_key(SIDE));
    let players = player_list();
    let mut list = players.write().expect("legacy player list");
    *list = PlayerList::new();
    list.add_player(Arc::new(RwLock::new(player)));
}

struct ScriptEngineSlotRestore(Option<ScriptEngine>);

impl ScriptEngineSlotRestore {
    fn install(replacement: Option<ScriptEngine>) -> Self {
        let handle = get_script_engine();
        let mut slot = handle.write().expect("script engine slot");
        Self(std::mem::replace(&mut *slot, replacement))
    }
}

impl Drop for ScriptEngineSlotRestore {
    fn drop(&mut self) {
        *get_script_engine().write().expect("script engine slot") = self.0.take();
    }
}

struct ScienceStoreRestore(Option<ScienceStore>);

impl ScienceStoreRestore {
    fn install() -> Self {
        // Exact-test child process isolates this real typed store fixture from
        // other tests without relying on retail INI assets being loaded there.
        if get_science_store_mut().is_none() {
            init_science_store();
        }
        let mut store = get_science_store_mut().expect("ScienceStore initialized");
        let previous = std::mem::replace(&mut *store, ScienceStore::new());
        store.add_science(ScienceInfo::new(SCIENCE_INVALID, SCIENCE));
        Self(Some(previous))
    }
}

impl Drop for ScienceStoreRestore {
    fn drop(&mut self) {
        *get_science_store_mut().expect("ScienceStore initialized") =
            self.0.take().expect("saved ScienceStore");
    }
}

fn engine_flag(name: &str) -> bool {
    gamelogic::scripting::engine::with_script_engine_ref(|engine| {
        engine.get_flag(name).is_some_and(|flag| flag.value)
    })
    .expect("script engine installed")
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn one_acquisition_edge_triggers_only_one_of_two_one_shot_observers() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "one_acquisition_edge_triggers_only_one_of_two_one_shot_observers",
        || {
            let _science_store = ScienceStoreRestore::install();
            let store = get_science_store().expect("ScienceStore initialized by test harness");
            let science = store.get_science_from_internal_name(SCIENCE);
            assert_ne!(science, game_engine::common::rts::science::SCIENCE_INVALID);
            assert!(store.is_science_grantable(science));
            drop(store);

            let mut world = GameLogic::new();
            world.add_player(Player::new(0, Team::USA, SIDE, true));
            set_named_indexed_legacy_player();

            let mut scripts = ScriptList::new();
            scripts.append_script(script("grant_once", truth_condition(), grant_action()));
            scripts.append_script(script(
                "observe_edge_a_once",
                acquired_condition(),
                flag_action(OBSERVER_A),
            ));
            // A second authored grant of an already-owned science must not
            // create a second addScience edge.
            scripts.append_script(script(
                "grant_duplicate_once",
                truth_condition(),
                grant_action(),
            ));
            scripts.append_script(script(
                "observe_edge_b_once",
                acquired_condition(),
                flag_action(OBSERVER_B),
            ));

            let mut engine = ScriptEngine::new().expect("script engine");
            engine
                .set_script_list_for_player(0, Some(Box::new(scripts)))
                .expect("script list");
            *get_script_engine().write().expect("script engine slot") = Some(engine);
            world.scripts_loaded = true;

            // Host census is captured before this tick's grant action, so the
            // observer result comes from the actual PlayerList acquisition edge.
            world.evaluate_and_execute_scripts(0.0);
            assert!(world.players[&0].has_unlocked_science(SCIENCE));
            assert!(engine_flag(OBSERVER_A), "first observer consumes the edge");
            assert!(
                !engine_flag(OBSERVER_B),
                "second observer has no second edge"
            );

            // Next tick's persistent host census still contains the science.
            // It must not recreate the already-consumed edge for observer B.
            world.frame += 1;
            world.evaluate_and_execute_scripts(0.0);
            assert!(engine_flag(OBSERVER_A));
            assert!(
                !engine_flag(OBSERVER_B),
                "level membership must not synthesize a repeat acquisition edge"
            );
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn persisted_host_science_without_playerlist_or_pending_edge_is_not_acquired() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "persisted_host_science_without_playerlist_or_pending_edge_is_not_acquired",
        || {
            let _science_store = ScienceStoreRestore::install();
            let mut world = GameLogic::new();
            let mut host = Player::new(0, Team::USA, SIDE, true);
            assert!(host.grant_science(SCIENCE));
            world.add_player(host);

            // The live census says the science is owned, but the matching
            // name/indexed legacy player is absent. C++ playerFromParam fails.
            *player_list().write().expect("legacy player list") = PlayerList::new();

            let mut scripts = ScriptList::new();
            scripts.append_script(script(
                "observe_missing_legacy_player",
                acquired_condition(),
                flag_action(OBSERVER_A),
            ));
            let mut engine = ScriptEngine::new().expect("script engine");
            engine
                .set_script_list_for_player(0, Some(Box::new(scripts)))
                .expect("script list");
            *get_script_engine().write().expect("script engine slot") = Some(engine);
            world.scripts_loaded = true;
            world.evaluate_and_execute_scripts(0.0);

            assert!(
                !engine_flag(OBSERVER_A),
                "missing legacy player fails closed"
            );
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn owned_science_before_fresh_engine_has_no_acquisition_event() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "owned_science_before_fresh_engine_has_no_acquisition_event",
        || {
            let _science_store = ScienceStoreRestore::install();
            let _slot_restore = ScriptEngineSlotRestore::install(None);

            let mut world = GameLogic::new();
            let mut host = Player::new(0, Team::USA, SIDE, true);
            assert!(host.grant_science(SCIENCE));
            world.add_player(host);

            // Grant into a named/indexed legacy Player while there is no
            // ScriptEngine installed, then start a fresh owner with an empty
            // acquired-science queue. This is persistent ownership only.
            let mut legacy = LogicPlayer::new(0);
            legacy.set_player_name_key(NameKeyGenerator::name_to_key(SIDE));
            {
                let handle = player_list();
                let mut list = handle.write().expect("legacy player list");
                *list = PlayerList::new();
                let player = Arc::new(RwLock::new(legacy));
                let science = get_science_store()
                    .unwrap()
                    .get_science_from_internal_name(SCIENCE);
                assert!(player.write().unwrap().grant_science(science));
                list.add_player(player);
            }

            let mut scripts = ScriptList::new();
            scripts.append_script(script(
                "observe_no_new_acquisition",
                acquired_condition(),
                flag_action(OBSERVER_A),
            ));
            let mut fresh = ScriptEngine::new().expect("fresh script engine");
            assert!(
                !fresh.is_science_acquired(
                    0,
                    get_science_store()
                        .unwrap()
                        .get_science_from_internal_name(SCIENCE),
                    false
                ),
                "fresh engine starts with no event tail"
            );
            fresh
                .set_script_list_for_player(0, Some(Box::new(scripts)))
                .expect("script list");
            *get_script_engine().write().expect("script engine slot") = Some(fresh);
            world.scripts_loaded = true;
            world.evaluate_and_execute_scripts(0.0);

            assert!(world.players[&0].has_unlocked_science(SCIENCE));
            assert!(
                !engine_flag(OBSERVER_A),
                "membership alone is not acquisition"
            );
        },
    );
}
