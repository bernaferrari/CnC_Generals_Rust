//! Body writes and their boundary effects belong to the same admitted world.
use super::*;
use crate::game_logic::{ObjectId, Player};

fn admitted_world() -> (GameLogic, ObjectId) {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "HealthOwner", true));
    ensure_template(&mut logic, "HealthEventOwnerTarget", 100.0);
    let id = logic
        .create_object("HealthEventOwnerTarget", Team::USA, Vec3::ZERO)
        .expect("admitted target");
    (logic, id)
}

#[test]
fn another_world_boundary_cannot_consume_completed_body_damage() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "0");
    let (mut owner, id) = admitted_world();
    let (mut other, other_id) = admitted_world();
    assert_eq!(id, other_id, "numeric IDs deliberately alias");
    owner.health_events.clear();
    owner.host_object_mut(id).unwrap().fire_weapon_when_damaged = Some(
        crate::game_logic::host_fire_weapon_when_damaged::HostFireWeaponWhenDamagedData {
            reaction_pristine: Some("OwnerPristine".into()),
            reaction_damaged: Some("OwnerDamaged".into()),
            reaction_really_damaged: Some("OwnerReallyDamaged".into()),
            ..Default::default()
        },
    );
    // CPP ActiveBody changes HP before onDamage. At 40% health the default
    // yellow threshold selects Damaged, rather than the pristine pre-hit state.
    let (target, events) = owner.host_object_and_health_events_mut(id).unwrap();
    assert!(!target.take_damage(60.0, events));
    assert_eq!(target.health.current, 40.0);
    assert_eq!(target.previous_health, 100.0);
    assert_eq!(
        target.pending_fire_when_damaged_weapon.as_deref(),
        Some("OwnerDamaged")
    );
    let pending = owner.health_events.snapshot_damage();
    assert_eq!(pending.len(), 1);
    assert!(pending[0].owner_health_already_applied());
    run_post_logic_shadow_boundary(None, &mut other);
    assert_eq!(other.host_object(other_id).unwrap().health.current, 100.0);
    assert_eq!(
        other
            .host_object(other_id)
            .unwrap()
            .pending_fire_when_damaged_weapon,
        None
    );
    assert_eq!(
        owner.health_events.snapshot_damage(),
        pending,
        "a foreign boundary consumed the owner's event"
    );
    run_post_logic_shadow_boundary(None, &mut owner);
    assert!(owner.health_events.snapshot_damage().is_empty());
    assert_eq!(owner.host_object(id).unwrap().health.current, 40.0);
    assert_eq!(owner.host_object(id).unwrap().previous_health, 100.0);
    assert_eq!(
        owner
            .host_object(id)
            .unwrap()
            .pending_fire_when_damaged_weapon
            .as_deref(),
        Some("OwnerDamaged")
    );
    owner.health_events.clear();
}

#[test]
fn another_world_boundary_cannot_admit_the_owners_lethal_damage() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "0");
    let (mut owner, id) = admitted_world();
    let (mut other, other_id) = admitted_world();
    assert_eq!(id, other_id);
    owner.health_events.clear();
    let (target, events) = owner.host_object_and_health_events_mut(id).unwrap();
    assert!(target.take_damage(1000.0, events));
    assert!(owner.host_object(id).unwrap().status.destroyed);
    run_post_logic_shadow_boundary(None, &mut other);
    let foreign = other.host_object(other_id).unwrap();
    assert!(
        foreign.is_alive(),
        "a foreign boundary admitted the owner's death"
    );
    assert_eq!(foreign.health.current, 100.0);
    assert!(!foreign.status.on_die_started);
    assert!(!other.has_pending_destroy_work());
    run_post_logic_shadow_boundary(None, &mut owner);
    // The completed production boundary performs onDie and final cleanup;
    // only this owner may remove the admitted object and retain its receipt.
    assert!(owner.host_object(id).is_none());
    assert!(!owner.has_pending_destroy_work());
    let completed = owner.health_events.snapshot_last_damage();
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].target, id);
    assert!(completed[0].destroyed);
    owner.process_destroy_list();
    assert!(owner.host_object(id).is_none());
    assert_eq!(owner.health_events.snapshot_last_damage(), completed);
    assert!(other.host_object(other_id).unwrap().is_alive());
    assert!(owner.health_events.snapshot_damage().is_empty());
    owner.health_events.clear();
}

