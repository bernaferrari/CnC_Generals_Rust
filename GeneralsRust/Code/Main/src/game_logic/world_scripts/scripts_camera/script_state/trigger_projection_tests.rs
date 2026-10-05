//! hq-3dabn: OLD/GREEN behavior controls at the ordinary Main script update.
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::{ScriptEngine, get_script_engine, with_script_engine_ref};

fn area() -> gamelogic::polygon_trigger::PolygonTrigger {
    use gamelogic::common::{AsciiString, ICoord3D};
    gamelogic::polygon_trigger::PolygonTrigger::new(
        19321,
        AsciiString::from("ProjectionArea"),
        vec![
            ICoord3D::new(100, 100, 0),
            ICoord3D::new(140, 100, 0),
            ICoord3D::new(140, 140, 0),
            ICoord3D::new(100, 140, 0),
        ],
    )
}

fn world(inside: bool) -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world
        .host_trigger_world
        .lock()
        .unwrap()
        .set_trigger_areas(&[area()]);
    world.set_current_frame(20);
    for (name, kind) in [
        ("ProjectionUnit", None),
        ("ProjectionProjectile", Some(KindOf::Projectile)),
        ("ProjectionInert", Some(KindOf::Inert)),
    ] {
        let mut template = ThingTemplate::new(name);
        template.set_health(100.0);
        if let Some(kind) = kind {
            template.add_kind_of(kind);
        }
        world.templates.insert(name.into(), template);
    }
    let ids: Vec<_> = ["ProjectionUnit", "ProjectionProjectile", "ProjectionInert"]
        .into_iter()
        .map(|name| {
            world
                .create_object(name, Team::USA, Vec3::new(50.0, 0.0, 50.0))
                .unwrap()
        })
        .collect();
    for (id, name) in ids.iter().zip(["Scout", "Projectile", "Inert"]) {
        let object = world.host_object_mut(*id).unwrap();
        object.name = name.into();
        if inside {
            object.set_position(Vec3::new(120.0, 0.0, 120.0));
        }
    }
    let peer = world
        .create_object("ProjectionUnit", Team::USA, Vec3::new(60.0, 0.0, 60.0))
        .unwrap();
    world.host_object_mut(peer).unwrap().name = "Peer".into();
    // C++ early scripts still execute while script time is frozen. This keeps
    // the control bounded to the production script phase of public update().
    world.script_time_frozen_by_script = true;
    (world, ids[0], ids[1], ids[2])
}

fn install_condition(world: &mut GameLogic, unit: &str, kind: ConditionType) {
    let mut condition = Condition::new(kind);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Unit, unit.into()))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "ProjectionArea".into(),
        ))
        .unwrap();
    let mut or = OrCondition::new();
    or.set_first_and_condition(Some(Box::new(condition)));
    let mut action = ScriptAction::new(ScriptActionType::SetFlag);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Flag,
            "ProjectedCondition".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
        .unwrap();
    let mut script = Script::new();
    script.script_name = "ProjectionControl".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(or));
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
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

fn state(world: &GameLogic) -> String {
    format!("{:?}", world.host_trigger_world.lock().unwrap().capture())
}

fn check_update(world: &mut GameLogic, unit: &str, kind: ConditionType, expected: bool) {
    install_condition(world, unit, kind);
    let before = world.mission_script_counter;
    world.update();
    assert_eq!(
        world.mission_script_counter,
        before + 1,
        "ordinary early script phase must run"
    );
    let actual = with_script_engine_ref(|engine| {
        engine
            .get_flag("ProjectedCondition")
            .is_some_and(|flag| flag.value)
    })
    .unwrap();
    assert_eq!(
        actual, expected,
        "authored native condition on driving Main owner"
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn trigger_projection_authored_update_keeps_same_id_owners_skip_and_two_frame_edges() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "trigger_projection_authored_update_keeps_same_id_owners_skip_and_two_frame_edges",
        || {
            let (mut a, id, projectile, inert) = world(true);
            let (mut b, b_id, _, _) = world(false);
            assert_eq!(id, b_id);
            check_update(&mut a, "Scout", ConditionType::NamedEnteredArea, true);
            let saved_a = state(&a);
            check_update(&mut b, "Scout", ConditionType::NamedInsideArea, false);
            assert_eq!(state(&a), saved_a);
            check_update(&mut a, "Scout", ConditionType::NamedInsideArea, true);
            check_update(&mut a, "Peer", ConditionType::NamedEnteredArea, false);
            check_update(&mut a, "Projectile", ConditionType::NamedEnteredArea, false);
            check_update(&mut a, "Inert", ConditionType::NamedEnteredArea, false);
            let rows = a.host_trigger_world.lock().unwrap().capture();
            assert!(!rows.iter().any(|entry| entry.object_id == projectile.0));
            assert!(!rows.iter().any(|entry| entry.object_id == inert.0));
            a.set_current_frame(21);
            check_update(&mut a, "Scout", ConditionType::NamedEnteredArea, true);
            a.set_current_frame(22);
            check_update(&mut a, "Scout", ConditionType::NamedEnteredArea, false);
            check_update(&mut a, "Scout", ConditionType::NamedInsideArea, true);
            a.set_current_frame(23);
            a.host_object_mut(id)
                .unwrap()
                .set_position(Vec3::new(170.0, 0.0, 170.0));
            check_update(&mut a, "Scout", ConditionType::NamedExitedArea, false);
            // C++ checks the previous integer pose for exit. A second ordinary
            // movement observes the outside prior pose, without injecting flags.
            a.host_object_mut(id)
                .unwrap()
                .set_position(Vec3::new(180.0, 0.0, 180.0));
            check_update(&mut a, "Scout", ConditionType::NamedExitedArea, true);
            a.set_current_frame(24);
            check_update(&mut a, "Scout", ConditionType::NamedExitedArea, true);
            a.set_current_frame(25);
            check_update(&mut a, "Scout", ConditionType::NamedExitedArea, false);
            check_update(&mut a, "Scout", ConditionType::NamedInsideArea, false);
            let saved_a = state(&a);
            b.reset();
            assert!(b.host_trigger_world.lock().unwrap().capture().is_empty());
            assert_eq!(state(&a), saved_a);
            drop(b);
            let other = GameLogic::new();
            assert!(
                other
                    .host_trigger_world
                    .lock()
                    .unwrap()
                    .capture()
                    .is_empty()
            );
            assert_eq!(state(&a), saved_a);
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn trigger_projection_empty_public_census_preserves_owner_frame() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "trigger_projection_empty_public_census_preserves_owner_frame",
        || {
            let mut world = GameLogic::new();
            world.frame = 77;
            world.inject_host_script_query_snapshot();
            assert_eq!(world.host_trigger_world.lock().unwrap().current_frame(), 0);
            assert!(
                world
                    .host_trigger_world
                    .lock()
                    .unwrap()
                    .capture()
                    .is_empty()
            );
        },
    );
}
