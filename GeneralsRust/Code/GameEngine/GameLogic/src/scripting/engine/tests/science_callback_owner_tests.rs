//! Player science callbacks retain the driving engine and pending event order.
use super::*;
use crate::common::NameKeyGenerator;
use crate::player::{Player, PlayerTemplate};
use crate::scripting::engine::get_script_engine;
use crate::scripting::executor::ScriptContext;
use crate::system::rank_info::{
    RankCatalog, RankDefinition, RankDefinitionMode, RankInfoStore as GameLogicRankInfoStore,
};
use game_engine::common::rts::player_template::PlayerTemplate as CommonPlayerTemplate;
use game_engine::common::rts::{SCIENCE_INVALID, ScienceType};
use std::sync::Arc;

/// Preserve the exact prior process slot, including its empty state.
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
        let handle = get_script_engine();
        let mut slot = handle.write().expect("script engine slot");
        *slot = self.0.take();
    }
}

/// A rank catalog owned by the test's players alone; no process or published
/// world catalog is touched.
fn rank_catalog(rank_sciences: &[Vec<ScienceType>]) -> RankCatalog {
    let mut rows = GameLogicRankInfoStore::new();
    for (index, sciences_granted) in rank_sciences.iter().enumerate() {
        let mut rank = RankDefinition::new(index + 1, RankDefinitionMode::Create);
        rank.sciences_granted = sciences_granted.clone();
        rows.apply_rank_definition(rank).expect("sequential authored rank");
    }
    RankCatalog::from_rows(rows)
}

fn science_test_engine(flag: &str) -> ScriptEngine {
    let mut engine = ScriptEngine::new_in_published_world().expect("script engine");
    let mut script = Script::new();
    script.script_name = format!("ScienceOwner_{flag}");
    script.is_one_shot = true;
    script.condition = Some(always_true_condition());
    script.action = Some(set_flag_action(flag));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .expect("install science owner script");
    engine
}

#[derive(Default)]
struct AddScienceObservations {
    first_add: Option<bool>,
    duplicate_add: Option<bool>,
    invalid_add: Option<bool>,
}

struct AddScienceDriver<'a> {
    player: &'a mut Player,
    science: ScienceType,
    observations: AddScienceObservations,
}

impl ScriptExecutionDriver for AddScienceDriver<'_> {
    fn after_action(&mut self) -> GameLogicResult<()> {
        // A single SetFlag action reaches this callback from the real update path.
        if self.observations.first_add.is_none() {
            self.observations.first_add = Some(self.player.add_science(self.science));
            self.observations.duplicate_add = Some(self.player.add_science(self.science));
            self.observations.invalid_add = Some(self.player.add_science(SCIENCE_INVALID));
        }
        Ok(())
    }
}

fn acquired(engine: &ScriptEngine) -> Vec<ScienceType> {
    engine.snapshot_xfer_tail().acquired_sciences[0]
        .iter()
        .map(|science| *science as ScienceType)
        .collect()
}

fn interleaved_science_updates(foreign_slot: bool) {
    let _guard = crate::test_sync::lock();
    let ranks = rank_catalog(&[Vec::new()]);
    let _slot = ScriptEngineSlotRestore::install(
        foreign_slot.then(|| ScriptEngine::new_in_published_world().expect("foreign engine")),
    );
    let science = NameKeyGenerator::name_to_key("SCIENCE_OWNER_CALLBACK") as ScienceType;
    let mut first_player = Player::with_rank_catalog(0, ranks.clone());
    let mut second_player = Player::with_rank_catalog(0, ranks.clone());
    let first_engine = science_test_engine("First");
    let second_engine = science_test_engine("Second");

    for (owner, player) in [
        (&first_engine, &mut first_player),
        (&second_engine, &mut second_player),
    ] {
        let mut driver = AddScienceDriver {
            player,
            science,
            observations: AddScienceObservations::default(),
        };
        owner
            .update_with_driver(ScriptContext::new(), &mut driver)
            .expect("owner's script update");
        assert_eq!(driver.observations.first_add, Some(true));
        assert_eq!(driver.observations.duplicate_add, Some(false));
        assert_eq!(driver.observations.invalid_add, Some(false));
        assert_eq!(driver.player.get_sciences(), &[science]);

        let foreign_pending = {
            let handle = get_script_engine();
            let slot = handle.read().expect("process slot");
            slot.as_ref().map(acquired).unwrap_or_default()
        };
        assert!(
            foreign_pending.is_empty(),
            "callback must not notify a foreign engine"
        );
        assert_eq!(
            acquired(owner),
            vec![science],
            "notification belongs to its driving engine"
        );
    }
    assert_eq!(acquired(&first_engine), vec![science]);
    assert_eq!(acquired(&second_engine), vec![science]);
}

