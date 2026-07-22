// FireSpreadUpdate - Update looks for ::Aflame and explicitly ignites someone nearby if set
// Author: Graham Smallwood, April 2002
// Ported to Rust

use crate::common::types::PartitionManagerInterface;
use crate::modules::FlammableUpdateExt;
use crate::object::behavior::behavior_module::xfer_update_module_base_state;
use crate::object::ObjectArcExt;
use crate::prelude::*;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    FireSpreadControlInterface, Module, ModuleData, NameKeyType,
};
use std::sync::Arc;

/// Module data for FireSpreadUpdate
/// Matches C++ FireSpreadUpdate.cpp
#[derive(Debug, Clone)]
pub struct FireSpreadUpdateModuleData {
    pub module_tag_name_key: NameKeyType,
    /// Object creation list for ember effects
    pub ocl_embers: Option<ObjectCreationListId>,
    /// Minimum delay between spread attempts (frames)
    pub min_spread_try_delay: u32,
    /// Maximum delay between spread attempts (frames)
    pub max_spread_try_delay: u32,
    /// Range within which fire can spread
    pub spread_try_range: f32,
}

impl Default for FireSpreadUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            ocl_embers: None,
            min_spread_try_delay: 0,
            max_spread_try_delay: 0,
            spread_try_range: 0.0,
        }
    }
}

impl ModuleData for FireSpreadUpdateModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl Snapshotable for FireSpreadUpdateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl FireSpreadUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, FIRE_SPREAD_UPDATE_FIELDS)
    }
}

fn first_value_token<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_ocl_embers(
    _ini: &mut INI,
    data: &mut FireSpreadUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.ocl_embers = Some(name_key_generate(first_value_token(tokens)?) as ObjectCreationListId);
    Ok(())
}

fn parse_min_spread_delay(
    _ini: &mut INI,
    data: &mut FireSpreadUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.min_spread_try_delay = INI::parse_duration_unsigned_int(first_value_token(tokens)?)?;
    Ok(())
}

fn parse_max_spread_delay(
    _ini: &mut INI,
    data: &mut FireSpreadUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.max_spread_try_delay = INI::parse_duration_unsigned_int(first_value_token(tokens)?)?;
    Ok(())
}

fn parse_spread_try_range(
    _ini: &mut INI,
    data: &mut FireSpreadUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.spread_try_range = INI::parse_real(first_value_token(tokens)?)?;
    Ok(())
}

const FIRE_SPREAD_UPDATE_FIELDS: &[FieldParse<FireSpreadUpdateModuleData>] = &[
    FieldParse {
        token: "OCLEmbers",
        parse: parse_ocl_embers,
    },
    FieldParse {
        token: "MinSpreadDelay",
        parse: parse_min_spread_delay,
    },
    FieldParse {
        token: "MaxSpreadDelay",
        parse: parse_max_spread_delay,
    },
    FieldParse {
        token: "SpreadTryRange",
        parse: parse_spread_try_range,
    },
];

/// FireSpreadUpdate - Spreads fire from burning objects to nearby flammable objects
/// Matches C++ FireSpreadUpdate.cpp:80-156
#[derive(Debug, Clone)]
pub struct FireSpreadUpdate {
    thing: ThingId,
    module_data: FireSpreadUpdateModuleData,
    next_call_frame_and_phase: UnsignedInt,
}

impl FireSpreadUpdate {
    /// Create new FireSpreadUpdate module
    /// Matches C++ FireSpreadUpdate.cpp:80-83
    pub fn new(thing: ThingId, module_data: FireSpreadUpdateModuleData) -> Self {
        Self {
            thing,
            module_data,
            next_call_frame_and_phase: 0,
        }
    }

