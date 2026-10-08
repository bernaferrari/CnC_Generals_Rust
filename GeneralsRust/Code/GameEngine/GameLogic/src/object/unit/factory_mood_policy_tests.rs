//! C++ AIUpdate.h:565, AIUpdate.cpp:4459-4648 and AI.cpp:780-835.
//! Query through the real factory AI and authored module data.
use super::*;
use crate::modules::AIAttitudeType;

fn target(fixture: &MoodFixture) -> Arc<RwLock<crate::object::Object>> {
    fixture
        ._factory
        .get_object(fixture.target_id)
        .unwrap()
        .get_base_object()
        .unwrap()
}

fn source_team(fixture: &MoodFixture) -> Arc<RwLock<Team>> {
    fixture.source.read().unwrap().get_team().unwrap()
}

fn fixture_without_controller() -> MoodFixture {
    let mut fixture = MoodFixture::new("Yes");
    // Admit the source on an already controller-free team. Changing a live
    // team's controller is a separate partition-maintenance operation.
    let team = Arc::new(RwLock::new(Team::new("MoodNoController".into(), 9410)));
    let id = create(
        &mut fixture._factory,
        "MoodQueryAttacker",
        Coord3D::ZERO,
        Some(team),
        ObjectCreationFlags::empty(),
    );
    let (source, ai) = native_ai(&fixture._factory, id);
    install_attack_weapon(&source);
    source.write().unwrap().set_vision_range(1000.0);
    ai.lock()
        .unwrap()
        .execute_command(&AiCommandParams::new(
            AiCommandType::Idle,
            CommandSourceType::FromAI,
        ))
        .unwrap();
    fixture.source = source;
    fixture.ai = ai;
    fixture.source_id = id;
    fixture
}

fn vision(fixture: &MoodFixture) -> f32 {
    // Supplying the actual driving attitude avoids locking the cached AI
    // twice. This is the same borrowed-source operation used by mood queries.
    let attitude = fixture.ai.lock().unwrap().get_attitude();
    let source = fixture.source.read().unwrap();
    crate::ai::the_ai()
        .read()
        .unwrap()
        .get_adjusted_vision_range_for_source(
            &source,
            crate::ai::vision_factors::OWNER_TYPE | crate::ai::vision_factors::MOOD,
            Some(attitude),
        )
}

