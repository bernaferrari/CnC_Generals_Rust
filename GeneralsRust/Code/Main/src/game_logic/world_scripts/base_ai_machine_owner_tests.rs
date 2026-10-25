//! Commands enter the actual admitted Jet provider; no mutable FSM seeding.
use crate::game_logic::{AIState, GameLogic, ObjectId, Player, Team};
use glam::Vec3;

fn jet(helipad: bool) -> (GameLogic, ObjectId) {
    let kind = if helipad {
        "VEHICLE AIRCRAFT SELECTABLE PRODUCED_AT_HELIPAD"
    } else {
        "VEHICLE AIRCRAFT SELECTABLE"
    };
    let content = format!(
        "Object JetIdleOwner\n KindOf = {kind}\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = JetAIUpdate FirstJet\n End\nEnd\n"
    );
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "jet-idle-owner.ini".into(),
                content,
            )])
            .unwrap(),
        )
        .unwrap();
    world.add_player(Player::new(1, Team::USA, "JetOwner", true));
    let id = world
        .create_object_for_player("JetIdleOwner", 1, Vec3::ZERO)
        .unwrap();
    (world, id)
}
fn control(world: &GameLogic, id: ObjectId) -> Vec<u8> {
    serde_json::to_vec(
        &world
            .host_object(id)
            .unwrap()
            .capture_base_ai_continuation(),
    )
    .unwrap()
}
#[test]
fn airborne_jet_idle_exits_guard_before_return_and_does_not_clear_same_id_peer() {
    let (mut world, id) = jet(false);
    let (mut peer, peer_id) = jet(false);
    assert_eq!(id, peer_id);
    let anchor = Vec3::new(70., 0., 30.);
    assert!(world.unit_command_guard_position(id, anchor));
    assert!(peer.unit_command_guard_position(peer_id, Vec3::new(90., 0., 10.)));
    let peer_before = control(&peer, peer_id);
    let raw_guard = world.host_object(id).unwrap().guard_position();
    // Physical airborne classification is independent of retained FSM control.
    world.host_object_mut(id).unwrap().status.airborne_target = true;
    assert!(
        world
            .unit_command_ai_idle_owned(id)
            .expect("receiving Idle succeeds")
    );
    let actor = world.host_object(id).unwrap();
    assert!(actor.return_to_base_requested);
    assert_eq!(
        actor.last_command_source,
        crate::game_logic::HUNT_CMD_FROM_AI
    );
    assert!(actor.active_guard_position().is_none());
    assert_eq!(
        actor.guard_position(),
        raw_guard,
        "clearGuardTargetType preserves inactive raw source coordinates"
    );
    assert_ne!(actor.retained_ai_root_state(), AIState::GuardingArea);
    assert_eq!(control(&peer, peer_id), peer_before);
    world.reset();
    assert_eq!(control(&peer, peer_id), peer_before);
}
#[test]
fn grounded_and_helipad_jet_idle_use_base_idle_entry() {
    for helipad in [false, true] {
        let (mut world, id) = jet(helipad);
        assert!(world.unit_command_guard_position(id, Vec3::new(50., 0., 20.)));
        world.frame = 73;
        world.host_object_mut(id).unwrap().status.airborne_target = helipad;
        assert!(
            world
                .unit_command_ai_idle_owned(id)
                .expect("receiving Idle succeeds")
        );
        let actor = world.host_object(id).unwrap();
        assert_eq!(actor.ai_state(), AIState::Idle);
        assert!(!actor.return_to_base_requested);
        assert!(actor.active_guard_position().is_none());
        assert_eq!(
            actor.next_mood_check_time,
            73u32.wrapping_add(world.ai_definitions.data().force_idle_frames_count)
        );
        assert!(actor.randomly_offset_mood_check);
        assert!(
            !actor.idle_mood_reset_pending,
            "onEnter resets mood immediately, not in a later object batch"
        );
    }
}
#[test]
fn jet_takeoff_idle_records_pending_command_without_exiting_current_guard() {
    let (mut world, id) = jet(false);
    assert!(world.unit_command_guard_position(id, Vec3::new(80., 0., 40.)));
    // Enter the actual runway continuation, rather than writing pending flags.
    world.host_object_mut(id).unwrap().begin_jet_runway_takeoff(
        0,
        Vec3::new(100., 0., 0.),
        100.,
        false,
    );
    let before = control(&world, id);
    let old_source = world.host_object(id).unwrap().last_command_source;
    assert!(
        world
            .unit_command_ai_idle_owned(id)
            .expect("receiving Idle succeeds")
    );
    let actor = world.host_object(id).unwrap();
    assert!(actor.jet_ai.has_pending_command);
    assert_eq!(
        actor.jet_ai.pending_resume,
        crate::game_logic::object::HostJetPendingResume::AiIdle
    );
    assert_eq!(actor.last_command_source, old_source);
    assert_eq!(control(&world, id), before);
}
#[test]
fn idle_machine_native_capsule_retains_entry_without_reentering_or_touching_peer() {
    use crate::save_load::snapshot::base_ai_machine_persist::BaseAiObjectSnapshot;
    use crate::save_load::{XferData, XferLoad, XferSave};
    let (mut world, id) = jet(true);
    assert!(world.unit_command_guard_position(id, Vec3::new(40., 0., 15.)));
    assert!(
        world
            .unit_command_ai_idle_owned(id)
            .expect("receiving Idle succeeds")
    );
    let before = control(&world, id);
    let mut snapshot = BaseAiObjectSnapshot::capture(world.host_object(id).unwrap()).unwrap();
    let mut bytes = std::io::Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut bytes)).unwrap();
    let mut loaded = BaseAiObjectSnapshot::default();
    loaded
        .xfer(&mut XferLoad::new(std::io::Cursor::new(bytes.into_inner())))
        .unwrap();
    let (mut receiving, rid) = jet(true);
    let (mut peer, pid) = jet(true);
    assert_eq!(id, rid);
    assert_eq!(id, pid);
    assert!(peer.unit_command_guard_position(pid, Vec3::new(5., 0., 95.)));
    let peer_before = control(&peer, pid);
    loaded
        .apply_after_admission(receiving.host_object_mut(rid).unwrap())
        .unwrap();
    assert_eq!(control(&receiving, rid), before);
    assert_eq!(control(&peer, pid), peer_before);
    assert_eq!(control(&world, id), before);
}

