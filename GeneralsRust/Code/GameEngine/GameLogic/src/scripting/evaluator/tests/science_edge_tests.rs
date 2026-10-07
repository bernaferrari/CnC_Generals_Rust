//! Science acquisition conditions consume the injected engine event queue.
use super::super::*;
use crate::player::{Player, PlayerList, player_list};
use crate::scripting::conditions::{HostScriptPlayerCensus, HostScriptQuerySnapshot};
use crate::scripting::core::{Condition, ConditionType, Parameter, ParameterType};
use crate::scripting::engine::{ScriptEngine, ScriptEngineHandle};
use crate::scripting::evaluator::ScriptEvaluator;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::science::{
    SCIENCE_INVALID, ScienceInfo, ScienceStore, get_science_store_mut, init_science_store,
};
use std::sync::{Arc, RwLock};

const SIDE: &str = "ScienceEvaluatorOwner";
const SCIENCE: &str = "SCIENCE_EvaluatorEdgeFixture";

struct ScienceStoreRestore(Option<ScienceStore>);

impl ScienceStoreRestore {
    fn install() -> Self {
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

struct PlayerListRestore(Option<PlayerList>);

impl PlayerListRestore {
    fn install(player: Option<Player>) -> Self {
        let handle = player_list();
        let mut list = handle.write().expect("player list");
        let previous = std::mem::replace(&mut *list, PlayerList::new());
        if let Some(mut player) = player {
            player.set_player_name_key(NameKeyGenerator::name_to_key(SIDE));
            list.add_player(Arc::new(RwLock::new(player)));
        }
        Self(Some(previous))
    }
}

impl Drop for PlayerListRestore {
    fn drop(&mut self) {
        *player_list().write().expect("player list") = self.0.take().expect("saved PlayerList");
    }
}

struct HostScienceSnapshotRestore(Option<HostScriptQuerySnapshot>);

impl HostScienceSnapshotRestore {
    fn install() -> Self {
        let mut census = HostScriptPlayerCensus::default();
        census.unlocked_sciences.push(SCIENCE.to_string());
        let mut snapshot = HostScriptQuerySnapshot::default();
        snapshot
            .player_census
            .insert(SIDE.to_ascii_lowercase(), census);
        let mut previous = None;
        crate::scripting::merge_host_script_query_snapshot(|current| {
            previous = Some(std::mem::replace(current, snapshot));
        });
        Self(previous)
    }
}

impl Drop for HostScienceSnapshotRestore {
    fn drop(&mut self) {
        crate::scripting::merge_host_script_query_snapshot(|current| {
            *current = self.0.take().expect("saved host query snapshot");
        });
    }
}

fn acquired_science_condition(side: &str) -> Condition {
    let mut condition = Condition::new(ConditionType::PlayerAcquiredScience);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Side, side.into()))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Science,
            SCIENCE.into(),
        ))
        .unwrap();
    condition
}

fn fixture(pending_events: usize) -> (ScriptEvaluator, ScriptEngineHandle, i32) {
    let science = {
        let store =
            game_engine::common::rts::science::get_science_store().expect("authored ScienceStore");
        store.get_science_from_internal_name(SCIENCE)
    };
    let owner = ScriptEngine::new().expect("script engine");
    for _ in 0..pending_events {
        owner.notify_of_acquired_science(0, science);
    }
    let handle = ScriptEngineHandle::from_engine(owner);
    let evaluator = ScriptEvaluator::new(handle.clone());
    (evaluator, handle, science)
}

#[test]
fn one_pending_edge_is_consumed_once_even_while_census_stays_owned() {
    let _serial = crate::test_sync::lock();
    let _science = ScienceStoreRestore::install();
    let _players = PlayerListRestore::install(Some({
        let mut player = Player::new(0);
        player.set_display_name(SIDE);
        player
    }));
    let _snapshot = HostScienceSnapshotRestore::install();

    let (evaluator, handle, science) = fixture(1);
    let mut condition = acquired_science_condition(SIDE);
    assert!(evaluator.evaluate_condition(&mut condition).unwrap());
    assert!(
        !handle
            .read()
            .unwrap()
            .as_ref()
            .unwrap()
            .is_science_acquired(0, science, false),
        "the one pending event was consumed"
    );
    assert!(
        !evaluator.evaluate_condition(&mut condition).unwrap(),
        "persistent membership must not synthesize a second acquired edge"
    );
}

#[test]
fn matching_pending_tail_is_consumed_before_any_census_fallback() {
    let _serial = crate::test_sync::lock();
    let _science = ScienceStoreRestore::install();
    let _players = PlayerListRestore::install(Some({
        let mut player = Player::new(0);
        player.set_display_name(SIDE);
        player
    }));
    let _snapshot = HostScienceSnapshotRestore::install();

    let (evaluator, handle, science) = fixture(2);
    let mut condition = acquired_science_condition(SIDE);
    assert!(evaluator.evaluate_condition(&mut condition).unwrap());
    assert!(
        handle
            .read()
            .unwrap()
            .as_ref()
            .unwrap()
            .is_science_acquired(0, science, false)
    );
    assert!(evaluator.evaluate_condition(&mut condition).unwrap());
    assert!(
        !handle
            .read()
            .unwrap()
            .as_ref()
            .unwrap()
            .is_science_acquired(0, science, false)
    );
    assert!(
        !evaluator.evaluate_condition(&mut condition).unwrap(),
        "after the matching pending tail drains, census membership is not a new event"
    );
}

#[test]
fn persisted_membership_cannot_replace_player_resolution_or_pending_edge() {
    let _serial = crate::test_sync::lock();
    let _science = ScienceStoreRestore::install();
    let _players = PlayerListRestore::install(None);
    let _snapshot = HostScienceSnapshotRestore::install();

    let (evaluator, _handle, _science) = fixture(0);
    let mut condition = acquired_science_condition(SIDE);
    assert!(
        !evaluator.evaluate_condition(&mut condition).unwrap(),
        "C++ playerFromParam fails closed when the named player is absent"
    );
    let mut unknown = acquired_science_condition("NoSuchScienceOwner");
    assert!(!evaluator.evaluate_condition(&mut unknown).unwrap());
}
