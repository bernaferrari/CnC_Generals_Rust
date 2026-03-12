// FILE: ocl_special_power.rs
// Port of OCLSpecialPower.h and OCLSpecialPower.cpp
// Author: Rust Port
// Desc: Special powers that are driven by object creation lists

use crate::common::science::ScienceType;
use crate::common::{AsciiString, Coord3D, LegacyModuleData};
use crate::helpers::{TheGameLogic, ThePartitionManager, TheTerrainLogic};
use crate::modules::SpecialPowerModuleInterface as EngineSpecialPowerModuleInterface;
use crate::object::special_power_module::{
    FrameCount, ObjectId, SpecialPowerCommandOptions, SpecialPowerModule, SpecialPowerModuleData,
    SpecialPowerModuleInterface as ObjSpecialPowerModuleInterface, Waypoint,
};
use crate::object::special_power_template::find_or_create_special_power_template;
use crate::object::special_power_template::SpecialPowerTemplate;
use crate::object_creation_list::get_object_creation_list_store;
use crate::object_creation_list::nuggets::INVALID_ANGLE;
use crate::object_creation_list::{live_creation_context, ObjectCreationList};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::rts::get_science_store;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use std::sync::Arc;

/// OCL create location type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OclCreateLocType {
    /// Create at edge nearest to source object
    CreateAtEdgeNearSource,
    /// Create at edge nearest to target location
    CreateAtEdgeNearTarget,
    /// Create at exact target location
    CreateAtLocation,
    /// Use owner object's location
    UseOwnerObject,
    /// Create above target location (airborne)
    CreateAboveLocation,
    /// Create at edge farthest from target
    CreateAtEdgeFarthestFromTarget,
}

impl Default for OclCreateLocType {
    fn default() -> Self {
        OclCreateLocType::CreateAtEdgeNearSource
    }
}

/// Upgrade pair for OCL with science requirement
#[derive(Debug, Clone)]
pub struct OclUpgrade {
    /// Science required for this upgrade
    pub science: ScienceType,
    /// Object creation list to use when science is available
    pub ocl: Arc<ObjectCreationList>,
}

/// Module data for OCL special power
#[derive(Debug, Clone)]
pub struct OclSpecialPowerModuleData {
    pub module_tag_name_key: NameKeyType,
    /// Base special power data
    pub base: SpecialPowerModuleData,

    /// Upgrade OCLs that replace default when science is available
    pub upgrade_ocl: Vec<OclUpgrade>,

    /// Default object creation list
    pub default_ocl: Option<Arc<ObjectCreationList>>,

    /// Where to create objects
    pub create_loc: OclCreateLocType,

    /// Adjust position to nearest passable cell
    pub ocl_adjust_position_to_passable: bool,

    /// Reference thing name (for construction sites)
    pub reference_thing_name: Option<String>,
}

impl Default for OclSpecialPowerModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: SpecialPowerModuleData::default(),
            upgrade_ocl: Vec::new(),
            default_ocl: None,
            create_loc: OclCreateLocType::CreateAtEdgeNearSource,
            ocl_adjust_position_to_passable: false,
            reference_thing_name: None,
        }
    }
}

impl OclSpecialPowerModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, OCL_SPECIAL_POWER_FIELDS)
    }
}

impl Snapshotable for OclSpecialPowerModuleData {
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

crate::impl_legacy_module_data_with_key_field!(OclSpecialPowerModuleData, module_tag_name_key);

fn parse_special_power_template_field(
    _ini: &mut INI,
    data: &mut OclSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let name = AsciiString::from(*token);
    data.base.special_power_template = Some(find_or_create_special_power_template(&name));
    Ok(())
}

fn parse_bool_field(setter: &mut dyn FnMut(bool), tokens: &[&str]) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    setter(INI::parse_bool(token)?);
    Ok(())
}

fn parse_audio_event(
    _ini: &mut INI,
    data: &mut OclSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.base.initiate_sound = crate::common::audio::AudioEventRts::new(*token);
    Ok(())
}

fn parse_ocl_field(
    _ini: &mut INI,
    data: &mut OclSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let store = get_object_creation_list_store();
    if let Some(store) = store.as_ref() {
        data.default_ocl = store.find_object_creation_list(token);
    }
    Ok(())
}

