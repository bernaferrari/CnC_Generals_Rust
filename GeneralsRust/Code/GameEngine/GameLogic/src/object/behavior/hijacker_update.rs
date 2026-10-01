//! HijackerUpdate - Rust conversion of C++ HijackerUpdate
//!
//! Allows hijacker to stay with hijacked vehicle until it dies.
//! Author: Mark Lorenzen, July 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::{
    Bool, CommandSourceType, Coord3D, ModuleData, ObjectID, ObjectStatusMaskType, UnsignedInt,
};
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::{BehaviorModuleData, xfer_update_module_base_state};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::INVALID_ID as OBJECT_INVALID_ID;
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::HijackerControlInterface;
use std::sync::Arc;

/// Wave 288: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

#[derive(Clone, Debug)]
pub struct HijackerUpdateModuleData {
    pub base: BehaviorModuleData,
    pub attach_to_bone: String,
    pub parachute_name: String,
}

impl Default for HijackerUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            attach_to_bone: String::new(),
            parachute_name: String::new(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(HijackerUpdateModuleData, base);

impl HijackerUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, HIJACKER_UPDATE_FIELDS)
    }
}

fn parse_attach_to_target_bone(
    _ini: &mut INI,
    data: &mut HijackerUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)?;
    data.attach_to_bone = token.to_string();
    Ok(())
}

fn parse_parachute_name(
    _ini: &mut INI,
    data: &mut HijackerUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)?;
    data.parachute_name = token.to_string();
    Ok(())
}

const HIJACKER_UPDATE_FIELDS: &[FieldParse<HijackerUpdateModuleData>] = &[
    FieldParse {
        token: "AttachToTargetBone",
        parse: parse_attach_to_target_bone,
    },
    FieldParse {
        token: "ParachuteName",
        parse: parse_parachute_name,
    },
];

pub struct HijackerUpdate {
    object_id: ObjectID,
    #[allow(dead_code)]
    module_data: Arc<HijackerUpdateModuleData>,
    /// UpdateModule scheduler state serialized by the C++ base class.
    next_call_frame_and_phase: UnsignedInt,
    target_id: ObjectID,
    eject_pos: Coord3D,
    update: Bool,
    is_in_vehicle: Bool,
    was_target_airborne: Bool,
}

impl HijackerUpdate {
    pub fn new(
        object_id: ObjectID,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<HijackerUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object_id,
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            target_id: OBJECT_INVALID_ID,
            eject_pos: Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            update: false,
            is_in_vehicle: false,
            was_target_airborne: false,
        })
    }

    pub fn set_target_object(&mut self, target_id: ObjectID) {
        self.target_id = target_id;
    }

    pub fn get_target_object(&self) -> ObjectID {
        self.target_id
    }

    pub fn set_update(&mut self, update: Bool) {
        self.update = update;
    }

    pub fn set_is_in_vehicle(&mut self, is_in_vehicle: Bool) {
        self.is_in_vehicle = is_in_vehicle;
    }

    /// Vehicle is gone. Put the hijacker back in the world at the last eject pos.
    fn restore_hijacker_after_vehicle_death(&mut self) {
        let hijacker_id = self.object_id;
        let eject_pos = self.eject_pos;
        if hijacker_id == OBJECT_INVALID_ID {
            return;
        }

        // Read the container id first. `with_object_mut` checks that object out,
        // so the container release must not run while the hijacker is borrowed.
        let container_id = OBJECT_REGISTRY
            .with_object(hijacker_id, |hijacker| hijacker.get_container_id())
            .flatten();
        if let Some(container_id) = container_id {
            let _ = OBJECT_REGISTRY.with_object_mut(container_id, |container| {
                if let Some(contain) = container.get_contain_mut() {
                    let _ = contain.release_object(hijacker_id);
                }
            });
        }

        let _ = OBJECT_REGISTRY.with_object_mut(hijacker_id, |hijacker| {
            let _ = hijacker.set_position(&eject_pos);
            if let Some(drawable) = hijacker.get_drawable() {
                if let Ok(mut drawable_guard) = drawable.write() {
                    let _ = drawable_guard.set_drawable_hidden(false);
                }
            }
            hijacker.set_status(
                ObjectStatusMaskType::NO_COLLISIONS
                    | ObjectStatusMaskType::MASKED
                    | ObjectStatusMaskType::UNSELECTABLE,
                false,
            );
            hijacker.handle_partition_cell_maintenance();
            if let Some(ai) = hijacker.get_ai_mut() {
                // C++ AIUpdateInterface::aiIdle(FromAI) issued after eject.
                let params = crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::Idle,
                    CommandSourceType::FromAi,
                );
                let _ = ai.execute_command(&params);
            }
        });
    }
}

