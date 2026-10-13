//! Actual cached AIExit and original-point Transport callback continuation.
use super::*;
use crate::common::ObjectID;
use crate::modules::ai_world::AiWorld;
fn boarded() -> NativeMatch {
    boarded_with_target_ai(false)
}
fn boarded_with_target_ai(target_ai: bool) -> NativeMatch {
    boarded_with_modules(target_ai, false)
}
fn boarded_with_modules(target_ai: bool, physics: bool) -> NativeMatch {
    let template = if target_ai {
        let original = transport_template();
        let mut base = DefaultThingTemplate::new("NativeExitAiTransport".into());
        base.add_kind_of(KindOf::Structure);
        let mut modules = original.get_behavior_module_info().to_vec();
        modules.push(TemplateModuleInfo {
            name: "AIUpdateInterface".into(),
            module_tag: "NativeExitTransportAI".into(),
            data: Arc::new(crate::object::update::AIUpdateModuleData::default()),
            interface_mask: ModuleInterfaceType::UPDATE,
        });
        Arc::new(EntryTemplate { base, modules }) as Arc<dyn ThingTemplate>
    } else {
        transport_template()
    };
    let mut game = if physics {
        let original = infantry_template();
        let mut modules = original.get_behavior_module_info().to_vec();
        modules.push(TemplateModuleInfo {
            name: "PhysicsBehavior".into(),
            module_tag: "OwnedCargoPhysics".into(),
            data: Arc::new(
                crate::object::behavior::physics_update::PhysicsBehaviorModuleData::default(),
            ),
            interface_mask: ModuleInterfaceType::UPDATE,
        });
        let mut base = DefaultThingTemplate::new("OwnedPhysicalCargo".into());
        base.add_kind_of(KindOf::Infantry);
        NativeMatch::with_source_template(
            50.0,
            template,
            false,
            Arc::new(EntryTemplate { base, modules }),
        )
    } else {
        NativeMatch::with_container(50.0, template)
    };
    game.issue_enter(31);
    let ai = game
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    for frame in 31..35 {
        game.operate(frame, |world| {
            world.process_enter_path_queue(8).unwrap();
            if ai.lock().unwrap().get_current_state_id()
                == Some(crate::ai::states::AIStateType::Enter as u32)
            {
                ai.lock()
                    .unwrap()
                    .update_enter_phase_with_world(world)
                    .unwrap();
            }
        });
        if game.source.read().unwrap().get_contained_by().is_some() {
            break;
        }
    }
    assert_eq!(
        game.source.read().unwrap().get_contained_by(),
        Some(game.target.read().unwrap().get_id())
    );
    game
}
#[test]
fn owned_regular_exit_releases_ai_before_removal_then_completes_same_phase() {
    let mut game = boarded();
    let peer = boarded();
    let id = game.source.read().unwrap().get_id();
    let ai = game
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    let _peer_source = peer.source.write().unwrap();
    let _peer_target = peer.target.write().unwrap();
    game.operate(41, |world| {
        world
            .execute_exit_command(id, false, CommandSourceType::FromScript)
            .unwrap()
    });
    assert!(game.source.read().unwrap().get_contained_by().is_some());
    assert_eq!(
        ai.lock().unwrap().get_current_state_id(),
        Some(crate::ai::states::AIStateType::Exit as u32)
    );
    {
        let mut actor = ai.lock().unwrap();
        actor
            .unit_ai_for_test()
            .unwrap()
            .set_queue_deadline_for_test(1234);
    }
    game.operate(42, |world| world.update_exit_phase(id).unwrap());
    assert_eq!(
        ai.lock()
            .unwrap()
            .unit_ai_for_test()
            .unwrap()
            .queue_deadline_for_test(),
        0,
        "the terminal state-change callback clears the actor's saved queue deadline"
    );
    assert_eq!(game.source.read().unwrap().get_contained_by(), None);
    assert!(
        !game
            .source
            .read()
            .unwrap()
            .is_disabled_by_type(crate::common::DisabledType::Held)
    );
    assert_eq!(
        game.target
            .read()
            .unwrap()
            .get_contain()
            .unwrap()
            .lock()
            .unwrap()
            .get_contained_count(),
        0
    );
    assert_eq!(
        ai.lock().unwrap().get_current_state_id(),
        Some(crate::ai::states::AIStateType::Idle as u32)
    );
}
#[test]
fn owned_instant_exit_removes_before_command_return_and_completes_next_update() {
    let mut game = boarded();
    let id = game.source.read().unwrap().get_id();
    let ai = game
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    game.operate(51, |world| {
        world
            .execute_exit_command(id, true, CommandSourceType::FromScript)
            .unwrap()
    });
    assert_eq!(game.source.read().unwrap().get_contained_by(), None);
    assert_eq!(
        ai.lock().unwrap().get_current_state_id(),
        Some(crate::ai::states::AIStateType::ExitInstantly as u32)
    );
    game.operate(52, |world| world.update_exit_phase(id).unwrap());
    assert_eq!(
        ai.lock().unwrap().get_current_state_id(),
        Some(crate::ai::states::AIStateType::Idle as u32)
    );
}

