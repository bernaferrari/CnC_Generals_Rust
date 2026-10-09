//! AIUpdate.cpp:4471-4648, queried through the actual factory-cached AI.
use super::*;
use crate::common::{INVALID_ID, ObjectStatusTypes};
use game_engine::common::random_value::{
    get_game_logic_random_seed_state, set_game_logic_random_seed_state,
};

struct MoodFixture {
    _factory: ObjectFactory,
    source: Arc<RwLock<crate::object::Object>>,
    ai: Arc<Mutex<dyn AIUpdateInterface>>,
    source_id: u32,
    target_id: u32,
}

impl MoodFixture {
    fn new(auto_acquire: &str) -> Self {
        Self::configured(auto_acquire, false)
    }

    fn with_primary_turret() -> Self {
        Self::configured("No", true)
    }

    fn configured(auto_acquire: &str, primary_turret: bool) -> Self {
        definitions();
        let turret_ini = if primary_turret {
            " Turret\n  ControlledWeaponSlots = PRIMARY\n  MinIdleScanInterval = 0\n  MaxIdleScanInterval = 0\n End\n"
        } else {
            ""
        };
        // These are loaded AI-rule inputs in a game; a bare AI's INI-backed
        // defaults are zero. Configure the real rule store for this fixture.
        crate::ai::the_ai()
            .write()
            .unwrap()
            .update_ai_data(|rules| {
                rules.guard_outer_modifier_ai = 1.0;
                rules.guard_outer_modifier_human = 1.0;
                rules.alert_range_modifier = 1.0;
                rules.aggressive_range_modifier = 1.0;
            });
        assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(&format!(
            "Object MoodQueryAttacker\n KindOf = INFANTRY CAN_ATTACK\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface MoodAI\n AutoAcquireEnemiesWhenIdle = {auto_acquire}\n MoodAttackCheckRate = 2000\n{turret_ini} End\n Locomotor = SET_NORMAL EnterCapacityLoco\nEnd\n"
        )), 1);
        {
            let mut players = ThePlayerList().write().unwrap();
            players.clear();
            for id in [0, 1] {
                let mut player = Player::new(id);
                player.set_player_type(crate::player::PlayerType::Computer, false);
                player
                    .set_player_relationship_by_index(1 - id, crate::common::Relationship::Enemies);
                players.add_player(Arc::new(RwLock::new(player)));
            }
        }
        let mut factory = ObjectFactory::new();
        let source_team = team("MoodSource", 9401, 0);
        let target_team = team("MoodTarget", 9402, 1);
        let source_id = create(
            &mut factory,
            "MoodQueryAttacker",
            Coord3D::new(0.0, 0.0, 0.0),
            Some(source_team),
            ObjectCreationFlags::empty(),
        );
        let (source, ai) = native_ai(&factory, source_id);
        install_attack_weapon(&source);
        source.write().unwrap().set_vision_range(1000.0);
        {
            let mut ai = ai.lock().unwrap();
            ai.execute_command(&AiCommandParams::new(
                AiCommandType::Idle,
                CommandSourceType::FromAI,
            ))
            .unwrap();
            ai.set_attitude(crate::modules::AIAttitudeType::Normal)
                .unwrap();
            assert_eq!(ai.get_current_state_id(), Some(AIStateType::Idle as u32));
        }
        let target_id = create(
            &mut factory,
            "EnterCapacityOccupant",
            Coord3D::new(30.0, 0.0, 0.0),
            Some(target_team),
            ObjectCreationFlags::NO_AI,
        );
        let target = factory
            .get_object(target_id)
            .unwrap()
            .get_base_object()
            .unwrap();
        {
            let source = source.read().unwrap();
            let target = target.read().unwrap();
            assert_eq!(
                source.relationship_to(&target),
                crate::common::Relationship::Enemies
            );
            assert_eq!(
                source.get_able_to_attack_specific_object_for_objects(
                    AbleToAttackType::NewTarget,
                    &target,
                    CommandSourceType::FromAi
                ),
                CanAttackResult::Possible
            );
        }
        crate::system::game_logic::get_game_logic()
            .lock()
            .unwrap()
            .partition_manager_mut()
            .update()
            .unwrap();
        let candidates = crate::helpers::ThePartitionManager::get()
            .unwrap()
            .get_objects_in_range(&Coord3D::new(0.0, 0.0, 0.0), 1000.0);
        assert!(
            candidates.contains(&target_id),
            "actual target must be indexed"
        );
        Self {
            _factory: factory,
            source,
            ai,
            source_id,
            target_id,
        }
    }
}