    /// Update callback - spreads fire to nearby flammable objects
    /// Matches C++ FireSpreadUpdate.cpp:93-135
    pub fn update(&mut self, ctx: &mut UpdateContext<'_>) -> UpdateSleepTime {
        let Some(me) = ctx.game_logic.find_object(self.thing) else {
            return UpdateSleepTime::Forever;
        };

        // Not on fire -- sleep forever (C++ line 98-99)
        if !me.get_status_bits().test(ObjectStatus::Aflame) {
            return UpdateSleepTime::Forever;
        }

        // Create ember effects if configured (C++ line 101)
        if let Some(ocl) = self.module_data.ocl_embers {
            if let Some(ocl_mgr) = ctx.object_creation_list_manager.as_mut() {
                ocl_mgr.create(
                    ocl,
                    Some(me),
                    me.get_position(),
                    me.get_position(),
                    me.get_orientation(),
                );
            }
        }

        // Spread fire explicitly if range is set (C++ line 103-131)
        if self.module_data.spread_try_range != 0.0 {
            // Find closest flammable object (C++ line 106-123)
            let filters = vec![PartitionFilter::Flammable];

            if let Some(object_to_light) = ctx.partition_manager.get_closest_object(
                me,
                self.module_data.spread_try_range,
                PartitionDistanceType::Center3D,
                &filters,
            ) {
                // Try to ignite the found object (C++ line 124-130)
                if let Some(flammable) = object_to_light.find_flammable_update() {
                    flammable.try_to_ignite(ctx);
                }
            }
        }

        // Sleep until next spread attempt (C++ line 133)
        UpdateSleepTime::Frames(self.calc_next_spread_delay())
    }

    pub fn update_simple(&mut self) -> UpdateSleepTime {
        let object_to_light = {
            let Some((aflame, pos, orientation)) =
                crate::object::OBJECT_REGISTRY.with_object(self.thing, |me| {
                    (
                        me.get_status_bits().test(ObjectStatus::Aflame),
                        *me.get_position(),
                        me.get_orientation(),
                    )
                })
            else {
                return UpdateSleepTime::Forever;
            };

            if !aflame {
                return UpdateSleepTime::Forever;
            }

            if let Some(ocl_key) = self.module_data.ocl_embers {
                if let Some(ocl_name) = NameKeyGenerator::key_to_name(ocl_key as NameKeyType) {
                    if let Some(ocl) =
                        crate::helpers::TheObjectCreationListStore::find_object_creation_list(
                            &ocl_name,
                        )
                    {
                        let ctx = crate::object_creation_list::live_creation_context();
                        let _ = crate::object::OBJECT_REGISTRY.with_object(self.thing, |me| {
                            ocl.create_with_angle(&ctx, Some(me), &pos, &pos, orientation, 0)
                        });
                    }
                }
            }

            if self.module_data.spread_try_range != 0.0 {
                let Some(me_arc) = crate::object::OBJECT_REGISTRY.get_object(self.thing) else {
                    return UpdateSleepTime::Forever;
                };
                let Ok(me) = me_arc.read() else {
                    return UpdateSleepTime::Forever;
                };
                let partition = crate::helpers::ThePartitionManagerBridge;
                partition.get_closest_object(
                    &me,
                    self.module_data.spread_try_range,
                    PartitionDistanceType::Center3D,
                    &[PartitionFilter::Flammable],
                )
            } else {
                None
            }
        };

        if let Some(object_to_light) = object_to_light {
            if let Some(flammable) = object_to_light.find_flammable_update() {
                flammable.try_to_ignite_without_context();
            }
        }

        UpdateSleepTime::Frames(self.calc_next_spread_delay())
    }

    /// Start fire spreading behavior
    /// Matches C++ FireSpreadUpdate.cpp:139-145
    pub fn start_fire_spreading(&mut self, ctx: &mut UpdateContext<'_>) {
        let Some(object) = ctx.game_logic.find_object(self.thing) else {
            return;
        };

        // Must be on fire (C++ line 141-142)
        if !object.get_status_bits().test(ObjectStatus::Aflame) {
            return;
        }

        // Wake up after calculated delay (C++ line 144)
        let delay = self.calc_next_spread_delay();
        ctx.set_wake_frame(object.id(), UpdateSleepTime::Frames(delay));
    }

    fn wake_delay_if_aflame(&mut self, is_aflame: bool) -> Option<u32> {
        is_aflame.then(|| self.calc_next_spread_delay())
    }

