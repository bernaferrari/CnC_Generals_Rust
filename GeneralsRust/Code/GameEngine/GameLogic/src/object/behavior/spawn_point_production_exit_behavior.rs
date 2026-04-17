//! SpawnPointProductionExitUpdate behavior.
//!
//! Matches C++ SpawnPointProductionExitUpdate.cpp/.h.

use crate::ai::THE_AI;
use crate::common::*;
use crate::helpers::{TheGameLogic, TheTerrainLogic};
use crate::modules::{
    BehaviorModule, BehaviorModuleInterface, ExitDoorType as ModuleExitDoorType,
    ExitInterface as ModuleExitInterface, UpdateModuleInterface, UpdateSleepTime,
    UPDATE_SLEEP_FOREVER,
};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData, ModuleInterfaceType, NameKeyType, Thing as ModuleThing,
};
use glam::EulerRot;
use std::any::Any;
use std::sync::{Arc, RwLock};

const MAX_SPAWN_POINTS: usize = 10;

/// Module data for SpawnPointProductionExitUpdate.
#[derive(Debug, Clone)]
pub struct SpawnPointProductionExitModuleData {
    pub base: BehaviorModuleData,
    pub spawn_point_bone_name: AsciiString,
}

impl SpawnPointProductionExitModuleData {
    pub fn new() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            spawn_point_bone_name: AsciiString::new(),
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SPAWN_POINT_PRODUCTION_EXIT_FIELDS)
    }
}

impl Default for SpawnPointProductionExitModuleData {
    fn default() -> Self {
        Self::new()
    }
}

crate::impl_behavior_module_data_via_base!(SpawnPointProductionExitModuleData, base);

/// SpawnPointProductionExitUpdate behavior.
#[derive(Debug)]
pub struct SpawnPointProductionExitBehavior {
    data: SpawnPointProductionExitModuleData,
    owner_id: ObjectID,
    bones_initialized: bool,
    spawn_point_count: usize,
    world_coord_spawn_points: [Coord3D; MAX_SPAWN_POINTS],
    world_angle_spawn_points: [Real; MAX_SPAWN_POINTS],
    spawn_point_occupier: [ObjectID; MAX_SPAWN_POINTS],
}

impl SpawnPointProductionExitBehavior {
    pub fn new(data: SpawnPointProductionExitModuleData, owner_id: ObjectID) -> Self {
        Self {
            data,
            owner_id,
            bones_initialized: false,
            spawn_point_count: 0,
            world_coord_spawn_points: [Coord3D::new(0.0, 0.0, 0.0); MAX_SPAWN_POINTS],
            world_angle_spawn_points: [0.0; MAX_SPAWN_POINTS],
            spawn_point_occupier: [INVALID_ID; MAX_SPAWN_POINTS],
        }
    }

    pub fn from_module_thing(
        thing: Arc<dyn ModuleThing>,
        module_data: Arc<SpawnPointProductionExitModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let module_object = thing.as_object().ok_or_else(|| {
            "SpawnPointProductionExitUpdate requires an owning object".to_string()
        })?;
        Ok(Self::new(
            module_data.as_ref().clone(),
            module_object.get_object_id(),
        ))
    }

    fn initialize_bone_positions(&mut self) {
        let Some(owner_arc) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return;
        };
        let Ok(owner_guard) = owner_arc.read() else {
            return;
        };
        let Some(drawable) = owner_guard.get_drawable() else {
            return;
        };
        let Ok(drawable_guard) = drawable.read() else {
            return;
        };

        let bone_name = self.data.spawn_point_bone_name.as_str();
        let transforms =
            drawable_guard.get_pristine_bone_transforms(bone_name, 1, MAX_SPAWN_POINTS);
        if transforms.is_empty() {
            return;
        }

        self.spawn_point_count = transforms.len().min(MAX_SPAWN_POINTS);
        for (index, transform) in transforms.iter().enumerate().take(self.spawn_point_count) {
            let (_, rotation, translation) = transform.to_scale_rotation_translation();
            let (_, _, yaw) = rotation.to_euler(EulerRot::XYZ);
            self.world_coord_spawn_points[index] = Coord3D::new(translation.x, translation.y, 0.0);
            self.world_angle_spawn_points[index] = yaw;
        }

