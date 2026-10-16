//! Authored world effects must follow the receiver, not a retained adapter.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};

#[test]
fn authored_freeze_speed_weather_follow_receiver_without_or_with_foreign_handler() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "authored_freeze_speed_weather_follow_receiver_without_or_with_foreign_handler",
        || {
            for foreign_handler in [false, true] {
                let mut owner = GameLogic::new();
                let mut peer = GameLogic::new();
                let mut engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
                if foreign_handler {
                    engine.set_action_handler(Some(Arc::new(
                        crate::game_logic::mission_scripts::MissionScriptActionHandler::new(
                            Arc::clone(&peer.mission_scripts),
                        ),
                    )));
                }
                // Observe each action after the actual executor's after_action,
                // including repeated state changes before save/restore.
                for (frozen, speed, visible) in
                    [(true, 3, false), (false, 2, true), (true, 4, false)]
                {
                    let freeze = ScriptAction::new(if frozen {
                        ScriptActionType::FreezeTime
                    } else {
                        ScriptActionType::UnfreezeTime
                    });
                    let mut speed_action =
                        ScriptAction::new(ScriptActionType::SetVisualSpeedMultiplier);
                    speed_action
                        .add_parameter(Parameter::with_int(ParameterType::Int, speed))
                        .unwrap();
                    let mut weather = ScriptAction::new(ScriptActionType::ShowWeather);
                    weather
                        .add_parameter(Parameter::with_int(
                            ParameterType::Boolean,
                            i32::from(visible),
                        ))
                        .unwrap();
                    for action in [freeze, speed_action, weather] {
                        engine.friend_execute_action_with_driver(
                            &action,
                            None,
                            gamelogic::scripting::executor::ScriptContext::at_frame(owner.frame),
                            &mut HostScriptExecutionDriver::with_interpreter(&mut owner, &engine),
                        );
                    }
                    assert_eq!(owner.is_script_time_frozen(), frozen);
                    assert_eq!(owner.visual_speed_multiplier(), speed as f32);
                    assert_eq!(owner.weather_state().visible, visible);
                    assert!(!peer.is_script_time_frozen());
                    assert_eq!(peer.visual_speed_multiplier(), 1.0);
                    assert!(
                        peer.mission_scripts
                            .drain_script_freeze_time_requests()
                            .is_empty()
                    );
                    assert!(
                        peer.mission_scripts
                            .drain_visual_speed_multiplier_requests()
                            .is_empty()
                    );
                    assert!(
                        peer.mission_scripts
                            .drain_weather_visibility_updates()
                            .is_empty()
                    );
                    let bytes = owner.capture_camera_continuation().unwrap();
                    let mut restored = GameLogic::new();
                    restored.admit_camera_continuation(
                        GameLogic::decode_camera_continuation(&bytes).unwrap(),
                    );
                    assert_eq!(restored.is_script_time_frozen(), frozen);
                    assert_eq!(restored.visual_speed_multiplier(), speed as f32);
                    // Camera continuation does not serialize weather. Physical
                    // SnowManager isolation remains a separate migration.
                    peer.reset();
                    assert_eq!(owner.is_script_time_frozen(), frozen);
                    assert_eq!(owner.visual_speed_multiplier(), speed as f32);
                    assert_eq!(owner.weather_state().visible, visible);
                }
            }
        },
    );
}
