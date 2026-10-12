//! Session-owned idle-worker presentation events.

use crate::common::{Int, ObjectID};

#[derive(Debug, Default)]
pub struct IdleWorkerEvents {
    additions: Vec<(ObjectID, Int)>,
    removals: Vec<(ObjectID, Int)>,
}

impl IdleWorkerEvents {
    pub fn add(&mut self, object: ObjectID, player: Int) {
        self.additions.push((object, player));
    }

    pub fn remove(&mut self, object: ObjectID, player: Int) {
        self.removals.push((object, player));
    }

    pub fn take(&mut self) -> (Vec<(ObjectID, Int)>, Vec<(ObjectID, Int)>) {
        (
            std::mem::take(&mut self.additions),
            std::mem::take(&mut self.removals),
        )
    }
}
