//! CPP GarrisonContain.cpp529–557: Open rules, live body, NoGarrison, count.
use super::*;

impl GarrisonContain {
    /// Standalone entry for a caller without an existing container borrow.
    pub fn is_valid_container_for(&self, passenger: &Object, check_capacity: bool) -> bool {
        if !self.base.is_valid_container_for(passenger, check_capacity) {
            return false;
        }
        self.with_owner_object(|owner| self.admits_after_open(passenger, owner, check_capacity))
            .unwrap_or(false)
    }

    pub(super) fn admits_with_owner(
        &self,
        passenger: &Object,
        owner: &Object,
        check_capacity: bool,
    ) -> bool {
        // C++ evaluates OpenContain first. This also rejects wrong relations
        // before looking at body state and avoids borrowing the owner again.
        self.base
            .is_valid_container_for_with_owner(passenger, owner, check_capacity)
            && self.admits_after_open(passenger, owner, check_capacity)
    }

    fn admits_after_open(&self, passenger: &Object, owner: &Object, check_capacity: bool) -> bool {
        let Some(body) = owner.get_body_module() else {
            return false;
        };
        let Ok(body) = body.lock() else {
            return false;
        };
        if body.get_health() <= 0.0
            || (body.get_damage_state() == BodyDamageType::ReallyDamaged
                && !owner.is_kind_of(KindOf::GarrisonableUntilDestroyed))
        {
            return false;
        }
        drop(body);
        if passenger.is_kind_of(KindOf::NoGarrison) {
            return false;
        }
        !check_capacity || (self.base.get_contain_count() as i32) < self.base.get_contain_max()
    }
}
