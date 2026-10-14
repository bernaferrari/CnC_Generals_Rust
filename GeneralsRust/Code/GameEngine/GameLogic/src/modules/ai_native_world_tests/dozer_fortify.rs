//! Actual installed Fortify chain and CPP's task-only nested-machine snapshots.
use super::*;
use crate::modules::ai_world::AiWorld;
use crate::object::behavior::physics_update::PhysicsBehaviorModuleData;
use crate::object::body::active_body::ActiveBodyModuleData;
use crate::object::update::ai_update::dozer_ai_update::owned_state::DozerOwnedObservation;
use crate::object::update::ai_update::dozer_ai_update::{
    DozerAIUpdateModule, DozerAIUpdateModuleData, DozerTask,
};
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;
use std::sync::Mutex;
fn source_template() -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("OwnedFortifyDozer".into());
    base.add_kind_of(KindOf::Vehicle);
    base.add_kind_of(KindOf::Dozer);
    let _ = infantry_template();
    let mut data = DozerAIUpdateModuleData::default();
    data.base.set_locomotor_set_entries(
        crate::common::LocomotorSetType::Normal,
        vec!["NativeEnterGround".into()],
    );
    Arc::new(EntryTemplate {
        base,
        modules: vec![
            TemplateModuleInfo {
                name: "ActiveBody".into(),
                module_tag: "FortifyDozerBody".into(),
                data: Arc::new(ActiveBodyModuleData {
                    max_health: 100.0,
                    initial_health: 100.0,
                    ..Default::default()
                }),
                interface_mask: ModuleInterfaceType::BODY,
            },
            TemplateModuleInfo {
                name: "DozerAIUpdate".into(),
                module_tag: "FortifyDozerAI".into(),
                data: Arc::new(data),
                interface_mask: ModuleInterfaceType::UPDATE,
            },
            TemplateModuleInfo {
                name: "PhysicsBehavior".into(),
                module_tag: "FortifyDozerPhysics".into(),
                data: Arc::new(PhysicsBehaviorModuleData::default()),
                interface_mask: ModuleInterfaceType::UPDATE | ModuleInterfaceType::COLLIDE,
            },
        ],
    })
}
fn target_template() -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("OwnedFortifyTarget".into());
    base.add_kind_of(KindOf::Structure);
    Arc::new(EntryTemplate {
        base,
        modules: vec![TemplateModuleInfo {
            name: "ActiveBody".into(),
            module_tag: "FortifyTargetBody".into(),
            data: Arc::new(ActiveBodyModuleData {
                max_health: 100.0,
                initial_health: 100.0,
                ..Default::default()
            }),
            interface_mask: ModuleInterfaceType::BODY,
        }],
    })
}
fn world() -> NativeMatch {
    let mut game =
        NativeMatch::with_source_template(220.0, target_template(), false, source_template());
    register_receiving_geometry(&mut game);
    game
}
fn register_receiving_geometry(game: &mut NativeMatch) {
    for object in [&game.source, &game.target] {
        let object = object.read().unwrap();
        let pose = object.get_position();
        game.collision
            .partition_manager_mut()
            .register_object_oriented(
                object.get_id(),
                crate::object::collide::Coord3D::new(pose.x, pose.y, pose.z),
                crate::object::collide::collision_geometry::GeometryInfo::from_engine_geometry(
                    &object.get_geometry_info().shape,
                ),
                object.get_orientation(),
            )
            .unwrap();
    }
}
fn ai(game: &NativeMatch) -> Arc<Mutex<dyn crate::modules::AIUpdateInterface>> {
    game.source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap()
}
fn observation(game: &NativeMatch) -> DozerOwnedObservation {
    ai(game)
        .lock()
        .unwrap()
        .get_dozer_ai_update_interface_mut()
        .unwrap()
        .owned_state_observation_for_test()
        .unwrap()
}
fn idle(game: &mut NativeMatch, frame: u32) {
    let runtime = ai(game);
    game.operate(frame, |world| {
        runtime.lock().unwrap().execute_command_with_world(
            &crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::Idle,
                CommandSourceType::FromAi,
            ),
            world,
        )
    })
    .unwrap();
}
fn request(game: &mut NativeMatch, frame: u32) {
    idle(game, frame);
    let runtime = ai(game);
    let target = game.target.read().unwrap().get_id();
    game.operate(frame, |world| {
        runtime
            .lock()
            .unwrap()
            .request_dozer_fortify_with_world(target, world)
    })
    .unwrap();
}
fn advance(game: &mut NativeMatch, frame: u32) {
    game.operate(frame, |world| {
        world.process_enter_path_queue(10)?;
        world.tick_owned_updates(frame)
    })
    .unwrap();
}
fn save(game: &NativeMatch) -> Vec<u8> {
    let handle = game
        .source
        .read()
        .unwrap()
        .find_update_module("DozerAIUpdate")
        .unwrap();
    let mut bytes = Cursor::new(Vec::new());
    let loco = Arc::new(ground());
    handle
        .with_module(|module| {
            module
                .as_any_mut()
                .downcast_mut::<DozerAIUpdateModule>()
                .unwrap()
                .xfer_with_locomotors(&mut XferSave::new(&mut bytes, 1), &mut |name| {
                    Ok((name == "NativeEnterGround").then(|| loco.clone()))
                })
        })
        .unwrap();
    bytes.into_inner()
}
fn ground() -> crate::locomotor::LocomotorTemplate {
    let mut loco = crate::locomotor::LocomotorTemplate::new("NativeEnterGround".into());
    loco.surfaces = crate::locomotor::SURFACE_GROUND;
    loco.max_speed = 3.0;
    loco.acceleration = 0.1;
    loco
}
fn load(game: &NativeMatch, bytes: Vec<u8>) {
    let handle = game
        .source
        .read()
        .unwrap()
        .find_update_module("DozerAIUpdate")
        .unwrap();
    let loco = Arc::new(ground());
    handle
        .with_module(|module| {
            module
                .as_any_mut()
                .downcast_mut::<DozerAIUpdateModule>()
                .unwrap()
                .xfer_with_locomotors(&mut XferLoad::new(Cursor::new(bytes), 1), &mut |name| {
                    Ok((name == "NativeEnterGround").then(|| loco.clone()))
                })
        })
        .unwrap();
}
#[test]
fn actual_fortify_runs_installed_ai_and_physics_without_moving_same_id_peer() {
    let mut game = world();
    let peer = world();
    request(&mut game, 10);
    let peer_before = observation(&peer);
    let start = *game.source.read().unwrap().get_position();
    let mut phases = Vec::new();
    for frame in 11..200 {
        advance(&mut game, frame);
        let state = observation(&game);
        phases.push((frame, state));
        if state.action == 2 {
            break;
        }
    }
    let state = observation(&game);
    assert_eq!(state.primary, 3, "actual phases={phases:?}");
    assert_eq!(state.action, 2, "actual phases={phases:?}");
    assert_eq!(state.current_task, DozerTask::Fortify);
    assert!(
        game.source.read().unwrap().get_position().x > start.x + 20.0,
        "actual scheduled Physics must move toward the task"
    );
    assert_eq!(observation(&peer), peer_before);
    assert_eq!(*peer.source.read().unwrap().get_position(), start);
    let target_health = game
        .target
        .read()
        .unwrap()
        .get_body_module()
        .unwrap()
        .lock()
        .unwrap()
        .get_health();
    advance(&mut game, 200);
    assert_eq!(observation(&game).action, 2);
    assert_eq!(
        game.target
            .read()
            .unwrap()
            .get_body_module()
            .unwrap()
            .lock()
            .unwrap()
            .get_health(),
        target_health,
        "original Fortify body has no invented upgrade or healing"
    );
}
#[test]
fn real_primary_save_preserves_pending_task_and_original_fresh_action_restart() {
    let mut game = world();
    request(&mut game, 10);
    advance(&mut game, 11);
    assert_eq!(observation(&game).primary, 3);
    assert_eq!(observation(&game).action, 0);
    let bytes = save(&game);
    let restored = world();
    load(&restored, bytes.clone());
    assert_eq!(
        save(&restored),
        bytes,
        "original initialized primary/task shape roundtrips exactly"
    );
    assert_eq!(observation(&restored).current_task, DozerTask::Fortify);
    advance(&mut game, 12);
    assert_eq!(observation(&game).action, 1);
    let moving_bytes = save(&game);
    let mut restarted = world();
    load(&restarted, moving_bytes);
    assert_eq!(observation(&restarted).primary, 3);
    assert_eq!(
        observation(&restarted).action,
        0,
        "CPP819 saves task only: newly constructed action machine restarts Pick"
    );
    assert_eq!(observation(&restarted).goal, crate::common::INVALID_ID);
    advance(&mut restarted, 13);
    assert_eq!(observation(&restarted).action, 1);
    assert_eq!(
        observation(&restarted).goal,
        restarted.target.read().unwrap().get_id()
    );
}
#[test]
fn missing_action_target_returns_to_pick_then_cancels_without_peer_task_loss() {
    let mut game = world();
    let mut peer = world();
    request(&mut game, 10);
    request(&mut peer, 10);
    advance(&mut game, 11);
    advance(&mut game, 12);
    assert_eq!(observation(&game).action, 1);
    let target = game.target.read().unwrap().get_id();
    game.operate(12, |world| world.destroy_object_owned(target))
        .unwrap();
    assert_eq!(game.dispose_destroyed(12).unwrap(), 1);
    advance(&mut game, 13);
    assert_eq!(observation(&game).action, 0);
    advance(&mut game, 14);
    assert_eq!(observation(&game).primary, 0);
    assert_eq!(observation(&game).current_task, DozerTask::Invalid);
    assert!(
        !ai(&game)
            .lock()
            .unwrap()
            .get_dozer_ai_update_interface_mut()
            .unwrap()
            .is_any_task_pending()
    );
    assert!(
        ai(&peer)
            .lock()
            .unwrap()
            .get_dozer_ai_update_interface_mut()
            .unwrap()
            .is_task_pending(DozerTask::Fortify)
    );
}