struct RestoreRng([u32; 6]);
impl RestoreRng {
    fn set() -> Self {
        let old = get_game_logic_random_seed_state();
        set_game_logic_random_seed_state([1, 2, 3, 4, 5, 6]);
        Self(old)
    }
}
impl Drop for RestoreRng {
    fn drop(&mut self) {
        set_game_logic_random_seed_state(self.0);
    }
}

#[test]
fn native_mood_non_ai_query_bypasses_future_timer_without_rng() {
    if !child(concat!(
        module_path!(),
        "::native_mood_non_ai_query_bypasses_future_timer_without_rng"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::RestoreAmbientFrame::set(17);
    let fixture = MoodFixture::new("Yes");
    let _rng = RestoreRng::set();
    let before = get_game_logic_random_seed_state();
    let mut ai = fixture.ai.lock().unwrap();
    ai.set_next_mood_check_time(1000);
    assert_eq!(ai.get_next_mood_target_id(true, false), INVALID_ID);
    assert_eq!(ai.get_next_mood_check_time(), 1000);
    assert_eq!(get_game_logic_random_seed_state(), before);
    assert_eq!(ai.get_next_mood_target_id(false, false), fixture.target_id);
    assert_eq!(ai.get_next_mood_check_time(), 1000);
    assert_eq!(get_game_logic_random_seed_state(), before);
}

#[test]
fn native_mood_due_query_updates_owned_timer_without_unrequested_rng() {
    if !child(concat!(
        module_path!(),
        "::native_mood_due_query_updates_owned_timer_without_unrequested_rng"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::RestoreAmbientFrame::set(17);
    let fixture = MoodFixture::new("Yes");
    let _rng = RestoreRng::set();
    let before = get_game_logic_random_seed_state();
    let mut ai = fixture.ai.lock().unwrap();
    ai.set_next_mood_check_time(1);
    assert_eq!(ai.get_next_mood_target_id(true, true), fixture.target_id);
    assert_eq!(
        ai.get_next_mood_check_time(),
        77,
        "2000 ms is 60 logic frames"
    );
    assert_eq!(get_game_logic_random_seed_state(), before);
    assert_eq!(ai.get_next_mood_target_id(true, true), INVALID_ID);
    assert_eq!(ai.get_next_mood_check_time(), 77);
}

#[test]
fn native_mood_uses_canonical_candidates_with_empty_handle_index() {
    if !child(concat!(
        module_path!(),
        "::native_mood_uses_canonical_candidates_with_empty_handle_index"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::RestoreAmbientFrame::set(17);
    let fixture = MoodFixture::new("Yes");
    let _rng = RestoreRng::set();
    let before = get_game_logic_random_seed_state();
    // Clearing the auxiliary handle index does not clear the canonical world.
    // Lookup must still reach its admitted target, and the due timer advances
    // before the search (AIUpdate.cpp:4545).
    crate::object::registry::OBJECT_REGISTRY.clear();
    assert!(crate::object::registry::OBJECT_REGISTRY.store_is_empty());
    assert!(!crate::object::registry::OBJECT_REGISTRY.is_empty());
    let mut ai = fixture.ai.lock().unwrap();
    ai.set_next_mood_check_time(1);
    assert_eq!(ai.get_next_mood_target_id(true, true), fixture.target_id);
    assert_eq!(ai.get_next_mood_check_time(), 77);
    assert_eq!(get_game_logic_random_seed_state(), before);
}

#[test]
fn native_mood_idle_and_ability_vetoes_precede_timer() {
    if !child(concat!(
        module_path!(),
        "::native_mood_idle_and_ability_vetoes_precede_timer"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::RestoreAmbientFrame::set(17);
    let fixture = MoodFixture::new("No");
    let _rng = RestoreRng::set();
    let before = get_game_logic_random_seed_state();
    let mut ai = fixture.ai.lock().unwrap();
    ai.set_next_mood_check_time(1);
    assert_eq!(ai.get_next_mood_target_id(true, true), INVALID_ID);
    assert_eq!(ai.get_next_mood_check_time(), 1);
    fixture
        .source
        .write()
        .unwrap()
        .set_status(ObjectStatusTypes::IsUsingAbility.into(), true);
    assert_eq!(ai.get_next_mood_target_id(true, false), INVALID_ID);
    assert_eq!(ai.get_next_mood_check_time(), 1);
    fixture
        .source
        .write()
        .unwrap()
        .set_status(ObjectStatusTypes::IsUsingAbility.into(), false);
    assert_eq!(ai.get_next_mood_target_id(true, false), fixture.target_id);
    assert_eq!(ai.get_next_mood_check_time(), 77);
    assert_eq!(get_game_logic_random_seed_state(), before);
}

#[test]
fn native_mood_query_and_blocked_fields_ignore_locked_foreign_unit() {
    if !child(concat!(
        module_path!(),
        "::native_mood_query_and_blocked_fields_ignore_locked_foreign_unit"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::RestoreAmbientFrame::set(17);
    let fixture = MoodFixture::new("Yes");
    let foreign_owner = Arc::new(RwLock::new(crate::object::Object::new_test(
        fixture.source_id,
        200.0,
    )));
    let foreign = Arc::new(RwLock::new(
        super::super::super::identity::Unit::new(
            Arc::clone(&foreign_owner),
            &crate::common::DefaultThingTemplate::new("MoodForeignUnit".into()),
        )
        .unwrap(),
    ));
    crate::object::registry::OBJECT_REGISTRY.register_object(fixture.source_id, &foreign_owner);
    crate::ai::object_registry::register_legacy_object(&foreign_owner);
    super::super::super::registry::register_unit(fixture.source_id, &foreign);
    let foreign_guard = foreign.write().unwrap();
    let _foreign_object_guard = foreign_owner.write().unwrap();
    let _rng = RestoreRng::set();
    let before = get_game_logic_random_seed_state();
    let mut ai = fixture.ai.lock().unwrap();
    ai.set_next_mood_check_time(1);
    assert!(!ai.is_attacking());
    assert!(!ai.is_blocked_and_stuck());
    assert_eq!(ai.get_num_frames_blocked(), 0);
    assert_eq!(ai.get_next_mood_target_id(true, true), fixture.target_id);
    assert_eq!(ai.get_next_mood_check_time(), 77);
    assert_eq!(get_game_logic_random_seed_state(), before);
    assert_eq!(foreign_guard.get_id(), fixture.source_id);
    drop(ai);
    drop(_foreign_object_guard);
    drop(foreign_guard);
    super::super::super::registry::unregister_unit(fixture.source_id);
}

#[test]
fn native_mood_wake_offset_consumes_exactly_one_logic_rng_draw() {
    if !child(concat!(
        module_path!(),
        "::native_mood_wake_offset_consumes_exactly_one_logic_rng_draw"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::RestoreAmbientFrame::set(17);
    let fixture = MoodFixture::new("Yes");
    let _rng = RestoreRng::set();
    let before = get_game_logic_random_seed_state();
    let expected_offset = game_engine::common::random_value::get_game_logic_random_value(-30, 30);
    let expected_words = get_game_logic_random_seed_state();
    set_game_logic_random_seed_state(before);
    let mut ai = fixture.ai.lock().unwrap();
    ai.wake_up_and_attempt_to_target();
    assert_eq!(ai.get_next_mood_check_time(), 17);
    assert_eq!(ai.get_next_mood_target_id(true, true), fixture.target_id);
    assert_eq!(
        ai.get_next_mood_check_time(),
        77u32.wrapping_add(expected_offset as u32)
    );
    assert_eq!(get_game_logic_random_seed_state(), expected_words);
    assert!(
        !ai.take_random_mood_offset(),
        "the query consumed the one-shot offset"
    );
}

#[test]
fn native_mood_attacking_veto_applies_outside_idle_query() {
    if !child(concat!(
        module_path!(),
        "::native_mood_attacking_veto_applies_outside_idle_query"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::RestoreAmbientFrame::set(17);
    let fixture = MoodFixture::new("Yes NOTWHILEATTACKING");
    let _rng = RestoreRng::set();
    let before = get_game_logic_random_seed_state();
    let mut ai = fixture.ai.lock().unwrap();
    let mut attack = AiCommandParams::new(AiCommandType::AttackObject, CommandSourceType::FromAI);
    attack.obj = Some(fixture.target_id);
    ai.execute_command(&attack).unwrap();
    assert_eq!(
        ai.get_current_state_id(),
        Some(AIStateType::AttackObject as u32)
    );
    assert!(ai.is_attacking());
    ai.set_next_mood_check_time(1);
    assert_eq!(ai.get_next_mood_target_id(true, false), INVALID_ID);
    assert_eq!(ai.get_next_mood_target_id(false, false), INVALID_ID);
    assert_eq!(ai.get_next_mood_check_time(), 1);
    assert_eq!(get_game_logic_random_seed_state(), before);
    ai.execute_command(&AiCommandParams::new(
        AiCommandType::Idle,
        CommandSourceType::FromAI,
    ))
    .unwrap();
    assert!(!ai.is_attacking());
    assert_eq!(ai.get_next_mood_target_id(false, false), fixture.target_id);
}

#[test]
fn native_mood_stealth_veto_only_applies_during_idle() {
    if !child(concat!(
        module_path!(),
        "::native_mood_stealth_veto_only_applies_during_idle"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::RestoreAmbientFrame::set(17);
    let fixture = MoodFixture::new("Yes");
    let mut ai = fixture.ai.lock().unwrap();
    ai.set_next_mood_check_time(1);
    fixture
        .source
        .write()
        .unwrap()
        .set_status(ObjectStatusTypes::Stealthed.into(), true);
    assert_eq!(ai.get_next_mood_target_id(true, true), INVALID_ID);
    assert_eq!(ai.get_next_mood_check_time(), 1);
    assert_eq!(ai.get_next_mood_target_id(false, false), fixture.target_id);
    fixture
        .source
        .write()
        .unwrap()
        .set_status(ObjectStatusTypes::Stealthed.into(), false);
    assert_eq!(ai.get_next_mood_target_id(true, true), fixture.target_id);
}

#[test]
fn native_mood_passive_with_no_damage_has_no_target() {
    if !child(concat!(
        module_path!(),
        "::native_mood_passive_with_no_damage_has_no_target"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::RestoreAmbientFrame::set(17);
    let fixture = MoodFixture::new("Yes");
    assert!(fixture.source.read().unwrap().get_body_module().is_some());
    assert!(
        fixture
            .source
            .read()
            .unwrap()
            .get_last_damage_info()
            .is_none()
    );
    let mut ai = fixture.ai.lock().unwrap();
    ai.set_next_mood_check_time(1000);
    ai.set_attitude(crate::modules::AIAttitudeType::Passive)
        .unwrap();
    assert_eq!(ai.get_next_mood_target_id(false, false), INVALID_ID);
    assert_eq!(ai.get_next_mood_check_time(), 1000);
    ai.set_attitude(crate::modules::AIAttitudeType::Normal)
        .unwrap();
    assert_eq!(ai.get_next_mood_target_id(false, false), fixture.target_id);
}

#[path = "factory_mood_policy_tests.rs"]
mod policy;

#[path = "factory_target_filter_tests.rs"]
mod target_filters;

#[path = "factory_idle_callback_tests.rs"]
mod idle_callbacks;
