//! OCLSpecialPower
//!
//! Port of OCLSpecialPower.h and OCLSpecialPower.cpp
//! Author: Colin Day, April 2002 (C++), Rust Port
//!
//! Special powers that are driven by object creation lists (OCL).
//! Creates objects (units, effects, projectiles) at a target location.
//! Used by many other special powers: airdrops, missile strikes, etc.
//!
//! C++ enum OCLCreateLocType determines WHERE created objects originate:
//!   CREATE_AT_EDGE_NEAR_SOURCE (0)  - edge of map near the source object
//!   CREATE_AT_EDGE_NEAR_TARGET (1)  - edge of map near the target location
//!   CREATE_AT_LOCATION (2)          - directly at the target location
//!   USE_OWNER_OBJECT (3)            - at the target location, same team as owner
//!   CREATE_ABOVE_LOCATION (4)       - above the target location (+300 height)
//!   CREATE_AT_EDGE_FARTHEST_FROM_TARGET (5) - farthest map edge from target

use std::sync::Arc;

use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::Snapshotable;
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

use crate::common::science::ScienceType;
use crate::common::{AsciiString, Coord3D, ObjectID, Real};
use crate::helpers::{TheGameLogic, TheObjectCreationListStore, TheTerrainLogic};
use crate::modules::BehaviorModuleInterface;
use crate::object::special_power_module::SpecialPowerModuleData;

/// Matches C++ CREATE_ABOVE_LOCATION_HEIGHT = 300
const CREATE_ABOVE_LOCATION_HEIGHT: Real = 300.0;

/// Matches C++ MAX_ADJUST_RADIUS = 500
const MAX_ADJUST_RADIUS: Real = 500.0;

/// OCL create location type. Matches C++ OCLCreateLocType enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OCLCreateLocType {
    /// CREATE_AT_EDGE_NEAR_SOURCE
    CreateAtEdgeNearSource = 0,
    /// CREATE_AT_EDGE_NEAR_TARGET
    CreateAtEdgeNearTarget = 1,
    /// CREATE_AT_LOCATION
    CreateAtLocation = 2,
    /// USE_OWNER_OBJECT
    UseOwnerObject = 3,
    /// CREATE_ABOVE_LOCATION
    CreateAboveLocation = 4,
    /// CREATE_AT_EDGE_FARTHEST_FROM_TARGET
    CreateAtEdgeFarthestFromTarget = 5,
}

impl Default for OCLCreateLocType {
    fn default() -> Self {
        OCLCreateLocType::CreateAtEdgeNearSource
    }
}

/// OCL upgrade pair: a science prerequisite maps to a different OCL.
/// Matches C++ OCLSpecialPowerModuleData::Upgrades.
#[derive(Debug, Clone)]
pub struct OCLUpgrade {
    /// Science that unlocks this OCL variant
    pub science: ScienceType,
    /// Name of the object creation list
    pub ocl_name: AsciiString,
}

impl Default for OCLUpgrade {
    fn default() -> Self {
        Self {
            science: crate::common::science::SCIENCE_INVALID,
            ocl_name: AsciiString::default(),
        }
    }
}

/// Module data for OCLSpecialPower.
/// Matches C++ OCLSpecialPowerModuleData.
#[derive(Debug, Clone)]
pub struct OclSpecialPowerModuleData {
    pub base: SpecialPowerModuleData,
    /// Default OCL name (used when no upgrade science is met)
    pub default_ocl: AsciiString,
    /// Upgrade OCL pairs: if player has science, use that OCL instead
    pub upgrade_ocl: Vec<OCLUpgrade>,
    /// Where created objects originate from
    pub create_loc: OCLCreateLocType,
    /// Whether to adjust target position to nearest passable cell
    pub ocl_adjust_position_to_passable: bool,
    /// Reference thing template name (for construction site placement)
    pub reference_thing_name: AsciiString,
}

