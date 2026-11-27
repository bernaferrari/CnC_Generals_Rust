//! Body writes and publication receipts belong to the world that applied them.
use crate::fixture_content::{apply_fixture_damage, set_fixture_health};
use generals_sim::object::BodyDamageType;
use crate::game_logic::{GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate};
use crate::presentation_frame::{PresentationEvent, PresentationFrame};
use glam::Vec3;

fn admitted_world() -> (GameLogic, ObjectId) {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "HealthOwner", true));
    let mut template = ThingTemplate::new("HealthEventOwnerTarget");
    template
        .set_health(100.0)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable);
    template.authored_instant_death = Some(vec![Default::default()]);
    logic.templates.insert(template.name.clone(), template);
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
    // ActiveBody changes HP and damage state before a later publication boundary.
    let (target, events) = owner.host_object_and_health_events_mut(id).unwrap();
    assert!(!target.take_damage(60.0, 0, events));
    assert_eq!(target.health.current, 40.0);
    assert_eq!(target.previous_health, 100.0);
    assert_eq!(target.body_damage_state, BodyDamageType::Damaged);
    let pending = owner.health_events.snapshot_damage();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].body_observation().current, 40.0);
    other.complete_presentation_observations();
    let peer = other.host_object(other_id).unwrap();
    assert!(peer.is_alive());
    assert_eq!(peer.get_health_percentage(), 1.0);
    let foreign = PresentationFrame::try_build_from_logic(&mut other, 0).unwrap();
    assert_eq!(foreign.objects[0].health_current, 100.0);
    assert_eq!(
        foreign.objects[0].body_damage_state,
        BodyDamageType::Pristine as u8
    );
    assert_eq!(owner.health_events.snapshot_damage(), pending);
    owner.complete_presentation_observations();
    assert!(owner.health_events.snapshot_damage().is_empty());
    assert_eq!(owner.health_events.snapshot_last_damage(), pending);
    let frame = PresentationFrame::try_build_from_logic(&mut owner, 0).unwrap();
    assert_eq!(frame.objects[0].health_current, 40.0);
    assert_eq!(
        frame.objects[0].body_damage_state,
        BodyDamageType::Damaged as u8
    );
    assert_eq!(owner.host_object(id).unwrap().previous_health, 100.0);
}

#[test]
fn another_world_boundary_cannot_admit_the_owners_lethal_damage() {
    let (mut owner, id) = admitted_world();
    let (mut other, other_id) = admitted_world();
    assert_eq!(id, other_id);
    owner.health_events.clear();
    apply_fixture_damage(&mut owner, id, 1000.0);
    assert!(owner.host_object(id).unwrap().status.destroyed);
    other.complete_presentation_observations();
    let foreign = other.host_object(other_id).unwrap();
    assert!(
        foreign.is_alive(),
        "a foreign boundary admitted the owner's death"
    );
    assert_eq!(foreign.health.current, 100.0);
    assert!(!foreign.status.on_die_started);
    assert!(!other.has_pending_destroy_work());
    owner.complete_presentation_observations();
    assert!(owner.host_object(id).unwrap().status.destroyed);
    assert!(owner.objects.ordered_ids().contains(&id));
    assert!(owner.has_pending_destroy_work());
    assert!(other.host_object(other_id).unwrap().is_alive());
    // onDie already ran on this owner. Only its explicit checked endphase
    // performs physical cleanup. Completing receipts preserves the queued object.
    owner
        .process_destroy_list_with_effects_checked(&mut Default::default())
        .unwrap();
    owner.complete_presentation_observations();
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
    other.complete_presentation_observations();
    assert_eq!(other.host_object(other_id).unwrap().health.current, 70.0);
    assert_eq!(owner.health_events.snapshot_ordered(), pending);
    assert_eq!(owner.health_events.cumulative_totals(), (20.0, 0));
    assert_eq!(other.health_events.cumulative_totals(), (30.0, 0));
    owner.complete_presentation_observations();
    assert_eq!(owner.host_object(id).unwrap().health.current, 90.0);
    let receipt = owner.health_events.snapshot_last_damage();
    assert_eq!(id, other_id);
    let foreign = PresentationFrame::try_build_from_logic(&mut other, 0).unwrap();
    assert_eq!(foreign.objects[0].health_current, 70.0);
    assert!(foreign.events.iter().any(|event| matches!(
        event,
        PresentationEvent::DamageApplied { target, amount: 30.0, .. } if *target == other_id
    )));
    assert_eq!(owner.health_events.snapshot_last_damage(), receipt);
    assert_eq!(owner.health_events.snapshot_last_heal().len(), 1);
    let frame = PresentationFrame::try_build_from_logic(&mut owner, 0).unwrap();
    assert_eq!(frame.objects[0].health_current, 90.0);
    assert!(frame.events.iter().any(|event| matches!(
        event,
        PresentationEvent::HealApplied { target, health: 90.0 } if *target == id
    )));
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
        set_fixture_health(&mut other, other_id, 10.0);
        other.reset();
        assert!(other.health_events.snapshot_ordered().is_empty());
        assert_eq!(other.health_events.cumulative_totals(), (0.0, 0));
        assert_eq!(owner.health_events.snapshot_ordered(), pending);
    }
    assert_eq!(owner.health_events.snapshot_ordered(), pending);
    owner.complete_presentation_observations();
    assert_eq!(owner.host_object(id).unwrap().health.current, 80.0);
    owner.reset();
    assert!(owner.health_events.snapshot_ordered().is_empty());
    assert!(owner.health_events.snapshot_last_damage().is_empty());
    assert_eq!(owner.health_events.cumulative_totals(), (0.0, 0));
}

