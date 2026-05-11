//! SupplyCenterProductionExitUpdate behavior.
//!
//! Matches C++ SupplyCenterProductionExitUpdate.cpp/.h.

use crate::ai::THE_AI;
use crate::common::*;
use crate::helpers::{TheGameLogic, TheTerrainLogic};
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModule, BehaviorModuleInterface,
    ExitDoorType as ModuleExitDoorType, ExitInterface as ModuleExitInterface,
    SupplyTruckAIInterface, UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_FOREVER,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::path::PATHFIND_CELL_SIZE_F;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData, ModuleInterfaceType, NameKeyType, Thing as ModuleThing,
};
use std::any::Any;
use std::sync::{Arc, RwLock};

/// Module data for SupplyCenterProductionExitUpdate.
#[derive(Debug, Clone)]
pub struct SupplyCenterProductionExitModuleData {
    pub base: BehaviorModuleData,
    pub unit_create_point: Coord3D,
    pub natural_rally_point: Coord3D,
    pub grant_temporary_stealth_frames: UnsignedInt,
}

impl SupplyCenterProductionExitModuleData {
    pub fn new() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            unit_create_point: Coord3D::new(0.0, 0.0, 0.0),
            natural_rally_point: Coord3D::new(0.0, 0.0, 0.0),
            grant_temporary_stealth_frames: 0,
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SUPPLY_CENTER_PRODUCTION_EXIT_FIELDS)
    }
}

impl Default for SupplyCenterProductionExitModuleData {
    fn default() -> Self {
        Self::new()
    }
}

crate::impl_behavior_module_data_via_base!(SupplyCenterProductionExitModuleData, base);

/// SupplyCenterProductionExitUpdate behavior.
#[derive(Debug)]
pub struct SupplyCenterProductionExitBehavior {
    data: SupplyCenterProductionExitModuleData,
    owner_id: ObjectID,
    next_call_frame_and_phase: UnsignedInt,
    rally_point: Coord3D,
    rally_point_exists: bool,
}

impl SupplyCenterProductionExitBehavior {
    pub fn new(data: SupplyCenterProductionExitModuleData, owner_id: ObjectID) -> Self {
        Self {
            data,
            owner_id,
            next_call_frame_and_phase: 0,
            rally_point: Coord3D::new(0.0, 0.0, 0.0),
            rally_point_exists: false,
        }
    }

    pub fn from_module_thing(
        thing: Arc<dyn ModuleThing>,
        module_data: Arc<SupplyCenterProductionExitModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let module_object = thing.as_object().ok_or_else(|| {
            "SupplyCenterProductionExitUpdate requires an owning object".to_string()
        })?;
        Ok(Self::new(
            module_data.as_ref().clone(),
            module_object.get_object_id(),
        ))
    }

    pub fn set_rally_point(&mut self, pos: Coord3D) {
        self.rally_point = pos;
        self.rally_point_exists = true;
    }

    fn transform_point(&self, local: &Coord3D, transform: &Matrix3D) -> Coord3D {
        transform.transform_point3(*local)
    }

    fn get_natural_rally_point(&self, transform: &Matrix3D, offset: bool) -> Coord3D {
        let mut p = self.data.natural_rally_point;
        if offset {
            let mut offset_vec = p;
            let len = (offset_vec.x * offset_vec.x
                + offset_vec.y * offset_vec.y
                + offset_vec.z * offset_vec.z)
                .sqrt();
            if len > 0.001 {
                offset_vec.x /= len;
                offset_vec.y /= len;
                offset_vec.z /= len;
                offset_vec.x *= 2.0 * PATHFIND_CELL_SIZE_F;
                offset_vec.y *= 2.0 * PATHFIND_CELL_SIZE_F;
                offset_vec.z *= 2.0 * PATHFIND_CELL_SIZE_F;
                p.x += offset_vec.x;
                p.y += offset_vec.y;
                p.z += offset_vec.z;
            }
        }
        self.transform_point(&p, transform)
    }

