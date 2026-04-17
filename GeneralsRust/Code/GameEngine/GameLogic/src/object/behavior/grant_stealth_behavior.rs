//! GrantStealthBehavior - Rust conversion of C++ GrantStealthBehavior
//!
//! Update that grants permanent stealth to units within a radius.
//! Author: Mark Lorenzen, June 2003 (C++ version)
//! Rust conversion: 2025

use crate::common::{
    AsciiString, Coord3D, KindOfMaskType, ModuleData, NameKeyType, ObjectID, ParticleSystemID,
    ParticleSystemTemplate, Real, Relationship,
};
use crate::helpers::{TheGameLogic, TheParticleSystemManager};
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::auto_heal_behavior::parse_kind_of_mask;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use log::warn;
use std::any::Any;
use std::sync::{Arc, RwLock, Weak};

const INVALID_PARTICLE_SYSTEM_ID: ParticleSystemID = 0;
const KIND_OF_MASK_ALL: KindOfMaskType = !0;
const UPDATE_SLEEP_NONE: UpdateSleepTime = UpdateSleepTime::None;
const UPDATE_SLEEP_FOREVER: UpdateSleepTime = UpdateSleepTime::Forever;

/// Module data for GrantStealthBehavior
#[derive(Clone, Debug)]
pub struct GrantStealthBehaviorModuleData {
    module_tag_name_key: NameKeyType,
    pub start_radius: Real,
    pub final_radius: Real,
    pub radius_grow_rate: Real,
    pub kind_of: KindOfMaskType,
    pub radius_particle_system_tmpl: Option<Arc<ParticleSystemTemplate>>,
}

impl Default for GrantStealthBehaviorModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            start_radius: 0.0,
            final_radius: 200.0,
            radius_grow_rate: 10.0,
            kind_of: KIND_OF_MASK_ALL,
            radius_particle_system_tmpl: None,
        }
    }
}

impl crate::common::LegacyModuleData for GrantStealthBehaviorModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl Snapshotable for GrantStealthBehaviorModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn first_value_token<'a>(tokens: &'a [&'a str]) -> Option<&'a str> {
    tokens.iter().copied().find(|token| !token.is_empty())
}

fn parse_real_field(
    target: &mut Real,
    tokens: &[&str],
) -> Result<(), game_engine::common::ini::INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    *target = value.parse::<Real>().map_err(|_| INIError::InvalidData)?;
    Ok(())
}

fn parse_start_radius_field(
    _ini: &mut INI,
    data: &mut GrantStealthBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_real_field(&mut data.start_radius, tokens)
}

fn parse_final_radius_field(
    _ini: &mut INI,
    data: &mut GrantStealthBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_real_field(&mut data.final_radius, tokens)
}

fn parse_radius_grow_rate_field(
    _ini: &mut INI,
    data: &mut GrantStealthBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_real_field(&mut data.radius_grow_rate, tokens)
}

fn parse_kind_of_field(
    _ini: &mut INI,
    data: &mut GrantStealthBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.kind_of = parse_kind_of_mask(tokens);
    Ok(())
}

fn parse_radius_particle_system_name_field(
    _ini: &mut INI,
    data: &mut GrantStealthBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    if value.eq_ignore_ascii_case("none") {
        data.radius_particle_system_tmpl = None;
    } else {
        data.radius_particle_system_tmpl = Some(Arc::new(ParticleSystemTemplate::new(
            AsciiString::from(value),
        )));
    }
    Ok(())
}

const GRANT_STEALTH_BEHAVIOR_FIELDS: &[FieldParse<GrantStealthBehaviorModuleData>] = &[
    FieldParse {
        token: "StartRadius",
        parse: parse_start_radius_field,
    },
    FieldParse {
        token: "FinalRadius",
        parse: parse_final_radius_field,
    },
    FieldParse {
        token: "RadiusGrowRate",
        parse: parse_radius_grow_rate_field,
    },
    FieldParse {
        token: "KindOf",
        parse: parse_kind_of_field,
    },
    FieldParse {
        token: "RadiusParticleSystemName",
        parse: parse_radius_particle_system_name_field,
    },
];

impl GrantStealthBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields_allow_unknown(self, GRANT_STEALTH_BEHAVIOR_FIELDS)
    }
}

/// GrantStealthBehavior module
pub struct GrantStealthBehavior {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<GrantStealthBehaviorModuleData>,
    radius_particle_system_id: ParticleSystemID,
    current_scan_radius: Real,
}