    /// Calculate next spread delay with randomization
    /// Matches C++ FireSpreadUpdate.cpp:149-156
    fn calc_next_spread_delay(&self) -> u32 {
        let delay = game_logic_random_value(
            self.module_data.min_spread_try_delay,
            self.module_data.max_spread_try_delay,
        );

        // Ensure at least 1 frame delay (C++ line 153-154)
        delay.max(1)
    }

    /// Save state to xfer
    /// Matches C++ FireSpreadUpdate.cpp:174-184
    pub fn save(&self, xfer: &mut dyn Xfer) {
        xfer.xfer_version_write(1);
        // No instance data to save, only module data
    }

    /// Load state from xfer
    /// Matches C++ FireSpreadUpdate.cpp:174-184
    pub fn load(&mut self, xfer: &mut dyn Xfer) {
        let version = xfer.xfer_version_read();
        if version >= 1 {
            // No instance data to load
        }
    }
}

impl Snapshotable for FireSpreadUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        let mut next_call_frame_and_phase = self.next_call_frame_and_phase;
        xfer_update_module_base_state(xfer, &mut next_call_frame_and_phase)?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes FireSpreadUpdate through the common Module trait.
pub struct FireSpreadUpdateModule {
    behavior: FireSpreadUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<FireSpreadUpdateModuleData>,
}

impl FireSpreadUpdateModule {
    pub fn new(
        behavior: FireSpreadUpdate,
        module_name: &AsciiString,
        module_data: Arc<FireSpreadUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut FireSpreadUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for FireSpreadUpdateModule {
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

impl Module for FireSpreadUpdateModule {
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
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }

    fn get_fire_spread_control_interface(&mut self) -> Option<&mut dyn FireSpreadControlInterface> {
        Some(self)
    }
}

impl FireSpreadControlInterface for FireSpreadUpdateModule {
    fn wake_delay_if_aflame(&mut self, is_aflame: bool) -> Option<u32> {
        self.behavior.wake_delay_if_aflame(is_aflame)
    }
}

trait FlammableUpdateGlobalExt {
    fn try_to_ignite_without_context(&self);
}

impl FlammableUpdateGlobalExt
    for Arc<std::sync::Mutex<dyn crate::modules::BehaviorModuleInterface>>
{
    fn try_to_ignite_without_context(&self) {
        if let Ok(mut guard) = self.lock() {
            guard.try_to_ignite_flammable();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fire_spread_creation() {
        let data = FireSpreadUpdateModuleData {
            module_tag_name_key: 0,
            min_spread_try_delay: 30,
            max_spread_try_delay: 60,
            spread_try_range: 50.0,
            ocl_embers: None,
        };

        let update = FireSpreadUpdate::new(1, data);
        assert_eq!(update.module_data.min_spread_try_delay, 30);
        assert_eq!(update.module_data.max_spread_try_delay, 60);
        assert_eq!(update.module_data.spread_try_range, 50.0);
    }

    #[test]
    fn test_calc_next_spread_delay() {
        let data = FireSpreadUpdateModuleData {
            module_tag_name_key: 0,
            min_spread_try_delay: 10,
            max_spread_try_delay: 20,
            spread_try_range: 100.0,
            ocl_embers: None,
        };

        let update = FireSpreadUpdate::new(1, data);

        // Test multiple times to verify randomness
        for _ in 0..10 {
            let delay = update.calc_next_spread_delay();
            assert!(delay >= 10 && delay <= 20, "Delay {} out of range", delay);
        }
    }

    #[test]
    fn test_minimum_delay() {
        let data = FireSpreadUpdateModuleData {
            module_tag_name_key: 0,
            min_spread_try_delay: 0,
            max_spread_try_delay: 0,
            spread_try_range: 50.0,
            ocl_embers: None,
        };

        let update = FireSpreadUpdate::new(1, data);
        let delay = update.calc_next_spread_delay();

        // Should be at least 1 (C++ line 153-154)
        assert_eq!(delay, 1);
    }
}

fn game_logic_random_value(min: u32, max: u32) -> u32 {
    if min >= max {
        return min;
    }
    crate::helpers::game_logic_random_value(min, max)
}