#[test]
fn full_object_restore_rebinds_fortify_and_restarts_original_nested_action() {
    use super::object_restore_tests::{Catalog, capture_at, fresh_with_templates};
    let catalog = Catalog::from_templates(source_template(), target_template(), ground());
    let mut game = fresh_with_templates(
        &catalog,
        None,
        "OwnedFortifyDozer",
        "OwnedFortifyTarget",
        true,
        17,
    )
    .unwrap();
    register_receiving_geometry(&mut game);
    request(&mut game, 18);
    advance(&mut game, 19);
    advance(&mut game, 20);
    assert_eq!(observation(&game).action, 1);
    let pose = *game.source.read().unwrap().get_position();
    let base_state = ai(&game).lock().unwrap().get_current_state_id();
    let records = capture_at(&mut game, &catalog, 20);
    let mut restored = fresh_with_templates(
        &catalog,
        Some(&records),
        "OwnedFortifyDozer",
        "OwnedFortifyTarget",
        true,
        20,
    )
    .unwrap();
    assert_eq!(*restored.source.read().unwrap().get_position(), pose);
    assert_eq!(
        ai(&restored).lock().unwrap().get_current_state_id(),
        base_state
    );
    assert_eq!(observation(&restored).primary, 3);
    assert_eq!(observation(&restored).current_task, DozerTask::Fortify);
    assert_eq!(
        observation(&restored).action,
        0,
        "CPP action Xfer saves task only"
    );
    let original = observation(&game);
    register_receiving_geometry(&mut restored);
    advance(&mut restored, 21);
    assert_eq!(observation(&restored).action, 1);
    assert_eq!(
        observation(&restored).goal,
        restored.target.read().unwrap().get_id()
    );
    assert_eq!(
        observation(&game),
        original,
        "same-ID original owner remains untouched"
    );
}

