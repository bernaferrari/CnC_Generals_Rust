//! Production request/phase boundaries with admitted Dozer module identity.
//! These fixtures never assign current-task or AI/action state directly.
use super::super::*;
use crate::command_executor::CommandExecutor;
use crate::command_system::{CommandResult, CommandType, GameCommand, ModifierKeys};
use crate::save_load::snapshot::Snapshot;
use game_engine::common::thing::update_module::DisabledType;
use gamelogic::object::update::ai_update::dozer_ai_update::DozerTask;
use gamelogic::object::update::ai_update::dozer_ai_update::owned_state::ActionState;

fn world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.frame = 10;
    world.add_player(Player::new(0, Team::USA, "DozerOwner", true));
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([(
        "dozer-request-phase.ini".into(),
        "Object PhaseDozer\nKindOf = VEHICLE DOZER SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = DozerAIUpdate DozerTaskSource\nRepairHealthPercentPerSecond = 2\nEnd\nEnd\nObject PhaseSite\nKindOf = STRUCTURE\nBuildCost = 0\nBuildTime = 10\nBody = ActiveBody Body\nMaxHealth = 500\nInitialHealth = 500\nEnd\nEnd\nObject PhasePaidSite\nKindOf = STRUCTURE\nBuildCost = 300\nBuildTime = 10\nBody = ActiveBody Body\nMaxHealth = 500\nInitialHealth = 500\nEnd\nBehavior = SupplyWarehouseCreate WarehouseBirth\nEnd\nEnd\nObject PhaseDamaged\nKindOf = STRUCTURE\nBody = ActiveBody Body\nMaxHealth = 500\nInitialHealth = 200\nEnd\nEnd\n".into(),
    )]).expect("authored content")).expect("admission");
    let actor = world
        .create_object_for_player_with_effects_checked(
            "PhaseDozer",
            0,
            Vec3::ZERO,
            None,
            &mut Default::default(),
        )
        .expect("factory")
        .expect("actor");
    let target = world
        .create_object_for_player_with_effects_checked(
            "PhaseDamaged",
            0,
            Vec3::new(20.0, 0.0, 0.0),
            None,
            &mut Default::default(),
        )
        .expect("factory")
        .expect("target");
    assert!(
        world
            .host_object(actor)
            .unwrap()
            .thing()
            .template
            .receiving_client_template
            .as_ref()
            .and_then(|d| d.dozer_ai_identity())
            .is_some()
    );
    (world, actor, target)
}
fn command(world: &mut GameLogic, id: ObjectId, kind: CommandType) -> CommandResult {
    CommandExecutor::new(world, 0)
        .execute_command(GameCommand {
            command_type: kind,
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![id],
            modifier_keys: ModifierKeys::default(),
        })
        .expect("command")
}
fn drive(world: &mut GameLogic, id: ObjectId, frame: u32) {
    world.frame = frame;
    world
        .update_ai(&[id], 1.0 / 30.0)
        .expect("actual AI/support boundary");
}
#[test]
fn authored_repair_command_records_request_then_enters_action_on_original_phases() {
    let (mut world, id, target) = world();
    let hp = world.host_object(target).unwrap().health.current;
    assert_eq!(
        command(&mut world, id, CommandType::Repair { target_id: target }),
        CommandResult::Success
    );
    let actor = world.host_object(id).unwrap();
    assert_eq!(actor.dozer_tasks.current, DozerTask::Invalid);
    assert_eq!(actor.dozer_tasks.requests[1].target, Some(target));
    assert!(actor.dozer_tasks.docks[1].iter().all(|point| point.valid));
    assert_eq!(world.host_object(target).unwrap().health.current, hp);
    drive(&mut world, id, 11);
    assert_eq!(
        world.host_object(id).unwrap().dozer_tasks.current,
        DozerTask::Repair
    );
    assert!(matches!(
        world.host_object(id).unwrap().dozer_tasks.primary.actions[1].state,
        ActionState::PickPosition { .. }
    ));
    assert_eq!(world.host_object(target).unwrap().health.current, hp);
    drive(&mut world, id, 12);
    assert!(matches!(
        world.host_object(id).unwrap().dozer_tasks.primary.actions[1].state,
        ActionState::MoveToPosition
    ));
    drive(&mut world, id, 13);
    assert!(matches!(
        world.host_object(id).unwrap().dozer_tasks.primary.actions[1].state,
        ActionState::DoAction { enter_frame: 13 }
    ));
    assert_eq!(
        world.host_object(target).unwrap().health.current,
        hp,
        "DoAction onEnter is not its update"
    );
    drive(&mut world, id, 14);
    assert!(world.host_object(target).unwrap().health.current > hp);
}
#[test]
fn disabled_entered_task_cancels_only_the_driving_same_id_actor() {
    let (mut first, id, target) = world();
    let (mut peer, peer_id, peer_target) = world();
    assert_eq!((id, target), (peer_id, peer_target));
    assert_eq!(
        command(&mut first, id, CommandType::Repair { target_id: target }),
        CommandResult::Success
    );
    assert_eq!(
        command(
            &mut peer,
            peer_id,
            CommandType::Repair {
                target_id: peer_target
            }
        ),
        CommandResult::Success
    );
    drive(&mut first, id, 11);
    drive(&mut peer, peer_id, 11);
    assert!(
        first
            .set_owned_disabled_until(id, DisabledType::DisabledEmp, 50)
            .expect("receiving disabled fixture operation")
    );
    assert_eq!(
        first.host_object(id).unwrap().dozer_tasks.current,
        DozerTask::Invalid
    );
    assert_eq!(
        first.host_object(id).unwrap().dozer_tasks.requests[1].target,
        None
    );
    assert_eq!(
        peer.host_object(peer_id).unwrap().dozer_tasks.current,
        DozerTask::Repair
    );
    assert_eq!(
        peer.host_object(peer_id).unwrap().dozer_tasks.requests[1].target,
        Some(peer_target)
    );
    let hp = first.host_object(target).unwrap().health.current;
    for frame in 12..16 {
        drive(&mut first, id, frame);
    }
    assert_eq!(first.host_object(target).unwrap().health.current, hp);
}
#[test]
fn frame_zero_pending_request_is_not_selected_and_same_positive_frame_tie_is_build() {
    let (mut world, id, target) = world();
    world.frame = 0;
    world.dozer_new_task_repair(id, target).unwrap();
    drive(&mut world, id, 0);
    assert_eq!(
        world.host_object(id).unwrap().dozer_tasks.current,
        DozerTask::Invalid
    );
    world.frame = 10;
    // Both public original newTask endpoints accept the same supplied object;
    // neither task can be inferred from that shared target identity.
    world.dozer_new_task_build(id, target).unwrap();
    world.dozer_new_task_repair(id, target).unwrap();
    drive(&mut world, id, 11);
    let actor = world.host_object(id).unwrap();
    assert_eq!(
        actor.dozer_tasks.current,
        DozerTask::Build,
        "CPP2244 strict > scans BUILD before REPAIR; hq-ylhfz title is stale"
    );
    assert_eq!(actor.dozer_tasks.requests[0].target, Some(target));
    assert_eq!(actor.dozer_tasks.requests[1].target, Some(target));
}
#[test]
fn native32_same_target_pending_requests_restore_without_legacy_repair_activation() {
    let (mut source, id, target) = world();
    source.dozer_new_task_build(id, target).unwrap();
    source.dozer_new_task_repair(id, target).unwrap();
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).expect("save");
    assert!(snapshot.version >= 32);
    let (mut restored, _, _) = world();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("receiving restore");
    let actor = restored.host_object(id).unwrap();
    assert_eq!(
        actor.dozer_tasks.current,
        DozerTask::Invalid,
        "DZRP must not invent active Repair"
    );
    assert_eq!(actor.dozer_tasks.requests[0].target, Some(target));
    assert_eq!(actor.dozer_tasks.requests[1].target, Some(target));
    drive(&mut restored, id, 11);
    assert_eq!(
        restored.host_object(id).unwrap().dozer_tasks.current,
        DozerTask::Build
    );
}

