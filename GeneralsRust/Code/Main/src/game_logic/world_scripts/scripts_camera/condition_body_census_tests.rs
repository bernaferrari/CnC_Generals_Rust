//! Actual borrowed script walks for InitialHealth and player object census.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;

impl GameLogic {
    pub(crate) fn owned_script_condition_for_test(&mut self, condition: Condition) -> bool {
        let mut branch = OrCondition::new();
        branch.set_first_and_condition(Some(Box::new(condition)));
        let mut action = ScriptAction::new(ScriptActionType::IncrementCounter);
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, 1))
            .unwrap();
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Counter,
                "OwnedConditionObserved".into(),
            ))
            .unwrap();
        let mut script = Script::new();
        script.script_name = "OwnedBodyCensusCondition".into();
        script.is_one_shot = false;
        script.condition = Some(Box::new(branch));
        script.action = Some(Box::new(action));
        let mut list = ScriptList::new();
        list.append_script(Box::new(script));
        let mut engine = ScriptEngine::new_in_published_world().unwrap();
        engine.set_counter("OwnedConditionObserved", 0).unwrap();
        engine
            .set_script_list_for_player(0, Some(Box::new(list)))
            .unwrap();
        engine
            .update_with_driver(
                gamelogic::scripting::executor::ScriptContext::at_frame(self.frame),
                &mut HostScriptExecutionDriver::new(self),
            )
            .unwrap();
        engine.get_counter("OwnedConditionObserved").unwrap().value == 1
    }
}

fn health(name: &str, threshold: i32) -> Condition {
    let mut condition = Condition::new(ConditionType::UnitHealth);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Unit, name.into()))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 3))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Int, threshold))
        .unwrap();
    condition
}

fn destroyed(name: &str) -> Condition {
    let mut condition = Condition::new(ConditionType::PlayerAllDestroyed);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Side, name.into()))
        .unwrap();
    condition
}

fn world(current: f32) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(
        Player::new(0, Team::USA, "OwnedBodyPlayer", true).with_script_identity("OwnedBodyPlayer"),
    );
    let mut template = ThingTemplate::new("OwnedBodyUnit");
    template.set_health(100.0).add_kind_of(KindOf::Infantry);
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object_for_player("OwnedBodyUnit", 0, Vec3::ZERO)
        .unwrap();
    let object = world.host_object_mut(id).unwrap();
    object.name = "OwnedBodyName".into();
    object.initial_health = 80.0;
    object.health.current = current;
    world.initialize_receiving_named_cache_for_fixture();
    (world, id)
}

#[test]
fn owned_condition_health_and_census_ignore_foreign_same_ids() {
    let (mut source, id) = world(80.0);
    let (mut foreign, foreign_id) = world(1.0);
    assert_eq!(id, foreign_id);
    foreign
        .host_object_mut(foreign_id)
        .unwrap()
        .status
        .effectively_dead = true;
    assert!(source.owned_script_condition_for_test(health("OwnedBodyName", 100)));
    assert!(!foreign.owned_script_condition_for_test(health("OwnedBodyName", 100)));
    assert!(!source.owned_script_condition_for_test(destroyed("OwnedBodyPlayer")));
    assert!(foreign.owned_script_condition_for_test(destroyed("OwnedBodyPlayer")));
    source.host_object_mut(id).unwrap().health.current = 79.0;
    assert!(!source.owned_script_condition_for_test(health("OwnedBodyName", 100)));
    assert!(!source.owned_script_condition_for_test(health("AbsentOwnedUnit", 1)));
    assert!(source.owned_script_condition_for_test(destroyed("AbsentOwnedPlayer")));
}