impl Default for OclSpecialPowerModuleData {
    fn default() -> Self {
        Self {
            base: SpecialPowerModuleData::default(),
            default_ocl: AsciiString::default(),
            upgrade_ocl: Vec::new(),
            create_loc: OCLCreateLocType::default(),
            ocl_adjust_position_to_passable: false,
            reference_thing_name: AsciiString::default(),
        }
    }
}

impl OclSpecialPowerModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, OCL_SPECIAL_POWER_FIELDS)
    }
}

impl ModuleData for OclSpecialPowerModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.base.base.set_module_tag_name_key(key);
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.base.get_module_tag_name_key()
    }
}

impl Snapshotable for OclSpecialPowerModuleData {
    fn crc(&self, _xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        self.base.crc(_xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

/// OCLSpecialPower module.
///
/// Matches C++ OCLSpecialPower which extends SpecialPowerModule.
/// Creates objects from an ObjectCreationList at a target location.
pub struct OclSpecialPower {
    module_name_key: NameKeyType,
    data: Arc<OclSpecialPowerModuleData>,
    owner_object_id: ObjectID,
}

impl OclSpecialPower {
    pub fn new(
        module_name_key: NameKeyType,
        owner_object_id: ObjectID,
        data: Arc<OclSpecialPowerModuleData>,
    ) -> Self {
        Self {
            module_name_key,
            data,
            owner_object_id,
        }
    }

    /// Find the best OCL to use, checking upgrade science first.
    /// Matches C++ OCLSpecialPower::findOCL().
    fn find_ocl_name(&self) -> Option<AsciiString> {
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) {
            if let Ok(owner_guard) = owner.read() {
                if let Some(player) = owner_guard.get_controlling_player() {
                    if let Ok(player_guard) = player.read() {
                        for upgrade in &self.data.upgrade_ocl {
                            if player_guard.has_science(upgrade.science) {
                                return Some(upgrade.ocl_name.clone());
                            }
                        }
                    }
                }
            }
        }
        if self.data.default_ocl.is_empty() {
            None
        } else {
            Some(self.data.default_ocl.clone())
        }
    }

    /// Get the reference thing template name (for construction site placement).
    /// Matches C++ OCLSpecialPower::getReferenceThingTemplate().
    pub fn get_reference_thing_template(&self) -> Option<String> {
        if self.data.reference_thing_name.is_empty() {
            None
        } else {
            Some(self.data.reference_thing_name.to_string())
        }
    }

    /// Execute the OCL power at a location.
    /// Matches C++ OCLSpecialPower::doSpecialPowerAtLocation().
    pub fn do_special_power_at_location(&self, loc: &Coord3D, angle: Real) -> Result<(), String> {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return Ok(());
        };
        {
            let Ok(owner_guard) = owner.read() else {
                return Ok(());
            };
            if owner_guard.is_disabled() {
                return Ok(());
            }
        }

        let Some(ocl_name) = self.find_ocl_name() else {
            return Ok(());
        };

        let Some(ocl) = TheObjectCreationListStore::find_object_creation_list(ocl_name.as_str())
        else {
            log::warn!("OCLSpecialPower: OCL '{}' not found", ocl_name.as_str());
            return Ok(());
        };

        let mut target_coord = *loc;

        // Adjust position to passable if requested (C++ m_isOCLAdjustPositionToPassable)
        if self.data.ocl_adjust_position_to_passable {
            // Try to find a passable position near the target
            if let Some(adjusted) = find_passable_position_near(&target_coord) {
                target_coord = adjusted;
            }
            // If findPosition fails, don't monkey with target coord (C++ behavior)
        }

        let Ok(owner_guard) = owner.read() else {
            return Ok(());
        };

        // Compute creation coordinate based on create_loc
        // Matches C++ OCLSpecialPower::doSpecialPowerAtLocation() creation coord logic
        let creation_coord = match self.data.create_loc {
            OCLCreateLocType::CreateAtEdgeNearSource => TheTerrainLogic::get()
                .map(|terrain| terrain.find_closest_edge_point(owner_guard.get_position()))
                .unwrap_or(*owner_guard.get_position()),
            OCLCreateLocType::CreateAtEdgeNearTarget => TheTerrainLogic::get()
                .map(|terrain| terrain.find_closest_edge_point(&target_coord))
                .unwrap_or(target_coord),
            OCLCreateLocType::CreateAtEdgeFarthestFromTarget => {
                let mut edge = TheTerrainLogic::get()
                    .map(|terrain| terrain.find_farthest_edge_point(&target_coord))
                    .unwrap_or(target_coord);
                edge.z += CREATE_ABOVE_LOCATION_HEIGHT;
                edge
            }
            OCLCreateLocType::CreateAtLocation => target_coord,
            OCLCreateLocType::UseOwnerObject => target_coord,
            OCLCreateLocType::CreateAboveLocation => {
                let mut above = target_coord;
                above.z += CREATE_ABOVE_LOCATION_HEIGHT;
                above
            }
        };

        // Execute the OCL
        let ctx = crate::object_creation_list::live_creation_context();
        let create_owner = Self::create_owner_flag_for_create_loc(self.data.create_loc);

        let result = if create_owner {
            ocl.create_with_angle(
                &ctx,
                Some(&*owner_guard),
                &creation_coord,
                &target_coord,
                angle,
                0,
            )
        } else {
            // C++ USE_OWNER_OBJECT passes createOwner=false.
            ocl.create_with_angle_and_owner_flag(
                &ctx,
                Some(&*owner_guard),
                &creation_coord,
                &target_coord,
                angle,
                false,
                0,
            )
        };

        if let Some(created) = result {
            log::debug!(
                "OCLSpecialPower: created object at ({:.1}, {:.1}, {:.1})",
                target_coord.x,
                target_coord.y,
                target_coord.z
            );
            let _ = created; // Object created and added to game world
        }

        Ok(())
    }

    fn create_owner_flag_for_create_loc(create_loc: OCLCreateLocType) -> bool {
        create_loc != OCLCreateLocType::UseOwnerObject
    }

    /// Execute the OCL power at an object's position.
    /// Matches C++ OCLSpecialPower::doSpecialPowerAtObject().
    pub fn do_special_power_at_object(&self, obj_id: ObjectID) -> Result<(), String> {
        // Check disabled
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) {
            if let Ok(owner_guard) = owner.read() {
                if owner_guard.is_disabled() {
                    return Ok(());
                }
            }
        }

        let Some(obj) = TheGameLogic::find_object_by_id(obj_id) else {
            return Ok(());
        };

        let Ok(obj_guard) = obj.read() else {
            return Ok(());
        };

        let pos = *obj_guard.get_position();
        drop(obj_guard);

        // C++ uses INVALID_ANGLE for object targeting
        let invalid_angle: Real = -999999.0;
        self.do_special_power_at_location(&pos, invalid_angle)
    }