fn parse_upgrade_ocl_field(
    _ini: &mut INI,
    data: &mut OclSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.len() < 2 {
        return Err(INIError::InvalidData);
    }
    let store = get_object_creation_list_store();
    let store = store.as_ref().ok_or(INIError::InvalidData)?;
    let science_store = get_science_store().ok_or(INIError::InvalidData)?;
    let science = science_store.get_science_from_internal_name(tokens[0].trim());
    if let Some(ocl) = store.find_object_creation_list(tokens[1].trim()) {
        data.upgrade_ocl.push(OclUpgrade { science, ocl });
    }
    Ok(())
}

fn parse_create_location_field(
    _ini: &mut INI,
    data: &mut OclSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let value = token.trim().to_ascii_uppercase();
    data.create_loc = match value.as_str() {
        "CREATE_AT_EDGE_NEAR_SOURCE" => OclCreateLocType::CreateAtEdgeNearSource,
        "CREATE_AT_EDGE_NEAR_TARGET" => OclCreateLocType::CreateAtEdgeNearTarget,
        "CREATE_AT_LOCATION" => OclCreateLocType::CreateAtLocation,
        "USE_OWNER_OBJECT" => OclCreateLocType::UseOwnerObject,
        "CREATE_ABOVE_LOCATION" => OclCreateLocType::CreateAboveLocation,
        "CREATE_AT_EDGE_FARTHEST_FROM_TARGET" => OclCreateLocType::CreateAtEdgeFarthestFromTarget,
        _ => return Err(INIError::InvalidData),
    };
    Ok(())
}

fn parse_reference_thing_field(
    _ini: &mut INI,
    data: &mut OclSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.reference_thing_name = Some(token.trim().to_string());
    Ok(())
}

const OCL_SPECIAL_POWER_FIELDS: &[FieldParse<OclSpecialPowerModuleData>] = &[
    FieldParse {
        token: "SpecialPowerTemplate",
        parse: parse_special_power_template_field,
    },
    FieldParse {
        token: "UpdateModuleStartsAttack",
        parse: |_, data, tokens| {
            parse_bool_field(&mut |v| data.base.update_module_starts_attack = v, tokens)
        },
    },
    FieldParse {
        token: "StartsPaused",
        parse: |_, data, tokens| parse_bool_field(&mut |v| data.base.starts_paused = v, tokens),
    },
    FieldParse {
        token: "InitiateSound",
        parse: parse_audio_event,
    },
    FieldParse {
        token: "ScriptedSpecialPowerOnly",
        parse: |_, data, tokens| {
            parse_bool_field(&mut |v| data.base.scripted_special_power_only = v, tokens)
        },
    },
    FieldParse {
        token: "OCL",
        parse: parse_ocl_field,
    },
    FieldParse {
        token: "UpgradeOCL",
        parse: parse_upgrade_ocl_field,
    },
    FieldParse {
        token: "CreateLocation",
        parse: parse_create_location_field,
    },
    FieldParse {
        token: "OCLAdjustPositionToPassable",
        parse: |_, data, tokens| {
            parse_bool_field(&mut |v| data.ocl_adjust_position_to_passable = v, tokens)
        },
    },
    FieldParse {
        token: "ReferenceThing",
        parse: parse_reference_thing_field,
    },
];

/// OCL special power implementation
/// Spawns objects from an object creation list (OCL) at specified locations
#[derive(Debug, Clone)]
pub struct OclSpecialPower {
    /// Base special power module
    base: SpecialPowerModule,

    /// OCL-specific module data
    ocl_data: Arc<OclSpecialPowerModuleData>,
    module_name_key: NameKeyType,
}

impl OclSpecialPower {
    const CREATE_ABOVE_LOCATION_HEIGHT: f32 = 300.0;
    const MAX_ADJUST_RADIUS: f32 = 500.0;

    /// Create a new OCL special power
    pub fn new(
        module_name_key: NameKeyType,
        owner_object_id: ObjectId,
        data: Arc<OclSpecialPowerModuleData>,
    ) -> Self {
        let base = SpecialPowerModule::new(owner_object_id, data.base.clone());
        Self {
            base,
            ocl_data: data,
            module_name_key,
        }
    }

    /// Find the appropriate OCL based on available sciences
    /// Matches C++ OCLSpecialPower::findOCL
    fn find_ocl(&self) -> Option<Arc<ObjectCreationList>> {
        if let Some(object) = TheGameLogic::find_object_by_id(self.base.get_owner_object_id()) {
            if let Ok(obj_read) = object.read() {
                if let Some(player) = obj_read.get_controlling_player() {
                    if let Ok(player_guard) = player.read() {
                        for upgrade in &self.ocl_data.upgrade_ocl {
                            if player_guard.has_science(upgrade.science) {
                                return Some(upgrade.ocl.clone());
                            }
                        }
                    }
                }
            }
        }

        self.ocl_data.default_ocl.clone()
    }

