//! Squish collide module – ports the legacy tank-crush logic.
//!
//! Mirrors the behaviour of the C++ `SquishCollide` implementation: infantry that
//! are run down by vehicles with a non-zero crusher level receive an immediate
//! crush damage packet provided the attacking vehicle is actually intersecting
//! and moving towards the victim.

use super::{
    CollideModule as CollideModuleTrait, CollisionError, Coord3D, GameObject, ObjectId, PlayerId,
    Relationship,
};
use crate::damage::{
    DamageInfo, DamageInfoInput, DamageInfoOutput, DamageType, DeathType, HUGE_DAMAGE_AMOUNT,
};
use crate::modules::PhysicsBehavior;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{ModuleData, NameKeyType};
use log::trace;
use std::sync::{Arc, Mutex, RwLock};

/// Legacy compatibility – the original code makes our geometry a 1.0 radius disk.
const TARGET_COLLISION_RADIUS: f32 = 1.0;

/// Module data for `SquishCollide`. The legacy C++ module does not expose any
/// additional INI-configurable state, but we retain the module tag to keep the
/// module factory parity intact.
#[derive(Debug, Clone, Default)]
pub struct SquishCollideModuleData {
    module_tag_name_key: NameKeyType,
}

impl SquishCollideModuleData {
    pub fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    pub fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl crate::common::LegacyModuleData for SquishCollideModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl Snapshotable for SquishCollideModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("SquishCollideModuleData xfer version: {e:?}"))?;

        xfer.xfer_unsigned_int(&mut self.module_tag_name_key)
            .map_err(|e| format!("SquishCollideModuleData module_tag_name_key: {e:?}"))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for SquishCollide {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1);
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Runtime state for the crush collision module.
#[derive(Debug)]
pub struct SquishCollide {
    owner_id: ObjectId,
    module_data: Arc<SquishCollideModuleData>,
    #[allow(dead_code)]
    version: u32,
}

impl SquishCollide {
    /// Construct a new collision module for the supplied object.
    pub fn new(owner_id: ObjectId, module_data: Arc<SquishCollideModuleData>) -> Self {
        Self {
            owner_id,
            module_data,
            version: 1,
        }
    }

    /// Provide read-only access to the module data.
    pub fn module_data(&self) -> &SquishCollideModuleData {
        &self.module_data
    }

    fn owner_handle(&self) -> Result<Arc<RwLock<Object>>, CollisionError> {
        OBJECT_REGISTRY.get_object(self.owner_id).ok_or_else(|| {
            CollisionError::InvalidObject(format!(
                "SquishCollide owner {} missing from registry",
                self.owner_id
            ))
        })
    }

    fn owner_snapshot(
        &self,
        object: &Arc<RwLock<Object>>,
    ) -> Result<OwnerSnapshot, CollisionError> {
        OwnerSnapshot::from_arc(object)
    }

    fn target_snapshot(&self, other: &dyn GameObject, owner: &dyn GameObject) -> TargetSnapshot {
        TargetSnapshot::from_game_object(other, owner)
    }

    fn should_allow_squish(&self, owner: &OwnerSnapshot, target: &TargetSnapshot) -> bool {
        let Some(owner_arc) = OBJECT_REGISTRY.get_object(owner.id) else {
            return true;
        };
        let Ok(owner_guard) = owner_arc.read() else {
            return true;
        };

        let goal_matches_target = owner_guard
            .get_ai_update_interface()
            .and_then(|ai| ai.lock().ok().and_then(|guard| guard.get_goal_object()))
            .and_then(|goal| goal.read().ok().map(|guard| guard.get_id()))
            .map(|goal_id| goal_id == target.id)
            .unwrap_or(false);
        if !goal_matches_target {
            return true;
        }

        if owner_guard.find_update_module("HijackerUpdate").is_some() {
            return false;
        }

        let is_tnt_active = owner_guard
            .find_special_ability_update(
                crate::common::types::SpecialPowerType::SpecialTankHunterTntAttack,
            )
            .and_then(|update| update.lock().ok().map(|guard| guard.is_ability_active()))
            .unwrap_or(false);
        if is_tnt_active {
            return false;
        }

        true
    }

