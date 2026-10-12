//! Actual cached AIExit and original-point Transport callback continuation.
use super::*;
use crate::common::ObjectID;
use crate::modules::ai_world::AiWorld;
fn boarded() -> NativeMatch {
    boarded_with_target_ai(false)
}
fn boarded_with_target_ai(target_ai: bool) -> NativeMatch {
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
    let mut game = NativeMatch::with_container(50.0, template);
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
    use crate::common::{Matrix3D, ModelConditionFlags};
    use crate::object::draw::w3d_model_draw::{
        ModelConditionInfo, PristineBoneInfo, W3DModelDraw, W3DModelDrawModuleData,
    };
    use crate::object::drawable::{Drawable, DrawableExt, DrawableType};
    use game_engine::common::name_key_generator::NameKeyGenerator;
    let mut base = DefaultThingTemplate::new("NativeNumberedExit".into());
    base.add_kind_of(KindOf::Structure);
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
    let target = Arc::new(EntryTemplate {
        base,
        modules: vec![TemplateModuleInfo {
            name: "TransportContain".into(),
            module_tag: "NumberedCargo".into(),
            data: Arc::new(crate::contain_module_overrides::ContainModuleDataAdapter::new(data)),
            interface_mask: ModuleInterfaceType::CONTAIN,
        }],
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
    game.operate(41, |world| {
        world
            .execute_exit_command(id, false, CommandSourceType::FromScript)
            .unwrap()
    });
    game.operate(42, |world| world.update_exit_phase(id).unwrap());
    assert_eq!(game.source.read().unwrap().get_contained_by(), None);
    assert_eq!(
        *game.source.read().unwrap().get_position(),
        Coord3D::new(30.0, 50.0, 0.0)
    );
    assert_eq!(
        ai.lock().unwrap().get_current_state_id(),
        Some(crate::ai::states::AIStateType::FollowPath as u32),
        "original exit body must not finish its replacement FollowPath state"
    );
    let owner = Arc::clone(&game.source);
    for frame in 43..=1500 {
        let ai_proxy = owner
            .read()
            .unwrap()
            .named_update_registrations("AIUpdateInterface")
            .pop()
            .unwrap();
        game.operate_with_current(frame, Some(ai_proxy), |world| {
            world.process_enter_path_queue(8).unwrap();
            if ai.lock().unwrap().get_current_state_id()
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
        if ai.lock().unwrap().get_current_state_id()
            == Some(crate::ai::states::AIStateType::Idle as u32)
        {
            break;
        }
    }
    assert_eq!(
        ai.lock().unwrap().get_current_state_id(),
        Some(crate::ai::states::AIStateType::Idle as u32)
    );
    assert!(game.source.read().unwrap().get_position().x >= 95.0);
}