#[test]
fn owned_exit_observes_actual_container_ai_before_removing_regular_or_instant() {
    for instant in [false, true] {
        let mut game = boarded_with_target_ai(true);
        let peer = boarded_with_target_ai(true);
        let id = game.source.read().unwrap().get_id();
        let target_ai = game
            .target
            .read()
            .unwrap()
            .get_ai_update_interface()
            .unwrap();
        let peer_target_ai = peer
            .target
            .read()
            .unwrap()
            .get_ai_update_interface()
            .unwrap();
        let target_state = target_ai.lock().unwrap().get_current_state_id();
        let _peer_ai = peer_target_ai.lock().unwrap();
        let _peer_source = peer.source.write().unwrap();
        let _peer_target = peer.target.write().unwrap();
        game.operate(81, |world| {
            assert_eq!(
                world
                    .exit_free_to_exit(id, game_target_id(world, id))
                    .unwrap(),
                crate::object::production::AIFreeToExitType::FreeToExit
            );
            world
                .execute_exit_command(id, instant, CommandSourceType::FromScript)
                .unwrap();
        });
        if !instant {
            assert!(game.source.read().unwrap().get_contained_by().is_some());
        }
        game.operate(82, |world| world.update_exit_phase(id).unwrap());
        assert_eq!(game.source.read().unwrap().get_contained_by(), None);
        assert_eq!(
            game.target
                .read()
                .unwrap()
                .get_contain()
                .unwrap()
                .lock()
                .unwrap()
                .get_contained_count(),
            0
        );
        assert_eq!(
            target_ai.lock().unwrap().get_current_state_id(),
            target_state,
            "read-only goal AI callback does not advance its FSM"
        );
    }
}
fn game_target_id(world: &dyn crate::modules::ai_world::AiWorld, rider: ObjectID) -> ObjectID {
    world
        .object(rider)
        .unwrap()
        .read()
        .unwrap()
        .get_contained_by()
        .unwrap()
}

#[test]
fn numbered_exit_uses_actual_pristine_bones_and_keeps_follow_path_until_done() {
    numbered_exit_control(false, false, false);
}

#[test]
fn numbered_instant_exit_uses_original_door_path_before_command_returns() {
    numbered_exit_control(true, false, false);
}

#[test]
fn numbered_hurry_exit_keeps_parent_and_runs_temporary_production_path() {
    numbered_exit_control(false, true, false);
}

#[test]
fn numbered_vehicle_exit_refreshes_idle_owner_occupancy_and_finishes_rider_path() {
    numbered_exit_control(false, false, true);
}

