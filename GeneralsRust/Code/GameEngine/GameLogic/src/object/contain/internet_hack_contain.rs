//! Internet Hack Contain Module
//!
//! Specialized container for internet hacking functionality

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

use super::{ContainerIniParse, ContainerInterface};
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{GameResult, ObjectID, PlayerMaskType};
use crate::damage::DamageInfo;
use crate::helpers::TheGameLogic;
use crate::modules::{ContainModuleInterface, ContainWant, UpdateSleepTime};
use crate::object::contain::TransportContain;
use crate::object::Object;
use game_engine::common::ini::{INIError, INI};

/// Configuration data for InternetHackContain module
#[derive(Debug, Clone, Default)]
pub struct InternetHackContainModuleData {
    /// Configuration from parent TransportContain
    pub base: super::TransportContainModuleData,
    // Hack-specific parameters would go here
}

impl InternetHackContainModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)
    }

    pub fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        self.base.parse_from_config(config)
    }
}

impl ContainerIniParse for InternetHackContainModuleData {
    fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        InternetHackContainModuleData::parse_from_config(self, config)
    }
}

/// Internet hack contain module
#[derive(Debug)]
pub struct InternetHackContain {
    /// Base functionality from TransportContain
    pub base: TransportContain,
    /// Reference to the owning object
    #[allow(dead_code)]
    object_id: ObjectID,
}

impl InternetHackContain {
    /// Create a new InternetHackContain module
    pub fn new(
        object: Weak<RwLock<Object>>,
        module_data: &InternetHackContainModuleData,
    ) -> GameResult<Self> {
        let base = TransportContain::new(object.clone(), &module_data.base)?;

        Ok(Self {
            base,
            object_id: object
                .upgrade()
                .and_then(|arc| arc.read().ok().map(|g| g.get_id()))
                .unwrap_or(crate::common::INVALID_ID),
        })
    }

    pub fn add_to_contain(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.base.add_to_contain(
            obj.read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID),
        )?;
        self.on_containing(obj.read().map(|g| g.get_id()).unwrap_or(0))?;
        Ok(())
    }

    fn on_containing(&mut self, obj_id: ObjectID) -> GameResult<()> {
        let Some(rider) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        if let Ok(rider_guard) = rider.read() {
            if let Some(ai) = rider_guard.get_ai() {
                if let Ok(mut ai_guard) = ai.lock() {
                    let params = AiCommandParams::new(
                        AiCommandType::HackInternet,
                        CommandSourceType::FromAi,
                    );
                    let _ = ai_guard.execute_command(&params);
                }
            }
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

impl ContainModuleInterface for InternetHackContain {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        if let Some(obj) = TheGameLogic::find_object_by_id(object_id) {
            if let Ok(obj_guard) = obj.read() {
                return self.base.is_valid_container_for(&*obj_guard, true);
            }
        }
        false
    }

    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        let obj = TheGameLogic::find_object_by_id(object_id)
            .ok_or_else(|| format!("Contain object {} not found", object_id))?;
        self.add_to_contain(obj).map_err(|e| e.to_string())
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        let obj = match TheGameLogic::find_object_by_id(object_id) {
            Some(obj) => obj,
            None => return Ok(()),
        };
        self.base
            .remove_from_contain(
                obj.read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID),
                false,
            )
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
        if max < 0 {
            usize::MAX
        } else {
            max as usize
        }
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
        old_owner: Option<&Arc<RwLock<crate::player::Player>>>,
        new_owner: Option<&Arc<RwLock<crate::player::Player>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .on_capture(owner, old_owner, new_owner)
            .map_err(|e| e.into())
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
}

impl ContainerInterface for InternetHackContain {
    fn can_contain(&self, obj: &Object) -> bool {
        self.base.is_valid_container_for(obj, true)
    }

    fn add_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.add_to_contain(obj)
    }

    fn remove_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.base.remove_from_contain(
            obj.read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID),
            false,
        )
    }

    fn get_usage(&self) -> (u32, u32) {
        let current = self.base.get_contain_count();
        let max = match self.base.get_contain_max() {
            super::CONTAIN_MAX_UNKNOWN => u32::MAX,
            value if value < 0 => u32::MAX,
            value => value as u32,
        };
        (current, max)
    }
}