    /// Adjust target coordinate to nearest passable cell
    /// Matches C++ OCLSpecialPower::adjustToPassable
    fn adjust_to_passable(&self, target: &Coord3D) -> Coord3D {
        if !self.ocl_data.ocl_adjust_position_to_passable {
            return *target;
        }

        let Some(partition) = ThePartitionManager::get() else {
            return *target;
        };

        let mut result = Coord3D::new(target.x, target.y, target.z);
        let center = Coord3D::new(target.x, target.y, target.z);
        let mut options = crate::helpers::FindPositionOptions::default();
        options.min_radius = 0.0;
        options.max_radius = Self::MAX_ADJUST_RADIUS;
        options.flags = crate::helpers::FPF_CLEAR_CELLS_ONLY;
        if partition.find_position_around_with_options(&center, &options, &mut result) {
            return result;
        }

        *target
    }

    /// Find closest edge point to a location
    /// Matches C++ TheTerrainLogic->findClosestEdgePoint
    fn find_closest_edge_point(&self, location: &Coord3D) -> Coord3D {
        if let Some(terrain_logic) = TheTerrainLogic::get() {
            let pos = Coord3D::new(location.x, location.y, location.z);
            let edge = terrain_logic.find_closest_edge_point(&pos);
            return edge;
        }

        *location
    }

    /// Find farthest edge point from a location
    /// Matches C++ TheTerrainLogic->findFarthestEdgePoint
    fn find_farthest_edge_point(&self, location: &Coord3D) -> Coord3D {
        if let Some(terrain_logic) = TheTerrainLogic::get() {
            let pos = Coord3D::new(location.x, location.y, location.z);
            let edge = terrain_logic.find_farthest_edge_point(&pos);
            return edge;
        }

        *location
    }

    /// Get owner object position
    /// Matches C++ getObject()->getPosition()
    fn get_owner_position(&self) -> Coord3D {
        if let Some(object) = TheGameLogic::find_object_by_id(self.base.get_owner_object_id()) {
            if let Ok(obj_read) = object.read() {
                return *obj_read.get_position();
            }
        }

        Coord3D::new(0.0, 0.0, 0.0)
    }

    /// Create objects from OCL
    /// Matches C++ ObjectCreationList::create
    fn create_from_ocl(
        &self,
        ocl: &ObjectCreationList,
        creation_pos: &Coord3D,
        target_pos: &Coord3D,
        angle: f32,
        create_owner: bool,
    ) {
        let Some(owner_obj) = TheGameLogic::find_object_by_id(self.base.get_owner_object_id())
        else {
            return;
        };
        let Ok(owner_guard) = owner_obj.read() else {
            return;
        };

        let ctx = live_creation_context();
        let creation = Coord3D::new(creation_pos.x, creation_pos.y, creation_pos.z);
        let target = Coord3D::new(target_pos.x, target_pos.y, target_pos.z);

        if create_owner {
            let _ = ocl.create_with_angle(&ctx, Some(&*owner_guard), &creation, &target, angle, 0);
        } else {
            let _ = ocl.create_with_angle_and_owner_flag(
                &ctx,
                Some(&*owner_guard),
                &creation,
                &target,
                angle,
                false,
                0,
            );
        }
    }
}

// Implement the special power module interface
impl ObjSpecialPowerModuleInterface for OclSpecialPower {
    fn is_module_for_power(&self, special_power_template: &SpecialPowerTemplate) -> bool {
        self.base.is_module_for_power(special_power_template)
    }

    fn get_percent_ready(&self) -> f32 {
        crate::object::special_power_module::SpecialPowerModuleInterface::get_percent_ready(
            &self.base,
        )
    }

    fn get_power_name(&self) -> String {
        crate::object::special_power_module::SpecialPowerModuleInterface::get_power_name(&self.base)
    }

    fn get_special_power_template_full(&self) -> Option<Arc<SpecialPowerTemplate>> {
        self.base.get_special_power_template_full()
    }

    fn get_required_science(&self) -> ScienceType {
        self.base.get_required_science()
    }

    fn on_special_power_creation(&mut self) {
        self.base.on_special_power_creation()
    }

    fn set_ready_frame(&mut self, frame: u32) {
        self.base.set_ready_frame(frame)
    }

