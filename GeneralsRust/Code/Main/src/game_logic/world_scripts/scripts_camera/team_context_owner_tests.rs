//! Exact C++ Team* contexts through the actual Main script driver.
use super::named_command_test_support::{named_infantry, world, world_with};
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use generals_sim::scripting::program::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList, THIS_TEAM,
};
use gamelogic::scripting::engine::{ScriptEngine, ScriptExecutionDriver};
use generals_sim::scripting::world::ScriptOwnerQuery;

fn admit(world: &GameLogic, name: &str, member: ObjectId, owner: u32) -> u32 {
    let mut factory = world.team_factory.lock().unwrap();
    if factory.find_team_prototype(name).is_none() {
        factory
            .init_team(name.into(), "".into(), false, None)
            .unwrap();
    }
    let team = factory.create_inactive_team(name).unwrap();
    let mut team = team.write().unwrap();
    team.add_member(member.0);
    team.set_controlling_player_id(Some(owner));
    team.get_id()
}

fn guard(name: &str) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::TeamGuard);
    action
        .add_parameter(Parameter::with_string(ParameterType::Team, name.into()))
        .unwrap();
    action
}

fn force_select(team: &str, template: &str, center: bool, sound: &str) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::ObjectForceSelect);
    for parameter in [
        Parameter::with_string(ParameterType::Team, team.into()),
        Parameter::with_string(ParameterType::ObjectType, template.into()),
        Parameter::with_int(ParameterType::Int, i32::from(center)),
        Parameter::with_string(ParameterType::Sound, sound.into()),
    ] {
        action.add_parameter(parameter).unwrap();
    }
    action
}

#[test]
fn force_select_uses_exact_calling_team_and_selects_the_drawable() {
    let (mut owner, first, second) = world();
    let (peer, peer_first, _) = world();
    let older = admit(&owner, "SelectRoster", first, 1);
    admit(&owner, "SelectRoster", second, 2);
    assert_eq!(older, admit(&peer, "SelectRoster", peer_first, 1));
    assert_eq!(first, peer_first);
    owner.apply_player_drawable_selection(1, &[second], true);
    owner.host_object_mut(first).unwrap().status.unselectable = true;
    let engine = ScriptEngine::new_in_published_world().unwrap();
    engine
        .friend_execute_action_with_driver_checked(
            &force_select(THIS_TEAM, "OwnedNamedInfantry", false, ""),
            Some(older),
            gamelogic::scripting::executor::ScriptContext::at_frame(owner.frame),
            &mut HostScriptExecutionDriver::with_interpreter(&mut owner, &engine),
        )
        .unwrap();
    assert_eq!(
        owner.get_player(1).unwrap().selected_objects,
        [first],
        "force selection takes the calling Team's Drawable even when ordinary selection is disabled"
    );
    assert!(owner.host_object(first).unwrap().selected);
    assert!(!owner.host_object(second).unwrap().selected);
    assert!(!peer.host_object(peer_first).unwrap().selected);
}

#[test]
fn force_select_checks_the_lowest_matching_drawable_before_side_effects() {
    let (mut owner, first, previous) = world();
    let later = owner
        .create_object_for_player("OwnedNamedInfantry", 1, Vec3::splat(90.0))
        .unwrap();
    assert!(first < later);
    let team = owner.host_object(first).unwrap().team_instance_name.clone();
    let execute = |world: &mut GameLogic, template, center, audio| {
        world.with_own_services_mut(|world| {
            world.execute_owned_script_action_for_test(&force_select(&team, template, center, audio))
        });
    };
    execute(&mut owner, "OwnedNamedInfantry", false, "NoSound");
    assert_eq!(owner.get_player(1).unwrap().selected_objects, [first]);
    owner.apply_player_drawable_selection(1, &[previous], true);
    let before_audio = owner.queued_audio_event_count_for_test();
    let before_camera = owner.capture_camera_continuation().unwrap();

    execute(&mut owner, "ownednamedinfantry", true, "WrongCase");
    assert_eq!(
        owner.get_player(1).unwrap().selected_objects,
        [previous],
        "force selection compares the authored template name exactly"
    );
    owner.host_object_mut(first).unwrap().has_drawable = false;
    execute(&mut owner, "OwnedNamedInfantry", true, "MissingDrawable");
    assert_eq!(owner.get_player(1).unwrap().selected_objects, [previous]);
    assert!(!owner.host_object(later).unwrap().selected);
    assert_eq!(owner.queued_audio_event_count_for_test(), before_audio);
    assert_eq!(owner.capture_camera_continuation().unwrap(), before_camera);
}

