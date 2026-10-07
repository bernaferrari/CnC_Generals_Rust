//! Admission rules from C++ OpenContain.cpp:856-904.
//! Callers holding the container Object use its existing borrow.

use super::{Object, ObjectRelationship, OpenContain};

impl OpenContain {
    /// Compatibility entry for callers that do not hold the owning Object.
    pub fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        if !self.accepts_kind(obj) {
            return false;
        }
        let _ = check_capacity;
        let owner_id = self.get_object_id();
        if owner_id == crate::common::INVALID_ID {
            return true;
        }
        let Some(owner_arc) = self.get_object() else {
            return false;
        };
        let Ok(owner) = owner_arc.try_read() else {
            return false;
        };
        self.accepts_relationship(obj, &owner)
    }

    pub(super) fn is_valid_container_for_with_owner(
        &self,
        obj: &Object,
        owner: &Object,
        check_capacity: bool,
    ) -> bool {
        // OpenContain deliberately leaves capacity checks to its derived containers.
        let _ = check_capacity;
        self.accepts_kind(obj) && self.accepts_relationship(obj, owner)
    }

    fn accepts_kind(&self, obj: &Object) -> bool {
        // Check kind restrictions
        let obj_kind = obj.get_kind_of();

        if self.module_data.allow_inside_kind_of != 0
            && (obj_kind & self.module_data.allow_inside_kind_of) == 0
        {
            return false;
        }

        // Must have none of the forbidden kind bits
        if (obj_kind & self.module_data.forbid_inside_kind_of) != 0 {
            return false;
        }

        true
    }

    fn accepts_relationship(&self, obj: &Object, owner: &Object) -> bool {
        let relationship = obj.get_relationship_to(owner);
        match relationship {
            ObjectRelationship::Ally => self.module_data.allow_allies_inside,
            ObjectRelationship::Enemy => self.module_data.allow_enemies_inside,
            ObjectRelationship::Neutral => self.module_data.allow_neutral_inside,
            _ => false,
        }
    }
}
