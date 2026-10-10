//! ScriptActions.cpp1042/6081: commands belong to the driving session.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;

fn world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.world_min = Vec3::ZERO;
    world.world_max = Vec3::splat(1000.0);
    world.add_player(Player::new(1, Team::USA, "NamedOwner", true));
    world.add_player(Player::new(2, Team::China, "NamedVictim", false));
    let mut template = ThingTemplate::new("OwnedNamedInfantry");
    template
        .add_kind_of(KindOf::Infantry)
        .set_health(100.0)
        .set_primary_weapon(Weapon::default());
    world.templates.insert(template.name.clone(), template);
    let unit = world
        .create_object_for_player("OwnedNamedInfantry", 1, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    let target = world
        .create_object_for_player("OwnedNamedInfantry", 2, Vec3::new(70.0, 0.0, 70.0))
        .unwrap();
    world.host_object_mut(unit).unwrap().name = "NamedUnit".into();
    world.host_object_mut(target).unwrap().name = "NamedTarget".into();
    let object = world.host_object_mut(unit).unwrap();
    object.set_formation(19, glam::Vec2::new(4.0, 6.0));
    object.max_shots_to_fire = 3;
    object.is_panicking = true;
    (world, unit, target)
}

fn execute(world: &mut GameLogic, actions: &[(ScriptActionType, &str, &str)]) -> i32 {
    let mut next = None;
    for (kind, unit, target) in actions.iter().rev() {
        let mut action = ScriptAction::new(*kind);
        for name in [unit, target] {
            action
                .add_parameter(Parameter::with_string(ParameterType::Unit, (*name).into()))
                .unwrap();
        }
        action.next_action = next;
        next = Some(Box::new(action));
    }
    // A real counter after the commands calibrates the production action walk.
    let mut counter = ScriptAction::new(ScriptActionType::IncrementCounter);
    counter
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    counter
        .add_parameter(Parameter::with_string(
            ParameterType::Counter,
            "NamedObserved".into(),
        ))
        .unwrap();
    if let Some(head) = next.as_mut() {
        let mut tail = head.as_mut();
        while let Some(ref mut following) = tail.next_action {
            tail = following.as_mut();
        }
        tail.next_action = Some(Box::new(counter));
    } else {
        next = Some(Box::new(counter));
    }
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "OwnedNamedCommand".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(branch));
    script.action = next;
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_counter("NamedObserved", 0).unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
        .update_with_driver(
            gamelogic::scripting::executor::ScriptContext::at_frame(world.frame),
            &mut HostScriptExecutionDriver::new(world),
        )
        .unwrap();
    engine.get_counter("NamedObserved").unwrap().value
}

#[test]
fn named_action_walk_calibration() {
    let (mut world, _, _) = world();
    assert_eq!(execute(&mut world, &[]), 1);
}

