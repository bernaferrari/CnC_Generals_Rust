//! Actual control is loaned beside the active body; it is never a mirror.
use super::*;

#[derive(Debug)]
struct ControlState {
    owner: Arc<RwLock<Object>>,
    enclosing: Weak<Mutex<StateMachine>>,
    attack: bool,
    requested: Option<StateId>,
    request_transition: bool,
}
impl StateImplementation for ControlState {
    fn update(&mut self) -> StateReturnType {
        panic!("live-control driver must select the borrowed callback")
    }
    fn update_with_control(
        &mut self,
        control: &mut StateMachineControl,
        _ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
        machine_locked: bool,
        _owner: &mut dyn Any,
    ) -> StateReturnType {
        assert!(self.enclosing.upgrade().unwrap().try_lock().is_err());
        assert!(Arc::ptr_eq(&control.get_owner().unwrap(), &self.owner));
        assert_eq!(control.get_current_state_id(), Some(5));
        assert_eq!(control.is_locked(), machine_locked);
        control.set_goal_object(Some(Arc::downgrade(&self.owner)));
        control.set_goal_position(Coord3D::new(13.0, 17.0, 19.0));
        self.attack = true;
        if self.request_transition {
            self.requested = Some(7);
        }
        StateReturnType::Sleep(31)
    }
    fn take_requested_state_change(&mut self) -> Option<StateId> {
        self.requested.take()
    }
    fn is_attack(&self) -> bool {
        self.attack
    }
}
#[derive(Debug)]
struct TargetState;
impl StateImplementation for TargetState {
    fn on_enter_with_ai(
        &mut self,
        _ai: &mut dyn crate::modules::AIUpdateInterface,
        goal_id: crate::common::ObjectID,
        goal_pos: Coord3D,
    ) -> StateReturnType {
        assert_eq!(goal_id, 0x7af10101);
        assert_eq!(goal_pos, Coord3D::new(13.0, 17.0, 19.0));
        StateReturnType::Continue
    }
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Continue
    }
}
#[derive(Debug)]
struct TestAI;
impl crate::modules::AIUpdateInterface for TestAI {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    fn is_moving(&self) -> bool {
        false
    }
    fn is_idle(&self) -> bool {
        true
    }
    fn set_movement_target(&mut self, _: &Coord3D) -> Result<(), String> {
        Ok(())
    }
}
fn machine(transition: bool) -> Arc<Mutex<StateMachine>> {
    let owner = Arc::new(RwLock::new(Object::new_test(0x7af10101, 100.0)));
    let core = Arc::new(Mutex::new(StateMachine::new(
        Some(Arc::downgrade(&owner)),
        "live control",
    )));
    {
        let mut guard = core.lock().unwrap();
        guard.define_state(
            5,
            Box::new(ControlState {
                owner,
                enclosing: Arc::downgrade(&core),
                attack: false,
                requested: None,
                request_transition: transition,
            }),
            None,
            None,
            None,
        );
        guard.define_state(7, Box::new(TargetState), None, None, None);
        assert_eq!(guard.init_default_state(), StateReturnType::Continue);
    }
    core
}
#[test]
fn borrowed_owner_callback_changes_actual_goal_and_body_classification() {
    let _serial = crate::test_sync::lock();
    let core = machine(false);
    let mut guard = core.lock().unwrap();
    assert!(!guard.is_in_attack_state());
    assert_eq!(guard.update_with_owner(&mut ()), StateReturnType::Sleep(31));
    assert_eq!(guard.get_goal_position(), Coord3D::new(13.0, 17.0, 19.0));
    assert_eq!(guard.get_goal_object_id(), 0x7af10101);
    assert!(
        guard.is_in_attack_state(),
        "classification must read the live body"
    );
}
#[test]
fn borrowed_ai_callback_goal_reaches_next_entry_before_sleep() {
    let _serial = crate::test_sync::lock();
    let core = machine(true);
    let mut guard = core.lock().unwrap();
    assert_eq!(guard.update_with_ai(&mut TestAI), StateReturnType::Continue);
    assert_eq!(guard.get_current_state_id(), Some(7));
    assert_eq!(guard.control.sleep_till, 0);
    assert_eq!(guard.get_goal_position(), Coord3D::new(13.0, 17.0, 19.0));
}
#[test]
fn logical_lock_rejects_goal_writes_and_tail_transition_on_live_control() {
    let _serial = crate::test_sync::lock();
    let core = machine(true);
    let mut guard = core.lock().unwrap();
    guard.set_goal_position(Coord3D::new(1.0, 2.0, 3.0));
    guard.lock();
    assert_eq!(
        guard.update_with_ai(&mut TestAI),
        StateReturnType::Sleep(31)
    );
    assert_eq!(guard.get_current_state_id(), Some(5));
    assert_eq!(guard.get_goal_position(), Coord3D::new(1.0, 2.0, 3.0));
    assert_eq!(guard.get_goal_object_id(), crate::common::INVALID_ID);
    assert!(guard.is_locked());
    assert!(guard.is_in_attack_state());
}
