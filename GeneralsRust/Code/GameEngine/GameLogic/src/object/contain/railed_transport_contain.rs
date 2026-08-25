//! Railed Transport Contain Module
//!
//! C++ `RailedTransportContain`: transit lock via dock open state, reopen the
//! dock when empty, and unload through `RailedTransportDockUpdateInterface`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

use super::{ContainerIniParse, ContainerInterface};
use crate::common::{GameResult, ObjectID, PlayerMaskType};
use crate::damage::DamageInfo;
use crate::helpers::TheGameLogic;
use crate::modules::{ContainModuleInterface, ContainWant, ExitDoorType, UpdateSleepTime};
use crate::object::Object;
use crate::object::contain::{ObjectTemplate, TransportContain};
use crate::player::Player;
use game_engine::common::ini::{INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};

/// Configuration data for RailedTransportContain module
#[derive(Debug, Clone, Default)]
pub struct RailedTransportContainModuleData {
    /// Configuration from parent TransportContain
    pub base: super::TransportContainModuleData,
}

impl RailedTransportContainModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)
    }

    pub fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        self.base.parse_from_config(config)
    }
}

impl ContainerIniParse for RailedTransportContainModuleData {
    fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        RailedTransportContainModuleData::parse_from_config(self, config)
    }
}

/// Railed transport contain module - for rail-based transport
#[derive(Debug)]
pub struct RailedTransportContain {
    /// Base functionality from TransportContain
    pub base: TransportContain,
    /// Reference to the owning object
    object_id: ObjectID,
}

impl RailedTransportContain {
    /// Create a new RailedTransportContain module
    pub fn new(
        object: Weak<RwLock<Object>>,
        module_data: &RailedTransportContainModuleData,
    ) -> GameResult<Self> {
        let mut base = TransportContain::new(object.clone(), &module_data.base)?;
        // C++ RailedTransportContain::isSpecificRiderFreeToExit: dock closed = in transit.
        base.set_require_open_dock_to_exit(true);

        Ok(Self {
            base,
            object_id: object
                .upgrade()
                .and_then(|arc| arc.read().ok().map(|g| g.get_id()))
                .unwrap_or(crate::common::INVALID_ID),
        })
    }

    fn with_owner_object<R>(&self, f: impl FnOnce(&Object) -> R) -> Option<R> {
        if self.object_id == crate::common::INVALID_ID {
            return None;
        }
        crate::object::registry::OBJECT_REGISTRY.with_object(self.object_id, f)
    }

    fn with_owner_object_mut<R>(&self, f: impl FnOnce(&mut Object) -> R) -> Option<R> {
        if self.object_id == crate::common::INVALID_ID {
            return None;
        }
        crate::object::registry::OBJECT_REGISTRY.with_object_mut(self.object_id, f)
    }

    /// C++ RailedTransportContain::onRemoving: reopen dock when empty.
    pub fn on_removing(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.base.on_removing(obj_id)?;
        if self.base.base.get_contain_count() == 0 {
            let _ = self.with_owner_object_mut(|owner| {
                owner.with_dock_update_interface(|dock| {
                    dock.set_dock_open(true);
                })
            });
        }
        Ok(())
    }

    /// C++ RailedTransportContain::exitObjectViaDoor: unload via rail dock.
    pub fn exit_object_via_door(
        &mut self,
        obj_id: ObjectID,
        _exit_door: ExitDoorType,
    ) -> GameResult<()> {
        let unloaded = self
            .with_owner_object_mut(|owner| {
                owner.with_railed_transport_dock_update_interface(|rtdui| {
                    rtdui.unload_single_object(obj_id);
                })
            })
            .flatten()
            .is_some();
        if !unloaded {
            return Ok(());
        }
        Ok(())
    }

    /// Serialize state for save/load
    pub fn save_state(&self) -> GameResult<HashMap<String, Vec<u8>>> {
        self.base.save_state()
    }

    /// Deserialize state for save/load
    pub fn load_state(&mut self, state: &HashMap<String, Vec<u8>>) -> GameResult<()> {
        self.base.load_state(state)
    }
}

