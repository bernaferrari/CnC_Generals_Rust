//! Script bridge queries observe the driving world's existing terrain latch.
#![cfg(not(target_arch = "wasm32"))]
use super::*;
use crate::save_load::SnapshotBuilder;
use gamelogic::common::{BodyDamageType, Coord3D};
use gamelogic::path::PathfindLayerEnum;
use gamelogic::terrain::BridgeInfo;

fn isolated(name: &str, run: impl FnOnce()) {
    super::sequential_actor_tests::isolated(module_path!(), name, run);
}

fn attach_bridge(world: &mut GameLogic, id: ObjectId, state: BodyDamageType) {
    let mut info = BridgeInfo::new();
    info.bridge_object_id = id.0;
    info.cur_damage_state = state;
    info.from_left = Coord3D::new(10.0, 10.0, 5.0);
    info.from_right = Coord3D::new(10.0, 30.0, 5.0);
    info.to_left = Coord3D::new(70.0, 10.0, 5.0);
    info.to_right = Coord3D::new(70.0, 30.0, 5.0);
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .prepend_bridge_on_layer(info, "ScriptOwnerBridge".into(), PathfindLayerEnum::Ground);
}

fn world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "BridgeOwner", false));
    let mut template = ThingTemplate::new("ScriptOwnerBridge");
    template.add_kind_of(KindOf::Bridge).set_health(100.0);
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object_for_player("ScriptOwnerBridge", 1, Vec3::new(40.0, 5.0, 20.0))
        .unwrap();
    world.host_object_mut(id).unwrap().name = "Crossing".into();
    attach_bridge(&mut world, id, BodyDamageType::Pristine);
    (world, id)
}

fn change(world: &mut GameLogic, id: ObjectId, state: BodyDamageType) {
    let object = world.host_object_mut(id).unwrap();
    object.health.current = if state == BodyDamageType::Rubble {
        0.0
    } else {
        100.0
    };
    world.sync_owned_bridge_body_state(id, state);
}

fn query(world: &GameLogic) -> (bool, bool) {
    // Exercise the actual production snapshot admission plus condition reads.
    world.inject_host_script_query_snapshot();
    (
        gamelogic::scripting::host_bridge_broken("Crossing"),
        gamelogic::scripting::host_bridge_repaired("Crossing"),
    )
}

#[test]
fn bridge_queries_are_instance_owned_and_do_not_consume_transitions() {
    isolated(
        "bridge_queries_are_instance_owned_and_do_not_consume_transitions",
        || {
            let (mut first, id) = world();
            let (mut second, other_id) = world();
            assert_eq!(id, other_id);
            assert_eq!(query(&first), (false, false));
            change(&mut second, other_id, BodyDamageType::Rubble);
            assert_eq!(query(&second), (true, false));
            assert_eq!(
                query(&first),
                (false, false),
                "foreign same-name bridge is not a repair"
            );
            change(&mut first, id, BodyDamageType::Rubble);
            assert_eq!(query(&first), (true, false));
            assert_eq!(
                query(&first),
                (true, false),
                "script queries do not consume a terrain event"
            );
            second.update_owned_water();
            assert_eq!(query(&second), (false, false));
            assert_eq!(
                query(&first),
                (true, false),
                "foreign terrain phase cannot clear our event"
            );
        },
    );
}

#[test]
fn bridge_conditions_require_rubble_transition_and_clear_in_terrain_phase() {
    isolated(
        "bridge_conditions_require_rubble_transition_and_clear_in_terrain_phase",
        || {
            let (mut world, id) = world();
            change(&mut world, id, BodyDamageType::Damaged);
            assert_eq!(
                query(&world),
                (false, false),
                "ordinary damage is not repair"
            );
            change(&mut world, id, BodyDamageType::Rubble);
            assert_eq!(query(&world), (true, false));
            world.update_owned_water();
            assert_eq!(query(&world), (false, false));
            change(&mut world, id, BodyDamageType::Rubble);
            assert_eq!(
                query(&world),
                (false, false),
                "no-op sync cannot relatch break"
            );
            change(&mut world, id, BodyDamageType::Pristine);
            assert_eq!(query(&world), (false, true));
            assert_eq!(query(&world), (false, true));
            world.update_owned_water();
            assert_eq!(query(&world), (false, false));
        },
    );
}

