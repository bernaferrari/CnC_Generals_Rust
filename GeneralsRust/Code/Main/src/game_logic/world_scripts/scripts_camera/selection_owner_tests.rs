//! Actual Main selection producers and CPP ScriptConditions1372 cache timing.
fn world() -> (GameLogic, ObjectId, ObjectId) {
    let (mut world, unit, target) = super::named_command_test_support::world();
    // This fixture exercises actual drawable selection, whose authored
    // SELECTABLE gate is independent of the AI command fixture's kinds.
    for id in [unit, target] {
        world
            .host_object_mut(id)
            .unwrap()
            .template_mut()
            .add_kind_of(KindOf::Selectable);
        assert!(world.host_object(id).unwrap().is_selectable());
    }
    (world, unit, target)
}
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script,
};
use gamelogic::scripting::engine::{ScriptEngine, ScriptExecutionDriver, ScriptOwnerQuery};

fn selected_script(name: &str) -> Script {
    let mut condition = Condition::new(ConditionType::NamedSelected);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Unit, name.into()))
        .unwrap();
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(condition)));
    let mut script = Script::new();
    script.condition = Some(Box::new(branch));
    script
}
fn evaluate(world: &mut GameLogic, engine: &ScriptEngine, script: &mut Script) -> bool {
    let frame = world.frame;
    let mut driver = HostScriptExecutionDriver::with_interpreter(world, engine);
    engine.evaluate_conditions_with_driver(
        script,
        None,
        None,
        gamelogic::scripting::executor::ScriptContext::at_frame(frame),
        &mut driver,
    )
}
fn status(
    world: &mut GameLogic,
    name: &str,
) -> gamelogic::scripting::engine::ScriptSelectionStatus {
    match HostScriptExecutionDriver::new(world).named_selection(name) {
        ScriptOwnerQuery::Present(status) => status,
        _ => panic!("Main selection owner must answer its query"),
    }
}

#[test]
fn native_selection_stamp_changes_only_at_actual_local_mutations() {
    let (mut owner, id, _) = world();
    let (mut peer, peer_id, _) = world();
    assert_eq!(id, peer_id);
    owner.frame = 7;
    owner.select_objects_no_sound(1, vec![id]);
    assert_eq!(owner.selection_changed_frame, 7);
    assert!(status(&mut owner, "NamedUnit").selected);
    assert!(
        !status(&mut owner, "namedunit").selected,
        "CPP uses literal case-sensitive names"
    );
    assert!(!status(&mut owner, "ThisObject").selected);
    owner.frame = 8;
    owner.select_objects_no_sound(1, vec![id]);
    assert_eq!(
        owner.selection_changed_frame, 7,
        "no-op selection does not stamp"
    );
    peer.frame = 9;
    peer.select_objects_no_sound(1, vec![peer_id]);
    peer.deselect_drawable(peer_id);
    owner.frame = 80;
    assert_eq!(
        status(&mut owner, "NamedUnit").changed_frame,
        7,
        "query never samples observation frame"
    );
    owner.deselect_drawable(id);
    assert_eq!(owner.selection_changed_frame, 80);
    owner.frame = 81;
    owner.deselect_drawable(id);
    assert_eq!(owner.selection_changed_frame, 80);
}

#[test]
fn actual_selection_condition_preserves_same_frame_cache_and_multiplayer_cache() {
    let (mut owner, id, _) = world();
    let engine = ScriptEngine::new().unwrap();
    let mut script = selected_script("NamedUnit");
    owner.frame = 7;
    owner.select_objects_no_sound(1, vec![id]);
    assert!(evaluate(&mut owner, &engine, &mut script));
    owner.deselect_drawable(id);
    assert!(
        evaluate(&mut owner, &engine, &mut script),
        "same mutation frame intentionally retains CPP cache"
    );
    owner.frame = 8;
    owner.select_objects_no_sound(1, vec![id]);
    owner.frame = 9;
    owner.deselect_drawable(id);
    assert!(!evaluate(&mut owner, &engine, &mut script));
    owner.game_mode = GameMode::Multiplayer;
    owner.frame = 10;
    owner.select_objects_no_sound(1, vec![id]);
    assert!(!evaluate(&mut owner, &engine, &mut script));
    owner.game_mode = GameMode::SinglePlayer;
    assert!(
        evaluate(&mut owner, &engine, &mut script),
        "multiplayer must leave cached frame unchanged"
    );
}

#[test]
fn actual_area_click_unit_delete_and_remote_mask_use_local_selection_owner() {
    let (mut owner, id, target) = world();
    owner.frame = 3;
    assert_eq!(
        owner.select_objects_in_area(1, Vec3::ZERO, Vec3::splat(60.0), false),
        vec![id]
    );
    assert!(status(&mut owner, "NamedUnit").selected);
    owner.frame = 4;
    assert_eq!(
        owner.select_object_at_position(1, Vec3::splat(900.0), 1.0, false),
        None
    );
    assert!(!status(&mut owner, "NamedUnit").selected);
    owner.frame = 5;
    assert!(owner.unit_select_if_player(id, 1));
    assert_eq!(owner.selection_changed_frame, 5);
    owner.frame = 6;
    owner.select_objects_no_sound(2, vec![target]);
    assert_eq!(
        owner.selection_changed_frame, 5,
        "remote replicated player selection does not change native UI selection"
    );
    assert!(!status(&mut owner, "NamedTarget").selected);
    owner.frame = 7;
    owner.destroy_object(id);
    assert!(
        status(&mut owner, "NamedUnit").selected,
        "CPP destroyObject queues removal; onDelete does not destroy the drawable"
    );
    owner.process_destroy_list();
    assert!(!status(&mut owner, "NamedUnit").selected);
    assert_eq!(
        owner.selection_changed_frame, 7,
        "physical drawable removal stamps the actual removal phase"
    );
}

#[test]
fn actual_owned_mask_operation_deselects_only_its_session_before_queries() {
    let (mut owner, id, _) = world();
    let (mut peer, peer_id, _) = world();
    owner.frame = 4;
    peer.frame = 4;
    owner.select_objects_no_sound(1, vec![id]);
    peer.select_objects_no_sound(1, vec![peer_id]);
    owner.frame = 5;
    owner.with_host_object_mut(id, |object| object.set_status_masked(true));
    assert_eq!(owner.selection_changed_frame, 5);
    assert!(!status(&mut owner, "NamedUnit").selected);
    assert!(owner.get_player(1).unwrap().selected_objects.is_empty());
    peer.drain_masked_object_selection();
    assert!(status(&mut peer, "NamedUnit").selected);
    assert_eq!(peer.selection_changed_frame, 4);
}
