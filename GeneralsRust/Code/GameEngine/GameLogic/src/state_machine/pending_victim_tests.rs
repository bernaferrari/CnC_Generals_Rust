//! Fire completion resolves the current target through its driving owner.
use super::*;
use crate::common::Coord3D;

use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};

fn prepared_pair(target_x: f32) -> (ObjectFactory, Arc<RwLock<Object>>, Arc<RwLock<Object>>) {
    let mut factory = ObjectFactory::new();
    let mut identity_owner = crate::system::game_logic::GameLogic::new();
    let mut objects = Vec::new();
    for (name, x) in [("PendingSource", 0.0), ("PendingTarget", target_x)] {
        let id = factory
            .prepare_object(
                &mut identity_owner,
                Arc::new(crate::common::DefaultThingTemplate::new(name.into())),
                Coord3D::new(x, 0.0, 0.0),
                None,
                ObjectCreationFlags::NO_AI,
            )
            .unwrap();
        objects.push(factory.get_object(id).unwrap().get_base_object().unwrap());
    }
    (factory, objects.remove(0), objects.remove(0))
}

#[test]
fn pending_self_target_releases_owner_loan_before_lookup() {
    let (_factory, owner, _target) = prepared_pair(10.0);
    let owner_id = owner.read().unwrap().get_id();
    owner.write().unwrap().ai_fire_pending_victim = Some(owner_id);
    let mut machine = StateMachine::new(Some(Arc::downgrade(&owner)), "self pending victim");
    machine.apply_pending_victim_goal_with_lookup(|id| {
        assert_eq!(id, owner_id);
        // A retained owner read would prevent this native owner operation.
        assert!(owner.try_write().is_ok());
        Some(owner.clone())
    });
    assert_eq!(machine.get_goal_object_id(), owner_id);
    assert_eq!(machine.get_goal_position().x, 0.0);
}

#[test]
fn same_id_pending_targets_resolve_exact_supplied_store_and_missing_stays_missing() {
    let (_first_factory, first_owner, first_target) = prepared_pair(25.0);
    let (_second_factory, second_owner, second_target) = prepared_pair(75.0);
    let target_id = first_target.read().unwrap().get_id();
    assert_eq!(
        first_owner.read().unwrap().get_id(),
        second_owner.read().unwrap().get_id()
    );
    assert_eq!(target_id, second_target.read().unwrap().get_id());
    assert!(!Arc::ptr_eq(&first_target, &second_target));
    for owner in [&first_owner, &second_owner] {
        owner.write().unwrap().ai_fire_pending_victim = Some(target_id);
    }
    let mut first = StateMachine::new(Some(Arc::downgrade(&first_owner)), "first victim store");
    let mut second = StateMachine::new(Some(Arc::downgrade(&second_owner)), "second victim store");
    first.set_goal_position(Coord3D::new(10.0, 0.0, 0.0));
    second.set_goal_position(Coord3D::new(90.0, 0.0, 0.0));
    first.apply_pending_victim_goal_with_lookup(|id| {
        assert_eq!(id, first_target.read().unwrap().get_id());
        Some(first_target.clone())
    });
    // The peer has no target admitted. Absence must not discover the first
    // world's same-ID target through a compatibility registry fallback.
    second.apply_pending_victim_goal_with_lookup(|id| {
        assert_eq!(id, second_target.read().unwrap().get_id());
        None
    });
    assert_eq!(first.get_goal_position().x, 25.0);
    assert_eq!(second.get_goal_position().x, 90.0);
    second.apply_pending_victim_goal_with_lookup(|_| Some(second_target.clone()));
    assert_eq!(second.get_goal_position().x, 75.0);
}