        self.bones_initialized = true;
    }

    fn revalidate_occupiers(&mut self) {
        for index in 0..self.spawn_point_count {
            let id = self.spawn_point_occupier[index];
            if id == INVALID_ID {
                continue;
            }
            if TheGameLogic::find_object_by_id(id).is_none() {
                self.spawn_point_occupier[index] = INVALID_ID;
            }
        }
    }

    fn exit_object_via_door_internal(
        &mut self,
        new_obj: &Arc<RwLock<Object>>,
        door: ModuleExitDoorType,
    ) -> Result<(), String> {
        if matches!(
            door,
            ModuleExitDoorType::None | ModuleExitDoorType::NoneAvailable
        ) {
            return Ok(());
        }

        if !self.bones_initialized {
            self.initialize_bone_positions();
        }
        if !self.bones_initialized {
            return Ok(());
        }

        self.revalidate_occupiers();

        let mut position_index = None;
        for index in 0..self.spawn_point_count {
            if self.spawn_point_occupier[index] == INVALID_ID {
                position_index = Some(index);
                break;
            }
        }

        let Some(position_index) = position_index else {
            return Ok(());
        };

        let Some(owner_arc) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return Ok(());
        };
        let Ok(owner_guard) = owner_arc.read() else {
            return Ok(());
        };
        let layer = owner_guard.get_layer();
        drop(owner_guard);

        let mut create_point = self.world_coord_spawn_points[position_index];
        if let Some(terrain) = TheTerrainLogic::get() {
            create_point.z = terrain.get_layer_height(create_point.x, create_point.y, layer);
        }
        let create_angle = self.world_angle_spawn_points[position_index];

        {
            let mut guard = new_obj.write().map_err(|_| "Failed to lock new object")?;
            let obj_id = guard.get_id();
            self.spawn_point_occupier[position_index] = obj_id;
            guard.set_position(&create_point)?;
            guard.set_orientation(create_angle)?;
            guard.set_layer(layer);
            guard.set_disabled(DisabledType::Held);
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

        Ok(())
    }
}

impl UpdateModuleInterface for SpawnPointProductionExitBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        Ok(UPDATE_SLEEP_FOREVER)
    }
}

impl BehaviorModuleInterface for SpawnPointProductionExitBehavior {
    fn get_module_name(&self) -> &'static str {
        "SpawnPointProductionExitUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_update_exit_interface(&mut self) -> Option<&mut dyn ModuleExitInterface> {
        Some(self)
    }
}

impl ModuleExitInterface for SpawnPointProductionExitBehavior {
    fn can_exit(&self, _object_id: ObjectID) -> bool {
        true
    }

    fn exit(&mut self, _object_id: ObjectID) -> bool {
        true
    }

    fn reserve_door_for_exit(
        &mut self,
        _spawner: Option<&Object>,
        _spawn: Option<&Object>,
    ) -> ModuleExitDoorType {
        if !self.bones_initialized {
            self.initialize_bone_positions();
        }
        if !self.bones_initialized || self.spawn_point_count == 0 {
            return ModuleExitDoorType::NoneAvailable;
        }

        self.revalidate_occupiers();
        for index in 0..self.spawn_point_count {
            if self.spawn_point_occupier[index] == INVALID_ID {
                return ModuleExitDoorType::Primary;
            }
        }

        ModuleExitDoorType::NoneAvailable
    }

    fn unreserve_door_for_exit(&mut self, _door: ModuleExitDoorType) {}

    fn exit_object_via_door(
        &mut self,
        obj: &Arc<RwLock<Object>>,
        door: ModuleExitDoorType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.exit_object_via_door_internal(obj, door)?;
        Ok(())
    }
}

/// Glue that exposes SpawnPointProductionExitBehavior through the common Module trait.
pub struct SpawnPointProductionExitBehaviorModule {
    behavior: SpawnPointProductionExitBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<SpawnPointProductionExitModuleData>,
}

impl SpawnPointProductionExitBehaviorModule {
    pub fn new(
        behavior: SpawnPointProductionExitBehavior,
        module_name: &AsciiString,
        module_data: Arc<SpawnPointProductionExitModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut SpawnPointProductionExitBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for SpawnPointProductionExitBehaviorModule {
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

impl Module for SpawnPointProductionExitBehaviorModule {

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

fn parse_spawn_point_bone_name(
    _ini: &mut INI,
    data: &mut SpawnPointProductionExitModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.spawn_point_bone_name = AsciiString::from(token);
    Ok(())
}

const SPAWN_POINT_PRODUCTION_EXIT_FIELDS: &[FieldParse<SpawnPointProductionExitModuleData>] =
    &[FieldParse {
        token: "SpawnPointBoneName",
        parse: parse_spawn_point_bone_name,
    }];

impl Snapshotable for SpawnPointProductionExitBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.data.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1).map_err(|err| {
            format!("SpawnPointProductionExitBehavior::xfer version failed: {err}")
        })?;

        self.data.xfer(xfer)?;
        for id in &mut self.spawn_point_occupier {
            xfer.xfer_object_id(id).map_err(|err| {
                format!("SpawnPointProductionExitBehavior::xfer spawn_point_occupier failed: {err}")
            })?;
        }
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
    fn parse_spawn_point_bone_name_ignores_equals() {
        let mut data = SpawnPointProductionExitModuleData::default();
        let mut ini = INI::new();
        parse_spawn_point_bone_name(&mut ini, &mut data, &["=", "DockStart"])
            .expect("spawn point bone parse should succeed");
        assert_eq!(data.spawn_point_bone_name.as_str(), "DockStart");
    }
}
