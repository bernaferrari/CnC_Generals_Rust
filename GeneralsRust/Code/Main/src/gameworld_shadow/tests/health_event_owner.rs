//! Body writes and their boundary effects belong to the same admitted world.
use super::*;
use crate::game_logic::{ObjectId, Player};

fn admitted_world() -> (GameLogic, ObjectId) {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "HealthOwner", true));
    ensure_template(&mut logic, "HealthEventOwnerTarget", 100.0);
    logic
        .templates
        .get_mut("HealthEventOwnerTarget")
        .unwrap()
        .authored_instant_death = Some(vec![Default::default()]);
    let id = logic
        .create_object("HealthEventOwnerTarget", Team::USA, Vec3::ZERO)
        .expect("admitted target");
    (logic, id)
}

#[test]
fn another_world_boundary_cannot_consume_completed_body_damage() {
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
    assert_eq!(pending[0].body_observation().current, 40.0);
    crate::gameworld_shadow::run_owned_host_boundary(None, &mut other);
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
    crate::gameworld_shadow::run_owned_host_boundary(None, &mut owner);
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
    let (mut owner, id) = admitted_world();
    let (mut other, other_id) = admitted_world();
    assert_eq!(id, other_id);
    owner.health_events.clear();
    perform_owner_damage(&mut owner, id, 1000.0, None);
    assert!(owner.host_object(id).unwrap().status.destroyed);
    crate::gameworld_shadow::run_owned_host_boundary(None, &mut other);
    let foreign = other.host_object(other_id).unwrap();
    assert!(
        foreign.is_alive(),
        "a foreign boundary admitted the owner's death"
    );
    assert_eq!(foreign.health.current, 100.0);
    assert!(!foreign.status.on_die_started);
    assert!(!other.has_pending_destroy_work());
    crate::gameworld_shadow::run_owned_host_boundary(None, &mut owner);
    assert!(owner.host_object(id).unwrap().status.destroyed);
    assert!(owner.objects.ordered_ids().contains(&id));
    assert!(owner.has_pending_destroy_work());
    assert!(other.host_object(other_id).unwrap().is_alive());
    // onDie already ran on this owner. Only its explicit checked endphase
    // performs physical cleanup; the observer merely retains the receipt.
    owner
        .process_destroy_list_with_effects_checked(&mut Default::default())
        .unwrap();
    crate::gameworld_shadow::run_owned_host_boundary(None, &mut owner);
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
    let (mut owner, id) = admitted_world();
    let (mut other, other_id) = admitted_world();
    let body_frame = owner.frame;
    let (object, events) = owner.host_object_and_health_events_mut(id).unwrap();
    object.take_damage(20.0, 0, events);
    object.heal(10.0, events, body_frame);
    let pending = owner.health_events.snapshot_ordered();
    let (object, events) = other.host_object_and_health_events_mut(other_id).unwrap();
    object.take_damage(30.0, 0, events);
    crate::gameworld_shadow::run_owned_host_boundary(None, &mut other);
    assert_eq!(other.host_object(other_id).unwrap().health.current, 70.0);
    assert_eq!(owner.health_events.snapshot_ordered(), pending);
    assert_eq!(owner.health_events.cumulative_totals(), (20.0, 0));
    assert_eq!(other.health_events.cumulative_totals(), (30.0, 0));
    crate::gameworld_shadow::run_owned_host_boundary(None, &mut owner);
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
    let (mut owner, id) = admitted_world();
    let (object, events) = owner.host_object_and_health_events_mut(id).unwrap();
    object.take_damage(20.0, 0, events);
    let pending = owner.health_events.snapshot_ordered();
    {
        let (mut other, other_id) = admitted_world();
        assert_eq!(id, other_id);
        assert_eq!(owner.health_events.snapshot_ordered(), pending);
        perform_owner_health(&mut other, other_id, 10.0);
        other.reset();
        assert!(other.health_events.snapshot_ordered().is_empty());
        assert_eq!(other.health_events.cumulative_totals(), (0.0, 0));
        assert_eq!(owner.health_events.snapshot_ordered(), pending);
    }
    assert_eq!(owner.health_events.snapshot_ordered(), pending);
    crate::gameworld_shadow::run_owned_host_boundary(None, &mut owner);
    assert_eq!(owner.host_object(id).unwrap().health.current, 80.0);
    owner.reset();
    assert!(owner.health_events.snapshot_ordered().is_empty());
    assert!(owner.health_events.snapshot_last_damage().is_empty());
    assert_eq!(owner.health_events.cumulative_totals(), (0.0, 0));
}