#[test]
fn force_select_delivers_selection_audio_and_camera_during_the_action() {
    use crate::game_logic::{NativeAudioEffects, ScriptPresentationEffects, ScriptUiEffects};
    use game_engine::common::audio::{AudioEventRts, AudioSubmissionLocality};
    use std::cell::RefCell;

    struct Ui<'a>(&'a RefCell<Vec<&'static str>>);
    impl ScriptUiEffects for Ui<'_> {
        fn weather_visible(&mut self, _: bool) {}
        fn letterbox(
            &mut self,
            _: &mut crate::presentation_frame::camera::SessionCamera,
            _: bool,
            _: bool,
            _: game_engine::common::thing::receiving_module_content::UpgradeTooltipSource<'_>,
            _: bool,
        ) {}
        fn mark_control_bar_dirty(&mut self) {
            self.0.borrow_mut().push("selection");
        }
    }
    struct Audio<'a>(&'a RefCell<Vec<&'static str>>);
    impl NativeAudioEffects for Audio<'_> {
        fn add_audio_event(&mut self, event: &AudioEventRts, _: &AudioSubmissionLocality) {
            assert_eq!(event.get_event_name(), "SelectScriptUnit");
            assert_eq!(event.get_player_index(), 1);
            self.0.borrow_mut().push("audio");
        }
    }

    let (mut owner, first, _) = world();
    let team = owner.host_object(first).unwrap().team_instance_name.clone();
    let position = owner.host_object(first).unwrap().get_position();
    let events = RefCell::new(Vec::new());
    let mut ui = Ui(&events);
    let mut audio = Audio(&events);
    let engine = ScriptEngine::new_in_published_world().unwrap();
    let context = gamelogic::scripting::executor::ScriptContext::at_frame(owner.frame);
    let mut observe = |world: &GameLogic| {
        assert_eq!(world.get_player(1).unwrap().selected_objects, [first]);
        assert_eq!(world.session_view.scripted.motion.points[2].world(), position);
        assert_eq!(*events.borrow(), ["selection", "audio"]);
        events.borrow_mut().push("after action");
    };
    let effects = ScriptPresentationEffects {
        ui: Some(&mut ui),
        audio: Some(&mut audio),
        ..Default::default()
    };
    engine
        .friend_execute_action_with_driver_checked(
            &force_select(&team, "OwnedNamedInfantry", true, "SelectScriptUnit"),
            None,
            context,
            &mut HostScriptExecutionDriver::with_effects(&mut owner, &engine, effects)
                .with_after_action_observer(&mut observe),
        )
        .unwrap();
    assert_eq!(*events.borrow(), ["selection", "audio", "after action"]);
    assert_eq!(owner.queued_audio_event_count_for_test(), 0);
}

#[test]
fn team_attitude_moves_members_into_a_new_group_before_writing_mood() {
    let (mut owner, first, _) = world();
    let (peer, peer_first, _) = world();
    let older = admit(&owner, "AttitudeGroupRoster", first, 1);
    let group = owner.create_team_ai_group(&[first.0]);
    let engine = ScriptEngine::new_in_published_world().unwrap();

    attitude(&engine, &mut owner, Some(older), THIS_TEAM, 3);

    let current = owner.ai_group_id_of(first).unwrap();
    assert_ne!(current, group, "Team attitude admits its members to a new AIGroup");
    assert_eq!(owner.ai_group_members(current), [first]);
    assert_eq!(owner.host_object(first).unwrap().ai_attitude, 3);
    assert_eq!(first, peer_first);
    assert!(peer.ai_group_id_of(peer_first).is_none());
    assert_eq!(peer.host_object(peer_first).unwrap().ai_attitude, 0);
}