#[test]
fn one_bridge_damage_update_does_not_erase_another_bridge_break() {
    isolated(
        "one_bridge_damage_update_does_not_erase_another_bridge_break",
        || {
            let (mut world, first_id) = world();
            let second_id = world
                .create_object_for_player("ScriptOwnerBridge", 1, Vec3::new(120.0, 5.0, 20.0))
                .unwrap();
            world.host_object_mut(second_id).unwrap().name = "OtherCrossing".into();
            attach_bridge(&mut world, second_id, BodyDamageType::Pristine);
            change(&mut world, first_id, BodyDamageType::Rubble);
            change(&mut world, second_id, BodyDamageType::Damaged);
            assert_eq!(query(&world), (true, false));
            assert!(!gamelogic::scripting::host_bridge_broken("OtherCrossing"));
            assert!(!gamelogic::scripting::host_bridge_repaired("OtherCrossing"));
            world.update_owned_water();
            assert_eq!(query(&world), (false, false));
            assert!(!gamelogic::scripting::host_bridge_broken("OtherCrossing"));
            assert!(!gamelogic::scripting::host_bridge_repaired("OtherCrossing"));
        },
    );
}

#[test]
fn bridge_query_history_does_not_leak_through_reset_or_detached_restore() {
    isolated(
        "bridge_query_history_does_not_leak_through_reset_or_detached_restore",
        || {
            let (mut running, id) = world();
            change(&mut running, id, BodyDamageType::Rubble);
            assert_eq!(query(&running), (true, false));
            let builder = SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&running).unwrap();
            let mut candidate = GameLogic::new();
            candidate.templates = running.templates.clone();
            builder
                .restore_from_snapshot(&snapshot, &mut candidate)
                .unwrap();
            // Map geometry admission/restoration supplies canonical bridge records;
            // a read cannot invent a transition from the restored health value.
            attach_bridge(&mut candidate, id, BodyDamageType::Rubble);
            assert_eq!(query(&candidate), (false, false));
            assert_eq!(
                query(&running),
                (true, false),
                "load candidate never replaces the running latch"
            );
            candidate.reset();
            assert_eq!(query(&candidate), (false, false));
            assert_eq!(query(&running), (true, false));
            drop(candidate);
            assert_eq!(query(&running), (true, false));
            running.reset();
            assert_eq!(query(&running), (false, false));
            assert!(
                !running
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .bridge_damage_states_changed()
            );
        },
    );
}

fn execute_bridge_condition(world: &mut GameLogic, name: &str, repaired: bool) -> bool {
    use super::script_execution_driver::HostScriptExecutionDriver;
    use gamelogic::scripting::core::{
        Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
        ScriptActionType, ScriptList,
    };
    use gamelogic::scripting::engine::ScriptEngine;

    let mut condition = Condition::new(if repaired {
        ConditionType::BridgeRepaired
    } else {
        ConditionType::BridgeBroken
    });
    condition
        .add_parameter(Parameter::with_string(ParameterType::Unit, name.into()))
        .unwrap();
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(condition)));
    let mut action = ScriptAction::new(ScriptActionType::SetCounter);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Counter,
            "BridgeObserved".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    let mut script = Script::new();
    script.script_name = "ObserveOwnedBridge".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_counter("BridgeObserved", 0).unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    let mut context = gamelogic::scripting::executor::ScriptContext::new();
    context.current_frame = world.frame;
    engine
        .update_with_driver(context, &mut HostScriptExecutionDriver::new(world))
        .unwrap();
    engine.get_counter("BridgeObserved").unwrap().value == 1
}