#[test]
fn native_auto_acquire_reads_authored_mask_without_legacy_unit() {
    if !child(concat!(
        module_path!(),
        "::native_auto_acquire_reads_authored_mask_without_legacy_unit"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = MoodFixture::new("Yes STEALTHED");
    assert!(super::super::super::super::registry::get_unit_arc(fixture.source_id).is_none());
    let ai = fixture.ai.lock().unwrap();
    assert!(
        ai.can_auto_acquire(),
        "C++ returns whether the authored mask is nonzero"
    );
    assert!(ai.can_auto_acquire_while_stealthed());
}

#[test]
fn native_auto_acquire_zero_mask_is_false() {
    if !child(concat!(
        module_path!(),
        "::native_auto_acquire_zero_mask_is_false"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = MoodFixture::new("");
    let ai = fixture.ai.lock().unwrap();
    assert!(!ai.can_auto_acquire());
    assert!(!ai.can_auto_acquire_while_stealthed());
}

#[test]
fn native_auto_acquire_no_bit_is_still_a_nonzero_cpp_mask() {
    if !child(concat!(
        module_path!(),
        "::native_auto_acquire_no_bit_is_still_a_nonzero_cpp_mask"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = MoodFixture::new("No");
    let mut ai = fixture.ai.lock().unwrap();
    assert!(
        ai.can_auto_acquire(),
        "AIUpdate.h:565 converts the whole mask to Bool"
    );
    assert!(!ai.can_auto_acquire_while_stealthed());
    assert_eq!(ai.get_next_mood_target_id(false, true), INVALID_ID);
}

#[test]
fn native_stealthed_mask_allows_idle_scan_without_rng() {
    if !child(concat!(
        module_path!(),
        "::native_stealthed_mask_allows_idle_scan_without_rng"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::super::RestoreAmbientFrame::set(17);
    let fixture = MoodFixture::new("Yes STEALTHED");
    fixture
        .source
        .write()
        .unwrap()
        .set_status(ObjectStatusTypes::Stealthed.into(), true);
    let _rng = RestoreRng::set();
    let before = get_game_logic_random_seed_state();
    let mut ai = fixture.ai.lock().unwrap();
    ai.set_next_mood_check_time(1);
    assert_eq!(ai.get_next_mood_target_id(true, true), fixture.target_id);
    assert_eq!(ai.get_next_mood_check_time(), 77);
    assert_eq!(get_game_logic_random_seed_state(), before);
}

#[test]
fn native_vision_without_controller_ignores_sleep_mood() {
    if !child(concat!(
        module_path!(),
        "::native_vision_without_controller_ignores_sleep_mood"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = fixture_without_controller();
    assert!(
        fixture
            .source
            .read()
            .unwrap()
            .get_controlling_player()
            .is_none()
    );
    fixture
        .ai
        .lock()
        .unwrap()
        .set_attitude(AIAttitudeType::Sleep)
        .unwrap();
    assert_eq!(fixture.ai.lock().unwrap().get_mood_matrix_value(), 0);
    assert_eq!(
        vision(&fixture),
        1000.0,
        "C++ mood matrix zero has no Sleep bit"
    );
}

#[test]
fn native_vision_without_controller_ignores_aggressive_mood() {
    if !child(concat!(
        module_path!(),
        "::native_vision_without_controller_ignores_aggressive_mood"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = fixture_without_controller();
    crate::ai::the_ai()
        .write()
        .unwrap()
        .update_ai_data(|rules| rules.aggressive_range_modifier = 3.0);
    fixture
        .ai
        .lock()
        .unwrap()
        .set_attitude(AIAttitudeType::Aggressive)
        .unwrap();
    assert_eq!(fixture.ai.lock().unwrap().get_mood_matrix_value(), 0);
    assert_eq!(vision(&fixture), 1000.0);
}

#[test]
fn native_vision_applies_computer_mood_but_not_human_mood() {
    if !child(concat!(
        module_path!(),
        "::native_vision_applies_computer_mood_but_not_human_mood"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = MoodFixture::new("Yes");
    crate::ai::the_ai()
        .write()
        .unwrap()
        .update_ai_data(|rules| {
            rules.guard_outer_modifier_ai = 2.0;
            rules.guard_outer_modifier_human = 0.5;
            rules.aggressive_range_modifier = 3.0;
            rules.alert_range_modifier = 4.0;
        });
    fixture
        .ai
        .lock()
        .unwrap()
        .set_attitude(AIAttitudeType::Aggressive)
        .unwrap();
    assert_eq!(vision(&fixture), 6000.0);
    fixture
        .ai
        .lock()
        .unwrap()
        .set_attitude(AIAttitudeType::Defensive)
        .unwrap();
    assert_eq!(vision(&fixture), 8000.0);
    fixture
        .ai
        .lock()
        .unwrap()
        .set_attitude(AIAttitudeType::Sleep)
        .unwrap();
    assert_eq!(vision(&fixture), 0.0);
    fixture
        .source
        .read()
        .unwrap()
        .with_controlling_player_mut(|player| {
            player.set_player_type(crate::player::PlayerType::Human, false);
        })
        .unwrap();
    assert_eq!(vision(&fixture), 500.0, "human controller has no mood bits");
}

#[test]
fn factory_object_without_ai_has_zero_adjusted_vision() {
    if !child(concat!(
        module_path!(),
        "::factory_object_without_ai_has_zero_adjusted_vision"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = MoodFixture::new("Yes");
    let target = target(&fixture);
    target.write().unwrap().set_vision_range(600.0);
    assert!(target.read().unwrap().get_ai_update_interface().is_none());
    let range = crate::ai::the_ai()
        .read()
        .unwrap()
        .get_adjusted_vision_range_for_object(
            fixture.target_id,
            crate::ai::vision_factors::OWNER_TYPE | crate::ai::vision_factors::MOOD,
        )
        .unwrap();
    assert_eq!(
        range, 0.0,
        "C++ returns zero before adjustment when getAI is null"
    );
}

#[test]
fn native_mood_timer_and_offset_survive_base_xfer() {
    if !child(concat!(
        module_path!(),
        "::native_mood_timer_and_offset_survive_base_xfer"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::super::RestoreAmbientFrame::set(17);
    let fixture = MoodFixture::new("Yes");
    let _rng = RestoreRng::set();
    let before = get_game_logic_random_seed_state();
    // C++ LocomotorSet::xfer recreates the one admitted locomotor on load.
    // Locomotor.cpp:647-649 consumes these three constructor draws before
    // restoring its saved fields; the AI query then consumes one mood draw.
    game_engine::common::random_value::get_game_logic_random_value_real(
        -std::f32::consts::PI / 6.0,
        std::f32::consts::PI / 6.0,
    );
    game_engine::common::random_value::get_game_logic_random_value_real(0.8, 1.2);
    game_engine::common::random_value::get_game_logic_random_value(0, 1);
    let restored_rng = get_game_logic_random_seed_state();
    let offset = game_engine::common::random_value::get_game_logic_random_value(-30, 30);
    let expected = get_game_logic_random_seed_state();
    set_game_logic_random_seed_state(before);
    let mut ai = fixture.ai.lock().unwrap();
    ai.wake_up_and_attempt_to_target();
    let mut saved = Cursor::new(Vec::new());
    assert!(
        ai.xfer_ai_update_state(&mut XferSave::new(&mut saved, 1))
            .unwrap()
    );
    let saved = saved.into_inner();
    assert!(!saved.is_empty());
    ai.set_next_mood_check_time(1000);
    assert!(
        ai.xfer_ai_update_state(&mut XferLoad::new(Cursor::new(saved.clone()), 1))
            .unwrap()
    );
    let mut resaved = Cursor::new(Vec::new());
    assert!(
        ai.xfer_ai_update_state(&mut XferSave::new(&mut resaved, 1))
            .unwrap()
    );
    assert_eq!(resaved.into_inner(), saved);
    assert_eq!(ai.get_next_mood_check_time(), 17);
    assert_eq!(get_game_logic_random_seed_state(), restored_rng);
    assert_eq!(ai.get_next_mood_target_id(true, true), fixture.target_id);
    assert_eq!(
        ai.get_next_mood_check_time(),
        77u32.wrapping_add(offset as u32)
    );
    assert_eq!(get_game_logic_random_seed_state(), expected);
    assert!(!ai.take_random_mood_offset());
}

fn refresh_partition() {
    crate::system::game_logic::get_game_logic()
        .lock()
        .unwrap()
        .partition_manager_mut()
        .update()
        .unwrap();
}

#[test]
fn native_team_common_target_precedes_timer_and_rng() {
    if !child(concat!(
        module_path!(),
        "::native_team_common_target_precedes_timer_and_rng"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::super::RestoreAmbientFrame::set(17);
    let mut fixture = MoodFixture::new("Yes");
    let enemy_team = target(&fixture).read().unwrap().get_team().unwrap();
    let far_id = create(
        &mut fixture._factory,
        "EnterCapacityOccupant",
        Coord3D::new(500.0, 0.0, 0.0),
        Some(enemy_team),
        ObjectCreationFlags::NO_AI,
    );
    refresh_partition();
    let mut dict = game_engine::common::dict::Dict::new();
    dict.set_bool(
        game_engine::common::well_known_keys::key_team_attack_common_target(),
        true,
    );
    crate::team::get_team_factory()
        .lock()
        .unwrap()
        .init_team(
            "MoodSource".into(),
            "MoodSourceOwner".into(),
            false,
            Some(&dict),
        )
        .unwrap();
    let team = source_team(&fixture);
    {
        let mut team = team.write().unwrap();
        team.set_team_target_object(far_id);
        assert_eq!(team.get_team_target_object(), far_id);
        assert!(team.attack_common_target());
    }
    let _rng = RestoreRng::set();
    let before = get_game_logic_random_seed_state();
    let mut ai = fixture.ai.lock().unwrap();
    ai.set_next_mood_check_time(1000);
    assert_eq!(ai.get_next_mood_target_id(true, false), far_id);
    assert_eq!(ai.get_next_mood_check_time(), 1000);
    assert_eq!(get_game_logic_random_seed_state(), before);
    // CalledByAI=false bypasses both common-target and the timer.
    assert_eq!(ai.get_next_mood_target_id(false, false), fixture.target_id);
    ai.set_attitude(AIAttitudeType::Passive).unwrap();
    assert_eq!(ai.get_next_mood_target_id(true, false), INVALID_ID);
    ai.set_attitude(AIAttitudeType::Normal).unwrap();
    dict.set_bool(
        game_engine::common::well_known_keys::key_team_attack_common_target(),
        false,
    );
    crate::team::get_team_factory()
        .lock()
        .unwrap()
        .init_team(
            "MoodSource".into(),
            "MoodSourceOwner".into(),
            false,
            Some(&dict),
        )
        .unwrap();
    assert!(!team.read().unwrap().attack_common_target());
    assert_eq!(ai.get_next_mood_target_id(true, false), INVALID_ID);
    assert_eq!(ai.get_next_mood_check_time(), 1000);
    assert_eq!(get_game_logic_random_seed_state(), before);
}

#[test]
fn native_passive_damage_target_is_not_a_range_or_enemy_search() {
    if !child(concat!(
        module_path!(),
        "::native_passive_damage_target_is_not_a_range_or_enemy_search"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::super::RestoreAmbientFrame::set(17);
    let mut fixture = MoodFixture::new("Yes");
    let own_team = source_team(&fixture);
    let friendly_id = create(
        &mut fixture._factory,
        "EnterCapacityOccupant",
        Coord3D::new(1500.0, 0.0, 0.0),
        Some(own_team),
        ObjectCreationFlags::NO_AI,
    );
    refresh_partition();
    let friendly = fixture
        ._factory
        .get_object(friendly_id)
        .unwrap()
        .get_base_object()
        .unwrap();
    assert_eq!(
        fixture
            .source
            .read()
            .unwrap()
            .relationship_to(&friendly.read().unwrap()),
        crate::common::Relationship::Allies
    );
    let body = fixture.source.read().unwrap().get_body_module().unwrap();
    let mut damage = crate::damage::DamageInfo::with_simple(
        10.0,
        friendly_id,
        crate::damage::DamageType::Explosion,
        crate::damage::DeathType::Normal,
    );
    body.lock().unwrap().attempt_damage(&mut damage).unwrap();
    assert_eq!(
        fixture
            .source
            .read()
            .unwrap()
            .get_last_damage_info()
            .unwrap()
            .input
            .source_id,
        friendly_id
    );
    let _rng = RestoreRng::set();
    let before = get_game_logic_random_seed_state();
    {
        let mut ai = fixture.ai.lock().unwrap();
        ai.set_attitude(AIAttitudeType::Passive).unwrap();
        ai.set_next_mood_check_time(1000);
        assert_eq!(ai.get_next_mood_target_id(false, false), friendly_id);
        assert_eq!(ai.get_next_mood_check_time(), 1000);
    }
    let mut healing = crate::damage::DamageInfo::with_simple(
        5.0,
        friendly_id,
        crate::damage::DamageType::Healing,
        crate::damage::DeathType::Normal,
    );
    body.lock().unwrap().attempt_healing(&mut healing).unwrap();
    assert_eq!(
        fixture
            .source
            .read()
            .unwrap()
            .get_last_damage_info()
            .unwrap()
            .input
            .damage_type,
        crate::damage::DamageType::Healing
    );
    let mut ai = fixture.ai.lock().unwrap();
    assert_eq!(
        ai.get_next_mood_target_id(false, false),
        fixture.target_id,
        "healing falls through to the ordinary closest-enemy search"
    );
    assert_eq!(ai.get_next_mood_check_time(), 1000);
    assert_eq!(get_game_logic_random_seed_state(), before);
}

fn stealth_source(
    fixture: &mut MoodFixture,
    special_power: bool,
) -> Arc<Mutex<dyn AIUpdateInterface>> {
    let team = source_team(fixture);
    let name = if special_power {
        "MoodSpecialStealth"
    } else {
        "MoodOrdinaryStealth"
    };
    let granted = if special_power { "Yes" } else { "No" };
    assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(&format!(
        "Object {name}\n KindOf = INFANTRY CAN_ATTACK\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface MoodAI\n AutoAcquireEnemiesWhenIdle = Yes\n MoodAttackCheckRate = 2000\n End\n Behavior = StealthUpdate Stealth\n GrantedBySpecialPower = {granted}\n End\n Locomotor = SET_NORMAL EnterCapacityLoco\nEnd\n"
    )), 1);
    let id = create(
        &mut fixture._factory,
        name,
        Coord3D::ZERO,
        Some(team),
        ObjectCreationFlags::empty(),
    );
    let (owner, ai) = native_ai(&fixture._factory, id);
    install_attack_weapon(&owner);
    {
        let mut owner = owner.write().unwrap();
        owner.set_vision_range(1000.0);
        owner.set_status(ObjectStatusTypes::Stealthed.into(), true);
        let stealth = owner
            .get_stealth()
            .expect("factory on_object_created binds the real controller");
        assert_eq!(
            stealth.lock().unwrap().is_granted_by_special_power(),
            special_power
        );
    }
    {
        let mut ai = ai.lock().unwrap();
        ai.execute_command(&AiCommandParams::new(
            AiCommandType::Idle,
            CommandSourceType::FromAI,
        ))
        .unwrap();
        ai.set_attitude(AIAttitudeType::Normal).unwrap();
        ai.set_next_mood_check_time(1);
    }
    refresh_partition();
    ai
}

#[test]
fn native_special_power_stealth_exempts_idle_veto() {
    if !child(concat!(
        module_path!(),
        "::native_special_power_stealth_exempts_idle_veto"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::super::RestoreAmbientFrame::set(17);
    let mut fixture = MoodFixture::new("Yes");
    let ordinary = stealth_source(&mut fixture, false);
    let special = stealth_source(&mut fixture, true);
    let _rng = RestoreRng::set();
    let before = get_game_logic_random_seed_state();
    {
        let mut ai = ordinary.lock().unwrap();
        assert!(!ai.can_auto_acquire_while_stealthed());
        assert_eq!(ai.get_next_mood_target_id(true, true), INVALID_ID);
        assert_eq!(ai.get_next_mood_check_time(), 1);
    }
    let mut ai = special.lock().unwrap();
    assert!(ai.can_auto_acquire_while_stealthed());
    assert_eq!(ai.get_next_mood_target_id(true, true), fixture.target_id);
    assert_eq!(ai.get_next_mood_check_time(), 77);
    assert_eq!(get_game_logic_random_seed_state(), before);
}

#[test]
fn native_firing_container_exempts_idle_stealth_veto() {
    if !child(concat!(
        module_path!(),
        "::native_firing_container_exempts_idle_stealth_veto"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::super::RestoreAmbientFrame::set(17);
    let mut fixture = MoodFixture::new("Yes");
    assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
        "Object MoodFiringContainer\n KindOf = STRUCTURE IMMOBILE\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = OpenContain Cargo\n ContainMax = 4\n AllowInsideKindOf = INFANTRY\n PassengersAllowedToFire = No\n End\nEnd\n"
    ), 1);
    let team = source_team(&fixture);
    let id = create(
        &mut fixture._factory,
        "MoodFiringContainer",
        Coord3D::ZERO,
        Some(team),
        ObjectCreationFlags::NO_AI,
    );
    let container = fixture
        ._factory
        .get_object(id)
        .unwrap()
        .get_base_object()
        .unwrap();
    let contain = container.read().unwrap().get_contain().unwrap();
    contain
        .lock()
        .unwrap()
        .contain_object(fixture.source_id)
        .unwrap();
    assert_eq!(fixture.source.read().unwrap().get_contained_by(), Some(id));
    assert!(
        contain
            .lock()
            .unwrap()
            .get_contained_objects()
            .contains(&fixture.source_id)
    );
    fixture
        .source
        .write()
        .unwrap()
        .set_status(ObjectStatusTypes::Stealthed.into(), true);
    refresh_partition();
    let _rng = RestoreRng::set();
    let before = get_game_logic_random_seed_state();
    let mut ai = fixture.ai.lock().unwrap();
    ai.set_next_mood_check_time(1);
    assert!(!ai.can_auto_acquire_while_stealthed());
    assert_eq!(ai.get_next_mood_target_id(true, true), INVALID_ID);
    assert_eq!(ai.get_next_mood_check_time(), 1);
    contain.lock().unwrap().set_passenger_allowed_to_fire(true);
    assert!(contain.lock().unwrap().is_passenger_allowed_to_fire(None));
    assert_eq!(ai.get_next_mood_target_id(true, true), fixture.target_id);
    assert_eq!(ai.get_next_mood_check_time(), 77);
    assert_eq!(get_game_logic_random_seed_state(), before);
}

#[test]
fn native_auto_acquire_getters_ignore_locked_foreign_owner() {
    if !child(concat!(
        module_path!(),
        "::native_auto_acquire_getters_ignore_locked_foreign_owner"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = MoodFixture::new("Yes STEALTHED");
    // The source and AI are genuine factory objects. Only the unrelated
    // compatibility Unit is synthetic, with the same ID and opposite flags.
    let foreign_owner = Arc::new(RwLock::new(crate::object::Object::new_test(
        fixture.source_id,
        200.0,
    )));
    let foreign = Arc::new(RwLock::new(
        super::super::super::super::identity::Unit::new(
            Arc::clone(&foreign_owner),
            &crate::common::DefaultThingTemplate::new("MoodPolicyForeign".into()),
        )
        .unwrap(),
    ));
    {
        let mut unit = foreign.write().unwrap();
        unit.auto_acquire_enemies = false;
        unit.auto_acquire_while_stealthed = false;
    }
    super::super::super::super::registry::register_unit(fixture.source_id, &foreign);
    let _foreign_unit_guard = foreign.write().unwrap();
    let _foreign_owner_guard = foreign_owner.write().unwrap();
    let ai = fixture.ai.lock().unwrap();
    assert!(ai.can_auto_acquire());
    assert!(ai.can_auto_acquire_while_stealthed());
    drop(ai);
    drop(_foreign_owner_guard);
    drop(_foreign_unit_guard);
    super::super::super::super::registry::unregister_unit(fixture.source_id);
}