impl GrantStealthBehavior {
    /// Create a new GrantStealthBehavior instance
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = {
            let data_ref = module_data
                .as_ref()
        .downcast_ref::<GrantStealthBehaviorModuleData>()
                .ok_or("Invalid module data type for GrantStealthBehavior")?;
            data_ref.clone()
        };
        let mut behavior = Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            radius_particle_system_id: INVALID_PARTICLE_SYSTEM_ID,
            current_scan_radius: specific_data.start_radius,
        };

        if let Some(radius_tmpl) = &behavior.module_data.radius_particle_system_tmpl {
            if let Some(manager) = TheParticleSystemManager::get() {
                if let Some(system_id) =
                    manager.create_particle_system(Some(radius_tmpl.name.as_str()))
                {
                    if let Ok(obj_guard) = object.read() {
                        manager.set_particle_system_position(system_id, obj_guard.get_position());
                    }
                    behavior.radius_particle_system_id = system_id;
                }
            }
        }

        Ok(behavior)
    }

    /// Grant stealth to an object
    /// Matches C++ GrantStealthBehavior::grantStealthToObject lines 159-182
    fn grant_stealth_to_object(&mut self, target_id: ObjectID) {
        // Get self object for filtering (C++ line 162-163)
        let Some(self_obj) = self.object.upgrade() else {
            return;
        };
        let Ok(self_guard) = self_obj.read() else {
            return;
        };
        let self_id = self_guard.get_id();
        drop(self_guard);

        // Don't grant to self (C++ line 162-163)
        if target_id == self_id {
            return;
        }

        // Get target object
        let Some(target_obj) = OBJECT_REGISTRY.get_object(target_id) else {
            return;
        };
        let Ok(target_guard) = target_obj.read() else {
            return;
        };

        // Check if target matches KindOf requirements (C++ line 167-168)
        if !self.matches_kind_of(&*target_guard) {
            return;
        }

        // Find StealthUpdate module on target (C++ line 170)
        let stealth_handle = match target_guard.get_stealth() {
            Some(handle) => handle,
            None => return,
        };
        let target_drawable = target_guard.get_drawable();

        drop(target_guard);

        // Call receive_grant() on the stealth module (C++ line 173)
        // C++ calls stealth->receiveGrant() with no parameters (defaults to active=TRUE, frames=0)
        // StealthUpdateHandle is Arc<Mutex<StealthController>>
        let mut stealth_guard = match stealth_handle.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        // Get current frame from game logic
        let current_frame = TheGameLogic::get_frame();
        if let Err(e) = stealth_guard.receive_grant(true, 0, current_frame) {
            warn!("Failed to grant stealth: {}", e);
        }

        // C++ lines 174-178: Flash as selected (visual feedback)
        if let Some(drawable) = target_drawable {
            if let Ok(mut draw_guard) = drawable.write() {
                draw_guard.flash_as_selected();
            }
        }
    }

    /// Check if object matches KindOf mask
    /// Helper for C++ line 167: obj->isAnyKindOf(d->m_kindOf)
    fn matches_kind_of(&self, obj: &GameObject) -> bool {
        // Matches C++ `obj->isAnyKindOf(m_kindOf)` semantics.
        if self.module_data.kind_of == KIND_OF_MASK_ALL {
            return true;
        }

        (obj.get_kind_of() & self.module_data.kind_of) != 0
    }

    /// Scan for objects to grant stealth to
    /// Matches C++ GrantStealthBehavior::update lines 124-145
    fn scan_for_objects(&mut self) {
        // Get self object
        let Some(self_obj) = self.object.upgrade() else {
            return;
        };
        let Ok(self_guard) = self_obj.read() else {
            return;
        };

        // Get object position (C++ line 141: self->getPosition())
        let position: Coord3D = *self_guard.get_position();
        drop(self_guard);

        // C++ lines 124-128: Setup scan filters
        // PartitionFilterRelationship relationship( self, PartitionFilterRelationship::ALLOW_ALLIES )
        // PartitionFilterSameMapStatus filterMapStatus( self )
        // PartitionFilterAlive filterAlive

        // C++ lines 141-142: Query nearby objects within current_scan_radius
        // ObjectIterator *iter = ThePartitionManager->iterateObjectsInRange(
        //     self->getPosition(), m_currentScanRadius, FROM_CENTER_2D, filters )

        // NOTE: Since we don't have PartitionManager fully integrated yet,
        // we'll use OBJECT_REGISTRY.get_all_objects() and filter manually
        // This is less efficient but functionally equivalent for now
        let all_objects = OBJECT_REGISTRY.get_all_objects();

        // C++ lines 143-145: For each object, grant stealth
        for obj_arc in all_objects {
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };

            let obj_id = obj_guard.get_id();
            let obj_pos = obj_guard.get_position();

            // Check distance (C++ uses FROM_CENTER_2D - 2D distance only)
            let dx = obj_pos.x - position.x;
            let dy = obj_pos.y - position.y;
            let dist_sqr = dx * dx + dy * dy;
            let radius_sqr = self.current_scan_radius * self.current_scan_radius;

            if dist_sqr > radius_sqr {
                continue;
            }

            // C++ line 125: PartitionFilterRelationship - ALLOW_ALLIES
            if !Self::is_allied_or_self(&self_obj, &obj_guard) {
                continue;
            }

            // C++ line 126: PartitionFilterSameMapStatus - check not off-map
            if obj_guard.is_off_map() {
                continue;
            }

            // C++ line 127: PartitionFilterAlive - check alive
            if obj_guard.is_effectively_dead() {
                continue;
            }
            let rider_id = obj_guard.get_contain().and_then(|contain| {
                contain
                    .lock()
                    .ok()
                    .and_then(|guard| guard.friend_get_rider())
            });

            drop(obj_guard);

            // Grant stealth to this object (C++ line 145)
            // In C++: grantStealthToObject( obj )
            self.grant_stealth_to_object(obj_id);
            if let Some(rider_id) = rider_id {
                self.grant_stealth_to_object(rider_id);
            }
        }
    }

    fn is_allied_or_self(self_obj: &Arc<RwLock<GameObject>>, other: &GameObject) -> bool {
        let Ok(self_guard) = self_obj.read() else {
            return false;
        };
        matches!(
            self_guard.relationship_to(other),
            Relationship::Allies
        )
    }
}