fn parking_pair(airborne: bool) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    let source = "Object JetIdlePad\nKindOf = STRUCTURE FS_AIRFIELD\nBody = ActiveBody Body\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\nBehavior = ParkingPlaceBehavior Parking\nNumRows = 1\nNumCols = 1\nHasRunways = Yes\nParkInHangars = No\nEnd\nEnd\nObject JetIdleArmed\nKindOf = VEHICLE AIRCRAFT SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = JetAIUpdate Jet\nNeedsRunway = Yes\nEnd\nWeaponSet\nConditions = None\nWeapon = PRIMARY RaptorJetMissileWeapon\nEnd\nEnd\n";
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "jet-idle-parking.ini".into(),
                source.into(),
            )])
            .unwrap(),
        )
        .unwrap();
    let mut weapons = std::collections::HashMap::new();
    assert_eq!(
        crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
            &mut weapons,
            "Weapon RaptorJetMissileWeapon\nPrimaryDamage = 1\nAttackRange = 200\nClipSize = 2\nAutoReloadsClip = RETURN_TO_BASE\nClipReloadTime = 1000\nDelayBetweenShots = 0\nEnd\n",
            crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal
        ),
        1
    );
    world
        .admit_weapon_content_snapshot(
            weapons.into_values().map(|w| (w.name.clone(), w)).collect(),
            &[],
        )
        .unwrap();
    world.add_player(Player::new(1, Team::USA, "JetParkingOwner", true));
    let pad = world
        .create_object_for_player("JetIdlePad", 1, Vec3::ZERO)
        .unwrap();
    let pos = if airborne {
        Vec3::new(500., 100., 500.)
    } else {
        Vec3::ZERO
    };
    let jet = world
        .create_object_for_player("JetIdleArmed", 1, pos)
        .unwrap();
    let actor = world.host_object_mut(jet).unwrap();
    actor.status.airborne_target = airborne;
    // Actual shot-consumption primitive; reload state/deadline is never seeded.
    let weapon = actor
        .weapon
        .as_mut()
        .expect("authored admitted primary weapon");
    crate::game_logic::Object::consume_ammo_on_fire(weapon, 0.);
    crate::game_logic::Object::consume_ammo_on_fire(weapon, 0.);
    assert_eq!(actor.weapon.as_ref().unwrap().ammo, Some(0));
    (world, jet, pad)
}
#[test]
fn actual_landing_idle_records_pending_without_exiting_landing_body() {
    let (mut world, id, pad) = parking_pair(true);
    assert!(world.do_jet_landing_command(id, pad));
    assert!(world.host_object(id).unwrap().jet_ai.landing_in_progress);
    let before = control(&world, id);
    let phase = world.host_object(id).unwrap().jet_ai.rtb_landing_phase;
    let source = world.host_object(id).unwrap().last_command_source;
    assert!(
        world
            .unit_command_ai_idle_owned(id)
            .expect("receiving Idle succeeds")
    );
    let actor = world.host_object(id).unwrap();
    assert!(actor.jet_ai.has_pending_command);
    assert_eq!(
        actor.jet_ai.pending_resume,
        crate::game_logic::object::HostJetPendingResume::AiIdle
    );
    assert_eq!(actor.jet_ai.rtb_landing_phase, phase);
    assert_eq!(actor.last_command_source, source);
    assert_eq!(control(&world, id), before);
}
#[test]
fn actual_parked_reload_idle_preserves_deadline_and_clip_progress() {
    let (mut world, id, pad) = parking_pair(false);
    assert!(world.do_jet_landing_command(id, pad));
    let actor = world.host_object(id).unwrap();
    assert_eq!(actor.contained_by, Some(pad));
    let deadline = actor
        .airfield_rearm_ready_frame
        .expect("actual landing entered reload");
    let ammo = actor.weapon.as_ref().unwrap().ammo;
    let before = control(&world, id);
    assert!(
        world
            .unit_command_ai_idle_owned(id)
            .expect("receiving Idle succeeds")
    );
    let actor = world.host_object(id).unwrap();
    assert!(actor.jet_ai.has_pending_command);
    assert_eq!(
        actor.jet_ai.pending_resume,
        crate::game_logic::object::HostJetPendingResume::AiIdle
    );
    assert_eq!(actor.airfield_rearm_ready_frame, Some(deadline));
    assert_eq!(actor.weapon.as_ref().unwrap().ammo, ammo);
    assert_eq!(control(&world, id), before);
}
#[test]
fn no_airfield_idle_enters_dead_return_move_in_the_command_call() {
    let (mut world, id) = jet(false);
    assert!(world.unit_command_guard_position(id, Vec3::new(30., 0., 40.)));
    world.host_object_mut(id).unwrap().status.airborne_target = true;
    assert!(
        world
            .unit_command_ai_idle_owned(id)
            .expect("receiving Idle succeeds")
    );
    let actor = world.host_object(id).unwrap();
    assert_eq!(
        actor.retained_ai_root_state(),
        AIState::JetReturnToDeadAirfield
    );
    assert_eq!(actor.ai_state(), AIState::Moving);
    let goal = actor.jet_producer_location_vec().unwrap();
    assert_eq!(actor.capture_base_ai_continuation().goals.position, goal);
    assert!(actor.status.moving);
    assert!(
        actor.waiting_for_path || actor.movement.target_position.is_some(),
        "entry installs actual path continuation before command returns"
    );
    assert_eq!(actor.producer_id, None);
    use crate::save_load::snapshot::base_ai_machine_persist::BaseAiObjectSnapshot;
    use crate::save_load::{XferData, XferLoad, XferSave};
    let before = control(&world, id);
    let mut snapshot = BaseAiObjectSnapshot::capture(world.host_object(id).unwrap()).unwrap();
    let mut bytes = std::io::Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut bytes)).unwrap();
    let mut decoded = BaseAiObjectSnapshot::default();
    decoded
        .xfer(&mut XferLoad::new(std::io::Cursor::new(bytes.into_inner())))
        .unwrap();
    let (mut receiving, rid) = jet(false);
    decoded
        .apply_after_admission(receiving.host_object_mut(rid).unwrap())
        .unwrap();
    assert_eq!(control(&receiving, rid), before);
    assert_eq!(
        receiving.host_object(rid).unwrap().retained_ai_root_state(),
        AIState::JetReturnToDeadAirfield
    );
}
#[test]
fn attack_move_clear_resets_nested_idle_mood_before_parent_move_exit() {
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([("attack-move-clear.ini".into(), "Object ClearAttackMove\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = AIUpdateInterface FirstAI\nEnd\nEnd\n".into())]).unwrap()).unwrap();
    world.add_player(Player::new(1, Team::USA, "AttackMoveOwner", true));
    let id = world
        .create_object_for_player("ClearAttackMove", 1, Vec3::ZERO)
        .unwrap();
    assert!(
        world
            .unit_command_attack_move_to(id, Vec3::new(140., 0., 90.))
            .expect("checked AttackMove command")
    );
    assert_eq!(
        world.host_object(id).unwrap().retained_ai_root_state(),
        AIState::AttackMoving
    );
    world.frame = 91;
    assert!(
        world
            .clear_base_ai_machine_owned(id, &mut Default::default())
            .unwrap()
    );
    let actor = world.host_object(id).unwrap();
    assert_eq!(actor.retained_ai_root_state(), AIState::Uninitialized);
    assert!(!actor.status.moving);
    assert_eq!(
        actor.next_mood_check_time,
        91u32.wrapping_add(world.ai_definitions.data().force_idle_frames_count)
    );
    assert!(actor.randomly_offset_mood_check);
}