#[test]
fn team_stopping_distance_stops_in_roster_order() {
    use crate::fixture_content::FixtureObject;

    let (mut owner, first, _) = world_with(
        &[FixtureObject::new("StopDistanceStructure")
            .kind_of("STRUCTURE IMMOBILE")
            .health(10.0)],
        &[],
        "",
    );
    let blocker = owner
        .create_object_for_player("StopDistanceStructure", 1, Vec3::splat(80.0))
        .unwrap();
    let newest = owner
        .create_object_for_player("OwnedNamedInfantry", 1, Vec3::splat(95.0))
        .unwrap();
    let team = owner.host_object(first).unwrap().team_instance_name.clone();
    let (_, members) = owner.resolve_owned_script_team(&team, None, None).unwrap().unwrap();
    assert_eq!(members, [newest.0, blocker.0, first.0]);
    owner.host_object_mut(first).unwrap().close_enough_dist = Some(6.0);
    owner.host_object_mut(newest).unwrap().close_enough_dist = Some(8.0);

    let mut action = ScriptAction::new(ScriptActionType::SetStoppingDistance);
    action.add_parameter(Parameter::with_string(ParameterType::Team, team)).unwrap();
    action.add_parameter(Parameter::with_real(ParameterType::Real, 25.0)).unwrap();
    owner.execute_owned_script_action_for_test(&action);

    assert_eq!(
        owner.host_object(newest).unwrap().close_enough_dist,
        Some(25.0),
        "the newest member precedes the structure in the Team roster"
    );
    assert_eq!(owner.host_object(first).unwrap().close_enough_dist, Some(6.0));
}

#[test]
fn team_flash_zero_clears_a_prior_named_flash() {
    let (mut owner, id, _) = world();
    let (peer, peer_id, _) = world();
    assert_eq!(id, peer_id);
    let team = owner.host_object(id).unwrap().team_instance_name.clone();

    for (opcode, kind, name, seconds) in [
        (ScriptActionType::NamedFlash, ParameterType::Unit, "NamedUnit", 2),
        (ScriptActionType::NamedFlash, ParameterType::Unit, "NamedUnit", 0),
        (ScriptActionType::TeamFlashWhite, ParameterType::Team, team.as_str(), 0),
    ] {
        let mut action = ScriptAction::new(opcode);
        action.add_parameter(Parameter::with_string(kind, name.into())).unwrap();
        action.add_parameter(Parameter::with_int(ParameterType::Int, seconds)).unwrap();
        owner.execute_owned_script_action_for_test(&action);
        if opcode == ScriptActionType::NamedFlash {
            assert_eq!(owner.host_object(id).unwrap().flash_count, 4);
        }
    }
    let object = owner.host_object(id).unwrap();
    assert_eq!(object.flash_count, 0, "zero duration Team flash clears the count");
    assert_eq!(object.flash_color, 0x00FF_FFFF);
    assert_eq!(peer.host_object(peer_id).unwrap().flash_count, 0);
}

#[test]
fn team_emoticon_moves_eligible_members_into_a_new_group() {
    use crate::fixture_content::FixtureObject;

    let (mut owner, id, _) = world_with(
        &[FixtureObject::new("EmoticonWithoutAi")
            .kind_of("INERT")
            .health(10.0)],
        &[],
        "",
    );
    let ineligible = owner
        .create_object_for_player("EmoticonWithoutAi", 1, Vec3::splat(90.0))
        .unwrap();
    let object = owner.host_object(ineligible).unwrap();
    assert!(!object.has_ai_update_interface());
    assert!(!object.is_kind_of(KindOf::Structure));
    assert!(!object.is_kind_of(KindOf::AlwaysSelectable));
    let old_group = owner.create_team_ai_group(&[id.0]);
    assert_eq!(owner.ai_group_id_of(id), Some(old_group));
    let team = owner.host_object(id).unwrap().team_instance_name.clone();
    let mut action = ScriptAction::new(ScriptActionType::TeamSetEmoticon);
    action.add_parameter(Parameter::with_string(ParameterType::Team, team)).unwrap();
    action.add_parameter(Parameter::with_string(ParameterType::Emoticon, "EmoticonAlert".into())).unwrap();
    action.add_parameter(Parameter::with_real(ParameterType::Real, 1.5)).unwrap();
    owner.execute_owned_script_action_for_test(&action);

    let new_group = owner.ai_group_id_of(id).expect("admitted into the Team emoticon group");
    assert_ne!(new_group, old_group, "Team emoticon uses a new AIGroup");
    assert_eq!(owner.ai_group_members(new_group), [id]);
    assert_eq!(owner.host_object(id).unwrap().emoticon_frames_left, 45);
    assert!(owner.host_object(ineligible).unwrap().emoticon_name.is_empty());
}

