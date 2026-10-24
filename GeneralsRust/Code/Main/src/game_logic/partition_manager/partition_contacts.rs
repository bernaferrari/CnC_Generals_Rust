//! Operation-local PartitionData lifetime handles for original contact gathering.
use std::sync::{Arc, Weak};
#[derive(Debug)]
pub(in crate::game_logic) struct PartitionContactObject {
    pub(in crate::game_logic) id: u32,
    actor: Weak<()>,
    module: Weak<()>,
}
#[derive(Debug)]
pub(in crate::game_logic) struct PartitionContact {
    pub(in crate::game_logic) first: PartitionContactObject,
    pub(in crate::game_logic) second: PartitionContactObject,
}
impl super::PartitionManager {
    pub(in crate::game_logic) fn receiving_contact_object(
        &self,
        object: &crate::game_logic::Object,
    ) -> Option<PartitionContactObject> {
        let module = self.object_cells.get(&object.id.0)?;
        if !module.birth.as_ref().is_some_and(|birth| {
            Weak::ptr_eq(birth, &Arc::downgrade(&object.visual_draw_identity_token))
        }) {
            return None;
        }
        Some(PartitionContactObject {
            id: object.id.0,
            actor: Arc::downgrade(&object.visual_draw_identity_token),
            module: Arc::downgrade(&module.identity),
        })
    }
    pub(in crate::game_logic) fn contact_object_is_current(
        &self,
        contact: &PartitionContactObject,
        object: &crate::game_logic::Object,
    ) -> bool {
        contact.id == object.id.0
            && Weak::ptr_eq(
                &contact.actor,
                &Arc::downgrade(&object.visual_draw_identity_token),
            )
            && self.object_cells.get(&contact.id).is_some_and(|module| {
                Weak::ptr_eq(&contact.module, &Arc::downgrade(&module.identity))
            })
    }
    /// Original addPossibleCollisions walks our COI allocation order, then
    /// each cell's prepend list. Duplication across cells is removed only by
    /// the contact list's first-pair admission, without sorting either side.
    pub(in crate::game_logic) fn neighbor_ids_in_coi_order(&self, id: u32) -> Vec<u32> {
        let Some(module) = self.object_cells.get(&id) else {
            return Vec::new();
        };
        let mut ids = Vec::new();
        for cell in &module.cells {
            if let Some(occupants) = self.cells.get(cell) {
                if occupants.len() < 2 {
                    continue;
                }
                for other in occupants.iter().rev().copied() {
                    if other != id {
                        ids.push(other);
                    }
                }
            }
        }
        ids
    }
}