#[test]
fn actual_attack_move_enters_child_idle_with_parent_goal_and_owned_deadline() {
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "attack-move-entry.ini".into(),
                "Object AttackMoveEntry\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = AIUpdateInterface FirstAI\nEnd\nEnd\n".into(),
            )])
            .unwrap(),
        )
        .unwrap();
    world.add_player(Player::new(1, Team::USA, "AttackMoveEntryOwner", true));
    let id = world
        .create_object_for_player("AttackMoveEntry", 1, Vec3::ZERO)
        .unwrap();
    let destination = Vec3::new(140., 0., 90.);
    world.frame = 41;
    assert!(
        world
            .unit_command_attack_move_to(id, destination)
            .expect("checked AttackMove command")
    );
    let unit = world.host_object(id).unwrap();
    assert_eq!(unit.unit_ai_runtime.root_state(), AIState::AttackMoving);
    assert_eq!(
        unit.unit_ai_runtime.parent_state(),
        Some(AIState::AttackMoving)
    );
    assert_eq!(unit.unit_ai_runtime.current_leaf_state(), AIState::Idle);
    let parent = unit.unit_ai_runtime.machine().parent.as_ref().unwrap();
    assert_eq!(parent.goals.position, destination);
    assert!(unit.unit_ai_runtime.machine().idle_initialized);
    assert!(!unit.idle_mood_reset_pending);
    assert_eq!(
        unit.next_mood_check_time,
        41u32.wrapping_add(world.ai_definitions.data().force_idle_frames_count)
    );
    assert!(unit.randomly_offset_mood_check);
}