#[test]
fn scripted_boobytrap_uses_the_calling_team_before_the_next_action() {
    use crate::fixture_content::FixtureObject;

    let charge = FixtureObject::new("ContextStickyCharge")
        .kind_of("IMMOBILE BOOBY_TRAP")
        .health(1.0)
        .module("Behavior = StickyBombUpdate Sticky\nEnd");
    let (mut owner, first, second) = world_with(std::slice::from_ref(&charge), &[], "");
    let (peer, peer_first, peer_second) = world_with(&[charge], &[], "");
    assert_eq!((first, second), (peer_first, peer_second));
    let team = admit(&owner, "TrapRoster", first, 1);
    admit(&owner, "TrapRoster", second, 2);
    assert_eq!(team, admit(&peer, "TrapRoster", peer_first, 1));
    admit(&peer, "TrapRoster", peer_second, 2);

    let engine = ScriptEngine::new_in_published_world().unwrap();
    for team_name in [THIS_TEAM, "TrapRoster"] {
        let mut action = ScriptAction::new(ScriptActionType::TeamSetBoobytrapped);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::ObjectType,
                "ContextStickyCharge".into(),
            ))
            .unwrap();
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                team_name.into(),
            ))
            .unwrap();
        engine
            .friend_execute_action_with_driver_checked(
                &action,
                Some(team),
                gamelogic::scripting::executor::ScriptContext::at_frame(owner.frame),
                &mut HostScriptExecutionDriver::new(&mut owner),
            )
            .unwrap();
        assert!(
            owner
                .host_object(first)
                .unwrap()
                .has_object_status_bit("BOOBY_TRAPPED"),
            "the calling Team member must be trapped after {team_name}"
        );
        assert!(
            !owner
                .host_object(second)
                .unwrap()
                .has_object_status_bit("BOOBY_TRAPPED")
        );
        for id in [first, second] {
            assert!(
                !peer
                    .host_object(id)
                    .unwrap()
                    .has_object_status_bit("BOOBY_TRAPPED")
            );
        }
    }
    let charges: Vec<_> = owner
        .host_objects()
        .registered_values()
        .filter(|object| object.template_name == "ContextStickyCharge")
        .collect();
    assert_eq!(charges.len(), 2);
    for charge in charges {
        assert_eq!(charge.booby_trap_attached_to, Some(first));
        assert_eq!(charge.owner_player_id, Some(1));
    }
}

fn increment(name: &str) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::IncrementCounter);
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(ParameterType::Counter, name.into()))
        .unwrap();
    action
}

fn update(engine: &ScriptEngine, world: &mut GameLogic) {
    engine
        .update_with_driver(
            gamelogic::scripting::executor::ScriptContext::at_frame(world.frame),
            &mut HostScriptExecutionDriver::new(world),
        )
        .unwrap();
}

fn sequence_engine() -> ScriptEngine {
    let mut body = guard(THIS_TEAM);
    body.next_action = Some(Box::new(increment("AfterGuard")));
    let mut script = Script::new();
    script.script_name = "ExactSequence".into();
    script.is_subroutine = true;
    script.action = Some(Box::new(body));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new_in_published_world().unwrap();
    engine.set_counter("AfterGuard", 0).unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}

fn start(engine: &ScriptEngine, world: &mut GameLogic, id: u32) {
    let mut action = ScriptAction::new(ScriptActionType::TeamExecuteSequentialScriptLooping);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            THIS_TEAM.into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Script,
            "ExactSequence".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 3))
        .unwrap();
    engine.friend_execute_action_with_driver(
        &action,
        Some(id),
        gamelogic::scripting::executor::ScriptContext::at_frame(world.frame),
        &mut HostScriptExecutionDriver::new(world),
    );
}

