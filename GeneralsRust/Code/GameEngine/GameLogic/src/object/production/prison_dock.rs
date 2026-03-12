#![allow(unexpected_cfgs)]
/*
** Command & Conquer Generals Zero Hour(tm)
** Copyright 2025 Electronic Arts Inc.
**
** This program is free software: you can redistribute it and/or modify
** it under the terms of the GNU General Public License as published by
** the Free Software Foundation, either version 3 of the License, or
** (at your option) any later version.
**
** This program is distributed in the hope that it will be useful,
** but WITHOUT ANY WARRANTY; without even the implied warranty of
** MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
** GNU General Public License for more details.
**
** You should have received a copy of the GNU General Public License
** along with this program.  If not, see <http://www.gnu.org/licenses/>.
*/

//! Prison Dock Update Module
//!
//! Original C++ location: GameLogic/Module/PrisonDockUpdate.h/.cpp
//! Original C++ Author: Colin Day, August 2002
//! Rust conversion: 2025

use crate::common::*;
use crate::modules::{BehaviorModule, BehaviorModuleInterface, DockUpdateInterface};
use crate::object::Object;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData};
use std::sync::{Arc, RwLock};

/// Prison dock configuration data (no extra fields beyond DockUpdate)
#[derive(Debug, Clone)]
pub struct PrisonDockUpdateData {
    pub base: super::DockUpdateData,
}

impl Default for PrisonDockUpdateData {
    fn default() -> Self {
        Self {
            base: super::DockUpdateData::default(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(PrisonDockUpdateData, base);

#[cfg(feature = "allow_surrender")]
#[derive(Debug)]
pub struct PrisonDockUpdate {
    base: super::DockUpdate,
}

#[cfg(feature = "allow_surrender")]
impl PrisonDockUpdate {
    pub fn new(data: PrisonDockUpdateData, owner_id: ObjectID, owner_position: &Coord3D) -> Self {
        let base = super::DockUpdate::new(data.base.clone(), owner_id, owner_position);
        Self { base }
    }
}

#[cfg(feature = "allow_surrender")]
impl BehaviorModuleInterface for PrisonDockUpdate {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.update()
    }

    fn get_module_name(&self) -> &str {
        "PrisonDockUpdate"
    }

    fn get_interface_mask() -> u32 {
        0x00000004
    }

    fn get_dock_update_interface(&mut self) -> Option<&mut dyn DockUpdateInterface> {
        Some(self)
    }
}

#[cfg(feature = "allow_surrender")]
impl BehaviorModule for PrisonDockUpdate {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.init()
    }

    fn on_destroy(&mut self) {
        self.base.on_destroy();
    }
}

#[cfg(feature = "allow_surrender")]
impl DockUpdateInterface for PrisonDockUpdate {
    fn is_dock_open(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base.is_dock_open()
    }

    fn set_dock_open(&mut self, open: Bool) {
        self.base.set_dock_open(open);
    }

    fn cancel_dock(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.cancel_dock(obj)
    }

    fn reserve_approach_position(
        &mut self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
        approach_pos: &mut i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .reserve_approach_position(obj, goal_pos, approach_pos)
    }

    fn advance_approach_position(
        &mut self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
        approach_pos: &mut i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .advance_approach_position(obj, goal_pos, approach_pos)
    }

    fn is_clear_to_advance(
        &self,
        obj: &Arc<RwLock<Object>>,
        approach_position: i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base.is_clear_to_advance(obj, approach_position)
    }

    fn on_approach_reached(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_approach_reached(obj)
    }

    fn is_clear_to_enter(
        &self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base.is_clear_to_enter(obj)
    }

    fn get_enter_position(
        &self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.get_enter_position(obj, goal_pos)
    }

    fn on_enter_reached(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_enter_reached(obj)
    }

    fn get_dock_position(
        &self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.get_dock_position(obj, goal_pos)
    }

    fn on_dock_reached(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_dock_reached(obj)
    }

    fn action(
        &mut self,
        obj: &Arc<RwLock<Object>>,
        _drone: Option<&Arc<RwLock<Object>>>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let docker_guard = obj.read().map_err(|_| "Failed to lock docker")?;
        if let Some(contain) = docker_guard.get_contain() {
            if let Ok(contain_guard) = contain.lock() {
                if contain_guard.get_contained_count() == 0 {
                    return Ok(false);
                }
            }
        }

        let ai = docker_guard.get_ai_update_interface().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                "PrisonDockUpdate requires POW truck AI",
            )
        })?;
        drop(docker_guard);

        let Some(prison) = TheGameLogic::find_object_by_id(self.base.owner_id) else {
            return Ok(false);
        };

        let mut ai_guard = ai.lock().map_err(|_| "Failed to lock POW truck AI")?;
        let pow_ai = ai_guard
            .get_pow_truck_ai_update_interface()
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::Other, "POW truck AI interface missing")
            })?;
        pow_ai.unload_prisoners_to_prison(&prison);

        Ok(false)
    }

    fn get_exit_position(
        &self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.get_exit_position(obj, goal_pos)
    }

    fn on_exit_reached(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_exit_reached(obj)
    }

    fn is_allow_passthrough_type(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base.is_allow_passthrough_type()
    }

    fn is_rally_point_after_dock_type(
        &self,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base.is_rally_point_after_dock_type()
    }

    fn set_dock_crippled(
        &mut self,
        crippled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.set_dock_crippled(crippled)
    }
}

// Stub implementation when surrender feature is not enabled
#[cfg(not(feature = "allow_surrender"))]
#[derive(Debug)]
pub struct PrisonDockUpdate {
    _phantom: std::marker::PhantomData<()>,
}

#[cfg(not(feature = "allow_surrender"))]
impl PrisonDockUpdate {
    pub fn new(
        _data: PrisonDockUpdateData,
        _owner_id: ObjectID,
        _owner_position: &Coord3D,
    ) -> Self {
        log::warn!("PrisonDockUpdate is not available - ALLOW_SURRENDER feature not enabled");
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

/// Glue that exposes PrisonDockUpdate through the common Module trait.
pub struct PrisonDockUpdateModule {
    behavior: PrisonDockUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<PrisonDockUpdateData>,
}

impl PrisonDockUpdateModule {
    pub fn new(
        behavior: PrisonDockUpdate,
        module_name: &AsciiString,
        module_data: Arc<PrisonDockUpdateData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior(&self) -> &PrisonDockUpdate {
        &self.behavior
    }

    pub fn behavior_mut(&mut self) -> &mut PrisonDockUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for PrisonDockUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.module_data.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Arc::make_mut(&mut self.module_data).xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Arc::make_mut(&mut self.module_data).load_post_process()
    }
}

impl Module for PrisonDockUpdateModule {
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
        game_engine::common::thing::module::ModuleData::get_module_tag_name_key(
            self.module_data.as_ref(),
        )
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prison_dock_data_defaults() {
        let data = PrisonDockUpdateData::default();
        let _ = data.base;
    }
}