#[test]
fn interleaved_worlds_keep_mixed_health_events_totals_and_presentation_receipts() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "0");
    let (mut owner, id) = admitted_world();
    let (mut other, other_id) = admitted_world();
    let (object, events) = owner.host_object_and_health_events_mut(id).unwrap();
    object.take_damage(20.0, events);
    object.heal(10.0, events);
    let pending = owner.health_events.snapshot_ordered();
    let (object, events) = other.host_object_and_health_events_mut(other_id).unwrap();
    object.take_damage(30.0, events);
    run_post_logic_shadow_boundary(None, &mut other);
    assert_eq!(other.host_object(other_id).unwrap().health.current, 70.0);
    assert_eq!(owner.health_events.snapshot_ordered(), pending);
    assert_eq!(owner.health_events.cumulative_totals(), (20.0, 0));
    assert_eq!(other.health_events.cumulative_totals(), (30.0, 0));
    run_post_logic_shadow_boundary(None, &mut owner);
    assert_eq!(owner.host_object(id).unwrap().health.current, 90.0);
    let receipt = owner.health_events.snapshot_last_damage();
    let _ = crate::presentation_frame::PresentationFrame::build_from_logic(&mut other, 0);
    assert_eq!(owner.health_events.snapshot_last_damage(), receipt);
    assert_eq!(owner.health_events.snapshot_last_heal().len(), 1);
    let _ = crate::presentation_frame::PresentationFrame::build_from_logic(&mut owner, 0);
    assert!(owner.health_events.snapshot_last_damage().is_empty());
    assert!(owner.health_events.snapshot_last_heal().is_empty());
    assert_eq!(owner.health_events.cumulative_totals(), (20.0, 0));
}

#[test]
fn constructing_resetting_and_dropping_other_world_preserve_owner_health_transport() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "0");
    let (mut owner, id) = admitted_world();
    let (object, events) = owner.host_object_and_health_events_mut(id).unwrap();
    object.take_damage(20.0, events);
    let pending = owner.health_events.snapshot_ordered();
    {
        let (mut other, other_id) = admitted_world();
        assert_eq!(id, other_id);
        assert_eq!(owner.health_events.snapshot_ordered(), pending);
        other.health_events.record_heal(other_id, 10.0);
        other.reset();
        assert!(other.health_events.snapshot_ordered().is_empty());
        assert_eq!(other.health_events.cumulative_totals(), (0.0, 0));
        assert_eq!(owner.health_events.snapshot_ordered(), pending);
    }
    assert_eq!(owner.health_events.snapshot_ordered(), pending);
    run_post_logic_shadow_boundary(None, &mut owner);
    assert_eq!(owner.host_object(id).unwrap().health.current, 80.0);
    owner.reset();
    assert!(owner.health_events.snapshot_ordered().is_empty());
    assert!(owner.health_events.snapshot_last_damage().is_empty());
    assert_eq!(owner.health_events.cumulative_totals(), (0.0, 0));
}

#[test]
fn interleaved_eager_and_session_boundaries_keep_the_owners_health_batch() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "1");
    let (mut owner, id) = admitted_world();
    let (mut other, other_id) = admitted_world();
    owner.set_damage_authority(true);
    other.set_damage_authority(true);
    let mut owner_shadow = GameWorldShadow::new(64);
    let mut other_shadow = GameWorldShadow::new(64);
    owner_shadow.sync_from_host(&owner);
    other_shadow.sync_from_host(&other);
    let owner_entity = owner_shadow.entity_for_host(id).unwrap();
    let other_entity = other_shadow.entity_for_host(other_id).unwrap();
    owner.health_events.record_damage(id, 20.0, None, false);
    let _couple = ShadowCoupleGuard::enter();
    assert!(
        crate::gameworld_shadow::tick::eager_apply_host_health_after_logic(
            &mut owner_shadow,
            &mut owner
        ) > 0
    );
    assert_eq!(
        owner_shadow.world().entity(owner_entity).unwrap().health,
        80.0
    );
    let _ = crate::gameworld_shadow::tick::eager_apply_host_health_after_logic(
        &mut other_shadow,
        &mut other,
    );
    let _ = shadow_session_after_host_tick(&mut other_shadow, &mut other);
    assert_eq!(
        other_shadow.world().entity(other_entity).unwrap().health,
        100.0
    );
    assert_eq!(other.host_object(other_id).unwrap().health.current, 100.0);
    let batch = owner
        .health_events
        .take_early_batch()
        .expect("owner receipt retained");
    assert_eq!(batch.events().len(), 1);
    assert_eq!(batch.shadow_applied_events().len(), 1);
    owner.health_events.set_early_batch(batch);
    let _ = shadow_session_after_host_tick(&mut owner_shadow, &mut owner);
    assert_eq!(
        owner_shadow.world().entity(owner_entity).unwrap().health,
        80.0
    );
    assert_eq!(owner.host_object(id).unwrap().health.current, 80.0);
}

