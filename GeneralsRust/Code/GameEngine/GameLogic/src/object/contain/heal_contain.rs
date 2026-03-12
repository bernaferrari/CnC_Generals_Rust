//! Heal Contain Module
//!
//! Objects that are contained inside a heal contain get healed over time.
//! This is used for medical facilities and healing structures.

use std::collections::HashMap;
use std::sync::{Arc, RwLock, Weak};

use super::{ContainerIniParse, ContainerInterface, OpenContain};
use crate::common::{Coord3D, GameResult, ObjectID, PlayerMaskType};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::TheGameLogic;
use crate::modules::{ContainModuleInterface, ContainWant, ExitDoorType, UpdateSleepTime};
use crate::object::Object;
use game_engine::common::ini::{FieldParse, INIError, INI};

/// Configuration data for HealContain module
#[derive(Debug, Clone)]
pub struct HealContainModuleData {
    /// Configuration from parent OpenContain
    pub base: super::OpenContainModuleData,
    /// Time in frames for something to become fully healed
    pub frames_for_full_heal: u32,
}

impl Default for HealContainModuleData {
    fn default() -> Self {
        Self {
            base: Default::default(),
            frames_for_full_heal: 0,
        }
    }
}

impl HealContainModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)?;
        ini.init_from_ini_with_fields_allow_unknown(self, HEAL_CONTAIN_FIELDS)
    }

    pub fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        self.base.parse_from_config(config)?;
        super::parse_with_fields_allow_unknown(config, self, HEAL_CONTAIN_FIELDS)
    }
}

impl ContainerIniParse for HealContainModuleData {
    fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        HealContainModuleData::parse_from_config(self, config)
    }
}

