use super::*;

#[test]
fn immediate_move_records_with_the_object_owner_and_preserves_force_inputs() {
    use crate::game_logic::{GameLogic, KindOf, Object, Team, ThingTemplate};
    fn world_with_object(id: ObjectId) -> GameLogic {
        let mut world = GameLogic::new();
        let mut template = ThingTemplate::new("OwnerImmediateMove");
        template.add_kind_of(KindOf::Infantry);
        world
            .objects
            .insert(id, Object::new(template, id, Team::USA));
        world
    }
    let id = ObjectId(48_110);
    let mut first = world_with_object(id);
    first.frame = 7;
    let velocity = glam::Vec3::new(-2.0, 0.5, 4.0);
    let path = vec![glam::Vec3::ZERO, glam::Vec3::X];
    let object = first.host_object_mut(id).unwrap();
    object.movement.velocity = velocity;
    object.movement.path = path.clone();
    object.movement.current_path_index = 1;
    let goal = glam::Vec3::new(11.0, 12.0, 13.0);
    assert!(first.move_object_to(id, goal));
    let object = first.host_object(id).unwrap();
    assert_eq!(object.movement.velocity, velocity);
    assert_eq!(object.movement.path, path);
    assert_eq!(object.movement.current_path_index, 1);
    assert_eq!(object.movement.target_position, Some(goal));
    assert!(object.status.moving);
    let receipts = first.movement_events.snapshot_pending().to_vec();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].unit, id);
    assert_eq!(receipts[0].logic_frame, 7);
    assert_eq!(receipts[0].destination, Some(goal.to_array()));

    let mut foreign = world_with_object(id);
    foreign.frame = 9;
    assert!(foreign.move_object_to(id, glam::Vec3::splat(99.0)));
    assert_ne!(
        foreign.movement_events.session(),
        first.movement_events.session()
    );
    assert_eq!(first.movement_events.snapshot_pending(), receipts);
    assert!(!first.move_object_to(ObjectId(48_111), goal));
    assert_eq!(first.movement_events.snapshot_pending(), receipts);
    drop(foreign);
    assert_eq!(first.movement_events.snapshot_pending(), receipts);
}

