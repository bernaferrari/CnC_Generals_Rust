//! Actual admitted AI commands and receiving native continuation.
use crate::game_logic::{AIState, GameLogic, ObjectId, Player, Team};
use glam::Vec3;
fn world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([("temporary-owner.ini".into(),"Object TemporaryOwner\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = AIUpdateInterface FirstAI\nEnd\nWeaponSet\nConditions = None\nWeapon = PRIMARY TemporaryOwnerGun\nEnd\nEnd\n".into())]).unwrap()).unwrap();
    let mut definitions = std::collections::HashMap::new();
    assert_eq!(
        crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
            &mut definitions,
            "Weapon TemporaryOwnerGun\nPrimaryDamage = 1\nAttackRange = 200\nClipSize = 10\nAutoReloadsClip = Yes\nDelayBetweenShots = 100\nEnd\n",
            crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal
        ),
        1
    );
    world
        .admit_weapon_content_snapshot(
            definitions
                .into_values()
                .map(|w| (w.name.clone(), w))
                .collect(),
            &[],
        )
        .unwrap();
    world.add_player(Player::new(1, Team::USA, "Owner", true));
    world.add_player(Player::new(2, Team::China, "Enemy", false));
    let actor = world
        .create_object_for_player("TemporaryOwner", 1, Vec3::ZERO)
        .unwrap();
    let enemy = world
        .create_object_for_player("TemporaryOwner", 2, Vec3::new(10., 0., 0.))
        .unwrap();
    (world, actor, enemy)
}
fn observe(world: &mut GameLogic, id: ObjectId, frame: u32) {
    let (actor, audio) = world.host_object_and_audio_events_mut(id).unwrap();
    actor.tick_path_queue(frame, &mut crate::game_logic::ObjectAudio::queue(audio));
}
fn bytes(world: &GameLogic, id: ObjectId) -> Vec<u8> {
    serde_json::to_vec(
        &world
            .host_object(id)
            .unwrap()
            .capture_base_ai_continuation(),
    )
    .unwrap()
}
#[test]
fn actual_ai_diversion_retains_base_body_through_strict_deadline_and_peer_reset() {
    let (mut world, id, _) = world();
    let (mut peer, pid, _) = self::world();
    assert_eq!(id, pid);
    let base = Vec3::new(100., 0., 50.);
    assert!(world.unit_command_ai_move_to(id, base));
    assert_eq!(
        world
            .host_object(id)
            .unwrap()
            .capture_base_ai_continuation()
            .goals
            .position,
        base
    );
    let saved = world
        .host_object(id)
        .map(|a| (a.requested_destination, a.path_goal_position))
        .unwrap();
    let normal_goal = world
        .host_object(id)
        .unwrap()
        .capture_base_ai_continuation()
        .normal_move_goal;
    assert!(normal_goal.is_some());
    let peer_before = bytes(&peer, pid);
    world.frame = 10;
    assert!(world.unit_command_ai_move_to(id, Vec3::new(900., 0., 800.)));
    let actor = world.host_object(id).unwrap();
    assert_eq!(
        actor
            .capture_base_ai_continuation()
            .temporary_move
            .as_ref()
            .unwrap()
            .deadline,
        610
    );
    assert_eq!(
        actor.capture_base_ai_continuation().goals.position,
        Vec3::new(900., 0., 800.)
    );
    for _ in 0..3 {
        observe(&mut world, id, 610);
    }
    assert!(world.host_object(id).unwrap().has_temporary_ai_move());
    observe(&mut world, id, 611);
    let actor = world.host_object(id).unwrap();
    assert!(!actor.has_temporary_ai_move());
    assert_eq!(actor.ai_state(), AIState::Moving);
    assert_eq!(
        actor.capture_base_ai_continuation().normal_move_goal,
        normal_goal
    );
    assert_eq!(
        (actor.requested_destination, actor.path_goal_position),
        saved
    );
    assert_eq!(
        actor.capture_base_ai_continuation().goals.position,
        Vec3::new(900., 0., 800.)
    );
    assert_eq!(bytes(&peer, pid), peer_before);
    world.reset();
    assert_eq!(bytes(&peer, pid), peer_before);
}
#[test]
fn actual_attack_diversion_restores_absent_physical_goals() {
    let (mut world, id, enemy) = world();
    assert!(world.unit_command_attack(id, enemy));
    let saved = world
        .host_object(id)
        .map(|a| (a.requested_destination, a.path_goal_position))
        .unwrap();
    assert_eq!(saved, (None, None));
    assert!(world.unit_command_ai_move_to(id, Vec3::new(700., 0., 600.)));
    let actor = world.host_object(id).unwrap();
    assert!(actor.has_temporary_ai_move());
    assert_eq!(actor.ai_state(), AIState::Attacking);
    observe(&mut world, id, 601);
    let actor = world.host_object(id).unwrap();
    assert!(!actor.has_temporary_ai_move());
    assert_eq!(
        (actor.requested_destination, actor.path_goal_position),
        (None, None)
    );
    assert_eq!(actor.ai_state(), AIState::Attacking);
}
#[test]
fn replacement_and_native_restore_keep_original_base_receipt() {
    use crate::save_load::snapshot::base_ai_machine_persist::BaseAiObjectSnapshot;
    use crate::save_load::{XferData, XferLoad, XferSave};
    let (mut source, id, _) = world();
    let (mut receiving, rid, _) = world();
    let (peer, pid, _) = world();
    assert_eq!((id, id), (rid, pid));
    assert!(source.unit_command_ai_move_to(id, Vec3::new(100., 0., 50.)));
    let saved = source
        .host_object(id)
        .map(|a| (a.requested_destination, a.path_goal_position))
        .unwrap();
    source.frame = 20;
    assert!(source.unit_command_ai_move_to(id, Vec3::new(900., 0., 800.)));
    source.frame = 40;
    assert!(source.unit_command_ai_move_to(id, Vec3::new(800., 0., 900.)));
    let control = source
        .host_object(id)
        .unwrap()
        .capture_base_ai_continuation();
    let temporary = control.temporary_move.as_ref().unwrap();
    assert_eq!(temporary.deadline, 640);
    assert_eq!(
        (temporary.saved_destination, temporary.saved_path_goal),
        saved
    );
    let peer_before = bytes(&peer, pid);
    let mut capture = BaseAiObjectSnapshot::capture(source.host_object(id).unwrap()).unwrap();
    let mut wire = std::io::Cursor::new(Vec::new());
    capture.xfer(&mut XferSave::new(&mut wire)).unwrap();
    let mut loaded = BaseAiObjectSnapshot::default();
    loaded
        .xfer(&mut XferLoad::new(std::io::Cursor::new(wire.into_inner())))
        .unwrap();
    loaded
        .apply_after_admission(receiving.host_object_mut(rid).unwrap())
        .unwrap();
    assert_eq!(bytes(&receiving, rid), bytes(&source, id));
    observe(&mut receiving, rid, 641);
    let actor = receiving.host_object(rid).unwrap();
    assert_eq!(
        (actor.requested_destination, actor.path_goal_position),
        saved
    );
    assert!(!actor.has_temporary_ai_move());
    assert!(source.host_object(id).unwrap().has_temporary_ai_move());
    assert_eq!(bytes(&peer, pid), peer_before);
}
#[test]
fn actual_nested_attack_completion_resumes_existing_guard_body_and_goals() {
    let (mut world, id, enemy) = world();
    assert!(world.unit_command_guard_position(id, Vec3::new(40., 0., 20.)));
    let before = world
        .host_object(id)
        .unwrap()
        .capture_base_ai_continuation();
    assert_eq!(
        world.attack_state_enter(id, enemy),
        crate::game_logic::AttackMachineResult::Continue
    );
    let child = world
        .host_object(id)
        .unwrap()
        .capture_base_ai_continuation();
    assert_eq!(child.ai_state, AIState::Attacking);
    assert_eq!(child.parent_state(), Some(before.ai_state));
    assert_eq!(child.goals.object, Some(enemy));
    let (actor, audio) = world.host_object_and_audio_events_mut(id).unwrap();
    actor.stop_attack(&mut crate::game_logic::ObjectAudio::queue(audio));
    let after = world
        .host_object(id)
        .unwrap()
        .capture_base_ai_continuation();
    assert_eq!(after.ai_state, before.ai_state);
    assert_eq!(after.parent_state(), None);
    assert_eq!(after.guard, before.guard);
    assert_eq!(after.goals, before.goals);
}

