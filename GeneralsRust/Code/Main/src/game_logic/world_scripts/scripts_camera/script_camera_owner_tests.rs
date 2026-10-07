//! Actual driving-world camera callbacks, with deliberately foreign retained owners.
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::{ScriptEngine, get_script_engine};

fn real(kind: ScriptActionType, values: &[f32]) -> Box<ScriptAction> {
    let mut action = ScriptAction::new(kind);
    for value in values {
        action
            .add_parameter(Parameter::with_real(ParameterType::Real, *value))
            .unwrap();
    }
    Box::new(action)
}

fn authored(zoom: f32, hooks: Arc<MissionScriptHooks>) -> ScriptEngine {
    let mut rotate = real(ScriptActionType::RotateCamera, &[2.0, 3.0, 0.25, 0.5]);
    rotate.next_action = Some(real(ScriptActionType::ZoomCamera, &[zoom, 2.0, 0.2, 0.4]));
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "SameCamera".into();
    script.is_one_shot = false;
    script.condition = Some(Box::new(condition));
    script.action = Some(rotate);
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_action_handler(Some(Arc::new(MissionScriptActionHandler::new(hooks))));
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}

fn walk(world: &mut GameLogic, engine: ScriptEngine) -> ScriptEngine {
    *get_script_engine().write().unwrap() = Some(engine);
    world.scripts_loaded = true;
    world.evaluate_and_execute_scripts(0.0);
    get_script_engine().write().unwrap().take().unwrap()
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_main_camera_actions_ignore_another_world_retained_handler() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_main_camera_actions_ignore_another_world_retained_handler",
        || {
            let mut first = GameLogic::new();
            let mut second = GameLogic::new();
            let first_engine = authored(0.75, second.mission_scripts.clone());
            let second_engine = authored(1.25, first.mission_scripts.clone());
            assert!(first.pending_camera_rotate.is_none());
            assert!(second.pending_camera_zoom.is_none());
            let first_engine = walk(&mut first, first_engine);
            let rotate = first
                .pending_camera_rotate
                .as_ref()
                .expect("driving world must receive rotation");
            assert_eq!(
                (
                    rotate.rotations,
                    rotate.duration_seconds,
                    rotate.ease_in_seconds,
                    rotate.ease_out_seconds
                ),
                (2.0, 3.0, 0.25, 0.5)
            );
            let zoom = first.pending_camera_zoom.as_ref().unwrap();
            assert_eq!(
                (
                    zoom.zoom,
                    zoom.duration_seconds,
                    zoom.ease_in_seconds,
                    zoom.ease_out_seconds
                ),
                (0.75, 2.0, 0.2, 0.4)
            );
            assert!(second.pending_camera_rotate.is_none());
            assert!(
                second
                    .mission_scripts
                    .drain_camera_rotate_requests()
                    .is_empty()
            );
            assert!(
                second
                    .mission_scripts
                    .drain_camera_zoom_requests()
                    .is_empty()
            );
            let second_engine = walk(&mut second, second_engine);
            assert_eq!(second.pending_camera_zoom.as_ref().unwrap().zoom, 1.25);
            assert_eq!(first.pending_camera_zoom.as_ref().unwrap().zoom, 0.75);
            let _ = walk(&mut first, first_engine);
            let _ = walk(&mut second, second_engine);
            assert_eq!(first.pending_camera_zoom.as_ref().unwrap().zoom, 0.75);
            assert_eq!(second.pending_camera_zoom.as_ref().unwrap().zoom, 1.25);
            assert_eq!(
                (first.mission_script_counter, second.mission_script_counter),
                (2, 2)
            );
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_main_camera_move_keeps_engine_axes_and_zero_duration_without_a_handler() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_main_camera_move_keeps_engine_axes_and_zero_duration_without_a_handler",
        || {
            for seconds in [0.0, -2.0, 3.0] {
                let mut world = GameLogic::new();
                let mut action = ScriptAction::new(ScriptActionType::MoveCameraTo);
                action
                    .add_parameter(Parameter::with_coord(
                        ParameterType::Coord3D,
                        gamelogic::common::Coord3D::new(120.0, 240.0, 35.0),
                    ))
                    .unwrap();
                for value in [seconds, 0.5, 0.25, 0.75] {
                    action
                        .add_parameter(Parameter::with_real(ParameterType::Real, value))
                        .unwrap();
                }
                let mut script = Script::new();
                let mut condition = OrCondition::new();
                condition.set_first_and_condition(Some(Box::new(Condition::new(
                    ConditionType::ConditionTrue,
                ))));
                script.condition = Some(Box::new(condition));
                script.action = Some(Box::new(action));
                let mut list = ScriptList::new();
                list.append_script(Box::new(script));
                let mut engine = ScriptEngine::new().unwrap();
                assert!(engine.action_handler().is_none());
                engine
                    .set_script_list_for_player(0, Some(Box::new(list)))
                    .unwrap();
                let _ = walk(&mut world, engine);
                if seconds <= 0.0 {
                    assert!(world.script_camera_move_to.is_none());
                    assert_eq!(
                        world.peek_pending_camera_focus(),
                        Some(Vec3::new(120.0, 35.0, 240.0))
                    );
                } else {
                    let request = world
                        .script_camera_move_to
                        .as_ref()
                        .expect("owned timed move");
                    assert_eq!(request.target, Vec3::new(120.0, 35.0, 240.0));
                    assert_eq!(request.total_time_seconds, 3.0);
                    assert_eq!(request.shutter_frames, 15);
                }
            }
        },
    );
}