    fn can_crush(&self, owner: &OwnerSnapshot, target: &TargetSnapshot) -> bool {
        if target.crusher_level == 0 {
            return false;
        }

        if matches!(target.relationship_with_owner, Relationship::Allies) {
            return false;
        }

        if !target.intersects(owner.position) {
            return false;
        }

        target.is_moving_towards(owner.position)
    }

    fn apply_crush(&self, source_id: ObjectId) -> Result<(), CollisionError> {
        let owner_arc = OBJECT_REGISTRY.get_object(self.owner_id).ok_or_else(|| {
            CollisionError::InvalidObject(format!(
                "SquishCollide owner {} missing from registry",
                self.owner_id
            ))
        })?;

        let mut owner = owner_arc.write().map_err(|_| {
            CollisionError::InvalidObject("failed to lock owner for crush damage".into())
        })?;

        let mut damage_info = DamageInfo {
            input: DamageInfoInput {
                source_id,
                damage_type: DamageType::Crush,
                death_type: DeathType::Crushed,
                amount: HUGE_DAMAGE_AMOUNT,
                ..DamageInfoInput::default()
            },
            output: DamageInfoOutput::default(),
            // Compatibility fields
            amount: HUGE_DAMAGE_AMOUNT,
            damage_type: DamageType::Crush,
            death_type: DeathType::Crushed,
            source_id,
        };

        owner
            .attempt_damage(&mut damage_info)
            .map_err(|err| CollisionError::DamageApplicationFailed(err.to_string()))?;

        owner.friend_set_undetected_defector(false);
        Ok(())
    }
}

impl CollideModuleTrait for SquishCollide {
    fn on_collide(
        &mut self,
        other: Option<&dyn GameObject>,
        _loc: &Coord3D,
        _normal: &Coord3D,
    ) -> Result<(), CollisionError> {
        let Some(other) = other else {
            // Ground collision – nothing to do.
            return Ok(());
        };

        let owner_handle = self.owner_handle()?;
        let owner_snapshot = self.owner_snapshot(&owner_handle)?;
        let target_snapshot = self.target_snapshot(other, &owner_handle);

        if !self.should_allow_squish(&owner_snapshot, &target_snapshot) {
            trace!(
                "SquishCollide: skipping crush because special-case guard vetoed it (owner {}, target {})",
                self.owner_id,
                target_snapshot.id
            );
            return Ok(());
        }

        if !self.can_crush(&owner_snapshot, &target_snapshot) {
            return Ok(());
        }

        self.apply_crush(target_snapshot.id)
    }

    fn would_like_to_collide_with(&self, _other: &dyn GameObject) -> bool {
        // Mirrors the default C++ implementation – no special-case desires.
        false
    }
}

/// Captured state for the owner object needed to evaluate crush logic.
#[derive(Debug, Clone)]
struct OwnerSnapshot {
    id: ObjectId,
    position: Coord3D,
}

impl OwnerSnapshot {
    fn from_arc(object: &Arc<RwLock<Object>>) -> Result<Self, CollisionError> {
        let guard = object
            .read()
            .map_err(|_| CollisionError::InvalidObject("failed to read owner object".into()))?;

        let pos = guard.get_position();
        let position = Coord3D::new(pos.x, pos.y, pos.z);

        Ok(Self {
            id: guard.get_id(),
            position,
        })
    }
}

/// Snapshot of the potential crusher (the "other" object).
#[derive(Debug, Clone)]
struct TargetSnapshot {
    id: ObjectId,
    position: Coord3D,
    relationship_with_owner: Relationship,
    crusher_level: u32,
    velocity_xy: Option<(f32, f32)>,
    geometry_radius: f32,
}