    /// Execute the OCL power with no specific target (use owner position).
    /// Matches C++ OCLSpecialPower::doSpecialPower().
    pub fn do_special_power(&self) -> Result<(), String> {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return Ok(());
        };
        {
            let Ok(owner_guard) = owner.read() else {
                return Ok(());
            };
            if owner_guard.is_disabled() {
                return Ok(());
            }
        }

        let Some(ocl_name) = self.find_ocl_name() else {
            return Ok(());
        };

        let Some(ocl) = TheObjectCreationListStore::find_object_creation_list(ocl_name.as_str())
        else {
            return Ok(());
        };

        let Ok(owner_guard) = owner.read() else {
            return Ok(());
        };
        let pos = *owner_guard.get_position();

        let ctx = crate::object_creation_list::live_creation_context();
        let _ = ocl.create_with_angle(&ctx, Some(&*owner_guard), &pos, &pos, 0.0, 0);

        Ok(())
    }
}

/// Try to find a passable position near the given coordinate.
/// Simplified version - the full C++ version uses PartitionManager::findPositionAround
/// with FPF_CLEAR_CELLS_ONLY flag and MAX_ADJUST_RADIUS.
fn find_passable_position_near(target: &Coord3D) -> Option<Coord3D> {
    let Some(partition) = crate::helpers::ThePartitionManager::get() else {
        return Some(*target);
    };

    let options = crate::helpers::FindPositionOptions {
        max_radius: MAX_ADJUST_RADIUS,
        flags: crate::helpers::FPF_CLEAR_CELLS_ONLY,
        ..Default::default()
    };

    let mut result = *target;
    if partition.find_position_around_with_options(target, &options, &mut result) {
        Some(result)
    } else {
        Some(*target)
    }
}

