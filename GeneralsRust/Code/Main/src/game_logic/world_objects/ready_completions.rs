//! Receiving Main object ID allocation.
use super::super::*;
impl GameLogic {
    pub(in super::super) fn allocate_object_id(&mut self) -> ObjectId {
        let id = self.next_object_id;
        self.next_object_id = ObjectId(self.next_object_id.0 + 1);
        id
    }
}