#[test]
fn supplied_build_request_does_not_interrupt_busy_motion_but_repair_invalid_current_does() {
    let (mut world, id, target) = world();
    assert_eq!(
        command(
            &mut world,
            id,
            CommandType::Move {
                destination: Vec3::new(500.0, 0.0, 0.0)
            }
        ),
        CommandResult::Success
    );
    let before = world.host_object(id).unwrap().movement.target_position;
    assert_eq!(world.host_object(id).unwrap().ai_state, AIState::Moving);
    let site = world
        .create_object_under_construction_for_player(
            "PhaseSite",
            0,
            Vec3::new(200.0, 0.0, 0.0),
            0.0,
        )
        .expect("factory")
        .expect("scaffold");
    world.dozer_new_task_build(id, site).unwrap();
    let actor = world.host_object(id).unwrap();
    assert_eq!(actor.dozer_tasks.current, DozerTask::Invalid);
    assert_eq!(
        actor.ai_state,
        AIState::Moving,
        "newTask resets only primary"
    );
    assert_eq!(actor.movement.target_position, before);
    drive(&mut world, id, 11);
    assert_eq!(
        world.host_object(id).unwrap().dozer_tasks.current,
        DozerTask::Invalid,
        "primary Idle waits for inherited AI Idle"
    );
    assert_eq!(
        command(&mut world, id, CommandType::Repair { target_id: target }),
        CommandResult::Success
    );
    let actor = world.host_object(id).unwrap();
    assert_eq!(
        actor.ai_state,
        AIState::Idle,
        "CPP2359 preamble only for INVALID current"
    );
    assert!(actor.movement.path.is_empty());
    assert_eq!(actor.dozer_tasks.current, DozerTask::Invalid);
    assert!(
        actor.dozer_tasks.requests[0].target.is_some(),
        "BUILD remains parked"
    );
}
#[test]
fn native32_entered_action_keeps_current_but_restarts_nested_pick() {
    let (mut source, id, target) = world();
    assert_eq!(
        command(&mut source, id, CommandType::Repair { target_id: target }),
        CommandResult::Success
    );
    for frame in 11..14 {
        drive(&mut source, id, frame);
    }
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("save entered action");
    let (mut restored, _, _) = world();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore entered action");
    let actor = restored.host_object(id).unwrap();
    assert_eq!(actor.dozer_tasks.current, DozerTask::Repair);
    assert!(
        matches!(
            actor.dozer_tasks.primary.actions[1].state,
            ActionState::PickPosition { .. }
        ),
        "CPP819 saves task only; no invented nested continuation"
    );
    let hp = restored.host_object(target).unwrap().health.current;
    drive(&mut restored, id, 14);
    assert_eq!(
        restored.host_object(target).unwrap().health.current,
        hp,
        "first restored action phase is Pick"
    );
}
#[test]
fn supplied_fortify_request_uses_shared_pick_move_empty_action_and_disabled_cancel() {
    let (mut source, id, target) = world();
    source
        .dozer_request_task_owned(id, target, DozerTask::Fortify)
        .unwrap();
    assert_eq!(
        source.host_object(id).unwrap().dozer_tasks.current,
        DozerTask::Invalid
    );
    drive(&mut source, id, 11);
    assert_eq!(
        source.host_object(id).unwrap().dozer_tasks.current,
        DozerTask::Fortify
    );
    let hp = source.host_object(target).unwrap().health.current;
    for frame in 12..16 {
        drive(&mut source, id, frame);
    }
    assert_eq!(
        source.host_object(target).unwrap().health.current,
        hp,
        "original Fortify body has no upgrade/heal effect"
    );
    assert!(
        source
            .set_owned_disabled_until(id, DisabledType::DisabledEmp, 50)
            .expect("receiving disabled fixture operation")
    );
    assert_eq!(
        source.host_object(id).unwrap().dozer_tasks.current,
        DozerTask::Invalid
    );
    assert_eq!(
        source.host_object(id).unwrap().dozer_tasks.requests[2].target,
        None
    );
}

#[test]
fn disabled_pending_sibling_waits_for_original_module_disabled_mask() {
    let (mut source, id, target) = world();
    source.dozer_new_task_build(id, target).unwrap();
    source.dozer_new_task_repair(id, target).unwrap();
    drive(&mut source, id, 11);
    assert_eq!(
        source.host_object(id).unwrap().dozer_tasks.current,
        DozerTask::Build
    );
    assert!(
        source
            .set_owned_disabled_until(id, DisabledType::DisabledEmp, 50)
            .expect("receiving disabled fixture operation")
    );
    assert_eq!(
        source.host_object(id).unwrap().dozer_tasks.requests[0].target,
        None
    );
    assert_eq!(
        source.host_object(id).unwrap().dozer_tasks.requests[1].target,
        Some(target)
    );
    for frame in 12..16 {
        drive(&mut source, id, frame);
    }
    assert_eq!(
        source.host_object(id).unwrap().dozer_tasks.current,
        DozerTask::Invalid,
        "EMP is not in AIUpdate disabled processing mask"
    );
    assert!(
        source
            .clear_owned_disabled(id, DisabledType::DisabledEmp)
            .expect("receiving disabled fixture operation")
    );
    drive(&mut source, id, 16);
    assert_eq!(
        source.host_object(id).unwrap().dozer_tasks.current,
        DozerTask::Repair
    );
}