#[test]
fn real_engine_bridge_conditions_read_the_driving_world_instead_of_foreign_snapshot() {
    isolated(
        "real_engine_bridge_conditions_read_the_driving_world_instead_of_foreign_snapshot",
        || {
            let (mut first, id) = world();
            let (mut second, other_id) = world();
            assert_eq!(id, other_id);
            change(&mut second, other_id, BodyDamageType::Rubble);
            second.inject_host_script_query_snapshot();
            assert!(gamelogic::scripting::host_bridge_broken("Crossing"));
            assert!(!execute_bridge_condition(&mut first, "Crossing", false));
            assert!(execute_bridge_condition(&mut second, "Crossing", false));

            // A published false is equally stale when this owner has changed.
            first.inject_host_script_query_snapshot();
            assert!(!gamelogic::scripting::host_bridge_broken("Crossing"));
            assert!(execute_bridge_condition(&mut second, "Crossing", false));
            change(&mut second, other_id, BodyDamageType::Pristine);
            assert!(execute_bridge_condition(&mut second, "Crossing", true));
            assert!(!execute_bridge_condition(&mut first, "Crossing", true));
            first.reset();
            drop(first);
            assert!(execute_bridge_condition(&mut second, "Crossing", true));
        },
    );
}

#[test]
fn real_engine_missing_bridge_is_authoritative_even_when_snapshot_has_same_id() {
    isolated(
        "real_engine_missing_bridge_is_authoritative_even_when_snapshot_has_same_id",
        || {
            let (mut first, id) = world();
            let (mut second, other_id) = world();
            assert_eq!(id, other_id);
            first.host_object_mut(id).unwrap().name = "LocalCrossing".into();
            change(&mut second, other_id, BodyDamageType::Rubble);
            second.inject_host_script_query_snapshot();
            assert!(gamelogic::scripting::host_bridge_broken("Crossing"));
            assert!(!execute_bridge_condition(&mut first, "Crossing", false));
            change(&mut second, other_id, BodyDamageType::Pristine);
            second.inject_host_script_query_snapshot();
            assert!(gamelogic::scripting::host_bridge_repaired("Crossing"));
            assert!(!execute_bridge_condition(&mut first, "Crossing", true));
        },
    );
}

#[test]
fn real_engine_bridge_conditions_apply_frame_gate_without_consuming_record_flags() {
    isolated(
        "real_engine_bridge_conditions_apply_frame_gate_without_consuming_record_flags",
        || {
            let (mut world, id) = world();
            change(&mut world, id, BodyDamageType::Rubble);
            assert!(execute_bridge_condition(&mut world, "Crossing", false));
            assert!(execute_bridge_condition(&mut world, "Crossing", false));
            world.update_owned_water();
            {
                let terrain = world.world_services.terrain().read().unwrap();
                assert!(!terrain.bridge_damage_states_changed());
                assert!(
                    terrain.is_bridge_broken(id.0),
                    "CPP TerrainLogic::update1007 clears only the aggregate gate"
                );
            }
            assert!(!execute_bridge_condition(&mut world, "Crossing", false));
            change(&mut world, id, BodyDamageType::Pristine);
            assert!(execute_bridge_condition(&mut world, "Crossing", true));
            assert!(execute_bridge_condition(&mut world, "Crossing", true));
            world.update_owned_water();
            assert!(
                world
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .is_bridge_repaired(id.0)
            );
            assert!(!execute_bridge_condition(&mut world, "Crossing", true));
        },
    );
}

