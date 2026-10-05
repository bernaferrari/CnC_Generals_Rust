//! hq-g90ur: actual authored/action-boundary and external queue OLD/GREEN controls.
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::{ScriptActionHandler, ScriptEngine, get_script_engine};

fn world(x: f32) -> (GameLogic, ObjectId, Vec3) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "FocusOwner", true));
    let mut unit = ThingTemplate::new("FocusUnit");
    unit.set_health(100.0).add_kind_of(KindOf::Selectable);
    world.templates.insert("FocusUnit".into(), unit);
    let mut home = ThingTemplate::new("AmericaCommandCenter");
    home.add_kind_of(KindOf::Structure).set_health(100.0);
    world.templates.insert("AmericaCommandCenter".into(), home);
    let id = world
        .create_object_for_player("FocusUnit", 0, Vec3::new(x, 0.0, 130.0))
        .unwrap();
    world.host_object_mut(id).unwrap().name = "FocusScout".into();
    assert!(world.host_object(id).unwrap().is_selectable());
    world.select_objects(0, vec![id]);
    assert_eq!(world.players.get(&0).unwrap().selected_objects, vec![id]);
    let home = Vec3::new(x + 40.0, 0.0, 170.0);
    world
        .create_object_for_player("AmericaCommandCenter", 0, home)
        .unwrap();
    world.script_time_frozen_by_script = true;
    (world, id, home)
}

fn named(kind: ScriptActionType, tether: bool) -> Box<ScriptAction> {
    let mut action = ScriptAction::new(kind);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "FocusScout".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
        .unwrap();
    if tether {
        action
            .add_parameter(Parameter::with_real(ParameterType::Real, 12.0))
            .unwrap();
    }
    Box::new(action)
}

fn move_to(position: Vec3, seconds: f32) -> Box<ScriptAction> {
    let mut action = ScriptAction::new(ScriptActionType::MoveCameraTo);
    action
        .add_parameter(Parameter::with_coord(
            ParameterType::Coord3D,
            gamelogic::common::Coord3D::new(position.x, position.z, position.y),
        ))
        .unwrap();
    for value in [seconds, 0.0, 0.0, 0.0] {
        action
            .add_parameter(Parameter::with_real(ParameterType::Real, value))
            .unwrap();
    }
    Box::new(action)
}

