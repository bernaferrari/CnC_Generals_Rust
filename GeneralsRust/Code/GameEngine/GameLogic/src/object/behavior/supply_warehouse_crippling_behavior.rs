//! SupplyWarehouseCripplingBehavior - Rust conversion of C++ SupplyWarehouseCripplingBehavior
//!
//! Behavior that Disables the building on ReallyDamaged edge state, and manages an Update timer to heal
//! Original Author: Graham Smallwood, September 2002
//! Rust conversion: 2025

use crate::common::{ModuleData, ObjectID, Real, UnsignedInt, XferVersion};
use std::sync::{Arc, RwLock, Weak};

// Forward declarations
use crate::common::TheGameLogic;
use crate::damage::{BodyDamageType, DamageInfo, BODY_REALLYDAMAGED};
use crate::modules::{
    BehaviorModuleInterface, BodyModuleInterface, DamageModuleInterface, DockUpdateInterface,
    UpdateModuleInterface, UpdateSleepTime, MODULEINTERFACE_DAMAGE, MODULEINTERFACE_UPDATE,
    UPDATE_SLEEP_FOREVER,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::Object;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::AsciiString;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::any::Any;

/// Module data for SupplyWarehouseCripplingBehavior
#[derive(Debug, Clone)]
pub struct SupplyWarehouseCripplingBehaviorModuleData {
    pub base: BehaviorModuleData,
    /// Time since last damage until I can start to heal
    pub self_heal_suppression: UnsignedInt,
    /// Once I am okay to heal, how often to do so
    pub self_heal_delay: UnsignedInt,
    /// And how much to heal
    pub self_heal_amount: Real,
}

impl SupplyWarehouseCripplingBehaviorModuleData {
    pub fn new() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            self_heal_suppression: 0,
            self_heal_delay: 0,
            self_heal_amount: 0.0,
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SUPPLY_WAREHOUSE_CRIPPLING_FIELDS)
    }
}

impl Default for SupplyWarehouseCripplingBehaviorModuleData {
    fn default() -> Self {
        Self::new()
    }
}

crate::impl_behavior_module_data_via_base!(SupplyWarehouseCripplingBehaviorModuleData, base);

fn parse_self_heal_suppression(
    _ini: &mut INI,
    data: &mut SupplyWarehouseCripplingBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.self_heal_suppression = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_self_heal_delay(
    _ini: &mut INI,
    data: &mut SupplyWarehouseCripplingBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.self_heal_delay = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_self_heal_amount(
    _ini: &mut INI,
    data: &mut SupplyWarehouseCripplingBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.self_heal_amount = INI::parse_real(token)?;
    Ok(())
}

const SUPPLY_WAREHOUSE_CRIPPLING_FIELDS: &[FieldParse<
    SupplyWarehouseCripplingBehaviorModuleData,
>] = &[
    FieldParse {
        token: "SelfHealSupression",
        parse: parse_self_heal_suppression,
    },
    FieldParse {
        token: "SelfHealDelay",
        parse: parse_self_heal_delay,
    },
    FieldParse {
        token: "SelfHealAmount",
        parse: parse_self_heal_amount,
    },
];

/// Main SupplyWarehouseCripplingBehavior implementation
#[derive(Debug)]
pub struct SupplyWarehouseCripplingBehavior {
    // Base module data
    object: Weak<RwLock<Object>>,
    module_data: Arc<SupplyWarehouseCripplingBehaviorModuleData>,

    // State tracking
    next_call_frame_and_phase: UnsignedInt,
    healing_suppressed_until_frame: UnsignedInt,
    next_healing_frame: UnsignedInt,
}

impl SupplyWarehouseCripplingBehavior {
    pub fn new(
        thing: Arc<RwLock<Object>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = {
            let data_ref = module_data
                .as_any()
                .downcast_ref::<SupplyWarehouseCripplingBehaviorModuleData>()
                .ok_or("Invalid module data type")?;
            data_ref.clone()
        };

        if let Ok(obj_guard) = thing.read() {
            TheGameLogic::set_wake_frame(obj_guard.get_id(), UPDATE_SLEEP_FOREVER);
        }

        Ok(Self {
            object: Arc::downgrade(&thing),
            module_data: Arc::new(data),
            next_call_frame_and_phase: 0,
            healing_suppressed_until_frame: 0,
            next_healing_frame: 0,
        })
    }

    fn get_object(&self) -> Result<Arc<RwLock<Object>>, Box<dyn std::error::Error + Send + Sync>> {
        self.object.upgrade().ok_or_else(|| "Object not set".into())
    }

    /// Reset our ability to heal timer, as we took damage
    fn reset_self_heal_suppression(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let data = &self.module_data;
        let now = TheGameLogic::get_frame();

        self.healing_suppressed_until_frame = now + data.self_heal_suppression;
        self.next_healing_frame = self.healing_suppressed_until_frame;

        Ok(())
    }

    /// Disable our object (when crippled)
    fn start_crippled_effects(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let object = self.get_object()?;
        let obj_guard = object.read().map_err(|_| "Failed to read object")?;

        obj_guard.with_dock_update_interface(|dock| {
            let _ = dock.set_dock_crippled(true);
        });

        drop(obj_guard);
        Ok(())
    }

    /// Enable our object (when healed from crippled state)
    fn stop_crippled_effects(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let object = self.get_object()?;
        let obj_guard = object.read().map_err(|_| "Failed to read object")?;

        obj_guard.with_dock_update_interface(|dock| {
            let _ = dock.set_dock_crippled(false);
        });

        drop(obj_guard);
        Ok(())
    }
}

impl UpdateModuleInterface for SupplyWarehouseCripplingBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        // Suppression is handled by sleeping the module, so if we're here, it's time to heal
        let data = &self.module_data;
        let object = self.get_object()?;
        let now = TheGameLogic::get_frame();

        self.next_healing_frame = now + data.self_heal_delay;

        // Attempt healing
        {
            let mut obj_guard = object.write().map_err(|_| "Failed to write object")?;
            obj_guard.attempt_healing(data.self_heal_amount, None)?;
            drop(obj_guard);
        }

        // Check if we're at full health
        let obj_guard = object.read().map_err(|_| "Failed to read object")?;
        let is_at_full_health = if let Some(body) = obj_guard.get_body_module() {
            let body_guard = body.lock().map_err(|_| "Failed to lock body module")?;
            let current_health = body_guard.get_health();
            let max_health = body_guard.get_max_health();
            drop(body_guard);
            current_health >= max_health
        } else {
            false
        };
        drop(obj_guard);

        if is_at_full_health {
            // Sleep forever if at full health - can't heal anymore
            return Ok(UpdateSleepTime::Forever);
        }

        // Delay between heals is also handled by sleeping the module
        Ok(UpdateSleepTime::from_u32(
            self.next_healing_frame.saturating_sub(now),
        ))
    }
}