#[test]
fn interleaved_eager_and_session_boundaries_keep_the_owners_health_batch() {
    let (mut owner, id) = admitted_world();
    let (mut other, other_id) = admitted_world();
    assert_eq!(id, other_id);
    let mut shadow = GameWorldShadow::new(64);
    let (object, events) = owner.host_object_and_health_events_mut(id).unwrap();
    object.take_damage(20.0, 0, events);
    shadow.sync_from_host(&owner);
    let entity = shadow.entity_for_host(id).unwrap();
    assert_eq!(shadow.world().entity(entity).unwrap().health, 80.0);
    let pending = owner.health_events.snapshot_ordered();
    crate::gameworld_shadow::run_owned_host_boundary(None, &mut other);
    assert_eq!(other.host_object(other_id).unwrap().health.current, 100.0);
    assert_eq!(owner.health_events.snapshot_ordered(), pending);
    observe_completed_owner(&mut shadow, &mut owner);
    assert_eq!(owner.host_object(id).unwrap().health.current, 80.0);
    assert_eq!(shadow.world().entity(entity).unwrap().health, 80.0);
    assert_eq!(owner.health_events.snapshot_last_damage().len(), 1);
    assert!(owner.health_events.snapshot_ordered().is_empty());
}

#[test]
fn snapshot_restore_invalidates_only_the_receivers_transient_health_transport() {
    let (mut owner, id) = admitted_world();
    let (mut receiver, receiver_id) = admitted_world();
    let (object, events) = owner.host_object_and_health_events_mut(id).unwrap();
    object.take_damage(20.0, 0, events);
    let pending = owner.health_events.snapshot_ordered();
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&owner).unwrap();
    assert_eq!(owner.health_events.snapshot_ordered(), pending);
    perform_owner_health(&mut receiver, receiver_id, 50.0);
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
    crate::gameworld_shadow::run_owned_host_boundary(None, &mut owner);
    assert_eq!(owner.host_object(id).unwrap().health.current, 80.0);
}

#[test]
fn health_event_fallback_admits_unmapped_eager_health_before_new_owner_events() {
    let (mut owner, id) = admitted_world();
    let mut shadow = GameWorldShadow::new(64);
    perform_owner_health(&mut owner, id, 90.0);
    // An observer lookup cannot admit, reorder, or consume pending owner work.
    let pending = owner.health_events.snapshot_ordered();
    assert!(shadow.entity_for_host(id).is_none());
    assert_eq!(owner.health_events.snapshot_ordered(), pending);
    perform_owner_damage(&mut owner, id, 20.0, None);
    observe_completed_owner(&mut shadow, &mut owner);
    assert_eq!(owner.host_object(id).unwrap().health.current, 70.0);
    let entity = shadow.entity_for_host(id).unwrap();
    assert_eq!(shadow.world().entity(entity).unwrap().health, 70.0);
    assert!(owner.health_events.snapshot_ordered().is_empty());
    assert_eq!(owner.health_events.snapshot_last_heal().len(), 1);
    assert_eq!(owner.health_events.snapshot_last_damage().len(), 1);
    observe_completed_owner(&mut shadow, &mut owner);
    assert_eq!(owner.host_object(id).unwrap().health.current, 70.0);
}

#[test]
fn health_handoff_session_admits_events_recorded_after_eager_damage() {
    let (mut owner, id) = admitted_world();
    let mut shadow = GameWorldShadow::new(64);
    let (object, events) = owner.host_object_and_health_events_mut(id).unwrap();
    object.take_damage(20.0, 0, events);
    assert_eq!(object.health.current, 80.0);
    shadow.sync_from_host(&owner);
    let entity = shadow.entity_for_host(id).unwrap();
    assert_eq!(shadow.world().entity(entity).unwrap().health, 80.0);
    let body_frame = owner.frame;
    let (object, events) = owner.host_object_and_health_events_mut(id).unwrap();
    object.heal(10.0, events, body_frame);
    assert_eq!(object.health.current, 90.0);
    object.take_damage(20.0, 0, events);
    assert_eq!(object.health.current, 70.0);
    assert_eq!(object.previous_health, 90.0);
    observe_completed_owner(&mut shadow, &mut owner);
    assert_eq!(owner.host_object(id).unwrap().health.current, 70.0);
    assert_eq!(owner.host_object(id).unwrap().previous_health, 90.0);
    assert_eq!(shadow.world().entity(entity).unwrap().health, 70.0);
    assert_eq!(owner.health_events.snapshot_last_damage().len(), 2);
    assert_eq!(owner.health_events.snapshot_last_heal().len(), 1);
    assert!(owner.health_events.snapshot_ordered().is_empty());
}