fn numbered_exit_control(instant: bool, hurry: bool, vehicle: bool) {
    use crate::common::{Matrix3D, ModelConditionFlags};
    use crate::object::draw::w3d_model_draw::{
        ModelConditionInfo, PristineBoneInfo, W3DModelDraw, W3DModelDrawModuleData,
    };
    use crate::object::drawable::{Drawable, DrawableExt, DrawableType};
    use game_engine::common::name_key_generator::NameKeyGenerator;
    let mut base = DefaultThingTemplate::new("NativeNumberedExit".into());
    base.add_kind_of(if vehicle {
        KindOf::Vehicle
    } else {
        KindOf::Structure
    });
    let data = crate::object::contain::transport_contain::TransportContainModuleData {
        slot_capacity: 2,
        base: crate::object::contain::open_contain::OpenContainModuleData {
            contain_max: 2,
            number_of_exit_paths: 2,
            door_open_time: 9,
            allow_inside_kind_of: KindOf::Infantry.cpp_mask(),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut target_modules = vec![TemplateModuleInfo {
        name: "TransportContain".into(),
        module_tag: "NumberedCargo".into(),
        data: Arc::new(crate::contain_module_overrides::ContainModuleDataAdapter::new(data)),
        interface_mask: ModuleInterfaceType::CONTAIN,
    }];
    if vehicle {
        target_modules.extend_from_slice(infantry_template().get_behavior_module_info());
    }
    let target = Arc::new(EntryTemplate {
        base,
        modules: target_modules,
    });
    let original = infantry_template();
    let mut modules = original.get_behavior_module_info().to_vec();
    let mut body = crate::object::body::active_body::ActiveBodyModuleData::default();
    body.max_health = 100.0;
    body.initial_health = 100.0;
    modules.push(TemplateModuleInfo {
        name: "ActiveBody".into(),
        module_tag: "ExitBody".into(),
        data: Arc::new(body),
        interface_mask: ModuleInterfaceType::BODY,
    });
    modules.push(TemplateModuleInfo {
        name: "PhysicsBehavior".into(),
        module_tag: "ExitPhysics".into(),
        data: Arc::new(
            crate::object::behavior::physics_update::PhysicsBehaviorModuleData::default(),
        ),
        interface_mask: ModuleInterfaceType::UPDATE,
    });
    let mut base = DefaultThingTemplate::new("NumberedExitInfantry".into());
    base.add_kind_of(KindOf::Infantry);
    let source = Arc::new(EntryTemplate { base, modules });
    let mut game = NativeMatch::with_source_template(50.0, target, false, source);
    Object::admit_update_schedule_from_handle(&game.source, &mut game.scheduler).unwrap();
    let id = game.source.read().unwrap().get_id();
    let owner_id = game.target.read().unwrap().get_id();
    let mut draw_data = W3DModelDrawModuleData::new();
    draw_data.default_state = 0;
    let mut state = ModelConditionInfo::new();
    state.conditions_yes.push(ModelConditionFlags::empty());
    for (name, point) in [
        ("exitstart01", Coord3D::new(-20.0, 0.0, 0.0)),
        ("exitend01", Coord3D::new(50.0, 0.0, 0.0)),
        ("exitstart02", Coord3D::new(-10.0, 0.0, 0.0)),
        ("exitend02", Coord3D::new(60.0, 0.0, 0.0)),
    ] {
        state.pristine_bones.insert(
            NameKeyGenerator::name_to_key(name),
            PristineBoneInfo {
                transform: Matrix3D::from_translation(point),
                bone_index: 1,
            },
        );
    }
    draw_data.condition_states.push(state);
    let mut drawable = Drawable::new(
        owner_id,
        owner_id,
        "NumberedTransport".into(),
        DrawableType::Static,
    );
    drawable.add_module(
        ModuleInterfaceType::DRAW,
        "W3DModelDraw".into(),
        "ExitBones".into(),
        Arc::new(draw_data.clone()),
        Box::new(W3DModelDraw::new(draw_data)),
    );
    game.target
        .write()
        .unwrap()
        .set_drawable(Some(Arc::new(RwLock::new(drawable))));
    assert!(
        game.target
            .read()
            .unwrap()
            .get_single_logical_bone_position("ExitStart01")
            .0
    );
    game.issue_enter(31);
    let ai = game
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    for frame in 31..=35 {
        game.operate(frame, |world| {
            world.process_enter_path_queue(8).unwrap();
            if ai.lock().unwrap().get_current_state_id()
                == Some(crate::ai::states::AIStateType::Enter as u32)
            {
                ai.lock()
                    .unwrap()
                    .update_enter_phase_with_world(world)
                    .unwrap();
            }
        });
        if game.source.read().unwrap().get_contained_by().is_some() {
            break;
        }
    }
    assert_eq!(
        game.source.read().unwrap().get_contained_by(),
        Some(owner_id)
    );
    let parent_before = ai.lock().unwrap().get_current_state_id();
    game.operate(41, |world| {
        if hurry {
            world.execute_hurry_exit(id).unwrap();
        } else {
            world
                .execute_exit_command(id, instant, CommandSourceType::FromScript)
                .unwrap();
        }
    });
    if !instant && !hurry {
        game.operate(42, |world| world.update_exit_phase(id).unwrap());
    }
    assert_eq!(game.source.read().unwrap().get_contained_by(), None);
    assert_eq!(
        *game.source.read().unwrap().get_position(),
        Coord3D::new(30.0, 50.0, 0.0)
    );
    assert_eq!(
        ai.lock().unwrap().get_current_state_id(),
        if hurry {
            parent_before
        } else {
            Some(crate::ai::states::AIStateType::FollowPath as u32)
        },
        "ordinary exit replaces the parent; hurry preserves it beneath the temporary path"
    );
    let owner = Arc::clone(&game.source);
    let mut hurry_trace = Vec::new();
    for frame in 43..=1500 {
        let ai_proxy = owner
            .read()
            .unwrap()
            .named_update_registrations("AIUpdateInterface")
            .pop()
            .unwrap();
        game.operate_with_current(frame, Some(ai_proxy), |world| {
            world.process_enter_path_queue(8).unwrap();
            if hurry {
                let before = ai
                    .lock()
                    .unwrap()
                    .unit_ai_for_test()
                    .unwrap()
                    .native_waypoint_observation_for_test();
                ai.lock()
                    .unwrap()
                    .update_scheduled_tick_with_world(world)
                    .unwrap();
                let after = ai
                    .lock()
                    .unwrap()
                    .unit_ai_for_test()
                    .unwrap()
                    .native_waypoint_observation_for_test();
                if frame <= 47 || before.temporary_state != after.temporary_state {
                    hurry_trace.push(format!(
                        "frame {frame}, pose {:?}, before {before:?}, after {after:?}",
                        owner.read().unwrap().get_position()
                    ));
                }
            } else if ai.lock().unwrap().get_current_state_id()
                == Some(crate::ai::states::AIStateType::FollowPath as u32)
            {
                ai.lock()
                    .unwrap()
                    .update_follow_path_phase_with_world(world)
                    .unwrap();
                ai.lock()
                    .unwrap()
                    .update_locomotor_phase_with_world(world)
                    .unwrap();
            }
        });
        let proxy = owner
            .read()
            .unwrap()
            .named_update_registrations("PhysicsBehavior")
            .pop()
            .unwrap();
        game.operate_with_current(frame, Some(proxy), |world| {
            world.update_physics_phase(&owner).unwrap()
        });
        if (hurry && game.source.read().unwrap().get_position().x >= 95.0)
            || (!hurry
                && ai.lock().unwrap().get_current_state_id()
                    == Some(crate::ai::states::AIStateType::Idle as u32))
        {
            break;
        }
    }
    assert_eq!(
        ai.lock().unwrap().get_current_state_id(),
        if hurry {
            parent_before
        } else {
            Some(crate::ai::states::AIStateType::Idle as u32)
        }
    );
    let actual_position = *game.source.read().unwrap().get_position();
    assert!(
        actual_position.x >= 95.0,
        "exit pose {actual_position:?}, parent {:?}, hurry {hurry}, trace {hurry_trace:#?}",
        ai.lock().unwrap().get_current_state_id()
    );
}

#[test]
fn owned_carrier_pose_redeploys_actual_cargo_and_observes_mass_without_peer_mutation() {
    let mut game = boarded_with_modules(false, true);
    let peer = boarded();
    let owner = Arc::clone(&game.target);
    let passenger = Arc::clone(&game.source);
    let peer_pose = peer.source.read().unwrap().get_transform_matrix();
    let expected_mass = passenger
        .read()
        .unwrap()
        .get_physics()
        .unwrap()
        .owned_base_mass()
        .unwrap();
    let mut pose = owner.read().unwrap().get_transform_matrix();
    pose.w_axis = glam::Vec4::new(140.0, 90.0, 0.0, 1.0);
    game.operate(60, |world| {
        let cargo = world.observe_physics_cargo_owned(&owner).unwrap();
        assert_eq!(cargo.contained_mass, expected_mass);
        assert_eq!(cargo.carrier_deck_height, 0.0);
        world.set_physics_transform_owned(&owner, &pose).unwrap();
    });
    assert_eq!(owner.read().unwrap().get_transform_matrix(), pose);
    assert_eq!(
        *passenger.read().unwrap().get_position(),
        Coord3D::new(140.0, 90.0, 0.0),
        "a container without FIREPOINT bones redeploys each rider at its current position"
    );
    assert_eq!(
        passenger.read().unwrap().get_contained_by(),
        Some(owner.read().unwrap().get_id())
    );
    assert_eq!(
        peer.source.read().unwrap().get_transform_matrix(),
        peer_pose
    );
}

#[test]
fn nested_transport_boards_redeploys_and_removes_with_actual_owned_children() {
    let original = infantry_template();
    let mut modules = original.get_behavior_module_info().to_vec();
    let data = crate::object::contain::transport_contain::TransportContainModuleData {
        slot_capacity: 2,
        base: crate::object::contain::open_contain::OpenContainModuleData {
            contain_max: 2,
            allow_inside_kind_of: KindOf::Infantry.cpp_mask(),
            ..Default::default()
        },
        ..Default::default()
    };
    modules.push(TemplateModuleInfo {
        name: "TransportContain".into(),
        module_tag: "NestedActualCargo".into(),
        data: Arc::new(crate::contain_module_overrides::ContainModuleDataAdapter::new(data)),
        interface_mask: ModuleInterfaceType::CONTAIN,
    });
    let mut base = DefaultThingTemplate::new("NestedInfantryTransport".into());
    base.add_kind_of(KindOf::Infantry);
    let mut game = NativeMatch::with_source_template(
        50.0,
        transport_template(),
        false,
        Arc::new(EntryTemplate { base, modules }),
    );
    let team = game.source.read().unwrap().get_team().unwrap();
    let child_id = game
        .factory
        .prepare_object(
            &mut game.scheduler,
            infantry_template(),
            Coord3D::new(50.0, 50.0, 0.0),
            Some(team),
            ObjectCreationFlags::empty(),
        )
        .unwrap();
    let child = game
        .factory
        .get_object(child_id)
        .unwrap()
        .get_base_object()
        .unwrap();
    let nested_id = game.source.read().unwrap().get_id();
    let outer_id = game.target.read().unwrap().get_id();
    game.operate(30, |world| {
        crate::modules::ai_world::complete_enter(world, child_id, nested_id).unwrap();
    });
    game.issue_enter(31);
    let ai = game
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    for frame in 31..=36 {
        game.operate(frame, |world| {
            world.process_enter_path_queue(8).unwrap();
            if ai.lock().unwrap().get_current_state_id()
                == Some(crate::ai::states::AIStateType::Enter as u32)
            {
                ai.lock()
                    .unwrap()
                    .update_enter_phase_with_world(world)
                    .unwrap();
            }
        });
        if game.source.read().unwrap().get_contained_by().is_some() {
            break;
        }
    }
    assert_eq!(child.read().unwrap().get_contained_by(), Some(nested_id));
    assert_eq!(
        game.source.read().unwrap().get_contained_by(),
        Some(outer_id)
    );
    let mut pose = game.target.read().unwrap().get_transform_matrix();
    pose.w_axis = glam::Vec4::new(145.0, 110.0, 0.0, 1.0);
    let outer = Arc::clone(&game.target);
    game.operate(40, |world| {
        world.set_physics_transform_owned(&outer, &pose).unwrap()
    });
    assert_eq!(
        *child.read().unwrap().get_position(),
        Coord3D::new(145.0, 110.0, 0.0)
    );
    game.operate(41, |world| {
        world.remove_contained(nested_id, outer_id, false).unwrap()
    });
    assert_eq!(game.source.read().unwrap().get_contained_by(), None);
    assert_eq!(child.read().unwrap().get_contained_by(), Some(nested_id));
    assert!(
        game.source
            .read()
            .unwrap()
            .get_contain()
            .unwrap()
            .lock()
            .unwrap()
            .get_contained_objects()
            .contains(&child_id)
    );
}

#[test]
fn boarding_removes_actual_group_member_and_retires_only_empty_supplied_group() {
    for keep_member in [false, true] {
        let mut game = NativeMatch::with_container(50.0, transport_template());
        let mut peer = NativeMatch::with_container(50.0, transport_template());
        let id = game.source.read().unwrap().get_id();
        let owner_id = game.target.read().unwrap().get_id();
        let mut group = crate::ai::AiGroup::new(7);
        group.add(id);
        if keep_member {
            group.add(owner_id);
            game.target
                .write()
                .unwrap()
                .enter_runtime_group(&group)
                .unwrap();
        }
        game.source
            .write()
            .unwrap()
            .enter_runtime_group(&group)
            .unwrap();
        let actual = Arc::new(RwLock::new(group));
        game.groups.push(Arc::clone(&actual));
        let mut peer_group = crate::ai::AiGroup::new(7);
        peer_group.add(peer.source.read().unwrap().get_id());
        peer.source
            .write()
            .unwrap()
            .enter_runtime_group(&peer_group)
            .unwrap();
        let peer_group = Arc::new(RwLock::new(peer_group));
        peer.groups.push(Arc::clone(&peer_group));
        game.operate(30, |world| {
            crate::modules::ai_world::complete_enter(world, id, owner_id).unwrap();
        });
        assert_eq!(game.source.read().unwrap().get_group_id(), None);
        assert!(!actual.read().unwrap().is_member(id));
        assert_eq!(game.groups.len(), usize::from(keep_member));
        if keep_member {
            assert!(Arc::ptr_eq(&game.groups[0], &actual));
            assert!(actual.read().unwrap().is_member(owner_id));
            assert_eq!(game.target.read().unwrap().get_group_id(), Some(7));
        }
        assert!(peer_group.read().unwrap().is_member(id));
    }
}