    fn pause_countdown(&mut self, pause: bool) {
        crate::object::special_power_module::SpecialPowerModuleInterface::pause_countdown(
            &mut self.base,
            pause,
        )
    }

    fn do_special_power(&mut self, command_options: SpecialPowerCommandOptions) {
        if let Some(object) = TheGameLogic::find_object_by_id(self.base.get_owner_object_id()) {
            if let Ok(obj_read) = object.read() {
                if obj_read.is_disabled() {
                    return;
                }
            }
        }

        let creation_coord = self.get_owner_position();

        // Call base class to handle triggers
        self.base
            .do_special_power_at_location(&creation_coord, INVALID_ANGLE, command_options);

        if let Some(ocl) = self.find_ocl() {
            self.create_from_ocl(&ocl, &creation_coord, &creation_coord, INVALID_ANGLE, false);
        }
    }

    fn do_special_power_at_object(
        &mut self,
        object_id: ObjectId,
        command_options: SpecialPowerCommandOptions,
    ) {
        if let Some(object) = TheGameLogic::find_object_by_id(self.base.get_owner_object_id()) {
            if let Ok(obj_read) = object.read() {
                if obj_read.is_disabled() {
                    return;
                }
            }
        }

        let object_pos = if let Some(target_object) =
            crate::helpers::TheGameLogic::find_object_by_id(object_id)
        {
            if let Ok(obj_read) = target_object.read() {
                let pos = obj_read.get_position();
                Some(Coord3D::new(pos.x, pos.y, pos.z))
            } else {
                None
            }
        } else {
            None
        };

        let Some(object_pos) = object_pos else {
            return;
        };

        ObjSpecialPowerModuleInterface::do_special_power_at_location(
            self,
            &object_pos,
            INVALID_ANGLE,
            command_options,
        );
    }

    fn do_special_power_at_location(
        &mut self,
        location: &Coord3D,
        angle: f32,
        command_options: SpecialPowerCommandOptions,
    ) {
        if let Some(object) = TheGameLogic::find_object_by_id(self.base.get_owner_object_id()) {
            if let Ok(obj_read) = object.read() {
                if obj_read.is_disabled() {
                    return;
                }
            }
        }

        let mut target_coord = self.adjust_to_passable(location);

        // Call base class to handle triggers
        self.base
            .do_special_power_at_location(&target_coord, angle, command_options);

        if let Some(ocl) = self.find_ocl() {
            let creation_coord = match self.ocl_data.create_loc {
                OclCreateLocType::CreateAtEdgeNearSource => {
                    let owner_pos = self.get_owner_position();
                    self.find_closest_edge_point(&owner_pos)
                }
                OclCreateLocType::CreateAtEdgeNearTarget => {
                    self.find_closest_edge_point(&target_coord)
                }
                OclCreateLocType::CreateAtEdgeFarthestFromTarget => {
                    let mut coord = self.find_farthest_edge_point(&target_coord);
                    coord.z += Self::CREATE_ABOVE_LOCATION_HEIGHT;
                    coord
                }
                OclCreateLocType::CreateAtLocation => target_coord,
                OclCreateLocType::UseOwnerObject => target_coord,
                OclCreateLocType::CreateAboveLocation => {
                    target_coord.z += Self::CREATE_ABOVE_LOCATION_HEIGHT;
                    target_coord
                }
            };

            let create_owner = self.ocl_data.create_loc != OclCreateLocType::UseOwnerObject;
            self.create_from_ocl(&ocl, &creation_coord, &target_coord, angle, create_owner);
        }
    }

    fn do_special_power_using_waypoints(
        &mut self,
        waypoint: &Waypoint,
        command_options: SpecialPowerCommandOptions,
    ) {
        // Waypoints not typically used for OCL powers
        self.base
            .do_special_power_using_waypoints(waypoint, command_options)
    }

    fn mark_special_power_triggered(&mut self, location: Option<&Coord3D>) {
        crate::object::special_power_module::SpecialPowerModuleInterface::mark_special_power_triggered(
            &mut self.base,
            location,
        )
    }

    fn start_power_recharge_at(&mut self, current_frame: FrameCount) {
        self.base.start_power_recharge_at(current_frame)
    }

    fn get_initiate_sound(&self) -> &crate::object::special_power_template::AudioEventRts {
        self.base.get_initiate_sound()
    }

    fn is_script_only(&self) -> bool {
        self.base.is_script_only()
    }