#[test]
fn expired_but_unobserved_actual_overlay_saves_and_resumes_on_receiving_owner() {
    use crate::save_load::{XferData, XferLoad, XferSave};
    let (mut source, id, _) = world();
    let (mut receiving, receiving_id, _) = world();
    let (peer, peer_id, _) = world();
    assert_eq!((id, id), (receiving_id, peer_id));
    source.frame = 1;
    assert!(source.unit_command_ai_move_to(id, Vec3::new(100., 0., 50.)));
    source.frame = 2;
    assert!(source.unit_command_ai_move_to(id, Vec3::new(900., 0., 800.)));
    source.frame = 603;
    assert!(source.host_object(id).unwrap().has_temporary_ai_move());
    assert_eq!(
        source
            .host_object(id)
            .unwrap()
            .temporary_ai_move_remaining(source.frame),
        0
    );
    let before = bytes(&source, id);
    let peer_before = bytes(&peer, peer_id);
    let factory = crate::save_load::snapshot::SnapshotBuilder::new();
    let mut snapshot = factory.create_world_snapshot(&source).unwrap();
    let mut wire = std::io::Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut wire)).unwrap();
    let mut decoded = crate::save_load::snapshot::WorldSnapshot::default();
    decoded
        .xfer(&mut XferLoad::new(std::io::Cursor::new(wire.into_inner())))
        .unwrap();
    factory
        .restore_from_snapshot(&decoded, &mut receiving)
        .unwrap();
    assert_eq!(bytes(&receiving, receiving_id), before);
    observe(&mut receiving, receiving_id, 603);
    observe(&mut source, id, 603);
    assert!(
        !receiving
            .host_object(receiving_id)
            .unwrap()
            .has_temporary_ai_move()
    );
    assert_eq!(bytes(&receiving, receiving_id), bytes(&source, id));
    assert_eq!(bytes(&peer, peer_id), peer_before);
}