fn install(world: &mut GameLogic, mut actions: Vec<Box<ScriptAction>>, nested: bool) {
    let mut next = None;
    while let Some(mut action) = actions.pop() {
        action.next_action = next;
        next = Some(action);
    }
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "FocusActions".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(condition));
    script.action = next;
    let mut list = ScriptList::new();
    if nested {
        script.is_subroutine = true;
        let mut call = ScriptAction::new(ScriptActionType::CallSubroutine);
        call.add_parameter(Parameter::with_string(
            ParameterType::Script,
            "FocusActions".into(),
        ))
        .unwrap();
        let mut outer = Script::new();
        outer.script_name = "OuterFocus".into();
        outer.is_one_shot = true;
        outer.condition = script.condition.clone();
        outer.action = Some(Box::new(call));
        list.append_script(Box::new(outer));
    }
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_action_handler(Some(Arc::new(MissionScriptActionHandler::new(
        world.mission_scripts.clone(),
    ))));
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    *get_script_engine().write().unwrap() = Some(engine);
    world.scripts_loaded = true;
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn camera_focus_authored_nested_public_update_keeps_tether_then_home_order() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "camera_focus_authored_nested_public_update_keeps_tether_then_home_order",
        || {
            let (mut world, _, home) = world(110.0);
            install(
                &mut world,
                vec![
                    move_to(Vec3::new(350.0, 0.0, 350.0), 10.0),
                    Box::new(ScriptAction::new(ScriptActionType::MoveCameraToSelection)),
                    named(ScriptActionType::CameraTetherNamed, true),
                    Box::new(ScriptAction::new(ScriptActionType::CameraMoveHome)),
                ],
                true,
            );
            world.update();
            assert_eq!(world.mission_script_counter, 1);
            assert_eq!(
                world.camera_follow_object_id(),
                None,
                "home sees the previous nested tether"
            );
            assert_eq!(world.take_camera_focus_request(), Some(home));
            assert!(
                world.script_camera_move_to_target().is_none(),
                "earlier tether clears earlier path"
            );
            // The prior walk was consumed; an empty actual final flush does not replay it.
            install(&mut world, vec![], false);
            world.update();
            assert!(world.take_camera_focus_request().is_none());
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn camera_focus_same_id_public_updates_and_next_flush_stay_on_driving_owner() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "camera_focus_same_id_public_updates_and_next_flush_stay_on_driving_owner",
        || {
            let (mut a, id, _) = world(110.0);
            let (mut b, b_id, _) = world(210.0);
            assert_eq!(id, b_id);
            for world in [&mut a, &mut b] {
                install(
                    world,
                    vec![named(ScriptActionType::CameraFollowNamed, false)],
                    false,
                );
                world.update();
                let expected = world.host_object(id).unwrap().get_position();
                assert_eq!(world.camera_follow_object_id(), Some(id));
                assert_eq!(world.take_camera_focus_request(), Some(expected));
            }
            assert_eq!(
                a.peek_camera_follow_target_position(),
                Some(Vec3::new(110.0, 0.0, 130.0))
            );
            install(
                &mut a,
                vec![
                    Box::new(ScriptAction::new(ScriptActionType::CameraStopFollow)),
                    move_to(Vec3::new(400.0, 5.0, 450.0), 0.0),
                ],
                true,
            );
            a.update();
            assert_eq!(a.camera_follow_object_id(), None);
            assert_eq!(
                a.take_camera_focus_request(),
                Some(Vec3::new(400.0, 5.0, 450.0))
            );
            assert_eq!(b.camera_follow_object_id(), Some(b_id));
            assert!(b.take_camera_focus_request().is_none());
            a.reset();
            install(&mut a, vec![], false);
            a.update();
            assert!(
                a.take_camera_focus_request().is_none(),
                "consumed batch must not replay after reset"
            );
            drop(a);
            let c = GameLogic::new();
            assert_eq!(c.camera_follow_object_id(), None);
            assert_eq!(
                b.peek_camera_follow_target_position(),
                Some(Vec3::new(210.0, 0.0, 130.0))
            );
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn camera_focus_external_public_handler_flush_keeps_family_last_and_presence_semantics() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "camera_focus_external_public_handler_flush_keeps_family_last_and_presence_semantics",
        || {
            let (mut world, id, home) = world(110.0);
            install(&mut world, vec![], false);
            let handler = MissionScriptActionHandler::new(world.mission_scripts.clone());
            handler.move_camera(10.0, 20.0, 3.0).unwrap();
            handler.move_camera(30.0, 40.0, 5.0).unwrap();
            handler.move_camera_to_selection().unwrap();
            handler.camera_move_home().unwrap();
            handler.camera_follow_object(id.0, true).unwrap();
            handler.stop_camera_follow().unwrap();
            handler.camera_tether_object(id.0, true, 4.0).unwrap();
            handler.camera_tether_object(id.0, true, 9.0).unwrap();
            world.update();
            assert_eq!(world.camera_follow_object_id(), Some(id));
            assert_eq!(world.peek_camera_tether_play(), Some(9.0));
            assert_eq!(
                world.take_camera_focus_request(),
                Some(Vec3::new(110.0, 0.0, 130.0))
            );
            assert!(world.mission_scripts.drain_camera_moves().is_empty());
            assert!(
                world
                    .mission_scripts
                    .drain_camera_move_to_selection_requests()
                    .is_empty()
            );
            assert!(
                world
                    .mission_scripts
                    .drain_camera_move_home_requests()
                    .is_empty()
            );
            assert!(world.mission_scripts.drain_camera_follows().is_empty());
            assert!(world.mission_scripts.drain_camera_tethers().is_empty());
            // New requests enter a fresh actual flush, once; no private queue insertion.
            handler.camera_move_home().unwrap();
            world.update();
            assert_eq!(world.camera_follow_object_id(), None);
            assert_eq!(world.take_camera_focus_request(), Some(home));
            world.update();
            assert!(world.take_camera_focus_request().is_none());
            handler.move_camera(10.0, 20.0, 3.0).unwrap();
            handler.move_camera(30.0, 40.0, 5.0).unwrap();
            world.update();
            assert_eq!(
                world.take_camera_focus_request(),
                Some(Vec3::new(30.0, 5.0, 40.0)),
                "moves retain their insertion order within a single final flush"
            );
            world.update();
            assert!(world.take_camera_focus_request().is_none());
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn camera_focus_authored_selection_mod_observes_previous_nested_move() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "camera_focus_authored_selection_mod_observes_previous_nested_move",
        || {
            let (mut world, id, _) = world(110.0);
            install(
                &mut world,
                vec![
                    move_to(Vec3::new(350.0, 0.0, 350.0), 10.0),
                    Box::new(ScriptAction::new(ScriptActionType::MoveCameraToSelection)),
                ],
                true,
            );
            world.update();
            assert_eq!(
                world.script_camera_move_to_target(),
                Some(world.host_object(id).unwrap().get_position()),
                "selection modifier must see the move created by the previous nested action"
            );
        },
    );
}
