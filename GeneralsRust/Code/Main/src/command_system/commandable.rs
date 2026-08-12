use super::*;

// Extension methods for Object to support command system
pub trait CommandableObject {
    fn can_move(&self) -> bool;
    fn can_attack(&self) -> bool;
    fn can_construct(&self) -> bool;
    fn can_repair(&self) -> bool;
    fn can_contain(&self) -> bool;
    fn is_damaged(&self) -> bool;
    fn is_injured(&self) -> bool;
    fn is_dead(&self) -> bool;
    fn is_medical_facility(&self) -> bool;
    fn provides_repair(&self) -> bool;
    fn provides_healing(&self) -> bool;
    fn has_capacity_for(&self, other: &Object) -> bool;
    fn set_destination(&mut self, destination: Vec3);
    fn set_target(&mut self, target: Option<ObjectId>);
    fn set_target_location(&mut self, location: Option<Vec3>);
    fn set_guard_position(&mut self, position: Option<Vec3>);
    fn set_guard_target(&mut self, target: Option<ObjectId>);
    fn set_force_attack(&mut self, force: bool);
    fn stop(&mut self);
}

impl CommandableObject for Object {
    fn can_move(&self) -> bool {
        // Check if object has mobility
        matches!(
            self.object_type,
            crate::game_logic::ObjectType::Vehicle
                | crate::game_logic::ObjectType::Infantry
                | crate::game_logic::ObjectType::Aircraft
        )
    }

    fn can_attack(&self) -> bool {
        // Check if object has weapons
        self.health.current > 0.0
            && !matches!(self.object_type, crate::game_logic::ObjectType::Supply)
    }

    fn can_construct(&self) -> bool {
        // C++ ActionManager / DozerAI routes construction through
        // KINDOF_DOZER.  A worker/harvester spelling is not authority.
        self.can_move() && self.is_kind_of(crate::game_logic::KindOf::Dozer)
    }

    fn can_repair(&self) -> bool {
        self.can_move() && self.is_kind_of(crate::game_logic::KindOf::Dozer)
    }

    fn can_contain(&self) -> bool {
        Object::can_contain(self)
    }

    fn is_damaged(&self) -> bool {
        self.health.current < self.max_health && self.health.current > 0.0
    }

    fn is_injured(&self) -> bool {
        self.is_damaged() // Same as damaged for now
    }

    fn is_dead(&self) -> bool {
        self.health.current <= 0.0
    }

    fn is_medical_facility(&self) -> bool {
        self.is_kind_of(crate::game_logic::KindOf::HealPad)
    }

    fn provides_repair(&self) -> bool {
        // This legacy no-source-argument probe means the ground-vehicle
        // Repair target only.  Aircraft repair deliberately stays out of it:
        // `ActionManager::canGetRepairedAt` pairs an aircraft exclusively
        // with `KINDOF_FS_AIRFIELD`, and the command/executor paths make that
        // source-aware check directly.
        self.is_kind_of(crate::game_logic::KindOf::RepairPad)
    }

    fn provides_healing(&self) -> bool {
        self.is_kind_of(crate::game_logic::KindOf::HealPad)
    }

    fn has_capacity_for(&self, _other: &Object) -> bool {
        Object::has_capacity_for(self, 1)
    }

    fn set_destination(&mut self, destination: Vec3) {
        Object::set_destination(self, destination);
    }

    fn set_target(&mut self, target: Option<ObjectId>) {
        Object::set_target(self, target);
    }

    fn set_target_location(&mut self, location: Option<Vec3>) {
        Object::set_target_location(self, location);
    }

    fn set_guard_position(&mut self, position: Option<Vec3>) {
        Object::set_guard_position(self, position);
    }

    fn set_guard_target(&mut self, target: Option<ObjectId>) {
        Object::set_guard_target(self, target);
    }

    fn set_force_attack(&mut self, force: bool) {
        Object::set_force_attack(self, force);
    }

    fn stop(&mut self) {
        Object::stop(self);
    }
}
