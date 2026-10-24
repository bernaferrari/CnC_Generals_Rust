//! Actual receiving launch -> flight -> encoded save -> Body continuation.
//! This does not certify equal-priority C++ scheduling (hq-om90a).
use super::tests::world;
use crate::game_logic::GameLogic;
use crate::game_logic::host_health_log::HostHealthEvent;
use crate::save_load::snapshot::{SnapshotBuilder, WorldSnapshot};
use glam::Vec3;

fn arena_bytes(world: &GameLogic) -> Vec<u8> {
    // State comparison only; never impose this sorting on gameplay visitation.
    let mut projectiles = world.combat_system.projectiles_snapshot();
    projectiles.sort_by_key(|projectile| projectile.id.0);
    bincode_legacy::serialize(&projectiles).unwrap()
}

fn encoded_snapshot(world: &GameLogic) -> WorldSnapshot {
    let snapshot = SnapshotBuilder::new().create_world_snapshot(world).unwrap();
    let encoded = bincode_legacy::serialize(&snapshot).unwrap();
    crate::save_load::snapshot::decode_bincode_world_snapshot(&encoded).unwrap()
}

fn launch(delay: u32) -> (GameLogic, super::super::ObjectId, super::super::ObjectId) {
    let (mut world, source, target) = world(delay);
    world.set_fixture_object_position(target, Vec3::new(150.0, 0.0, 0.0));
    assert!(world.fire_owned_passive_weapon(source, target, 0, Vec3::ZERO));
    assert_eq!(world.combat_system.projectile_count(), 1);
    world.frame += 1;
    assert!(world.update_owned_projectile_impacts(1.0 / 30.0).is_empty());
    assert_eq!(
        world.combat_system.projectile_count(),
        1,
        "save must interrupt real flight"
    );
    (world, source, target)
}

#[derive(Debug, PartialEq)]
struct FrameObservation {
    frame: u32,
    retired: Vec<super::super::ObjectId>,
    projectiles: Vec<u8>,
    health: u32,
    body: Vec<HostHealthEvent>,
}

fn continue_flight(world: &mut GameLogic, target: super::super::ObjectId) -> Vec<FrameObservation> {
    let mut observations = Vec::new();
    for _ in 0..40 {
        world.frame += 1;
        let retired = world.update_owned_projectile_impacts(1.0 / 30.0);
        observations.push(FrameObservation {
            frame: world.frame,
            retired,
            projectiles: arena_bytes(world),
            health: world.host_object(target).unwrap().health.current.to_bits(),
            body: world.health_events.drain_ordered(),
        });
        if world.combat_system.projectile_count() == 0 {
            break;
        }
    }
    assert_eq!(
        world.combat_system.projectile_count(),
        0,
        "real missile must retire"
    );
    observations
}

#[test]
fn encoded_live_missile_restores_exact_flight_and_real_body_future() {
    let (mut original, source, target) = launch(0);
    let saved_arena = arena_bytes(&original);
    let saved = encoded_snapshot(&original);
    let (mut restored, _, _) = world(1000);
    SnapshotBuilder::new()
        .restore_from_snapshot(&saved, &mut restored)
        .unwrap();
    assert_eq!(
        arena_bytes(&restored),
        saved_arena,
        "receiver definition cannot rewrite launched state"
    );
    let (peer, _, peer_target) = world(2000);
    let peer_health = peer.host_object(peer_target).unwrap().health.current;
    let before = original.host_object(target).unwrap().health.current;
    let expected = continue_flight(&mut original, target);
    assert_eq!(continue_flight(&mut restored, target), expected);
    let damage: Vec<_> = expected
        .iter()
        .flat_map(|frame| &frame.body)
        .filter_map(|event| {
            if let HostHealthEvent::Damage(event) = event {
                Some(event)
            } else {
                None
            }
        })
        .collect();
    assert!(
        damage.iter().any(|event| event.target == target
            && event.source == Some(source)
            && event.amount > 0.0)
    );
    assert!(
        original.host_object(target).unwrap().health.current < before,
        "flight must reach actual Body damage"
    );
    assert_eq!(
        peer.host_object(peer_target).unwrap().health.current,
        peer_health
    );
}

#[test]
fn restored_live_flight_survives_missing_launcher_and_keeps_allocation_cursor() {
    let (mut original, source, target) = launch(0);
    original.destroy_object(source);
    original.process_destroy_list();
    assert!(original.host_object(source).is_none());
    assert_eq!(
        original.combat_system.projectile_count(),
        1,
        "launcher retirement must not retire its live missile"
    );
    let saved = encoded_snapshot(&original);
    let (mut restored, _, _) = world(1000);
    SnapshotBuilder::new()
        .restore_from_snapshot(&saved, &mut restored)
        .unwrap();
    assert!(restored.host_object(source).is_none());
    assert_eq!(arena_bytes(&restored), arena_bytes(&original));
    assert_eq!(
        continue_flight(&mut restored, target),
        continue_flight(&mut original, target)
    );
    for receiver in [&mut original, &mut restored] {
        let fresh = receiver
            .create_object_for_player("ProfileSource", 0, Vec3::ZERO)
            .unwrap();
        assert!(receiver.fire_owned_passive_weapon(fresh, target, 0, Vec3::ZERO));
    }
    let a = original.combat_system.projectiles_snapshot()[0].id;
    let b = restored.combat_system.projectiles_snapshot()[0].id;
    assert_eq!(a, b);
    assert_eq!(
        a.0, 100001,
        "load must not replay launch or reset the allocation cursor"
    );
}