#[test]
fn checked_decision_aware_idle_exits_attack_move_root_immediately() {
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "attack-move-idle-command.ini".into(),
                "Object AttackMoveIdleCommand\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = AIUpdateInterface FirstAI\nEnd\nEnd\n".into(),
            )])
            .unwrap(),
        )
        .unwrap();
    world.add_player(Player::new(1, Team::USA, "AttackMoveIdleOwner", true));
    let id = world
        .create_object_for_player("AttackMoveIdleCommand", 1, Vec3::ZERO)
        .unwrap();
    assert!(
        world
            .unit_command_attack_move_to(id, Vec3::new(100., 0., 60.))
            .expect("checked AttackMove command")
    );
    world.frame = 52;
    assert!(
        world
            .unit_command_set_ai_state(id, AIState::Idle)
            .expect("checked owned Idle transition")
    );
    let unit = world.host_object(id).unwrap();
    assert_eq!(unit.ai_state(), AIState::Idle);
    assert_eq!(unit.unit_ai_runtime.parent_state(), None);
    assert!(unit.unit_ai_runtime.machine().idle_initialized);
    assert!(!unit.idle_mood_reset_pending);
    assert_eq!(
        unit.next_mood_check_time,
        52u32.wrapping_add(world.ai_definitions.data().force_idle_frames_count)
    );
}

#[test]
fn player_stop_routes_attack_move_through_owned_idle_before_stamping_source() {
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "attack-move-stop.ini".into(),
                "Object AttackMoveStop\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = AIUpdateInterface FirstAI\nEnd\nEnd\n".into(),
            )])
            .unwrap(),
        )
        .unwrap();
    world.add_player(Player::new(1, Team::USA, "AttackMoveStopOwner", true));
    let id = world
        .create_object_for_player("AttackMoveStop", 1, Vec3::ZERO)
        .unwrap();
    assert!(
        world
            .unit_command_attack_move_to(id, Vec3::new(100., 0., 60.))
            .expect("checked AttackMove command")
    );
    assert!(world.unit_command_stop(id).expect("checked Stop operation"));
    let unit = world.host_object(id).unwrap();
    assert_eq!(unit.ai_state(), AIState::Idle);
    assert_eq!(unit.unit_ai_runtime.parent_state(), None);
    assert!(unit.unit_ai_runtime.machine().idle_initialized);
    assert!(!unit.idle_mood_reset_pending);
    assert_eq!(
        unit.last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_PLAYER
    );
}