#[test]
fn named_damage_completes_bridge_callbacks_before_subroutine_condition() {
    isolated(
        "named_damage_completes_bridge_callbacks_before_subroutine_condition",
        || {
            use super::script_execution_driver::HostScriptExecutionDriver;
            use gamelogic::scripting::core::{
                Condition, ConditionType, OrCondition, Parameter, ParameterType, Script,
                ScriptAction, ScriptActionType, ScriptList,
            };
            use gamelogic::scripting::engine::ScriptEngine;
            let (mut world, id) = world();
            world.bridge_behavior.register_span(
                id,
                Vec3::new(10.0, 5.0, 10.0),
                Vec3::new(10.0, 5.0, 30.0),
                Vec3::new(70.0, 5.0, 10.0),
                Vec3::new(70.0, 5.0, 30.0),
            );
            // Deliberately admit the pre-action false snapshot. The subroutine
            // must read the body callback's completed live terrain scan instead.
            world.inject_host_script_query_snapshot();
            let mut condition = Condition::new(ConditionType::BridgeBroken);
            condition
                .add_parameter(Parameter::with_string(
                    ParameterType::Unit,
                    "Crossing".into(),
                ))
                .unwrap();
            let mut branch = OrCondition::new();
            branch.set_first_and_condition(Some(Box::new(condition)));
            let mut observed = ScriptAction::new(ScriptActionType::SetCounter);
            observed
                .add_parameter(Parameter::with_string(
                    ParameterType::Counter,
                    "ImmediateBridge".into(),
                ))
                .unwrap();
            observed
                .add_parameter(Parameter::with_int(ParameterType::Int, 1))
                .unwrap();
            let mut subroutine = Script::new();
            subroutine.script_name = "ObserveBreak".into();
            subroutine.is_subroutine = true;
            subroutine.condition = Some(Box::new(branch));
            subroutine.action = Some(Box::new(observed));
            let mut call = ScriptAction::new(ScriptActionType::CallSubroutine);
            call.add_parameter(Parameter::with_string(
                ParameterType::Script,
                "ObserveBreak".into(),
            ))
            .unwrap();
            let mut damage = ScriptAction::new(ScriptActionType::NamedDamage);
            damage
                .add_parameter(Parameter::with_string(
                    ParameterType::Unit,
                    "Crossing".into(),
                ))
                .unwrap();
            damage
                .add_parameter(Parameter::with_int(ParameterType::Int, 100))
                .unwrap();
            damage.next_action = Some(Box::new(call));
            let mut always = OrCondition::new();
            always.set_first_and_condition(Some(Box::new(Condition::new(
                ConditionType::ConditionTrue,
            ))));
            let mut outer = Script::new();
            outer.script_name = "BreakThenObserve".into();
            outer.is_one_shot = true;
            outer.condition = Some(Box::new(always));
            outer.action = Some(Box::new(damage));
            let mut list = ScriptList::new();
            list.append_script(Box::new(outer));
            list.append_script(Box::new(subroutine));
            let mut engine = ScriptEngine::new().unwrap();
            engine.set_counter("ImmediateBridge", 0).unwrap();
            engine
                .set_script_list_for_player(0, Some(Box::new(list)))
                .unwrap();
            let foreign = std::sync::Arc::new(std::sync::RwLock::new(
                gamelogic::object::Object::new_for_xfer_load(id.0, 917.0),
            ));
            gamelogic::object::registry::OBJECT_REGISTRY.register_object(id.0, &foreign);
            let mut context = gamelogic::scripting::executor::ScriptContext::new();
            context.current_frame = world.frame;
            engine
                .update_with_driver(context, &mut HostScriptExecutionDriver::new(&mut world))
                .unwrap();
            gamelogic::object::registry::OBJECT_REGISTRY.unregister_object(id.0);
            assert_eq!(
                world.objects[&id].health.current, 0.0,
                "NAMED_DAMAGE really executes"
            );
            assert_eq!(
                engine.get_counter("ImmediateBridge").unwrap().value,
                1,
                "BRIDGE_BROKEN sees the synchronous callback before script continuation"
            );
        },
    );
}
