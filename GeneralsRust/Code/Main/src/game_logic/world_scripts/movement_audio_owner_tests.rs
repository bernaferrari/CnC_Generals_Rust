//! Receiving movement-loop exit output: source AI move-state/temporary-state exit
//! points, not deferred thread-local drain order. Register below game_logic.
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_usa_pilot::HostDeathType;
use crate::game_logic::object::DamageHitContext;
use crate::game_logic::{
    AIState, AudioEventRequest, GameLogic, ObjectAudio, ObjectId, Player, Team,
};
use glam::Vec3;

fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, run);
}
const NAME: &str = "ReceivingMoveExitVehicle";
fn world(loop_name: &str) -> (GameLogic, ObjectId) {
    let ini = format!(
        "Object {NAME}\nKindOf = VEHICLE SELECTABLE\nSoundMoveStart = NoSound\nSoundMoveStartDamaged = NoSound\nSoundMoveLoop = {loop_name}\nSoundMoveLoopDamaged = {loop_name}\nSoundAmbient = NoSound\nVoiceFear = NoSound\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"
    );
    let content =
        crate::assets::ReceivingObjectContent::from_sources([("move-exit-owner.ini".into(), ini)])
            .unwrap();
    let mut owner = GameLogic::new();
    owner.admit_receiving_module_content(content).unwrap();
    owner.add_player(Player::new(1, Team::USA, "MoveOwner", true));
    let id = owner.create_object_for_player(NAME, 1, Vec3::ZERO).unwrap();
    owner.queued_audio_events.clear();
    (owner, id)
}
fn running(owner: &mut GameLogic, id: ObjectId, pose: Vec3) {
    let object = owner.host_object_mut(id).unwrap();
    object.set_position(pose);
    // Prepare the source move state; actual sound start runs its production API.
    object.ai_state = AIState::Moving;
    object.status.moving = true;
    owner.start_move_sound(id);
    assert!(owner.host_object(id).unwrap().move_loop_audio.is_some());
    assert_eq!(owner.queued_audio_events.len(), 1);
    assert!(owner.queued_audio_events[0].is_looping);
    owner.queued_audio_events.clear();
}
fn stop(request: &AudioEventRequest, id: ObjectId, name: &str, pose: Vec3) {
    assert_eq!(request.event_type, name);
    assert_eq!(request.object_id, Some(id));
    assert_eq!(request.position, Some(pose));
    assert!(request.stop);
    assert!(!request.is_looping);
}

#[test]
fn explicit_stop_is_immediate_and_same_id_peer_cannot_drain_it() {
    isolated(
        "explicit_stop_is_immediate_and_same_id_peer_cannot_drain_it",
        || {
            let (mut a, aid) = world("OwnerALoop");
            let (mut b, bid) = world("OwnerBLoop");
            assert_eq!(aid, bid);
            let pose = Vec3::new(14., 2., -33.);
            running(&mut a, aid, pose);
            running(&mut b, bid, Vec3::new(-9., 0., 77.));
            assert!(a.unit_command_stop(aid));
            assert_eq!(a.queued_audio_events.len(), 1);
            stop(&a.queued_audio_events[0], aid, "OwnerALoop", pose);
            b.retry_dropped_ambient_sounds();
            assert!(b.queued_audio_events.is_empty());
            assert_eq!(
                b.host_object(bid).unwrap().move_loop_audio.as_deref(),
                Some("OwnerBLoop")
            );
            b.reset();
            stop(&a.queued_audio_events[0], aid, "OwnerALoop", pose);
        },
    );
}

#[test]
fn state_exit_uses_its_receiving_session_without_peer_publication() {
    isolated(
        "state_exit_uses_its_receiving_session_without_peer_publication",
        || {
            let (mut a, id) = world("StateALoop");
            let (mut b, peer) = world("StateBLoop");
            assert_eq!(id, peer);
            running(&mut a, id, Vec3::new(6., 3., 9.));
            running(&mut b, peer, Vec3::ZERO);
            a.set_ai_state_decision_aware_for_ai(id, AIState::Idle);
            assert_eq!(a.queued_audio_events.len(), 1);
            stop(
                &a.queued_audio_events[0],
                id,
                "StateALoop",
                Vec3::new(6., 3., 9.),
            );
            b.retry_dropped_ambient_sounds();
            assert!(b.queued_audio_events.is_empty());
            assert_eq!(b.host_object(peer).unwrap().ai_state, AIState::Moving);
        },
    );
}

#[test]
fn ordinary_stop_then_waited_overlay_failure_keeps_exact_emission_order() {
    isolated(
        "ordinary_stop_then_waited_overlay_failure_keeps_exact_emission_order",
        || {
            let (mut owner, ordinary) = world("OrderedLoop");
            let overlay = owner
                .create_object_for_player(NAME, 1, Vec3::new(70., 0., 0.))
                .unwrap();
            running(&mut owner, ordinary, Vec3::new(5., 0., 6.));
            running(&mut owner, overlay, Vec3::new(70., 0., 0.));
            {
                let object = owner.host_object_mut(overlay).unwrap();
                object.ai_state = AIState::GuardingArea;
                object.temporary_move_frames = 9;
                object.waiting_for_path = true;
            }
            assert!(owner.unit_command_stop(ordinary));
            owner.on_waited_path_failed(overlay);
            assert_eq!(owner.queued_audio_events.len(), 2);
            stop(
                &owner.queued_audio_events[0],
                ordinary,
                "OrderedLoop",
                Vec3::new(5., 0., 6.),
            );
            stop(
                &owner.queued_audio_events[1],
                overlay,
                "OrderedLoop",
                Vec3::new(70., 0., 0.),
            );
            assert_eq!(owner.host_object(overlay).unwrap().temporary_move_frames, 0);
            assert_eq!(
                owner.host_object(overlay).unwrap().ai_state,
                AIState::GuardingArea
            );
        },
    );
}