impl TargetSnapshot {
    fn from_game_object(other: &dyn GameObject, owner: &dyn GameObject) -> Self {
        let relationship_with_owner = other.get_relationship(owner);
        let mut snapshot = Self {
            id: other.get_id(),
            position: other.get_position(),
            relationship_with_owner,
            crusher_level: other.get_crusher_level(),
            velocity_xy: None,
            geometry_radius: TARGET_COLLISION_RADIUS,
        };

        if let Some(handle) = OBJECT_REGISTRY.get_object(snapshot.id) {
            if let Ok(object) = handle.read() {
                snapshot.geometry_radius = object.get_geometry_info().get_major_radius();
                if snapshot.geometry_radius < TARGET_COLLISION_RADIUS {
                    snapshot.geometry_radius = TARGET_COLLISION_RADIUS;
                }
                if let Some(physics) = object.get_physics() {
                    snapshot.velocity_xy = physics.lock().ok().map(|guard| {
                        let velocity = guard.get_velocity();
                        (velocity.x, velocity.y)
                    });
                }
            }
        }

        snapshot
    }

    fn intersects(&self, target_position: Coord3D) -> bool {
        let dx = target_position.x - self.position.x;
        let dy = target_position.y - self.position.y;
        let radius_sum = self.geometry_radius + TARGET_COLLISION_RADIUS;
        (dx * dx + dy * dy) <= radius_sum * radius_sum
    }

    fn is_moving_towards(&self, target_position: Coord3D) -> bool {
        let (vx, vy) = match self.velocity_xy {
            Some(pair) => pair,
            None => return false,
        };

        let to_x = target_position.x - self.position.x;
        let to_y = target_position.y - self.position.y;
        (to_x * vx) + (to_y * vy) > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_data_defaults() {
        let _lock = crate::test_sync::lock();

        let data = SquishCollideModuleData::default();
        assert_eq!(data.get_module_tag_name_key(), 0);

        let mut copy = data.clone();
        copy.set_module_tag_name_key(123);
        assert_eq!(copy.get_module_tag_name_key(), 123);
    }

    #[test]
    fn target_snapshot_defaults_to_reasonable_values() {
        let _lock = crate::test_sync::lock();

        struct DummyObject;
        impl GameObject for DummyObject {
            fn get_id(&self) -> ObjectId {
                7
            }
            fn get_position(&self) -> Coord3D {
                Coord3D::new(0.0, 0.0, 0.0)
            }
            fn get_orientation(&self) -> f32 {
                0.0
            }
            fn get_controlling_player(&self) -> PlayerId {
                PlayerId::FIRST
            }
            fn get_veterancy_level(&self) -> super::super::VeterancyLevel {
                super::super::VeterancyLevel::Regular
            }
            fn get_relationship(&self, _other: &dyn GameObject) -> Relationship {
                Relationship::Enemies
            }
            fn get_crusher_level(&self) -> u32 {
                3
            }
            fn is_effectively_dead(&self) -> bool {
                false
            }
            fn is_significantly_above_terrain(&self) -> bool {
                false
            }
            fn is_using_airborne_locomotor(&self) -> bool {
                false
            }
            fn get_status_bits(&self) -> super::super::ObjectStatusMask {
                super::super::ObjectStatusMask::empty()
            }
            fn attempt_damage(&mut self, _damage: &super::super::DamageInfo) -> Result<(), String> {
                Ok(())
            }
            fn set_undetected_defector(&mut self, _value: bool) {}
        }

        let dummy = DummyObject;
        let snapshot = TargetSnapshot::from_game_object(&dummy, &dummy);
        assert_eq!(snapshot.id, 7);
        assert_eq!(snapshot.relationship_with_owner, Relationship::Enemies);
        assert_eq!(snapshot.crusher_level, 3);
        assert!(snapshot.geometry_radius >= TARGET_COLLISION_RADIUS);
    }

    #[test]
    fn allows_squish_when_owner_not_registered() {
        let _lock = crate::test_sync::lock();

        let module = SquishCollide::new(1, Arc::new(SquishCollideModuleData::default()));
        let owner = OwnerSnapshot {
            id: 1,
            position: Coord3D::new(0.0, 0.0, 0.0),
        };
        let target = TargetSnapshot {
            id: 2,
            position: Coord3D::new(1.0, 0.0, 0.0),
            relationship_with_owner: Relationship::Enemies,
            crusher_level: 5,
            velocity_xy: Some((1.0, 0.0)),
            geometry_radius: TARGET_COLLISION_RADIUS,
        };

        assert!(module.should_allow_squish(&owner, &target));
    }
}