    fn exit_object_via_door_internal(
        &self,
        new_obj: &Arc<RwLock<Object>>,
        door: ModuleExitDoorType,
    ) -> Result<(), String> {
        if matches!(
            door,
            ModuleExitDoorType::None | ModuleExitDoorType::NoneAvailable
        ) {
            return Ok(());
        }

        let Some(owner_arc) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return Ok(());
        };
        let Ok(owner_guard) = owner_arc.read() else {
            return Ok(());
        };

        let transform = owner_guard.get_transform_matrix();
        let exit_angle = owner_guard.get_orientation();
        let owner_stealthed = owner_guard.test_status(ObjectStatusTypes::Stealthed);
        drop(owner_guard);

        let mut create_point = self.transform_point(&self.data.unit_create_point, &transform);
        if let Some(terrain) = TheTerrainLogic::get() {
            create_point.z = terrain.get_ground_height(create_point.x, create_point.y, None);
        }

        {
            let mut guard = new_obj.write().map_err(|_| "Failed to lock new object")?;
            guard.set_position(&create_point)?;
            guard.set_orientation(exit_angle)?;
        }

        if let Ok(ai_guard) = THE_AI.read() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(mut pf) = pathfinder.write() {
                    pf.add_object_to_map(
                        new_obj.read().map(|obj| obj.get_id()).unwrap_or(INVALID_ID),
                        &[create_point],
                        false,
                    );
                }
            }
        }

        let mut exit_path = Vec::new();
        exit_path.push(self.get_natural_rally_point(&transform, false));
        if self.rally_point_exists {
            exit_path.push(self.rally_point);
        }

        if let Ok(guard) = new_obj.read() {
            if let Some(ai) = guard.get_ai_update_interface() {
                ai.ai_follow_exit_production_path(
                    &exit_path,
                    Some(self.owner_id),
                    CommandSourceType::FromAi,
                );

                if let Ok(mut ai_guard) = ai.lock() {
                    if let Some(truck_ai) = ai_guard.get_supply_truck_ai_interface_mut() {
                        truck_ai.set_force_wanting_state(true);
                    }
                }
            }
        }

        if self.data.grant_temporary_stealth_frames > 0 && owner_stealthed {
            if let Ok(guard) = new_obj.write() {
                let can_stealth = guard.test_status(ObjectStatusTypes::CanStealth);
                if let Some(stealth) = guard.get_stealth() {
                    if let Ok(mut stealth_guard) = stealth.lock() {
                        // Match C++: only grant when existing stealth is temporary or when
                        // unit does not already have CAN_STEALTH capability.
                        if stealth_guard.is_temporary_grant() || !can_stealth {
                            let frame = TheGameLogic::get_frame();
                            let _ = stealth_guard.receive_grant(
                                true,
                                self.data.grant_temporary_stealth_frames,
                                frame,
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

impl UpdateModuleInterface for SupplyCenterProductionExitBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        Ok(UPDATE_SLEEP_FOREVER)
    }
}

impl BehaviorModuleInterface for SupplyCenterProductionExitBehavior {
    fn get_module_name(&self) -> &'static str {
        "SupplyCenterProductionExitUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_update_exit_interface(&mut self) -> Option<&mut dyn ModuleExitInterface> {
        Some(self)
    }
}

impl ModuleExitInterface for SupplyCenterProductionExitBehavior {
    fn can_exit(&self, _object_id: ObjectID) -> bool {
        true
    }

    fn exit(&mut self, _object_id: ObjectID) -> bool {
        true
    }

    fn get_rally_point(&self) -> Result<Option<Coord3D>, Box<dyn std::error::Error + Send + Sync>> {
        if self.rally_point_exists {
            Ok(Some(self.rally_point))
        } else {
            Ok(None)
        }
    }

    fn reserve_door_for_exit(
        &mut self,
        _spawner: Option<&Object>,
        _spawn: Option<&Object>,
    ) -> ModuleExitDoorType {
        ModuleExitDoorType::Primary
    }

    fn unreserve_door_for_exit(&mut self, _door: ModuleExitDoorType) {}

    fn exit_object_via_door(
        &mut self,
        obj: &Arc<RwLock<Object>>,
        door: ModuleExitDoorType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.exit_object_via_door_internal(obj, door)
            .map_err(|e| e.into())
    }
}

/// Glue that exposes SupplyCenterProductionExitBehavior through the common Module trait.
pub struct SupplyCenterProductionExitBehaviorModule {
    behavior: SupplyCenterProductionExitBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<SupplyCenterProductionExitModuleData>,
}

impl SupplyCenterProductionExitBehaviorModule {
    pub fn new(
        behavior: SupplyCenterProductionExitBehavior,
        module_name: &AsciiString,
        module_data: Arc<SupplyCenterProductionExitModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut SupplyCenterProductionExitBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for SupplyCenterProductionExitBehaviorModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

impl Module for SupplyCenterProductionExitBehaviorModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        game_engine::common::thing::module::ModuleData::get_module_tag_name_key(
            self.module_data.as_ref(),
        )
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }
}

fn parse_coord3d(tokens: &[&str]) -> Result<Coord3D, INIError> {
    let values: Vec<&str> = tokens.iter().copied().filter(|t| *t != "=").collect();
    if values.len() < 3 {
        return Err(INIError::InvalidData);
    }
    let mut coords = [0.0f32; 3];
    for (idx, token) in values.iter().take(3).enumerate() {
        coords[idx] = INI::parse_real(token)?;
    }
    Ok(Coord3D::new(coords[0], coords[1], coords[2]))
}

fn parse_unit_create_point(
    _ini: &mut INI,
    data: &mut SupplyCenterProductionExitModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.unit_create_point = parse_coord3d(tokens)?;
    Ok(())
}

fn parse_natural_rally_point(
    _ini: &mut INI,
    data: &mut SupplyCenterProductionExitModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.natural_rally_point = parse_coord3d(tokens)?;
    Ok(())
}

fn parse_grant_temporary_stealth(
    _ini: &mut INI,
    data: &mut SupplyCenterProductionExitModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.grant_temporary_stealth_frames = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

const SUPPLY_CENTER_PRODUCTION_EXIT_FIELDS: &[FieldParse<SupplyCenterProductionExitModuleData>] = &[
    FieldParse {
        token: "UnitCreatePoint",
        parse: parse_unit_create_point,
    },
    FieldParse {
        token: "NaturalRallyPoint",
        parse: parse_natural_rally_point,
    },
    FieldParse {
        token: "GrantTemporaryStealth",
        parse: parse_grant_temporary_stealth,
    },
];

impl Snapshotable for SupplyCenterProductionExitBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.data.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1).map_err(|err| {
            format!("SupplyCenterProductionExitBehavior::xfer version failed: {err}")
        })?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        xfer.xfer_coord3d(&mut self.rally_point);
        xfer.xfer_bool(&mut self.rally_point_exists)
            .map_err(|err| {
                format!("SupplyCenterProductionExitBehavior::xfer rally_point_exists failed: {err}")
            })?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.data.load_post_process()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_grant_temporary_stealth_uses_duration_frames() {
        let mut data = SupplyCenterProductionExitModuleData::default();
        let mut ini = INI::new();

        parse_grant_temporary_stealth(&mut ini, &mut data, &["=", "500ms"])
            .expect("duration parse should succeed");
        assert_eq!(data.grant_temporary_stealth_frames, 15);

        parse_grant_temporary_stealth(&mut ini, &mut data, &["=", "1s"])
            .expect("duration parse should succeed");
        assert_eq!(data.grant_temporary_stealth_frames, 30);
    }

    fn assert_near(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.0001,
            "actual {actual} expected {expected}"
        );
    }

    #[test]
    fn natural_rally_offset_normalizes_full_3d_vector() {
        let mut data = SupplyCenterProductionExitModuleData::default();
        data.natural_rally_point = Coord3D::new(3.0, 4.0, 12.0);
        let exit = SupplyCenterProductionExitBehavior::new(data, 1);

        let point = exit.get_natural_rally_point(&Matrix3D::IDENTITY, true);
        let offset = 2.0 * PATHFIND_CELL_SIZE_F / 13.0;

        assert_near(point.x, 3.0 + 3.0 * offset);
        assert_near(point.y, 4.0 + 4.0 * offset);
        assert_near(point.z, 12.0 + 12.0 * offset);
    }
}
