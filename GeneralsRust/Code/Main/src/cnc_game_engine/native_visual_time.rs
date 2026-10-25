//! Late, immutable native Client input from the receiving session.
//! No camera is reconstructed and no visual/simulation clock advances here.
use crate::game_logic::GameLogic;
use game_client::core::game_client::NativeVisualTime;

pub(super) fn simulation_frozen(logic: &GameLogic, paused: bool) -> bool {
    logic.is_time_frozen_for_simulation() || paused
}

pub(super) fn observe_receiving(
    logic: &GameLogic,
    paused: bool,
    interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
) -> Result<NativeVisualTime, String> {
    try_observe(logic, paused, || {
        logic
            .script_debug_frozen(interpreter)
            .map_err(|error| error.to_string())
    })
}
fn try_observe(
    logic: &GameLogic,
    paused: bool,
    debug_frozen: impl FnOnce() -> Result<bool, String>,
) -> Result<NativeVisualTime, String> {
    // Camera first: never query an absent/debug owner on the branch that
    // original GameClient.cpp621-624 already short-circuited.
    let frozen = logic.is_script_camera_time_frozen()
        || debug_frozen()?
        || logic.is_script_time_frozen()
        || paused;
    Ok(NativeVisualTime {
        frozen,
        visual_speed: logic.visual_speed_multiplier(),
    })
}
#[cfg(test)]
fn observe(
    logic: &GameLogic,
    paused: bool,
    debug_frozen: impl FnOnce() -> bool,
) -> NativeVisualTime {
    try_observe(logic, paused, || Ok(debug_frozen()))
        .expect("infallible supplied fixture observation")
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_client::display::view::{FilterMode, FilterType};

    // A detached load candidate lets this fixture choose a speed without a
    // global script registry or exposing another mutable production setter.
    fn saved_speed(logic: &mut GameLogic, speed: f32) {
        let bytes = logic.capture_camera_continuation().unwrap();
        let state = GameLogic::decode_camera_continuation(&bytes).unwrap();
        let mut value = serde_json::to_value(state).unwrap();
        value["visual_speed_multiplier"] = serde_json::json!(speed);
        logic
            .admit_camera_continuation(serde_json::from_value(value).unwrap())
            .unwrap();
        let encoded = logic.capture_camera_continuation().unwrap();
        logic
            .admit_camera_continuation(GameLogic::decode_camera_continuation(&encoded).unwrap())
            .unwrap();
        assert_eq!(logic.visual_speed_multiplier(), speed);
    }

    #[test]
    fn native_visual_observation_uses_receiving_world_and_encoded_continuation() {
        let mut owner = GameLogic::new();
        let mut peer = GameLogic::new();
        saved_speed(&mut owner, 3.);
        saved_speed(&mut peer, 7.);
        peer.set_script_time_frozen_for_test(true);
        let before = owner.capture_camera_continuation().unwrap();
        assert_eq!(
            (
                observe(&owner, false, || false).frozen,
                observe(&owner, false, || false).visual_speed
            ),
            (false, 3.)
        );
        assert_eq!(
            (
                observe(&peer, false, || false).frozen,
                observe(&peer, false, || false).visual_speed
            ),
            (true, 7.)
        );
        peer = GameLogic::new();
        assert_eq!(observe(&peer, false, || false).visual_speed, 1.);
        let mut restored = GameLogic::new();
        restored
            .admit_camera_continuation(GameLogic::decode_camera_continuation(&before).unwrap())
            .unwrap();
        for _ in 0..120 {
            assert_eq!(observe(&owner, false, || false).visual_speed, 3.);
            assert_eq!(observe(&restored, false, || false).visual_speed, 3.);
            assert!(!observe(&owner, false, || false).frozen);
        }
        assert_eq!(owner.capture_camera_continuation().unwrap(), before);
        assert_eq!(restored.capture_camera_continuation().unwrap(), before);
    }

    #[test]
    fn native_camera_freeze_observes_current_completion_and_motion_blur_exceptions() {
        let mut world = GameLogic::new();
        world.session_view.scripted.script_camera_freeze_time = true;
        world.session_view.camera.camera_zoom_target = Some(2.);
        let before = world.capture_camera_continuation().unwrap();
        assert!(observe(&world, false, || false).frozen);
        assert_eq!(world.capture_camera_continuation().unwrap(), before);
        for mode in [
            FilterMode::MBInAndOutAlpha,
            FilterMode::MBInAndOutSaturate,
            FilterMode::MBInAlpha,
            FilterMode::MBOutAlpha,
            FilterMode::MBInSaturate,
            FilterMode::MBOutSaturate,
        ] {
            world.session_view.filter.set_filter(FilterType::MotionBlur);
            world.session_view.filter.set_mode(mode);
            assert!(!observe(&world, false, || false).frozen, "{mode:?}");
            assert_eq!(world.session_view.camera.camera_zoom_target, Some(2.));
        }
        world.session_view.filter.set_filter(FilterType::Null);
        assert!(observe(&world, false, || false).frozen);
        world.session_view.camera.camera_zoom_target = None;
        assert!(!observe(&world, false, || false).frozen);
    }

    #[test]
    fn native_freeze_truth_table_preserves_distinct_pause_debug_script_and_camera_inputs() {
        let mut world = GameLogic::new();
        for paused in [false, true] {
            for debug in [false, true] {
                for script in [false, true] {
                    for camera in [false, true] {
                        for moving in [false, true] {
                            world.set_script_time_frozen_for_test(script);
                            world.session_view.scripted.script_camera_freeze_time = camera;
                            world.session_view.camera.camera_zoom_target = moving.then_some(2.);
                            assert_eq!(
                                observe(&world, paused, || debug).frozen,
                                paused || debug || script || (camera && moving)
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn camera_freeze_short_circuits_the_late_debug_query_but_script_and_pause_do_not() {
        let mut world = GameLogic::new();
        world.session_view.scripted.script_camera_freeze_time = true;
        world.session_view.camera.camera_zoom_target = Some(2.);
        let mut calls = 0;
        assert!(
            observe(&world, false, || {
                calls += 1;
                false
            })
            .frozen
        );
        assert_eq!(calls, 0);
        world.session_view.camera.camera_zoom_target = None;
        world.set_script_time_frozen_for_test(true);
        assert!(
            observe(&world, false, || {
                calls += 1;
                false
            })
            .frozen
        );
        assert_eq!(calls, 1);
        world.set_script_time_frozen_for_test(false);
        assert!(
            observe(&world, true, || {
                calls += 1;
                false
            })
            .frozen
        );
        assert_eq!(calls, 2);
    }

    #[test]
    fn late_native_visual_time_reads_receiving_debug_owner_and_preserves_short_circuit() {
        let mut owner = GameLogic::new();
        let peer = GameLogic::new();
        let mut engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
        engine.set_time_frozen_debug(true);
        owner.install_owned_script_engine(engine);
        assert!(observe_receiving(&owner, false, None).unwrap().frozen);
        assert!(!observe_receiving(&peer, false, None).unwrap().frozen);
        owner
            .with_owned_script_engine(|engine, owner| {
                assert!(observe_receiving(owner, false, None).is_err());
                assert!(
                    observe_receiving(owner, false, Some(engine))
                        .unwrap()
                        .frozen
                );
                owner.session_view.scripted.script_camera_freeze_time = true;
                owner.session_view.camera.camera_zoom_target = Some(2.);
                assert!(observe_receiving(owner, false, None).unwrap().frozen);
                owner.session_view.camera.camera_zoom_target = None;
                owner.set_script_time_frozen_for_test(true);
                assert!(observe_receiving(owner, false, None).is_err());
                owner.set_script_time_frozen_for_test(false);
                assert!(observe_receiving(owner, true, None).is_err());
            })
            .unwrap();
    }
}