#[test]
fn projectile_names_cannot_spoof_legacy_tail_tags() {
    let (mut original, _, _) = launch(0);
    let id = original.combat_system.projectiles_snapshot()[0].id;
    let projectile = original.combat_system.projectile_mut(id).unwrap();
    projectile.projectile_object_name = "CPAD_APRO_SPCD_PJST".into();
    projectile.detonation_fx_name = "PJST_SPCD_APRO_CPAD".into();
    let saved = encoded_snapshot(&original);
    let (mut restored, _, _) = world(1000);
    SnapshotBuilder::new()
        .restore_from_snapshot(&saved, &mut restored)
        .unwrap();
    assert_eq!(arena_bytes(&restored), arena_bytes(&original));
}

#[test]
fn encoded_empty_arena_keeps_the_next_launch_identity() {
    let (mut original, source, target) = launch(0);
    continue_flight(&mut original, target);
    let saved = encoded_snapshot(&original);
    let (mut restored, _, _) = world(1000);
    SnapshotBuilder::new()
        .restore_from_snapshot(&saved, &mut restored)
        .unwrap();
    assert_eq!(restored.combat_system.projectile_count(), 0);
    assert!(restored.fire_owned_passive_weapon(source, target, 0, Vec3::ZERO));
    assert_eq!(
        restored.combat_system.projectiles_snapshot()[0].id.0,
        100001
    );
}

#[test]
fn encoded_missing_target_keeps_actual_missile_retirement_without_body_damage() {
    let (mut original, source, target) = launch(0);
    original.destroy_object(target);
    original.process_destroy_list();
    assert!(original.host_object(target).is_none());
    assert_eq!(
        original.combat_system.projectile_count(),
        1,
        "target retirement leaves the missile to resolve its missing reference"
    );
    let saved = encoded_snapshot(&original);
    let (mut restored, _, _) = world(1000);
    SnapshotBuilder::new()
        .restore_from_snapshot(&saved, &mut restored)
        .unwrap();
    assert!(restored.host_object(target).is_none());
    assert_eq!(arena_bytes(&restored), arena_bytes(&original));
    let original_health = original.host_object(source).unwrap().health.current;
    for _ in 0..100 {
        original.frame += 1;
        restored.frame += 1;
        assert_eq!(
            restored.update_owned_projectile_impacts(1.0 / 30.0),
            original.update_owned_projectile_impacts(1.0 / 30.0)
        );
        assert_eq!(arena_bytes(&restored), arena_bytes(&original));
        assert!(original.health_events.drain_ordered().is_empty());
        assert!(restored.health_events.drain_ordered().is_empty());
        if original.combat_system.projectile_count() == 0 {
            break;
        }
    }
    assert_eq!(original.combat_system.projectile_count(), 0);
    assert_eq!(
        original.host_object(source).unwrap().health.current,
        original_health
    );
    assert_eq!(
        restored.host_object(source).unwrap().health.current,
        original_health
    );
}

#[test]
fn corrupt_present_projectile_frame_rejects_before_receiver_mutation() {
    let (original, _, _) = launch(0);
    let mut saved = encoded_snapshot(&original);
    let prefix = crate::save_load::snapshot::encode_lifecycle_tail(
        &crate::save_load::snapshot::decode_lifecycle_tail(&saved.lifecycle_tail).unwrap(),
    )
    .len();
    assert_eq!(&saved.lifecycle_tail[prefix..prefix + 4], b"CPRJ");
    saved.lifecycle_tail[prefix + 4..prefix + 8].copy_from_slice(&99_u32.to_le_bytes());
    let (mut receiver, _, target) = launch(1000);
    let before = arena_bytes(&receiver);
    let frame = receiver.frame;
    let health = receiver.host_object(target).unwrap().health.current;
    let objects = receiver.objects.registered_len();
    assert!(
        SnapshotBuilder::new()
            .restore_from_snapshot(&saved, &mut receiver)
            .is_err()
    );
    assert_eq!(arena_bytes(&receiver), before);
    assert_eq!(receiver.frame, frame);
    assert_eq!(receiver.objects.registered_len(), objects);
    assert_eq!(receiver.host_object(target).unwrap().health.current, health);
}