#[test]
fn frozen_idle_worker_query_excludes_an_entered_task_with_inherited_ai_idle() {
    let (mut source, id, target) = world();
    assert_eq!(
        command(&mut source, id, CommandType::Repair { target_id: target }),
        CommandResult::Success
    );
    drive(&mut source, id, 11);
    assert_eq!(source.host_object(id).unwrap().ai_state, AIState::Idle);
    assert_eq!(
        source.host_object(id).unwrap().dozer_tasks.current,
        DozerTask::Repair
    );
    let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&mut source, 0);
    let frozen = frame.objects.iter().find(|o| o.id == id).unwrap();
    assert!(frozen.is_dozer_task_entered);
    assert!(
        !frame
            .alive_selectable_friendly_idle_worker_ids(Team::USA)
            .contains(&id)
    );
}
#[test]
fn resume_query_uses_recorded_current_owner_even_for_the_same_actor() {
    let (mut source, id, _) = world();
    let site = source
        .create_object_under_construction_for_player("PhaseSite", 0, Vec3::new(20.0, 0.0, 0.0), 0.0)
        .expect("scaffold factory")
        .expect("scaffold");
    assert!(source.resume_construction(&[id], site).unwrap());
    drive(&mut source, id, 11);
    assert_eq!(
        source.host_object(id).unwrap().dozer_tasks.current,
        DozerTask::Build
    );
    assert!(
        !source.can_resume_construction_of(id, site),
        "ActionManager463 has no self-builder exception"
    );
    assert!(!source.resume_construction(&[id], site).unwrap());
    assert_eq!(source.host_object(site).unwrap().builder_id, Some(id));
    assert_eq!(
        source.host_object(id).unwrap().dozer_tasks.current,
        DozerTask::Build
    );
}

#[test]
fn admitted_same_name_worker_kind_and_dock_end_are_receiving_world_values() {
    fn make(class: &str) -> (GameLogic, ObjectId, ObjectId) {
        let mut owner = GameLogic::new();
        owner.frame = 20;
        owner.add_player(Player::new(0, Team::USA, "ClassOwner", true));
        let source = format!(
            "Object SameNameTaskActor\nKindOf = INFANTRY DOZER HARVESTER SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = {class} TaskAI\nEnd\nEnd\nObject SameNameTaskTarget\nKindOf = STRUCTURE\nBody = ActiveBody Body\nMaxHealth = 500\nInitialHealth = 200\nEnd\nEnd\n"
        );
        owner
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "class-owner.ini".into(),
                    source,
                )])
                .unwrap(),
            )
            .unwrap();
        let id = owner
            .create_object_for_player_with_effects_checked(
                "SameNameTaskActor",
                0,
                Vec3::ZERO,
                None,
                &mut Default::default(),
            )
            .unwrap()
            .unwrap();
        let target = owner
            .create_object_for_player_with_effects_checked(
                "SameNameTaskTarget",
                0,
                Vec3::new(20.0, 0.0, 0.0),
                None,
                &mut Default::default(),
            )
            .unwrap()
            .unwrap();
        (owner, id, target)
    }
    let (mut dozer, id, target) = make("DozerAIUpdate");
    let (mut worker, peer, peer_target) = make("WorkerAIUpdate");
    assert_eq!((id, target), (peer, peer_target));
    assert!(!dozer.host_object(id).unwrap().worker_ai_update);
    assert!(
        worker.host_object(peer).unwrap().worker_ai_update,
        "actual admitted definition, not ambient same-name lookup"
    );
    dozer.dozer_new_task_build(id, target).unwrap();
    worker.dozer_new_task_build(peer, peer_target).unwrap();
    let a = &dozer.host_object(id).unwrap().dozer_tasks.docks[0];
    let b = &worker.host_object(peer).unwrap().dozer_tasks.docks[0];
    assert_ne!(a[1], a[2], "Dozer END is five cells beyond ACTION");
    assert_eq!(b[1], b[2], "Worker.cpp638 END equals ACTION");
    let _inert = GameLogic::new();
    assert!(!dozer.host_object(id).unwrap().worker_ai_update);
    assert!(worker.host_object(peer).unwrap().worker_ai_update);
}

#[test]
fn untouched_admitted_dozer_initial_primary_roundtrips_actual_native_wire() {
    use crate::save_load::{XferData, XferLoad, XferSave};
    let (source, id, target) = world();
    let actor = source.host_object(id).unwrap();
    assert!(actor.dozer_tasks.primary.control.initialized);
    let source_primary = actor
        .dozer_tasks
        .primary
        .save_original_control_bytes()
        .expect("initialized source control");
    assert!(actor.weapon_set_mine_clearing_detail);
    let hp = source.host_object(target).unwrap().health.current;
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let mut snapshot = builder
        .create_world_snapshot(&source)
        .expect("untouched source save");
    let mut encoded = std::io::Cursor::new(Vec::new());
    snapshot
        .xfer(&mut XferSave::new(&mut encoded))
        .expect("original primary wire save");
    let mut decoded = crate::save_load::snapshot::WorldSnapshot::default();
    decoded
        .xfer(&mut XferLoad::new(std::io::Cursor::new(
            encoded.into_inner(),
        )))
        .expect("original primary wire load");
    let (mut restored, _, _) = world();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .expect("receiving restore");
    let actor = restored.host_object(id).unwrap();
    assert_eq!(actor.dozer_tasks.current, DozerTask::Invalid);
    assert_eq!(
        actor
            .dozer_tasks
            .primary
            .save_original_control_bytes()
            .expect("restored control"),
        source_primary
    );
    assert!(actor.dozer_tasks.primary.control.initialized);
    assert!(actor.weapon_set_mine_clearing_detail);
    assert_eq!(restored.host_object(target).unwrap().health.current, hp);
}

#[test]
fn actual_player_move_cancels_only_the_receiving_dozer_same_id_task() {
    let (mut first, first_id, _) = world();
    let (mut peer, peer_id, _) = world();
    assert_eq!(first_id, peer_id);

    for world in [&mut first, &mut peer] {
        let target = world
            .create_object_for_player_with_effects_checked(
                "PhaseDamaged",
                0,
                Vec3::new(20.0, 0.0, 13.0),
                None,
                &mut Default::default(),
            )
            .expect("repair target factory")
            .expect("repair target");
        assert!(world.dozer_new_task_repair(first_id, target).unwrap());
        drive(world, first_id, 11);
        assert_eq!(
            world.host_object(first_id).unwrap().dozer_tasks.current,
            DozerTask::Repair
        );
    }
    let peer_request = peer.host_object(peer_id).unwrap().dozer_tasks.requests[1].target;

    assert_eq!(
        system_command(
            &mut first,
            first_id,
            CommandType::Move {
                destination: Vec3::new(500.0, 0.0, 0.0),
            },
        ),
        CommandResult::Success
    );
    assert_eq!(
        first.host_object(first_id).unwrap().dozer_tasks.current,
        DozerTask::Invalid,
        "the checked Dozer aiDoCommand cancel runs before the new move"
    );
    assert_eq!(
        first.host_object(first_id).unwrap().ai_state,
        AIState::Moving
    );
    assert_eq!(
        peer.host_object(peer_id).unwrap().dozer_tasks.current,
        DozerTask::Repair
    );
    assert_eq!(
        peer.host_object(peer_id).unwrap().dozer_tasks.requests[1].target,
        peer_request,
        "same numeric ObjectId in a peer world retains its task"
    );

    let snapshot = crate::save_load::snapshot::SnapshotBuilder::new()
        .create_world_snapshot(&first)
        .expect("cancelled construction source snapshot");
    let mut restored = GameLogic::new();
    crate::save_load::snapshot::SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("cancelled construction restore");
    assert_eq!(
        restored.host_object(first_id).unwrap().dozer_tasks.current,
        DozerTask::Invalid
    );
}

