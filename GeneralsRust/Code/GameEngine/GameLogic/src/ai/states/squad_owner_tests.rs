//! AIStates.cpp:1040-1071 copies squad membership; Squad.cpp:84-98 filters a cache.
use super::AIStateMachine;
use crate::ai::squad::Squad;
use crate::object::Object;
use game_engine::common::system::{Snapshotable, xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;
use std::sync::{Arc, RwLock};

fn squad(ids: &[u32]) -> Squad {
    let mut result = Squad::new();
    for &id in ids {
        result.add_object_id(id);
    }
    result
}
fn goal(ids: &[u32]) -> Arc<Squad> {
    Arc::new(squad(ids))
}
fn ids(goal: &Arc<Squad>) -> Vec<u32> {
    goal.get_object_ids().clone()
}
fn machine(owner: &Arc<RwLock<Object>>) -> AIStateMachine {
    AIStateMachine::new(Arc::downgrade(owner), "owned squad")
}
fn save(machine: &mut AIStateMachine) -> Vec<u8> {
    let mut bytes = Vec::new();
    machine
        .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
        .unwrap();
    bytes
}
#[test]
fn assigning_squad_copies_membership_and_keeps_source_independent() {
    let _serial = crate::test_sync::lock();
    let owner = Arc::new(RwLock::new(Object::new_test(0x7AF11010, 100.0)));
    let mut source_machine = machine(&owner);
    source_machine.set_goal_squad(Some(goal(&[19, 7, 19, 11])));
    let mut source = source_machine.get_goal_squad().unwrap().clone();
    let mut destination = machine(&owner);
    destination.set_goal_squad(Some(source.clone()));
    Arc::make_mut(&mut source).add_object_id(23);
    assert_eq!(ids(&destination.get_goal_squad().unwrap()), vec![19, 7, 11]);
    assert_eq!(ids(&source), vec![19, 7, 11, 23]);
    assert_eq!(
        ids(&destination.base.get_goal_squad().unwrap()),
        vec![19, 7, 11]
    );
}
#[test]
fn same_id_machine_goals_stay_separate_during_interleaved_replacement_and_clear() {
    let _serial = crate::test_sync::lock();
    let first = Arc::new(RwLock::new(Object::new_test(0x7AF11011, 100.0)));
    let second = Arc::new(RwLock::new(Object::new_test(0x7AF11011, 200.0)));
    let mut a = machine(&first);
    let mut b = machine(&second);
    a.set_goal_squad(Some(goal(&[3, 5])));
    b.set_goal_squad(Some(goal(&[13, 17])));
    a.set_goal_squad(Some(goal(&[29, 31])));
    assert_eq!(ids(&a.base.get_goal_squad().unwrap()), vec![29, 31]);
    assert_eq!(ids(&b.base.get_goal_squad().unwrap()), vec![13, 17]);
    a.clear();
    assert!(a.get_goal_squad().is_none());
    assert!(a.base.get_goal_squad().is_none());
    assert_eq!(ids(&b.get_goal_squad().unwrap()), vec![13, 17]);
}
#[test]
fn restoring_squad_replaces_existing_membership_and_resaves_identical_bytes() {
    let _serial = crate::test_sync::lock();
    let owner = Arc::new(RwLock::new(Object::new_test(0x7AF11012, 100.0)));
    let mut source = machine(&owner);
    assert_eq!(
        source.base.init_default_state(),
        crate::state_machine::StateReturnType::Continue
    );
    source.set_goal_squad(Some(goal(&[41, 43, 47])));
    let bytes = save(&mut source);
    let mut loaded = machine(&owner);
    loaded.set_goal_squad(Some(goal(&[101, 103])));
    loaded
        .xfer(&mut XferLoad::new(Cursor::new(bytes.clone()), 1))
        .unwrap();
    assert_eq!(ids(&loaded.get_goal_squad().unwrap()), vec![41, 43, 47]);
    assert_eq!(
        ids(&loaded.base.get_goal_squad().unwrap()),
        vec![41, 43, 47]
    );
    assert_eq!(save(&mut loaded), bytes);
}
#[test]
fn live_member_query_does_not_change_authored_membership() {
    let _serial = crate::test_sync::lock();
    let owner = Arc::new(RwLock::new(Object::new_test(0x7AF11013, 100.0)));
    let mut machine = machine(&owner);
    machine.set_goal_squad(Some(goal(&[0x7AF11101, 0x7AF11102])));
    let mut goal = machine.get_goal_squad().unwrap().clone();
    let before = ids(&goal);
    assert_eq!(Arc::make_mut(&mut goal).get_live_object_ids(), before);
    assert_eq!(ids(&goal), before);
    assert_eq!(ids(&machine.get_goal_squad().unwrap()), before);
}