#[test]
fn health_handoff_empty_eager_preserves_an_unmapped_receipt() {
    let (mut owner, id) = admitted_world();
    let mut shadow = GameWorldShadow::new(64);
    perform_owner_health(&mut owner, id, 90.0);
    // An observer lookup cannot admit, reorder, or consume pending owner work.
    let pending = owner.health_events.snapshot_ordered();
    assert!(shadow.entity_for_host(id).is_none());
    assert_eq!(owner.health_events.snapshot_ordered(), pending);
    perform_owner_damage(&mut owner, id, 20.0, None);
    observe_completed_owner(&mut shadow, &mut owner);
    assert_eq!(owner.host_object(id).unwrap().health.current, 70.0);
    let entity = shadow.entity_for_host(id).unwrap();
    assert_eq!(shadow.world().entity(entity).unwrap().health, 70.0);
    assert!(owner.health_events.snapshot_ordered().is_empty());
    assert_eq!(owner.health_events.snapshot_last_heal().len(), 1);
    assert_eq!(owner.health_events.snapshot_last_damage().len(), 1);
    observe_completed_owner(&mut shadow, &mut owner);
    assert_eq!(owner.host_object(id).unwrap().health.current, 70.0);
}

#[test]
fn health_handoff_repeated_eager_preserves_the_applied_prefix() {
    let (mut owner, id) = admitted_world();
    let mut shadow = GameWorldShadow::new(64);
    let (object, events) = owner.host_object_and_health_events_mut(id).unwrap();
    object.take_damage(20.0, 0, events);
    shadow.sync_from_host(&owner);
    let entity = shadow.entity_for_host(id).unwrap();
    assert_eq!(shadow.world().entity(entity).unwrap().health, 80.0);
    shadow.sync_from_host(&owner);
    assert_eq!(owner.host_object(id).unwrap().health.current, 80.0);
    let (object, events) = owner.host_object_and_health_events_mut(id).unwrap();
    object.take_damage(10.0, 0, events);
    shadow.sync_from_host(&owner);
    assert_eq!(shadow.world().entity(entity).unwrap().health, 70.0);
    observe_completed_owner(&mut shadow, &mut owner);
    assert_eq!(owner.host_object(id).unwrap().health.current, 70.0);
    assert_eq!(owner.health_events.snapshot_last_damage().len(), 2);
    assert_eq!(owner.health_events.cumulative_totals(), (30.0, 0));
    assert!(owner.health_events.snapshot_ordered().is_empty());
}

#[test]
fn health_handoff_preserves_completed_body_writes_after_eager_admission() {
    let (mut owner, id) = admitted_world();
    let mut shadow = GameWorldShadow::new(64);
    let (object, events) = owner.host_object_and_health_events_mut(id).unwrap();
    object.take_damage(20.0, 0, events);
    assert_eq!(object.health.current, 80.0);
    shadow.sync_from_host(&owner);
    let entity = shadow.entity_for_host(id).unwrap();
    assert_eq!(shadow.world().entity(entity).unwrap().health, 80.0);
    let body_frame = owner.frame;
    let (object, events) = owner.host_object_and_health_events_mut(id).unwrap();
    object.heal(10.0, events, body_frame);
    assert_eq!(object.health.current, 90.0);
    object.take_damage(20.0, 0, events);
    assert_eq!(object.health.current, 70.0);
    assert_eq!(object.previous_health, 90.0);
    observe_completed_owner(&mut shadow, &mut owner);
    assert_eq!(owner.host_object(id).unwrap().health.current, 70.0);
    assert_eq!(owner.host_object(id).unwrap().previous_health, 90.0);
    assert_eq!(shadow.world().entity(entity).unwrap().health, 70.0);
    assert_eq!(owner.health_events.snapshot_last_damage().len(), 2);
    assert_eq!(owner.health_events.snapshot_last_heal().len(), 1);
    assert!(owner.health_events.snapshot_ordered().is_empty());
}
