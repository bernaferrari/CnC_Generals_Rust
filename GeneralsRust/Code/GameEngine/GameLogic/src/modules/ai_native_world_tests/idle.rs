//! Receiving Idle dependencies execute against actual owners and partition data.
use super::*;
use crate::modules::ai_world::AiWorld;

fn admit_actual_search_partition(game: &mut NativeMatch) {
    // NativeMatch starts with only the source collision owner. Register the
    // target with its authored position/geometry; vision maintenance alone
    // updates influence data and does not create collision membership.
    let target = game.target.read().unwrap();
    let position = target.get_position();
    game.collision
        .register_object(
            target.get_id(),
            crate::object::collide::Coord3D::new(position.x, position.y, position.z),
            crate::object::collide::collision_geometry::GeometryInfo::new_sphere(
                target.get_geometry_info().get_bounding_circle_radius(),
                false,
            ),
            None,
        )
        .unwrap();
}

#[test]
fn idle_first_frame_restakes_then_uses_supplied_goal_pose_without_peer_mutation() {
    let mut game = super::containment_lifecycle::lifecycle_world(0.0, false);
    let peer = super::containment_lifecycle::lifecycle_world(0.0, false);
    game.operate(11, |world| {
        world.remove_contained(1, 2, false).unwrap();
    });
    let source = Arc::clone(&game.source);
    let peer_position = *peer.source.read().unwrap().get_position();
    let ai = source.read().unwrap().get_ai_update_interface().unwrap();
    let proxy = source
        .read()
        .unwrap()
        .named_update_registrations("AIUpdateInterface")
        .pop()
        .unwrap();
    game.operate_with_current(1, Some(proxy), |world| {
        world
            .set_object_position_handle(&source, &Coord3D::new(53.25, 54.75, 0.0))
            .unwrap();
        ai.lock()
            .unwrap()
            .update_locomotor_phase_with_world(world)
            .unwrap();
        // Re-enter Idle from an actual different state. The fixture's earlier
        // lifecycle tick has already consumed its first Idle initialization.
        ai.lock()
            .unwrap()
            .execute_command_with_world(
                &crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::Busy,
                    crate::ai::CommandSourceType::FromAi,
                ),
                world,
            )
            .unwrap();
        ai.lock()
            .unwrap()
            .execute_command_with_world(
                &crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::Idle,
                    crate::ai::CommandSourceType::FromAi,
                ),
                world,
            )
            .unwrap();
        ai.lock()
            .unwrap()
            .update_idle_phase_sleep_with_world(world)
            .unwrap();
        let goal_cell = ai
            .lock()
            .unwrap()
            .unit_ai_for_test()
            .unwrap()
            .native_waypoint_observation_for_test()
            .pathfind_goal_cell;
        let source = source.read().unwrap();
        let goal = world
            .idle_goal_position(&source, goal_cell)
            .unwrap()
            .expect("Idle must stake the actual owner before goal snapping");
        assert_eq!(*source.get_position(), goal);
    });
    assert_eq!(*peer.source.read().unwrap().get_position(), peer_position);
}

#[test]
fn repulsor_query_reads_actual_rule_and_partition_owner_without_peer_selection() {
    let mut game = NativeMatch::new(90.0);
    let mut peer = NativeMatch::new(90.0);
    admit_actual_search_partition(&mut game);
    admit_actual_search_partition(&mut peer);
    game.rules.enable_repulsors = true;
    peer.rules.enable_repulsors = false;
    game.target
        .write()
        .unwrap()
        .set_status(crate::common::ObjectStatusMaskType::REPULSOR, true);
    peer.target
        .write()
        .unwrap()
        .set_status(crate::common::ObjectStatusMaskType::REPULSOR, true);
    let source = Arc::clone(&game.source);
    let peer_source = Arc::clone(&peer.source);
    game.operate(31, |world| {
        assert_eq!(
            world
                .closest_repulsor(&source.read().unwrap(), 100.0)
                .unwrap(),
            Some(2)
        );
        assert_eq!(
            world
                .closest_repulsor(&source.read().unwrap(), 2.0)
                .unwrap(),
            None
        );
    });
    peer.operate(31, |world| {
        assert_eq!(
            world
                .closest_repulsor(&peer_source.read().unwrap(), 100.0)
                .unwrap(),
            None
        );
    });
}