impl Module for OclSpecialPower {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
}

impl Snapshotable for OclSpecialPower {
    fn crc(&self, _xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        // Version 1: Initial version - extends base class only
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        // Version 1: Initial version - extends base class only
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("OCLSpecialPower xfer version failed: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Matches C++ OCLSpecialPower::loadPostProcess()
        Ok(())
    }
}

impl BehaviorModuleInterface for OclSpecialPower {
    fn get_module_name(&self) -> &'static str {
        "OCLSpecialPower"
    }
}

// INI field parsers

fn parse_special_power_template_field(
    _ini: &mut INI,
    data: &mut OclSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let name = AsciiString::from(*token);
    data.base.special_power_template =
        Some(crate::object::special_power_template::find_or_create_special_power_template(&name));
    Ok(())
}

fn parse_ocl_field(
    _ini: &mut INI,
    data: &mut OclSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.default_ocl = AsciiString::from(*token);
    Ok(())
}

fn parse_create_location(
    _ini: &mut INI,
    data: &mut OclSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.create_loc = match *token {
        "CREATE_AT_EDGE_NEAR_SOURCE" => OCLCreateLocType::CreateAtEdgeNearSource,
        "CREATE_AT_EDGE_NEAR_TARGET" => OCLCreateLocType::CreateAtEdgeNearTarget,
        "CREATE_AT_LOCATION" => OCLCreateLocType::CreateAtLocation,
        "USE_OWNER_OBJECT" => OCLCreateLocType::UseOwnerObject,
        "CREATE_ABOVE_LOCATION" => OCLCreateLocType::CreateAboveLocation,
        "CREATE_AT_EDGE_FARTHEST_FROM_TARGET" => OCLCreateLocType::CreateAtEdgeFarthestFromTarget,
        _ => {
            // Try to parse as index
            match token.parse::<u8>() {
                Ok(0) => OCLCreateLocType::CreateAtEdgeNearSource,
                Ok(1) => OCLCreateLocType::CreateAtEdgeNearTarget,
                Ok(2) => OCLCreateLocType::CreateAtLocation,
                Ok(3) => OCLCreateLocType::UseOwnerObject,
                Ok(4) => OCLCreateLocType::CreateAboveLocation,
                Ok(5) => OCLCreateLocType::CreateAtEdgeFarthestFromTarget,
                _ => OCLCreateLocType::default(),
            }
        }
    };
    Ok(())
}

fn parse_reference_object(
    _ini: &mut INI,
    data: &mut OclSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.reference_thing_name = AsciiString::from(*token);
    Ok(())
}

fn parse_ocl_adjust_position(
    _ini: &mut INI,
    data: &mut OclSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.ocl_adjust_position_to_passable = INI::parse_bool(token)?;
    Ok(())
}