#[test]
fn request_cancel_preserves_invalid_dock_raw_coordinates_through_native_wire() {
    use crate::save_load::{XferData, XferLoad, XferSave};
    let (mut source, id, _) = world();
    let target = source
        .create_object_for_player_with_effects_checked(
            "PhaseDamaged",
            0,
            Vec3::new(20.0, 7.0, 13.0),
            None,
            &mut Default::default(),
        )
        .expect("target factory")
        .expect("target");
    source.dozer_new_task_repair(id, target).unwrap();
    let original = source.host_object(id).unwrap().dozer_tasks.docks[1];
    assert!(original.iter().all(|point| point.valid));
    assert!(original.iter().any(|point| point.location != Vec3::ZERO));
    drive(&mut source, id, 11);
    source.dozer_cancel_current_task_from_player(id).unwrap();
    let cancelled = source.host_object(id).unwrap().dozer_tasks.docks[1];
    for (before, after) in original.iter().zip(cancelled) {
        assert!(!after.valid, "original cancel clears validity only");
        assert_eq!(after.location, before.location);
    }
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let mut snapshot = builder
        .create_world_snapshot(&source)
        .expect("cancelled source save");
    let mut bytes = std::io::Cursor::new(Vec::new());
    snapshot
        .xfer(&mut XferSave::new(&mut bytes))
        .expect("native cancelled dock save");
    let mut decoded = crate::save_load::snapshot::WorldSnapshot::default();
    decoded
        .xfer(&mut XferLoad::new(std::io::Cursor::new(bytes.into_inner())))
        .expect("native cancelled dock load");
    let (mut restored, _, _) = world();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .expect("receiving restore");
    let actor = restored.host_object(id).unwrap();
    assert_eq!(actor.dozer_tasks.current, DozerTask::Invalid);
    assert!(actor.dozer_tasks.requests[1].target.is_none());
    assert_eq!(
        actor.dozer_tasks.docks[1], cancelled,
        "invalid raw locations must survive both coordinate conversions"
    );
}

#[test]
fn direct_snapshot_rejects_foreign_action_definition_before_receiving_mutation() {
    let (mut source, id, target) = world();
    source.dozer_new_task_repair(id, target).unwrap();
    drive(&mut source, id, 11);
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let mut snapshot = builder
        .create_world_snapshot(&source)
        .expect("entered source save");
    let actor = snapshot
        .objects
        .values_mut()
        .find(|object| object.id == id)
        .unwrap();
    let saved = actor
        .modules
        .iter_mut()
        .find_map(|(_, module)| match module {
            crate::save_load::snapshot::ModuleSnapshot::DozerTasks(saved) => Some(saved),
            _ => None,
        })
        .expect("actual source task payload");
    // Corrupt only detached input, never live task/action state.
    saved.primary.actions[1].task = DozerTask::Fortify;
    let (mut receiving, _, _) = world();
    receiving.frame = 77;
    let error = builder
        .restore_from_snapshot(&snapshot, &mut receiving)
        .expect_err("foreign nested task definition must be rejected");
    assert!(error.to_string().contains("action task differs"));
    assert_eq!(receiving.frame, 77);
    assert_eq!(
        receiving.host_object(id).unwrap().dozer_tasks.current,
        DozerTask::Invalid
    );
    assert_eq!(receiving.host_object(target).unwrap().health.current, 200.0);
}

fn system_command(world: &mut GameLogic, id: ObjectId, kind: CommandType) -> CommandResult {
    crate::command_system::CommandSystem::new()
        .execute_command(
            &GameCommand {
                command_type: kind,
                player_id: 0,
                command_id: 81,
                timestamp: std::time::SystemTime::now(),
                selected_units: vec![id],
                modifier_keys: ModifierKeys::default(),
            },
            world,
        )
        .expect("checked actual CommandSystem fixture operation")
}

#[test]
fn actual_command_system_build_runs_source_idle_then_factory_then_pending_request() {
    let (mut first, id, _) = world();
    let (mut peer, peer_id, _) = world();
    assert_eq!(id, peer_id);
    for world in [&mut first, &mut peer] {
        world.get_player_mut(0).unwrap().resources.supplies = 1000;
        assert_eq!(
            system_command(
                world,
                id,
                CommandType::Move {
                    destination: Vec3::new(500.0, 0.0, 0.0),
                }
            ),
            CommandResult::Success
        );
        assert_eq!(world.host_object(id).unwrap().ai_state, AIState::Moving);
        world.attack_events.clear();
    }
    let peer_goal = peer.host_object(peer_id).unwrap().movement.target_position;
    let warehouse_callbacks = first.supply_create_warehouse_registers;
    assert_eq!(
        system_command(
            &mut first,
            id,
            CommandType::Build {
                template_name: "PhasePaidSite".into(),
                location: Vec3::new(200.0, 0.0, 0.0),
            }
        ),
        CommandResult::Success
    );
    let actor = first.host_object(id).unwrap();
    assert_eq!(
        actor.ai_state,
        AIState::Idle,
        "BuildAssistant Idle precedes construct"
    );
    assert!(actor.movement.path.is_empty());
    assert_eq!(
        actor.target, None,
        "the new scaffold is not an attack/order target mirror"
    );
    assert_eq!(actor.dozer_tasks.current, DozerTask::Invalid);
    let site = actor.dozer_tasks.requests[0]
        .target
        .expect("supplied BUILD request");
    assert_eq!(actor.dozer_tasks.requests[0].order_frame, 10);
    assert!(actor.dozer_tasks.docks[0].iter().all(|point| point.valid));
    let scaffold = first.host_object(site).unwrap();
    assert_eq!(scaffold.producer_id, Some(id));
    assert_eq!(scaffold.builder_id, Some(id));
    assert!(scaffold.status.under_construction);
    assert_eq!(scaffold.health.current, 1.0);
    assert_eq!(scaffold.construction_percent, 0.0);
    assert_eq!(
        first.supply_create_warehouse_registers,
        warehouse_callbacks + 1
    );
    assert_eq!(first.get_player(0).unwrap().effective_supplies(), 700);
    first.attack_events.complete_boundary();
    assert!(
        first.attack_events.take_completed().is_empty(),
        "construct is not an attack receipt"
    );
    assert_eq!(peer.host_object(peer_id).unwrap().ai_state, AIState::Moving);
    assert_eq!(
        peer.host_object(peer_id).unwrap().movement.target_position,
        peer_goal
    );
    assert_eq!(
        peer.host_object(peer_id).unwrap().dozer_tasks.requests[0].target,
        None
    );
    assert_eq!(peer.get_player(0).unwrap().effective_supplies(), 1000);
    drive(&mut first, id, 11);
    assert_eq!(
        first.host_object(id).unwrap().dozer_tasks.current,
        DozerTask::Build
    );
    assert_eq!(
        system_command(
            &mut first,
            id,
            CommandType::DozerCancelConstruct { object_id: site }
        ),
        CommandResult::Success
    );
    assert_eq!(
        first.get_player(0).unwrap().effective_supplies(),
        1000,
        "explicit cancel refunds the actual paid scaffold once"
    );
    assert_eq!(peer.get_player(0).unwrap().effective_supplies(), 1000);
}