#[test]
fn player_add_science_interleaves_owners_with_empty_process_slot() {
    interleaved_science_updates(false);
}

#[test]
fn player_add_science_interleaves_owners_with_foreign_process_slot() {
    interleaved_science_updates(true);
}

#[test]
fn reset_sciences_routes_authored_intrinsic_science_to_owner_and_save_tail() {
    let _guard = crate::test_sync::lock();
    let ranks = rank_catalog(&[Vec::new()]);
    let _empty_slot = ScriptEngineSlotRestore::install(None);

    // Common's authored intrinsic-science field is public. A unique template
    // name avoids accidentally hydrating an unrelated entry from the global
    // PlayerTemplateStore; reset_sciences reads the copied runtime template.
    let science_name = "SCIENCE_OWNER_RESET_INTRINSIC";
    let science = NameKeyGenerator::name_to_key(science_name) as ScienceType;
    let mut common = CommonPlayerTemplate::new("FactionScienceOwnerTest".into());
    common.intrinsic_sciences = vec![science_name.into()];
    let template = Arc::new(PlayerTemplate::from_common(&common));
    let mut player = Player::with_rank_catalog(0, ranks.clone());
    player.init(template);
    assert_eq!(player.get_sciences(), &[science]);

    struct ResetScienceDriver<'a> {
        player: &'a mut Player,
        invoked: bool,
    }
    impl ScriptExecutionDriver for ResetScienceDriver<'_> {
        fn after_action(&mut self) -> GameLogicResult<()> {
            if !self.invoked {
                self.player.reset_sciences();
                self.invoked = true;
            }
            Ok(())
        }
    }

    let owner = science_test_engine("Reset");
    let mut driver = ResetScienceDriver {
        player: &mut player,
        invoked: false,
    };
    owner
        .update_with_driver(ScriptContext::new(), &mut driver)
        .expect("reset owner's script update");
    assert!(driver.invoked);
    assert_eq!(player.get_sciences(), &[science]);
    assert_eq!(
        acquired(&owner),
        vec![science],
        "C++ resetSciences notifies every resulting science; the live save tail must own it"
    );
}