#[test]
fn containment_exit_drop_uses_owned_idle_entry_for_the_receiving_actor() {
    let mut world = GameLogic::new();
    let id = world
        .create_object("Ranger", Team::USA, Vec3::ZERO)
        .expect("ranger");
    world.host_object_mut(id).unwrap().set_ai_state(
        AIState::Attacking,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    assert!(
        world
            .unit_command_exit_drop(id, Vec3::new(35., 0., 20.))
            .expect("checked containment exit")
    );
    let unit = world.host_object(id).unwrap();
    assert_eq!(unit.get_position(), Vec3::new(35., 0., 20.));
    assert_eq!(unit.ai_state(), AIState::Idle);
    assert!(unit.unit_ai_runtime.machine().idle_initialized);
    assert!(!unit.idle_mood_reset_pending);
}

#[test]
fn subdual_passenger_idle_uses_receiving_world_entry() {
    let (mut world, container_id) = jet(false);
    let passenger_id = world
        .create_object_for_player("JetIdleOwner", 1, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    world.host_object_mut(passenger_id).unwrap().set_ai_state(
        AIState::Moving,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    {
        let container = world.host_object_mut(container_id).unwrap();
        assert!(container.add_occupant(passenger_id));
        container.status.pending_subdual_passenger_idle = true;
    }
    world.set_fixture_object_contained_by(passenger_id, Some(container_id));

    world
        .flush_subdual_passenger_orders_with_effects(&mut Default::default())
        .expect("checked passenger Idle entry");

    let passenger = world.host_object(passenger_id).unwrap();
    assert_eq!(passenger.ai_state(), AIState::Idle);
    assert!(passenger.unit_ai_runtime.idle_initialized());
    assert!(!passenger.idle_mood_reset_pending);
    assert!(!passenger.status.attacking);
}

#[test]
fn real_attack_move_child_attack_returns_to_idle_without_losing_parent_goal() {
    use crate::game_logic::{Object, ThingTemplate, Weapon};
    let mut world = GameLogic::new();
    let id = ObjectId(41);
    let victim_id = ObjectId(42);
    let goal = Vec3::new(160., 0., 100.);
    let mut attacker = Object::new(ThingTemplate::new("AttackMoveAttacker"), id, Team::USA);
    attacker.set_position(Vec3::ZERO);
    attacker.weapon = Some(Weapon {
        range: 100.,
        damage: 10.,
        can_target_ground: true,
        ..Weapon::default()
    });
    attacker.requested_destination = Some(goal);
    attacker.attack_move_command_src = Some(13);
    attacker.last_command_source = crate::game_logic::HUNT_CMD_FROM_AI;
    attacker.set_ai_state(
        AIState::AttackMoving,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    let mut victim = Object::new(ThingTemplate::new("AttackMoveVictim"), victim_id, Team::GLA);
    victim.set_position(Vec3::new(30., 0., 0.));
    world.objects.insert(id, attacker);
    world.objects.insert(victim_id, victim);
    assert!(world.enter_attack_move_idle_child_owned(id));
    assert_eq!(
        world.attack_state_enter(id, victim_id),
        crate::game_logic::AttackMachineResult::Continue
    );
    let parent_goals = world
        .host_object(id)
        .unwrap()
        .unit_ai_runtime
        .machine()
        .parent
        .as_ref()
        .unwrap()
        .goals
        .clone();
    assert_eq!(
        world
            .host_object(id)
            .unwrap()
            .unit_ai_runtime
            .current_leaf_state(),
        AIState::Attacking
    );
    assert!(
        world
            .complete_attack_move_child_attack_owned(id)
            .expect("child completion preserves the receiving actor")
    );
    let actor = world.host_object(id).unwrap();
    assert_eq!(actor.unit_ai_runtime.root_state(), AIState::AttackMoving);
    assert_eq!(
        actor.unit_ai_runtime.parent_state(),
        Some(AIState::AttackMoving)
    );
    assert_eq!(actor.unit_ai_runtime.current_leaf_state(), AIState::Idle);
    assert_eq!(
        actor.unit_ai_runtime.machine().goals,
        parent_goals,
        "child completion restores the containing AttackMove goals"
    );
    assert_eq!(actor.requested_destination, Some(goal));
    assert_eq!(actor.last_command_source, 13);
    assert!(actor.target.is_none());
    assert!(actor.unit_ai_runtime.machine().idle_initialized);
}