#[test]
fn actual_command_system_unaffordable_build_idles_without_factory_payment_or_request() {
    let (mut world, id, _) = world();
    world.get_player_mut(0).unwrap().resources.supplies = 299;
    assert_eq!(
        system_command(
            &mut world,
            id,
            CommandType::Move {
                destination: Vec3::new(500.0, 0.0, 0.0),
            }
        ),
        CommandResult::Success
    );
    let callbacks = world.supply_create_warehouse_registers;
    assert_eq!(
        system_command(
            &mut world,
            id,
            CommandType::Build {
                template_name: "PhasePaidSite".into(),
                location: Vec3::new(200.0, 0.0, 0.0),
            }
        ),
        CommandResult::InvalidCommand
    );
    assert_eq!(world.host_object(id).unwrap().ai_state, AIState::Idle);
    assert_eq!(
        world.host_object(id).unwrap().dozer_tasks.requests[0].target,
        None
    );
    assert_eq!(world.get_player(0).unwrap().effective_supplies(), 299);
    assert_eq!(world.supply_create_warehouse_registers, callbacks);
}

#[test]
fn fresh_scaffold_admission_rejects_foreign_birth_without_refunding_committed_factory() {
    let (mut world, id, _) = world();
    world.get_player_mut(0).unwrap().resources.supplies = 1000;
    let site = world
        .create_dozer_scaffold(
            "PhasePaidSite",
            0,
            id,
            Vec3::new(200.0, 0.0, 0.0),
            0.0,
            Resources {
                supplies: 300,
                power: 0,
            },
        )
        .expect("factory")
        .expect("site");
    // Exact lifetime mismatch; this is not numeric-ID authority or callback injection.
    let foreign_birth = std::sync::Arc::new(());
    let result = world.dozer_admit_constructed_scaffold_owned(id, &foreign_birth, site);
    assert!(matches!(
        result,
        Err(gamelogic::GameLogicError::Configuration(_))
    ));
    assert_eq!(world.get_player(0).unwrap().effective_supplies(), 700);
    assert!(world.host_object(site).unwrap().status.under_construction);
    assert_eq!(
        world.host_object(id).unwrap().dozer_tasks.requests[0].target,
        None
    );
}

#[test]
fn actual_command_system_kindof_dozer_with_base_ai_cannot_admit_a_dozer_task() {
    let mut world = GameLogic::new();
    world.frame = 10;
    world.add_player(Player::new(0, Team::USA, "WrongVirtualConstructor", true));
    world.get_player_mut(0).unwrap().resources.supplies = 1000;
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([(
        "base-ai-is-not-dozer.ini".into(),
        "Object FalseDozer\nKindOf = VEHICLE DOZER SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = AIUpdateInterface BaseAI\nEnd\nEnd\nObject FalseDozerSite\nKindOf = STRUCTURE\nBuildCost = 300\nBody = ActiveBody Body\nMaxHealth = 500\nInitialHealth = 500\nEnd\nEnd\n".into(),
    )]).expect("authored base AI")).expect("admit base AI");
    let id = world
        .create_object_for_player_with_effects_checked(
            "FalseDozer",
            0,
            Vec3::ZERO,
            None,
            &mut Default::default(),
        )
        .expect("factory")
        .expect("actor");
    let birth = std::sync::Arc::clone(&world.host_object(id).unwrap().visual_draw_identity_token);
    assert_eq!(
        system_command(
            &mut world,
            id,
            CommandType::Build {
                template_name: "FalseDozerSite".into(),
                location: Vec3::new(200.0, 0.0, 0.0),
            }
        ),
        CommandResult::InvalidCommand
    );
    assert_eq!(world.get_player(0).unwrap().effective_supplies(), 1000);
    assert_eq!(
        world.host_object(id).unwrap().dozer_tasks.requests[0].target,
        None
    );
    let site = world
        .create_object_under_construction_for_player(
            "FalseDozerSite",
            0,
            Vec3::new(200.0, 0.0, 0.0),
            0.0,
        )
        .expect("factory")
        .expect("site");
    assert!(matches!(
        world.dozer_admit_constructed_scaffold_owned(id, &birth, site),
        Err(gamelogic::GameLogicError::Configuration(_))
    ));
    assert_eq!(
        world.host_object(id).unwrap().dozer_tasks.requests[0].target,
        None
    );
}

