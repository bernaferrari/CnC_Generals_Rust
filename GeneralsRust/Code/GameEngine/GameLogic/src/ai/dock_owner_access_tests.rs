use super::*;
pub(super) trait DockTestAccess {
    fn test_slot(&self) -> i32;
    fn test_set_slot(&mut self, slot: i32);
    fn test_state(&self) -> Option<u32>;
    fn test_locked(&self) -> bool;
    fn test_owner(&self) -> ObjectID;
}
impl DockTestAccess for AIDockMachine {
    fn test_slot(&self) -> i32 {
        self.context.approach_position
    }
    fn test_set_slot(&mut self, slot: i32) {
        self.context.approach_position = slot;
    }
    fn test_state(&self) -> Option<u32> {
        self.state_machine.get_current_state_id()
    }
    fn test_locked(&self) -> bool {
        self.state_machine.is_locked()
    }
    fn test_owner(&self) -> ObjectID {
        self.state_machine.get_owner_id()
    }
}