#[test]
fn completed_steps_append_in_fifo_order_with_producing_frames() {
    let mut events = MovementEvents::default();
    let id = ObjectId(9);
    events.record(u32::MAX, id, Some([1.0, 0.0, 1.0]));
    events.record(u32::MAX, id, Some([2.0, 0.0, 2.0]));
    assert!(
        events.take_completed().is_empty(),
        "pending is not a completed step"
    );
    let first = events.drain_for_host_boundary();
    events.record(0, id, Some([3.0, 0.0, 3.0]));
    let second = events.drain_for_host_boundary();
    let completed = events.take_completed();
    assert_eq!(completed, [first, second].concat());
    assert_eq!(
        completed.iter().map(|e| e.sequence).collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(
        completed.iter().map(|e| e.logic_frame).collect::<Vec<_>>(),
        [u32::MAX, u32::MAX, 0]
    );
    assert!(completed.iter().all(|e| &e.session == events.session()));
    assert!(
        events.take_completed().is_empty(),
        "publication consumes once"
    );
}

#[test]
fn early_admitted_prefix_and_late_suffix_do_not_replay_or_lose_events() {
    let mut events = MovementEvents::default();
    let a = ObjectId(1);
    let b = ObjectId(2);
    events.record(17, a, Some([1.0, 0.0, 0.0]));
    let mut eager = events.drain_for_shadow_boundary();
    eager.mark_shadow_applied();
    events.set_early_batch(eager);
    assert!(!events.has_pending(a));
    events.record(17, b, Some([2.0, 0.0, 0.0]));
    assert!(events.has_pending(b));
    let late = events.drain_for_shadow_boundary();
    assert_eq!(late.events().len(), 2);
    assert_eq!(late.pending_shadow_events().len(), 1);
    assert_eq!(late.pending_shadow_events()[0].unit, b);
    assert_eq!(events.take_completed(), late.events());
    assert!(events.take_completed().is_empty());
}

#[test]
fn mapping_failure_retains_earlier_records_before_late_suffix() {
    let mut events = MovementEvents::default();
    events.record(4, ObjectId(5), Some([5.0, 0.0, 0.0]));
    let batch = events.drain_for_shadow_boundary();
    events.set_early_batch(batch);
    assert!(
        events.has_pending(ObjectId(5)),
        "unadmitted eager handoff is pending"
    );
    events.record(4, ObjectId(6), None);
    let late = events.drain_for_shadow_boundary();
    assert_eq!(late.pending_shadow_events(), late.events());
    assert_eq!(
        late.events().iter().map(|e| e.unit).collect::<Vec<_>>(),
        [ObjectId(5), ObjectId(6)]
    );
    assert_eq!(events.take_completed(), late.events());
}

#[test]
fn another_owner_reset_and_drop_cannot_consume_completed_output() {
    let mut a = MovementEvents::default();
    a.record(8, ObjectId(1), Some([10.0, 0.0, 0.0]));
    a.drain_for_host_boundary();
    let expected = a.snapshot_completed().to_vec();
    {
        let mut b = MovementEvents::default();
        assert_ne!(a.session(), b.session());
        b.record(8, ObjectId(1), Some([20.0, 0.0, 0.0]));
        b.drain_for_host_boundary();
        b.reset();
        assert!(b.take_completed().is_empty());
        b.record(8, ObjectId(1), None);
    }
    assert_eq!(a.take_completed(), expected);
    assert!(a.take_completed().is_empty());
}

#[test]
fn reset_renews_identity_and_removes_pending_completed_and_eager_records() {
    let mut events = MovementEvents::default();
    let old_session = events.session().clone();
    events.record(11, ObjectId(1), None);
    let batch = events.drain_for_shadow_boundary();
    events.set_early_batch(batch);
    events.record(11, ObjectId(2), None);
    events.reset();
    assert_ne!(events.session(), &old_session);
    assert!(!events.has_pending(ObjectId(1)));
    assert!(!events.has_pending(ObjectId(2)));
    assert!(events.take_completed().is_empty());
    assert!(events.drain_for_shadow_boundary().events().is_empty());
    events.record(0, ObjectId(1), None);
    assert_eq!(events.snapshot_pending()[0].sequence, 1);
}

#[test]
fn saved_world_baseline_drops_transient_cues_without_reusing_receipt_numbers() {
    let mut events = MovementEvents::default();
    let session = events.session().clone();
    events.record(17, ObjectId(1), None);
    let batch = events.drain_for_shadow_boundary();
    events.set_early_batch(batch);
    events.record(17, ObjectId(2), None);
    events.clear();
    assert_eq!(&session, events.session());
    assert!(events.take_completed().is_empty());
    assert!(events.drain_for_shadow_boundary().events().is_empty());
    events.record(18, ObjectId(1), Some([1.0, 2.0, 3.0]));
    assert_eq!(events.snapshot_pending()[0].sequence, 3);
}

#[test]
fn foreign_handoff_is_rejected_without_changing_the_owner_output() {
    let mut a = MovementEvents::default();
    let mut b = MovementEvents::default();
    a.record(3, ObjectId(1), Some([1.0, 0.0, 0.0]));
    a.drain_for_host_boundary();
    let expected = a.snapshot_completed().to_vec();
    b.record(3, ObjectId(1), Some([2.0, 0.0, 0.0]));
    let foreign = b.drain_for_shadow_boundary();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.set_early_batch(foreign)))
            .is_err()
    );
    assert_eq!(a.take_completed(), expected);
}

#[test]
fn removed_subject_pruning_keeps_admitted_prefix_and_completed_receipts() {
    let mut events = MovementEvents::default();
    events.record(4, ObjectId(1), Some([1.0, 0.0, 0.0]));
    let mut early = events.drain_for_shadow_boundary();
    early.mark_shadow_applied();
    events.set_early_batch(early);
    events.record(4, ObjectId(2), Some([2.0, 0.0, 0.0]));
    events.record(4, ObjectId(3), Some([3.0, 0.0, 0.0]));
    let mut late = events.drain_for_shadow_boundary();
    late.retain_pending_subjects(|id| id == ObjectId(3));
    assert_eq!(
        late.events().iter().map(|e| e.unit).collect::<Vec<_>>(),
        [ObjectId(1), ObjectId(3)]
    );
    assert_eq!(
        late.pending_shadow_events()
            .iter()
            .map(|e| e.unit)
            .collect::<Vec<_>>(),
        [ObjectId(3)]
    );
    assert_eq!(
        events
            .take_completed()
            .iter()
            .map(|e| e.unit)
            .collect::<Vec<_>>(),
        [ObjectId(1), ObjectId(2), ObjectId(3)]
    );
}