fn action_build_world() -> (GameLogic, ObjectId, ObjectId) {
    use crate::assets::ReceivingObjectContent;
    use crate::game_logic::{AIState, Player, Resources};

    let mut logic = GameLogic::new();
    // CPP newTask records the current frame; mostRecent selects only a
    // strictly positive order frame, as checked by the frame-zero control.
    logic.frame = 10;
    logic.add_player(Player::new(0, Team::USA, "OwnedBuilder", true));
    logic
        .admit_receiving_module_content(ReceivingObjectContent::from_sources([(
            "wave715-dozer-phase.ini".into(),
            "Object SfDozer\nKindOf = VEHICLE DOZER SELECTABLE\nEnergyProduction = 1\nLocomotor = SET_NORMAL AmericaVehicleDozerLocomotor\nBody = ActiveBody Body\nMaxHealth = 200\nInitialHealth = 200\nEnd\nBehavior = DozerAIUpdate DozerTaskSource\nRepairHealthPercentPerSecond = 2\nEnd\nEnd\nObject SfBarracks\nKindOf = STRUCTURE SELECTABLE FS_SUPPLY_CENTER\nEnergyProduction = 20\nBuildCost = 0\nBuildTime = 10\nBody = ActiveBody Body\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\nBehavior = LockWeaponCreate CompletionLock\nSlotToLock = PRIMARY\nEnd\nEnd\nObject SfRebuildSite\nKindOf = STRUCTURE SELECTABLE FS_SUPPLY_CENTER SCORE_CREATE\nEnergyProduction = 20\nBuildCost = 400\nBuildTime = 10\nBody = ActiveBody Body\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\nBehavior = LockWeaponCreate CompletionLock\nSlotToLock = PRIMARY\nEnd\nEnd\n".into(),
        )])
        .expect("authored Dozer and structure content"))
        .expect("receiving module admission");
    let dozer = logic
        .create_object_for_player_with_effects_checked(
            "SfDozer",
            0,
            Vec3::ZERO,
            None,
            &mut Default::default(),
        )
        .expect("Dozer factory")
        .expect("admitted Dozer");
    let id = logic
        .create_dozer_scaffold(
            "SfBarracks",
            0,
            dozer,
            Vec3::ZERO,
            0.0,
            Resources::default(),
        )
        .expect("Dozer scaffold factory")
        .expect("admitted structure scaffold");

    // This is the real build request. Place the fixture at its authored
    // dock, then let the admitted DozerAIUpdate enter its construction
    // phase instead of writing target/current/AI state by hand.
    logic.dozer_new_task_build(dozer, id).unwrap();
    (logic, dozer, id)
}
fn action_build_first_piece(world: &mut GameLogic, builder: ObjectId, site: ObjectId, before: f32) {
    for _ in 0..600 {
        world.update_with_dt(1. / 30.);
        if world.host_object(site).unwrap().construction_percent > before {
            break;
        }
    }
    assert!(
        world.host_object(site).unwrap().construction_percent > before,
        "real full-World BUILD action contributes in its first arrived frame"
    );
    assert_eq!(
        world
            .host_object(builder)
            .unwrap()
            .dozer_tasks
            .build_sub_task,
        gamelogic::object::update::ai_update::dozer_ai_update::DozerBuildSubTask::DoBuildAtDock
    );
}

#[test]
fn owned_dozer_phase_completes_structure_without_ready_writeback() {
    use crate::game_logic::AIState;
    let (mut logic, dozer, id) = action_build_world();
    // Drive both inherited AI and movement. Updating only AI cannot finish
    // the approach to the selected ACTION dock, even if the initial fixture
    // position happened to match the previous dock candidate.
    for _ in 0..600 {
        logic.update_with_dt(1.0 / 30.0);
        let builder = logic.host_object(dozer).expect("live builder");
        if builder.dozer_tasks.build_sub_task
            == gamelogic::object::update::ai_update::dozer_ai_update::DozerBuildSubTask::DoBuildAtDock
        {
            break;
        }
    }
    let builder = logic.host_object(dozer).expect("live builder");
    assert_eq!(builder.dozer_tasks.current, DozerTask::Build);
    assert_eq!(
        builder.dozer_tasks.build_sub_task,
        gamelogic::object::update::ai_update::dozer_ai_update::DozerBuildSubTask::DoBuildAtDock,
        "real approach must reach the selected build dock"
    );
    assert_eq!(
        builder.ai_state,
        AIState::Idle,
        "CPP uses idle inherited AI while building"
    );
    assert_ne!(
        builder.model_condition_bits
            & (1u128
                << crate::game_logic::host_enum_table_residual::actively_constructing_model_bit()),
        0,
        "the actual Dozer action marks its builder actively constructing"
    );
    assert!(logic.host_object(id).unwrap().construction_percent > 0.0);
    assert!(logic.host_object(id).unwrap().status.under_construction);
    assert_eq!(logic.host_object(id).unwrap().builder_id, Some(dozer));
    // Preserve the original near-complete precondition, but reach the
    // construction phase through admitted Dozer AI before its next action
    // callbacks perform the final increments.
    logic.host_object_mut(id).unwrap().construction_percent = 0.99;
    assert_eq!(logic.host_object(id).unwrap().construction_percent, 0.99);

    for _ in 0..6 {
        logic.update_with_dt(1.0 / 30.0);
    }
    assert_eq!(logic.host_object(id).unwrap().construction_percent, 1.0);
    assert!(!logic.host_object(id).unwrap().status.under_construction);
    assert_eq!(
        logic.host_object(dozer).unwrap().dozer_tasks.requests[0].target,
        None
    );
}