#[test]
fn named_attack_leaves_formation_and_sets_script_unlimited_force_order() {
    let (mut first, id, target) = world();
    let (mut second, second_id, _) = world();
    assert_eq!(id, second_id);
    first.host_object_mut(id).unwrap().pending_move = Some(Vec3::new(250.0, 0.0, 250.0));
    assert_eq!(
        execute(
            &mut first,
            &[(
                ScriptActionType::NamedAttackNamed,
                "NamedUnit",
                "NamedTarget"
            )]
        ),
        1
    );
    first.reissue_pending_moves();
    let object = first.host_object(id).unwrap();
    assert_eq!(object.pending_move, None);
    assert_eq!(
        object.formation_id, 0,
        "script attack must leave the driving formation"
    );
    assert!(!object.is_panicking);
    assert_eq!(object.target, Some(target));
    assert_eq!(object.ai_state, AIState::Attacking);
    assert!(object.force_attack);
    assert_eq!(object.max_shots_to_fire, -1);
    assert_eq!(
        object.last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(second.host_object(id).unwrap().formation_id, 19);
    assert_eq!(second.host_object(id).unwrap().target, None);
    second.reset();
    assert_eq!(first.host_object(id).unwrap().target, Some(target));
}

#[test]
fn named_face_retains_object_goal_and_clears_route_without_idling_passengers() {
    let (mut world, id, target) = world();
    let passenger = world
        .create_object_for_player("OwnedNamedInfantry", 1, Vec3::ZERO)
        .unwrap();
    world
        .host_object_mut(passenger)
        .unwrap()
        .set_target(Some(target));
    world.host_object_mut(id).unwrap().occupants.push(passenger);
    assert!(world.append_unit_waypoint(id, Vec3::new(200.0, 0.0, 200.0)));
    world.host_object_mut(id).unwrap().num_frames_blocked = 7;
    world.host_object_mut(id).unwrap().pending_move = Some(Vec3::new(300.0, 0.0, 300.0));
    assert_eq!(
        execute(
            &mut world,
            &[(ScriptActionType::NamedFaceNamed, "NamedUnit", "NamedTarget")]
        ),
        1
    );
    world.reissue_pending_moves();
    let object = world.host_object(id).unwrap();
    assert_eq!(object.pending_move, None);
    assert_eq!(
        object.ai_state,
        AIState::FacingObject,
        "face must retain a live target, not a captured position"
    );
    assert_eq!(object.target, Some(target));
    assert_eq!(object.face_goal_pos, None);
    assert_eq!(object.formation_id, 0);
    assert!(!object.is_panicking);
    assert!(object.movement.path.is_empty());
    assert_eq!(object.requested_destination, None);
    assert!(!object.waiting_for_path);
    assert_eq!(object.num_frames_blocked, 0);
    assert_eq!(
        object.last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(
        world.host_object(passenger).unwrap().target,
        Some(target),
        "clearWaypointQueue is not passenger Stop"
    );
}

#[test]
fn named_commands_missing_exact_names_and_no_ai_are_inert() {
    let (mut world, id, target) = world();
    assert_eq!(
        execute(
            &mut world,
            &[
                (
                    ScriptActionType::NamedAttackNamed,
                    "namedunit",
                    "NamedTarget"
                ),
                (ScriptActionType::NamedFaceNamed, "NamedUnit", "namedtarget"),
                (ScriptActionType::NamedAttackNamed, "NamedUnit", "Absent"),
            ]
        ),
        1
    );
    // CPP named cache omits unnamed objects, even when an empty parameter
    // happens to match their empty Object.name.
    let unnamed = world
        .create_object_for_player("OwnedNamedInfantry", 1, Vec3::ZERO)
        .unwrap();
    world
        .host_object_mut(unnamed)
        .unwrap()
        .set_formation(43, glam::Vec2::ZERO);
    execute(
        &mut world,
        &[
            (ScriptActionType::NamedAttackNamed, "", "NamedTarget"),
            (ScriptActionType::NamedFaceNamed, "NamedUnit", ""),
        ],
    );
    assert_eq!(world.host_object(unnamed).unwrap().formation_id, 43);
    assert_eq!(world.host_object(id).unwrap().formation_id, 19);
    assert_eq!(world.host_object(id).unwrap().target, None);
    let mut template = ThingTemplate::new("OwnedNamedCrate");
    template.add_kind_of(KindOf::Crate).set_health(100.0);
    world.templates.insert(template.name.clone(), template);
    let crate_id = world
        .create_object("OwnedNamedCrate", Team::USA, Vec3::ZERO)
        .unwrap();
    world.host_object_mut(crate_id).unwrap().name = "Crate".into();
    world
        .host_object_mut(crate_id)
        .unwrap()
        .set_formation(33, glam::Vec2::ZERO);
    assert!(
        !world
            .host_object(crate_id)
            .unwrap()
            .has_ai_update_interface()
    );
    execute(
        &mut world,
        &[(ScriptActionType::NamedFaceNamed, "Crate", "NamedTarget")],
    );
    assert_eq!(world.host_object(crate_id).unwrap().formation_id, 33);
    assert_eq!(world.host_object(target).unwrap().target, None);
}

#[test]
fn named_attack_then_face_preserves_action_order_and_snapshot_continuation() {
    let (mut source, id, target) = world();
    execute(
        &mut source,
        &[
            (
                ScriptActionType::NamedAttackNamed,
                "NamedUnit",
                "NamedTarget",
            ),
            (ScriptActionType::NamedFaceNamed, "NamedUnit", "NamedTarget"),
        ],
    );
    assert_eq!(
        source.host_object(id).unwrap().ai_state,
        AIState::FacingObject
    );
    assert!(
        source.host_object(id).unwrap().face_active,
        "save witness must start during an active turn"
    );
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let (mut restored, _, _) = world();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    let loaded = restored.host_object(id).unwrap();
    assert_eq!(loaded.ai_state, AIState::FacingObject);
    assert_eq!(loaded.target, Some(target));
    assert_eq!(loaded.face_goal_pos, None);
    assert!(loaded.face_active);
    assert_eq!(
        loaded.last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    // Continue the actual action walk on both the running and restored owner.
    for world in [&mut source, &mut restored] {
        execute(
            world,
            &[(
                ScriptActionType::NamedAttackNamed,
                "NamedUnit",
                "NamedTarget",
            )],
        );
        assert_eq!(world.host_object(id).unwrap().target, Some(target));
        assert_eq!(world.host_object(id).unwrap().ai_state, AIState::Attacking);
        assert_eq!(world.host_object(id).unwrap().max_shots_to_fire, -1);
    }
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn named_commands_ignore_foreign_core_member() {
    // Only this retained-singleton witness requires a bounded child process.
    super::sequential_actor_tests::isolated(
        module_path!(),
        "named_commands_ignore_foreign_core_member",
        || {
            let (mut world, id, _) = world();
            let mut core = gamelogic::object::Object::new_for_xfer_load(id.0, 100.0);
            let group = gamelogic::ai::group::AIGroup::new(91);
            core.enter_group(&group);
            let foreign = Arc::new(RwLock::new(core));
            gamelogic::object::registry::OBJECT_REGISTRY.register_object(id.0, &foreign);
            execute(
                &mut world,
                &[(
                    ScriptActionType::NamedAttackNamed,
                    "NamedUnit",
                    "NamedTarget",
                )],
            );
            assert_eq!(world.host_object(id).unwrap().formation_id, 0);
            execute(
                &mut world,
                &[(ScriptActionType::NamedFaceNamed, "NamedUnit", "NamedTarget")],
            );
            assert_eq!(
                world.host_object(id).unwrap().ai_state,
                AIState::FacingObject
            );
            execute(
                &mut world,
                &[(ScriptActionType::NamedHunt, "NamedUnit", "")],
            );
            assert!(world.host_object(id).unwrap().hunting);
            execute(
                &mut world,
                &[(ScriptActionType::NamedGuard, "NamedUnit", "")],
            );
            assert_eq!(
                world.host_object(id).unwrap().ai_state,
                AIState::GuardingArea
            );
            assert_eq!(
                world.host_object(id).unwrap().last_command_source,
                crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
            );
            assert_eq!(foreign.read().unwrap().get_group_id(), Some(91));
            assert!(Arc::ptr_eq(
                &foreign,
                &gamelogic::object::registry::OBJECT_REGISTRY
                    .get_object(id.0)
                    .unwrap()
            ));
        },
    );
}

#[test]
fn active_face_snapshot_calibration() {
    let (mut source, id, target) = world();
    assert!(source.private_face_object(id, target));
    let face = source.host_object(id).unwrap();
    assert!(face.face_active, "angled target requires an active turn");
    assert!(face.face_can_turn_in_place);
    assert_eq!(face.locomotor_goal_type, LocoGoalType::Angle);
}

fn face_projection(
    world: &GameLogic,
    id: ObjectId,
) -> (
    AIState,
    Option<ObjectId>,
    bool,
    bool,
    Option<Vec3>,
    u32,
    LocoGoalType,
    f32,
    f32,
) {
    let o = world.host_object(id).unwrap();
    (
        o.ai_state.clone(),
        o.target,
        o.face_active,
        o.face_can_turn_in_place,
        o.face_goal_pos,
        o.face_loco_frame,
        o.locomotor_goal_type,
        o.locomotor_goal_angle,
        o.get_orientation(),
    )
}

#[test]
fn active_face_snapshot_preserves_captured_mode_and_future_ai_steps() {
    // Existing API also runs on OLD: no command-driver change can explain
    // a missing saved continuation in this independently calibrated witness.
    let (mut source, id, target) = world();
    source.frame = 7;
    assert!(source.private_face_object(id, target));
    assert!(source.host_object(id).unwrap().face_active);
    source.update_ai(&[id], 1.0 / 30.0);
    assert!(source.host_object(id).unwrap().face_active);
    let saved = face_projection(&source, id);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let (mut restored, _, _) = world();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(
        face_projection(&restored, id),
        saved,
        "restore must keep the captured Face state, not re-enter it"
    );
    for world in [&mut source, &mut restored] {
        world.update_ai(&[id], 1.0 / 30.0);
        assert_eq!(
            face_projection(world, id).8,
            saved.8,
            "same frame must not turn twice"
        );
        world
            .host_object_mut(target)
            .unwrap()
            .set_position(Vec3::new(100.0, 0.0, 120.0));
    }
    let mut finished = false;
    for frame in 8..188 {
        for world in [&mut source, &mut restored] {
            world.frame = frame;
            world.update_ai(&[id], 1.0 / 30.0);
        }
        assert_eq!(
            face_projection(&restored, id),
            face_projection(&source, id),
            "future face differs at frame {frame}"
        );
        if source.host_object(id).unwrap().ai_state == AIState::Idle {
            finished = true;
            break;
        }
    }
    assert!(finished, "both worlds must finish the turn");
}

#[test]
fn face_position_snapshot_preserves_explicit_goal() {
    let (mut source, id, _) = world();
    let goal = Vec3::new(120.0, 0.0, 110.0);
    assert!(source.private_face_position(id, goal));
    assert!(source.host_object(id).unwrap().face_active);
    let saved = face_projection(&source, id);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let (mut restored, _, _) = world();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(face_projection(&restored, id), saved);
    for world in [&mut source, &mut restored] {
        world.frame = 1;
        world.update_ai(&[id], 1.0 / 30.0);
    }
    assert_eq!(face_projection(&restored, id), face_projection(&source, id));
}

#[test]
fn world_snapshot_capture_rejects_invalid_continuation_without_dropping_objects() {
    let (mut source, id, target) = world();
    source.frame = 9;
    source
        .host_object_mut(id)
        .unwrap()
        .set_ai_state(AIState::Panic);
    assert!(source.host_object(id).unwrap().panic_runtime.is_none());
    let count = source.host_objects().len();
    let result = crate::save_load::SnapshotBuilder::new().create_world_snapshot(&source);
    assert!(
        matches!(result, Err(crate::save_load::SaveLoadError::Corrupted(ref message))
        if message.contains("AI_PANIC")),
        "invalid owned continuation must fail capture, never save a world missing the object"
    );
    assert_eq!(source.host_objects().len(), count);
    assert_eq!(source.frame, 9);
    assert_eq!(source.host_object(id).unwrap().ai_state, AIState::Panic);
    assert_eq!(source.host_object(target).unwrap().health.current, 100.0);
}

#[test]
fn world_snapshot_capture_keeps_every_valid_owned_object() {
    let (source, _, _) = world();
    let snapshot = crate::save_load::SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    assert_eq!(snapshot.objects.len(), source.host_objects().len());
    for id in source.host_objects().keys() {
        assert!(snapshot.objects.contains_key(id));
    }
}

// New capability-specific witness; OLD controls use the existing engine API above.
#[test]
fn named_this_object_is_an_owned_id_and_self_face_is_borrow_safe() {
    use gamelogic::scripting::engine::{ScriptExecutionDriver, ScriptNamedCommand};
    let (mut world, id, target) = world();
    let token = gamelogic::scripting::core::THIS_OBJECT;
    {
        let mut driver = HostScriptExecutionDriver::new(&mut world);
        assert!(
            driver
                .named_command(
                    ScriptNamedCommand::FaceObject {
                        unit: token,
                        target: token
                    },
                    Some(id.0)
                )
                .unwrap()
                .is_ok()
        );
    }
    assert_eq!(world.host_object(id).unwrap().target, Some(id));
    assert_eq!(
        world.host_object(id).unwrap().ai_state,
        AIState::FacingObject
    );
    assert_eq!(world.host_object(target).unwrap().target, None);
    let old_formation = world.host_object(id).unwrap().formation_id;
    HostScriptExecutionDriver::new(&mut world)
        .named_command(
            ScriptNamedCommand::FaceObject {
                unit: token,
                target: "NamedTarget",
            },
            Some(u32::MAX),
        )
        .unwrap()
        .unwrap();
    assert_eq!(world.host_object(id).unwrap().formation_id, old_formation);
    assert_eq!(world.host_object(id).unwrap().target, Some(id));
}