#[test]
fn scripted_music_completion_uses_receiving_device_receipts_and_reset() {
    use crate::cnc_game_engine::native_audio_effects::NativeAudioService;
    use crate::subsystem_manager::{AudioManagerSubsystem, SubsystemManager};
    use game_engine::common::audio::{AudioEventInfo, AudioEventRts, AudioManager, AudioType};
    use game_engine::common::audio::game_audio::SoundPlaybackHook;
    use std::sync::{Arc, Mutex};

    struct Playback;
    impl SoundPlaybackHook for Playback {
        fn play(&self, _: &AudioEventRts) -> Result<(), String> { Ok(()) }
        fn stop(&self, _: u32) {}
        fn pause(&self, _: u32) {}
        fn resume(&self, _: u32) {}
        fn is_playing(&self, _: u32) -> bool { true }
        fn music_loop_count(&self, handle: u32) -> i32 {
            match handle { 1001 => 1, 1002 => 4, _ => 0 }
        }
    }
    fn evaluate(world: &mut GameLogic, owner: &mut SubsystemManager, times: i32) -> bool {
        let mut condition = Condition::new(ConditionType::MusicTrackHasCompleted);
        condition.add_parameter(Parameter::with_string(ParameterType::Music, "SharedMusic".into())).unwrap();
        condition.add_parameter(Parameter::with_int(ParameterType::Int, times)).unwrap();
        let mut branch = OrCondition::new();
        branch.set_first_and_condition(Some(Box::new(condition)));
        let mut action = ScriptAction::new(ScriptActionType::IncrementCounter);
        action.add_parameter(Parameter::with_int(ParameterType::Int, 1)).unwrap();
        action.add_parameter(Parameter::with_string(ParameterType::Counter, "MusicObserved".into())).unwrap();
        let mut script = Script::new();
        script.script_name = "MusicCompletion".into();
        script.condition = Some(Box::new(branch));
        script.action = Some(Box::new(action));
        let mut list = ScriptList::new();
        list.append_script(Box::new(script));
        let mut engine = ScriptEngine::new_in_published_world().unwrap();
        engine.set_counter("MusicObserved", 0).unwrap();
        engine.set_script_list_for_player(0, Some(Box::new(list))).unwrap();
        let mut audio = NativeAudioService::receiving(owner).unwrap();
        let effects = crate::game_logic::ScriptPresentationEffects {
            audio: Some(&mut audio),
            ..Default::default()
        };
        engine.update_with_driver(
            gamelogic::scripting::executor::ScriptContext::at_frame(world.frame),
            &mut HostScriptExecutionDriver::with_effects(world, &engine, effects),
        ).unwrap();
        engine.get_counter("MusicObserved").unwrap().value == 1
    }

    let device = Arc::new(Mutex::new(AudioManager::with_playback_hook(Box::new(Playback))));
    let mut owners = [SubsystemManager::new(), SubsystemManager::new()];
    for (index, owner) in owners.iter_mut().enumerate() {
        let handle = 1001 + index as u32;
        let mut event = AudioEventRts::with_event_name("SharedMusic");
        event.set_playing_handle(handle);
        event.set_audio_event_info(Arc::new(AudioEventInfo {
            audio_name: "SharedMusic".into(),
            sound_type: AudioType::Music,
            ..Default::default()
        }));
        device.lock().unwrap().test_insert_active_audio_event(event);
        let mut subsystem = AudioManagerSubsystem::new();
        subsystem.bind_common_audio_device(Arc::clone(&device));
        subsystem.receiving_audio_parts().unwrap().1.push(handle);
        owner.add_subsystem(subsystem);
    }
    let mut worlds = [GameLogic::new(), GameLogic::new()];
    assert!(!evaluate(&mut worlds[0], &mut owners[0], 2));
    assert!(evaluate(&mut worlds[0], &mut owners[0], 1));
    assert!(evaluate(&mut worlds[1], &mut owners[1], 4));
    assert!(!evaluate(&mut worlds[1], &mut owners[1], 5));
    owners[0].reset_all().unwrap();
    assert!(!evaluate(&mut worlds[0], &mut owners[0], 1));
    assert!(evaluate(&mut worlds[1], &mut owners[1], 4));
    owners[1].reset_all().unwrap();
    assert!(!evaluate(&mut worlds[1], &mut owners[1], 1));
}