impl UpdateModuleInterface for GrantStealthBehavior {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let Some(object) = self.object.upgrade() else {
            return UPDATE_SLEEP_FOREVER;
        };
        let (object_id, is_dead) = match object.read() {
            Ok(obj_guard) => (obj_guard.get_id(), obj_guard.is_effectively_dead()),
            Err(_) => return UPDATE_SLEEP_FOREVER,
        };
        if is_dead {
            return UPDATE_SLEEP_FOREVER;
        }

        self.current_scan_radius += self.module_data.radius_grow_rate;
        let mut this_is_final_scan = false;
        if self.current_scan_radius >= self.module_data.final_radius {
            self.current_scan_radius = self.module_data.final_radius;
            this_is_final_scan = true;
        }

        self.scan_for_objects();

        if this_is_final_scan {
            if let Err(err) = TheGameLogic::destroy_object_by_id(object_id) {
                warn!(
                    "GrantStealthBehavior failed to destroy grantor object {}: {}",
                    object_id, err
                );
            }
            return UPDATE_SLEEP_FOREVER;
        }

        UPDATE_SLEEP_NONE
    }
}

impl Drop for GrantStealthBehavior {
    fn drop(&mut self) {
        if self.radius_particle_system_id != INVALID_PARTICLE_SYSTEM_ID {
            if let Some(manager) = TheParticleSystemManager::get() {
                manager.destroy_particle_system(self.radius_particle_system_id);
            }
            self.radius_particle_system_id = INVALID_PARTICLE_SYSTEM_ID;
        }
    }
}

impl BehaviorModuleInterface for GrantStealthBehavior {
    fn get_module_name(&self) -> &'static str {
        "GrantStealthBehavior"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

// Factory for creating GrantStealthBehavior instances
pub struct GrantStealthBehaviorFactory;

impl GrantStealthBehaviorFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        let behavior = GrantStealthBehavior::new(thing, module_data)?;
        Ok(Box::new(behavior))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grant_stealth_creation() {
        let data = GrantStealthBehaviorModuleData::default();
        assert_eq!(data.start_radius, 0.0);
        assert_eq!(data.final_radius, 200.0);
        assert_eq!(data.radius_grow_rate, 10.0);
        assert_eq!(data.kind_of, KIND_OF_MASK_ALL);
        assert!(data.radius_particle_system_tmpl.is_none());
    }

    #[test]
    fn test_grant_stealth_parse_from_ini() {
        let mut data = GrantStealthBehaviorModuleData::default();
        let mut ini = INI::new();
        let parsed = ini.with_inline_source(
            "StartRadius = 12.5\n\
             FinalRadius = 240.0\n\
             RadiusGrowRate = 4.0\n\
             KindOf = INFANTRY VEHICLE\n\
             RadiusParticleSystemName = StealthPulseFX\n\
             End\n",
            |ini| data.parse_from_ini(ini),
        );
        assert!(parsed.is_ok());
        assert!((data.start_radius - 12.5).abs() < f32::EPSILON);
        assert!((data.final_radius - 240.0).abs() < f32::EPSILON);
        assert!((data.radius_grow_rate - 4.0).abs() < f32::EPSILON);
        assert_ne!(data.kind_of, KIND_OF_MASK_ALL);
        let radius_fx = data
            .radius_particle_system_tmpl
            .as_ref()
            .expect("radius particle system template expected");
        assert_eq!(radius_fx.name.as_str(), "StealthPulseFX");
    }
}
