//! DefaultProductionExitUpdate behavior.
//!
//! Matches C++ DefaultProductionExitUpdate.cpp/.h.

use crate::ai::THE_AI;
use crate::common::*;
use crate::helpers::TheTerrainLogic;
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModule, BehaviorModuleInterface,
    ExitDoorType as ModuleExitDoorType, ExitInterface as ModuleExitInterface,
    UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_FOREVER,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::Object;
use crate::path::PATHFIND_CELL_SIZE_F;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData, ModuleInterfaceType, Thing as ModuleThing,
};
use std::any::Any;
use std::sync::{Arc, RwLock};

/// Module data for DefaultProductionExitUpdate.
#[derive(Debug, Clone)]
pub struct DefaultProductionExitModuleData {
    pub base: BehaviorModuleData,
    pub unit_create_point: Coord3D,
    pub natural_rally_point: Coord3D,
    pub use_spawn_rally_point: Bool,
}

impl DefaultProductionExitModuleData {
    pub fn new() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            unit_create_point: Coord3D::new(0.0, 0.0, 0.0),
            natural_rally_point: Coord3D::new(0.0, 0.0, 0.0),
            use_spawn_rally_point: false,
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, DEFAULT_PRODUCTION_EXIT_FIELDS)
    }
}

impl Default for DefaultProductionExitModuleData {
    fn default() -> Self {
        Self::new()
    }
}

crate::impl_behavior_module_data_via_base!(DefaultProductionExitModuleData, base);

/// DefaultProductionExitUpdate behavior.
#[derive(Debug)]
pub struct DefaultProductionExitBehavior {
    data: DefaultProductionExitModuleData,
    owner_id: ObjectID,
    next_call_frame_and_phase: UnsignedInt,
    rally_point: Coord3D,
    rally_point_exists: bool,
}

impl DefaultProductionExitBehavior {
    pub fn new(data: DefaultProductionExitModuleData, owner_id: ObjectID) -> Self {
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
        module_data: Arc<DefaultProductionExitModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let module_object = thing
            .as_object()
            .ok_or_else(|| "DefaultProductionExitUpdate requires an owning object".to_string())?;
        Ok(Self::new(
            module_data.as_ref().clone(),
            module_object.get_object_id(),
        ))
    }

    pub fn set_rally_point(&mut self, pos: Coord3D) {
        self.rally_point = pos;
        self.rally_point_exists = true;
    }

    pub fn use_spawn_rally_point(&self) -> bool {
        self.data.use_spawn_rally_point
    }

    fn transform_point(&self, local: &Coord3D, transform: &Matrix3D) -> Coord3D {
        transform.transform_point3(*local)
    }

    fn get_natural_rally_point(&self, transform: &Matrix3D, offset: bool) -> Coord3D {
        let mut p = self.data.natural_rally_point;
        if offset {
            let mut offset_vec = p;
            let len = (offset_vec.x * offset_vec.x + offset_vec.y * offset_vec.y).sqrt();
            if len > 0.001 {
                offset_vec.x /= len;
                offset_vec.y /= len;
                offset_vec.x *= 2.0 * PATHFIND_CELL_SIZE_F;
                offset_vec.y *= 2.0 * PATHFIND_CELL_SIZE_F;
                p.x += offset_vec.x;
                p.y += offset_vec.y;
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

        let Some(owner_arc) = crate::helpers::TheGameLogic::find_object_by_id(self.owner_id) else {
            return Ok(());
        };
        let Ok(owner_guard) = owner_arc.read() else {
            return Ok(());
        };

        let transform = owner_guard.get_transform_matrix();
        let exit_angle = owner_guard.get_orientation();
        let layer = owner_guard.get_layer();
        drop(owner_guard);

        let mut create_point = self.transform_point(&self.data.unit_create_point, &transform);
        if let Some(terrain) = TheTerrainLogic::get() {
            create_point.z = terrain.get_layer_height(create_point.x, create_point.y, layer);
        }

        {
            let mut guard = new_obj.write().map_err(|_| "Failed to lock new object")?;
            guard.set_position(&create_point)?;
            guard.set_orientation(exit_angle)?;
            guard.set_layer(layer);
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
        let natural_rally = self.get_natural_rally_point(&transform, false);
        exit_path.push(natural_rally);

        if let Ok(guard) = new_obj.read() {
            if let Some(ai) = guard.get_ai_update_interface() {
                if self.rally_point_exists {
                    if let Ok(mut ai_guard) = ai.lock() {
                        if ai_guard.is_doing_ground_movement() {
                            let mut rally = self.rally_point;
                            if ai_guard.adjust_destination(&mut rally) {
                                exit_path.push(rally);
                            }
                        }
                    }
                }
                ai.ai_follow_exit_production_path(
                    &exit_path,
                    Some(self.owner_id),
                    CommandSourceType::FromAi,
                );
            }
        }

        Ok(())
    }
}

impl UpdateModuleInterface for DefaultProductionExitBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        Ok(UPDATE_SLEEP_FOREVER)
    }
}

impl BehaviorModuleInterface for DefaultProductionExitBehavior {
    fn get_module_name(&self) -> &'static str {
        "DefaultProductionExitUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_update_exit_interface(&mut self) -> Option<&mut dyn ModuleExitInterface> {
        Some(self)
    }
}

impl ModuleExitInterface for DefaultProductionExitBehavior {
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

/// Glue that exposes DefaultProductionExitBehavior through the common Module trait.
pub struct DefaultProductionExitBehaviorModule {
    behavior: DefaultProductionExitBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<DefaultProductionExitModuleData>,
}

impl DefaultProductionExitBehaviorModule {
    pub fn new(
        behavior: DefaultProductionExitBehavior,
        module_name: &AsciiString,
        module_data: Arc<DefaultProductionExitModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut DefaultProductionExitBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for DefaultProductionExitBehaviorModule {
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

impl Module for DefaultProductionExitBehaviorModule {
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
    data: &mut DefaultProductionExitModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.unit_create_point = parse_coord3d(tokens)?;
    Ok(())
}

fn parse_natural_rally_point(
    _ini: &mut INI,
    data: &mut DefaultProductionExitModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.natural_rally_point = parse_coord3d(tokens)?;
    Ok(())
}

fn parse_use_spawn_rally_point(
    _ini: &mut INI,
    data: &mut DefaultProductionExitModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.use_spawn_rally_point = INI::parse_bool(token)?;
    Ok(())
}

const DEFAULT_PRODUCTION_EXIT_FIELDS: &[FieldParse<DefaultProductionExitModuleData>] = &[
    FieldParse {
        token: "UnitCreatePoint",
        parse: parse_unit_create_point,
    },
    FieldParse {
        token: "NaturalRallyPoint",
        parse: parse_natural_rally_point,
    },
    FieldParse {
        token: "UseSpawnRallyPoint",
        parse: parse_use_spawn_rally_point,
    },
];

impl Snapshotable for DefaultProductionExitBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.data.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| format!("DefaultProductionExitBehavior::xfer version failed: {err}"))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        xfer.xfer_coord3d(&mut self.rally_point);
        xfer.xfer_bool(&mut self.rally_point_exists)
            .map_err(|err| {
                format!("DefaultProductionExitBehavior::xfer rally_point_exists failed: {err}")
            })?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.data.load_post_process()
    }
}