#[test]
fn ordered_enemy_query_obeys_actual_relationship_and_range() {
    let mut hostile = NativeMatch::with_target_owner(90.0, container_template(), true);
    let mut allied = NativeMatch::with_target_owner(90.0, container_template(), false);
    admit_actual_search_partition(&mut hostile);
    admit_actual_search_partition(&mut allied);
    let source = Arc::clone(&hostile.source);
    let peer_source = Arc::clone(&allied.source);
    hostile.operate(41, |world| {
        let source = source.read().unwrap();
        assert_eq!(
            world
                .closest_mood_enemy(
                    &source,
                    100.0,
                    crate::ai::search_qualifiers::ATTACK_BUILDINGS
                )
                .unwrap(),
            Some(2)
        );
        assert_eq!(
            world
                .closest_mood_enemy(&source, 2.0, crate::ai::search_qualifiers::ATTACK_BUILDINGS)
                .unwrap(),
            None
        );
    });
    allied.operate(41, |world| {
        assert_eq!(
            world
                .closest_mood_enemy(
                    &peer_source.read().unwrap(),
                    100.0,
                    crate::ai::search_qualifiers::ATTACK_BUILDINGS
                )
                .unwrap(),
            None
        );
    });
}

#[test]
fn mood_priority_reads_receiving_script_definitions_without_peer_override() {
    let mut game = NativeMatch::with_target_owner(90.0, container_template(), true);
    let mut peer = NativeMatch::with_target_owner(90.0, container_template(), true);
    admit_actual_search_partition(&mut game);
    admit_actual_search_partition(&mut peer);
    let name = game
        .target
        .read()
        .unwrap()
        .get_template()
        .get_name()
        .to_string();
    let scripts = &game.disposal.scripts;
    assert!(
        scripts
            .set_priority_thing_with_lookup("OwnedIdlePriority", &name, 0, |query| (query == name)
                .then(|| name.clone()))
    );
    scripts.set_object_attack_priority_set(1, "OwnedIdlePriority");
    let source = Arc::clone(&game.source);
    let peer_source = Arc::clone(&peer.source);
    game.operate(43, |world| {
        assert_eq!(
            world
                .closest_mood_enemy(
                    &source.read().unwrap(),
                    100.0,
                    crate::ai::search_qualifiers::ATTACK_BUILDINGS
                )
                .unwrap(),
            None
        );
    });
    peer.operate(43, |world| {
        assert_eq!(
            world
                .closest_mood_enemy(
                    &peer_source.read().unwrap(),
                    100.0,
                    crate::ai::search_qualifiers::ATTACK_BUILDINGS
                )
                .unwrap(),
            Some(2)
        );
    });
    assert!(game.disposal.scripts.set_priority_thing_with_lookup(
        "OwnedIdlePriority",
        &name,
        7,
        |query| (query == name).then(|| name.clone())
    ));
    game.operate(44, |world| {
        assert_eq!(
            world
                .closest_mood_enemy(
                    &source.read().unwrap(),
                    100.0,
                    crate::ai::search_qualifiers::ATTACK_BUILDINGS
                )
                .unwrap(),
            Some(2)
        );
    });
}

#[test]
fn mood_range_uses_receiving_player_and_attitude_rules() {
    let mut game = NativeMatch::new(90.0);
    let mut peer = NativeMatch::new(90.0);
    game.rules.guard_outer_modifier_ai = 2.0;
    game.rules.guard_outer_modifier_human = 3.0;
    game.rules.aggressive_range_modifier = 4.0;
    peer.rules.guard_outer_modifier_ai = 5.0;
    peer.rules.guard_outer_modifier_human = 6.0;
    peer.rules.aggressive_range_modifier = 7.0;
    game.source.write().unwrap().set_vision_range(10.0);
    peer.source.write().unwrap().set_vision_range(10.0);
    let source = Arc::clone(&game.source);
    let peer_source = Arc::clone(&peer.source);
    game.operate(45, |world| {
        let source = source.read().unwrap();
        let human = world.controller_is_human(&source) == Some(true);
        assert_eq!(
            world
                .mood_vision_range(&source, crate::modules::AIAttitudeType::Aggressive)
                .unwrap(),
            if human { 30.0 } else { 80.0 }
        );
    });
    peer.operate(45, |world| {
        let source = peer_source.read().unwrap();
        let human = world.controller_is_human(&source) == Some(true);
        assert_eq!(
            world
                .mood_vision_range(&source, crate::modules::AIAttitudeType::Aggressive)
                .unwrap(),
            if human { 60.0 } else { 350.0 }
        );
    });
}
