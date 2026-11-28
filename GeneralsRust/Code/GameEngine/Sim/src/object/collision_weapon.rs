//! Authored FireWeaponCollide eligibility from FireWeaponCollide.cpp:70–89.

use super::ObjectStatusMaskType;

#[derive(Debug, Clone)]
pub struct CollisionWeapon {
    pub weapon_name: String,
    pub gate: CollisionWeaponGate,
}

#[derive(Debug, Clone, Copy)]
pub struct CollisionWeaponGate {
    pub fire_once: bool,
    pub required_status: ObjectStatusMaskType,
    pub forbidden_status: ObjectStatusMaskType,
}

impl CollisionWeaponGate {
    pub const UNRESTRICTED: Self = Self {
        fire_once: false,
        required_status: ObjectStatusMaskType::empty(),
        forbidden_status: ObjectStatusMaskType::empty(),
    };

    pub fn can_fire(&self, status: ObjectStatusMaskType, ever_fired: bool) -> bool {
        status.contains(self.required_status)
            && !status.intersects(self.forbidden_status)
            && !(ever_fired && self.fire_once)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_fire_matches_cpp_status_gates() {
        let spec = CollisionWeaponGate {
            fire_once: true,
            required_status: ObjectStatusMaskType::DETECTED | ObjectStatusMaskType::CAN_ATTACK,
            forbidden_status: ObjectStatusMaskType::STEALTHED,
        };
        let allowed = spec.required_status;
        assert!(spec.can_fire(allowed, false));
        assert!(!spec.can_fire(ObjectStatusMaskType::DETECTED, false));
        assert!(!spec.can_fire(allowed | spec.forbidden_status, false));
        assert!(!spec.can_fire(allowed, true));
    }
}
