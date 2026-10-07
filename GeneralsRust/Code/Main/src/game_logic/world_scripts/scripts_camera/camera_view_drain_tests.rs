//! hq-7z881: count real flush acquisitions and preserve C++ action boundaries.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::{ScriptEngine, ScriptExecutionDriver, get_script_engine};

fn flush(world: &mut GameLogic) {
    HostScriptExecutionDriver::new(world)
        .after_action()
        .unwrap();
}

fn seed(world: &GameLogic, value: f32, enabled: bool) {
    let hooks = &world.mission_scripts;
    for x in [value - 1.0, value] {
        hooks.push_view_guardband(ViewGuardbandRequest {
            x_bias: x,
            y_bias: x + 0.5,
        });
    }
    for frames in [11, 29] {
        hooks.push_camera_bw_mode(CameraBwModeRequest { enabled, frames });
    }
    hooks.push_skybox_enabled(!enabled);
    hooks.push_skybox_enabled(enabled);
    hooks.push_camera_motion_blur(CameraMotionBlurRequest::Basic {
        zoom_in: false,
        saturate: true,
    });
    hooks.push_camera_motion_blur(CameraMotionBlurRequest::Follow { amount: 6 });
    hooks.push_camera_motion_blur(CameraMotionBlurRequest::EndFollow);
}

fn check(world: &mut GameLogic, value: f32, enabled: bool) {
    assert_eq!(
        world.take_view_guardband_request(),
        Some(ViewGuardbandRequest {
            x_bias: value,
            y_bias: value + 0.5
        })
    );
    assert_eq!(
        world.take_camera_bw_mode_request(),
        Some(CameraBwModeRequest {
            enabled,
            frames: 29
        })
    );
    assert_eq!(world.script_skybox_enabled, enabled);
    assert_eq!(
        world.take_camera_motion_blur_requests(),
        vec![
            CameraMotionBlurRequest::Basic {
                zoom_in: false,
                saturate: true
            },
            CameraMotionBlurRequest::Follow { amount: 6 },
            CameraMotionBlurRequest::EndFollow,
        ]
    );
    #[cfg(feature = "game_client")]
    game_client::display::view::with_tactical_view_ref(|view| {
        assert_eq!(
            view.guard_band_bias(),
            game_client::display::view::Vector2::new(value, value + 0.5)
        );
        assert_eq!(
            view.get_view_filter_type(),
            game_client::display::view::FilterType::MotionBlur
        );
        assert_eq!(
            view.get_view_filter_mode(),
            game_client::display::view::FilterMode::MBEndPanAlpha
        );
    });
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn camera_view_drain_real_driver_uses_one_acquisition_even_when_empty() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "camera_view_drain_real_driver_uses_one_acquisition_even_when_empty",
        || {
            let mut world = GameLogic::new();
            for populated in [true, false] {
                if populated {
                    seed(&world, 2.5, false);
                }
                let before = world.mission_scripts.view_drain_acquisitions_for_test();
                flush(&mut world);
                if populated {
                    check(&mut world, 2.5, false);
                } else {
                    assert!(world.take_view_guardband_request().is_none());
                    assert!(world.take_camera_bw_mode_request().is_none());
                    assert!(world.take_camera_motion_blur_requests().is_empty());
                }
                assert_eq!(
                    world.mission_scripts.view_drain_acquisitions_for_test() - before,
                    1,
                    "one notification acquisition for all four view families at the actual after_action"
                );
            }
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn camera_view_drain_interleaved_worlds_keep_payload_order_and_requeue_after_poison() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "camera_view_drain_interleaved_worlds_keep_payload_order_and_requeue_after_poison",
        || {
            let mut first = GameLogic::new();
            let mut second = GameLogic::new();
            seed(&first, 2.5, false);
            seed(&second, 7.5, true);
            first.mission_scripts.poison_notifications_for_test();
            flush(&mut first);
            check(&mut first, 2.5, false);
            assert!(second.pending_view_guardband.is_none());
            assert!(second.pending_camera_bw_mode.is_none());
            assert!(second.pending_camera_motion_blur.is_empty());
            flush(&mut second);
            check(&mut second, 7.5, true);
            assert!(first.pending_view_guardband.is_none());
            assert!(first.pending_camera_motion_blur.is_empty());
            // A later callback is a new request. Never replay the consumed batch.
            seed(&first, 4.5, true);
            flush(&mut first);
            check(&mut first, 4.5, true);
            flush(&mut first);
            assert!(first.take_view_guardband_request().is_none());
            assert!(first.take_camera_bw_mode_request().is_none());
            assert!(first.take_camera_motion_blur_requests().is_empty());
        },
    );
}

fn authored_action(kind: ScriptActionType, value: i32) -> Box<ScriptAction> {
    let mut action = ScriptAction::new(kind);
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, value))
        .unwrap();
    Box::new(action)
}

#[test]
#[cfg(all(not(target_arch = "wasm32"), feature = "game_client"))]
fn camera_view_drain_nested_authored_blur_then_bw_applies_before_next_action() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "camera_view_drain_nested_authored_blur_then_bw_applies_before_next_action",
        || {
            let mut condition = OrCondition::new();
            condition.set_first_and_condition(Some(Box::new(Condition::new(
                ConditionType::ConditionTrue,
            ))));
            let mut blur = authored_action(ScriptActionType::CameraMotionBlurFollow, 6);
            blur.next_action = Some(authored_action(ScriptActionType::CameraBwModeBegin, 29));
            let mut inner = Script::new();
            inner.script_name = "ViewOrder".into();
            inner.is_subroutine = true;
            inner.condition = Some(Box::new(condition.clone()));
            inner.action = Some(blur);
            let mut call = ScriptAction::new(ScriptActionType::CallSubroutine);
            call.add_parameter(Parameter::with_string(
                ParameterType::Script,
                "ViewOrder".into(),
            ))
            .unwrap();
            let mut outer = Script::new();
            outer.script_name = "OuterViewOrder".into();
            outer.is_one_shot = true;
            outer.condition = Some(Box::new(condition));
            outer.action = Some(Box::new(call));
            let mut list = ScriptList::new();
            list.append_script(Box::new(outer));
            list.append_script(Box::new(inner));
            let mut engine = ScriptEngine::new().unwrap();
            engine
                .set_script_list_for_player(0, Some(Box::new(list)))
                .unwrap();
            *get_script_engine().write().unwrap() = Some(engine);
            let mut world = GameLogic::new();
            world.scripts_loaded = true;
            world.evaluate_and_execute_scripts(0.0);
            assert_eq!(world.mission_script_counter, 1);
            assert_eq!(
                world.take_camera_motion_blur_requests(),
                vec![CameraMotionBlurRequest::Follow { amount: 6 }]
            );
            assert_eq!(
                world.take_camera_bw_mode_request(),
                Some(CameraBwModeRequest {
                    enabled: true,
                    frames: 29
                })
            );
            // A whole-walk snapshot would run BW before blur and fail this assertion.
            game_client::display::view::with_tactical_view_ref(|view| {
                assert_eq!(
                    view.get_view_filter_type(),
                    game_client::display::view::FilterType::BlackAndWhite
                );
                assert_eq!(
                    view.get_view_filter_mode(),
                    game_client::display::view::FilterMode::BWBlackAndWhite
                );
            });
        },
    );
}