#[test]
fn snapshot_restore_invalidates_only_the_receivers_transient_health_transport() {
    let (mut owner, id) = admitted_world();
    let (mut receiver, receiver_id) = admitted_world();
    assert_eq!(id, receiver_id);
    let (object, events) = owner.host_object_and_health_events_mut(id).unwrap();
    object.take_damage(20.0, 0, events);
    let pending = owner.health_events.snapshot_ordered();
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&owner).unwrap();
    assert_eq!(snapshot.objects[&id].health.current, 80.0);
    assert_eq!(owner.health_events.snapshot_ordered(), pending);
    set_fixture_health(&mut receiver, receiver_id, 50.0);
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
    assert_eq!(
        receiver.host_object(receiver_id).unwrap().health.current,
        80.0
    );
    assert!(receiver.health_events.snapshot_ordered().is_empty());
    assert_eq!(receiver.health_events.cumulative_totals(), (0.0, 0));
    assert_eq!(owner.health_events.snapshot_ordered(), pending);
    assert_eq!(owner.health_events.cumulative_totals(), (20.0, 0));
    owner.complete_presentation_observations();
    assert_eq!(owner.host_object(id).unwrap().health.current, 80.0);
}

#[test]
fn completed_batches_accumulate_receipts_for_one_publication() {
    let (mut owner, id) = admitted_world();
    let (object, events) = owner.host_object_and_health_events_mut(id).unwrap();
    assert!(!object.take_damage(20.0, 0, events));
    owner.complete_presentation_observations();
    let first_hit = owner.health_events.snapshot_last_damage();
    assert_eq!(first_hit.len(), 1);
    assert_eq!(first_hit[0].body_observation().current, 80.0);
    owner.complete_presentation_observations();
    assert_eq!(owner.health_events.snapshot_last_damage(), first_hit);

    let body_frame = owner.frame;
    let (object, events) = owner.host_object_and_health_events_mut(id).unwrap();
    object.heal(10.0, events, body_frame);
    assert_eq!(object.health.current, 90.0);
    assert!(!object.take_damage(20.0, 0, events));
    assert_eq!(object.previous_health, 90.0);
    owner.complete_presentation_observations();
    let hits = owner.health_events.snapshot_last_damage();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0], first_hit[0]);
    assert_eq!(hits[1].body_observation().current, 70.0);

    let frame = PresentationFrame::try_build_from_logic(&mut owner, 0).unwrap();
    assert_eq!(frame.objects[0].health_current, 70.0);
    let receipts: Vec<_> = frame
        .events
        .iter()
        .filter_map(|event| match event {
            PresentationEvent::DamageApplied { target, amount, .. } if *target == id => {
                Some(*amount)
            }
            _ => None,
        })
        .collect();
    assert_eq!(receipts, [20.0, 20.0]);
    assert!(frame.events.iter().any(|event| matches!(
        event, PresentationEvent::HealApplied { target, health: 90.0 } if *target == id
    )));
    assert_eq!(owner.health_events.cumulative_totals(), (40.0, 0));
    assert!(owner.health_events.snapshot_ordered().is_empty());

    let following = PresentationFrame::try_build_from_logic(&mut owner, 0).unwrap();
    assert_eq!(following.objects[0].health_current, 70.0);
    assert!(following.events.iter().all(|event| !matches!(
        event,
        PresentationEvent::DamageApplied { .. } | PresentationEvent::HealApplied { .. }
    )));
}