fn parse_time_for_full_heal(
    _ini: &mut INI,
    data: &mut HealContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.frames_for_full_heal = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

const HEAL_CONTAIN_FIELDS: &[FieldParse<HealContainModuleData>] = &[FieldParse {
    token: "TimeForFullHeal",
    parse: parse_time_for_full_heal,
}];

/// Heal contain module - heals contained units over time
#[derive(Debug)]
pub struct HealContain {
    /// Base functionality from OpenContain
    pub base: OpenContain,
    /// Reference to the owning object
    object: Weak<RwLock<Object>>,
    module_data: HealContainModuleData,
}

impl HealContain {
    /// Create a new HealContain module
    pub fn new(
        object: Weak<RwLock<Object>>,
        module_data: &HealContainModuleData,
    ) -> GameResult<Self> {
        let base = OpenContain::new(object.clone(), &module_data.base)?;
        Ok(Self {
            base,
            object,
            module_data: module_data.clone(),
        })
    }

    /// Get the object this module belongs to
    pub fn get_object(&self) -> Option<Arc<RwLock<Object>>> {
        self.object.upgrade()
    }

    /// Update method called once per frame
    pub fn update(&mut self, module_data: &HealContainModuleData) -> GameResult<UpdateSleepTime> {
        // Extend base functionality
        self.base.update()?;

        // Get contained objects list (need to collect to avoid borrow issues)
        let contained_objects: Vec<_> = self
            .base
            .get_contained_items_list()?
            .iter()
            .cloned()
            .collect();

        // Process each contained object for healing
        for obj in contained_objects {
            let done_healing = self.do_heal(obj.clone(), module_data.frames_for_full_heal)?;

            if done_healing {
                // Reserve door for exit
                let obj_clone = obj.clone();
                if let Ok(object) = obj.read() {
                    if let Ok(exit_door) = self
                        .base
                        .reserve_door_for_exit(&super::open_contain::ObjectTemplate {}, &object)
                    {
                        if exit_door != ExitDoorType::NoneAvailable {
                            drop(object); // Release lock before calling exit
                            self.exit_object_via_door(obj_clone, exit_door)?;
                        }
                    }
                }
            }
        }

        Ok(UpdateSleepTime::None)
    }

    /// Check if this is a heal container
    pub fn is_heal_contain(&self) -> bool {
        true // This container only contains units while healing (not a transport!)
    }

    /// Check if this is a tunnel container
    pub fn is_tunnel_contain(&self) -> bool {
        false
    }

    /// Perform healing on a single object for a single frame
    pub fn do_heal(
        &mut self,
        obj: Arc<RwLock<Object>>,
        frames_for_full_heal: u32,
    ) -> GameResult<bool> {
        let mut done_healing = false;

        // Setup healing damage info structure
        let source_id = self
            .get_object()
            .and_then(|o| o.read().ok().map(|guard| guard.get_id()))
            .unwrap_or_default();

        let mut heal_info = DamageInfo::new();
        heal_info.input.damage_type = DamageType::Healing;
        heal_info.input.death_type = DeathType::None;
        heal_info.input.source_id = source_id;
        heal_info.input.amount = 0.0;
        heal_info.sync_from_input();

        // Get current frame and contained frame
        let current_frame = self.get_current_frame();

        if let Ok(object) = obj.read() {
            let contained_by_frame = object.get_contained_by_frame();

            // Get body module for healing
            if let Some(body) = object.get_body_module() {
                if let Ok(body_module) = body.lock() {
                    let max_health = body_module.get_max_health();

                    // Check if we've been contained long enough for full healing
                    if current_frame >= contained_by_frame + frames_for_full_heal {
                        // Set amount to max health to ensure full healing
                        heal_info.input.amount = max_health;
                        heal_info.sync_from_input();

                        // Apply full healing
                        drop(body_module); // Release lock before mutable operation
                        if let Ok(mut body_mut) = body.lock() {
                            body_mut.attempt_healing(&mut heal_info)?;
                        }

                        done_healing = true;
                    } else {
                        // Give incremental healing over time
                        // Calculate healing amount as if object started at 0 health
                        // and would be fully healed at frames_for_full_heal
                        heal_info.input.amount = max_health / frames_for_full_heal as f32;
                        heal_info.sync_from_input();

                        // Apply incremental healing
                        drop(body_module); // Release lock before mutable operation
                        if let Ok(mut body_mut) = body.lock() {
                            body_mut.attempt_healing(&mut heal_info)?;
                        }
                    }
                }
            }
        }

        Ok(done_healing)
    }

    /// Get current frame from game logic singleton (C++ parity path).
    fn get_current_frame(&self) -> u32 {
        TheGameLogic::get_frame()
    }

    /// Exit object via specified door
    fn exit_object_via_door(
        &mut self,
        obj: Arc<RwLock<Object>>,
        door: ExitDoorType,
    ) -> GameResult<()> {
        // Remove object from container
        self.base.remove_from_contain(obj.clone(), false)?;

        // Position object at exit door location
        if let Some(owner_obj) = self.get_object() {
            if let (Ok(owner), Ok(mut exiting_obj)) = (owner_obj.read(), obj.write()) {
                // Calculate exit position based on door type
                let exit_pos = self.calculate_exit_position(&owner, door)?;
                if let Err(err) = exiting_obj.set_position(&exit_pos) {
                    log::warn!(
                        "HealContain::exit_object_via_door failed to place object {}: {}",
                        exiting_obj.get_id(),
                        err
                    );
                }

                // Register in partition manager
                exiting_obj.register_in_partition_manager()?;
                exiting_obj.set_layer(owner.get_layer());

                // Show the object if it was hidden
                if let Some(drawable) = exiting_obj.get_drawable() {
                    if let Ok(mut draw) = drawable.write() {
                        draw.set_drawable_hidden(false)?;
                    }
                }
            }
        }

        self.base.unreserve_door_for_exit(door)?;

        Ok(())
    }

    /// Calculate exit position based on door type
    fn calculate_exit_position(&self, owner: &Object, door: ExitDoorType) -> GameResult<Coord3D> {
        let mut pos = *owner.get_position();
        let (forward_x, forward_y) = owner.get_unit_direction_vector_2d();
        let right_x = -forward_y;
        let right_y = forward_x;

        let owner_radius = owner
            .get_geometry_info()
            .get_bounding_circle_radius()
            .max(6.0);
        let step = owner_radius + 8.0;

        match door {
            ExitDoorType::Primary => {
                pos.x += forward_x * step;
                pos.y += forward_y * step;
            }
            ExitDoorType::Secondary => {
                pos.x -= forward_x * step;
                pos.y -= forward_y * step;
            }
            ExitDoorType::Emergency => {
                pos.x += right_x * step;
                pos.y += right_y * step;
            }
            _ => {
                pos.x += forward_x * owner_radius;
                pos.y += forward_y * owner_radius;
            }
        }

        Ok(pos)
    }

    /// Serialize state for save/load
    pub fn save_state(&self) -> GameResult<HashMap<String, Vec<u8>>> {
        let mut state = HashMap::new();

        // Save base state
        let base_state = self.base.save_state()?;
        for (key, value) in base_state {
            state.insert(format!("base_{}", key), value);
        }

        // HealContain doesn't have additional state beyond base class

        Ok(state)
    }

    /// Deserialize state for save/load
    pub fn load_state(&mut self, state: &HashMap<String, Vec<u8>>) -> GameResult<()> {
        // Extract base state
        let mut base_state = HashMap::new();
        for (key, value) in state {
            if let Some(base_key) = key.strip_prefix("base_") {
                base_state.insert(base_key.to_string(), value.clone());
            }
        }

        // Load base state
        self.base.load_state(&base_state)?;

        Ok(())
    }

    /// Perform CRC calculation for network synchronization
    pub fn calculate_crc(&self) -> u32 {
        // Implementation would calculate CRC of relevant state
        // For now, delegate to base class
        self.base.calculate_crc()
    }

    /// Post-process after loading from save file
    pub fn load_post_process(&mut self) -> GameResult<()> {
        // Extend base class post-processing
        self.base.load_post_process()?;

        // HealContain doesn't need additional post-processing

        Ok(())
    }
}

impl ContainModuleInterface for HealContain {
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
        self.base.add_to_contain(obj).map_err(|e| e.to_string())
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        let obj = match TheGameLogic::find_object_by_id(object_id) {
            Some(obj) => obj,
            None => return Ok(()),
        };
        self.base
            .remove_from_contain(obj, false)
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
        let module_data = self.module_data.clone();
        HealContain::update(self, &module_data).map_err(|e| e.into())
    }

    fn on_damage(
        &mut self,
        info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_damage(info).map_err(|e| e.into())
    }

    fn on_die(
        &mut self,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_die(damage_info).map_err(|e| e.into())
    }

    fn is_heal_contain(&self) -> bool {
        Self::is_heal_contain(self)
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
        self.base.enable_load_sounds(enabled);
        Ok(())
    }

    fn on_object_wants_to_enter_or_exit(
        &mut self,
        obj: &Object,
        want: ContainWant,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_object_wants_to_enter_or_exit(obj, want);
        Ok(())
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

impl ContainerInterface for HealContain {
    fn can_contain(&self, obj: &Object) -> bool {
        self.base.is_valid_container_for(obj, true)
    }

    fn add_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.base.add_to_contain(obj)
    }

    fn remove_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.base.remove_from_contain(obj, false)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heal_contain_creation() {
        let module_data = HealContainModuleData {
            frames_for_full_heal: 100,
            ..Default::default()
        };

        // Test would create objects and verify healing functionality
        assert_eq!(module_data.frames_for_full_heal, 100);
    }

    #[test]
    fn test_heal_contain_properties() {
        let module_data = HealContainModuleData::default();

        // Create weak reference for deferred object wiring.
        // In real implementation, this would be a proper object reference

        // Test basic properties
        assert_eq!(module_data.frames_for_full_heal, 0);
    }

    #[test]
    fn test_healing_calculation() {
        // Test healing amount calculation
        let max_health = 100.0;
        let frames_for_full_heal = 50;
        let heal_per_frame = max_health / frames_for_full_heal as f32;

        assert_eq!(heal_per_frame, 2.0);
    }
}