#[test]
fn actual_build_completion_callbacks_finish_before_end_move_and_only_release_current_builder() {
    let (mut own, builder, site) = action_build_world();
    let (peer, peer_builder, peer_site) = action_build_world();
    assert_eq!((builder, site), (peer_builder, peer_site));
    action_build_first_piece(&mut own, builder, site, 0.);
    let other = own
        .create_object_for_player_with_effects_checked(
            "SfDozer",
            0,
            Vec3::new(120., 0., 0.),
            None,
            &mut Default::default(),
        )
        .expect("actual second constructor")
        .unwrap();
    own.dozer_new_task_build(other, site).unwrap();
    let other_request = own.host_object(other).unwrap().dozer_tasks.requests[0].clone();
    // Preserve the established near-complete precondition; contribution and
    // completion still execute the same production action operation.
    own.host_object_mut(site).unwrap().construction_percent = 0.9999;
    let mut observed = false;
    assert!(
        own.fixture_dozer_build_frame_with_completion_observer(
            builder,
            site,
            &mut Default::default(),
            |world, builder, site| {
                observed = true;
                let actor = world.host_object(builder).unwrap();
                assert_eq!(actor.dozer_tasks.current, DozerTask::Build);
                assert_eq!(actor.dozer_tasks.requests[0].target, Some(site));
                assert_eq!(actor.ai_state, AIState::Idle, "end move has not run yet");
                let target = world.host_object(site).unwrap();
                assert_eq!(target.producer_id, Some(builder));
                assert_eq!(target.builder_id, Some(builder));
                assert_eq!(target.construction_percent, 1.);
                assert!(!target.status.under_construction);
                assert!(
                    target.health.current < target.health.maximum,
                    "completion never replaces damaged scaffold health with max"
                );
                assert!(
                    target.construction_power_admitted,
                    "Player callback finished"
                );
                assert_eq!(
                    world.get_player(0).unwrap().power_produced,
                    22,
                    "two actual1-power constructors plus completed20-power site"
                );
                assert!(
                    world
                        .get_player(0)
                        .unwrap()
                        .academy_stats
                        .had_a_supply_center(),
                    "Academy before Create/end move"
                );
                assert_eq!(
                    target.weapon_lock_type,
                    crate::game_logic::WeaponLockType::LockedPermanently,
                    "actual LockWeaponCreate callback finished"
                );
                assert_eq!(
                    world.structure_complete_events, 1,
                    "UI completion before end move"
                );
                assert_eq!(
                    world.radar_construction_events, 1,
                    "radar completion before end move"
                );
                assert_eq!(
                    world.host_object(other).unwrap().dozer_tasks.requests[0].target,
                    Some(site)
                );
            }
        )
        .expect("same actual action completion")
    );
    assert!(observed);
    assert_eq!(
        own.host_object(builder).unwrap().dozer_tasks.current,
        DozerTask::Invalid
    );
    assert_eq!(
        own.host_object(builder).unwrap().dozer_tasks.requests[0].target,
        None
    );
    assert_eq!(
        own.host_object(other).unwrap().dozer_tasks.requests[0].target,
        other_request.target
    );
    assert_eq!(
        own.host_object(other).unwrap().dozer_tasks.requests[0].order_frame,
        other_request.order_frame
    );
    assert_eq!(
        peer.host_object(peer_site).unwrap().construction_percent,
        0.
    );
    assert_eq!(
        peer.host_object(peer_builder).unwrap().dozer_tasks.requests[0].target,
        Some(peer_site)
    );
    assert_eq!(peer.structure_complete_events, 0);
}
#[test]
fn actual_build_native_mid_action_resumes_progress_without_production_timer_contribution() {
    use crate::save_load::{XferData, XferLoad, XferSave};
    let (mut source, builder, site) = action_build_world();
    action_build_first_piece(&mut source, builder, site, 0.);
    let before = source.host_object(site).unwrap().construction_percent;
    source
        .update_construction(&[site, builder], 1.)
        .expect("independent timer/door tick");
    assert_eq!(
        source.host_object(site).unwrap().construction_percent,
        before,
        "timer batch no longer contributes"
    );
    let factory = crate::save_load::snapshot::SnapshotBuilder::new();
    let mut snapshot = factory.create_world_snapshot(&source).unwrap();
    let mut bytes = std::io::Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut bytes)).unwrap();
    let mut decoded = crate::save_load::snapshot::WorldSnapshot::default();
    decoded
        .xfer(&mut XferLoad::new(std::io::Cursor::new(bytes.into_inner())))
        .unwrap();
    let (mut restored, restored_builder, restored_site) = action_build_world();
    assert_eq!((builder, site), (restored_builder, restored_site));
    factory
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(
        restored.host_object(site).unwrap().construction_percent,
        before
    );
    action_build_first_piece(&mut restored, builder, site, before);
    assert_eq!(
        source.host_object(site).unwrap().construction_percent,
        before,
        "sameID source remains at saved piece"
    );
    assert_eq!(
        restored.host_object(site).unwrap().producer_id,
        Some(builder)
    );
}
#[test]
fn actual_construct_latch_is_builder_owned_survives_resume_and_is_omitted_from_original_native_wire()
 {
    use crate::save_load::{XferData, XferLoad, XferSave};
    fn bytes(world: &GameLogic) -> Vec<u8> {
        let mut snapshot = crate::save_load::snapshot::SnapshotBuilder::new()
            .create_world_snapshot(world)
            .unwrap();
        let mut out = std::io::Cursor::new(Vec::new());
        snapshot.xfer(&mut XferSave::new(&mut out)).unwrap();
        out.into_inner()
    }
    let (mut source, builder, site) = action_build_world();
    let (peer, peer_builder, _) = action_build_world();
    assert_eq!(builder, peer_builder);
    let birth = std::sync::Arc::clone(
        &source
            .host_object(builder)
            .unwrap()
            .visual_draw_identity_token,
    );
    let foreign = std::sync::Arc::clone(
        &peer
            .host_object(peer_builder)
            .unwrap()
            .visual_draw_identity_token,
    );
    assert!(
        source
            .dozer_record_construct_kind_owned(builder, &foreign, true)
            .is_err()
    );
    assert!(
        !source
            .host_object(builder)
            .unwrap()
            .dozer_tasks
            .construct_is_rebuild
    );
    source
        .host_object_mut(site)
        .unwrap()
        .set_status_reconstructing(true);
    let original = bytes(&source);
    // Same operation is called before RebuildHole's actual construct factory.
    source
        .dozer_record_construct_kind_owned(builder, &birth, true)
        .unwrap();
    assert!(
        source
            .host_object(builder)
            .unwrap()
            .dozer_tasks
            .construct_is_rebuild
    );
    assert_eq!(
        bytes(&source),
        original,
        "CPP Dozer/Worker Xfer omits m_isRebuild: no invented current-native field"
    );
    source.dozer_internal_task_complete(builder, false);
    source.dozer_primary_reset_owned(builder);
    source
        .host_object_mut(site)
        .unwrap()
        .set_status_reconstructing(false);
    assert!(source.resume_construction(&[builder], site).unwrap());
    assert!(
        source
            .host_object(builder)
            .unwrap()
            .dozer_tasks
            .construct_is_rebuild,
        "aiResumeConstruction/newTask preserve previous construct latch even on ordinary scaffold"
    );
    source
        .dozer_prepare_construct_owned_with_effects(builder, &mut Default::default())
        .unwrap()
        .unwrap();
    assert!(
        !source
            .host_object(builder)
            .unwrap()
            .dozer_tasks
            .construct_is_rebuild,
        "next fresh construct entry resets false before factory"
    );
    let mut decoded = crate::save_load::snapshot::WorldSnapshot::default();
    decoded
        .xfer(&mut XferLoad::new(std::io::Cursor::new(original)))
        .unwrap();
    let (mut receiving, _, _) = action_build_world();
    crate::save_load::snapshot::SnapshotBuilder::new()
        .restore_from_snapshot(&decoded, &mut receiving)
        .unwrap();
    assert!(receiving.host_object(site).unwrap().status.reconstructing);
    assert!(
        !receiving
            .host_object(builder)
            .unwrap()
            .dozer_tasks
            .construct_is_rebuild,
        "fresh receiving constructor retains false; site status never repairs an omitted source latch"
    );
    assert!(
        !peer
            .host_object(peer_builder)
            .unwrap()
            .dozer_tasks
            .construct_is_rebuild
    );
}

#[test]
fn actual_entered_build_rejects_changed_controller_without_canceling_pending_request() {
    let (mut world, builder, site) = action_build_world();
    action_build_first_piece(&mut world, builder, site, 0.);
    world.add_player(Player::new(1, Team::China, "OtherController", false));
    let team = {
        let mut factory = world.team_factory.lock().unwrap();
        factory
            .init_team_owned(
                "ForeignBuildController".into(),
                "OtherController".into(),
                false,
                None,
                Some(1),
            )
            .unwrap();
        let team = factory.create_team("ForeignBuildController").unwrap();
        let id = team.read().unwrap().get_id();
        id
    };
    assert!(
        world
            .set_host_object_team(site, team)
            .expect("actual setTeam callback")
    );
    let before = (
        world.host_object(site).unwrap().construction_percent,
        world.host_object(site).unwrap().health.current,
    );
    world.frame = world.frame.wrapping_add(1);
    world
        .dozer_update_primary_owned(builder)
        .expect("source BUILD controller failure");
    assert_eq!(
        (
            world.host_object(site).unwrap().construction_percent,
            world.host_object(site).unwrap().health.current
        ),
        before
    );
    assert_eq!(
        world.host_object(builder).unwrap().dozer_tasks.current,
        DozerTask::Invalid
    );
    assert_eq!(
        world.host_object(builder).unwrap().dozer_tasks.requests[0].target,
        Some(site),
        "STATE_FAILURE resets primary; it does not invent internalTaskComplete/cancel for the pending request"
    );
}

