//! Create Crate Die Module
//!
//! FILE: create_crate_die.rs
//! Author: Converted from Graham Smallwood's C++ implementation, February 2002
//! Desc: A chance to create a crate on death according to certain condition checks
//!
//! Matches C++ CreateCrateDie.cpp and CreateCrateDie.h

use std::f32::consts::PI;
use std::sync::{Arc, RwLock};
use serde::{Deserialize, Serialize};

use crate::common::*;
use crate::damage::DamageInfo;
use crate::experience::VeterancyLevel;
use crate::helpers::TheThingFactory;
use crate::object::{Object, ObjectId};
use crate::object::registry::OBJECT_REGISTRY;
use crate::common::science::SCIENCE_INVALID;
use crate::object::crate_system::{CrateTemplate, get_crate_system};

/// Module data for CreateCrateDie behavior
/// Matches C++ CreateCrateDieModuleData
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCrateDieModuleData {
    /// Base die module data (would extend DieModuleData in full implementation)
    pub base_die_data: BaseDieModuleData,

    /// List of crate template names that can be created on death
    /// Matches C++ m_crateNameList
    pub crate_name_list: Vec<String>,
}

impl Default for CreateCrateDieModuleData {
    fn default() -> Self {
        Self {
            base_die_data: BaseDieModuleData::default(),
            crate_name_list: Vec::new(),
        }
    }
}

impl CreateCrateDieModuleData {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a crate template name to the list
    pub fn add_crate_name(mut self, name: String) -> Self {
        self.crate_name_list.push(name);
        self
    }

    /// Build field parse configuration for INI parsing
    /// Matches C++ buildFieldParse
    pub fn build_field_parse() -> Vec<FieldParse> {
        vec![
            FieldParse::new("CrateData", FieldType::String, "crate_name_list"),
        ]
    }
}

/// Base die module data carrier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseDieModuleData {
    // Would contain base die module fields
}

impl Default for BaseDieModuleData {
    fn default() -> Self {
        Self {}
    }
}

use crate::helpers::{FindPositionOptions, FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS, FPF_NONE};


/// Create Crate Die Module
/// Matches C++ CreateCrateDie class
pub struct CreateCrateDie {
    /// Module configuration
    module_data: CreateCrateDieModuleData,

    /// The object this module belongs to
    object_id: ObjectId,

    /// Version for serialization
    version: u32,
}

impl CreateCrateDie {
    /// Create a new CreateCrateDie module
    /// Matches C++ CreateCrateDie::CreateCrateDie
    pub fn new(object_id: ObjectId, module_data: CreateCrateDieModuleData) -> Self {
        Self {
            module_data,
            object_id,
            version: 1,
        }
    }

    /// Handle object death and potentially create a crate
    /// Matches C++ CreateCrateDie::onDie
    pub fn on_die(
        &self,
        damage_info: &DamageInfo,
        object: &Arc<RwLock<Object>>,
        killer: Option<&Arc<RwLock<Object>>>,
    ) -> Result<Option<ObjectId>, String> {
        // Check if die is applicable (would call isDieApplicable in C++)
        if !self.is_die_applicable(damage_info) {
            return Ok(None);
        }

        // Get relationship to killer
        if let Some(killer_obj) = killer {
            let obj_lock = object.read().map_err(|_| "Failed to lock object")?;
            let killer_lock = killer_obj.read().map_err(|_| "Failed to lock killer")?;

            // No crate for killing an ally
            if matches!(
                obj_lock.relationship_to(&*killer_lock),
                Relationship::Allies | Relationship::Allies | Relationship::Allies
            ) {
                return Ok(None);
            }
        }

        // Access the crate system
        let crate_system = get_crate_system();
        let system_lock = crate_system.read().map_err(|_| "Failed to lock crate system")?;

        // Try each crate template in the list
        for crate_name in &self.module_data.crate_name_list {
            if let Some(template_arc) = system_lock.find_crate_template(crate_name) {
                let template = template_arc.read().map_err(|_| "Failed to lock crate template")?;

                // Test creation chance
                if !self.test_creation_chance(&template) {
                    continue;
                }

                // Test veterancy level if specified
                if template.veterancy_level != VeterancyLevel::Regular {
                    if !self.test_veterancy_level(&template, object)? {
                        continue;
                    }
                }

                // Test killer type if specified
                if template.killed_by_type_kindof != 0 {
                    if !self.test_killer_type(&template, killer)? {
                        continue;
                    }
                }

                // Test killer science if specified
                if template.killer_science != SCIENCE_INVALID {
                    if !self.test_killer_science(&template, killer)? {
                        continue;
                    }
                }

                // All tests passed - create the crate
                let crate_id = self.create_crate(&template, object, killer)?;
                if let Some(id) = crate_id {
                    return Ok(Some(id));
                }
            }
        }

        Ok(None)
    }