#[test]
fn snapshot_restore_invalidates_only_the_receivers_transient_health_transport() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "0");
    let (mut owner, id) = admitted_world();
    let (mut receiver, receiver_id) = admitted_world();
    let (object, events) = owner.host_object_and_health_events_mut(id).unwrap();
    object.take_damage(20.0, events);
    let pending = owner.health_events.snapshot_ordered();
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&owner).unwrap();
    assert_eq!(owner.health_events.snapshot_ordered(), pending);
    receiver.health_events.record_heal(receiver_id, 50.0);
    let receiver_pending = receiver.health_events.snapshot_ordered();
    let version = snapshot.version;
    snapshot.version = u32::MAX;
    assert!(
        builder
            .restore_from_snapshot(&snapshot, &mut receiver)
            .is_err()
    );
    assert_eq!(receiver.health_events.snapshot_ordered(), receiver_pending);
    assert_eq!(owner.health_events.snapshot_ordered(), pending);
    snapshot.version = version;
    builder
        .restore_from_snapshot(&snapshot, &mut receiver)
        .unwrap();
    assert!(receiver.health_events.snapshot_ordered().is_empty());
    assert_eq!(receiver.health_events.cumulative_totals(), (0.0, 0));
    assert_eq!(owner.health_events.snapshot_ordered(), pending);
    assert_eq!(owner.health_events.cumulative_totals(), (20.0, 0));
    run_post_logic_shadow_boundary(None, &mut owner);
    assert_eq!(owner.host_object(id).unwrap().health.current, 80.0);
}

#[test]
fn health_event_fallback_admits_unmapped_eager_health_before_new_owner_events() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "1");
    let (mut owner, id) = admitted_world();
    owner.set_damage_authority(true);
    let mut unmapped = GameWorldShadow::new(64);
    owner.health_events.record_heal(id, 90.0);
    {
        let _couple = ShadowCoupleGuard::enter();
        assert_eq!(
            crate::gameworld_shadow::tick::eager_apply_host_health_after_logic(
                &mut unmapped,
                &mut owner
            ),
            0
        );
    }
    owner.health_events.record_damage(id, 20.0, None, false);
    run_post_logic_shadow_boundary(None, &mut owner);
    assert_eq!(owner.host_object(id).unwrap().health.current, 70.0);
    assert!(owner.health_events.snapshot_ordered().is_empty());
    assert!(owner.health_events.take_early_batch().is_none());
    assert_eq!(owner.health_events.snapshot_last_heal().len(), 1);
    assert_eq!(owner.health_events.snapshot_last_damage().len(), 1);
    run_post_logic_shadow_boundary(None, &mut owner);
    assert_eq!(owner.host_object(id).unwrap().health.current, 70.0);
}

#[test]
fn health_handoff_session_admits_events_recorded_after_eager_damage() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "1");
    let (mut owner, id) = admitted_world();
    owner.set_damage_authority(true);
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&owner);
    let entity = shadow.entity_for_host(id).unwrap();
    owner.health_events.record_damage(id, 20.0, None, false);
    {
        let _couple = ShadowCoupleGuard::enter();
        assert!(
            crate::gameworld_shadow::tick::eager_apply_host_health_after_logic(
                &mut shadow,
                &mut owner,
            ) > 0
        );
    }
    assert_eq!(shadow.world().entity(entity).unwrap().health, 80.0);
    owner.health_events.record_heal(id, 90.0);
    owner.health_events.record_damage(id, 20.0, None, false);
    let _ = shadow_session_after_host_tick(&mut shadow, &mut owner);
    assert_eq!(owner.host_object(id).unwrap().health.current, 70.0);
    assert_eq!(shadow.world().entity(entity).unwrap().health, 70.0);
    assert!(owner.health_events.snapshot_ordered().is_empty());
    assert!(owner.health_events.take_early_batch().is_none());
    assert_eq!(owner.health_events.snapshot_last_damage().len(), 2);
    assert_eq!(owner.health_events.snapshot_last_heal().len(), 1);
}

