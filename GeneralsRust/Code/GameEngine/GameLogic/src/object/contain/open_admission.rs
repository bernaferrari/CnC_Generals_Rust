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
        kind_mask_admits(
            obj.get_kind_of(),
            self.module_data.allow_inside_kind_of,
            self.module_data.forbid_inside_kind_of,
        )
    }

    fn accepts_relationship(&self, obj: &Object, owner: &Object) -> bool {
        self.accepts_relationship_value(obj.get_relationship_to(owner))
    }

    pub(super) fn is_valid_container_for_with_world(
        &self,
        obj: &Object,
        owner: &Object,
        _capacity: bool,
        world: &dyn crate::modules::ai_world::AiWorld,
    ) -> bool {
        self.accepts_kind(obj) && self.accepts_relationship_value(world.relationship(obj, owner))
    }

    fn accepts_relationship_value(&self, relationship: ObjectRelationship) -> bool {
        match relationship {
            ObjectRelationship::Ally => self.module_data.allow_allies_inside,
            ObjectRelationship::Enemy => self.module_data.allow_enemies_inside,
            ObjectRelationship::Neutral => self.module_data.allow_neutral_inside,
            _ => false,
        }
    }
}

/// C++ OpenContain.cpp:862 `isAnyKindOf(allow)` then the forbid test. An empty
/// allow mask, or an entrant with no kinds, matches nothing; "everything
/// allowed" is the all-bits default of `OpenContainModuleData`.
fn kind_mask_admits(
    obj_kind: crate::common::KindOfMaskType,
    allow: crate::common::KindOfMaskType,
    forbid: crate::common::KindOfMaskType,
) -> bool {
    (obj_kind & allow) != 0 && (obj_kind & forbid) == 0
}

#[cfg(test)]
mod kind_mask_tests {
    use super::kind_mask_admits;
    use crate::common::KindOf;
    use crate::object::contain::open_contain::OpenContainModuleData;

    #[test]
    fn default_allows_every_kind_but_not_a_kindless_entrant() {
        let data = OpenContainModuleData::default();
        let infantry = KindOf::Infantry.cpp_mask();
        let vehicle = KindOf::Vehicle.cpp_mask();
        assert!(kind_mask_admits(infantry, data.allow_inside_kind_of, data.forbid_inside_kind_of));
        assert!(kind_mask_admits(vehicle, data.allow_inside_kind_of, data.forbid_inside_kind_of));
        assert!(!kind_mask_admits(0, data.allow_inside_kind_of, data.forbid_inside_kind_of));
    }

    #[test]
    fn explicit_empty_allow_mask_admits_nothing_and_forbid_still_applies() {
        let infantry = KindOf::Infantry.cpp_mask();
        assert!(!kind_mask_admits(infantry, 0, 0));
        assert!(kind_mask_admits(infantry, infantry, 0));
        assert!(!kind_mask_admits(infantry, infantry, infantry));
        assert!(!kind_mask_admits(KindOf::Vehicle.cpp_mask(), infantry, 0));
    }
}