    /// Test if the die event is applicable
    /// Matches C++ CreateCrateDie::isDieApplicable
    fn is_die_applicable(&self, _damage_info: &DamageInfo) -> bool {
        // Would check various conditions like death type, etc.
        // For now, accept all deaths
        true
    }

    /// Test creation chance
    /// Matches C++ CreateCrateDie::testCreationChance
    fn test_creation_chance(&self, template: &CrateTemplate) -> bool {
        let test_with = GameLogicRandomValueReal(0.0, 1.0);
        test_with < template.creation_chance
    }

    /// Test veterancy level requirement
    /// Matches C++ CreateCrateDie::testVeterancyLevel
    fn test_veterancy_level(
        &self,
        template: &CrateTemplate,
        object: &Arc<RwLock<Object>>,
    ) -> Result<bool, String> {
        let obj_lock = object.read().map_err(|_| "Failed to lock object")?;
        let object_level = obj_lock.get_veterancy_level();
        Ok(template.veterancy_level == object_level)
    }

    /// Test killer type requirement
    /// Matches C++ CreateCrateDie::testKillerType
    fn test_killer_type(
        &self,
        template: &CrateTemplate,
        killer: Option<&Arc<RwLock<Object>>>,
    ) -> Result<bool, String> {
        let killer_obj = match killer {
            Some(k) => k,
            None => return Ok(false),
        };

        let killer_lock = killer_obj.read().map_err(|_| "Failed to lock killer")?;

        // Must match the whole group of bits set in the KilledBy description
        if !killer_lock.is_kind_of_multi(template.killed_by_type_kindof, 0) {
            return Ok(false);
        }

        Ok(true)
    }

    /// Test killer science requirement
    /// Matches C++ CreateCrateDie::testKillerScience
    fn test_killer_science(
        &self,
        template: &CrateTemplate,
        killer: Option<&Arc<RwLock<Object>>>,
    ) -> Result<bool, String> {
        let killer_obj = match killer {
            Some(k) => k,
            None => return Ok(false),
        };

        let killer_lock = killer_obj.read().map_err(|_| "Failed to lock killer")?;

        // Get killer's player
        let killer_player = match killer_lock.get_controlling_player_ref() {
            Some(p) => p,
            None => return Ok(false),
        };

        let player_lock = killer_player.read().map_err(|_| "Failed to lock player")?;

        // Check if player has the required science
        Ok(player_lock.has_science(template.killer_science))
    }

    /// Create a crate object
    /// Matches C++ CreateCrateDie::createCrate
    fn create_crate(
        &self,
        template: &CrateTemplate,
        owner_object: &Arc<RwLock<Object>>,
        _killer: Option<&Arc<RwLock<Object>>>,
    ) -> Result<Option<ObjectId>, String> {
        let obj_lock = owner_object.read().map_err(|_| "Failed to lock object")?;
        let center_point = obj_lock.get_position();
        let layer = obj_lock.get_layer();
        drop(obj_lock);

        // Select which crate to create from the weighted list
        // Matches C++ lines 156-173
        let multiple_crate_pick = GameLogicRandomValueReal(0.0, 1.0);
        let crate_name = match template.select_crate(multiple_crate_pick) {
            Some(name) => name,
            None => return Ok(None), // No crate selected (empty list or sum < 1.0)
        };

        // Find the thing template for this crate
        // In a full implementation, would use TheThingFactory
        println!("Would create crate '{}' at position {:?}", crate_name, center_point);

        // Find a valid position for the crate
        // Matches C++ lines 180-207
        let creation_point = if layer != PathfindLayerEnum::Ground {
            // Non-ground layers - use center point directly
            center_point
        } else {
            // Ground layer - find position around the death location
            let mut fp_options = FindPositionOptions {
                min_radius: 0.0,
                max_radius: 5.0,
                relationship_object_id: Some(owner_object.read().map_err(|_| "Owner lock poisoned")?.get_id()),
                flags: FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS,
                ..Default::default()
            };

            let mut creation_point = center_point;

            // Try tight scan first
            if !self.find_position_around(&center_point, &fp_options, &mut creation_point)? {
                // Try larger scan if tight scan fails
                fp_options.min_radius = 0.0;
                fp_options.max_radius = 125.0;
                fp_options.relationship_object_id = None;
                fp_options.flags = FPF_NONE;

                if !self.find_position_around(&center_point, &fp_options, &mut creation_point)? {
                    // No valid position found
                    return Ok(None);
                }
            }

            creation_point
        };

        // Create the crate object
        // Matches C++ lines 211-226
        let crate_id = self.create_crate_object(&crate_name, &creation_point, layer)?;

        // Set team if owned by maker
        if template.is_owned_by_maker {
            if let Some(crate_id_val) = crate_id {
                self.set_crate_team(crate_id_val, owner_object)?;
            }
        }

        // Notify AI about the crate
        // Matches C++ lines 87-99
        if let Some(crate_id_val) = crate_id {
            self.notify_ai_about_crate(crate_id_val, _killer)?;
        }

        Ok(crate_id)
    }