fn parse_upgrade_ocl(
    _ini: &mut INI,
    data: &mut OclSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    // C++ parseOCLUpgradePair: first token is science, second is OCL name
    let non_eq: Vec<&&str> = tokens.iter().filter(|t| **t != "=").collect();
    if non_eq.len() < 2 {
        return Err(INIError::InvalidData);
    }

    // Parse science by name hash (ScienceType = i32)
    // In the full implementation, this would use ScienceStore lookup
    let science_name = *non_eq[0];
    let mut hash: i32 = 0;
    for c in science_name.chars() {
        hash = hash.wrapping_mul(31).wrapping_add(c as i32);
    }
    let science = if science_name.is_empty() {
        ScienceType::default()
    } else {
        hash.abs()
    };
    let ocl_name = AsciiString::from(*non_eq[1]);

    data.upgrade_ocl.push(OCLUpgrade { science, ocl_name });
    Ok(())
}

const OCL_SPECIAL_POWER_FIELDS: &[FieldParse<OclSpecialPowerModuleData>] = &[
    FieldParse {
        token: "SpecialPowerTemplate",
        parse: parse_special_power_template_field,
    },
    FieldParse {
        token: "OCL",
        parse: parse_ocl_field,
    },
    FieldParse {
        token: "CreateLocation",
        parse: parse_create_location,
    },
    FieldParse {
        token: "ReferenceObject",
        parse: parse_reference_object,
    },
    FieldParse {
        token: "OCLAdjustPositionToPassable",
        parse: parse_ocl_adjust_position,
    },
    FieldParse {
        token: "UpgradeOCL",
        parse: parse_upgrade_ocl,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ocl_special_power_default() {
        let data = OclSpecialPowerModuleData::default();
        assert!(data.default_ocl.is_empty());
        assert!(data.upgrade_ocl.is_empty());
        assert_eq!(data.create_loc, OCLCreateLocType::CreateAtEdgeNearSource);
        assert!(!data.ocl_adjust_position_to_passable);
    }

    #[test]
    fn test_ocl_create_loc_type_default() {
        assert_eq!(
            OCLCreateLocType::default(),
            OCLCreateLocType::CreateAtEdgeNearSource
        );
    }

    #[test]
    fn test_ocl_create_loc_type_values() {
        assert_eq!(OCLCreateLocType::CreateAtEdgeNearSource as u8, 0);
        assert_eq!(OCLCreateLocType::CreateAtEdgeNearTarget as u8, 1);
        assert_eq!(OCLCreateLocType::CreateAtLocation as u8, 2);
        assert_eq!(OCLCreateLocType::UseOwnerObject as u8, 3);
        assert_eq!(OCLCreateLocType::CreateAboveLocation as u8, 4);
        assert_eq!(OCLCreateLocType::CreateAtEdgeFarthestFromTarget as u8, 5);
    }

    #[test]
    fn test_find_ocl_name_no_upgrades() {
        let mut data = OclSpecialPowerModuleData::default();
        data.default_ocl = AsciiString::from("OCL_TestDefault");
        let arc_data = Arc::new(data);
        let power = OclSpecialPower::new(0, 0, arc_data);
        // Without a valid owner, falls back to default
        assert_eq!(
            power.find_ocl_name(),
            Some(AsciiString::from("OCL_TestDefault"))
        );
    }

    #[test]
    fn test_find_ocl_name_empty_default() {
        let data = OclSpecialPowerModuleData::default();
        let arc_data = Arc::new(data);
        let power = OclSpecialPower::new(0, 0, arc_data);
        assert_eq!(power.find_ocl_name(), None);
    }

    #[test]
    fn test_module_name() {
        let data = OclSpecialPowerModuleData::default();
        let arc_data = Arc::new(data);
        let power = OclSpecialPower::new(0, 0, arc_data);
        assert_eq!(power.get_module_name(), "OCLSpecialPower");
    }

    #[test]
    fn use_owner_object_matches_cpp_create_owner_false() {
        assert!(!OclSpecialPower::create_owner_flag_for_create_loc(
            OCLCreateLocType::UseOwnerObject
        ));
        assert!(OclSpecialPower::create_owner_flag_for_create_loc(
            OCLCreateLocType::CreateAtLocation
        ));
    }
}
