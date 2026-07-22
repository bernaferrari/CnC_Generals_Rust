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
use crate::object::behavior::behavior_module::xfer_behavior_module_base_versions;
use crate::object::collide::collision_geometry::{
    GeometryInfo as CollisionGeometryInfo, GeometryType as CollisionGeometryType,
};
use crate::object::collide::partition_manager::PARTITION_MANAGER;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{ModuleData, NameKeyType};
use game_engine::system::geometry::GeometryType as EngineGeometryType;
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
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
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
        // C++ SquishCollide::crc only delegates to CollideModule::crc.
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("SquishCollide xfer version: {e:?}"))?;
        let mut collide_version: u8 = 1;
        xfer.xfer_version(&mut collide_version, 1)
            .map_err(|e| format!("SquishCollide collide base xfer version: {e:?}"))?;
        xfer_behavior_module_base_versions(xfer)?;
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
        OBJECT_REGISTRY
            .with_object(owner.id, |owner_guard| {
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
            })
            .unwrap_or(true)
    }

    fn can_crush(&self, owner: &OwnerSnapshot, target: &TargetSnapshot) -> bool {
        if target.crusher_level == 0 {
            return false;
        }

        if matches!(target.relationship_with_owner, Relationship::Allies) {
            return false;
        }

        if !target.intersects(owner) {
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
    orientation: f32,
    squish_geometry: CollisionGeometryInfo,
}

impl OwnerSnapshot {
    fn from_arc(object: &Arc<RwLock<Object>>) -> Result<Self, CollisionError> {
        let guard = object
            .read()
            .map_err(|_| CollisionError::InvalidObject("failed to read owner object".into()))?;

        let pos = guard.get_position();
        let position = Coord3D::new(pos.x, pos.y, pos.z);
        let squish_geometry = squish_victim_geometry(guard.get_geometry_info());

        Ok(Self {
            id: guard.get_id(),
            position,
            orientation: guard.get_orientation(),
            squish_geometry,
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
    geometry: CollisionGeometryInfo,
    orientation: f32,
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
            geometry: CollisionGeometryInfo::new_cylinder(TARGET_COLLISION_RADIUS, 1.0, true),
            orientation: other.get_orientation(),
        };

        let _ = OBJECT_REGISTRY.with_object(snapshot.id, |object| {
            snapshot.geometry = collision_geometry_from_logic(object.get_geometry_info());
            snapshot.orientation = object.get_orientation();
            if let Some(physics) = object.get_physics() {
                snapshot.velocity_xy = physics.lock().ok().map(|guard| {
                    let velocity = guard.get_velocity();
                    (velocity.x, velocity.y)
                });
            }
        });

        snapshot
    }

    fn intersects(&self, owner: &OwnerSnapshot) -> bool {
        PARTITION_MANAGER
            .read()
            .map(|partition| {
                partition.geom_collides_with_geom(
                    &self.position,
                    &self.geometry,
                    self.orientation,
                    &owner.position,
                    &owner.squish_geometry,
                    owner.orientation,
                )
            })
            .unwrap_or(false)
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

fn collision_geometry_from_logic(info: &crate::common::GeometryInfo) -> CollisionGeometryInfo {
    let dx = (info.bounds.max.x - info.bounds.min.x).abs().max(0.01);
    let dy = (info.bounds.max.y - info.bounds.min.y).abs().max(0.01);
    let dz = (info.bounds.max.z - info.bounds.min.z).abs().max(0.01);
    let radius = (dx.max(dy) * 0.5).max(0.01);

    match info.geometry_type {
        EngineGeometryType::Sphere => CollisionGeometryInfo::new_sphere(radius, info.is_small),
        EngineGeometryType::Cylinder => {
            CollisionGeometryInfo::new_cylinder(radius, dz, info.is_small)
        }
        EngineGeometryType::Box => CollisionGeometryInfo::new_box(dx, dy, info.is_small),
    }
}

fn squish_victim_geometry(info: &crate::common::GeometryInfo) -> CollisionGeometryInfo {
    let mut geometry = collision_geometry_from_logic(info);
    match geometry.get_geom_type() {
        CollisionGeometryType::Sphere => {
            CollisionGeometryInfo::new_sphere(TARGET_COLLISION_RADIUS, geometry.is_small())
        }
        CollisionGeometryType::Cylinder => CollisionGeometryInfo::new_cylinder(
            TARGET_COLLISION_RADIUS,
            geometry.get_height().max(0.01),
            geometry.is_small(),
        ),
        CollisionGeometryType::Box => {
            geometry.set_major_radius(TARGET_COLLISION_RADIUS);
            geometry.set_minor_radius(TARGET_COLLISION_RADIUS);
            geometry
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::system::{Snapshot, XferBlockSize, XferMode, XferStatus};

    #[derive(Default)]
    struct CountingXfer {
        bytes: usize,
    }

    impl Xfer for CountingXfer {
        fn get_xfer_mode(&self) -> XferMode {
            XferMode::Save
        }

        fn get_identifier(&self) -> &str {
            "squish-collide-test"
        }

        fn set_options(&mut self, _options: u32) {}

        fn clear_options(&mut self, _options: u32) {}

        fn get_options(&self) -> u32 {
            0
        }

        fn open(&mut self, _identifier: &str) -> Result<(), XferStatus> {
            Ok(())
        }

        fn close(&mut self) -> Result<(), XferStatus> {
            Ok(())
        }

        fn begin_block(&mut self) -> Result<XferBlockSize, XferStatus> {
            Ok(0)
        }

        fn end_block(&mut self) -> Result<(), XferStatus> {
            Ok(())
        }

        fn skip(&mut self, data_size: i32) -> Result<(), XferStatus> {
            self.bytes += data_size.max(0) as usize;
            Ok(())
        }

        fn xfer_snapshot(&mut self, _snapshot: &mut Snapshot) -> Result<(), XferStatus> {
            Ok(())
        }

        fn xfer_ascii_string(&mut self, ascii_string_data: &mut String) -> std::io::Result<()> {
            self.bytes += 1 + ascii_string_data.len();
            Ok(())
        }

        fn xfer_unicode_string(&mut self, unicode_string_data: &mut String) -> std::io::Result<()> {
            self.bytes += 1 + unicode_string_data.len();
            Ok(())
        }

        unsafe fn xfer_implementation(
            &mut self,
            _data: *mut u8,
            data_size: usize,
        ) -> std::io::Result<()> {
            self.bytes += data_size;
            Ok(())
        }
    }

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
        assert!(snapshot.geometry.get_major_radius() >= TARGET_COLLISION_RADIUS);
    }

    #[test]
    fn allows_squish_when_owner_not_registered() {
        let _lock = crate::test_sync::lock();

        let module = SquishCollide::new(1, Arc::new(SquishCollideModuleData::default()));
        let owner = OwnerSnapshot {
            id: 1,
            position: Coord3D::new(0.0, 0.0, 0.0),
            orientation: 0.0,
            squish_geometry: CollisionGeometryInfo::new_cylinder(1.0, 2.0, true),
        };
        let target = TargetSnapshot {
            id: 2,
            position: Coord3D::new(1.0, 0.0, 0.0),
            relationship_with_owner: Relationship::Enemies,
            crusher_level: 5,
            velocity_xy: Some((1.0, 0.0)),
            geometry: CollisionGeometryInfo::new_cylinder(1.0, 2.0, false),
            orientation: 0.0,
        };

        assert!(module.should_allow_squish(&owner, &target));
    }

    #[test]
    fn crc_and_xfer_follow_cpp_base_chain() {
        let _lock = crate::test_sync::lock();

        let mut module = SquishCollide::new(1, Arc::new(SquishCollideModuleData::default()));
        let mut crc_xfer = CountingXfer::default();
        module.crc(&mut crc_xfer).expect("crc");
        assert_eq!(crc_xfer.bytes, 0);

        let mut save_xfer = CountingXfer::default();
        module.xfer(&mut save_xfer).expect("xfer");
        assert_eq!(save_xfer.bytes, 5);
    }

    #[test]
    fn intersection_uses_partition_geometry_instead_of_bounding_radius() {
        let _lock = crate::test_sync::lock();

        let owner = OwnerSnapshot {
            id: 1,
            position: Coord3D::new(0.0, 2.1, 0.0),
            orientation: 0.0,
            squish_geometry: CollisionGeometryInfo::new_cylinder(1.0, 2.0, true),
        };
        let near_owner = OwnerSnapshot {
            position: Coord3D::new(0.0, 1.9, 0.0),
            ..owner.clone()
        };
        let target = TargetSnapshot {
            id: 2,
            position: Coord3D::new(0.0, 0.0, 0.0),
            relationship_with_owner: Relationship::Enemies,
            crusher_level: 5,
            velocity_xy: Some((0.0, 1.0)),
            geometry: CollisionGeometryInfo::new_box(10.0, 2.0, false),
            orientation: 0.0,
        };

        assert!(!target.intersects(&owner));
        assert!(target.intersects(&near_owner));
    }
}