#[test]
fn literal_context_selects_older_same_name_instance_and_missing_id_cannot_retarget() {
    let (mut world, first, second) = world();
    let older = admit(&world, "SiblingRoster", first, 1);
    let newer = admit(&world, "SiblingRoster", second, 2);
    let engine = ScriptEngine::new_in_published_world().unwrap();
    engine.friend_execute_action_with_driver(
        &guard("SiblingRoster"),
        Some(older),
        gamelogic::scripting::executor::ScriptContext::at_frame(0),
        &mut HostScriptExecutionDriver::new(&mut world),
    );
    assert_eq!(
        world.host_object(first).unwrap().ai_state(),
        AIState::GuardingArea
    );
    assert_eq!(
        world.host_object(second).unwrap().active_guard_position(),
        None
    );
    engine.friend_execute_action_with_driver(
        &guard(THIS_TEAM),
        Some(u32::MAX),
        gamelogic::scripting::executor::ScriptContext::at_frame(0),
        &mut HostScriptExecutionDriver::new(&mut world),
    );
    assert_eq!(
        world.host_object(second).unwrap().active_guard_position(),
        None
    );
    let driver = HostScriptExecutionDriver::new(&mut world);
    assert_eq!(
        driver.team_instances("SiblingRoster"),
        ScriptOwnerQuery::Present(vec![newer, older])
    );
    assert_eq!(driver.team_name(u32::MAX), ScriptOwnerQuery::Missing);
    assert_eq!(
        driver.team_current_player(older),
        ScriptOwnerQuery::Present(Some("NamedOwner".into()))
    );
    assert_eq!(
        driver.team_current_player(newer),
        ScriptOwnerQuery::Present(Some("NamedVictim".into()))
    );
}

