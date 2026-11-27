//! One ObjectID sequence shared by the match's actors and live projectiles.

use generals_game_domain::ObjectId;

#[derive(Debug)]
pub struct ObjectIds {
    next: ObjectId,
}

impl Default for ObjectIds {
    fn default() -> Self {
        Self::from_next(ObjectId(1))
    }
}

impl ObjectIds {
    /// Restore the saved counter as-is. Allocation checks live occupancy.
    pub fn from_next(next: ObjectId) -> Self {
        Self { next }
    }

    pub fn next(&self) -> ObjectId {
        self.next
    }

    pub fn allocate(&mut self, mut is_in_use: impl FnMut(ObjectId) -> bool) -> ObjectId {
        loop {
            let id = self.next;
            self.next.0 = self.next.0.wrapping_add(1);
            // Older saves may retain live carriers beyond the stored counter.
            // Keep their IDs, including references already held by other actors.
            if id.0 != 0 && !is_in_use(id) {
                return id;
            }
        }
    }
}