#[test]
fn stop_captures_pose_and_precedes_the_next_real_loop_start() {
    isolated(
        "stop_captures_pose_and_precedes_the_next_real_loop_start",
        || {
            let (mut owner, id) = world("RestartLoop");
            let old = Vec3::new(11., 4., 22.);
            let next = Vec3::new(80., 7., 90.);
            running(&mut owner, id, old);
            owner.on_waited_path_failed(id);
            owner.host_object_mut(id).unwrap().set_position(next);
            owner.start_move_sound(id);
            assert_eq!(owner.queued_audio_events.len(), 2);
            stop(&owner.queued_audio_events[0], id, "RestartLoop", old);
            let start = &owner.queued_audio_events[1];
            assert_eq!(start.event_type, "RestartLoop");
            assert_eq!(start.position, Some(next));
            assert_eq!(start.object_id, Some(id));
            assert!(start.is_looping && !start.stop);
        },
    );
}

#[test]
fn nested_stop_and_overlay_exit_take_the_loop_handle_exactly_once() {
    isolated(
        "nested_stop_and_overlay_exit_take_the_loop_handle_exactly_once",
        || {
            let (mut owner, id) = world("NestedLoop");
            running(&mut owner, id, Vec3::ZERO);
            owner.host_object_mut(id).unwrap().temporary_move_frames = 8;
            assert!(owner.unit_command_stop(id));
            // stop -> stop_moving -> overlay exit all share one receiving loan.
            let (object, output) = owner.host_object_and_audio_events_mut(id).unwrap();
            let mut audio = ObjectAudio::queue(output);
            object.stop_moving(&mut audio);
            object.end_temporary_move_overlay(&mut audio);
            object.set_ai_state(AIState::Idle, &mut audio);
            assert_eq!(owner.queued_audio_events.len(), 1);
            stop(&owner.queued_audio_events[0], id, "NestedLoop", Vec3::ZERO);
        },
    );
}

#[test]
fn actual_physics_timer_exit_admits_stop_before_returning() {
    isolated(
        "actual_physics_timer_exit_admits_stop_before_returning",
        || {
            let (mut owner, id) = world("TimerLoop");
            let (mut peer, pid) = world("PeerTimerLoop");
            assert_eq!(id, pid);
            running(&mut owner, id, Vec3::ZERO);
            {
                let object = owner.host_object_mut(id).unwrap();
                object.ai_state = AIState::GuardingArea;
                object.temporary_move_frames = 1;
                object.movement.target_position = Some(Vec3::new(100., 0., 0.));
            }
            owner.tick_physics_collisions_all();
            assert_eq!(owner.host_object(id).unwrap().temporary_move_frames, 0);
            assert_eq!(owner.queued_audio_events.len(), 1);
            stop(&owner.queued_audio_events[0], id, "TimerLoop", Vec3::ZERO);
            peer.retry_dropped_ambient_sounds();
            assert!(peer.queued_audio_events.is_empty());
        },
    );
}

#[test]
fn detached_clone_uses_only_its_explicit_local_collector() {
    isolated(
        "detached_clone_uses_only_its_explicit_local_collector",
        || {
            let (mut owner, id) = world("OwnedCloneSourceLoop");
            running(&mut owner, id, Vec3::new(3., 0., 8.));
            let mut detached = owner.host_object(id).unwrap().clone();
            // Clone grants no receiving-owner capability. Give this fixture its own
            // explicit handle and output; make no assumption that Clone retains it.
            detached.move_loop_audio = Some("DetachedFixtureLoop".into());
            detached.set_position(Vec3::new(80., 0., 8.));
            let mut local = Vec::new();
            detached.stop_moving(&mut ObjectAudio::queue(&mut local));
            assert_eq!(local.len(), 1);
            stop(&local[0], id, "DetachedFixtureLoop", Vec3::new(80., 0., 8.));
            assert!(owner.queued_audio_events.is_empty());
            assert_eq!(
                owner.host_object(id).unwrap().move_loop_audio.as_deref(),
                Some("OwnedCloneSourceLoop")
            );
        },
    );
}

#[test]
fn actual_body_death_and_encoded_restore_keep_output_on_the_receiver() {
    isolated(
        "actual_body_death_and_encoded_restore_keep_output_on_the_receiver",
        || {
            let (mut owner, id) = world("DeathRestoreLoop");
            let (mut peer, pid) = world("PeerDeathRestoreLoop");
            assert_eq!(id, pid);
            let builder = crate::save_load::SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&owner).unwrap();
            let bytes = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded =
                crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
            running(&mut owner, id, Vec3::new(17., 0., 25.));
            assert!(
                owner
                    .apply_owned_damage(
                        id,
                        100.,
                        None,
                        DamageType::Unresistable,
                        HostDeathType::Normal,
                        None,
                        &DamageHitContext::default()
                    )
                    .unwrap()
                    .destroyed()
            );
            let stops: Vec<_> = owner
                .queued_audio_events
                .iter()
                .filter(|event| event.stop)
                .collect();
            assert_eq!(stops.len(), 1);
            stop(stops[0], id, "DeathRestoreLoop", Vec3::new(17., 0., 25.));
            peer.retry_dropped_ambient_sounds();
            assert!(peer.queued_audio_events.is_empty());
            builder.restore_from_snapshot(&decoded, &mut owner).unwrap();
            assert!(
                owner.queued_audio_events.is_empty(),
                "valid replacement discards outgoing death audio"
            );
            assert_eq!(owner.host_object(id).unwrap().health.current, 100.);
            assert!(peer.queued_audio_events.is_empty());
        },
    );
}
