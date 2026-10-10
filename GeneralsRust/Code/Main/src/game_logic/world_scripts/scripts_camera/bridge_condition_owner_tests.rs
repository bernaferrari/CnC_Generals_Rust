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