#[test]
fn actual_rebuild_construct_supplies_birth_flags_before_factory_and_uses_builder_latch_for_completion()
 {
    let (mut world, builder, abandoned) = action_build_world();
    world
        .destroy_object(abandoned)
        .expect("retire unused ordinary scaffold");
    world.process_destroy_list().expect("physical disposal");
    world.set_scoring_enabled(true);
    let available = world.get_player(0).unwrap().effective_supplies();
    world
        .players
        .get_mut(&0)
        .unwrap()
        .withdraw_construction_money(available, &mut world.economy_events);
    let cash = world.get_player(0).unwrap().effective_supplies();
    assert_eq!(
        cash, 0,
        "rebuild is admitted despite unaffordable authored400 cost"
    );
    let power = world.get_player(0).unwrap().power_produced;
    let mut observed = false;
    let site = world
        .fixture_create_rebuild_scaffold_observing(
            "SfRebuildSite",
            Team::USA,
            Some(0),
            builder,
            Vec3::new(100., 0., 100.),
            |world, site| {
                observed = true;
                assert!(
                    world
                        .host_object(builder)
                        .unwrap()
                        .dozer_tasks
                        .construct_is_rebuild,
                    "actual construct entry precedes ThingFactory callbacks"
                );
                let target = world.host_object(site).unwrap();
                assert!(target.status.under_construction);
                assert!(
                    target.status.reconstructing,
                    "CPP1667 supplies both bits before onCreate"
                );
                assert_eq!(
                    target.construction_percent, 1.,
                    "authored initial factory state before construct tail"
                );
                assert_eq!(target.health.current, 1000.);
                assert_eq!(target.builder_id, None);
                assert_eq!(target.producer_id, None);
                assert_eq!(
                    target.weapon_lock_type,
                    crate::game_logic::WeaponLockType::NotLocked,
                    "onBuildComplete has not run at birth"
                );
                assert!(!target.construction_power_admitted);
                assert_eq!(
                    world.get_player(0).unwrap().power_produced,
                    power,
                    "no premature constructor power then late status correction"
                );
            },
        )
        .expect("actual rebuild construct")
        .expect("rebuild bypasses normal cost/prerequisite refusal");
    assert!(observed);
    assert_eq!(world.host_object(site).unwrap().construction_percent, 0.);
    assert_eq!(world.host_object(site).unwrap().health.current, 1.);
    assert_eq!(world.host_object(site).unwrap().builder_id, Some(builder));
    assert_eq!(
        world.host_object(builder).unwrap().dozer_tasks.requests[0].target,
        Some(site)
    );
    assert_eq!(
        world.get_player(0).unwrap().effective_supplies(),
        cash,
        "rebuild never withdraws400 authored cost"
    );
    action_build_first_piece(&mut world, builder, site, 0.);
    world.host_object_mut(site).unwrap().construction_percent = 0.9999;
    assert!(
        world
            .dozer_build_action_frame_owned(builder, site, &mut Default::default())
            .expect("actual rebuilding completion")
    );
    assert!(!world.host_object(site).unwrap().status.reconstructing);
    assert_eq!(
        world.get_player(0).unwrap().statistics.structures_built(),
        0,
        "captured builder construct latch survives site status clear and skips rebuild score"
    );
    assert_eq!(
        world.host_object(site).unwrap().weapon_lock_type,
        crate::game_logic::WeaponLockType::LockedPermanently
    );
    assert_eq!(world.get_player(0).unwrap().power_produced, power + 20);
    assert!(
        world
            .host_object(builder)
            .unwrap()
            .dozer_tasks
            .construct_is_rebuild,
        "internalTaskComplete does not reset the construct latch"
    );
}

#[test]
fn invalid_admitted_primary_stops_real_command_flush_and_tick_before_next_command() {
    let (mut owner, constructor, _) = world();
    let (peer, peer_constructor, _) = world();
    assert_eq!(constructor, peer_constructor);
    owner.get_player_mut(0).unwrap().resources.supplies = 1000;
    // A damaged continuation must fail at the actual virtual construct boundary;
    // it must not turn into ordinary placement refusal or run the next command.
    owner
        .host_object_mut(constructor)
        .unwrap()
        .dozer_tasks
        .primary
        .control
        .initialized = false;
    let build = GameCommand {
        command_type: CommandType::Build {
            template_name: "PhasePaidSite".into(),
            location: Vec3::new(200., 0., 0.),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![constructor],
        modifier_keys: ModifierKeys::default(),
    };
    let next = GameCommand {
        command_type: CommandType::Move {
            destination: Vec3::new(900., 0., 0.),
        },
        command_id: 2,
        ..build.clone()
    };
    let count = owner.host_objects().registered_len();
    owner.queue_command(build.clone());
    owner.queue_command(next.clone());
    assert!(matches!(
        owner.process_commands(),
        Err(gamelogic::GameLogicError::Configuration(_))
    ));
    assert_eq!(owner.command_queue.len(), 1);
    assert_eq!(owner.host_objects().registered_len(), count);
    assert_eq!(owner.get_player(0).unwrap().effective_supplies(), 1000);
    assert_eq!(
        owner.host_object(constructor).unwrap().ai_state,
        AIState::Idle
    );
    assert_ne!(
        owner
            .host_object(constructor)
            .unwrap()
            .movement
            .target_position,
        Some(Vec3::new(900., 0., 0.))
    );
    assert!(
        peer.host_object(peer_constructor)
            .unwrap()
            .dozer_tasks
            .primary
            .control
            .initialized
    );
    assert!(peer.command_queue.is_empty());

    let (mut tick, constructor, _) = world();
    tick.get_player_mut(0).unwrap().resources.supplies = 1000;
    tick.host_object_mut(constructor)
        .unwrap()
        .dozer_tasks
        .primary
        .control
        .initialized = false;
    tick.queue_command(build);
    tick.queue_command(next);
    let frame = tick.frame;
    assert!(matches!(
        tick.tick_logic_frame(1. / 30., None, Some(1)),
        Err(gamelogic::GameLogicError::Configuration(_))
    ));
    assert_eq!(tick.frame, frame);
    assert_eq!(tick.command_queue.len(), 1);
    assert_eq!(tick.get_player(0).unwrap().effective_supplies(), 1000);
    assert_ne!(
        tick.host_object(constructor)
            .unwrap()
            .movement
            .target_position,
        Some(Vec3::new(900., 0., 0.))
    );
}