impl UpdateModuleInterface for HijackerUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        // Wave 288: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return UpdateSleepTime::None;
        }

        if !self.update {
            return UpdateSleepTime::None;
        }

        if self.is_in_vehicle {
            let target_id = self.target_id;
            // Snapshot the vehicle, then drop the checkout before touching the
            // hijacker. Same-id re-entry returns None for the duration of a callback.
            let target_snapshot = OBJECT_REGISTRY.with_object(target_id, |target| {
                (
                    *target.get_position(),
                    target.get_experience_tracker().is_some(),
                    target.get_veterancy_level(),
                    target.is_significantly_above_terrain(),
                )
            });

            if let Some((target_pos, target_has_tracker, target_level, airborne)) = target_snapshot
            {
                self.was_target_airborne = airborne;
                self.eject_pos = target_pos;

                let hijacker_id = self.object_id;
                if hijacker_id != OBJECT_INVALID_ID {
                    Self::track_hijacked_vehicle(
                        hijacker_id,
                        target_id,
                        &target_pos,
                        target_has_tracker,
                        target_level,
                    );
                }
            } else {
                self.restore_hijacker_after_vehicle_death();
                self.target_id = OBJECT_INVALID_ID;
                self.is_in_vehicle = false;
                self.update = false;
                self.was_target_airborne = false;
                return UpdateSleepTime::None;
            }

            return UpdateSleepTime::None;
        }

        self.was_target_airborne = false;
        UpdateSleepTime::None
    }
}

impl HijackerUpdate {
    fn track_hijacked_vehicle(
        hijacker_id: ObjectID,
        target_id: ObjectID,
        target_pos: &Coord3D,
        target_has_tracker: bool,
        target_level: crate::common::VeterancyLevel,
    ) {
        if hijacker_id == target_id {
            let _ = OBJECT_REGISTRY.with_object_mut(hijacker_id, |obj| {
                let _ = obj.set_position(target_pos);
                if target_has_tracker && obj.get_experience_tracker().is_some() {
                    let highest_level = target_level.max(obj.get_veterancy_level());
                    obj.set_veterancy_level_with_side_effects(highest_level, true);
                }
            });
            return;
        }

        let Some((hijacker_has_tracker, hijacker_level)) =
            OBJECT_REGISTRY.with_object_mut(hijacker_id, |hijacker| {
                let hijacker_has_tracker = hijacker.get_experience_tracker().is_some();
                let hijacker_level = hijacker.get_veterancy_level();
                let _ = hijacker.set_position(target_pos);
                (hijacker_has_tracker, hijacker_level)
            })
        else {
            return;
        };

        if target_has_tracker && hijacker_has_tracker {
            let highest_level = target_level.max(hijacker_level);
            // C++ HijackerUpdate.cpp:74-77 sets BOTH trackers to the
            // highest level via `setVeterancyLevel(highestLevel)`
            // (ExperienceTracker.h:30 default `provideFeedback = TRUE`),
            // and the C++ tracker fires Object::onVeterancyLevelChanged
            // itself (ExperienceTracker.cpp:82-95) — weapon-set swap,
            // body notify, promotion anim + sound included. Same-level
            // sets are a no-op there (`m_currentLevel != newLevel`).
            let _ = OBJECT_REGISTRY.with_object_mut(hijacker_id, |hijacker| {
                hijacker.set_veterancy_level_with_side_effects(highest_level, true);
            });
            let _ = OBJECT_REGISTRY.with_object_mut(target_id, |target| {
                target.set_veterancy_level_with_side_effects(highest_level, true);
            });
        }
    }
}