    /// Find a valid position around a point
    fn find_position_around(
        &self,
        center: &Coord3D,
        options: &FindPositionOptions,
        result: &mut Coord3D,
    ) -> Result<bool, String> {
        if let Some(partition) = crate::helpers::ThePartitionManager::get() {
            return Ok(partition.find_position_around_with_options(center, options, result));
        }
        Ok(false)
    }

    /// Create a crate object in the world
    fn create_crate_object(
        &self,
        crate_name: &str,
        position: &Coord3D,
        layer: PathfindLayerEnum,
    ) -> Result<Option<ObjectId>, String> {
        let template = match TheThingFactory::find_template(crate_name) {
            Some(template) => template,
            None => return Ok(None),
        };

        let factory = TheThingFactory::get().map_err(|e| e.to_string())?;
        let crate_arc = factory
            .new_object_optional_team(template, None)
            .map_err(|e| e.to_string())?;

        let mut id = INVALID_ID;
        if let Ok(mut crate_obj) = crate_arc.write() {
            id = crate_obj.get_id();
            let _ = crate_obj.set_position(position);
            let orient = GameLogicRandomValueReal(0.0, 2.0 * PI);
            let _ = crate_obj.set_orientation(orient);
            crate_obj.set_layer(layer);
        }

        if id == INVALID_ID {
            return Ok(None);
        }

        Ok(Some(id))
    }

    /// Set the team of the crate object
    fn set_crate_team(
        &self,
        crate_id: ObjectId,
        owner_object: &Arc<RwLock<Object>>,
    ) -> Result<(), String> {
        let obj_lock = owner_object.read().map_err(|_| "Failed to lock object")?;
        let Some(player_arc) = obj_lock.get_controlling_player() else {
            return Ok(());
        };
        drop(obj_lock);

        let player_lock = player_arc.read().map_err(|_| "Failed to lock player")?;
        let Some(team_arc) = player_lock.get_default_team() else {
            return Ok(());
        };
        drop(player_lock);

        let Some(crate_arc) = OBJECT_REGISTRY.get_object(crate_id) else {
            return Ok(());
        };
        if let Ok(mut crate_obj) = crate_arc.write() {
            let _ = crate_obj.set_team(Some(team_arc));
        }
        Ok(())
    }

