//! Port of `GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BridgeTowerBehavior.cpp`.
//!
//! BridgeTowerBehavior - Rust conversion of C++ BridgeTowerBehavior
//!
//! Behavior module for bridge towers that mirrors combat events across the span.
//! Author: Colin Day, July 2002 (C++ version)
//! Rust conversion: 2025

use std::any::Any;
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::common::xfer::XferExt;
use crate::common::{AsciiString, BehaviorModuleData, Int, ObjectID, Real, XferVersion};
use crate::damage::{BodyDamageType, DamageInfo};
use crate::modules::{BehaviorModuleInterface, DamageModuleInterface, DieModuleInterface};
use crate::object::{
    registry::OBJECT_REGISTRY, Object as GameObject, INVALID_ID as OBJECT_INVALID_ID,
};
use game_engine::common::ini::{INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::Snapshotable;
use game_engine::common::thing::module::{
    BridgeTowerControlInterface, Module as EngineModule, ModuleData as EngineModuleData,
    NameKeyType, Thing as ModuleThing,
};
use game_engine::system::Xfer as EngineXfer;

use super::behavior_module::{
    xfer_behavior_module_base_versions, BridgeTowerBehaviorInterface, BridgeTowerType,
};

const BRIDGE_MAX_TOWERS: usize = 4;

/// BridgeTowerBehaviorModuleData - configuration container for bridge towers.
#[derive(Debug, Clone)]
pub struct BridgeTowerBehaviorModuleData {
    pub base: BehaviorModuleData,
}

impl BridgeTowerBehaviorModuleData {
    pub fn new() -> Self {
        Self {
            base: BehaviorModuleData::new(),
        }
    }

    pub fn parse_from_ini(&mut self, _ini: &mut INI) -> Result<(), INIError> {
        // Tower behavior exposes no custom INI fields in the original implementation.
        Ok(())
    }
}

impl Default for BridgeTowerBehaviorModuleData {
    fn default() -> Self {
        Self::new()
    }
}

crate::impl_behavior_module_data_via_base!(BridgeTowerBehaviorModuleData, base);

/// BridgeTowerBehavior - mirrors tower damage/healing back to the owning bridge.
pub struct BridgeTowerBehavior {
    pub module_data: Arc<BridgeTowerBehaviorModuleData>,
    object_id: ObjectID,
    object_handle: Mutex<Option<Weak<RwLock<GameObject>>>>,
    bridge_id: ObjectID,
    tower_type: BridgeTowerType,
}

impl BridgeTowerBehavior {
    fn construct_with_object_id(
        object_id: ObjectID,
        module_data: Arc<BridgeTowerBehaviorModuleData>,
        initial_object: Option<Arc<RwLock<GameObject>>>,
    ) -> Self {
        let initial_handle = initial_object
            .as_ref()
            .map(|object| Arc::downgrade(object))
            .or_else(|| {
                if object_id == OBJECT_INVALID_ID {
                    None
                } else {
                    OBJECT_REGISTRY
                        .get_object(object_id)
                        .map(|obj| Arc::downgrade(&obj))
                }
            });

        Self {
            module_data,
            object_id,
            object_handle: Mutex::new(initial_handle),
            bridge_id: OBJECT_INVALID_ID,
            tower_type: BridgeTowerType::North,
        }
    }

    pub fn new_from_object_handle(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<BridgeTowerBehaviorModuleData>,
    ) -> Self {
        let object_id = object
            .read()
            .map(|guard| guard.get_id())
            .unwrap_or(OBJECT_INVALID_ID);

        Self::construct_with_object_id(object_id, module_data, Some(object))
    }

    pub fn from_module_thing(
        thing: Arc<dyn ModuleThing>,
        module_data: Arc<BridgeTowerBehaviorModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let module_object = thing
            .as_object()
            .ok_or_else(|| "BridgeTowerBehavior requires an owning object".to_string())?;

        let object_id = module_object.get_object_id();
        let object = OBJECT_REGISTRY
            .get_object(object_id)
            .ok_or_else(|| format!("BridgeTowerBehavior requires object {object_id} to exist"))?;

        Ok(Self::new_from_object_handle(object, module_data))
    }

    fn get_object(
        &self,
    ) -> Result<Arc<RwLock<GameObject>>, Box<dyn std::error::Error + Send + Sync>> {
        if self.object_id == OBJECT_INVALID_ID {
            return Err("BridgeTowerBehavior missing owning object id".into());
        }

        if let Ok(mut handle) = self.object_handle.lock() {
            if let Some(weak) = handle.as_ref() {
                if let Some(object) = weak.upgrade() {
                    return Ok(object);
                }
            }

            if let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) {
                *handle = Some(Arc::downgrade(&object));
                return Ok(object);
            }
        }

        Err(format!(
            "BridgeTowerBehavior unable to upgrade handle for object {}",
            self.object_id
        )
        .into())
    }

    fn get_bridge_object(&self) -> Option<Arc<RwLock<GameObject>>> {
        if self.bridge_id == OBJECT_INVALID_ID {
            None
        } else {
            OBJECT_REGISTRY.get_object(self.bridge_id)
        }
    }

    fn tower_type_from_index(index: usize) -> Option<BridgeTowerType> {
        match index {
            0 => Some(BridgeTowerType::North),
            1 => Some(BridgeTowerType::South),
            2 => Some(BridgeTowerType::East),
            3 => Some(BridgeTowerType::West),
            _ => None,
        }
    }

    fn collect_tower_ids(
        &self,
        bridge_object: &Arc<RwLock<GameObject>>,
    ) -> Result<Vec<(BridgeTowerType, ObjectID)>, Box<dyn std::error::Error + Send + Sync>> {
        let bridge_read = bridge_object
            .read()
            .map_err(|e| format!("bridge lock poisoned: {}", e))?;
        let mut ids: Option<[ObjectID; BRIDGE_MAX_TOWERS]> = None;

        for handle in bridge_read.behavior_modules() {
            handle.with_module(|module| {
                if let Some(bridge) = module.get_bridge_control_interface() {
                    ids = Some(bridge.tower_ids());
                }
            });
            if ids.is_some() {
                break;
            }
        }

        let tower_ids = ids
            .unwrap_or([OBJECT_INVALID_ID; BRIDGE_MAX_TOWERS])
            .iter()
            .enumerate()
            .filter_map(|(index, id)| Self::tower_type_from_index(index).map(|ty| (ty, *id)))
            .collect();

        Ok(tower_ids)
    }

    fn propagate_damage(
        &self,
        damage_info: &DamageInfo,
        damage_percentage: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if damage_percentage <= 0.0 {
            return Ok(());
        }

        let bridge_object = match self.get_bridge_object() {
            Some(obj) => obj,
            None => return Ok(()),
        };

        let tower_ids = self.collect_tower_ids(&bridge_object)?;
        let source_id = damage_info.input.source_id;
        if source_id == self.bridge_id || tower_ids.iter().any(|(_, id)| *id == source_id) {
            return Ok(());
        }

        for (tower_type, tower_id) in &tower_ids {
            if *tower_type == self.tower_type || *tower_id == OBJECT_INVALID_ID {
                continue;
            }

            if let Some(tower_object) = OBJECT_REGISTRY.get_object(*tower_id) {
                let mut tower_write = tower_object
                    .write()
                    .map_err(|e| format!("tower lock poisoned: {}", e))?;
                if let Some(body) = tower_write.get_body_module() {
                    let body_guard = body
                        .lock()
                        .map_err(|_| "BridgeTowerBehavior: body lock poisoned")?;
                    let max_health = body_guard.get_max_health();
                    if max_health > 0.0 {
                        let mut propagated = DamageInfo::new();
                        propagated.input.source_id = self.object_id;
                        propagated.input.damage_type = damage_info.input.damage_type;
                        propagated.input.damage_status_type = damage_info.input.damage_status_type;
                        propagated.input.damage_fx_override = damage_info.input.damage_fx_override;
                        propagated.input.death_type = damage_info.input.death_type;
                        propagated.input.amount = damage_percentage * max_health;
                        propagated.sync_from_input();
                        tower_write.attempt_damage(&mut propagated)?;
                    }
                }
            }
        }

        let mut bridge_write = bridge_object
            .write()
            .map_err(|e| format!("bridge lock poisoned: {}", e))?;
        if let Some(body) = bridge_write.get_body_module() {
            let body_guard = body
                .lock()
                .map_err(|_| "BridgeTowerBehavior: body lock poisoned")?;
            let max_health = body_guard.get_max_health();
            if max_health > 0.0 {
                let mut bridge_damage = DamageInfo::new();
                bridge_damage.input.source_id = self.object_id;
                bridge_damage.input.damage_type = damage_info.input.damage_type;
                bridge_damage.input.damage_status_type = damage_info.input.damage_status_type;
                bridge_damage.input.damage_fx_override = damage_info.input.damage_fx_override;
                bridge_damage.input.death_type = damage_info.input.death_type;
                bridge_damage.input.amount = damage_percentage * max_health;
                bridge_damage.sync_from_input();
                bridge_write.attempt_damage(&mut bridge_damage)?;
            }
        }

        Ok(())
    }

    fn propagate_healing(
        &self,
        damage_info: &DamageInfo,
        healing_percentage: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if healing_percentage <= 0.0 {
            return Ok(());
        }

        let bridge_object = match self.get_bridge_object() {
            Some(obj) => obj,
            None => return Ok(()),
        };

        let tower_ids = self.collect_tower_ids(&bridge_object)?;
        let source_id = damage_info.input.source_id;
        if source_id == self.bridge_id || tower_ids.iter().any(|(_, id)| *id == source_id) {
            return Ok(());
        }

        let source_object = self.get_object()?;
        let source_guard = source_object
            .read()
            .map_err(|e| format!("source lock poisoned: {}", e))?;

        for (tower_type, tower_id) in &tower_ids {
            if *tower_type == self.tower_type || *tower_id == OBJECT_INVALID_ID {
                continue;
            }

            if let Some(tower_object) = OBJECT_REGISTRY.get_object(*tower_id) {
                let mut tower_write = tower_object
                    .write()
                    .map_err(|e| format!("tower lock poisoned: {}", e))?;
                if let Some(body) = tower_write.get_body_module() {
                    let body_guard = body
                        .lock()
                        .map_err(|_| "BridgeTowerBehavior: body lock poisoned")?;
                    let max_health = body_guard.get_max_health();
                    if max_health > 0.0 {
                        let amount = healing_percentage * max_health;
                        tower_write.attempt_healing(amount, Some(&*source_guard))?;
                    }
                }
            }
        }

        let mut bridge_write = bridge_object
            .write()
            .map_err(|e| format!("bridge lock poisoned: {}", e))?;
        if let Some(body) = bridge_write.get_body_module() {
            let body_guard = body
                .lock()
                .map_err(|_| "BridgeTowerBehavior: body lock poisoned")?;
            let max_health = body_guard.get_max_health();
            if max_health > 0.0 {
                let amount = healing_percentage * max_health;
                bridge_write.attempt_healing(amount, Some(&*source_guard))?;
            }
        }

        Ok(())
    }

    pub fn crc(
        &self,
        xfer: &mut dyn EngineXfer,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        xfer_behavior_module_base_versions(xfer)?;

        let mut bridge_id = self.bridge_id;
        xfer.xfer_object_id(&mut bridge_id)?;

        let mut tower_kind = self.tower_type as u32;
        xfer.xfer_unsigned_int(&mut tower_kind)?;

        Ok(())
    }

    pub fn xfer(
        &mut self,
        xfer: &mut dyn EngineXfer,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        xfer_behavior_module_base_versions(xfer)?;

        xfer.xfer_object_id(&mut self.bridge_id)?;

        let mut tower_kind = self.tower_type as u32;
        xfer.xfer_unsigned_int(&mut tower_kind)?;
        self.tower_type = match tower_kind {
            0 => BridgeTowerType::North,
            1 => BridgeTowerType::South,
            2 => BridgeTowerType::East,
            3 => BridgeTowerType::West,
            _ => BridgeTowerType::North,
        };

        Ok(())
    }

    pub fn load_post_process(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    pub fn get_interface_mask() -> Int {
        (crate::modules::MODULEINTERFACE_DAMAGE | crate::modules::MODULEINTERFACE_DIE) as Int
    }
}

impl BridgeTowerBehaviorInterface for BridgeTowerBehavior {
    fn set_bridge(&mut self, bridge: Option<Arc<RwLock<GameObject>>>) {
        self.bridge_id = bridge
            .map(|b| {
                b.read()
                    .map(|guard| guard.get_id())
                    .unwrap_or(OBJECT_INVALID_ID)
            })
            .unwrap_or(OBJECT_INVALID_ID);
    }

    fn get_bridge_id(&self) -> ObjectID {
        self.bridge_id
    }

    fn set_tower_type(&mut self, tower_type: BridgeTowerType) {
        self.tower_type = tower_type;
    }
}

impl BridgeTowerControlInterface for BridgeTowerBehavior {
    fn bridge_id(&self) -> ObjectID {
        self.bridge_id
    }

    fn set_bridge_id(&mut self, bridge_id: Option<ObjectID>) {
        self.bridge_id = bridge_id.unwrap_or(OBJECT_INVALID_ID);
    }

    fn set_tower_type_index(&mut self, tower_type_index: usize) {
        self.tower_type = match tower_type_index {
            0 => BridgeTowerType::North,
            1 => BridgeTowerType::South,
            2 => BridgeTowerType::East,
            3 => BridgeTowerType::West,
            _ => BridgeTowerType::North,
        };
    }
}

impl DamageModuleInterface for BridgeTowerBehavior {
    fn receive_damage(&mut self, _object_id: ObjectID, _damage: &DamageInfo) -> Real {
        0.0
    }

    fn on_damage(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let me = self.get_object()?;
        let me_read = me
            .read()
            .map_err(|e| format!("tower lock poisoned: {}", e))?;
        let damage_percentage = if let Some(body) = me_read.get_body_module() {
            let body_guard = body
                .lock()
                .map_err(|_| "BridgeTowerBehavior: body lock poisoned")?;
            let max_health = body_guard.get_max_health();
            if max_health > 0.0 {
                damage_info.input.amount / max_health
            } else {
                0.0
            }
        } else {
            0.0
        };
        drop(me_read);

        self.propagate_damage(damage_info, damage_percentage)
    }

    fn on_healing(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let me = self.get_object()?;
        let me_read = me
            .read()
            .map_err(|e| format!("tower lock poisoned: {}", e))?;
        let healing_percentage = if let Some(body) = me_read.get_body_module() {
            let max_health = {
                let body_guard = body
                    .lock()
                    .map_err(|_| "BridgeTowerBehavior: body lock poisoned")?;
                body_guard.get_max_health()
            };
            if max_health > 0.0 {
                damage_info.input.amount / max_health
            } else {
                0.0
            }
        } else {
            0.0
        };
        drop(me_read);

        self.propagate_healing(damage_info, healing_percentage)
    }

    fn on_body_damage_state_change(
        &mut self,
        _damage_info: &DamageInfo,
        _old_state: BodyDamageType,
        _new_state: BodyDamageType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

impl DieModuleInterface for BridgeTowerBehavior {
    fn on_die(
        &mut self,
        _damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(bridge_object) = self.get_bridge_object() {
            if let Ok(mut bridge_write) = bridge_object.write() {
                bridge_write.kill(None, None);
            } else {
                return Err("BridgeTowerBehavior: bridge lock poisoned".into());
            }
        }
        Ok(())
    }
}

impl BehaviorModuleInterface for BridgeTowerBehavior {
    fn get_bridge_tower_behavior_interface(
        &mut self,
    ) -> Option<&mut dyn BridgeTowerBehaviorInterface> {
        Some(self)
    }
}

/// Glue that binds the behavior to the module factory infrastructure.
pub struct BridgeTowerBehaviorModule {
    behavior: BridgeTowerBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<BridgeTowerBehaviorModuleData>,
}

impl BridgeTowerBehaviorModule {
    pub fn new(
        behavior: BridgeTowerBehavior,
        module_name: &AsciiString,
        module_data: Arc<BridgeTowerBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior(&self) -> &BridgeTowerBehavior {
        &self.behavior
    }

    pub fn behavior_mut(&mut self) -> &mut BridgeTowerBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for BridgeTowerBehaviorModule {
    fn crc(&self, xfer: &mut dyn EngineXfer) -> Result<(), String> {
        self.behavior.crc(xfer).map_err(|err| err.to_string())
    }

    fn xfer(&mut self, xfer: &mut dyn EngineXfer) -> Result<(), String> {
        self.behavior.xfer(xfer).map_err(|err| err.to_string())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior
            .load_post_process()
            .map_err(|err| err.to_string())
    }
}

impl EngineModule for BridgeTowerBehaviorModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }

    fn get_bridge_tower_control_interface(
        &mut self,
    ) -> Option<&mut dyn BridgeTowerControlInterface> {
        Some(&mut self.behavior)
    }

    fn on_object_created(&mut self) {}

    fn on_delete(&mut self) {}
}

unsafe impl Send for BridgeTowerBehavior {}
unsafe impl Sync for BridgeTowerBehavior {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn bridge_tower_behavior_constructs_with_defaults() {
        let data = Arc::new(BridgeTowerBehaviorModuleData::default());
        let behavior = BridgeTowerBehavior::construct_with_object_id(
            OBJECT_INVALID_ID,
            Arc::clone(&data),
            None,
        );
        assert_eq!(behavior.bridge_id, OBJECT_INVALID_ID);
        assert_eq!(behavior.tower_type, BridgeTowerType::North);
    }
}