#[test]
fn target_disappearing_during_action_returns_idle_but_retains_original_pending_task() {
    let mut game = world();
    let peer = world();
    request(&mut game, 10);
    let mut action_frame = None;
    for frame in 11..200 {
        advance(&mut game, frame);
        if observation(&game).action == 2 {
            action_frame = Some(frame);
            break;
        }
    }
    let frame = action_frame.expect("real movement must reach the action state");
    let target = game.target.read().unwrap().get_id();
    game.operate(frame, |world| world.destroy_object_owned(target))
        .unwrap();
    assert_eq!(game.dispose_destroyed(frame).unwrap(), 1);
    let peer_before = observation(&peer);
    advance(&mut game, frame + 1);
    assert_eq!(observation(&game).primary, 0);
    assert_eq!(observation(&game).current_task, DozerTask::Invalid);
    assert!(
        ai(&game)
            .lock()
            .unwrap()
            .get_dozer_ai_update_interface_mut()
            .unwrap()
            .is_task_pending(DozerTask::Fortify),
        "CPP DoAction failure exits primary without canceling the queued task"
    );
    assert_eq!(observation(&peer), peer_before);
    assert!(peer.factory.get_object(target).is_some());
}

#[test]
fn lethal_receiving_body_keeps_dead_dozer_awake_without_advancing_primary() {
    use super::object_restore_tests::{Catalog, fresh_with_templates};
    let catalog = Catalog::from_templates(source_template(), target_template(), ground());
    let mut game = fresh_with_templates(
        &catalog,
        None,
        "OwnedFortifyDozer",
        "OwnedFortifyTarget",
        true,
        17,
    )
    .unwrap();
    register_receiving_geometry(&mut game);
    let peer = world();
    request(&mut game, 18);
    advance(&mut game, 19);
    let primary_before = observation(&game);
    let peer_before = observation(&peer);
    let owner = game.source.clone();
    let source = game.target.read().unwrap().get_id();
    game.operate(19, |world| {
        world.attempt_damage_owned(&owner, &mut super::lethal_damage::fatal(source))
    })
    .unwrap();
    assert!(owner.read().unwrap().is_effectively_dead());
    assert_eq!(
        owner
            .read()
            .unwrap()
            .get_body_module()
            .unwrap()
            .lock()
            .unwrap()
            .get_health(),
        0.0
    );
    for frame in 20..23 {
        advance(&mut game, frame);
        assert_eq!(
            observation(&game),
            primary_before,
            "CPP dead owner skips only derived primary update"
        );
        let wake = owner
            .read()
            .unwrap()
            .find_update_module("DozerAIUpdate")
            .unwrap()
            .with_module(|module| module.get_initial_wake_frame().unwrap());
        assert_eq!(wake, frame + 1, "CPP1578 still returns UPDATE_SLEEP_NONE");
    }
    assert_eq!(observation(&peer), peer_before);
    assert!(!peer.source.read().unwrap().is_effectively_dead());
    assert_eq!(
        peer.source
            .read()
            .unwrap()
            .get_body_module()
            .unwrap()
            .lock()
            .unwrap()
            .get_health(),
        100.0
    );
}