impl ContainModuleInterface for RailedTransportContain {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        if let Some(obj) = TheGameLogic::find_object_by_id(object_id) {
            if let Ok(obj_guard) = obj.read() {
                return self.base.is_valid_container_for(&*obj_guard, true);
            }
        }
        false
    }

    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.base
            .add_to_contain(object_id)
            .map_err(|e| e.to_string())
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.base
            .remove_from_contain(object_id, false)
            .map_err(|e| e.to_string())
    }

    fn get_contained_objects(&self) -> &[ObjectID] {
        ContainModuleInterface::get_contained_objects(&self.base)
    }

    fn get_contained_count(&self) -> usize {
        ContainModuleInterface::get_contained_count(&self.base)
    }

    fn get_player_who_entered(&self) -> PlayerMaskType {
        self.base.get_player_who_entered()
    }

    fn get_max_capacity(&self) -> usize {
        let max = self.base.get_contain_max();
        if max < 0 { usize::MAX } else { max as usize }
    }

    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        self.base.update().map_err(|e| e.into())
    }

    fn on_damage(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_damage(damage_info).map_err(|e| e.into())
    }

    fn on_die(
        &mut self,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_die(damage_info).map_err(|e| e.into())
    }

    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        self.base.is_valid_container_for(obj, check_capacity)
    }

    fn add_to_contain(
        &mut self,
        obj: &Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain_object(obj.get_id()).map_err(|e| e.into())
    }

    fn enable_load_sounds(
        &mut self,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.base.enable_load_sounds(enabled);
        Ok(())
    }

    fn on_object_wants_to_enter_or_exit(
        &mut self,
        obj: &Object,
        want: ContainWant,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .on_object_wants_to_enter_or_exit(obj, want)
            .map_err(|e| e.into())
    }

    fn on_capture(
        &mut self,
        owner: &Object,
        old_owner: Option<&Arc<RwLock<Player>>>,
        new_owner: Option<&Arc<RwLock<Player>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .on_capture(owner, old_owner, new_owner)
            .map_err(|e| e.into())
    }

    fn can_exit(&self, object_id: ObjectID) -> bool {
        if !self.get_contained_objects().contains(&object_id) {
            return false;
        }
        let Some(obj) = TheGameLogic::find_object_by_id(object_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(object_id))
        else {
            return false;
        };
        obj.read()
            .ok()
            .and_then(|guard| {
                self.base
                    .reserve_door_for_exit(&ObjectTemplate {}, &*guard)
                    .ok()
            })
            .map(|door| !matches!(door, ExitDoorType::None | ExitDoorType::NoneAvailable))
            .unwrap_or(false)
    }

    fn reserve_door_for_exit(
        &mut self,
        _spawner: Option<&Object>,
        spawn: Option<&Object>,
    ) -> ExitDoorType {
        let Some(obj) = spawn else {
            return ExitDoorType::Primary;
        };
        match self.base.reserve_door_for_exit(&ObjectTemplate {}, obj) {
            Ok(door) => door,
            Err(_) => ExitDoorType::NoneAvailable,
        }
    }

    fn exit_object_via_door(
        &mut self,
        obj_id: ObjectID,
        door: ExitDoorType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        RailedTransportContain::exit_object_via_door(self, obj_id, door).map_err(|e| e.into())
    }

    fn on_removing(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        RailedTransportContain::on_removing(self, obj_id).map_err(|e| e.into())
    }

    fn passes_weapon_bonus_to_passengers(&self) -> bool {
        self.base.passes_weapon_bonus_to_passengers()
    }

    fn set_passenger_allowed_to_fire(&mut self, allowed: bool) {
        self.base.set_passenger_allowed_to_fire(allowed);
    }

    fn harm_and_force_exit_all_contained(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .harm_and_force_exit_all_contained(damage_info)
            .map_err(|e| e.into())
    }

    fn kill_all_contained(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.kill_all_contained().map_err(|e| e.into())
    }

    fn process_damage_to_contained(&mut self, percent_damage: f32) {
        let _ = self.base.process_damage_to_contained(percent_damage);
    }

    fn snapshot_crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::crc(self, xfer)
    }

    fn snapshot_xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn snapshot_load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(self)
    }

    fn on_selling(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_selling().map_err(|e| e.into())
    }
}

impl Snapshotable for RailedTransportContain {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::crc(&self.base, xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Snapshotable::xfer(&mut self.base, xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.base)
    }
}

impl ContainerInterface for RailedTransportContain {
    fn can_contain(&self, obj: &Object) -> bool {
        ContainerInterface::can_contain(&self.base, obj)
    }

    fn add_object(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.base.add_object(obj_id)
    }

    fn remove_object(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.base.remove_object(obj_id)
    }

    fn get_usage(&self) -> (u32, u32) {
        self.base.get_usage()
    }
}
