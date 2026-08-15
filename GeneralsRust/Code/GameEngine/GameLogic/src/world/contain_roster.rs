//! Occupant roster owned by GameWorld (Entity stay host-id residual only).
//!
//! C++ `ContainModule` keeps the live passenger list on the container and
//! `containedBy` on each rider (`Object.cpp` container helpers). Destroy of a
//! container ejects before `processDestroyList` deletes the object
//! (`GameLogic.cpp` destroyObject / processDestroyList).

use super::entities::EntityId;
use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct GameWorldContainRoster {
    occupants: HashMap<u32, Vec<EntityId>>,
    contained_by: HashMap<u32, EntityId>,
}

impl GameWorldContainRoster {
    pub fn clear(&mut self) {
        self.occupants.clear();
        self.contained_by.clear();
    }

    pub fn occupants(&self, id: EntityId) -> &[EntityId] {
        self.occupants
            .get(&id.get())
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn contained_by(&self, id: EntityId) -> Option<EntityId> {
        self.contained_by.get(&id.get()).copied()
    }

    /// Fail-closed: refuse self-contain and missing prior container unlink is ok.
    pub fn enter(&mut self, container: EntityId, occupant: EntityId) -> bool {
        if container == occupant {
            return false;
        }
        self.unlink_occupant(occupant);
        let row = self.occupants.entry(container.get()).or_default();
        if !row.contains(&occupant) {
            row.push(occupant);
        }
        self.contained_by.insert(occupant.get(), container);
        true
    }

    pub fn exit(&mut self, container: EntityId, occupant: EntityId) -> bool {
        let Some(row) = self.occupants.get_mut(&container.get()) else {
            return false;
        };
        let before = row.len();
        row.retain(|id| *id != occupant);
        let changed = row.len() != before;
        if row.is_empty() {
            self.occupants.remove(&container.get());
        }
        if self.contained_by.get(&occupant.get()) == Some(&container) {
            self.contained_by.remove(&occupant.get());
        }
        changed
    }

    pub fn eject_all(&mut self, container: EntityId) -> Vec<EntityId> {
        let occ = self.occupants.remove(&container.get()).unwrap_or_default();
        for occupant in &occ {
            self.contained_by.remove(&occupant.get());
        }
        occ
    }

    pub fn remove(&mut self, id: EntityId) {
        let _ = self.eject_all(id);
        self.unlink_occupant(id);
    }

    fn unlink_occupant(&mut self, occupant: EntityId) {
        let Some(container) = self.contained_by.remove(&occupant.get()) else {
            return;
        };
        if let Some(row) = self.occupants.get_mut(&container.get()) {
            row.retain(|id| *id != occupant);
            if row.is_empty() {
                self.occupants.remove(&container.get());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_exit_roster_and_eject_all() {
        let mut roster = GameWorldContainRoster::default();
        let bunker = EntityId::from_raw(1);
        let rider = EntityId::from_raw(2);
        assert!(roster.enter(bunker, rider));
        assert_eq!(roster.occupants(bunker), &[rider]);
        assert_eq!(roster.contained_by(rider), Some(bunker));
        assert!(roster.exit(bunker, rider));
        assert!(roster.occupants(bunker).is_empty());
        assert!(roster.contained_by(rider).is_none());
        assert!(roster.enter(bunker, rider));
        let ejected = roster.eject_all(bunker);
        assert_eq!(ejected, vec![rider]);
        assert!(roster.contained_by(rider).is_none());
    }

    #[test]
    fn enter_refuses_self_contain() {
        let mut roster = GameWorldContainRoster::default();
        let id = EntityId::from_raw(3);
        assert!(!roster.enter(id, id));
    }
}