impl BehaviorModuleInterface for HijackerUpdate {
    fn get_module_name(&self) -> &'static str {
        "HijackerUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_hijacker_control_interface(&mut self) -> Option<&mut dyn HijackerControlInterface> {
        Some(self)
    }
}

impl HijackerControlInterface for HijackerUpdate {
    fn configure_hijacked_vehicle(&mut self, target_id: ObjectID) {
        self.set_target_object(target_id);
        self.set_update(true);
        self.set_is_in_vehicle(true);
    }
}

impl Snapshotable for HijackerUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("HijackerUpdate xfer version: {:?}", e))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        xfer.xfer_object_id(&mut self.target_id)
            .map_err(|e| format!("HijackerUpdate xfer target_id: {:?}", e))?;
        xfer.xfer_coord3d(&mut self.eject_pos);
        xfer.xfer_bool(&mut self.update)
            .map_err(|e| format!("HijackerUpdate xfer update: {:?}", e))?;
        xfer.xfer_bool(&mut self.is_in_vehicle)
            .map_err(|e| format!("HijackerUpdate xfer is_in_vehicle: {:?}", e))?;
        xfer.xfer_bool(&mut self.was_target_airborne)
            .map_err(|e| format!("HijackerUpdate xfer was_target_airborne: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct HijackerUpdateFactory;
impl HijackerUpdateFactory {
    pub fn create_behavior(
        object_id: ObjectID,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(HijackerUpdate::new(object_id, module_data)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{ObjectStatusTypes, VeterancyLevel};
    use crate::object::registry::test_isolation_lock;
    use crate::object::Object as GameObject;
    use crate::weapon::WeaponSetType;

    fn module_data() -> Arc<dyn ModuleData> {
        Arc::new(HijackerUpdateModuleData::default())
    }

    fn register_tracked(id: ObjectID, level: VeterancyLevel) {
        let mut object = GameObject::new_test(id, 100.0);
        object.attach_experience_tracker_for_test(true);
        if let Some(tracker) = object.get_experience_tracker() {
            if let Ok(mut guard) = tracker.lock() {
                guard.set_veterancy_level(level);
            }
        }
        OBJECT_REGISTRY.register_object(id, object);
    }

    fn veterancy_of(id: ObjectID) -> VeterancyLevel {
        OBJECT_REGISTRY
            .with_object(id, |object| object.get_veterancy_level())
            .expect("tracked object")
    }

    #[test]
    fn inactive_hijacker_update_runs_again_next_frame_like_cpp() {
        let mut update = HijackerUpdate::new(9301, module_data()).unwrap();

        assert!(matches!(update.update_simple(), UpdateSleepTime::None));
    }

    #[test]
    fn hijacker_in_vehicle_tracks_target_position_each_frame() {
        let _lock = test_isolation_lock().lock().unwrap();
        let hijacker = GameObject::new_test(9302, 100.0);
        let mut target = GameObject::new_test(9303, 100.0);
        let target_pos = Coord3D {
            x: 35.0,
            y: -12.0,
            z: 4.0,
        };
        target.set_position(&target_pos).unwrap();
        OBJECT_REGISTRY.register_object(9302, hijacker);
        OBJECT_REGISTRY.register_object(9303, target);

        let mut update = HijackerUpdate::new(9302, module_data()).unwrap();
        update.configure_hijacked_vehicle(9303);

        assert!(matches!(update.update_simple(), UpdateSleepTime::None));
        let pos = OBJECT_REGISTRY
            .with_object(9302, |hijacker| *hijacker.get_position())
            .expect("hijacker registered");
        assert_eq!(pos, target_pos);
        assert_eq!(update.eject_pos, target_pos);
        assert_eq!(update.target_id, 9303);
        assert!(update.update);
        assert!(update.is_in_vehicle);

        OBJECT_REGISTRY.unregister_object(9302);
        OBJECT_REGISTRY.unregister_object(9303);
    }

    #[test]
    fn hijacker_in_vehicle_keeps_highest_veterancy_with_target() {
        let _lock = test_isolation_lock().lock().unwrap();
        register_tracked(9304, VeterancyLevel::Veteran);
        register_tracked(9305, VeterancyLevel::Elite);

        let mut update = HijackerUpdate::new(9304, module_data()).unwrap();
        update.configure_hijacked_vehicle(9305);

        assert!(matches!(update.update_simple(), UpdateSleepTime::None));
        assert_eq!(veterancy_of(9304), VeterancyLevel::Elite);
        assert_eq!(veterancy_of(9305), VeterancyLevel::Elite);

        OBJECT_REGISTRY.unregister_object(9304);
        OBJECT_REGISTRY.unregister_object(9305);
    }

    #[test]
    fn hijacker_merge_fires_cpp_on_veterancy_level_changed_side_effects() {
        // C++ HijackerUpdate.cpp:74-77 setVeterancyLevel(highestLevel) fires
        // Object::onVeterancyLevelChanged only when the level actually changes
        // (ExperienceTracker.cpp:87-93). The hijacker promotes Regular → Elite
        // and must pick up the Elite weapon set. The vehicle is already Elite,
        // so the same-level set does not re-fire side effects.
        let _lock = test_isolation_lock().lock().unwrap();
        register_tracked(9309, VeterancyLevel::Regular);
        register_tracked(9310, VeterancyLevel::Elite);

        let mut update = HijackerUpdate::new(9309, module_data()).unwrap();
        update.configure_hijacked_vehicle(9310);
        assert!(matches!(update.update_simple(), UpdateSleepTime::None));

        let hijacker_elite = OBJECT_REGISTRY
            .with_object(9309, |hijacker| {
                hijacker.test_weapon_set_flag(WeaponSetType::Elite)
            })
            .expect("hijacker registered");
        assert!(
            hijacker_elite,
            "hijacker weapon set must follow the merged Elite level"
        );
        let target_elite = OBJECT_REGISTRY
            .with_object(9310, |target| target.test_weapon_set_flag(WeaponSetType::Elite))
            .expect("vehicle registered");
        assert!(
            !target_elite,
            "already-Elite vehicle must not re-fire onVeterancyLevelChanged"
        );

        OBJECT_REGISTRY.unregister_object(9309);
        OBJECT_REGISTRY.unregister_object(9310);
    }

    #[test]
    fn missing_target_restores_hijacker_object_state() {
        let _lock = test_isolation_lock().lock().unwrap();
        let mut hijacker = GameObject::new_test(9306, 100.0);
        let eject_pos = Coord3D {
            x: 8.0,
            y: 9.0,
            z: 10.0,
        };
        hijacker
            .set_position(&Coord3D {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            })
            .unwrap();
        hijacker.set_status(ObjectStatusMaskType::NO_COLLISIONS, true);
        hijacker.set_status(ObjectStatusMaskType::MASKED, true);
        hijacker.set_status(ObjectStatusMaskType::UNSELECTABLE, true);
        OBJECT_REGISTRY.register_object(9306, hijacker);

        let mut update = HijackerUpdate::new(9306, module_data()).unwrap();
        update.configure_hijacked_vehicle(99_999);
        update.eject_pos = eject_pos;
        update.was_target_airborne = true;

        assert!(matches!(update.update_simple(), UpdateSleepTime::None));
        assert_eq!(update.target_id, OBJECT_INVALID_ID);
        assert!(!update.update);
        assert!(!update.is_in_vehicle);
        assert!(!update.was_target_airborne);

        let (pos, no_collisions, masked, unselectable) = OBJECT_REGISTRY
            .with_object(9306, |hijacker| {
                (
                    *hijacker.get_position(),
                    hijacker.test_status(ObjectStatusTypes::NoCollisions),
                    hijacker.test_status(ObjectStatusTypes::Masked),
                    hijacker.test_status(ObjectStatusTypes::Unselectable),
                )
            })
            .expect("hijacker registered");
        assert_eq!(pos, eject_pos);
        assert!(!no_collisions);
        assert!(!masked);
        assert!(!unselectable);

        OBJECT_REGISTRY.unregister_object(9306);
    }
}
