//! Original attached PartitionData identity and first-dirty scheduling.
//! This metadata augments the existing object->COI map; it is not an Object owner.
use std::sync::{Arc, Weak};
#[derive(Debug, Default)]
pub(super) struct PartitionObject {
    pub(super) cells: Vec<(i32, i32)>,
    pub(super) birth: Option<Weak<()>>,
    pub(super) identity: Arc<()>,
    /// C++ m_lastCell; advanced only after original cell-maintenance callback.
    pub(super) last_cell: Option<(i32, i32)>,
}
#[derive(Debug)]
pub(in crate::game_logic) struct PartitionDirty {
    pub(in crate::game_logic) object_id: u32,
    pub(in crate::game_logic) update_cells: bool,
    pub(in crate::game_logic) check_collisions: bool,
    pub(super) birth: Weak<()>,
}
impl PartitionDirty {
    pub(in crate::game_logic) fn matches(&self, birth: &Arc<()>) -> bool {
        Weak::ptr_eq(&self.birth, &Arc::downgrade(birth))
    }
}
impl super::PartitionManager {
    /// ThingFactory.cpp317: after all onCreate, before initObject/Drawable.
    /// Allocation attaches a module and marks it dirty, but does not fill cells.
    pub(in crate::game_logic) fn namespace(&self) -> &Arc<()> {
        &self.namespace
    }

    pub(in crate::game_logic) fn attach_receiving_object(
        &mut self,
        object: &mut crate::game_logic::Object,
    ) {
        // Reject an observation clone/foreign attachment before mutating cells.
        object.validate_partition_namespace(&self.namespace);
        self.attach_receiving_identity(object.id.0, &object.visual_draw_identity_token);
        object.bind_partition_namespace(&self.namespace);
    }

    fn attach_receiving_identity(&mut self, id: u32, birth: &Arc<()>) {
        if let Some(module) = self.object_cells.get(&id) {
            assert!(
                module
                    .birth
                    .as_ref()
                    .is_some_and(|old| Weak::ptr_eq(old, &Arc::downgrade(birth))),
                "partition attachment cannot retarget an existing Object lifetime"
            );
            return;
        }
        self.object_cells.insert(
            id,
            PartitionObject {
                cells: Vec::new(),
                birth: Some(Arc::downgrade(birth)),
                identity: Arc::new(()),
                last_cell: None,
            },
        );
        self.mark_receiving_object_dirty(id, birth, true);
    }
    pub(in crate::game_logic) fn receiving_object_is_attached(
        &self,
        id: u32,
        birth: &Arc<()>,
    ) -> bool {
        self.object_cells
            .get(&id)
            .and_then(|module| module.birth.as_ref())
            .is_some_and(|old| Weak::ptr_eq(old, &Arc::downgrade(birth)))
    }
    /// First dirty request prepends; subsequent requests upgrade flags in place.
    /// A detached Object has no PartitionData and cannot dirty this owner.
    pub(in crate::game_logic) fn mark_receiving_object_dirty(
        &mut self,
        id: u32,
        birth: &Arc<()>,
        cells: bool,
    ) {
        if !self
            .object_cells
            .get(&id)
            .and_then(|module| module.birth.as_ref())
            .is_some_and(|old| Weak::ptr_eq(old, &Arc::downgrade(birth)))
        {
            return;
        }
        if let Some(dirty) = self.dirty.iter_mut().find(|dirty| dirty.object_id == id) {
            assert!(
                dirty.matches(birth),
                "pending partition dirty lifetime mismatch"
            );
            dirty.update_cells |= cells;
            dirty.check_collisions = true;
        } else {
            self.dirty.push(PartitionDirty {
                object_id: id,
                update_cells: cells,
                check_collisions: true,
                birth: Arc::downgrade(birth),
            });
        }
    }
    pub(in crate::game_logic) fn take_next_receiving_dirty(&mut self) -> Option<PartitionDirty> {
        self.dirty.pop()
    }
    pub(in crate::game_logic) fn receiving_transform_changed(
        &mut self,
        object: &crate::game_logic::Object,
        old: glam::Vec3,
        old_angle: f32,
    ) {
        let current = object.get_position();
        if (old.x - current.x).abs() > 0.01
            || (old.y - current.y).abs() > 0.01
            || (old.z - current.z).abs() > 0.01
            || (old_angle - object.get_orientation()).abs() > 0.01
        {
            self.mark_receiving_object_dirty(object.id.0, &object.visual_draw_identity_token, true);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_partition_owner_preserves_first_dirty_upgrade_and_birth_identity() {
        let mut owner = super::super::PartitionManager::new();
        let a = Arc::new(());
        let b = Arc::new(());
        owner.attach_receiving_identity(1, &a);
        owner.attach_receiving_identity(2, &b);
        owner.mark_receiving_object_dirty(1, &a, true);
        assert_eq!(owner.take_next_receiving_dirty().unwrap().object_id, 2);
        assert_eq!(owner.take_next_receiving_dirty().unwrap().object_id, 1);
        assert!(owner.take_next_receiving_dirty().is_none());
        owner.mark_receiving_object_dirty(1, &a, false);
        owner.mark_receiving_object_dirty(2, &b, false);
        owner.mark_receiving_object_dirty(1, &a, true);
        assert_eq!(owner.take_next_receiving_dirty().unwrap().object_id, 2);
        let first = owner.take_next_receiving_dirty().unwrap();
        assert_eq!(first.object_id, 1);
        assert!(first.update_cells);
        let mut peer = super::super::PartitionManager::new();
        peer.attach_receiving_identity(1, &Arc::new(()));
        owner.unregister_object(1);
        assert!(owner.take_next_receiving_dirty().is_none());
        assert_eq!(peer.take_next_receiving_dirty().unwrap().object_id, 1);
    }
}