#[test]
fn health_handoff_empty_eager_preserves_an_unmapped_receipt() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "1");
    let (mut owner, id) = admitted_world();
    owner.set_damage_authority(true);
    let mut shadow = GameWorldShadow::new(64);
    owner.health_events.record_heal(id, 90.0);
    {
        let _couple = ShadowCoupleGuard::enter();
        for _ in 0..2 {
            assert_eq!(
                crate::gameworld_shadow::tick::eager_apply_host_health_after_logic(
                    &mut shadow,
                    &mut owner,
                ),
                0
            );
        }
    }
    owner.health_events.record_damage(id, 20.0, None, false);
    run_post_logic_shadow_boundary(None, &mut owner);
    assert_eq!(owner.host_object(id).unwrap().health.current, 70.0);
    assert_eq!(owner.health_events.snapshot_last_heal().len(), 1);
    assert_eq!(owner.health_events.snapshot_last_damage().len(), 1);
    run_post_logic_shadow_boundary(None, &mut owner);
    assert_eq!(owner.host_object(id).unwrap().health.current, 70.0);
}

#[test]
fn health_handoff_repeated_eager_preserves_the_applied_prefix() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "1");
    let (mut owner, id) = admitted_world();
    owner.set_damage_authority(true);
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&owner);
    let entity = shadow.entity_for_host(id).unwrap();
    {
        let _couple = ShadowCoupleGuard::enter();
        owner.health_events.record_damage(id, 20.0, None, false);
        assert!(
            crate::gameworld_shadow::tick::eager_apply_host_health_after_logic(
                &mut shadow,
                &mut owner,
            ) > 0
        );
        assert_eq!(
            crate::gameworld_shadow::tick::eager_apply_host_health_after_logic(
                &mut shadow,
                &mut owner,
            ),
            0
        );
        owner.health_events.record_damage(id, 10.0, None, false);
        assert!(
            crate::gameworld_shadow::tick::eager_apply_host_health_after_logic(
                &mut shadow,
                &mut owner,
            ) > 0
        );
    }
    assert_eq!(shadow.world().entity(entity).unwrap().health, 70.0);
    let _ = shadow_session_after_host_tick(&mut shadow, &mut owner);
    assert_eq!(owner.host_object(id).unwrap().health.current, 70.0);
    assert_eq!(owner.health_events.snapshot_last_damage().len(), 2);
    assert_eq!(owner.health_events.cumulative_totals(), (30.0, 0));
    assert!(owner.health_events.snapshot_ordered().is_empty());
    assert!(owner.health_events.take_early_batch().is_none());
}

#[test]
fn health_handoff_preserves_completed_body_writes_after_eager_admission() {
    let _env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", "1");
    let (mut owner, id) = admitted_world();
    owner.set_damage_authority(true);
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&owner);
    let entity = shadow.entity_for_host(id).unwrap();
    let _couple = ShadowCoupleGuard::enter();
    let (object, events) = owner.host_object_and_health_events_mut(id).unwrap();
    object.take_damage(20.0, events);
    assert_eq!(object.health.current, 80.0);
    assert!(
        crate::gameworld_shadow::tick::eager_apply_host_health_after_logic(&mut shadow, &mut owner,)
            > 0
    );
    let (object, events) = owner.host_object_and_health_events_mut(id).unwrap();
    object.heal(10.0, events);
    assert_eq!(object.health.current, 90.0);
    object.take_damage(20.0, events);
    assert_eq!(object.health.current, 70.0);
    assert_eq!(object.previous_health, 90.0);
    let _ = shadow_session_after_host_tick(&mut shadow, &mut owner);
    let object = owner.host_object(id).unwrap();
    assert_eq!(object.health.current, 70.0);
    assert_eq!(object.previous_health, 90.0);
    assert_eq!(shadow.world().entity(entity).unwrap().health, 70.0);
    assert_eq!(owner.health_events.snapshot_last_damage().len(), 2);
    assert_eq!(owner.health_events.snapshot_last_heal().len(), 1);
    assert!(owner.health_events.snapshot_ordered().is_empty());
}