#[test]
fn sequential_start_keeps_same_name_chains_distinct_and_stops_only_selected_context() {
    let (mut world, first, second) = world();
    let first_id = admit(&world, "SiblingRoster", first, 1);
    let second_id = admit(&world, "SiblingRoster", second, 2);
    let frame = world.frame;
    world.host_object_mut(first).unwrap().set_ai_state(
        AIState::Moving,
        frame,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    world.host_object_mut(second).unwrap().set_ai_state(
        AIState::Moving,
        frame,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    let engine = sequence_engine();
    start(&engine, &mut world, first_id);
    assert_eq!(world.host_object(first).unwrap().ai_state(), AIState::Idle);
    assert_eq!(
        world.host_object(second).unwrap().ai_state(),
        AIState::Moving
    );
    assert_eq!(
        world.host_object(first).unwrap().last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(world.host_object(first).unwrap().formation_id, 19);
    assert_eq!(
        world
            .host_object(first)
            .unwrap()
            .cur_locomotor_name
            .as_deref(),
        Some("RedguardLocomotor")
    );
    start(&engine, &mut world, second_id);
    let before = engine.snapshot_sequential_scripts();
    assert_eq!(
        before.iter().map(|s| s.team_id).collect::<Vec<_>>(),
        vec![first_id, second_id]
    );
    assert!(before.iter().all(|s| s.times_to_loop == 2));
    update(&engine, &mut world);
    assert_eq!(
        world.host_object(first).unwrap().ai_state(),
        AIState::GuardingArea
    );
    assert_eq!(
        world.host_object(second).unwrap().ai_state(),
        AIState::GuardingArea
    );
    assert_eq!(engine.get_counter("AfterGuard").unwrap().value, 0);
    let mut stop = ScriptAction::new(ScriptActionType::TeamStopSequentialScript);
    stop.add_parameter(Parameter::with_string(
        ParameterType::Team,
        THIS_TEAM.into(),
    ))
    .unwrap();
    engine.friend_execute_action_with_driver(
        &stop,
        Some(first_id),
        gamelogic::scripting::executor::ScriptContext::at_frame(0),
        &mut HostScriptExecutionDriver::new(&mut world),
    );
    assert!(!engine.has_active_sequential_script_for_team(first_id));
    assert!(engine.has_active_sequential_script_for_team(second_id));
    assert_eq!(engine.get_calling_team_id(), None);
}

#[test]
fn same_ids_in_two_worlds_keep_context_sequence_effects_and_reset_independent() {
    let (mut first_world, first, _) = world();
    let (mut second_world, second, _) = world();
    let first_id = admit(&first_world, "SiblingRoster", first, 1);
    let second_id = admit(&second_world, "SiblingRoster", second, 2);
    assert_eq!((first_id, first), (second_id, second));
    second_world.set_fixture_object_position(second, Vec3::splat(500.0));
    let first_engine = sequence_engine();
    let second_engine = sequence_engine();
    start(&first_engine, &mut first_world, first_id);
    start(&second_engine, &mut second_world, second_id);
    update(&first_engine, &mut first_world);
    assert_eq!(
        second_world
            .host_object(second)
            .unwrap()
            .active_guard_position(),
        None
    );
    update(&second_engine, &mut second_world);
    assert_eq!(
        first_world
            .host_object(first)
            .unwrap()
            .active_guard_position(),
        Some(Vec3::new(50.0, 0.0, 50.0))
    );
    assert_eq!(
        second_world
            .host_object(second)
            .unwrap()
            .active_guard_position(),
        Some(Vec3::splat(500.0))
    );
    let retained = first_engine.snapshot_sequential_scripts();
    second_world.reset();
    assert_eq!(first_engine.snapshot_sequential_scripts(), retained);
    assert_eq!(
        first_world.host_object(first).unwrap().ai_state(),
        AIState::GuardingArea
    );
}

#[test]
fn exact_sequence_snapshot_restore_preserves_selected_sibling_identity() {
    let (mut source, first, second) = world();
    let older = admit(&source, "SiblingRoster", first, 1);
    let newer = admit(&source, "SiblingRoster", second, 2);
    let engine = sequence_engine();
    start(&engine, &mut source, older);
    let saved = engine.snapshot_sequential_scripts();
    assert_eq!(saved[0].team_id, older);
    assert_ne!(saved[0].team_id, newer);
    let (mut restored, first, second) = world();
    assert_eq!(admit(&restored, "SiblingRoster", first, 1), older);
    assert_eq!(admit(&restored, "SiblingRoster", second, 2), newer);
    let restored_engine = sequence_engine();
    restored_engine.restore_sequential_scripts(&saved);
    update(&restored_engine, &mut restored);
    assert_eq!(
        restored.host_object(first).unwrap().ai_state(),
        AIState::GuardingArea
    );
    assert_eq!(
        restored
            .host_object(second)
            .unwrap()
            .active_guard_position(),
        None
    );
}

#[test]
fn native_save_resumes_a_waiting_sequence_on_the_same_frame() {
    use crate::save_load::SnapshotBuilder;
    use generals_sim::scripting::sequential_scripts::SequentialScript;

    fn install(world: &mut GameLogic) {
        let mut script = Script::new();
        script.script_name = "SavedWait".into();
        script.is_subroutine = true;
        script.action = Some(Box::new(increment("SavedWaitCount")));
        let mut list = ScriptList::new();
        list.append_script(Box::new(script));
        world
            .with_owned_script_engine(|engine, _| {
                engine.set_counter("SavedWaitCount", 0).unwrap();
                engine
                    .set_script_list_for_player(0, Some(Box::new(list)))
                    .unwrap();
            })
            .unwrap();
    }

    fn tick(world: &mut GameLogic, frame: u32) -> i32 {
        world.frame = frame;
        world
            .with_owned_script_engine(|engine, world| {
                world.update_local_script_engine_with_owner(engine).unwrap();
                engine.get_counter("SavedWaitCount").unwrap().value
            })
            .unwrap()
    }

    let (mut source, object, _) = world();
    install(&mut source);
    source
        .with_owned_script_engine(|engine, _| {
            let mut sequence = SequentialScript::new();
            sequence.object_id = object.0;
            sequence.frames_to_wait = 3;
            sequence.script_to_execute_sequentially =
                engine.find_script_clone_by_name("SavedWait").map(Box::new);
            engine.append_sequential_script(sequence);
        })
        .unwrap();
    assert_eq!(tick(&mut source, 100), 0);
    let builder = SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&source).unwrap();
    assert_eq!(saved.persist_v18.script_sequential[0].frames_to_wait, 2);

    let (mut receiver, receiving_object, _) = world();
    assert_eq!(object, receiving_object);
    install(&mut receiver);
    builder
        .restore_from_snapshot(&saved, &mut receiver)
        .unwrap();
    for (frame, expected) in [(101, 0), (102, 0), (103, 1), (104, 1)] {
        assert_eq!(tick(&mut receiver, frame), expected);
        assert_eq!(tick(&mut source, frame), expected);
    }
}

#[test]
fn retained_missing_team_id_rejects_before_persist_tail_mutates_receiver() {
    use crate::save_load::snapshot::persist_v18::{
        SequentialScriptPersist, WorldPersistV18, restore_persist_v18,
    };
    let (mut world, id, _) = world();
    let before = world.host_object(id).unwrap().ai_state().clone();
    let mut persist = WorldPersistV18::default();
    persist.script_sequential.push(SequentialScriptPersist {
        team_id: u32::MAX,
        object_id: 0,
        script_name: "MissingTeamSequence".into(),
        current_instruction: -1,
        times_to_loop: 0,
        frames_to_wait: 0,
        dont_advance_instruction: false,
    });
    let error = restore_persist_v18(&persist, &mut world).unwrap_err();
    assert!(error.to_string().contains("missing team"));
    assert_eq!(world.host_object(id).unwrap().ai_state(), before);
}

#[test]
fn sequence_idle_contains_cycle_is_bounded_and_still_visits_each_live_member() {
    // privateIdle recurses through an actual Contain interface. A projected
    // occupants list alone (for example parked aircraft) is not that interface.
    let (mut world, _, _) = world_with(
        &[named_infantry("OwnedCycleCarrier")
            .module("Behavior = TransportContain ModuleTag_Contain\n Slots = 2\nEnd")],
        &[],
        "",
    );
    let first = world
        .create_object_for_player("OwnedCycleCarrier", 1, Vec3::ZERO)
        .unwrap();
    let second = world
        .create_object_for_player("OwnedCycleCarrier", 2, Vec3::ZERO)
        .unwrap();
    let id = admit(&world, "CycleRoster", first, 1);
    world.host_object_mut(first).unwrap().occupants = vec![second];
    world.host_object_mut(second).unwrap().occupants = vec![first, ObjectId(u32::MAX)];
    let frame = world.frame;
    world.host_object_mut(first).unwrap().set_ai_state(
        AIState::Moving,
        frame,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    world.host_object_mut(second).unwrap().set_ai_state(
        AIState::Moving,
        frame,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    let engine = sequence_engine();
    start(&engine, &mut world, id);
    for member in [first, second] {
        let unit = world.host_object(member).unwrap();
        assert_eq!(unit.ai_state(), AIState::Idle);
        assert_eq!(
            unit.last_command_source,
            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
        );
    }
    assert!(engine.has_active_sequential_script_for_team(id));
}

fn attitude(engine: &ScriptEngine, world: &mut GameLogic, id: Option<u32>, raw: &str, mood: i32) {
    let mut action = ScriptAction::new(ScriptActionType::TeamSetAttitude);
    action
        .add_parameter(Parameter::with_string(ParameterType::Team, raw.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, mood))
        .unwrap();
    engine.friend_execute_action_with_driver(
        &action,
        id,
        gamelogic::scripting::executor::ScriptContext::at_frame(world.frame),
        &mut HostScriptExecutionDriver::new(world),
    );
}

#[test]
fn team_attitude_same_ids_in_two_worlds_select_only_exact_older_context() {
    let (mut first_world, older_member, newer_member) = world();
    let (mut foreign, foreign_member, _) = world();
    let older = admit(&first_world, "SiblingRoster", older_member, 1);
    let newer = admit(&first_world, "SiblingRoster", newer_member, 1);
    let foreign_id = admit(&foreign, "ForeignRoster", foreign_member, 1);
    assert_eq!(older, foreign_id);
    assert_ne!(older, newer);
    for id in [older_member, newer_member] {
        first_world.host_object_mut(id).unwrap().team_instance_name = "ForeignRoster".into();
    }
    let engine = ScriptEngine::new_in_published_world().unwrap();
    attitude(&engine, &mut first_world, Some(older), THIS_TEAM, -2);
    assert_eq!(
        first_world.host_object(older_member).unwrap().ai_attitude,
        -2
    );
    assert_eq!(
        first_world.host_object(newer_member).unwrap().ai_attitude,
        0
    );
    assert_eq!(foreign.host_object(foreign_member).unwrap().ai_attitude, 0);
    // Literal equality selects the same exact contextual instance, not head.
    attitude(&engine, &mut first_world, Some(older), "SiblingRoster", 2);
    assert_eq!(
        first_world.host_object(older_member).unwrap().ai_attitude,
        2
    );
    assert_eq!(
        first_world.host_object(newer_member).unwrap().ai_attitude,
        0
    );
    attitude(&engine, &mut foreign, Some(foreign_id), THIS_TEAM, -1);
    assert_eq!(foreign.host_object(foreign_member).unwrap().ai_attitude, -1);
    assert_eq!(
        first_world.host_object(older_member).unwrap().ai_attitude,
        2
    );
}

#[test]
fn team_attitude_modifies_dead_immobile_ai_but_skips_non_ai_and_missing_context() {
    let (mut world, first, second) = world();
    let id = admit(&world, "AttitudeRoster", first, 1);
    world
        .team_factory
        .lock()
        .unwrap()
        .find_team_by_id(id)
        .unwrap()
        .write()
        .unwrap()
        .add_member(second.0);
    world
        .host_object_mut(first)
        .unwrap()
        .status
        .effectively_dead = true;
    world
        .host_object_mut(first)
        .unwrap()
        .template_mut()
        .add_kind_of(KindOf::Immobile);
    world
        .host_object_mut(second)
        .unwrap()
        .template_mut()
        .set_authored_ai_update_interface(Some(false));
    let engine = ScriptEngine::new_in_published_world().unwrap();
    attitude(&engine, &mut world, Some(id), THIS_TEAM, 3);
    assert_eq!(world.host_object(first).unwrap().ai_attitude, 3);
    assert_eq!(world.host_object(second).unwrap().ai_attitude, 0);
    attitude(&engine, &mut world, Some(u32::MAX), THIS_TEAM, -2);
    assert_eq!(world.host_object(first).unwrap().ai_attitude, 3);
}

#[test]
fn sequence_script_idle_admits_immobile_ai_without_locomotor_preparation() {
    let (mut world, first, _) = world();
    let id = admit(&world, "IdleRoster", first, 1);
    let frame = world.frame;
    let unit = world.host_object_mut(first).unwrap();
    unit.template_mut().add_kind_of(KindOf::Immobile);
    unit.set_ai_state(
        AIState::Moving,
        frame,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    let engine = sequence_engine();
    start(&engine, &mut world, id);
    let unit = world.host_object(first).unwrap();
    assert_eq!(unit.ai_state(), AIState::Idle);
    assert_eq!(
        unit.last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(
        unit.cur_locomotor_name.as_deref(),
        Some("RedguardLocomotor")
    );
    assert_eq!(unit.formation_id, 19);
}

#[test]
fn sequence_script_idle_rejects_dead_sleep_surrender_and_projectile_members() {
    let rejects: [fn(&mut GameLogic, ObjectId); 4] = [
        |world, id| world.host_object_mut(id).unwrap().status.effectively_dead = true,
        |world, id| {
            world.get_player_mut(1).unwrap().is_human = false;
            world.host_object_mut(id).unwrap().set_ai_attitude_i8(-2);
        },
        |world, id| world.host_object_mut(id).unwrap().is_surrendered = true,
        |world, id| {
            world
                .host_object_mut(id)
                .unwrap()
                .template_mut()
                .add_kind_of(KindOf::Projectile);
        },
    ];
    for reject in rejects {
        let (mut world, rejected, accepted) = world();
        let team_id = admit(&world, "IdleRoster", rejected, 1);
        world
            .team_factory
            .lock()
            .unwrap()
            .find_team_by_id(team_id)
            .unwrap()
            .write()
            .unwrap()
            .add_member(accepted.0);
        let frame = world.frame;
        for id in [rejected, accepted] {
            world.host_object_mut(id).unwrap().set_ai_state(
                AIState::Moving,
                frame,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
        }
        reject(&mut world, rejected);
        let before = world.host_object(rejected).unwrap().last_command_source;
        let engine = sequence_engine();
        start(&engine, &mut world, team_id);
        assert_eq!(
            world.host_object(rejected).unwrap().ai_state(),
            AIState::Moving
        );
        assert_eq!(
            world.host_object(rejected).unwrap().last_command_source,
            before
        );
        assert_eq!(
            world.host_object(accepted).unwrap().ai_state(),
            AIState::Idle
        );
        assert_eq!(
            world.host_object(accepted).unwrap().last_command_source,
            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
        );
    }
}