impl DamageModuleInterface for SupplyWarehouseCripplingBehavior {
    fn receive_damage(&mut self, _object_id: ObjectID, _damage: &DamageInfo) -> Real {
        0.0
    }

    /// Damage has been dealt, this is an opportunity to react to that damage
    fn on_damage(
        &mut self,
        _damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let now = TheGameLogic::get_frame();

        self.reset_self_heal_suppression()?;

        // We got hit, time to get up for work after a quick snooze
        let sleep_time = self.healing_suppressed_until_frame.saturating_sub(now);
        if let Some(obj) = self.object.upgrade() {
            if let Ok(obj_guard) = obj.read() {
                TheGameLogic::set_wake_frame(
                    obj_guard.get_id(),
                    UpdateSleepTime::from_u32(sleep_time),
                );
            }
        }

        Ok(())
    }

    fn on_healing(
        &mut self,
        _damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // No special handling for healing events
        Ok(())
    }

    fn on_body_damage_state_change(
        &mut self,
        _damage_info: &DamageInfo,
        old_state: BodyDamageType,
        new_state: BodyDamageType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if new_state == BODY_REALLYDAMAGED {
            self.start_crippled_effects()?;
        } else if old_state == BODY_REALLYDAMAGED && new_state != BODY_REALLYDAMAGED {
            self.stop_crippled_effects()?;
        }

        Ok(())
    }
}

impl BehaviorModuleInterface for SupplyWarehouseCripplingBehavior {
    fn get_interface_mask() -> u32 {
        MODULEINTERFACE_UPDATE | MODULEINTERFACE_DAMAGE
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_damage(&mut self) -> Option<&mut dyn DamageModuleInterface> {
        Some(self)
    }
}

/// Module wrapper for SupplyWarehouseCripplingBehavior.
pub struct SupplyWarehouseCripplingBehaviorModule {
    behavior: SupplyWarehouseCripplingBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<SupplyWarehouseCripplingBehaviorModuleData>,
}

impl SupplyWarehouseCripplingBehaviorModule {
    pub fn new(
        behavior: SupplyWarehouseCripplingBehavior,
        module_name: &AsciiString,
        module_data: Arc<SupplyWarehouseCripplingBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut SupplyWarehouseCripplingBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for SupplyWarehouseCripplingBehaviorModule {
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

impl Module for SupplyWarehouseCripplingBehaviorModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }
}

impl Snapshotable for SupplyWarehouseCripplingBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut next_call_frame_and_phase = self.next_call_frame_and_phase;
        xfer_update_module_base_state(xfer, &mut next_call_frame_and_phase)
            .map_err(|e| format!("Failed to xfer update module base state: {}", e))?;

        let mut healing_suppressed_until_frame = self.healing_suppressed_until_frame;
        xfer.xfer_unsigned_int(&mut healing_suppressed_until_frame)
            .map_err(|e| format!("Failed to xfer healing_suppressed_until_frame: {:?}", e))?;
        let mut next_healing_frame = self.next_healing_frame;
        xfer.xfer_unsigned_int(&mut next_healing_frame)
            .map_err(|e| format!("Failed to xfer next_healing_frame: {:?}", e))?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)
            .map_err(|e| format!("Failed to xfer update module base state: {}", e))?;

        xfer.xfer_unsigned_int(&mut self.healing_suppressed_until_frame)
            .map_err(|e| format!("Failed to xfer healing_suppressed_until_frame: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.next_healing_frame)
            .map_err(|e| format!("Failed to xfer next_healing_frame: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

// Thread safety
unsafe impl Send for SupplyWarehouseCripplingBehavior {}
unsafe impl Sync for SupplyWarehouseCripplingBehavior {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_data_creation() {
        let data = SupplyWarehouseCripplingBehaviorModuleData::new();
        assert_eq!(data.self_heal_suppression, 0);
        assert_eq!(data.self_heal_delay, 0);
        assert_eq!(data.self_heal_amount, 0.0);
    }

    #[test]
    fn test_module_data_default() {
        let data = SupplyWarehouseCripplingBehaviorModuleData::default();
        assert_eq!(data.self_heal_suppression, 0);
        assert_eq!(data.self_heal_delay, 0);
        assert_eq!(data.self_heal_amount, 0.0);
    }
}