    /// Notify AI about crate creation
    fn notify_ai_about_crate(
        &self,
        crate_id: ObjectId,
        killer: Option<&Arc<RwLock<Object>>>,
    ) -> Result<(), String> {
        let Some(killer_obj) = killer else {
            return Ok(());
        };
        let killer_lock = killer_obj.read().map_err(|_| "Failed to lock killer")?;

        if let Some(player) = killer_lock.get_controlling_player() {
            let player_lock = player.read().map_err(|_| "Failed to lock player")?;

            if player_lock.get_player_type() == PlayerType::Computer {
                if let Some(ai) = killer_lock.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.notify_crate(crate_id);
                    }
                }
            }
        }

        Ok(())
    }

    /// Serialize module state
    /// Matches C++ CreateCrateDie::xfer
    pub fn serialize<W: std::io::Write>(&self, writer: &mut W) -> Result<(), String> {
        // Version
        writer
            .write_all(&self.version.to_le_bytes())
            .map_err(|e| format!("Serialization error: {}", e))?;

        // Module would serialize its state here
        Ok(())
    }

    /// Deserialize module state
    pub fn deserialize<R: std::io::Read>(&mut self, reader: &mut R) -> Result<(), String> {
        // Read version
        let mut version_bytes = [0u8; 4];
        reader
            .read_exact(&mut version_bytes)
            .map_err(|e| format!("Deserialization error: {}", e))?;
        let version = u32::from_le_bytes(version_bytes);

        if version != self.version {
            return Err(format!(
                "Version mismatch: expected {}, got {}",
                self.version, version
            ));
        }

        Ok(())
    }

    /// Compute CRC for this module
    /// Matches C++ CreateCrateDie::crc
    pub fn compute_crc(&self) -> u32 {
        let mut crc = 0u32;
        crc ^= self.version;
        crc ^= self.object_id as u32;
        for name in &self.module_data.crate_name_list {
            crc ^= name.chars().fold(0u32, |acc, c| acc.wrapping_add(c as u32));
        }
        crc
    }

    /// Load post-processing
    /// Matches C++ CreateCrateDie::loadPostProcess
    pub fn load_post_process(&mut self) -> Result<(), String> {
        // Perform any post-load processing
        Ok(())
    }
}

/// Helper functions matching C++ GameLogic random functions
fn GameLogicRandomValueReal(min: f32, max: f32) -> f32 {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    rng.gen::<f32>() * (max - min) + min
}

fn GameLogicRandomValue(min: i32, max: i32) -> i32 {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    rng.gen_range(min..=max)
}

/// Relationship enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relationship {
    Allies,
    Enemies,
    Neutral,
}

/// Player type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerType {
    Human,
    Computer,
    Observer,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_crate_die_module_data() {
        let data = CreateCrateDieModuleData::new()
            .add_crate_name("MoneyCrate".to_string())
            .add_crate_name("HealthCrate".to_string());

        assert_eq!(data.crate_name_list.len(), 2);
        assert_eq!(data.crate_name_list[0], "MoneyCrate");
        assert_eq!(data.crate_name_list[1], "HealthCrate");
    }

    #[test]
    fn test_find_position_options() {
        let options = FindPositionOptions {
            min_radius: 5.0,
            max_radius: 15.0,
            relationship_object_id: None,
            flags: FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS,
            ..Default::default()
        };

        assert_eq!(options.min_radius, 5.0);
        assert_eq!(options.max_radius, 15.0);
        assert_eq!(options.flags, FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS);
    }

    #[test]
    fn test_pathfind_layer_enum() {
        let layer = PathfindLayerEnum::Ground;
        assert_eq!(layer, PathfindLayerEnum::Ground);
        assert_ne!(layer, PathfindLayerEnum::Air);
    }

    #[test]
    fn test_create_crate_die_creation() {
        let data = CreateCrateDieModuleData::new()
            .add_crate_name("TestCrate".to_string());

        let module = CreateCrateDie::new(123, data);

        assert_eq!(module.object_id, 123);
        assert_eq!(module.version, 1);
        assert_eq!(module.module_data.crate_name_list.len(), 1);
    }

    #[test]
    fn test_crc_computation() {
        let data = CreateCrateDieModuleData::new()
            .add_crate_name("TestCrate".to_string());

        let module = CreateCrateDie::new(123, data);
        let crc = module.compute_crc();

        // CRC should be non-zero
        assert_ne!(crc, 0);

        // Same module should produce same CRC
        let crc2 = module.compute_crc();
        assert_eq!(crc, crc2);
    }

    #[test]
    fn test_serialization_version() {
        let data = CreateCrateDieModuleData::new();
        let module = CreateCrateDie::new(456, data);

        let mut buffer = Vec::new();
        module.serialize(&mut buffer).unwrap();

        assert!(buffer.len() >= 4); // At least version number
    }

    #[test]
    fn test_random_value_real_range() {
        for _ in 0..100 {
            let value = GameLogicRandomValueReal(0.0, 1.0);
            assert!(value >= 0.0 && value <= 1.0);
        }
    }

    #[test]
    fn test_random_value_int_range() {
        for _ in 0..100 {
            let value = GameLogicRandomValue(1, 10);
            assert!(value >= 1 && value <= 10);
        }
    }
}