    fn get_reference_thing_template(&self) -> Option<String> {
        self.ocl_data.reference_thing_name.clone()
    }
}

impl Snapshotable for OclSpecialPower {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
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
        LegacyModuleData::get_module_tag_name_key(self.ocl_data.as_ref())
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.ocl_data.as_ref()
    }

    fn on_object_created(&mut self) {
        self.base.on_object_created();
    }
}

impl crate::modules::BehaviorModuleInterface for OclSpecialPower {
    fn get_special_power(&mut self) -> Option<&mut dyn EngineSpecialPowerModuleInterface> {
        Some(self)
    }

    fn get_special_power_module_interface(
        &mut self,
    ) -> Option<&mut dyn EngineSpecialPowerModuleInterface> {
        Some(self)
    }

    fn get_special_power_module_interface_const(
        &self,
    ) -> Option<&dyn EngineSpecialPowerModuleInterface> {
        Some(self)
    }
}

impl EngineSpecialPowerModuleInterface for OclSpecialPower {
    fn activate(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.activate()
    }

    fn can_activate(&self) -> bool {
        self.base.can_activate()
    }

    fn get_power_type(&self) -> u32 {
        self.base.get_power_type()
    }

    fn start_power_recharge(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.start_power_recharge()
    }

    fn get_ready_frame(&self) -> u32 {
        self.base.get_ready_frame()
    }

    fn is_ready(&self) -> bool {
        self.base.is_ready()
    }

    fn get_special_power_template(&self) -> Option<Arc<dyn std::any::Any>> {
        self.base.get_special_power_template()
    }

    fn get_special_power_template_full(&self) -> Option<Arc<SpecialPowerTemplate>> {
        self.base.get_special_power_template_full()
    }

    fn get_power_name(&self) -> String {
        crate::modules::SpecialPowerModuleInterface::get_power_name(&self.base)
    }

    fn get_percent_ready(&self) -> f32 {
        crate::modules::SpecialPowerModuleInterface::get_percent_ready(&self.base)
    }

    fn pause_countdown(&mut self, pause: bool) {
        crate::modules::SpecialPowerModuleInterface::pause_countdown(&mut self.base, pause)
    }

    fn mark_special_power_triggered(&mut self, location: Option<&crate::common::Coord3D>) {
        crate::modules::SpecialPowerModuleInterface::mark_special_power_triggered(
            &mut self.base,
            location,
        )
    }

    fn do_special_power(&mut self, command_options: SpecialPowerCommandOptions) {
        crate::object::special_power_module::SpecialPowerModuleInterface::do_special_power(
            self,
            command_options,
        );
    }

    fn do_special_power_at_object(
        &mut self,
        object_id: ObjectId,
        command_options: SpecialPowerCommandOptions,
    ) {
        crate::object::special_power_module::SpecialPowerModuleInterface::do_special_power_at_object(
            self,
            object_id,
            command_options,
        );
    }

    fn do_special_power_at_location(
        &mut self,
        location: &Coord3D,
        angle: f32,
        command_options: SpecialPowerCommandOptions,
    ) {
        crate::object::special_power_module::SpecialPowerModuleInterface::do_special_power_at_location(
            self,
            location,
            angle,
            command_options,
        );
    }

    fn do_special_power_using_waypoints(
        &mut self,
        waypoint: &Waypoint,
        command_options: SpecialPowerCommandOptions,
    ) {
        crate::object::special_power_module::SpecialPowerModuleInterface::do_special_power_using_waypoints(
            self,
            waypoint,
            command_options,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ocl_special_power_creation() {
        let data = OclSpecialPowerModuleData::default();
        let power = OclSpecialPower::new(0, 1, Arc::new(data));

        assert!(power.is_ready());
    }

    #[test]
    fn test_find_ocl_default() {
        let mut data = OclSpecialPowerModuleData::default();
        let ocl = Arc::new(ObjectCreationList::new());
        data.default_ocl = Some(ocl.clone());

        let power = OclSpecialPower::new(0, 1, Arc::new(data));
        let found = power.find_ocl();

        assert!(found.is_some());
        assert!(Arc::ptr_eq(&found.unwrap(), &ocl));
    }

    #[test]
    fn test_create_location_types() {
        let data = OclSpecialPowerModuleData {
            create_loc: OclCreateLocType::CreateAboveLocation,
            ..Default::default()
        };

        assert_eq!(data.create_loc, OclCreateLocType::CreateAboveLocation);
    }
}