#[test]
fn receiving_membership_delivers_only_actual_idle_dozer_events() {
    use super::object_restore_tests::{Catalog, fresh_with_templates, transfer};
    let catalog = Catalog::from_templates(source_template(), target_template(), ground());
    let mut game = fresh_with_templates(
        &catalog,
        None,
        "OwnedFortifyDozer",
        "OwnedFortifyTarget",
        true,
        17,
    )
    .unwrap();
    let mut peer = world();
    game.idle_workers.take();
    idle(&mut game, 18);
    let owner = game.source.clone();
    let player =
        owner.read().unwrap().get_controlling_player_id().unwrap() as crate::player::PlayerIndex;
    transfer(&mut game, &catalog, 18, |services| {
        let object = owner.read().unwrap();
        services.power.validate_membership(player, &object).unwrap();
        services
            .power
            .membership_idle_worker(player, &object, true)
            .unwrap();
        services
            .power
            .membership_idle_worker(player, &object, false)
            .unwrap();
    });
    let id = owner.read().unwrap().get_id();
    assert_eq!(
        game.idle_workers.take(),
        vec![
            crate::helpers::IdleWorkerEvent::Add { object: id, player },
            crate::helpers::IdleWorkerEvent::Remove { object: id, player }
        ]
    );
    assert_eq!(
        peer.idle_workers.take(),
        Vec::<crate::helpers::IdleWorkerEvent>::new()
    );
}