#[test]
fn player_rank_up_and_down_callbacks_publish_exact_reset_science_sequence() {
    let _guard = crate::test_sync::lock();
    let _empty_slot = ScriptEngineSlotRestore::install(None);
    let intrinsic = NameKeyGenerator::name_to_key("SCIENCE_RANK_TEST_INTRINSIC") as ScienceType;
    let rank_one = NameKeyGenerator::name_to_key("SCIENCE_RANK_TEST_ONE") as ScienceType;
    let rank_two = NameKeyGenerator::name_to_key("SCIENCE_RANK_TEST_TWO") as ScienceType;
    let ranks = rank_catalog(&[vec![rank_one], vec![rank_two]]);

    let mut common = CommonPlayerTemplate::new("FactionScienceRankOwnerTest".into());
    common.intrinsic_sciences = vec!["SCIENCE_RANK_TEST_INTRINSIC".into()];
    let template = Arc::new(PlayerTemplate::from_common(&common));
    let mut player = Player::with_rank_catalog(0, ranks.clone());
    player.init(template);
    assert_eq!(player.get_rank_level(), 1);
    assert_eq!(player.get_sciences(), &[intrinsic, rank_one]);

    struct RankDriver<'a> {
        player: &'a mut Player,
        rank_up: Option<bool>,
        rank_down: Option<bool>,
    }
    impl ScriptExecutionDriver for RankDriver<'_> {
        fn after_action(&mut self) -> GameLogicResult<()> {
            if self.rank_up.is_none() {
                self.rank_up = Some(self.player.set_rank_level(2, crate::player::DEFAULT_RANK_LEVEL_LIMIT));
                self.rank_down = Some(self.player.set_rank_level(1, crate::player::DEFAULT_RANK_LEVEL_LIMIT));
            }
            Ok(())
        }
    }

    let owner = science_test_engine("RankUpDown");
    let mut driver = RankDriver {
        player: &mut player,
        rank_up: None,
        rank_down: None,
    };
    owner
        .update_with_driver(ScriptContext::new(), &mut driver)
        .expect("rank owner's script update");
    assert_eq!(driver.rank_up, Some(true));
    assert_eq!(driver.rank_down, Some(true));
    assert_eq!(player.get_rank_level(), 1);
    assert_eq!(player.get_sciences(), &[intrinsic, rank_one]);
    assert_eq!(
        acquired(&owner),
        vec![rank_two, rank_one, intrinsic, rank_one],
        "rank-up adds rank 2 once; rank-down reset adds rank 1, then notifies intrinsic + rank 1"
    );
}

#[test]
fn acquired_science_consumption_removes_first_match_after_saved_tail_snapshot() {
    let _guard = crate::test_sync::lock();
    let _empty_slot = ScriptEngineSlotRestore::install(None);
    let owner = ScriptEngine::new_in_published_world().expect("script engine");
    let first = NameKeyGenerator::name_to_key("SCIENCE_FIRST") as ScienceType;
    let between = NameKeyGenerator::name_to_key("SCIENCE_BETWEEN") as ScienceType;

    owner.with_active_for_test(|| {
        owner.notify_of_acquired_science(0, first);
        owner.notify_of_acquired_science(0, between);
        owner.notify_of_acquired_science(0, first);
    });

    let before_consumption = owner.snapshot_xfer_tail();
    assert_eq!(
        before_consumption.acquired_sciences[0],
        vec![first as i32, between as i32, first as i32],
        "save/Xfer state must include ordered pending HAS_SCIENCE edges before consume"
    );

    let restored = ScriptEngine::new_in_published_world().expect("restored engine");
    restored.restore_xfer_tail(&before_consumption);
    assert_eq!(acquired(&restored), vec![first, between, first]);
    assert!(restored.is_science_acquired(0, first, true));
    assert_eq!(acquired(&restored), vec![between, first]);

    assert!(owner.is_science_acquired(0, first, true));
    assert_eq!(
        owner.snapshot_xfer_tail().acquired_sciences[0],
        vec![between as i32, first as i32],
        "C++ HAS_SCIENCE erases the first matching occurrence, preserving the later duplicate"
    );
    assert!(owner.is_science_acquired(0, first, true));
    assert!(!owner.is_science_acquired(0, first, true));
    assert!(owner.is_science_acquired(0, between, true));
}

#[test]
fn standalone_player_add_science_still_notifies_the_process_engine() {
    let _guard = crate::test_sync::lock();
    let _empty_slot = ScriptEngineSlotRestore::install(None);
    let ranks = rank_catalog(&[Vec::new()]);
    let science = NameKeyGenerator::name_to_key("SCIENCE_STANDALONE_OWNER") as ScienceType;
    let standalone = ScriptEngine::new_in_published_world().expect("standalone engine");
    let _standalone_slot = ScriptEngineSlotRestore::install(Some(standalone));

    let mut player = Player::with_rank_catalog(0, ranks.clone());
    assert!(player.add_science(science));
    let handle = get_script_engine();
    let slot = handle.read().expect("standalone slot");
    assert_eq!(
        acquired(slot.as_ref().expect("standalone owner")),
        vec![science]
    );
}