#[test]
fn body_receipts_cover_nonlethal_zero_and_lethal_damage_once() {
    let (mut logic, ordinary) = admitted_world();
    let zero = logic
        .create_object("HealthEventOwnerTarget", Team::USA, Vec3::X)
        .unwrap();
    let lethal = logic
        .create_object("HealthEventOwnerTarget", Team::USA, Vec3::Z)
        .unwrap();
    logic.health_events.clear_damage();

    // Production Object::take_damage path commits this owner mutation at once.
    {
        let (object, health_events) = logic
            .host_object_and_health_events_mut(ordinary)
            .expect("ordinary");
        assert!(!object.status.destroyed);
        assert!(!object.take_damage(20.0, 0, health_events));
        assert_eq!(object.health.current, 80.0);
        assert_eq!(object.previous_health, 100.0);
        assert!(!object.status.destroyed);
    }
    let ordinary_event = logic.health_events.snapshot_damage();
    assert_eq!(ordinary_event.len(), 1);
    assert_eq!(ordinary_event[0].target, ordinary);
    assert_eq!(ordinary_event[0].amount, 20.0);
    assert!(!ordinary_event[0].destroyed);

    // The presentation boundary records the body's completed write.
    logic.complete_presentation_observations();
    {
        let object = logic
            .host_object(ordinary)
            .expect("ordinary after boundary");
        assert_eq!(object.health.current, 80.0);
        assert_eq!(object.previous_health, 100.0);
        assert!(!object.status.destroyed);
    }
    assert!(logic.health_events.snapshot_damage().is_empty());

    // Another boundary preserves the completed prefix until publication.
    logic.complete_presentation_observations();
    assert_eq!(logic.host_object(ordinary).unwrap().health.current, 80.0);
    assert_eq!(logic.host_object(ordinary).unwrap().previous_health, 100.0);
    assert!(!logic.host_object(ordinary).unwrap().status.destroyed);

    // Zero damage produces no damage event and must leave health/history alone.
    let zero_previous = logic.host_object(zero).unwrap().previous_health;
    {
        let (object, health_events) = logic.host_object_and_health_events_mut(zero).expect("zero");
        assert!(!object.take_damage(0.0, 0, health_events));
        assert_eq!(object.health.current, 100.0);
        assert_eq!(object.previous_health, zero_previous);
    }
    assert!(logic.health_events.snapshot_damage().is_empty());
    logic.complete_presentation_observations();
    assert_eq!(logic.host_object(zero).unwrap().health.current, 100.0);
    assert_eq!(
        logic.host_object(zero).unwrap().previous_health,
        zero_previous
    );
    assert!(!logic.host_object(zero).unwrap().status.destroyed);

    // The actual world's onDie dispatch executes the explicitly authored
    // InstantDeath callback before physical removal at the frame boundary.
    apply_fixture_damage(&mut logic, lethal, 1000.0);
    let object = logic.host_object(lethal).unwrap();
    assert_eq!(object.health.current, 0.0);
    assert_eq!(object.previous_health, 100.0);
    assert!(object.status.destroyed);
    let lethal_event = logic.health_events.snapshot_damage();
    assert_eq!(lethal_event.len(), 1);
    assert_eq!(lethal_event[0].target, lethal);
    assert!(lethal_event[0].destroyed);
    logic.complete_presentation_observations();
    assert!(logic.host_object(lethal).unwrap().status.destroyed);
    assert_eq!(logic.host_object(lethal).unwrap().health.current, 0.0);
    assert!(logic.objects.ordered_ids().contains(&lethal));
    assert!(logic.has_pending_destroy_work());
    let completed = logic.health_events.snapshot_last_damage();
    assert_eq!(completed.len(), 2);
    assert_eq!(completed[0].target, ordinary);
    assert!(!completed[0].destroyed);
    assert_eq!(completed[1].target, lethal);
    assert!(completed[1].destroyed);
    // CPP GameLogic.cpp3762 is the physical endphase, before the next frame.
    logic
        .process_destroy_list_with_effects_checked(&mut Default::default())
        .unwrap();
    logic.complete_presentation_observations();
    assert!(logic.host_object(lethal).is_none());
    assert!(!logic.has_pending_destroy_work());
    assert_eq!(logic.health_events.snapshot_last_damage(), completed);
    logic.complete_presentation_observations();
    assert!(logic.host_object(lethal).is_none());
    assert!(!logic.has_pending_destroy_work());
    assert_eq!(logic.health_events.snapshot_last_damage(), completed);

    let published = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    assert!(published.objects.iter().all(|row| row.id != lethal));
    let damage_receipts: Vec<_> = published
        .events
        .iter()
        .filter_map(|event| match event {
            PresentationEvent::DamageApplied {
                target, destroyed, ..
            } => Some((*target, *destroyed)),
            _ => None,
        })
        .collect();
    assert_eq!(damage_receipts, [(ordinary, false), (lethal, true)]);
}
